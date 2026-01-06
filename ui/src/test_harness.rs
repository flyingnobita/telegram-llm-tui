use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};

use crate::view::{draw, UiState};

pub fn render_to_buffer(state: &UiState, size: (u16, u16)) -> Buffer {
    let (width, height) = size;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("create test terminal");

    terminal
        .draw(|frame| draw(frame, state))
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
    use insta::assert_snapshot;

    #[test]
    fn renders_basic_layout() {
        let mut state = UiState::default();
        state.input.text = "hello world".to_string();

        let rendered = render_to_string(&state, (40, 10));

        assert_snapshot!(rendered);
    }
}
