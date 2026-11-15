use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::git::runner::run_git;
use crate::git::status::{parse_status, RepositorySnapshot};

/// 从工作树内的任意目录打开已有仓库。
pub async fn open_repository(path: &Path) -> AppResult<RepositorySnapshot> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map_err(|error| AppError::from_io("INVALID_PATH", "无法访问所选目录", &error))?;

    if !canonical.is_dir() {
        return Err(AppError::new("INVALID_PATH", "所选路径不是目录"));
    }

    let output = run_git(&canonical, &["rev-parse", "--show-toplevel"]).await?;
    if !output.status.success() {
        return Err(non_repository_error(&output.stderr));
    }

    let root = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches(['\r', '\n'])
        .to_string();
    repository_status(Path::new(&root)).await
}

/// 读取已有仓库的分支元数据和变更路径。
pub async fn repository_status(path: &Path) -> AppResult<RepositorySnapshot> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map_err(|error| AppError::from_io("INVALID_PATH", "无法访问仓库目录", &error))?;
    let output = run_git(&canonical, &["status", "--porcelain=v2", "--branch", "-z"]).await?;

    if !output.status.success() {
        return Err(non_repository_error(&output.stderr));
    }

    let mut snapshot = parse_status(&output.stdout);
    snapshot.repository.root = display_path(&canonical);
    snapshot.repository.name = repository_name(&canonical);
    Ok(snapshot)
}

fn non_repository_error(stderr: &[u8]) -> AppError {
    AppError::new("NOT_REPOSITORY", "所选目录不是有效的 Git 仓库")
        .with_details(String::from_utf8_lossy(stderr).trim())
}

fn repository_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            PathBuf::from(root)
                .to_string_lossy()
                .trim_end_matches(['\\', '/'])
                .to_string()
        })
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc}");
        }
        if let Some(local) = value.strip_prefix(r"\\?\") {
            return local.to_string();
        }
    }
    value
}
