use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::git::runner::{run_git, run_git_tracked, GitOutput};
use crate::operations::OperationRegistry;

/// Pull 或 Push 完成后返回给前端的 Git 输出。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationResult {
    /// Git 的合并 stdout/stderr 可读输出。
    pub output: String,
}

/// 仅快进拉取当前分支的 upstream，不自动合并、rebase 或 stash。
pub async fn pull(
    repository: &Path,
    operation_id: &str,
    registry: &OperationRegistry,
) -> AppResult<OperationResult> {
    let repository = canonical_repository(repository).await?;
    let lock = registry.repository_lock(&repository).await;
    let _guard = lock.lock().await;
    ensure_upstream(&repository).await?;

    let output =
        run_git_tracked(&repository, &["pull", "--ff-only"], operation_id, registry).await?;
    success_output(output, "拉取失败")
}

/// 推送当前分支；仅在用户确认后为 origin 设置首次 upstream。
pub async fn push(
    repository: &Path,
    set_upstream: bool,
    operation_id: &str,
    registry: &OperationRegistry,
) -> AppResult<OperationResult> {
    let repository = canonical_repository(repository).await?;
    let lock = registry.repository_lock(&repository).await;
    let _guard = lock.lock().await;

    if !set_upstream {
        ensure_upstream(&repository).await?;
    }
    let args = if set_upstream {
        vec!["push", "-u", "origin", "HEAD"]
    } else {
        vec!["push"]
    };
    let output = run_git_tracked(&repository, &args, operation_id, registry).await?;
    success_output(output, "推送失败")
}

/// 取消由 operation ID 标识的 Pull、Push 或提交进程。
pub use crate::operations::cancel_operation;

async fn canonical_repository(repository: &Path) -> AppResult<PathBuf> {
    tokio::fs::canonicalize(repository)
        .await
        .map_err(|error| AppError::from_io("INVALID_PATH", "无法访问仓库目录", &error))
}

async fn ensure_upstream(repository: &Path) -> AppResult<()> {
    let output = run_git(
        repository,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .await?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::new(
            "UPSTREAM_MISSING",
            "当前分支尚未配置 upstream，请先完成首次 Push",
        ))
    }
}

fn success_output(output: GitOutput, message: &str) -> AppResult<OperationResult> {
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Ok(OperationResult {
            output: format!("{stdout}{stderr}").trim().to_string(),
        })
    } else {
        Err(map_sync_error(&output, message))
    }
}

fn map_sync_error(output: &GitOutput, message: &str) -> AppError {
    let details = output.details();
    let lower = details.to_lowercase();
    let code = if lower.contains("non-fast-forward")
        || lower.contains("fetch first")
        || (lower.contains("rejected") && lower.contains("failed to push"))
    {
        "NON_FAST_FORWARD"
    } else if lower.contains("authentication failed")
        || lower.contains("could not read username")
        || lower.contains("permission denied (publickey)")
        || lower.contains("terminal prompts disabled")
        || lower.contains("invalid username or password")
    {
        "AUTH_FAILED"
    } else {
        "GIT_FAILED"
    };
    AppError::new(code, message).with_details(details)
}
