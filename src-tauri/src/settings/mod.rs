mod secrets;
mod store;

pub use secrets::{SecretStore, SystemSecretStore};
pub use store::SettingsStore;

use serde::{Deserialize, Serialize};

/// 首次启动和规则留空时使用的提交规范。
pub const DEFAULT_COMMIT_RULE: &str = "使用 Conventional Commits，提交说明使用中文。\n\
格式：<type>(<scope>): <中文摘要>\n\
type 使用 feat、fix、refactor、perf、docs、test、build、ci、chore、style、revert。\n\
scope 可选，例如 ui、git、ai；无法明确判断时省略。\n\
摘要简洁说明实际改动，不加句号，尽量不超过 50 个汉字。\n\
正文仅在需要解释动机、影响或复杂行为时生成，使用中文。\n\
代码标识符、命令和专有名词保持原样。\n\
破坏性变更使用 type(scope)!: 并增加 BREAKING CHANGE: 脚注。\n\
不要编造 issue、提交哈希或 diff 中不存在的功能。\n\
只输出提交说明本身，不要输出解释、标题或 Markdown 代码块。";

/// 一个 OpenAI 兼容模型档案，不包含持久化密钥值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    /// 稳定档案标识，同时用于系统凭据账户名。
    pub id: String,
    /// 用户可读配置名称。
    pub name: String,
    /// OpenAI 兼容服务根地址。
    pub base_url: String,
    /// 请求中发送的模型标识。
    pub model: String,
    /// HTTP 请求超时秒数。
    pub timeout_seconds: u64,
    /// 是否为默认模型档案。
    pub is_default: bool,
    /// 是否已经配置密钥，仅作为返回视图字段。
    #[serde(skip)]
    pub has_api_key: bool,
}

/// 新增或编辑模型档案时从前端接收的输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfileInput {
    /// 编辑现有档案时传入 ID；新增时为空。
    pub id: Option<String>,
    /// 用户可读配置名称。
    pub name: String,
    /// OpenAI 兼容服务根地址。
    pub base_url: String,
    /// 请求中发送的模型标识。
    pub model: String,
    /// HTTP 请求超时秒数。
    pub timeout_seconds: u64,
    /// 是否设为默认档案。
    pub is_default: bool,
    /// 新密钥；为空时保留已有密钥。
    #[serde(default)]
    pub api_key: Option<String>,
    /// 是否显式清除已有密钥。
    #[serde(default)]
    pub clear_api_key: bool,
}

/// 持久化并返回给前端的全部非敏感应用设置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// 设置文件结构版本。
    pub version: u32,
    /// 最近打开的仓库路径，按最近使用排序。
    pub recent_repositories: Vec<String>,
    /// 当前选中的模型档案。
    pub active_model_id: Option<String>,
    /// 全部模型档案。
    pub models: Vec<ModelProfile>,
    /// 唯一一套全局提交规则。
    pub commit_rule: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: 1,
            recent_repositories: Vec::new(),
            active_model_id: None,
            models: Vec::new(),
            commit_rule: DEFAULT_COMMIT_RULE.to_string(),
        }
    }
}
