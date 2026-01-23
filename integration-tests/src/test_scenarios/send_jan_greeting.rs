use std::error::Error;
use telegram_llm_core::telegram::{CacheManager, SendPipeline, SendRequest};
use tracing::{error, info};
use ui::bridge::UiCacheBridge;

pub async fn run(
    cache_manager: &CacheManager,
    ui_bridge: &mut UiCacheBridge,
    send_pipeline: &SendPipeline,
) -> Result<(), Box<dyn Error>> {
    // 1. Wait for sync
    info!("[AGENT] Waiting for background sync...");
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    ui_bridge.refresh(cache_manager);

    // 2. Find chat "Jan" and select it
    let target_title = "Jan";
    let chat_idx = ui_bridge
        .state
        .chats
        .iter()
        .position(|c| c.title == target_title);

    if let Some(idx) = chat_idx {
        // Deselect others
        for c in &mut ui_bridge.state.chats {
            c.is_selected = false;
        }
        ui_bridge.state.chats[idx].is_selected = true;

        // Sync selection to bridge
        ui_bridge.sync_selected_chat_from_state();
        let peer = ui_bridge
            .selected_peer()
            .ok_or("Failed to resolve peer from selection")?;

        info!(
            title = target_title,
            "[AGENT] Found and selected target chat"
        );

        // 3. Send "hi"
        let request = SendRequest::SendText {
            peer,
            text: "hi".to_string(),
            reply_to: None,
        };

        let ticket = send_pipeline.enqueue(request)?;
        info!(?ticket, "[AGENT] Enqueued message");
        println!("[AGENT-TEST] STATUS: OK");
        Ok(())
    } else {
        error!(target_title, "[AGENT] Chat not found");
        Err(format!("Chat '{}' not found", target_title).into())
    }
}
