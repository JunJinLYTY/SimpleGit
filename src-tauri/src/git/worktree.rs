use std::path::{Component, Path};

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::git::runner::{run_git, run_git_with_input, run_git_with_input_tracked, GitOutput};
use crate::git::service::repository_status;
use crate::operations::OperationRegistry;

/// 一次成功提交的标识和摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    /// 新提交对象的完整 SHA。
    pub hash: String,
    /// 新提交的摘要行。
    pub summary: String,
}

/// 校验仓库相对路径，拒绝绝对路径、父目录跳转和 NUL。
pub fn validate_relative_path(path: &str) -> AppResult<()> {
    if path.is_empty() || path.contains('\0') {
        return Err(AppError::new("INVALID_PATH", "文件路径不能为空"));
    }

    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(AppError::new("INVALID_PATH", "文件路径必须位于当前仓库内"));
    }

    Ok(())
}

/// 暂存用户选中的文件、删除记录或未跟踪文件。
pub async fn stage_paths(repository: &Path, paths: &[String]) -> AppResult<()> {
    if paths.is_empty() {
        return Ok(());
    }
    validate_paths(paths)?;
    let args = add_args(paths);
    ensure_success(run_git(repository, &borrowed_args(&args)).await?)?;
    Ok(())
}

/// 暂存工作区中的全部更改，包括删除和未跟踪文件。
pub async fn stage_all(repository: &Path) -> AppResult<()> {
    ensure_success(run_git(repository, &["add", "-A"]).await?)?;
    Ok(())
}

/// 取消暂存选中路径，并兼容尚无首次提交的仓库。
pub async fn unstage_paths(repository: &Path, paths: &[String]) -> AppResult<()> {
    if paths.is_empty() {
        return Ok(());
    }
    validate_paths(paths)?;
    let has_head = run_git(repository, &["rev-parse", "--verify", "HEAD"])
        .await?
        .status
        .success();
    let mut args = if has_head {
        vec![
            "restore".to_string(),
            "--staged".to_string(),
            "--".to_string(),
        ]
    } else {
        vec![
            "rm".to_string(),
            "--cached".to_string(),
            "--ignore-unmatch".to_string(),
            "--".to_string(),
        ]
    };
    args.extend(paths.iter().cloned());
    ensure_success(run_git(repository, &borrowed_args(&args)).await?)?;
    Ok(())
}

/// 读取指定路径已暂存或工作区中的文本 diff。
pub async fn file_diff(repository: &Path, path: &str, staged: bool) -> AppResult<String> {
    validate_relative_path(path)?;
    let mut args = vec!["diff", "--no-ext-diff", "--unified=3"];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    let output = run_git(repository, &args).await?;
    let stdout = ensure_success(output)?;
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

/// 读取全部已暂存内容的统一 diff，供提交说明生成使用。
pub async fn staged_diff(repository: &Path) -> AppResult<String> {
    let output = run_git(
        repository,
        &["diff", "--cached", "--no-ext-diff", "--unified=3"],
    )
    .await?;
    let stdout = ensure_success(output)?;
    let diff = String::from_utf8_lossy(&stdout).into_owned();
    if diff.trim().is_empty() {
        return Err(AppError::new("NO_STAGED_CHANGES", "没有已暂存的文件"));
    }
    Ok(diff)
}

/// 读取最近提交摘要；尚无首次提交时返回空列表。
pub async fn recent_subjects(repository: &Path, limit: usize) -> AppResult<Vec<String>> {
    let count = format!("-{limit}");
    let output = run_git(repository, &["log", &count, "--pretty=%s"]).await?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(ToString::to_string)
        .collect())
}

/// 使用 stdin 提交当前已暂存内容，并返回新提交信息。
pub async fn commit(repository: &Path, message: &str) -> AppResult<CommitResult> {
    validate_commit_request(repository, message).await?;
    let output = run_git_with_input(
        repository,
        &["commit", "--cleanup=whitespace", "-F", "-"],
        message.as_bytes(),
    )
    .await?;
    ensure_success(output)?;
    read_commit_result(repository).await
}

/// 在仓库写锁内提交，并允许取消耗时 hook。
pub async fn commit_tracked(
    repository: &Path,
    message: &str,
    operation_id: &str,
    registry: &OperationRegistry,
) -> AppResult<CommitResult> {
    let lock = registry.repository_lock(repository).await;
    let _guard = lock.lock().await;
    validate_commit_request(repository, message).await?;
    let output = run_git_with_input_tracked(
        repository,
        &["commit", "--cleanup=whitespace", "-F", "-"],
        message.as_bytes(),
        operation_id,
        registry,
    )
    .await?;
    ensure_success(output)?;
    read_commit_result(repository).await
}

async fn validate_commit_request(repository: &Path, message: &str) -> AppResult<()> {
    if message.trim().is_empty() {
        return Err(AppError::new("GIT_FAILED", "提交说明不能为空"));
    }
    let snapshot = repository_status(repository).await?;
    if snapshot.repository.has_conflicts {
        return Err(AppError::new(
            "CONFLICTS_PRESENT",
            "存在未解决的冲突，无法提交",
        ));
    }
    if !snapshot.changes.iter().any(|change| change.staged) {
        return Err(AppError::new("NO_STAGED_CHANGES", "没有已暂存的文件"));
    }
    Ok(())
}

async fn read_commit_result(repository: &Path) -> AppResult<CommitResult> {
    let hash_output = run_git(repository, &["rev-parse", "HEAD"]).await?;
    let hash = ensure_success(hash_output)?;
    let summary_output = run_git(repository, &["log", "-1", "--pretty=%s"]).await?;
    let summary = ensure_success(summary_output)?;

    Ok(CommitResult {
        hash: String::from_utf8_lossy(&hash).trim().to_string(),
        summary: String::from_utf8_lossy(&summary).trim().to_string(),
    })
}

fn validate_paths(paths: &[String]) -> AppResult<()> {
    for path in paths {
        validate_relative_path(path)?;
    }
    Ok(())
}

fn add_args(paths: &[String]) -> Vec<String> {
    let mut args = vec!["add".to_string(), "-A".to_string(), "--".to_string()];
    args.extend(paths.iter().cloned());
    args
}

fn borrowed_args(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

fn ensure_success(output: GitOutput) -> AppResult<Vec<u8>> {
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(AppError::new("GIT_FAILED", "Git 操作失败").with_details(output.details()))
    }
}
