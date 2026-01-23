use crate::{LlmError, LlmProvider, LlmRequest, LlmResponse};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use async_trait::async_trait;

pub struct OpenAiProvider {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiProvider {
    pub fn new(base_url: Option<String>, api_key: Option<String>, model: String) -> Self {
        let mut config = OpenAIConfig::new();

        if let Some(url) = base_url {
            config = config.with_api_base(url);
        }

        if let Some(key) = api_key {
            config = config.with_api_key(key);
        } else {
            // Provide dummy key if none exists (e.g. for local LM Studio)
            config = config.with_api_key("dummy");
        }

        let client = Client::with_config(config);
        Self { client, model }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn generate_draft(&self, request: LlmRequest) -> Result<LlmResponse, LlmError> {
        let system_msg = ChatCompletionRequestSystemMessageArgs::default()
            .content(request.system_prompt.as_str())
            .build()
            .map_err(|e| LlmError::Provider(format!("failed to build system prompt: {}", e)))?;

        let user_content = format!(
            "{}\n\nTranscript:\n{}",
            request.user_instruction, request.transcript
        );
        let user_msg = ChatCompletionRequestUserMessageArgs::default()
            .content(user_content)
            .build()
            .map_err(|e| LlmError::Provider(format!("failed to build user prompt: {}", e)))?;

        let messages: Vec<ChatCompletionRequestMessage> = vec![system_msg.into(), user_msg.into()];

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .build()
            .map_err(|e| LlmError::Provider(format!("failed to build request: {}", e)))?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .map_err(|e| LlmError::Network(format!("openai api error: {}", e)))?;

        let text = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(LlmResponse { text })
    }
}
