use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub enum ClipboardResult {
    Success,
    Error(String),
}

pub fn spawn_worker(status_tx: Option<UnboundedSender<ClipboardResult>>) -> Sender<String> {
    let (tx, rx) = channel::<String>();
    thread::spawn(move || {
        // Lazy init arboard to prevent blocking thread startup
        let mut clipboard: Option<arboard::Clipboard> = None;
        // Keep track of spawned CLI processes to reap zombies
        let mut children: Vec<Child> = Vec::new();

        while let Ok(text) = rx.recv() {
            info!(length = text.len(), "clipboard worker received text");

            // Reap dead children
            children.retain_mut(|child| match child.try_wait() {
                Ok(Some(_)) => false, // Finished
                Ok(None) => true,     // Still running
                Err(_) => false,      // Error (assume gone)
            });

            let mut success = false;

            // 1. Try Linux CLI fallbacks FIRST (priority)
            if cfg!(target_os = "linux") {
                match spawn_linux_cli_copy(&text) {
                    Ok(child) => {
                        info!("linux cli copy spawned successfully");
                        children.push(child);
                        success = true;
                    }
                    Err(e) => {
                        warn!("linux cli copy failed: {}", e);
                    }
                }
            }

            // 2. Try arboard second (fallback for Linux, primary for others)
            if !success {
                if clipboard.is_none() {
                    match arboard::Clipboard::new() {
                        Ok(cb) => clipboard = Some(cb),
                        Err(e) => {
                            warn!("arboard init failed: {}", e);
                            if !cfg!(target_os = "linux") {
                                if let Some(ref s) = status_tx {
                                    let _ = s.send(ClipboardResult::Error(format!(
                                        "Init failed: {}",
                                        e
                                    )));
                                }
                                continue;
                            }
                        }
                    }
                }

                if let Some(cb) = &mut clipboard {
                    if let Err(e) = cb.set_text(&text) {
                        warn!("arboard set_text failed: {}", e);
                    } else {
                        info!("arboard set_text success");
                        success = true;
                    }
                }
            }

            // Report status
            if let Some(ref s) = status_tx {
                if success {
                    let _ = s.send(ClipboardResult::Success);
                } else {
                    let _ = s.send(ClipboardResult::Error(
                        "All clipboard methods failed".to_string(),
                    ));
                }
            }
        }
    });
    tx
}

fn spawn_linux_cli_copy(text: &str) -> Result<Child, String> {
    // Try wl-copy (Wayland)
    if is_command_available("wl-copy") {
        info!("Attempting wl-copy");
        let mut command = Command::new("wl-copy");
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        drop(stdin); // Signal EOF
                        return Ok(child);
                    }
                }
                return Err("Failed to write to wl-copy stdin".to_string());
            }
            Err(e) => return Err(format!("Failed to spawn wl-copy: {}", e)),
        }
    }

    // Try xclip (X11)
    if is_command_available("xclip") {
        info!("Attempting xclip");
        let mut command = Command::new("xclip");
        command
            .arg("-selection")
            .arg("clipboard")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        drop(stdin);
                        return Ok(child);
                    }
                }
                return Err("Failed to write to xclip stdin".to_string());
            }
            Err(e) => return Err(format!("Failed to spawn xclip: {}", e)),
        }
    }

    // Try xsel (X11)
    if is_command_available("xsel") {
        info!("Attempting xsel");
        let mut command = Command::new("xsel");
        command
            .arg("--clipboard")
            .arg("--input")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        drop(stdin);
                        return Ok(child);
                    }
                }
                return Err("Failed to write to xsel stdin".to_string());
            }
            Err(e) => return Err(format!("Failed to spawn xsel: {}", e)),
        }
    }

    Err("No clipboard CLI tools found".to_string())
}

fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
