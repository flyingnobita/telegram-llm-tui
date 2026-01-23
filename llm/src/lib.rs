//! LLM providers and prompt templates.

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub system_prompt: String,
    pub user_instruction: String,
    pub transcript: String,
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

pub mod openai;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate_draft(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;
}

pub struct MockProvider;

#[async_trait]
impl LlmProvider for MockProvider {
    async fn generate_draft(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let transcript_len = request.transcript.len();
        Ok(LlmResponse {
            text: format!(
                "Draft generated based on transcript ({} chars). Instruction: {}",
                transcript_len, request.user_instruction
            ),
        })
    }
}
