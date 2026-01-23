use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::command::UiCommand;
use crate::ui_state::UiCacheBridge;
use llm::{self, LlmProvider};
use telegram_llm_core::telegram::{CacheManager, SendPipeline};

pub async fn run_agent_loop(
    cache_manager: &CacheManager,
    ui_bridge: &mut UiCacheBridge,
    _send_pipeline: &SendPipeline,
    llm_provider: Arc<dyn LlmProvider>,
    scenario: String,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(scenario, "starting agent test mode");

    match scenario.as_str() {
        "draft_reply" => run_draft_reply_scenario(cache_manager, ui_bridge, llm_provider).await?,
        _ => {
            error!(scenario, "unknown scenario");
            return Err("Unknown scenario".into());
        }
    }

    Ok(())
}

async fn run_draft_reply_scenario(
    cache_manager: &CacheManager,
    ui_bridge: &mut UiCacheBridge,
    llm_provider: Arc<dyn LlmProvider>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Wait for dialogs and history background sync
    // In a real environment, we might want to poll for state changes, but for now a fixed sleep is "good enough" for MVP
    info!("[AGENT] Waiting for background sync...");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // 2. Refresh bridge to ensure we have the latest data from cache
    ui_bridge.refresh(cache_manager);

    // 3. Select first chat
    if ui_bridge.state.chats.is_empty() {
        error!("[AGENT] No chats found. Cannot test.");
        return Err("No chats found".into());
    }

    // Select the first chat
    ui_bridge.state.chats[0].is_selected = true;
    let chat_name = ui_bridge.state.chats[0].title.clone();
    info!(chat_name, "[AGENT] Selected chat");

    // Refresh to load messages for this chat
    if ui_bridge.sync_selected_chat_from_state() {
        ui_bridge.refresh(cache_manager);
    }

    if ui_bridge.state.messages.is_empty() {
        error!(chat_name, "[AGENT] No messages in chat. Cannot test draft.");
        return Err("No messages in chat".into());
    }

    // 4. Select last message
    let last_idx = ui_bridge.state.messages.len() - 1;
    let msg_id = ui_bridge.state.messages[last_idx].id;
    ui_bridge.state.message_view.selected_ids.insert(msg_id);
    info!(msg_id, "[AGENT] Selected message");

    // 5. Trigger action
    let (tx, mut rx) = mpsc::unbounded_channel();

    // We use the REPLY kit (default)
    crate::actions::handle_export_selected(
        ui_bridge,
        cache_manager,
        tx,
        llm_provider,
        llm::kits::get_default_kit(),
    );

    // 6. Wait for response
    info!("[AGENT] Waiting for LLM response...");
    loop {
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await {
            Ok(Some(cmd)) => match cmd {
                UiCommand::ShowNotification(msg) => info!("[AGENT] Notification: {}", msg),
                UiCommand::UpdateComposer(draft) => {
                    info!("[AGENT] Received Draft: {}", draft);
                    if !draft.is_empty() {
                        println!("[AGENT-TEST] STATUS: OK");
                        return Ok(());
                    } else {
                        error!("[AGENT] Draft was empty");
                        return Err("Empty draft".into());
                    }
                }
            },
            Ok(None) => {
                error!("[AGENT] Channel closed without result");
                return Err("Channel closed".into());
            }
            Err(_) => {
                error!("[AGENT] Timed out waiting for LLM");
                return Err("Timeout".into());
            }
        }
    }
}
