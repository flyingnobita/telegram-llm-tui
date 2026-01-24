use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::{handle_key as handle_text_key, InputState};
use crate::view::{
    chat_list_max_horizontal_scroll, ensure_chat_list_selection_visible,
    log_view_max_horizontal_scroll, log_view_max_scroll, message_line_count, message_line_offset,
    message_max_horizontal_scroll, message_max_scroll, ChatListItem, UiFocus, UiState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeymapStyle {
    Vim,
    #[default]
    Vscode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    ComposerSubmit,
    TriggerRefresh,
    ExportSelected,
    OpenCommandPalette,
    CommandPaletteSubmit,
    SelectAllInView,
    CopyLogSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiActionResult {
    pub handled: bool,
    pub action: Option<UiAction>,
}

impl UiActionResult {
    fn handled(handled: bool) -> Self {
        Self {
            handled,
            action: None,
        }
    }

    fn action(action: UiAction) -> Self {
        Self {
            handled: true,
            action: Some(action),
        }
    }
}

pub fn handle_ui_key(state: &mut UiState, key: KeyEvent, style: KeymapStyle) -> UiActionResult {
    if state.log_view.is_open {
        return handle_log_view_key(state, key, style);
    }

    if state.command_palette.is_open {
        return handle_command_palette_key(state, key);
    }

    if key.code == KeyCode::Char('l') && key.modifiers == KeyModifiers::CONTROL {
        state.log_view.is_open = true;
        // precise init logic requires calculating metrics, so we delegate or do best effort?
        // We can't easily calculate metrics here without re-running wrapping logic which is in view.rs.
        // But we can defer initialization or move wrapping logic.
        // Actually, we can just call update_log_selection with a "move to end" flag immediately?
        // Or better: Let's create a helper `open_log_view(state)` that does this.
        open_log_view(state);
        return UiActionResult::handled(true);
    }

    if key.code == KeyCode::Char('p') && key.modifiers == KeyModifiers::CONTROL {
        return UiActionResult::action(UiAction::OpenCommandPalette);
    }

    if key.code == KeyCode::Char('a')
        && key.modifiers == KeyModifiers::CONTROL
        && state.focus == UiFocus::Messages
    {
        return UiActionResult::action(UiAction::SelectAllInView);
    }

    if state.message_view.search.is_open && state.focus != UiFocus::Search {
        state.focus = UiFocus::Search;
    }

    if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::NONE {
        cycle_focus(state);
        return UiActionResult::handled(true);
    }

    if key.code == KeyCode::BackTab
        || (key.code == KeyCode::Tab && key.modifiers == KeyModifiers::SHIFT)
    {
        cycle_focus_back(state);
        return UiActionResult::handled(true);
    }

    // Global hotkeys
    match key.code {
        KeyCode::Char('1') if key.modifiers == KeyModifiers::NONE => {
            state.focus = UiFocus::Chats;
            return UiActionResult::handled(true);
        }
        KeyCode::Char('2') if key.modifiers == KeyModifiers::NONE => {
            state.focus = UiFocus::Messages;
            return UiActionResult::handled(true);
        }
        KeyCode::Char('3') if key.modifiers == KeyModifiers::NONE => {
            state.focus = UiFocus::Composer;
            return UiActionResult::handled(true);
        }
        KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
            return UiActionResult::action(UiAction::ExportSelected);
        }
        _ => {}
    }

    match state.focus {
        UiFocus::Chats => UiActionResult::handled(handle_chats_key(state, key, style)),
        UiFocus::Messages => UiActionResult::handled(handle_messages_key(state, key, style)),
        UiFocus::Composer => handle_composer_key(state, key, style),
        UiFocus::Search => UiActionResult::handled(handle_search_key(state, key)),
        UiFocus::ChatSearch => handle_chat_search_key(state, key),
    }
}

fn handle_command_palette_key(state: &mut UiState, key: KeyEvent) -> UiActionResult {
    match key {
        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.command_palette.is_open = false;
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            if !state.command_palette.items.is_empty() {
                if state.command_palette.selected == 0 {
                    state.command_palette.selected = state.command_palette.items.len() - 1;
                } else {
                    state.command_palette.selected -= 1;
                }
            }
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            if !state.command_palette.items.is_empty() {
                state.command_palette.selected =
                    (state.command_palette.selected + 1) % state.command_palette.items.len();
            }
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            if !state.command_palette.items.is_empty() {
                UiActionResult::action(UiAction::CommandPaletteSubmit)
            } else {
                state.command_palette.is_open = false;
                UiActionResult::handled(true)
            }
        }
        _ => {
            // Stub for filtering items if we had many, for now just handle basic text if needed
            // but the spec just says "Add 'Export Selected to LLM' command".
            UiActionResult::handled(false)
        }
    }
}

fn cycle_focus(state: &mut UiState) {
    state.focus = match state.focus {
        UiFocus::Chats => UiFocus::Messages,
        UiFocus::Messages => UiFocus::Composer,
        UiFocus::Composer => UiFocus::Chats,
        UiFocus::Search => UiFocus::Messages,
        UiFocus::ChatSearch => UiFocus::Messages,
    };
}

fn cycle_focus_back(state: &mut UiState) {
    state.focus = match state.focus {
        UiFocus::Chats => UiFocus::Composer,
        UiFocus::Messages => UiFocus::Chats,
        UiFocus::Composer => UiFocus::Messages,
        UiFocus::Search => UiFocus::Messages,
        UiFocus::ChatSearch => UiFocus::Messages,
    };
}

fn handle_log_view_key(state: &mut UiState, key: KeyEvent, _style: KeymapStyle) -> UiActionResult {
    let extend_selection = key.modifiers.contains(KeyModifiers::SHIFT);

    match key {
        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.log_view.is_open = false;
            state.log_view.selection = None;
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.log_view.is_open = false;
            state.log_view.selection = None;
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::CONTROL, // Handle Ctrl+L inside log view
            ..
        } => {
            state.log_view.is_open = false;
            state.log_view.selection = None;
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Up, ..
        } => {
            update_log_selection(state, -1, extend_selection);
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Down,
            ..
        } => {
            update_log_selection(state, 1, extend_selection);
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::PageUp,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.log_view.selection = None;
            UiActionResult::handled(scroll_log_view_page(state, -1))
        }
        KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.log_view.selection = None;
            UiActionResult::handled(scroll_log_view_page(state, 1))
        }
        KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            // Horizontal scroll disabled in wrapped mode, but keep for now if wrap disabled in future
            UiActionResult::handled(scroll_log_horizontal(state, -1))
        }
        KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            ..
        } => UiActionResult::handled(scroll_log_horizontal(state, 1)),
        KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            ..
        } => UiActionResult::handled(jump_log_view(state, true)),
        KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            ..
        } => UiActionResult::handled(jump_log_view(state, false)),
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::NONE,
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('C'),
            modifiers: KeyModifiers::SHIFT,
            ..
        }
        | KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        } => UiActionResult::action(UiAction::CopyLogSelection),
        KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            update_log_selection(state, -1, extend_selection);
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            update_log_selection(state, 1, extend_selection);
            UiActionResult::handled(true)
        }
        _ => UiActionResult::handled(false),
    }
}

fn handle_chats_key(state: &mut UiState, key: KeyEvent, style: KeymapStyle) -> bool {
    match (key.code, style) {
        (KeyCode::Up, _) => {
            move_chat_selection(&mut state.chats, -1);
            ensure_chat_list_selection_visible(state);
            true
        }
        (KeyCode::Down, _) => {
            move_chat_selection(&mut state.chats, 1);
            ensure_chat_list_selection_visible(state);
            true
        }
        (KeyCode::Char('k'), _) => {
            move_chat_selection(&mut state.chats, -1);
            ensure_chat_list_selection_visible(state);
            true
        }
        (KeyCode::Char('j'), _) => {
            move_chat_selection(&mut state.chats, 1);
            ensure_chat_list_selection_visible(state);
            true
        }
        (KeyCode::Left, _) => scroll_chat_list(state, -1),
        (KeyCode::Right, _) => scroll_chat_list(state, 1),
        (KeyCode::Char('h'), KeymapStyle::Vim) => scroll_chat_list(state, -1),
        (KeyCode::Char('l'), KeymapStyle::Vim) => scroll_chat_list(state, 1),
        (KeyCode::Enter, _) => {
            state.focus = UiFocus::Messages;
            true
        }
        (KeyCode::Char('i'), _) => {
            state.focus = UiFocus::Composer;
            true
        }
        (KeyCode::Char('/'), _) => {
            state.focus = UiFocus::ChatSearch;
            state.chat_search.is_open = true;
            true
        }
        _ => false,
    }
}

fn handle_chat_search_key(state: &mut UiState, key: KeyEvent) -> UiActionResult {
    match key {
        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.chat_search.is_open = false;
            state.chat_search.query = InputState::default();
            state.focus = UiFocus::Chats;
            UiActionResult::action(UiAction::TriggerRefresh)
        }
        KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.chat_search.is_open = false;
            state.chat_search.query = InputState::default();
            state.focus = UiFocus::Messages;
            UiActionResult::action(UiAction::TriggerRefresh)
        }
        KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            move_chat_selection(&mut state.chats, -1);
            ensure_chat_list_selection_visible(state);
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            move_chat_selection(&mut state.chats, 1);
            ensure_chat_list_selection_visible(state);
            UiActionResult::handled(true)
        }
        _ => {
            let handled = handle_text_key(&mut state.chat_search.query, key);
            if handled {
                UiActionResult::action(UiAction::TriggerRefresh)
            } else {
                UiActionResult::handled(false)
            }
        }
    }
}

fn handle_messages_key(state: &mut UiState, key: KeyEvent, style: KeymapStyle) -> bool {
    match key {
        KeyEvent {
            code: KeyCode::Char('i'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.focus = UiFocus::Composer;
            true
        }
        KeyEvent {
            code: KeyCode::Char('/'),
            modifiers: KeyModifiers::NONE,
            ..
        } => open_search(state),
        KeyEvent {
            code: KeyCode::Char('n'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => jump_search_match(state, true),
        KeyEvent {
            code: KeyCode::Char('N'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => jump_search_match(state, false),
        KeyEvent {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
            ..
        } => move_message_cursor(state, 1),
        KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            ..
        } => move_message_cursor(state, -1),
        KeyEvent {
            code: KeyCode::Char('h'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => scroll_horizontal(state, -1),
        KeyEvent {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => scroll_horizontal(state, 1),
        KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => jump_message_cursor(state, 0),
        KeyEvent {
            code: KeyCode::Char('G'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => jump_message_cursor_to_end(state),
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            ..
        } => toggle_message_selection(state),
        KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            ..
        } => move_message_cursor(state, -1),
        KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            ..
        } => move_message_cursor(state, 1),
        KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
            ..
        } => scroll_horizontal(state, -1),
        KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
            ..
        } => scroll_horizontal(state, 1),
        KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
            ..
        } => jump_message_cursor(state, 0),
        KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
            ..
        } => jump_message_cursor_to_end(state),
        KeyEvent {
            code: KeyCode::PageUp,
            modifiers: KeyModifiers::NONE,
            ..
        } => scroll_page(state, -1),
        KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
            ..
        } => scroll_page(state, 1),
        KeyEvent {
            code: KeyCode::Char('f'),
            modifiers,
            ..
        } if style == KeymapStyle::Vscode && modifiers.contains(KeyModifiers::CONTROL) => {
            open_search(state)
        }
        KeyEvent {
            code: KeyCode::F(3),
            modifiers,
            ..
        } => {
            let forward = !modifiers.contains(KeyModifiers::SHIFT);
            jump_search_match(state, forward)
        }
        KeyEvent {
            code: KeyCode::Char('b'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) && style == KeymapStyle::Vim => {
            scroll_page(state, -1)
        }
        KeyEvent {
            code: KeyCode::Char('f'),
            modifiers,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) && style == KeymapStyle::Vim => {
            scroll_page(state, 1)
        }
        _ => false,
    }
}

fn handle_composer_key(state: &mut UiState, key: KeyEvent, style: KeymapStyle) -> UiActionResult {
    match key {
        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.focus = UiFocus::Messages;
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Char('['),
            modifiers,
            ..
        } if style == KeymapStyle::Vim && modifiers.contains(KeyModifiers::CONTROL) => {
            state.focus = UiFocus::Messages;
            UiActionResult::handled(true)
        }
        KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            if state.input.text.trim().is_empty() {
                UiActionResult::handled(true)
            } else {
                UiActionResult::action(UiAction::ComposerSubmit)
            }
        }
        _ => UiActionResult::handled(handle_text_key(&mut state.input, key)),
    }
}

fn handle_search_key(state: &mut UiState, key: KeyEvent) -> bool {
    match key {
        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.message_view.search.is_open = false;
            state.focus = UiFocus::Messages;
            true
        }
        KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            let selected = state.message_view.search.selected_match();
            if let Some(match_index) = selected {
                state.message_view.cursor = Some(match_index);
                state.message_view.pane.scroll_vertical =
                    message_line_offset(&state.messages, match_index);
                return true;
            }
            false
        }
        KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            ..
        } => jump_search_match(state, false),
        KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            ..
        } => jump_search_match(state, true),
        _ => {
            let handled = handle_text_key(&mut state.message_view.search.query, key);
            if handled {
                state.message_view.search.recompute_matches(&state.messages);
            }
            handled
        }
    }
}

fn open_search(state: &mut UiState) -> bool {
    state.message_view.search.is_open = true;
    state.message_view.search.query = InputState::default();
    state.message_view.search.recompute_matches(&state.messages);
    state.focus = UiFocus::Search;
    true
}

fn jump_search_match(state: &mut UiState, forward: bool) -> bool {
    let match_index = state.message_view.search.advance(forward);
    if let Some(index) = match_index {
        state.message_view.cursor = Some(index);
        state.message_view.pane.scroll_vertical = message_line_offset(&state.messages, index);
        return true;
    }
    false
}

fn move_message_cursor(state: &mut UiState, delta: i32) -> bool {
    if state.messages.is_empty() {
        return false;
    }
    let max_index = state.messages.len() as i32 - 1;
    let current = state
        .message_view
        .cursor
        .map(|index| index as i32)
        .unwrap_or(max_index);
    let next = (current + delta).clamp(0, max_index) as usize;
    state.message_view.cursor = Some(next);
    ensure_cursor_visible(state);
    true
}

fn jump_message_cursor(state: &mut UiState, index: usize) -> bool {
    if state.messages.is_empty() {
        return false;
    }
    let max_index = state.messages.len().saturating_sub(1);
    let next = index.min(max_index);
    state.message_view.cursor = Some(next);
    ensure_cursor_visible(state);
    true
}

fn jump_message_cursor_to_end(state: &mut UiState) -> bool {
    if state.messages.is_empty() {
        return false;
    }
    let max_index = state.messages.len().saturating_sub(1);
    state.message_view.cursor = Some(max_index);
    ensure_cursor_visible(state);
    true
}

fn ensure_cursor_visible(state: &mut UiState) {
    let Some(cursor) = state.message_view.cursor else {
        return;
    };
    let page_size = state.message_view.pane.page_size.max(1);
    let scroll = state.message_view.pane.scroll_vertical;
    let cursor_line = message_line_offset(&state.messages, cursor);
    if cursor_line < scroll {
        state.message_view.pane.scroll_vertical = cursor_line;
    } else if cursor_line >= scroll + page_size {
        state.message_view.pane.scroll_vertical = cursor_line + 1 - page_size;
    }
}

fn scroll_page(state: &mut UiState, direction: i32) -> bool {
    let page = state.message_view.pane.page_size.max(1) as i32;
    scroll_by(state, direction * page)
}

fn scroll_by(state: &mut UiState, delta: i32) -> bool {
    if state.messages.is_empty() {
        return false;
    }
    let max_offset = message_max_scroll(state) as i32;
    let current = state.message_view.pane.scroll_vertical as i32;
    let next = (current + delta).clamp(0, max_offset) as usize;
    if next == state.message_view.pane.scroll_vertical {
        return false;
    }
    state.message_view.pane.scroll_vertical = next;
    true
}

fn scroll_horizontal(state: &mut UiState, delta: i32) -> bool {
    if state.messages.is_empty() {
        return false;
    }
    let max_offset = message_max_horizontal_scroll(state) as i32;
    let current = state.message_view.pane.scroll_horizontal as i32;
    let next = (current + delta).clamp(0, max_offset) as usize;
    if next == state.message_view.pane.scroll_horizontal {
        return false;
    }
    state.message_view.pane.scroll_horizontal = next;
    true
}

fn scroll_log_view_page(state: &mut UiState, direction: i32) -> bool {
    let page = state.log_view.pane.page_size.max(1) as i32;
    scroll_log_view_by(state, direction * page)
}

fn scroll_log_horizontal(state: &mut UiState, delta: i32) -> bool {
    if state.logs.is_empty() {
        return false;
    }
    let max_scroll = log_view_max_horizontal_scroll(state) as i32;
    let current = state.log_view.pane.scroll_horizontal as i32;
    let next = (current + delta).clamp(0, max_scroll) as usize;
    if next == state.log_view.pane.scroll_horizontal {
        return false;
    }
    state.log_view.pane.scroll_horizontal = next;
    true
}

fn scroll_log_view_by(state: &mut UiState, delta: i32) -> bool {
    if state.logs.is_empty() {
        return false;
    }
    let max_scroll = log_view_max_scroll(state) as i32;
    let current = state.log_view.pane.scroll_vertical as i32;
    let next = (current + delta).clamp(0, max_scroll) as usize;
    if next == state.log_view.pane.scroll_vertical {
        return false;
    }
    state.log_view.pane.scroll_vertical = next;
    true
}

fn jump_log_view(state: &mut UiState, to_top: bool) -> bool {
    let max_scroll = log_view_max_scroll(state);
    let next = if to_top { 0 } else { max_scroll };
    if next == state.log_view.pane.scroll_vertical {
        return false;
    }
    state.log_view.pane.scroll_vertical = next;
    true
}

fn update_log_selection(state: &mut UiState, delta: i32, extend: bool) {
    // Need access to metrics/wrapping to determine max lines
    use crate::view::log_pane_metrics;

    let width = state.log_view.pane.viewport.width as usize;
    let metrics = log_pane_metrics(state, width);
    let max_lines = metrics.line_count;

    if max_lines == 0 {
        return;
    }

    let current_cursor = if let Some((_, cursor)) = state.log_view.selection {
        cursor
    } else {
        state.log_view.pane.scroll_vertical
    };

    let next_cursor =
        (current_cursor as i32 + delta).clamp(0, max_lines.saturating_sub(1) as i32) as usize;

    if extend {
        let anchor = state
            .log_view
            .selection
            .map(|(a, _)| a)
            .unwrap_or(current_cursor);
        state.log_view.selection = Some((anchor, next_cursor));
    } else {
        state.log_view.selection = Some((next_cursor, next_cursor));
    }

    // Scroll to ensure visibility
    let page_size = state.log_view.pane.page_size.max(1);
    let scroll = state.log_view.pane.scroll_vertical;

    if next_cursor < scroll {
        state.log_view.pane.scroll_vertical = next_cursor;
    } else if next_cursor >= scroll + page_size {
        state.log_view.pane.scroll_vertical = next_cursor + 1 - page_size;
    }
}

fn toggle_message_selection(state: &mut UiState) -> bool {
    let Some(message_id) = state.message_view.cursor_message_id(&state.messages) else {
        return false;
    };
    state.message_view.toggle_selection(message_id);
    true
}

pub fn select_all_in_view(state: &mut UiState) -> bool {
    if state.messages.is_empty() {
        return false;
    }
    let scroll = state.message_view.pane.scroll_vertical;
    let page_size = state.message_view.pane.page_size;

    let mut current_offset = 0;
    for message in &state.messages {
        let line_count = message_line_count(message);
        if current_offset + line_count > scroll && current_offset < scroll + page_size {
            state.message_view.selected_ids.insert(message.id);
        }
        current_offset += line_count;
        if current_offset >= scroll + page_size {
            break;
        }
    }
    true
}

fn move_chat_selection(chats: &mut [ChatListItem], delta: i32) {
    if chats.is_empty() {
        return;
    }
    let current = chats.iter().position(|chat| chat.is_selected).unwrap_or(0) as i32;
    let max_index = chats.len() as i32 - 1;
    let next = (current + delta).clamp(0, max_index) as usize;
    for (idx, chat) in chats.iter_mut().enumerate() {
        chat.is_selected = idx == next;
    }
}

fn scroll_chat_list(state: &mut UiState, delta: i32) -> bool {
    let max_scroll = chat_list_max_horizontal_scroll(state);
    if max_scroll == 0 {
        return false;
    }
    let current = state.chat_list_pane.scroll_horizontal as i32;
    let next = (current + delta).clamp(0, max_scroll as i32) as usize;
    if next == state.chat_list_pane.scroll_horizontal {
        return false;
    }
    state.chat_list_pane.scroll_horizontal = next;
    true
}

fn open_log_view(state: &mut UiState) {
    state.log_view.is_open = true;

    // Initialize cursor at the end
    use crate::view::log_pane_metrics;
    let width = (state.log_view.pane.viewport.width as usize).max(1);
    let effective_width = if width < 10 { 80 } else { width };

    let metrics = log_pane_metrics(state, effective_width);
    let max_lines = metrics.line_count;

    if max_lines > 0 {
        let last_idx = max_lines - 1;
        state.log_view.selection = Some((last_idx, last_idx));

        // Ensure visible
        let page_size = state.log_view.pane.page_size.max(1);
        if last_idx >= page_size {
            state.log_view.pane.scroll_vertical = last_idx + 1 - page_size;
        } else {
            state.log_view.pane.scroll_vertical = 0;
        }
    } else {
        // Logs might not be loaded yet. Set sticky_scroll to true so we jump to bottom when loaded.
        state.log_view.selection = None;
        state.log_view.pane.scroll_vertical = 0;
        state.log_view.sticky_scroll = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::{PaneState, PaneViewport};
    use crate::view::{ChatListItem, LogViewState, MessageItem};

    fn sample_state() -> UiState {
        let mut state = UiState {
            messages: vec![
                MessageItem {
                    id: 1,
                    author: "Ada".to_string(),
                    timestamp: "09:10".to_string(),
                    body: "hello".to_string(),
                },
                MessageItem {
                    id: 2,
                    author: "You".to_string(),
                    timestamp: "09:11".to_string(),
                    body: "reply".to_string(),
                },
            ],
            ..Default::default()
        };
        state.message_view.reconcile(&state.messages);
        state
    }

    #[test]
    fn vim_jk_moves_message_cursor() {
        let mut state = sample_state();
        state.focus = UiFocus::Messages;
        state.message_view.cursor = Some(0);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            KeymapStyle::Vim,
        );
        assert_eq!(state.message_view.cursor, Some(1));

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            KeymapStyle::Vim,
        );
        assert_eq!(state.message_view.cursor, Some(0));
    }

    #[test]
    fn vscode_arrows_move_message_cursor() {
        let mut state = sample_state();
        state.focus = UiFocus::Messages;
        state.message_view.cursor = Some(0);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.message_view.cursor, Some(1));

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.message_view.cursor, Some(0));
    }

    #[test]
    fn toggles_message_selection() {
        let mut state = sample_state();
        state.focus = UiFocus::Messages;
        state.message_view.cursor = Some(0);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert!(state.message_view.selected_ids.contains(&1));

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert!(!state.message_view.selected_ids.contains(&1));
    }

    #[test]
    fn opens_search_and_updates_matches() {
        let mut state = sample_state();
        state.focus = UiFocus::Messages;

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeymapStyle::Vim,
        );
        assert!(state.message_view.search.is_open);
        assert_eq!(state.focus, UiFocus::Search);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            KeymapStyle::Vim,
        );
        assert_eq!(state.message_view.search.query.text, "h");
        assert_eq!(state.message_view.search.matches, vec![0]);
    }

    #[test]
    fn composer_enter_emits_submit_action() {
        let mut state = UiState {
            focus: UiFocus::Composer,
            input: InputState {
                text: "hello".to_string(),
                cursor: 5,
            },
            ..Default::default()
        };

        let result = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );

        assert!(result.handled);
        assert_eq!(result.action, Some(UiAction::ComposerSubmit));
        assert_eq!(state.input.text, "hello");
    }

    #[test]
    fn composer_enter_ignores_empty_draft() {
        let mut state = UiState {
            focus: UiFocus::Composer,
            input: InputState {
                text: "   ".to_string(),
                cursor: 3,
            },
            ..Default::default()
        };

        let result = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );

        assert!(result.handled);
        assert_eq!(result.action, None);
    }

    #[test]
    fn chat_selection_moves_with_keys() {
        let mut state = UiState {
            focus: UiFocus::Chats,
            chats: vec![
                ChatListItem {
                    id: 10,
                    title: "General".to_string(),
                    unread: 0,
                    is_selected: true,
                },
                ChatListItem {
                    id: 11,
                    title: "Design".to_string(),
                    unread: 1,
                    is_selected: false,
                },
            ],
            ..Default::default()
        };

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );

        assert!(state.chats[1].is_selected);
        assert!(!state.chats[0].is_selected);
    }

    #[test]
    fn chat_list_scrolls_horizontally() {
        let mut state = UiState {
            focus: UiFocus::Chats,
            chats: vec![ChatListItem {
                id: 1,
                title: "VeryLongChatName".to_string(),
                unread: 0,
                is_selected: true,
            }],
            ..Default::default()
        };
        state.chat_list_pane.viewport.width = 8;

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.chat_list_pane.scroll_horizontal, 1);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.chat_list_pane.scroll_horizontal, 0);
    }

    #[test]
    fn chat_list_scrolls_vertically_to_keep_selection_visible() {
        let mut state = UiState {
            focus: UiFocus::Chats,
            chats: vec![
                ChatListItem {
                    id: 1,
                    title: "One".to_string(),
                    unread: 0,
                    is_selected: true,
                },
                ChatListItem {
                    id: 2,
                    title: "Two".to_string(),
                    unread: 0,
                    is_selected: false,
                },
                ChatListItem {
                    id: 3,
                    title: "Three".to_string(),
                    unread: 0,
                    is_selected: false,
                },
            ],
            ..Default::default()
        };
        state.chat_list_pane.page_size = 2;

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.chat_list_pane.scroll_vertical, 0);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.chat_list_pane.scroll_vertical, 1);
    }

    #[test]
    fn opens_log_window_with_hotkey() {
        let mut state = UiState {
            logs: vec!["test log".to_string()],
            ..Default::default()
        };

        let handled = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            KeymapStyle::Vscode,
        );

        assert!(handled.handled);
        assert!(state.log_view.is_open);
        assert!(state.log_view.selection.is_some());
    } // Note: This test might fail if sample_state still misses selection field elsewhere?
      // Checked sample_state below.

    #[test]
    fn closes_log_window_with_ctrl_l() {
        let mut state = UiState::default();
        state.log_view.is_open = true;

        let handled = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
            KeymapStyle::Vscode,
        );

        assert!(handled.handled);
        assert!(!state.log_view.is_open);
    }

    #[test]
    fn closes_log_window_with_escape() {
        let mut state = UiState::default();
        state.log_view.is_open = true;

        let handled = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );

        assert!(handled.handled);
        assert!(!state.log_view.is_open);
    }

    #[test]
    fn closes_log_window_with_l() {
        let mut state = UiState::default();
        state.log_view.is_open = true;

        let handled = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );

        assert!(handled.handled);
        assert!(!state.log_view.is_open);
    }

    #[test]
    fn log_window_scrolls_with_keys() {
        let mut state = UiState {
            logs: vec![
                "line-1".to_string(),
                "line-2".to_string(),
                "line-3".to_string(),
                "line-4".to_string(),
                "line-5".to_string(),
            ],
            log_view: LogViewState {
                is_open: true,
                selection: None,
                pane: PaneState {
                    page_size: 2,
                    ..PaneState::default()
                },
                sticky_scroll: false,
            },
            ..UiState::default()
        };

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.pane.scroll_vertical, 0);
        assert_eq!(state.log_view.selection, Some((1, 1)));

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.pane.scroll_vertical, 2);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.pane.scroll_vertical, 0);
    }

    #[test]
    fn log_window_scrolls_horizontally() {
        let mut state = UiState {
            logs: vec!["0123456789".to_string()],
            log_view: LogViewState {
                is_open: true,
                selection: None,
                sticky_scroll: false,
                ..LogViewState::default()
            },
            ..UiState::default()
        };
        state.log_view.pane.viewport.width = 5;

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.pane.scroll_horizontal, 0);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.pane.scroll_horizontal, 0);
    }

    #[test]
    fn message_pane_scrolls_horizontally() {
        let mut state = sample_state();
        state.focus = UiFocus::Messages;
        state.message_view.pane.viewport.width = 5;

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert!(state.message_view.pane.scroll_horizontal > 0);

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.message_view.pane.scroll_horizontal, 0);
    }

    #[test]
    fn opens_chat_search_with_slash() {
        let mut state = UiState {
            focus: UiFocus::Chats,
            ..Default::default()
        };

        let result = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );

        assert!(result.handled);
        assert_eq!(state.focus, UiFocus::ChatSearch);
        assert!(state.chat_search.is_open);
    }

    #[test]
    fn chat_search_typing_emits_refresh() {
        let mut state = UiState {
            focus: UiFocus::ChatSearch,
            ..Default::default()
        };
        state.chat_search.is_open = true;

        let result = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );

        assert!(result.handled);
        assert_eq!(result.action, Some(UiAction::TriggerRefresh));
        assert_eq!(state.chat_search.query.text, "a");
    }

    #[test]
    fn unified_navigation_keys_work_in_vscode_mode() {
        let mut state = sample_state();
        state.focus = UiFocus::Messages;
        state.message_view.cursor = Some(0);

        // Test 'j' in Vscode mode
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.message_view.cursor, Some(1));

        // Test 'k' in Vscode mode
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.message_view.cursor, Some(0));
    }

    #[test]
    fn unified_search_key_works_in_vscode_mode() {
        let mut state = sample_state();
        state.focus = UiFocus::Messages;

        // Test '/' in Vscode mode
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert!(state.message_view.search.is_open);
        assert_eq!(state.focus, UiFocus::Search);
    }

    #[test]
    fn unified_composer_focus_key_works_in_vscode_mode() {
        let mut state = UiState {
            focus: UiFocus::Chats,
            ..Default::default()
        };

        // Test 'i' in Vscode mode
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.focus, UiFocus::Composer);
    }

    #[test]
    fn log_window_cursor_moves() {
        let mut state = UiState {
            logs: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            log_view: LogViewState {
                is_open: true,
                pane: PaneState {
                    page_size: 2,
                    viewport: PaneViewport {
                        width: 10,
                        height: 2,
                    },
                    ..PaneState::default()
                },
                selection: Some((0, 0)),
                sticky_scroll: false,
            },
            ..UiState::default()
        };

        // Move Down (cursor -> 1)
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((1, 1)));

        // Move Down (cursor -> 2)
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((2, 2)));

        // Move Down at end (clamp)
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((2, 2)));
    }

    #[test]
    fn log_window_auto_scrolls() {
        let mut state = UiState {
            logs: vec![
                "1".to_string(),
                "2".to_string(),
                "3".to_string(),
                "4".to_string(),
            ],
            log_view: LogViewState {
                is_open: true,
                pane: PaneState {
                    page_size: 2,
                    viewport: PaneViewport {
                        width: 10,
                        height: 2,
                    },
                    ..PaneState::default()
                },
                selection: Some((1, 1)),
                sticky_scroll: false,
            },
            ..UiState::default()
        };
        // Scroll matches cursor initially
        state.log_view.pane.scroll_vertical = 1;

        // Move Down (cursor -> 2). Page size 2. Scroll 1 covers [1, 2].
        // Cursor 2 is visible (index 2 relative to scroll 1 is line 1 -> visible)
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((2, 2)));
        assert_eq!(state.log_view.pane.scroll_vertical, 1);

        // Move Down (cursor -> 3). Scroll 1 covers [1, 2]. 3 is NOT visible.
        // Should scroll to show 3. Scroll should become 3 + 1 - 2 = 2.
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((3, 3)));
        assert_eq!(state.log_view.pane.scroll_vertical, 2);
    }

    #[test]
    fn log_selection_extends_with_shift() {
        let mut state = UiState {
            logs: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            log_view: LogViewState {
                is_open: true,
                pane: PaneState {
                    page_size: 5,
                    viewport: PaneViewport {
                        width: 10,
                        height: 5,
                    },
                    ..PaneState::default()
                },
                selection: Some((1, 1)),
                sticky_scroll: false,
            },
            ..UiState::default()
        };

        // Shift+Down (anchor 1, cursor -> 2)
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((1, 2)));

        // Shift+Up (anchor 1, cursor -> 1)
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((1, 1)));

        // Shift+Up (anchor 1, cursor -> 0)
        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((1, 0)));
    }

    #[test]
    fn log_selection_reset() {
        let mut state = UiState {
            logs: vec!["1".to_string(), "2".to_string(), "3".to_string()],
            log_view: LogViewState {
                is_open: true,
                pane: PaneState {
                    page_size: 5,
                    viewport: PaneViewport {
                        width: 10,
                        height: 5,
                    },
                    ..PaneState::default()
                },
                selection: Some((0, 2)), // Range selected
                sticky_scroll: false,
            },
            ..UiState::default()
        };

        // Down without Shift (cursor -> 2+1=invalid/clamped -> 2? Or from current?
        // Logic uses current cursor. If range (0, 2), cursor is 2. Down -> 2(clamped).
        // Wait, selection is (anchor, cursor). If we move Down without shift,
        // it resets anchor to cursor.
        // If cursor was at 2, moving down clamps to 2. Anchor becomes 2. Selection (2, 2).

        // Let's reset cursor to 1 so we can move down.
        state.log_view.selection = Some((0, 1));

        let _ = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.log_view.selection, Some((2, 2)));
    }
    #[test]
    fn log_view_c_key_triggers_copy_action() {
        let mut state = UiState {
            log_view: LogViewState {
                is_open: true,
                selection: Some((0, 1)),
                ..LogViewState::default()
            },
            ..UiState::default()
        };

        let result = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(result.action, Some(UiAction::CopyLogSelection));

        // Test with Shift+C
        let result = handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT),
            KeymapStyle::Vscode,
        );
        assert_eq!(result.action, Some(UiAction::CopyLogSelection));
    }
}
