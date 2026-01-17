use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputState {
    pub text: String,
    pub cursor: usize,
}

impl InputState {
    pub fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.text.len());
        if !self.text.is_char_boundary(self.cursor) {
            self.cursor = prev_char_boundary(&self.text, self.cursor);
        }
    }
}

pub fn handle_key(state: &mut InputState, key: KeyEvent) -> bool {
    state.clamp_cursor();
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                return false;
            }
            state.text.insert(state.cursor, c);
            state.cursor += 1;
            true
        }
        KeyCode::Backspace => {
            if state.cursor > 0 {
                let prev = prev_char_boundary(&state.text, state.cursor);
                state.text.replace_range(prev..state.cursor, "");
                state.cursor = prev;
            }
            true
        }
        KeyCode::Left => {
            if state.cursor > 0 {
                state.cursor = prev_char_boundary(&state.text, state.cursor);
            }
            true
        }
        KeyCode::Right => {
            if state.cursor < state.text.len() {
                state.cursor = next_char_boundary(&state.text, state.cursor);
            }
            true
        }
        KeyCode::Home => {
            state.cursor = 0;
            true
        }
        KeyCode::End => {
            state.cursor = state.text.len();
            true
        }
        _ => false,
    }
}

fn prev_char_boundary(text: &str, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    let mut prev = 0;
    for (i, _) in text.char_indices() {
        if i >= idx {
            break;
        }
        prev = i;
    }
    prev
}

fn next_char_boundary(text: &str, idx: usize) -> usize {
    if idx >= text.len() {
        return text.len();
    }
    for (i, _) in text.char_indices() {
        if i > idx {
            return i;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_and_moves_cursor() {
        let mut state = InputState::default();

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
        );
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
        );

        assert_eq!(state.text, "hi");
        assert_eq!(state.cursor, 2);

        handle_key(&mut state, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
        );

        assert_eq!(state.text, "h!i");
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn backspace_and_ctrl_are_handled() {
        let mut state = InputState {
            text: "ok".to_string(),
            cursor: 2,
        };

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        );

        assert_eq!(state.text, "o");
        assert_eq!(state.cursor, 1);

        let handled = handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );

        assert!(!handled);
        assert_eq!(state.text, "o");
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn unicode_cursor_moves_by_char_boundary() {
        let mut state = InputState {
            text: "aé💡".to_string(),
            cursor: 0,
        };

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert_eq!(state.cursor, 1);

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert_eq!(state.cursor, 3);

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        );
        assert_eq!(state.cursor, state.text.len());

        handle_key(&mut state, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(state.cursor, 3);

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        );
        assert_eq!(state.text, "a💡");
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn clamps_mid_char_cursor_before_insert() {
        let mut state = InputState {
            text: "aé".to_string(),
            cursor: 2,
        };

        handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        );

        assert_eq!(state.text, "abé");
        assert_eq!(state.cursor, 2);
    }
}
