use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::input::InputState;

#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub input: InputState,
}

pub fn draw(frame: &mut Frame, state: &UiState) {
    let area = frame.size();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new("Telegram LLM TUI")
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM));

    let body =
        Paragraph::new("Messages").block(Block::default().title("Chat").borders(Borders::ALL));

    let composer = Paragraph::new(state.input.text.as_str())
        .block(Block::default().title("Composer").borders(Borders::ALL));

    frame.render_widget(header, rows[0]);
    frame.render_widget(body, rows[1]);
    frame.render_widget(composer, rows[2]);
}
