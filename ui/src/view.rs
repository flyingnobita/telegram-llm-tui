use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::input::InputState;

#[derive(Debug, Clone)]
pub struct ChatListItem {
    pub title: String,
    pub unread: u32,
    pub is_selected: bool,
}

#[derive(Debug, Clone)]
pub struct MessageItem {
    pub author: String,
    pub timestamp: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct DraftModalState {
    pub is_open: bool,
    pub title: String,
    pub body: String,
}

impl Default for DraftModalState {
    fn default() -> Self {
        Self {
            is_open: false,
            title: "LLM Draft".to_string(),
            body: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommandPaletteState {
    pub is_open: bool,
    pub query: String,
    pub items: Vec<String>,
    pub selected: usize,
}

#[derive(Debug, Clone, Default)]
pub struct UiState {
    pub input: InputState,
    pub chats: Vec<ChatListItem>,
    pub messages: Vec<MessageItem>,
    pub draft_modal: DraftModalState,
    pub command_palette: CommandPaletteState,
}

pub fn draw(frame: &mut Frame, state: &UiState) {
    let area = frame.size();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Min(1)])
        .split(rows[0]);

    let chat_items: Vec<ListItem> = if state.chats.is_empty() {
        vec![ListItem::new("No chats")]
    } else {
        state
            .chats
            .iter()
            .map(|chat| {
                let unread = if chat.unread > 0 {
                    format!(" ({})", chat.unread)
                } else {
                    String::new()
                };
                ListItem::new(format!("{}{}", chat.title, unread))
            })
            .collect()
    };

    let mut chat_state = ListState::default();
    let selected_chat = state.chats.iter().position(|chat| chat.is_selected);
    chat_state.select(selected_chat);

    let chat_list = List::new(chat_items)
        .block(Block::default().title("Chats").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let message_text = if state.messages.is_empty() {
        "No messages".to_string()
    } else {
        state
            .messages
            .iter()
            .map(|message| {
                let prefix = if message.timestamp.is_empty() {
                    String::new()
                } else {
                    format!("[{}] ", message.timestamp)
                };
                format!("{}{}: {}", prefix, message.author, message.body)
            })
            .collect::<Vec<String>>()
            .join("\n")
    };

    let message_view = Paragraph::new(message_text)
        .wrap(Wrap { trim: true })
        .block(Block::default().title("Messages").borders(Borders::ALL));

    let composer = Paragraph::new(state.input.text.as_str())
        .block(Block::default().title("Composer").borders(Borders::ALL));

    frame.render_stateful_widget(chat_list, columns[0], &mut chat_state);
    frame.render_widget(message_view, columns[1]);
    frame.render_widget(composer, rows[1]);

    if state.draft_modal.is_open {
        draw_draft_modal(frame, state, area);
    }

    if state.command_palette.is_open {
        draw_command_palette(frame, state, area);
    }
}

fn draw_draft_modal(frame: &mut Frame, state: &UiState, area: Rect) {
    let modal_area = centered_rect(area, 70, 60);
    frame.render_widget(Clear, modal_area);

    let draft = Paragraph::new(state.draft_modal.body.as_str())
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(state.draft_modal.title.as_str())
                .borders(Borders::ALL),
        );

    frame.render_widget(draft, modal_area);
}

fn draw_command_palette(frame: &mut Frame, state: &UiState, area: Rect) {
    let palette_area = centered_rect(area, 60, 35);
    frame.render_widget(Clear, palette_area);

    let palette_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(palette_area);

    let query = if state.command_palette.query.is_empty() {
        ">".to_string()
    } else {
        format!("> {}", state.command_palette.query)
    };

    let input =
        Paragraph::new(query).block(Block::default().title("Command").borders(Borders::ALL));
    frame.render_widget(input, palette_chunks[0]);

    let action_items: Vec<ListItem> = if state.command_palette.items.is_empty() {
        vec![ListItem::new("No matches")]
    } else {
        state
            .command_palette
            .items
            .iter()
            .map(|item| ListItem::new(item.as_str()))
            .collect()
    };

    let mut palette_state = ListState::default();
    if !state.command_palette.items.is_empty() {
        let selected = state
            .command_palette
            .selected
            .min(state.command_palette.items.len().saturating_sub(1));
        palette_state.select(Some(selected));
    }

    let actions = List::new(action_items)
        .block(Block::default().title("Actions").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(actions, palette_chunks[1], &mut palette_state);
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical_margin = 100u16.saturating_sub(percent_y);
    let horizontal_margin = 100u16.saturating_sub(percent_x);
    let top = vertical_margin / 2;
    let bottom = vertical_margin.saturating_sub(top);
    let left = horizontal_margin / 2;
    let right = horizontal_margin.saturating_sub(left);

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(top),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(bottom),
        ])
        .split(area);

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(right),
        ])
        .split(vertical_chunks[1]);

    horizontal_chunks[1]
}
