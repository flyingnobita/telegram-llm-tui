use crate::{LlmError, LlmProvider, LlmRequest, LlmResponse};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::info;

pub struct OpenAiProvider {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiProvider {
    pub fn new(base_url: Option<String>, api_key: Option<String>, model: String) -> Self {
        let client = Client::new();
        let base_url = base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        // Ensure no trailing slash for cleaner component appending, though strict usage usually handles it.
        // But here we might just append /chat/completions
        let base_url = base_url.trim_end_matches('/').to_string();

        let api_key = api_key.unwrap_or_else(|| "dummy".to_string());

        Self {
            client,
            base_url,
            api_key,
            model,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn generate_draft(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        info!(target: "llm_transcript", "User: {}", request.user_prompt);

        let url = format!("{}/chat/completions", self.base_url);

        let body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": request.system_prompt
                },
                {
                    "role": "user",
                    "content": request.user_prompt
                }
            ]
        });

        if let Ok(body_str) = serde_json::to_string_pretty(&body) {
            info!(target: "llm_transcript_full", "User Instruction:\n{}\n\nFull JSON Request:\n{}", request.user_prompt, body_str);
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network(format!("request failed for url {}: {}", url, e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!(
                "api error ({} at {}): {}",
                status, url, text
            )));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| LlmError::Network(format!("failed to parse json: {}", e)))?;

        // Lenient parsing: we don't care about 'id' or other meta fields, just want the content.
        let text = response_json["choices"][0]["message"]["content"]
            .as_str()
            .or_else(|| response_json["choices"][0]["text"].as_str()) // Fallback for some legacy completions endpoints if misused
            .ok_or_else(|| {
                LlmError::Provider(format!(
                    "malformed response: missing content. Full JSON: {}",
                    response_json
                ))
            })?
            .to_string();

        info!(target: "llm_transcript", "Provider: {}", text);
        info!(target: "llm_transcript_full", "Provider: {}", text);

        Ok(LlmResponse { text })
    }
}
