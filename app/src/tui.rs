use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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
use ui::view::{chat_list_max_scroll, chat_list_viewport_width, message_viewport_page_size};

use crate::ui_state::UiCacheBridge;

const DRAW_INTERVAL_MS: u64 = 250;
const INPUT_POLL_MS: u64 = 100;

pub async fn run_tui_loop(
    cache_manager: &CacheManager,
    ui_bridge: &mut UiCacheBridge,
    mut event_rx: EventReceiver,
    keymap: KeymapStyle,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("starting tui runtime");
    let mut tui = Tui::new()?;
    let (input_tx, mut input_rx) = mpsc::unbounded_channel();
    let running = Arc::new(AtomicBool::new(true));
    let input_handle = spawn_input_thread(running.clone(), input_tx);

    let mut ticker = tokio::time::interval(Duration::from_millis(DRAW_INTERVAL_MS));
    let mut should_exit = false;
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
}

fn message_page_size(terminal_area: Rect, chat_list_width: u16) -> usize {
    message_viewport_page_size(terminal_area, chat_list_width)
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
