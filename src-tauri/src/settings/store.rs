use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

use super::secrets::SecretStore;
use super::{AppSettings, ModelProfile, ModelProfileInput};

/// 非敏感设置的内存状态和原子 JSON 持久化入口。
#[derive(Debug)]
pub struct SettingsStore {
    /// 设置 JSON 的完整路径。
    file_path: PathBuf,
    /// 当前设置快照。
    settings: Mutex<AppSettings>,
}

impl SettingsStore {
    /// 加载设置文件；文件不存在时使用默认设置，损坏时拒绝覆盖。
    pub async fn load(file_path: PathBuf) -> AppResult<Self> {
        let settings = match tokio::fs::read(&file_path).await {
            Ok(bytes) => serde_json::from_slice::<AppSettings>(&bytes)
                .map_err(|error| settings_error("设置文件格式无效", error))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
            Err(error) => {
                return Err(AppError::from_io(
                    "SETTINGS_FAILED",
                    "无法读取设置文件",
                    &error,
                ));
            }
        };

        Ok(Self {
            file_path,
            settings: Mutex::new(settings),
        })
    }

    /// 返回一份带有密钥存在状态的设置副本。
    pub async fn get(&self, secrets: &dyn SecretStore) -> AppResult<AppSettings> {
        let mut settings = self.settings.lock().await.clone();
        for profile in &mut settings.models {
            profile.has_api_key = secrets.has(&profile.id)?;
        }
        Ok(settings)
    }

    /// 新增或更新模型档案，并管理活动档案、默认档案和密钥。
    pub async fn save_model(
        &self,
        input: ModelProfileInput,
        secrets: &dyn SecretStore,
    ) -> AppResult<AppSettings> {
        validate_profile(&input)?;
        let mut settings = self.settings.lock().await;
        let existing_index = match input.id.as_deref() {
            Some(id) => Some(
                settings
                    .models
                    .iter()
                    .position(|profile| profile.id == id)
                    .ok_or_else(|| AppError::new("SETTINGS_FAILED", "模型档案不存在"))?,
            ),
            None => None,
        };
        let id = existing_index
            .map(|index| settings.models[index].id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        if input.is_default {
            for profile in &mut settings.models {
                profile.is_default = false;
            }
        }
        let profile = ModelProfile {
            id: id.clone(),
            name: input.name.trim().to_string(),
            base_url: input.base_url.trim().trim_end_matches('/').to_string(),
            model: input.model.trim().to_string(),
            timeout_seconds: input.timeout_seconds.clamp(1, 600),
            is_default: input.is_default,
            has_api_key: false,
        };
        match existing_index {
            Some(index) => settings.models[index] = profile,
            None => settings.models.push(profile),
        }
        if settings.active_model_id.is_none() {
            settings.active_model_id = Some(id.clone());
        }
        normalize_selection(&mut settings);
        self.persist_locked(&settings).await?;

        if input.clear_api_key {
            secrets.clear(&id)?;
        } else if let Some(api_key) = input.api_key.filter(|value| !value.trim().is_empty()) {
            secrets.set(&id, api_key.trim())?;
        }
        drop(settings);
        self.get(secrets).await
    }

    /// 删除模型档案，并清理对应密钥和选择状态。
    pub async fn delete_model(
        &self,
        profile_id: &str,
        secrets: &dyn SecretStore,
    ) -> AppResult<AppSettings> {
        let mut settings = self.settings.lock().await;
        let previous_len = settings.models.len();
        settings.models.retain(|profile| profile.id != profile_id);
        if settings.models.len() == previous_len {
            return Err(AppError::new("SETTINGS_FAILED", "模型档案不存在"));
        }
        secrets.clear(profile_id)?;
        normalize_selection(&mut settings);
        self.persist_locked(&settings).await?;
        drop(settings);
        self.get(secrets).await
    }

    /// 设置当前活动模型；传入不存在档案时返回错误。
    pub async fn set_active_model(&self, profile_id: Option<String>) -> AppResult<AppSettings> {
        let mut settings = self.settings.lock().await;
        if let Some(id) = profile_id.as_deref() {
            if !settings.models.iter().any(|profile| profile.id == id) {
                return Err(AppError::new("SETTINGS_FAILED", "模型档案不存在"));
            }
        }
        settings.active_model_id = profile_id;
        normalize_selection(&mut settings);
        self.persist_locked(&settings).await?;
        Ok(settings.clone())
    }

    /// 保存唯一一套全局提交规则。
    pub async fn save_commit_rule(&self, rule: &str) -> AppResult<AppSettings> {
        let mut settings = self.settings.lock().await;
        settings.commit_rule = rule.trim().to_string();
        self.persist_locked(&settings).await?;
        Ok(settings.clone())
    }

    /// 将仓库路径放到最近列表首位，去重并限制为十条。
    pub async fn record_recent_repository(&self, path: PathBuf) -> AppResult<AppSettings> {
        let value = path.to_string_lossy().into_owned();
        let mut settings = self.settings.lock().await;
        settings
            .recent_repositories
            .retain(|existing| existing != &value);
        settings.recent_repositories.insert(0, value);
        settings.recent_repositories.truncate(10);
        self.persist_locked(&settings).await?;
        Ok(settings.clone())
    }

    async fn persist_locked(&self, settings: &AppSettings) -> AppResult<()> {
        let path = self.file_path.clone();
        let bytes = serde_json::to_vec_pretty(settings)
            .map_err(|error| settings_error("无法序列化设置", error))?;
        tokio::task::spawn_blocking(move || write_atomic(&path, &bytes))
            .await
            .map_err(|error| {
                AppError::new("SETTINGS_FAILED", "设置持久化任务失败")
                    .with_details(error.to_string())
            })?
            .map_err(|error| AppError::from_io("SETTINGS_FAILED", "无法保存设置文件", &error))
    }
}

fn validate_profile(input: &ModelProfileInput) -> AppResult<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::new("SETTINGS_FAILED", "配置名称不能为空"));
    }
    if input.model.trim().is_empty() {
        return Err(AppError::new("SETTINGS_FAILED", "Model ID 不能为空"));
    }
    if !(input.base_url.starts_with("http://") || input.base_url.starts_with("https://")) {
        return Err(AppError::new(
            "SETTINGS_FAILED",
            "Base URL 必须以 http:// 或 https:// 开头",
        ));
    }
    Ok(())
}

fn normalize_selection(settings: &mut AppSettings) {
    if settings.models.is_empty() {
        settings.active_model_id = None;
        return;
    }
    if !settings.models.iter().any(|profile| profile.is_default) {
        let default_id = settings
            .active_model_id
            .clone()
            .filter(|id| settings.models.iter().any(|profile| &profile.id == id))
            .unwrap_or_else(|| settings.models[0].id.clone());
        if let Some(profile) = settings
            .models
            .iter_mut()
            .find(|profile| profile.id == default_id)
        {
            profile.is_default = true;
        }
    }
    if settings
        .active_model_id
        .as_ref()
        .is_none_or(|id| !settings.models.iter().any(|profile| &profile.id == id))
    {
        settings.active_model_id = settings
            .models
            .iter()
            .find(|profile| profile.is_default)
            .or_else(|| settings.models.first())
            .map(|profile| profile.id.clone());
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    match temporary.persist(path) {
        Ok(_) => Ok(()),
        Err(persist_error) if persist_error.error.kind() == std::io::ErrorKind::CrossesDevices => {
            // Windows 加密漫游目录可能拒绝 MoveFileEx；此时保留内容复制作为可用性回退。
            let mut source = persist_error.file.reopen()?;
            let mut destination = fs::File::create(path)?;
            std::io::copy(&mut source, &mut destination)?;
            destination.sync_all()
        }
        Err(persist_error) => Err(persist_error.error),
    }
}

fn settings_error(message: &str, error: serde_json::Error) -> AppError {
    AppError::new("SETTINGS_FAILED", message).with_details(error.to_string())
}

#[cfg(all(test, windows))]
mod tests {
    use std::fs;

    use super::write_atomic;

    #[test]
    fn settings_write_handles_encrypted_roaming_app_data() {
        let app_data = std::env::var_os("APPDATA").expect("APPDATA");
        let directory = std::path::PathBuf::from(app_data)
            .join("com.simplegit.desktop")
            .join(format!("write-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("settings.json");

        write_atomic(&path, b"{\"version\":1}").unwrap();
        write_atomic(&path, b"{\"version\":2}").unwrap();
        let result = fs::read(&path).unwrap();

        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(result, b"{\"version\":2}");
    }
}
