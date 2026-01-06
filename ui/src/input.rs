use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputState {
    pub text: String,
    pub cursor: usize,
}

impl InputState {
    pub fn clamp_cursor(&mut self) {
        self.cursor = self.cursor.min(self.text.len());
    }
}

pub fn handle_key(state: &mut InputState, key: KeyEvent) -> bool {
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
                state.cursor -= 1;
                state.text.remove(state.cursor);
            }
            true
        }
        KeyCode::Left => {
            if state.cursor > 0 {
                state.cursor -= 1;
            }
            true
        }
        KeyCode::Right => {
            if state.cursor < state.text.len() {
                state.cursor += 1;
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
}
