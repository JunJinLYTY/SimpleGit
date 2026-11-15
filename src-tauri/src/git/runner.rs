use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::Ordering;
use std::sync::OnceLock;

use regex::Regex;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::operations::OperationRegistry;

/// 保留 Git 子进程的字节输出，供上层进行机器解析。
#[derive(Debug)]
pub struct GitOutput {
    /// Git 进程退出状态。
    pub status: ExitStatus,
    /// 保留原始字节的标准输出。
    pub stdout: Vec<u8>,
    /// 保留原始字节的标准错误。
    pub stderr: Vec<u8>,
}

impl GitOutput {
    /// 返回合并后的可读技术输出，供错误详情使用。
    pub fn details(&self) -> String {
        let stdout = String::from_utf8_lossy(&self.stdout);
        let stderr = String::from_utf8_lossy(&self.stderr);
        redact_credentials(format!("{stdout}{stderr}").trim())
    }
}

/// 在指定工作目录执行 Git，不经过 shell 或命令字符串拼接。
pub async fn run_git(repository: &Path, args: &[&str]) -> AppResult<GitOutput> {
    let output = command(repository)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(map_spawn_error)?;
    Ok(GitOutput {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

/// 将字节输入写入 Git stdin，把多行消息与命令参数彻底分离。
pub async fn run_git_with_input(
    repository: &Path,
    args: &[&str],
    input: &[u8],
) -> AppResult<GitOutput> {
    let mut child = command(repository)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(map_spawn_error)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::new("GIT_FAILED", "无法打开 Git 标准输入"))?;
    stdin
        .write_all(input)
        .await
        .map_err(|error| AppError::from_io("GIT_FAILED", "无法写入 Git 标准输入", &error))?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .await
        .map_err(|error| AppError::from_io("GIT_FAILED", "等待 Git 完成时失败", &error))?;
    Ok(GitOutput {
        status: output.status,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

/// 执行会注册进程 ID 的长操作，并在取消时终止完整 Git 进程树。
pub async fn run_git_tracked(
    repository: &Path,
    args: &[&str],
    operation_id: &str,
    registry: &OperationRegistry,
) -> AppResult<GitOutput> {
    let child = command(repository)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(map_spawn_error)?;
    wait_tracked(child, operation_id, registry).await
}

/// 将输入写入 stdin，并为 hook 等长耗时阶段注册取消能力。
pub async fn run_git_with_input_tracked(
    repository: &Path,
    args: &[&str],
    input: &[u8],
    operation_id: &str,
    registry: &OperationRegistry,
) -> AppResult<GitOutput> {
    let mut child = command(repository)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(map_spawn_error)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::new("GIT_FAILED", "无法打开 Git 标准输入"))?;
    stdin
        .write_all(input)
        .await
        .map_err(|error| AppError::from_io("GIT_FAILED", "无法写入 Git 标准输入", &error))?;
    drop(stdin);
    wait_tracked(child, operation_id, registry).await
}

async fn wait_tracked(
    mut child: tokio::process::Child,
    operation_id: &str,
    registry: &OperationRegistry,
) -> AppResult<GitOutput> {
    let pid = child
        .id()
        .ok_or_else(|| AppError::new("GIT_FAILED", "无法读取 Git 进程标识"))?;
    let (cancelled, mut cancel_receiver) = match registry.register(operation_id, pid).await {
        Ok(registration) => registration,
        Err(error) => {
            let _ = child.kill().await;
            return Err(error);
        }
    };

    if cancelled.load(Ordering::SeqCst) {
        let _ = child.kill().await;
        registry.finish(operation_id).await;
        return Err(AppError::new("OPERATION_CANCELLED", "操作已取消"));
    }

    let waiting = child.wait_with_output();
    tokio::pin!(waiting);
    tokio::select! {
        output = &mut waiting => {
            registry.finish(operation_id).await;
            if cancelled.load(Ordering::SeqCst) {
                return Err(AppError::new("OPERATION_CANCELLED", "操作已取消"));
            }
            let output = output
                .map_err(|error| AppError::from_io("GIT_FAILED", "等待 Git 完成时失败", &error))?;
            Ok(GitOutput {
                status: output.status,
                stdout: output.stdout,
                stderr: output.stderr,
            })
        }
        _ = &mut cancel_receiver => {
            registry.finish(operation_id).await;
            Err(AppError::new("OPERATION_CANCELLED", "操作已取消"))
        }
    }
}

fn command(repository: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(repository)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .kill_on_drop(true);
    crate::child_process::configure_background_command(command.as_std_mut());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    command
}

fn map_spawn_error(error: std::io::Error) -> AppError {
    if error.kind() == std::io::ErrorKind::NotFound {
        AppError::new("GIT_NOT_INSTALLED", "未找到 Git，请先安装并配置系统 Git")
    } else {
        AppError::from_io("GIT_FAILED", "无法启动 Git", &error)
    }
}

fn redact_credentials(details: &str) -> String {
    static URL_CREDENTIALS: OnceLock<Regex> = OnceLock::new();
    URL_CREDENTIALS
        .get_or_init(|| {
            Regex::new(r"(?i)(https?|ssh)://[^/\s@]+@")
                .expect("credential redaction regex must compile")
        })
        .replace_all(details, "$1://***@")
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::redact_credentials;

    #[test]
    fn redacts_credentials_from_remote_urls() {
        let details = "fatal: unable to access 'https://user:token@example.test/repo.git'";

        assert_eq!(
            redact_credentials(details),
            "fatal: unable to access 'https://***@example.test/repo.git'"
        );
    }
}
