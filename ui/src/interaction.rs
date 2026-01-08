use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::{handle_key as handle_text_key, InputState};
use crate::view::{ChatListItem, UiFocus, UiState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeymapStyle {
    Vim,
    #[default]
    Vscode,
}

pub fn handle_ui_key(state: &mut UiState, key: KeyEvent, style: KeymapStyle) -> bool {
    if state.message_view.search.is_open && state.focus != UiFocus::Search {
        state.focus = UiFocus::Search;
    }

    if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::NONE {
        cycle_focus(state);
        return true;
    }

    match state.focus {
        UiFocus::Chats => handle_chats_key(state, key, style),
        UiFocus::Messages => handle_messages_key(state, key, style),
        UiFocus::Composer => handle_composer_key(state, key, style),
        UiFocus::Search => handle_search_key(state, key),
    }
}

fn cycle_focus(state: &mut UiState) {
    state.focus = match state.focus {
        UiFocus::Chats => UiFocus::Messages,
        UiFocus::Messages => UiFocus::Composer,
        UiFocus::Composer => UiFocus::Chats,
        UiFocus::Search => UiFocus::Messages,
    };
}

fn handle_chats_key(state: &mut UiState, key: KeyEvent, style: KeymapStyle) -> bool {
    match (key.code, style) {
        (KeyCode::Up, _) => {
            move_chat_selection(&mut state.chats, -1);
            true
        }
        (KeyCode::Down, _) => {
            move_chat_selection(&mut state.chats, 1);
            true
        }
        (KeyCode::Char('k'), KeymapStyle::Vim) => {
            move_chat_selection(&mut state.chats, -1);
            true
        }
        (KeyCode::Char('j'), KeymapStyle::Vim) => {
            move_chat_selection(&mut state.chats, 1);
            true
        }
        (KeyCode::Enter, _) => {
            state.focus = UiFocus::Messages;
            true
        }
        (KeyCode::Char('i'), KeymapStyle::Vim) => {
            state.focus = UiFocus::Composer;
            true
        }
        _ => false,
    }
}

fn handle_messages_key(state: &mut UiState, key: KeyEvent, style: KeymapStyle) -> bool {
    match key {
        KeyEvent {
            code: KeyCode::Char('i'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => {
            state.focus = UiFocus::Composer;
            true
        }
        KeyEvent {
            code: KeyCode::Char('/'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => open_search(state),
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
        } if style == KeymapStyle::Vim => move_message_cursor(state, 1),
        KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            ..
        } if style == KeymapStyle::Vim => move_message_cursor(state, -1),
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

fn handle_composer_key(state: &mut UiState, key: KeyEvent, style: KeymapStyle) -> bool {
    match key {
        KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            state.focus = UiFocus::Messages;
            true
        }
        KeyEvent {
            code: KeyCode::Char('['),
            modifiers,
            ..
        } if style == KeymapStyle::Vim && modifiers.contains(KeyModifiers::CONTROL) => {
            state.focus = UiFocus::Messages;
            true
        }
        _ => handle_text_key(&mut state.input, key),
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
                state.message_view.scroll_offset = match_index;
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
        state.message_view.scroll_offset = index;
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
    let page_size = state.message_view.page_size.max(1);
    let scroll = state.message_view.scroll_offset;
    if cursor < scroll {
        state.message_view.scroll_offset = cursor;
    } else if cursor >= scroll + page_size {
        state.message_view.scroll_offset = cursor + 1 - page_size;
    }
}

fn scroll_page(state: &mut UiState, direction: i32) -> bool {
    let page = state.message_view.page_size.max(1) as i32;
    scroll_by(state, direction * page)
}

fn scroll_by(state: &mut UiState, delta: i32) -> bool {
    if state.messages.is_empty() {
        return false;
    }
    let max_offset = state.messages.len() as i32 - 1;
    let current = state.message_view.scroll_offset as i32;
    let next = (current + delta).clamp(0, max_offset) as usize;
    state.message_view.scroll_offset = next;
    true
}

fn toggle_message_selection(state: &mut UiState) -> bool {
    let Some(message_id) = state.message_view.cursor_message_id(&state.messages) else {
        return false;
    };
    state.message_view.toggle_selection(message_id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{ChatListItem, MessageItem};

    fn sample_state() -> UiState {
        let mut state = UiState::default();
        state.messages = vec![
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
        ];
        state.message_view.reconcile(&state.messages);
        state
    }

    #[test]
    fn vim_jk_moves_message_cursor() {
        let mut state = sample_state();
        state.focus = UiFocus::Messages;
        state.message_view.cursor = Some(0);

        handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
            KeymapStyle::Vim,
        );
        assert_eq!(state.message_view.cursor, Some(1));

        handle_ui_key(
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

        handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert_eq!(state.message_view.cursor, Some(1));

        handle_ui_key(
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

        handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );
        assert!(state.message_view.selected_ids.contains(&1));

        handle_ui_key(
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

        handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeymapStyle::Vim,
        );
        assert!(state.message_view.search.is_open);
        assert_eq!(state.focus, UiFocus::Search);

        handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            KeymapStyle::Vim,
        );
        assert_eq!(state.message_view.search.query.text, "h");
        assert_eq!(state.message_view.search.matches, vec![0]);
    }

    #[test]
    fn chat_selection_moves_with_keys() {
        let mut state = UiState::default();
        state.focus = UiFocus::Chats;
        state.chats = vec![
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
        ];

        handle_ui_key(
            &mut state,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeymapStyle::Vscode,
        );

        assert!(state.chats[1].is_selected);
        assert!(!state.chats[0].is_selected);
    }
}
