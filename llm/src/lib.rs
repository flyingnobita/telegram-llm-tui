//! LLM providers and prompt templates.

pub use openai::OpenAiProvider;

use async_trait::async_trait;
use regex::Regex;
use std::sync::OnceLock;
use thiserror::Error;

pub struct LlmRequest {
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub text: String,
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("configuration error: {0}")]
    Configuration(String),
}

pub mod kits;
pub mod openai;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate_draft(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;
}

pub struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    async fn generate_draft(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        Ok(LlmResponse {
            text: format!("Mock response for prompt: {}", request.user_prompt),
        })
    }
}

/// Simple tokenizer trait for estimation.
pub trait Tokenizer {
    fn count_tokens(&self, text: &str) -> usize;
    fn truncate(&self, text: &str, max_tokens: usize) -> String;
}

/// A simple character-based tokenizer (4 chars ~= 1 token).
pub struct CharTokenizer;

impl Tokenizer for CharTokenizer {
    fn count_tokens(&self, text: &str) -> usize {
        text.len() / 4
    }

    fn truncate(&self, text: &str, max_tokens: usize) -> String {
        let max_chars = max_tokens * 4;
        if text.len() <= max_chars {
            text.to_string()
        } else {
            text.chars().take(max_chars).collect()
        }
    }
}

/// Redacts potential secrets from the text.
/// Currently targets:
/// - Open AI keys (sk-...)
/// - Generic private keys (BEGIN PRIVATE KEY...)
pub fn redact_secrets(text: &str) -> String {
    static SECRET_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = SECRET_PATTERNS.get_or_init(|| {
        vec![
            // OpenAI / generic sk- keys: sk-[a-zA-Z0-9]{20,}
            Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap(),
            // Common private key headers
            Regex::new(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----").unwrap(),
        ]
    });

    let mut redacted = text.to_string();
    for pattern in patterns {
        redacted = pattern
            .replace_all(&redacted, "[REDACTED_SECRET]")
            .to_string();
    }
    redacted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redaction() {
        let input = "Here is my key: sk-123456789012345678901234 and some text.";
        let output = redact_secrets(input);
        assert_eq!(output, "Here is my key: [REDACTED_SECRET] and some text.");

        let input_safe = "Here is a safe string.";
        assert_eq!(redact_secrets(input_safe), input_safe);
    }

    #[test]
    fn test_char_tokenizer() {
        let t = CharTokenizer;
        assert_eq!(t.count_tokens("1234"), 1);
        assert_eq!(t.count_tokens("12345678"), 2);
        assert_eq!(t.truncate("12345678", 1), "1234");
    }
}
