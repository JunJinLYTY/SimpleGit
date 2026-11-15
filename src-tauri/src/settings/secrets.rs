use keyring::Entry;

use crate::error::{AppError, AppResult};

/// 将模型密钥与普通设置分离保存的抽象。
pub trait SecretStore: Send + Sync {
    /// 写入或替换指定档案的密钥。
    fn set(&self, profile_id: &str, secret: &str) -> AppResult<()>;
    /// 删除指定档案的密钥，密钥不存在时视为成功。
    fn clear(&self, profile_id: &str) -> AppResult<()>;
    /// 判断指定档案是否已有密钥。
    fn has(&self, profile_id: &str) -> AppResult<bool>;
    /// 读取指定档案的密钥，供 Rust 侧模型请求使用。
    fn get(&self, profile_id: &str) -> AppResult<Option<String>>;
}

/// 使用 Windows Credential Manager 或 macOS Keychain 的系统秘密存储。
pub struct SystemSecretStore {
    /// 写入系统凭据库的应用服务名。
    service: String,
}

impl SystemSecretStore {
    /// 创建使用应用标识作为服务名的系统秘密存储。
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, profile_id: &str) -> AppResult<Entry> {
        Entry::new(&self.service, &format!("model:{profile_id}"))
            .map_err(|error| secret_error("无法访问系统凭据库", error))
    }
}

impl SecretStore for SystemSecretStore {
    fn set(&self, profile_id: &str, secret: &str) -> AppResult<()> {
        self.entry(profile_id)?
            .set_password(secret)
            .map_err(|error| secret_error("无法保存模型密钥", error))
    }

    fn clear(&self, profile_id: &str) -> AppResult<()> {
        match self.entry(profile_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(secret_error("无法删除模型密钥", error)),
        }
    }

    fn has(&self, profile_id: &str) -> AppResult<bool> {
        self.get(profile_id).map(|value| value.is_some())
    }

    fn get(&self, profile_id: &str) -> AppResult<Option<String>> {
        match self.entry(profile_id)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(secret_error("无法读取模型密钥", error)),
        }
    }
}

fn secret_error(message: &str, error: keyring::Error) -> AppError {
    AppError::new("SETTINGS_FAILED", message).with_details(error.to_string())
}
