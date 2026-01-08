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
}
