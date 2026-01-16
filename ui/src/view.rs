use std::collections::BTreeSet;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::input::InputState;

const DEFAULT_CHAT_LIST_WIDTH: u16 = 32;

#[derive(Debug, Clone)]
pub struct ChatListItem {
    pub id: i64,
    pub title: String,
    pub unread: u32,
    pub is_selected: bool,
}

impl ChatListItem {
    pub fn label(&self) -> String {
        let unread = if self.unread > 0 {
            format!(" ({})", self.unread)
        } else {
            String::new()
        };
        format!("{}{}", self.title, unread)
    }
}

#[derive(Debug, Clone)]
pub struct MessageItem {
    pub id: i64,
    pub author: String,
    pub timestamp: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiFocus {
    Chats,
    #[default]
    Messages,
    Composer,
    Search,
}

#[derive(Debug, Clone, Default)]
pub struct MessageSearchState {
    pub is_open: bool,
    pub query: InputState,
    pub matches: Vec<usize>,
    pub selected: usize,
}

impl MessageSearchState {
    pub fn recompute_matches(&mut self, messages: &[MessageItem]) {
        let query = self.query.text.trim();
        if query.is_empty() {
            self.matches.clear();
            self.selected = 0;
            return;
        }
        let needle = query.to_lowercase();
        self.matches = messages
            .iter()
            .enumerate()
            .filter_map(|(idx, message)| {
                let haystack = format!("{} {}", message.author, message.body).to_lowercase();
                if haystack.contains(&needle) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        if self.matches.is_empty() || self.selected >= self.matches.len() {
            self.selected = 0;
        }
    }

    pub fn selected_match(&self) -> Option<usize> {
        self.matches.get(self.selected).copied()
    }

    pub fn advance(&mut self, forward: bool) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        if forward {
            self.selected = (self.selected + 1) % self.matches.len();
        } else if self.selected == 0 {
            self.selected = self.matches.len() - 1;
        } else {
            self.selected -= 1;
        }
        self.matches.get(self.selected).copied()
    }
}

#[derive(Debug, Clone)]
pub struct MessageViewState {
    pub scroll_offset: usize,
    pub cursor: Option<usize>,
    pub selected_ids: BTreeSet<i64>,
    pub search: MessageSearchState,
    pub page_size: usize,
}

impl Default for MessageViewState {
    fn default() -> Self {
        Self {
            scroll_offset: 0,
            cursor: None,
            selected_ids: BTreeSet::new(),
            search: MessageSearchState::default(),
            page_size: 8,
        }
    }
}

impl MessageViewState {
    pub fn reconcile(&mut self, messages: &[MessageItem]) {
        let existing_ids: BTreeSet<i64> = messages.iter().map(|message| message.id).collect();
        self.selected_ids.retain(|id| existing_ids.contains(id));

        if messages.is_empty() {
            self.cursor = None;
            self.scroll_offset = 0;
        } else {
            let max_index = messages.len().saturating_sub(1);
            self.cursor = Some(self.cursor.unwrap_or(max_index).min(max_index));
            self.scroll_offset = self.scroll_offset.min(max_index);
        }

        self.search.recompute_matches(messages);
    }

    pub fn toggle_selection(&mut self, message_id: i64) {
        if !self.selected_ids.insert(message_id) {
            self.selected_ids.remove(&message_id);
        }
    }

    pub fn cursor_message_id(&self, messages: &[MessageItem]) -> Option<i64> {
        self.cursor
            .and_then(|index| messages.get(index).map(|message| message.id))
    }
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

#[derive(Debug, Clone)]
pub struct UiState {
    pub focus: UiFocus,
    pub input: InputState,
    pub chats: Vec<ChatListItem>,
    pub messages: Vec<MessageItem>,
    pub message_view: MessageViewState,
    pub draft_modal: DraftModalState,
    pub command_palette: CommandPaletteState,
    pub chat_list_width: u16,
    pub chat_list_scroll: usize,
    pub chat_list_viewport_width: u16,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            focus: UiFocus::default(),
            input: InputState::default(),
            chats: Vec::new(),
            messages: Vec::new(),
            message_view: MessageViewState::default(),
            draft_modal: DraftModalState::default(),
            command_palette: CommandPaletteState::default(),
            chat_list_width: DEFAULT_CHAT_LIST_WIDTH,
            chat_list_scroll: 0,
            chat_list_viewport_width: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LayoutAreas {
    chat_list: Rect,
    messages: Rect,
    composer: Rect,
}

fn layout_areas(area: Rect, chat_list_width: u16) -> LayoutAreas {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    let chat_list_width = clamp_chat_list_width(area.width, chat_list_width);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(chat_list_width), Constraint::Min(1)])
        .split(rows[0]);

    LayoutAreas {
        chat_list: columns[0],
        messages: columns[1],
        composer: rows[1],
    }
}

pub fn message_viewport_area(area: Rect, chat_list_width: u16) -> Rect {
    layout_areas(area, chat_list_width).messages
}

pub fn message_viewport_page_size(area: Rect, chat_list_width: u16) -> usize {
    let message_area = message_viewport_area(area, chat_list_width);
    let inner = Block::default().borders(Borders::ALL).inner(message_area);
    inner.height.max(1) as usize
}

pub fn chat_list_viewport_width(area: Rect, chat_list_width: u16) -> u16 {
    let chat_area = layout_areas(area, chat_list_width).chat_list;
    Block::default()
        .borders(Borders::ALL)
        .inner(chat_area)
        .width
}

pub fn chat_list_max_scroll(state: &UiState) -> usize {
    let viewport_width = state.chat_list_viewport_width.max(1) as usize;
    let labels: Vec<String> = if state.chats.is_empty() {
        vec!["No chats".to_string()]
    } else {
        state.chats.iter().map(ChatListItem::label).collect()
    };
    let max_label_len = labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0);
    max_label_len.saturating_sub(viewport_width)
}

fn clamp_chat_list_width(area_width: u16, desired: u16) -> u16 {
    if area_width <= 1 {
        return area_width.max(1);
    }
    let max_width = area_width.saturating_sub(1).max(1);
    desired.max(1).min(max_width)
}

fn focus_border_style(is_focused: bool) -> Style {
    if is_focused {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    }
}

fn is_message_focus(focus: UiFocus) -> bool {
    matches!(focus, UiFocus::Messages | UiFocus::Search)
}

pub fn draw(frame: &mut Frame, state: &UiState) {
    let area = frame.size();
    let layout = layout_areas(area, state.chat_list_width);
    let overlay_active = state.draft_modal.is_open || state.command_palette.is_open;
    let chat_focused = !overlay_active && state.focus == UiFocus::Chats;
    let message_focused = !overlay_active && is_message_focus(state.focus);
    let composer_focused = !overlay_active && state.focus == UiFocus::Composer;

    frame.render_widget(Clear, layout.messages);

    let chat_labels: Vec<String> = if state.chats.is_empty() {
        vec!["No chats".to_string()]
    } else {
        state.chats.iter().map(ChatListItem::label).collect()
    };
    let chat_inner_width = Block::default()
        .borders(Borders::ALL)
        .inner(layout.chat_list)
        .width
        .max(1) as usize;
    let max_label_len = chat_labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0);
    let max_scroll = max_label_len.saturating_sub(chat_inner_width);
    let scroll_offset = state.chat_list_scroll.min(max_scroll);
    let chat_items: Vec<ListItem> = chat_labels
        .into_iter()
        .map(|label| ListItem::new(apply_horizontal_scroll(&label, scroll_offset)))
        .collect();

    let mut chat_state = ListState::default();
    let selected_chat = state.chats.iter().position(|chat| chat.is_selected);
    chat_state.select(selected_chat);

    let chat_block = Block::default()
        .title("Chats")
        .borders(Borders::ALL)
        .border_style(focus_border_style(chat_focused));
    let chat_list = List::new(chat_items)
        .block(chat_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let (message_text, scroll_offset) = build_message_text(state);
    let message_title = message_view_title(state);

    let message_view = Paragraph::new(message_text)
        .wrap(Wrap { trim: true })
        .scroll((scroll_offset, 0))
        .block(
            Block::default()
                .title(message_title)
                .borders(Borders::ALL)
                .border_style(focus_border_style(message_focused)),
        );

    let composer = Paragraph::new(state.input.text.as_str()).block(
        Block::default()
            .title("Composer")
            .borders(Borders::ALL)
            .border_style(focus_border_style(composer_focused)),
    );

    frame.render_stateful_widget(chat_list, layout.chat_list, &mut chat_state);
    frame.render_widget(message_view, layout.messages);
    frame.render_widget(composer, layout.composer);

    if state.draft_modal.is_open {
        draw_draft_modal(frame, state, area);
    }

    if state.command_palette.is_open {
        draw_command_palette(frame, state, area);
    }
}

fn apply_horizontal_scroll(text: &str, offset: usize) -> String {
    if offset == 0 {
        return text.to_string();
    }
    text.chars().skip(offset).collect()
}

fn message_view_title(state: &UiState) -> String {
    if state.message_view.search.is_open || !state.message_view.search.query.text.is_empty() {
        if state.message_view.search.query.text.is_empty() {
            "Messages (search)".to_string()
        } else {
            format!(
                "Messages (search: {})",
                state.message_view.search.query.text
            )
        }
    } else {
        "Messages".to_string()
    }
}

fn build_message_text(state: &UiState) -> (String, u16) {
    if state.messages.is_empty() {
        return ("No messages".to_string(), 0);
    }

    let search_matches = &state.message_view.search.matches;
    let lines: Vec<String> = state
        .messages
        .iter()
        .enumerate()
        .map(|(idx, message)| {
            let cursor_marker = if state.message_view.cursor == Some(idx) {
                ">"
            } else {
                " "
            };
            let selected_marker = if state.message_view.selected_ids.contains(&message.id) {
                "x"
            } else {
                " "
            };
            let match_marker = if search_matches.contains(&idx) {
                "*"
            } else {
                " "
            };
            let timestamp = if message.timestamp.is_empty() {
                String::new()
            } else {
                format!("[{}] ", message.timestamp)
            };
            format!(
                "{} [{}{}] {}{}: {}",
                cursor_marker,
                selected_marker,
                match_marker,
                timestamp,
                message.author,
                message.body
            )
        })
        .collect();

    let scroll_offset = state
        .message_view
        .scroll_offset
        .min(lines.len().saturating_sub(1))
        .min(u16::MAX as usize) as u16;

    (lines.join("\n"), scroll_offset)
}

fn draw_draft_modal(frame: &mut Frame, state: &UiState, area: Rect) {
    let modal_area = centered_rect(area, 70, 60);
    frame.render_widget(Clear, modal_area);

    let draft = Paragraph::new(state.draft_modal.body.as_str())
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(state.draft_modal.title.as_str())
                .borders(Borders::ALL)
                .border_style(focus_border_style(true)),
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

    let input = Paragraph::new(query).block(
        Block::default()
            .title("Command")
            .borders(Borders::ALL)
            .border_style(focus_border_style(true)),
    );
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
        .block(
            Block::default()
                .title("Actions")
                .borders(Borders::ALL)
                .border_style(focus_border_style(true)),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_viewport_page_size_clamps_to_minimum() {
        let area = Rect::new(0, 0, 10, 1);
        assert_eq!(message_viewport_page_size(area, DEFAULT_CHAT_LIST_WIDTH), 1);
    }

    #[test]
    fn message_viewport_page_size_reserves_composer_and_border() {
        let area = Rect::new(0, 0, 10, 10);
        assert_eq!(message_viewport_page_size(area, DEFAULT_CHAT_LIST_WIDTH), 5);
    }

    #[test]
    fn chat_list_max_scroll_respects_viewport() {
        let state = UiState {
            chat_list_viewport_width: 8,
            chats: vec![ChatListItem {
                id: 1,
                title: "Long Chat Title".to_string(),
                unread: 0,
                is_selected: true,
            }],
            ..Default::default()
        };

        assert_eq!(chat_list_max_scroll(&state), 7);
    }
}
