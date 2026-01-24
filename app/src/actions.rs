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

pub fn handle_open_llm_window(ui_bridge: &mut UiCacheBridge, cache_manager: &CacheManager) {
    let selected_ids: Vec<MessageId> = ui_bridge
        .state
        .message_view
        .selected_ids
        .iter()
        .map(|id| MessageId(*id))
        .collect();

    let messages = cache_manager.get_messages_by_ids(selected_ids);
    let transcript = format_transcript(&messages);

    ui_bridge.state.llm_window.transcript = transcript;
    ui_bridge.state.llm_window.is_open = true;
    ui_bridge.state.llm_window.focus_input = true;
    ui_bridge.state.llm_window.history.clear(); // Clear history on fresh open
    ui_bridge.state.llm_window.input = ui::input::InputState::default();
}

pub fn handle_llm_submit(
    ui_bridge: &mut UiCacheBridge,
    ui_command_tx: mpsc::UnboundedSender<UiCommand>,
    llm_provider: Arc<dyn LlmProvider>,
    kit: Box<dyn PromptKit>,
) {
    let input_text = ui_bridge.state.llm_window.input.text.clone();
    let transcript = ui_bridge.state.llm_window.transcript.clone();

    if input_text.trim().is_empty() {
        return;
    }

    ui_bridge
        .state
        .llm_window
        .history
        .push(ui::view::ChatMessage {
            author: "User".to_string(),
            text: input_text.clone(),
        });

    ui_bridge.state.llm_window.input = ui::input::InputState::default();

    // Create initial AI response placeholder?
    // No, UiCommand::LlmResponse will create it or append.

    tokio::spawn(async move {
        // Prepare prompt
        let user_prompt = kit.format_user_prompt(&transcript, &input_text);

        // For MVP, system prompt is from kit, user prompt is Context + Instruction.
        let request = LlmRequest {
            system_prompt: kit.system_prompt(),
            user_prompt,
        };

        match llm_provider.generate_draft(request).await {
            Ok(response) => {
                let _ = ui_command_tx.send(UiCommand::LlmResponse(response.text));
            }
            Err(err) => {
                warn!(error = %err, "llm window submit failed");
                let _ = ui_command_tx.send(UiCommand::LlmResponse(format!("Error: {}", err)));
            }
        }
    });
}
