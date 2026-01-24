use std::time::Duration;
use tokio::sync::mpsc;
use ui::clipboard::{spawn_worker, ClipboardResult};
use ui::view::{get_selected_log_text, UiState};

#[tokio::test]
async fn verify_clipboard_persistence_and_file_paste() {
    // 1. Setup realistic log data matching user request
    let log_line = "2026-01-24T00:53:52.671+08:00 INFO telegram_llm_core::telegram::cache: cache flushed chats=523 messages=19077".to_string();

    // Skip in CI environments where system clipboard is unavailable
    if std::env::var("CI").is_ok() {
        println!("Skipping clipboard verification in CI environment");
        return;
    }

    let mut state = UiState {
        logs: vec![
            "Previous line...".to_string(),
            log_line.clone(),
            "Next line...".to_string(),
        ],
        ..Default::default()
    };
    state.log_view.is_open = true;
    state.log_view.selection = Some((1, 1)); // Select the target line

    // Simulate viewport width large enough to avoid wrapping
    state.log_view.pane.viewport.width = 200;

    // 2. Extract text using actual UI logic
    let text_to_copy = get_selected_log_text(&state).expect("Failed to extract selected log text");
    assert_eq!(text_to_copy, log_line, "Extraction logic mismatch");

    // 3. Setup status channel & Worker
    let (status_tx, mut status_rx) = mpsc::unbounded_channel();
    let clipboard_tx = spawn_worker(Some(status_tx));

    // 4. Send text to clipboard
    clipboard_tx
        .send(text_to_copy.clone())
        .expect("Failed to send to clipboard worker");

    // 5. Wait for confirmation
    let result = status_rx
        .recv()
        .await
        .expect("Worker closed status channel");
    match result {
        ClipboardResult::Success => println!("Worker reported success"),
        ClipboardResult::Error(e) => panic!("Worker reported error: {}", e),
    }

    // 6. Simulate "User Delay" - user switches apps
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // 7. Verify by reading back from System Clipboard
    // We create a FRESH clipboard instance here to simulate an external app (like a text editor)
    // trying to read what we just wrote.
    // 7. Verify by reading back from System Clipboard
    // We create a FRESH clipboard instance here to simulate an external app.
    // We prioritize arboard, but if it fails (common on some Linux setups where we used CLI to write),
    // we fallback to CLI tools to verify the data is actually there.
    let content = match arboard::Clipboard::new() {
        Ok(mut reader) => match reader.get_text() {
            Ok(c) => c,
            Err(e) => {
                println!("arboard read failed: {}, trying CLI fallback...", e);
                read_clipboard_linux_cli().expect("Both arboard and CLI read failed")
            }
        },
        Err(e) => {
            println!("arboard init failed: {}, trying CLI fallback...", e);
            read_clipboard_linux_cli().expect("Both arboard and CLI init failed")
        }
    };

    // Note: On some systems/clipboards, trimming might be needed, but exact match is ideal
    assert_eq!(
        content.trim(),
        log_line.trim(),
        "Clipboard content mismatch! persistence failed?"
    );

    // 8. Write to file as requested by user to prove it "pastes"
    let mut file_path = std::env::current_dir().expect("Failed to get current dir");
    // We write to the current directory directly to avoid path issues
    file_path.push("clipboard_verification_output.txt");
    std::fs::write(&file_path, content).expect("Failed to write to file");

    // Print the path so we can tell the user and walkthrough
    println!("Pasted content saved to: {:?}", file_path);
}

fn read_clipboard_linux_cli() -> Result<String, String> {
    use std::process::Command;

    // Try wl-paste (Wayland)
    if let Ok(output) = Command::new("wl-paste").output() {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    // Try xclip (X11)
    if let Ok(output) = Command::new("xclip")
        .arg("-o")
        .arg("-selection")
        .arg("clipboard")
        .output()
    {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    // Try xsel (X11)
    if let Ok(output) = Command::new("xsel").arg("-o").arg("-b").output() {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }
    }

    Err("No clipboard CLI tools found or they failed".to_string())
}
