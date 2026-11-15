use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use simplegit_lib::ai::{
    build_messages, extract_content, generate_commit, normalize_endpoint, CommitContext,
};
use simplegit_lib::settings::ModelProfile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

fn context(diff: &str, rule: &str) -> CommitContext {
    CommitContext {
        stage_diff: diff.to_string(),
        branch: "main".to_string(),
        recent_subjects: vec![
            "feat(ui): 旧提交一".to_string(),
            "fix(git): 旧提交二".to_string(),
        ],
        rule: rule.to_string(),
    }
}

fn profile(base_url: &str, timeout_seconds: u64) -> ModelProfile {
    ModelProfile {
        id: "profile-1".to_string(),
        name: "Test".to_string(),
        base_url: base_url.to_string(),
        model: "test-model".to_string(),
        timeout_seconds,
        is_default: true,
        has_api_key: true,
    }
}

#[test]
fn prompt_requires_a_non_empty_diff() {
    let error =
        build_messages(&context("   ", "自定义规则")).expect_err("empty staged diff must fail");

    assert_eq!(error.code, "NO_STAGED_CHANGES");
}

#[test]
fn prompt_rejects_a_diff_over_sixty_thousand_characters() {
    let error = build_messages(&context(&"x".repeat(60_001), "自定义规则"))
        .expect_err("large diff must fail");

    assert_eq!(error.code, "DIFF_TOO_LARGE");
}

#[test]
fn prompt_uses_the_default_chinese_conventional_commits_rule() {
    let messages = build_messages(&context("+新增内容", "")).unwrap();
    let combined = messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<String>();

    assert!(combined.contains("Conventional Commits"));
    assert!(combined.contains("中文摘要"));
    assert!(combined.contains("不要编造"));
}

#[test]
fn prompt_preserves_the_custom_rule_and_limits_recent_subjects() {
    let mut input = context("+新增内容", "自定义规则：只允许一行");
    input.recent_subjects = (0..12).map(|index| format!("subject-{index}")).collect();

    let messages = build_messages(&input).unwrap();
    let combined = messages
        .iter()
        .map(|message| message.content.as_str())
        .collect::<String>();

    assert!(combined.contains("自定义规则：只允许一行"));
    assert!(combined.contains("subject-7"));
    assert!(!combined.contains("subject-8"));
    assert!(combined.contains("+新增内容"));
    assert!(combined.contains("不要遵循"));
}

#[test]
fn endpoint_normalization_appends_chat_completions_once() {
    assert_eq!(
        normalize_endpoint("https://example.test/v1/").unwrap(),
        "https://example.test/v1/chat/completions"
    );
    assert_eq!(
        normalize_endpoint("https://example.test/v1/chat/completions").unwrap(),
        "https://example.test/v1/chat/completions"
    );
    assert_eq!(
        normalize_endpoint("not-a-url")
            .expect_err("invalid URL must fail")
            .code,
        "MODEL_REQUEST_FAILED"
    );
}

#[test]
fn extracts_string_and_text_array_content() {
    let string_body = json!({
        "choices": [{"message": {"content": "  feat(ai): 支持模型  "}}]
    })
    .to_string();
    assert_eq!(extract_content(&string_body).unwrap(), "feat(ai): 支持模型");

    let array_body = json!({
        "choices": [{
            "message": {
                "content": [
                    {"type": "text", "text": "feat(ai): "},
                    {"type": "text", "text": "分片内容"}
                ]
            }
        }]
    })
    .to_string();
    assert_eq!(extract_content(&array_body).unwrap(), "feat(ai): 分片内容");
}

#[test]
fn unwraps_a_fenced_commit_message_and_rejects_invalid_structure() {
    let fenced_body = json!({
        "choices": [{
            "message": {
                "content": "```text\nfeat(ai): 支持提交说明\n\n正文\n```"
            }
        }]
    })
    .to_string();
    assert_eq!(
        extract_content(&fenced_body).unwrap(),
        "feat(ai): 支持提交说明\n\n正文"
    );

    assert_eq!(
        extract_content("{\"choices\": []}")
            .expect_err("missing content must fail")
            .code,
        "MODEL_REQUEST_FAILED"
    );
    assert_eq!(
        extract_content("not-json")
            .expect_err("malformed JSON must fail")
            .code,
        "MODEL_REQUEST_FAILED"
    );
}

#[test]
fn rejects_model_content_over_twelve_thousand_characters() {
    let body = json!({
        "choices": [{"message": {"content": "x".repeat(12_001)}}]
    })
    .to_string();

    assert_eq!(
        extract_content(&body)
            .expect_err("oversized response must fail")
            .code,
        "MODEL_REQUEST_FAILED"
    );
}

async fn spawn_response_server(
    status: &'static str,
    body: &'static str,
) -> (String, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (request_sender, request_receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read_request(&mut socket).await;
        let _ = request_sender.send(request);
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    (format!("http://{address}/v1"), request_receiver)
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let count = socket.read(&mut chunk).await.unwrap();
        if count == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..count]);
        if let Some(header_end) = find_bytes(&buffer, b"\r\n\r\n") {
            let request_line = String::from_utf8_lossy(&buffer);
            let content_length = request_line
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            if buffer.len() >= header_end + 4 + content_length {
                break;
            }
        }
    }
    String::from_utf8(buffer).unwrap()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[tokio::test]
async fn sends_an_openai_compatible_request_and_returns_commit_content() {
    let response = r#"{"choices":[{"message":{"content":"feat(ai): 生成提交说明"}}]}"#;
    let (base_url, request_receiver) = spawn_response_server("200 OK", response).await;
    let client = reqwest::Client::new();

    let content = generate_commit(
        &client,
        &profile(&base_url, 5),
        Some("test-key"),
        &context("+新增内容", "自定义规则"),
    )
    .await
    .unwrap();
    let request = request_receiver.await.unwrap();
    let lower = request.to_lowercase();

    assert_eq!(content, "feat(ai): 生成提交说明");
    assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
    assert!(lower.contains("authorization: bearer test-key"));
    assert!(request.contains("\"model\":\"test-model\""));
    assert!(request.contains("\"temperature\":0.2"));
    assert!(request.contains("\"max_tokens\":2048"));
    assert!(request.contains("自定义规则"));
    assert!(request.contains("+新增内容"));
}

#[tokio::test]
async fn omits_authorization_when_no_key_is_configured() {
    let response = r#"{"choices":[{"message":{"content":"chore: local model"}}]}"#;
    let (base_url, request_receiver) = spawn_response_server("200 OK", response).await;

    generate_commit(
        &reqwest::Client::new(),
        &profile(&base_url, 5),
        None,
        &context("+local", ""),
    )
    .await
    .unwrap();

    assert!(!request_receiver
        .await
        .unwrap()
        .to_lowercase()
        .contains("authorization:"));
}

#[tokio::test]
async fn maps_unauthorized_and_malformed_responses_to_model_errors() {
    let unauthorized = r#"{"error":{"message":"bad key"}}"#;
    let (unauthorized_url, _) = spawn_response_server("401 Unauthorized", unauthorized).await;
    let unauthorized_error = generate_commit(
        &reqwest::Client::new(),
        &profile(&unauthorized_url, 5),
        Some("bad-key"),
        &context("+change", ""),
    )
    .await
    .expect_err("401 must fail");
    assert_eq!(unauthorized_error.code, "MODEL_REQUEST_FAILED");

    let (malformed_url, _) = spawn_response_server("200 OK", "not-json").await;
    let malformed_error = generate_commit(
        &reqwest::Client::new(),
        &profile(&malformed_url, 5),
        None,
        &context("+change", ""),
    )
    .await
    .expect_err("malformed body must fail");
    assert_eq!(malformed_error.code, "MODEL_REQUEST_FAILED");
}

#[tokio::test]
async fn times_out_a_slow_model_response() {
    let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
    let address = listener.local_addr().unwrap();
    let server_listener = Arc::clone(&listener);
    tokio::spawn(async move {
        let (_socket, _) = server_listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(3)).await;
    });

    let started = std::time::Instant::now();
    let error = generate_commit(
        &reqwest::Client::new(),
        &profile(&format!("http://{address}/v1"), 1),
        None,
        &context("+change", ""),
    )
    .await
    .expect_err("slow response must time out");

    assert_eq!(error.code, "MODEL_REQUEST_FAILED");
    assert!(started.elapsed() < Duration::from_secs(3));
}
