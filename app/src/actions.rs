use std::sync::Arc;

use llm::kits::PromptKit;
use llm::{CharTokenizer, LlmProvider, LlmRequest, Tokenizer};
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
    max_input_tokens: usize,
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
    let mut transcript = format_transcript(&messages);
    info!(kit_id = kit.id(), "exporting transcript to LLM");

    // Safety: Truncate transcript if too long
    let tokenizer = CharTokenizer;
    let token_count = tokenizer.count_tokens(&transcript);
    if token_count > max_input_tokens {
        warn!(
            token_count,
            max_input_tokens, "truncating transcript to fit limit"
        );
        transcript = tokenizer.truncate(&transcript, max_input_tokens);
        transcript.push_str("\n[TRUNCATED]");
    }

    // Safety: Redact secrets from transcript
    transcript = llm::redact_secrets(&transcript);

    let _ = ui_command_tx.send(UiCommand::ShowNotification(format!(
        "Processing export with '{}'...",
        kit.name()
    )));

    tokio::spawn(async move {
        // We do NOT redact user prompt here again if the kit adds static text,
        // but if the kit includes user instructions, those might contain secrets.
        // For now, we rely on transcript redaction.
        let user_prompt = kit.format_user_prompt(&transcript, "");

        // Final safety check?
        // Let's run redaction on the final prompt just in case the kit introduced something?
        // Or assume kit is safe? Kit is code, but it formats strings.
        // Let's wrap the final user_prompt in redaction too if we're paranoid,
        // but redaction is expensive if large context.
        // We already redacted transcript.

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

    // Note: We do NOT truncate/redact here yet because the user sees this in the UI.
    // We want them to see the full transcript. Truncation/Redaction happens on submit.

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
    max_input_tokens: usize,
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
        // Safety: Redact/Truncate
        let tokenizer = CharTokenizer;
        let mut unsafe_transcript = transcript;
        if tokenizer.count_tokens(&unsafe_transcript) > max_input_tokens {
            unsafe_transcript = tokenizer.truncate(&unsafe_transcript, max_input_tokens);
            unsafe_transcript.push_str("\n[TRUNCATED]");
        }
        let safe_transcript = llm::redact_secrets(&unsafe_transcript);
        let safe_input = llm::redact_secrets(&input_text);

        // Prepare prompt
        let user_prompt = kit.format_user_prompt(&safe_transcript, &safe_input);

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
