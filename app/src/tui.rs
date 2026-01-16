use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tracing::{info, warn};

use telegram_llm_core::telegram::{CacheManager, EventReceiver};
use ui::interaction::{handle_ui_key, KeymapStyle};
use ui::view::UiState;
use ui::view::{
    chat_list_max_scroll, chat_list_viewport_width, log_view_max_scroll, log_window_page_size,
    message_viewport_page_size,
};

use crate::ui_state::UiCacheBridge;
use crate::ConsoleLogGate;

const DRAW_INTERVAL_MS: u64 = 250;
const INPUT_POLL_MS: u64 = 100;
const LOG_REFRESH_INTERVAL_MS: u64 = 500;

pub async fn run_tui_loop(
    cache_manager: &CacheManager,
    ui_bridge: &mut UiCacheBridge,
    mut event_rx: EventReceiver,
    keymap: KeymapStyle,
    console_gate: ConsoleLogGate,
    log_path: PathBuf,
    log_window_max_lines: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("starting tui runtime");
    let _console_guard = ConsoleLogGuard::new(console_gate);
    let mut tui = Tui::new()?;
    let (input_tx, mut input_rx) = mpsc::unbounded_channel();
    let running = Arc::new(AtomicBool::new(true));
    let input_handle = spawn_input_thread(running.clone(), input_tx);

    let mut ticker = tokio::time::interval(Duration::from_millis(DRAW_INTERVAL_MS));
    let mut should_exit = false;
    let mut last_log_refresh =
        Instant::now().checked_sub(Duration::from_millis(LOG_REFRESH_INTERVAL_MS));
    apply_page_size(&mut ui_bridge.state, tui.terminal_area()?);
    tui.draw(&ui_bridge.state)?;

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            maybe_input = input_rx.recv() => {
                if let Some(event) = maybe_input {
                    if handle_input_event(event, ui_bridge, cache_manager, keymap) {
                        should_exit = true;
                    }
                } else {
                    should_exit = true;
                }
            }
            event = event_rx.recv() => {
                match event {
                    Ok(event) => {
                        cache_manager.apply_event(&event);
                        ui_bridge.refresh(cache_manager);
                        info!(?event, "received domain event");
                    }
                    Err(RecvError::Lagged(_)) => {
                        continue;
                    }
                    Err(RecvError::Closed) => {
                        should_exit = true;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                should_exit = true;
            }
        }

        apply_page_size(&mut ui_bridge.state, tui.terminal_area()?);
        maybe_refresh_logs(
            &mut ui_bridge.state,
            &log_path,
            log_window_max_lines,
            &mut last_log_refresh,
        );
        tui.draw(&ui_bridge.state)?;
        if should_exit {
            break;
        }
    }

    running.store(false, Ordering::Relaxed);
    let _ = input_handle.join();
    Ok(())
}

fn handle_input_event(
    event: InputEvent,
    ui_bridge: &mut UiCacheBridge,
    cache_manager: &CacheManager,
    keymap: KeymapStyle,
) -> bool {
    match event {
        InputEvent::Key(key) => handle_key_event(key, ui_bridge, cache_manager, keymap),
        InputEvent::Resize(width, height) => {
            apply_page_size(&mut ui_bridge.state, Rect::new(0, 0, width, height));
            false
        }
    }
}

fn handle_key_event(
    key: KeyEvent,
    ui_bridge: &mut UiCacheBridge,
    cache_manager: &CacheManager,
    keymap: KeymapStyle,
) -> bool {
    if matches!(key.kind, KeyEventKind::Release) {
        return false;
    }
    if is_exit_key(&key) {
        return true;
    }

    let handled = handle_ui_key(&mut ui_bridge.state, key, keymap);
    if handled && ui_bridge.sync_selected_chat_from_state() {
        ui_bridge.refresh(cache_manager);
    }
    false
}

fn is_exit_key(key: &KeyEvent) -> bool {
    matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('c'), modifiers) | (KeyCode::Char('q'), modifiers)
            if modifiers.contains(KeyModifiers::CONTROL)
    )
}

fn apply_page_size(state: &mut UiState, terminal_area: Rect) {
    let chat_viewport_width = chat_list_viewport_width(terminal_area, state.chat_list_width);
    if state.chat_list_viewport_width != chat_viewport_width {
        state.chat_list_viewport_width = chat_viewport_width;
    }
    let max_scroll = chat_list_max_scroll(state);
    if state.chat_list_scroll > max_scroll {
        state.chat_list_scroll = max_scroll;
    }

    let page_size = message_page_size(terminal_area, state.chat_list_width);
    if state.message_view.page_size != page_size {
        state.message_view.page_size = page_size;
    }

    let log_page_size = log_window_page_size(terminal_area);
    if state.log_view.page_size != log_page_size {
        state.log_view.page_size = log_page_size;
    }
    let max_log_scroll = log_view_max_scroll(state);
    if state.log_view.scroll_offset > max_log_scroll {
        state.log_view.scroll_offset = max_log_scroll;
    }
}

fn message_page_size(terminal_area: Rect, chat_list_width: u16) -> usize {
    message_viewport_page_size(terminal_area, chat_list_width)
}

fn maybe_refresh_logs(
    state: &mut UiState,
    log_path: &Path,
    log_window_max_lines: usize,
    last_refresh: &mut Option<Instant>,
) {
    if !state.log_view.is_open {
        return;
    }
    let now = Instant::now();
    let should_refresh = match last_refresh {
        Some(last) => now.duration_since(*last) >= Duration::from_millis(LOG_REFRESH_INTERVAL_MS),
        None => true,
    };
    if !should_refresh && !state.logs.is_empty() {
        return;
    }
    let was_at_bottom = is_log_view_at_bottom(state);
    match read_log_lines(log_path, log_window_max_lines) {
        Ok(lines) => {
            state.logs = lines;
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            state.logs.clear();
        }
        Err(err) => {
            state.logs = vec![format!("Log read error: {err}")];
        }
    }
    let max_scroll = log_view_max_scroll(state);
    if was_at_bottom || state.log_view.scroll_offset > max_scroll {
        state.log_view.scroll_offset = max_scroll;
    }
    *last_refresh = Some(now);
}

fn read_log_lines(path: &Path, max_lines: usize) -> io::Result<Vec<String>> {
    let contents = std::fs::read_to_string(path)?;
    let mut lines: Vec<String> = contents.lines().map(|line| line.to_string()).collect();
    if lines.len() > max_lines {
        let drain_count = lines.len().saturating_sub(max_lines);
        lines.drain(0..drain_count);
    }
    Ok(lines)
}

fn is_log_view_at_bottom(state: &UiState) -> bool {
    state.log_view.scroll_offset >= log_view_max_scroll(state)
}

fn spawn_input_thread(
    running: Arc<AtomicBool>,
    sender: mpsc::UnboundedSender<InputEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            match event::poll(Duration::from_millis(INPUT_POLL_MS)) {
                Ok(true) => match event::read() {
                    Ok(Event::Key(key)) => {
                        if sender.send(InputEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                    Ok(Event::Resize(width, height)) => {
                        if sender.send(InputEvent::Resize(width, height)).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(err) => {
                        warn!(error = %err, "tui input read failed");
                        break;
                    }
                },
                Ok(false) => {}
                Err(err) => {
                    warn!(error = %err, "tui input poll failed");
                    break;
                }
            }
        }
    })
}

enum InputEvent {
    Key(KeyEvent),
    Resize(u16, u16),
}

struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    _guard: TerminalGuard,
}

impl Tui {
    fn new() -> io::Result<Self> {
        let guard = TerminalGuard::enter()?;
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            _guard: guard,
        })
    }

    fn draw(&mut self, state: &UiState) -> io::Result<()> {
        self.terminal.draw(|frame| ui::view::draw(frame, state))?;
        Ok(())
    }

    fn terminal_area(&self) -> io::Result<Rect> {
        self.terminal.size()
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            Hide,
            crossterm::event::EnableMouseCapture
        )?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            Show,
            crossterm::event::DisableMouseCapture
        );
    }
}

struct ConsoleLogGuard {
    gate: ConsoleLogGate,
}

impl ConsoleLogGuard {
    fn new(gate: ConsoleLogGate) -> Self {
        gate.disable();
        Self { gate }
    }
}

impl Drop for ConsoleLogGuard {
    fn drop(&mut self) {
        self.gate.enable();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use ui::view::{LogViewState, UiState};

    #[test]
    fn message_page_size_clamps_to_minimum() {
        let area = Rect::new(0, 0, 10, 0);
        assert_eq!(message_page_size(area, 32), 1);
        let area = Rect::new(0, 0, 10, 3);
        assert_eq!(message_page_size(area, 32), 1);
    }

    #[test]
    fn message_page_size_reserves_composer_and_border() {
        let area = Rect::new(0, 0, 10, 10);
        assert_eq!(message_page_size(area, 32), 5);
    }

    #[test]
    fn read_log_lines_returns_last_lines() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("telegram-llm-log-{now}.log"));
        let contents = (1..=10)
            .map(|idx| format!("line-{idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, contents).unwrap();

        let lines = read_log_lines(&path, 4).unwrap();
        assert_eq!(lines, vec!["line-7", "line-8", "line-9", "line-10"]);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn maybe_refresh_logs_clears_missing_file() {
        let path = env::temp_dir().join("telegram-llm-log-missing.log");
        let _ = fs::remove_file(&path);
        let mut state = UiState {
            logs: vec!["old".to_string()],
            log_view: LogViewState {
                is_open: true,
                ..LogViewState::default()
            },
            ..UiState::default()
        };

        maybe_refresh_logs(&mut state, &path, 5, &mut None);

        assert!(state.logs.is_empty());
    }
}
