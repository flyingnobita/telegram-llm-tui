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

use telegram_llm_core::telegram::{
    CacheManager, EventReceiver, MessageId, SendPipeline, SendRequest,
};
use ui::input::InputState;
use ui::interaction::{handle_ui_key, KeymapStyle, UiAction};
use ui::view::{
    chat_list_text_area, clamp_chat_list_scroll, composer_text_area,
    ensure_chat_list_selection_visible, log_pane_metrics, log_view_max_horizontal_scroll,
    log_view_max_scroll, log_window_text_area, message_max_horizontal_scroll,
    message_viewport_page_size, message_viewport_width, UiFocus, UiState,
};

use crate::command::UiCommand;
use crate::llm_workflow::format_transcript;
use crate::ui_state::UiCacheBridge;
use crate::ConsoleLogGate;
use llm::{LlmProvider, LlmRequest};

const DRAW_INTERVAL_MS: u64 = 250;
const INPUT_POLL_MS: u64 = 100;
const LOG_REFRESH_INTERVAL_MS: u64 = 500;
const CURSOR_BLINK_INTERVAL_MS: u64 = 500;

#[allow(clippy::too_many_arguments)]
pub async fn run_tui_loop(
    cache_manager: &CacheManager,
    ui_bridge: &mut UiCacheBridge,
    mut event_rx: EventReceiver,
    mut cache_refresh_rx: mpsc::UnboundedReceiver<()>,
    keymap: KeymapStyle,
    send_pipeline: &SendPipeline,
    console_gate: ConsoleLogGate,
    log_path: PathBuf,
    log_window_max_lines: usize,
    llm_provider: Arc<dyn LlmProvider>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("starting tui runtime");
    let _console_guard = ConsoleLogGuard::new(console_gate);
    let mut tui = Tui::new()?;
    let (input_tx, mut input_rx) = mpsc::unbounded_channel();
    let (ui_command_tx, mut ui_command_rx) = mpsc::unbounded_channel();
    let running = Arc::new(AtomicBool::new(true));
    let input_handle = spawn_input_thread(running.clone(), input_tx);

    let mut ticker = tokio::time::interval(Duration::from_millis(DRAW_INTERVAL_MS));
    let mut should_exit = false;
    let mut last_log_refresh =
        Instant::now().checked_sub(Duration::from_millis(LOG_REFRESH_INTERVAL_MS));
    let mut cursor_blink_on = true;
    let mut last_cursor_blink = Instant::now();
    apply_page_size(&mut ui_bridge.state, tui.terminal_area()?);
    tui.draw(&ui_bridge.state)?;

    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            maybe_input = input_rx.recv() => {
                if let Some(event) = maybe_input {
                    if handle_input_event(
                        event,
                        ui_bridge,
                        cache_manager,
                        keymap,
                        send_pipeline,
                        ui_command_tx.clone(),
                        llm_provider.clone(),
                    ) {
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
            refresh = cache_refresh_rx.recv() => {
                if refresh.is_none() {
                    should_exit = true;
                } else {
                    ui_bridge.refresh(cache_manager);
                }
            }
            command = ui_command_rx.recv() => {
                if let Some(command) = command {
                    handle_ui_command(command, ui_bridge);
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
        if last_cursor_blink.elapsed() >= Duration::from_millis(CURSOR_BLINK_INTERVAL_MS) {
            cursor_blink_on = !cursor_blink_on;
            last_cursor_blink = Instant::now();
        }
        ui_bridge.state.composer_cursor_visible = cursor_blink_on;
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
    send_pipeline: &SendPipeline,
    ui_command_tx: mpsc::UnboundedSender<UiCommand>,
    llm_provider: Arc<dyn LlmProvider>,
) -> bool {
    match event {
        InputEvent::Key(key) => handle_key_event(
            key,
            ui_bridge,
            cache_manager,
            keymap,
            send_pipeline,
            ui_command_tx,
            llm_provider,
        ),
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
    send_pipeline: &SendPipeline,
    ui_command_tx: mpsc::UnboundedSender<UiCommand>,
    llm_provider: Arc<dyn LlmProvider>,
) -> bool {
    if matches!(key.kind, KeyEventKind::Release) {
        return false;
    }
    if is_exit_key(&key) {
        return true;
    }

    let result = handle_ui_key(&mut ui_bridge.state, key, keymap);
    if result.handled && ui_bridge.sync_selected_chat_from_state() {
        ui_bridge.refresh(cache_manager);
    }
    if let Some(action) = result.action {
        handle_ui_action(
            action,
            ui_bridge,
            send_pipeline,
            cache_manager,
            ui_command_tx,
            llm_provider,
        );
    }
    false
}

fn handle_ui_action(
    action: UiAction,
    ui_bridge: &mut UiCacheBridge,
    send_pipeline: &SendPipeline,
    cache_manager: &CacheManager,
    ui_command_tx: mpsc::UnboundedSender<UiCommand>,
    llm_provider: Arc<dyn LlmProvider>,
) {
    match action {
        UiAction::ComposerSubmit => handle_composer_submit(ui_bridge, send_pipeline),
        UiAction::TriggerRefresh => {
            ui_bridge.refresh(cache_manager);
        }
        UiAction::ExportSelected => {
            handle_export_selected(ui_bridge, cache_manager, ui_command_tx, llm_provider);
        }
        UiAction::OpenCommandPalette => {
            handle_open_command_palette(ui_bridge);
        }
        UiAction::CommandPaletteSubmit => {
            handle_command_palette_submit(ui_bridge, cache_manager, ui_command_tx, llm_provider);
        }
        UiAction::SelectAllInView => {
            ui::interaction::select_all_in_view(&mut ui_bridge.state);
        }
        UiAction::CopyLogSelection => {
            if let Some(text) = ui::view::get_selected_log_text(&ui_bridge.state) {
                match arboard::Clipboard::new() {
                    Ok(mut clipboard) => {
                        if let Err(err) = clipboard.set_text(text) {
                            let _ = ui_command_tx.send(UiCommand::ShowNotification(format!(
                                "Clipboard error: {}",
                                err
                            )));
                        } else {
                            let _ = ui_command_tx.send(UiCommand::ShowNotification(
                                "Copied to clipboard".to_string(),
                            ));
                        }
                    }
                    Err(err) => {
                        let _ = ui_command_tx.send(UiCommand::ShowNotification(format!(
                            "Clipboard init error: {}",
                            err
                        )));
                    }
                }
            } else {
                let _ =
                    ui_command_tx.send(UiCommand::ShowNotification("No logs selected".to_string()));
            }
        }
    }
}

fn handle_open_command_palette(ui_bridge: &mut UiCacheBridge) {
    ui_bridge.state.command_palette.is_open = true;
    ui_bridge.state.command_palette.items = vec!["Export Selected to LLM".to_string()];
    ui_bridge.state.command_palette.selected = 0;
}

fn handle_command_palette_submit(
    ui_bridge: &mut UiCacheBridge,
    cache_manager: &CacheManager,
    ui_command_tx: mpsc::UnboundedSender<UiCommand>,
    llm_provider: Arc<dyn LlmProvider>,
) {
    let selected = ui_bridge.state.command_palette.selected;
    let item = ui_bridge.state.command_palette.items.get(selected).cloned();
    ui_bridge.state.command_palette.is_open = false;

    if let Some(command) = item {
        match command.as_str() {
            "Export Selected to LLM" => {
                handle_export_selected(ui_bridge, cache_manager, ui_command_tx, llm_provider);
            }
            _ => {
                warn!(command, "unknown command from palette");
            }
        }
    }
}

fn handle_ui_command(command: UiCommand, ui_bridge: &mut UiCacheBridge) {
    match command {
        UiCommand::UpdateComposer(text) => {
            ui_bridge.state.input.text = text;
            ui_bridge.state.input.cursor = ui_bridge.state.input.text.len();
            ui_bridge.state.focus = UiFocus::Composer;
            ui_bridge.state.status_message = None;
        }
        UiCommand::ShowNotification(text) => {
            ui_bridge.state.status_message = Some(text);
        }
    }
}

fn handle_export_selected(
    ui_bridge: &mut UiCacheBridge,
    cache_manager: &CacheManager,
    ui_command_tx: mpsc::UnboundedSender<UiCommand>,
    llm_provider: Arc<dyn LlmProvider>,
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
    info!("exporting transcript to LLM");

    let _ = ui_command_tx.send(UiCommand::ShowNotification(
        "Processing export...".to_string(),
    ));

    tokio::spawn(async move {
        let request = LlmRequest {
            system_prompt: "You are a helpful assistant. Write a draft reply for the user based on the transcript. Keep it concise and natural.".to_string(),
            user_instruction: "Draft a reply to this conversation. Return ONLY the reply text, no preamble.".to_string(),
            transcript,
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

fn handle_composer_submit(ui_bridge: &mut UiCacheBridge, send_pipeline: &SendPipeline) {
    let draft = ui_bridge.state.input.text.clone();
    if draft.trim().is_empty() {
        return;
    }

    let Some(chat_id) = ui_bridge.selected_chat_id() else {
        warn!("no chat selected for composer submit");
        return;
    };
    let Some(peer) = ui_bridge.selected_peer() else {
        warn!(chat_id = chat_id.0, "missing peer ref for composer submit");
        return;
    };

    let request = SendRequest::SendText {
        peer,
        text: draft,
        reply_to: None,
    };
    match send_pipeline.enqueue(request) {
        Ok(ticket) => {
            info!(
                chat_id = chat_id.0,
                send_id = ticket.id.0,
                "queued composer send"
            );
            ui_bridge.state.input = InputState::default();
        }
        Err(err) => {
            warn!(
                chat_id = chat_id.0,
                error = %err,
                "failed to enqueue composer send"
            );
        }
    }
}

fn is_exit_key(key: &KeyEvent) -> bool {
    matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('c'), modifiers) | (KeyCode::Char('q'), modifiers)
            if modifiers.contains(KeyModifiers::CONTROL)
    )
}

fn apply_page_size(state: &mut UiState, terminal_area: Rect) {
    let chat_text_area = chat_list_text_area(terminal_area, state.chat_list_width);
    state.chat_list_pane.viewport.width = chat_text_area.width;
    state.chat_list_pane.viewport.height = chat_text_area.height;
    state.chat_list_pane.page_size = chat_text_area.height.max(1) as usize;
    clamp_chat_list_scroll(state);
    ensure_chat_list_selection_visible(state);
    clamp_chat_list_scroll(state);

    let page_size = message_page_size(terminal_area, state.chat_list_width);
    if state.message_view.pane.page_size != page_size {
        state.message_view.pane.page_size = page_size;
    }
    state.message_view.pane.viewport.height = page_size.max(1) as u16;

    let log_text_area = log_window_text_area(terminal_area);
    state.log_view.pane.viewport.width = log_text_area.width;
    state.log_view.pane.viewport.height = log_text_area.height;
    state.log_view.pane.page_size = log_text_area.height.max(1) as usize;
    let max_log_scroll = log_view_max_scroll(state);
    if state.log_view.pane.scroll_vertical > max_log_scroll {
        state.log_view.pane.scroll_vertical = max_log_scroll;
    }
    let max_log_horizontal = log_view_max_horizontal_scroll(state);
    if state.log_view.pane.scroll_horizontal > max_log_horizontal {
        state.log_view.pane.scroll_horizontal = max_log_horizontal;
    }
    let viewport_width = message_viewport_width(terminal_area, state.chat_list_width);
    state.message_view.pane.viewport.width = viewport_width;
    let max_scroll = ui::view::message_max_scroll(state);
    if state.message_view.pane.scroll_vertical > max_scroll {
        state.message_view.pane.scroll_vertical = max_scroll;
    }
    let max_horizontal = message_max_horizontal_scroll(state);
    if state.message_view.pane.scroll_horizontal > max_horizontal {
        state.message_view.pane.scroll_horizontal = max_horizontal;
    }

    let composer_area = composer_text_area(terminal_area, state.chat_list_width);
    state.composer_pane.viewport.width = composer_area.width;
    state.composer_pane.viewport.height = composer_area.height;
    state.composer_pane.page_size = composer_area.height.max(1) as usize;
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
    if was_at_bottom || state.log_view.pane.scroll_vertical > max_scroll {
        state.log_view.pane.scroll_vertical = max_scroll;
    }

    // Handle sticky scroll (auto-tail on open)
    if state.log_view.sticky_scroll && !state.logs.is_empty() {
        let width = (state.log_view.pane.viewport.width as usize).max(1);
        let metrics = log_pane_metrics(state, width);
        if metrics.line_count > 0 {
            let last_idx = metrics.line_count - 1;
            state.log_view.selection = Some((last_idx, last_idx));
            state.log_view.pane.scroll_vertical = max_scroll;
            state.log_view.sticky_scroll = false;
        }
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
    state.log_view.pane.scroll_vertical >= log_view_max_scroll(state)
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
        assert_eq!(message_page_size(area, 32), 3);
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
                selection: None,
                ..LogViewState::default()
            },
            ..UiState::default()
        };

        maybe_refresh_logs(&mut state, &path, 5, &mut None);

        assert!(state.logs.is_empty());
    }
}
