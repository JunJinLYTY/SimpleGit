mod client;
mod prompt;

pub use client::generate_commit;
pub use prompt::{build_messages, extract_content, normalize_endpoint, ChatMessage, CommitContext};
