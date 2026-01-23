use std::sync::Arc;

use llm::kits::PromptKit;
use llm::{LlmProvider, LlmRequest};
use telegram_llm_core::telegram::{CacheManager, MessageId};
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::command::UiCommand;
use crate::llm_workflow::format_transcript;
use ui::bridge::UiCacheBridge;

pub fn handle_export_selected(
    ui_bridge: &mut UiCacheBridge,
    cache_manager: &CacheManager,
    ui_command_tx: mpsc::UnboundedSender<UiCommand>,
    llm_provider: Arc<dyn LlmProvider>,
    kit: Box<dyn PromptKit>,
) {
    let selected_ids: Vec<MessageId> = ui_bridge
        .state
        .message_view
        .selected_ids
        .iter()
        .map(|id| MessageId(*id))
        .collect();

    if selected_ids.is_empty() {
        info!("no messages selected for export");
        return;
    }

    let messages = cache_manager.get_messages_by_ids(selected_ids);
    let transcript = format_transcript(&messages);
    info!(kit_id = kit.id(), "exporting transcript to LLM");

    let _ = ui_command_tx.send(UiCommand::ShowNotification(format!(
        "Processing export with '{}'...",
        kit.name()
    )));

    tokio::spawn(async move {
        let user_prompt = kit.format_user_prompt(&transcript, "");

        let request = LlmRequest {
            system_prompt: kit.system_prompt(),
            user_prompt,
        };

        match llm_provider.generate_draft(request).await {
            Ok(response) => {
                if let Err(err) = ui_command_tx.send(UiCommand::UpdateComposer(response.text)) {
                    warn!(error = %err, "failed to send update composer command");
                }
            }
            Err(err) => {
                let _ = ui_command_tx.send(UiCommand::ShowNotification(format!("Error: {}", err)));
                warn!(error = %err, "llm provider failed");
            }
        }
    });
}
