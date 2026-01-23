use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{LlmError, LlmProvider, LlmRequest, LlmResponse};

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    client: Client,
    base_url: Url,
    model: String,
}

impl OpenAiProvider {
    pub fn new(base_url: String, model: String) -> Result<Self, LlmError> {
        let base_url = Url::parse(&base_url).map_err(|e| LlmError::Configuration(e.to_string()))?;
        Ok(Self {
            client: Client::new(),
            base_url,
            model,
        })
    }
}

#[derive(Serialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct CompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn generate_draft(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        // Prepare the endpoint URL.
        // We assume the user configures a base_url like "http://localhost:1234" or "http://localhost:1234/v1".
        // If it ends in /v1 (or /v1/), we append "chat/completions".
        // Otherwise we append "v1/chat/completions".
        // We must ensure the base URL has a trailing slash for Url::join to work as expected (appending instead of replacing).

        let path = self.base_url.path();
        let endpoint_suffix = if path.ends_with("/v1") || path.ends_with("/v1/") {
            "chat/completions"
        } else {
            "v1/chat/completions"
        };

        // Ensure base_url ends with slash for correct joining
        let mut base = self.base_url.clone();
        if !base.path().ends_with('/') {
            base.set_path(&format!("{}/", base.path()));
        }

        let url = base
            .join(endpoint_suffix)
            .map_err(|e| LlmError::Configuration(e.to_string()))?;

        // Construct messages
        let prompt = format!(
            "{}\n\nTranscript:\n{}",
            request.user_instruction, request.transcript
        );

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: request.system_prompt,
            },
            Message {
                role: "user".to_string(),
                content: prompt,
            },
        ];

        let req_body = CompletionRequest {
            model: self.model.clone(),
            messages,
        };

        let res = self
            .client
            .post(url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(LlmError::Provider(format!(
                "Status: {}, Body: {}",
                status, text
            )));
        }

        let response_body: CompletionResponse = res
            .json()
            .await
            .map_err(|e| LlmError::Network(e.to_string()))?;

        let text = response_body
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(LlmResponse { text })
    }
}
