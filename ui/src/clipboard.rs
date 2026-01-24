use base64::{engine::general_purpose::STANDARD, Engine as _};
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

            // 3. Try OSC 52 (Terminal fallback)
            if !success {
                match spawn_osc52_copy(&text) {
                    Ok(_) => {
                        info!("osc52 copy emitted");
                        success = true;
                    }
                    Err(e) => {
                        warn!("osc52 copy failed: {}", e);
                    }
                }
            }

            // Report status
            if let Some(ref s) = status_tx {
                if success {
                    let _ = s.send(ClipboardResult::Success);
                } else {
                    let _ = s.send(ClipboardResult::Error(
                        "Clipboard failed (tried xclip/wl-copy, arboard, osc52)".to_string(),
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

fn spawn_osc52_copy(text: &str) -> Result<(), String> {
    // OSC 52 format: \x1b]52;c;<base64_encoded_content>\x07
    // c indicates clipboard (p would be primary selection)
    let encoded = STANDARD.encode(text);
    let osc = format!("\x1b]52;c;{}\x07", encoded);

    // We print directly to stdout/stderr as this is a TUI app where stdout is likely raw mode.
    // However, we are in a background thread.
    // Ideally we should print this to the TUI's output buffer, but `println!` might disrupt layout if not handled.
    // In many TUI apps, writing OSC sequences to stderr or stdout works if the backend passes it through.
    // Since we are in a separate thread, we can't easily write to the TUI buffer.
    // But! TUI libraries usually put terminal in raw mode. Writing bytes might just work.
    // Let's try writing to /dev/tty directly if possible, or just stdout.

    // Using /dev/tty is safer for escape sequences to reach the terminal emulator directly
    // regardless of stdout redirection (though usually not redirected in TUI).
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/tty")
        .map_err(|e| format!("Failed to open /dev/tty: {}", e))?;

    output
        .write_all(osc.as_bytes())
        .map_err(|e| format!("Failed to write to /dev/tty: {}", e))?;

    Ok(())
}

fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
