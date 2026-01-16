use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

use crate::view::UiState;

pub fn render_to_buffer(state: &UiState, size: (u16, u16)) -> Buffer {
    let (width, height) = size;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");

    terminal
        .draw(|frame| crate::view::draw(frame, state))
        .expect("render test frame");

    terminal.backend().buffer().clone()
}

pub fn buffer_to_string(buffer: &Buffer) -> String {
    let mut output = String::new();
    let width = buffer.area.width;
    let height = buffer.area.height;

    for y in 0..height {
        let mut line = String::new();
        for x in 0..width {
            let cell = buffer.get(x, y);
            line.push_str(cell.symbol());
        }
        output.push_str(line.trim_end());
        if y + 1 < height {
            output.push('\n');
        }
    }

    output
}

pub fn render_to_string(state: &UiState, size: (u16, u16)) -> String {
    let buffer = render_to_buffer(state, size);
    buffer_to_string(&buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{ChatListItem, CommandPaletteState, DraftModalState, MessageItem};
    use insta::assert_snapshot;

    fn sample_state() -> UiState {
        let mut state = UiState::default();
        state.input.text = "drafting a reply".to_string();
        state.chats = vec![
            ChatListItem {
                id: 1,
                title: "General".to_string(),
                unread: 0,
                is_selected: true,
            },
            ChatListItem {
                id: 2,
                title: "Product".to_string(),
                unread: 3,
                is_selected: false,
            },
            ChatListItem {
                id: 3,
                title: "Design".to_string(),
                unread: 1,
                is_selected: false,
            },
        ];
        state.messages = vec![
            MessageItem {
                id: 100,
                author: "Ada".to_string(),
                timestamp: "09:12".to_string(),
                body: "Morning team".to_string(),
            },
            MessageItem {
                id: 101,
                author: "You".to_string(),
                timestamp: "09:13".to_string(),
                body: "Morning, syncing on layout".to_string(),
            },
            MessageItem {
                id: 102,
                author: "Ada".to_string(),
                timestamp: "09:15".to_string(),
                body: "Need the LLM draft soon".to_string(),
            },
        ];
        state.message_view.cursor = Some(1);
        state.message_view.selected_ids.insert(101);
        state.message_view.search.query.text = "draft".to_string();
        state.message_view.search.recompute_matches(&state.messages);
        state
    }

    fn state_with_single_message(body: &str) -> UiState {
        let mut state = UiState {
            chats: vec![ChatListItem {
                id: 1,
                title: "General".to_string(),
                unread: 0,
                is_selected: true,
            }],
            messages: vec![MessageItem {
                id: 1,
                author: "Ada".to_string(),
                timestamp: "09:12".to_string(),
                body: body.to_string(),
            }],
            ..Default::default()
        };
        state.message_view.cursor = Some(0);
        state
    }

    fn sample_logs() -> Vec<String> {
        vec![
            "2026-01-16T09:12:01+00:00 INFO app: booted".to_string(),
            "2026-01-16T09:12:02+00:00 WARN core: slow network".to_string(),
            "2026-01-16T09:12:03+00:00 INFO ui: render tick".to_string(),
            "2026-01-16T09:12:04+00:00 ERROR app: failed to sync".to_string(),
            "2026-01-16T09:12:05+00:00 INFO ui: redraw".to_string(),
        ]
    }

    #[test]
    fn renders_layout_v1() {
        let state = sample_state();
        let rendered = render_to_string(&state, (80, 20));

        assert_snapshot!(rendered);
    }

    #[test]
    fn renders_command_palette() {
        let mut state = sample_state();
        state.command_palette = CommandPaletteState {
            is_open: true,
            query: "open".to_string(),
            items: vec![
                "Open chat".to_string(),
                "Open settings".to_string(),
                "Open logs".to_string(),
            ],
            selected: 1,
        };

        let rendered = render_to_string(&state, (80, 20));

        assert_snapshot!(rendered);
    }

    #[test]
    fn renders_draft_modal() {
        let mut state = sample_state();
        state.draft_modal = DraftModalState {
            is_open: true,
            title: "LLM Draft".to_string(),
            body: "Here is a draft response that needs review.".to_string(),
        };

        let rendered = render_to_string(&state, (80, 20));

        assert_snapshot!(rendered);
    }

    #[test]
    fn renders_log_window() {
        let mut state = sample_state();
        state.logs = sample_logs();
        state.log_view.is_open = true;

        let rendered = render_to_string(&state, (80, 20));

        assert_snapshot!(rendered);
    }

    #[test]
    fn clears_message_area_between_draws() {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        let first_state = state_with_single_message("OLD-LINE-SHOULD-CLEAR");
        let second_state = state_with_single_message("New message");

        terminal
            .draw(|frame| crate::view::draw(frame, &first_state))
            .expect("render first frame");
        terminal
            .draw(|frame| crate::view::draw(frame, &second_state))
            .expect("render second frame");

        let rendered = buffer_to_string(terminal.backend().buffer());
        assert!(
            !rendered.contains("OLD-LINE-SHOULD-CLEAR"),
            "expected cleared message area, got:\n{rendered}"
        );
    }
}
