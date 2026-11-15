use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::ai::{generate_commit as generate_with_model, CommitContext};
use crate::error::{AppError, AppResult};
use crate::git::{self, CommitResult, RepositorySnapshot};
use crate::operations::OperationRegistry;
use crate::settings::{
    AppSettings, ModelProfileInput, SecretStore, SettingsStore, SystemSecretStore,
};

/// Tauri command 层共享的设置、密钥、模型客户端和操作注册表。
pub struct AppState {
    /// 非敏感设置和全局提交规则。
    pub settings: SettingsStore,
    /// 操作系统秘密存储。
    pub secrets: Arc<dyn SecretStore>,
    /// 复用连接池的模型 HTTP 客户端。
    pub ai_client: reqwest::Client,
    /// 同仓库串行锁和取消句柄。
    pub operations: OperationRegistry,
}

impl AppState {
    /// 构造命令层状态。
    pub fn new(settings: SettingsStore) -> AppResult<Self> {
        let ai_client = reqwest::Client::builder()
            .user_agent("SimpleGit/0.1")
            .build()
            .map_err(|error| {
                AppError::new("MODEL_REQUEST_FAILED", "无法初始化模型客户端")
                    .with_details(error.to_string())
            })?;
        Ok(Self {
            settings,
            secrets: Arc::new(SystemSecretStore::new("com.simplegit.desktop")),
            ai_client,
            operations: OperationRegistry::new(),
        })
    }
}

/// 模型连接测试返回的示例输出。
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionTestResult {
    /// 模型成功返回的文本。
    pub output: String,
}

/// 应用诊断信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    /// `git --version` 输出。
    pub git_version: String,
    /// 从 PATH 找到的 Git 可执行文件。
    pub git_path: String,
    /// SimpleGit 当前版本。
    pub app_version: String,
}

/// 打开已有仓库并返回状态快照。
#[tauri::command]
pub async fn open_repository(path: String) -> AppResult<RepositorySnapshot> {
    git::open_repository(Path::new(&path)).await
}

/// 刷新指定仓库状态。
#[tauri::command]
pub async fn repository_status(path: String) -> AppResult<RepositorySnapshot> {
    git::repository_status(Path::new(&path)).await
}

/// 暂存选中的仓库路径并返回新状态。
#[tauri::command]
pub async fn stage_selected(
    path: String,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> AppResult<RepositorySnapshot> {
    let lock = state.operations.repository_lock(Path::new(&path)).await;
    let _guard = lock.lock().await;
    git::stage_paths(Path::new(&path), &paths).await?;
    git::repository_status(Path::new(&path)).await
}

/// 暂存全部更改并返回新状态。
#[tauri::command]
pub async fn stage_all(path: String, state: State<'_, AppState>) -> AppResult<RepositorySnapshot> {
    let lock = state.operations.repository_lock(Path::new(&path)).await;
    let _guard = lock.lock().await;
    git::stage_all(Path::new(&path)).await?;
    git::repository_status(Path::new(&path)).await
}

/// 取消暂存选中的仓库路径并返回新状态。
#[tauri::command]
pub async fn unstage_selected(
    path: String,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> AppResult<RepositorySnapshot> {
    let lock = state.operations.repository_lock(Path::new(&path)).await;
    let _guard = lock.lock().await;
    git::unstage_paths(Path::new(&path), &paths).await?;
    git::repository_status(Path::new(&path)).await
}

/// 读取单个文件在工作区或索引中的 diff。
#[tauri::command]
pub async fn read_diff(path: String, file_path: String, staged: bool) -> AppResult<String> {
    git::file_diff(Path::new(&path), &file_path, staged).await
}

/// 提交当前已暂存内容，并允许取消耗时 hook。
#[tauri::command]
pub async fn commit(
    repository_path: String,
    message: String,
    operation_id: String,
    state: State<'_, AppState>,
) -> AppResult<CommitResult> {
    git::commit_tracked(
        Path::new(&repository_path),
        &message,
        &operation_id,
        &state.operations,
    )
    .await
}

/// 仅快进拉取当前分支。
#[tauri::command]
pub async fn pull(
    repository_path: String,
    operation_id: String,
    state: State<'_, AppState>,
) -> AppResult<git::OperationResult> {
    git::pull(
        Path::new(&repository_path),
        &operation_id,
        &state.operations,
    )
    .await
}

/// 推送当前分支，必要时设置 origin upstream。
#[tauri::command]
pub async fn push(
    repository_path: String,
    set_upstream: bool,
    operation_id: String,
    state: State<'_, AppState>,
) -> AppResult<git::OperationResult> {
    git::push(
        Path::new(&repository_path),
        set_upstream,
        &operation_id,
        &state.operations,
    )
    .await
}

/// 取消进行中的 Git 操作。
#[tauri::command]
pub async fn cancel_operation(operation_id: String, state: State<'_, AppState>) -> AppResult<bool> {
    git::cancel_operation(&operation_id, &state.operations).await
}

/// 加载模型、规则和最近仓库设置。
#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.settings.get(&*state.secrets).await
}

/// 新增或更新一个模型档案。
#[tauri::command]
pub async fn save_model_profile(
    input: ModelProfileInput,
    state: State<'_, AppState>,
) -> AppResult<AppSettings> {
    state.settings.save_model(input, &*state.secrets).await
}

/// 删除模型档案及其系统密钥。
#[tauri::command]
pub async fn delete_model_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> AppResult<AppSettings> {
    state
        .settings
        .delete_model(&profile_id, &*state.secrets)
        .await
}

/// 切换当前活动模型。
#[tauri::command]
pub async fn set_active_model(
    profile_id: Option<String>,
    state: State<'_, AppState>,
) -> AppResult<AppSettings> {
    state.settings.set_active_model(profile_id).await
}

/// 保存唯一一套全局提交规则。
#[tauri::command]
pub async fn save_commit_rule(rule: String, state: State<'_, AppState>) -> AppResult<AppSettings> {
    state.settings.save_commit_rule(&rule).await
}

/// 使用一个极小的真实请求测试模型档案连接。
#[tauri::command]
pub async fn test_model_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> AppResult<ConnectionTestResult> {
    let settings = state.settings.get(&*state.secrets).await?;
    let profile = find_profile(&settings, Some(&profile_id))?;
    let api_key = state.secrets.get(&profile.id)?;
    let context = CommitContext {
        stage_diff: "+ SimpleGit model connection test".to_string(),
        branch: "model-test".to_string(),
        recent_subjects: Vec::new(),
        rule: "只输出 test(ai): 模型连接成功".to_string(),
    };
    let output =
        generate_with_model(&state.ai_client, profile, api_key.as_deref(), &context).await?;
    Ok(ConnectionTestResult { output })
}

/// 使用当前规则和指定模型为已暂存内容生成提交说明。
#[tauri::command]
pub async fn generate_commit(
    repository_path: String,
    model_id: String,
    operation_id: String,
    state: State<'_, AppState>,
) -> AppResult<String> {
    let settings = state.settings.get(&*state.secrets).await?;
    let profile = find_profile(&settings, Some(&model_id))?;
    let api_key = state.secrets.get(&profile.id)?;
    let repository = Path::new(&repository_path);
    let lock = state.operations.repository_lock(repository).await;
    let (snapshot, diff, subjects) = {
        let _guard = lock.lock().await;
        let snapshot = git::repository_status(repository).await?;
        if snapshot.repository.has_conflicts {
            return Err(AppError::new(
                "CONFLICTS_PRESENT",
                "存在未解决的冲突，无法生成提交说明",
            ));
        }
        let diff = git::staged_diff(repository).await?;
        let subjects = git::recent_subjects(repository, 8).await?;
        (snapshot, diff, subjects)
    };

    let context = CommitContext {
        stage_diff: diff,
        branch: snapshot.repository.branch,
        recent_subjects: subjects,
        rule: settings.commit_rule.clone(),
    };
    let cancellation = state.operations.register_task(&operation_id).await?;
    let result = tokio::select! {
        result = generate_with_model(
            &state.ai_client,
            profile,
            api_key.as_deref(),
            &context,
        ) => result,
        _ = cancellation.cancelled() => Err(AppError::new(
            "OPERATION_CANCELLED",
            "模型生成已取消",
        )),
    };
    state.operations.finish_task(&operation_id).await;
    result
}

/// 把路径放到最近仓库列表首位。
#[tauri::command]
pub async fn record_recent_repository(
    path: String,
    state: State<'_, AppState>,
) -> AppResult<AppSettings> {
    state
        .settings
        .record_recent_repository(PathBuf::from(path))
        .await
}

/// 返回 Git 和应用版本诊断信息。
#[tauri::command]
pub async fn diagnostics() -> Diagnostics {
    let git_path = find_git_path();
    let mut command = Command::new(&git_path);
    command.arg("--version");
    crate::child_process::configure_background_command(&mut command);
    let git_version = command
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "Git 不可用".to_string());
    Diagnostics {
        git_version,
        git_path,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

fn find_profile<'a>(
    settings: &'a AppSettings,
    requested_id: Option<&str>,
) -> AppResult<&'a crate::settings::ModelProfile> {
    let id = requested_id
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| settings.active_model_id.clone())
        .ok_or_else(|| AppError::new("MODEL_NOT_CONFIGURED", "尚未配置可用的模型档案"))?;
    settings
        .models
        .iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| AppError::new("MODEL_NOT_CONFIGURED", "模型档案不存在"))
}

fn find_git_path() -> String {
    let executable = if cfg!(windows) { "git.exe" } else { "git" };
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join(executable);
            if candidate.is_file() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    executable.to_string()
}
