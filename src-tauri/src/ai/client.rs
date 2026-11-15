use std::time::Duration;

use reqwest::header::{HeaderValue, AUTHORIZATION};
use serde::Serialize;

use crate::ai::prompt::{build_messages, extract_content, normalize_endpoint, CommitContext};
use crate::error::{AppError, AppResult};
use crate::settings::ModelProfile;

/// 提交说明生成使用的低随机性。
const TEMPERATURE: f32 = 0.2;
/// 默认模型输出 token 上限。
const MAX_TOKENS: u32 = 2048;
/// 防止异常服务返回过大响应体的上限。
const MAX_RESPONSE_BYTES: usize = 2_000_000;

/// OpenAI Chat Completions 请求体。
#[derive(Serialize)]
struct ChatRequest<'a> {
    /// 模型标识。
    model: &'a str,
    /// 生成随机性。
    temperature: f32,
    /// 最大输出 token 数。
    max_tokens: u32,
    /// 系统与用户消息。
    messages: Vec<crate::ai::ChatMessage>,
}

/// 调用当前模型档案，只返回清理后的提交说明。
pub async fn generate_commit(
    client: &reqwest::Client,
    profile: &ModelProfile,
    api_key: Option<&str>,
    context: &CommitContext,
) -> AppResult<String> {
    let messages = build_messages(context)?;
    let endpoint = normalize_endpoint(&profile.base_url)?;
    let timeout = Duration::from_secs(profile.timeout_seconds.clamp(1, 600));
    let request_body = ChatRequest {
        model: &profile.model,
        temperature: TEMPERATURE,
        max_tokens: MAX_TOKENS,
        messages,
    };

    let mut request = client.post(endpoint).timeout(timeout).json(&request_body);
    if let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) {
        let header = HeaderValue::from_str(&format!("Bearer {}", api_key.trim()))
            .map_err(|error| model_error("API Key 包含无效字符", error))?;
        request = request.header(AUTHORIZATION, header);
    }

    let response = request.send().await.map_err(|error| {
        let message = if error.is_timeout() {
            "模型请求超时"
        } else {
            "无法连接模型服务"
        };
        model_error(message, error)
    })?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| model_error("读取模型响应失败", error))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(AppError::new("MODEL_REQUEST_FAILED", "模型响应体过大"));
    }
    let body = String::from_utf8_lossy(&bytes);
    if !status.is_success() {
        return Err(model_error(
            &format!("模型服务返回 HTTP {}", status.as_u16()),
            truncate(&body, 2000),
        ));
    }

    extract_content(&body)
}

fn truncate(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn model_error(message: &str, details: impl std::fmt::Display) -> AppError {
    AppError::new("MODEL_REQUEST_FAILED", message).with_details(details.to_string())
}
