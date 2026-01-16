use std::collections::BTreeSet;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
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
    pub scroll_horizontal: usize,
    pub cursor: Option<usize>,
    pub selected_ids: BTreeSet<i64>,
    pub search: MessageSearchState,
    pub page_size: usize,
}

impl Default for MessageViewState {
    fn default() -> Self {
        Self {
            scroll_offset: 0,
            scroll_horizontal: 0,
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
            self.scroll_horizontal = 0;
        } else {
            let max_index = messages.len().saturating_sub(1);
            self.cursor = Some(self.cursor.unwrap_or(max_index).min(max_index));
            let max_scroll = message_max_scroll_for(messages, self.page_size);
            self.scroll_offset = self.scroll_offset.min(max_scroll);
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
pub struct LogViewState {
    pub is_open: bool,
    pub scroll_offset: usize,
    pub page_size: usize,
}

impl Default for LogViewState {
    fn default() -> Self {
        Self {
            is_open: false,
            scroll_offset: 0,
            page_size: 8,
        }
    }
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
    pub logs: Vec<String>,
    pub log_view: LogViewState,
    pub chat_list_width: u16,
    pub chat_list_scroll: usize,
    pub chat_list_viewport_width: u16,
    pub message_viewport_width: u16,
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
            logs: Vec::new(),
            log_view: LogViewState::default(),
            chat_list_width: DEFAULT_CHAT_LIST_WIDTH,
            chat_list_scroll: 0,
            chat_list_viewport_width: 0,
            message_viewport_width: 0,
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
    inner.height.saturating_sub(1).max(1) as usize
}

pub fn message_viewport_width(area: Rect, chat_list_width: u16) -> u16 {
    let message_area = message_viewport_area(area, chat_list_width);
    let inner = Block::default().borders(Borders::ALL).inner(message_area);
    inner.width.saturating_sub(1).max(1)
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

pub fn log_view_max_scroll(state: &UiState) -> usize {
    let page_size = state.log_view.page_size.max(1);
    let line_count = state.logs.len().max(1);
    line_count.saturating_sub(page_size)
}

pub fn message_max_scroll(state: &UiState) -> usize {
    message_max_scroll_for(&state.messages, state.message_view.page_size)
}

pub fn message_max_horizontal_scroll(state: &UiState) -> usize {
    let viewport_width = state.message_viewport_width.max(1) as usize;
    let max_line_width = message_max_line_width(state);
    max_line_width.saturating_sub(viewport_width)
}

pub(crate) fn message_max_scroll_for(messages: &[MessageItem], page_size: usize) -> usize {
    let total_lines = message_total_lines(messages);
    let page = page_size.max(1);
    total_lines.saturating_sub(page)
}

pub(crate) fn message_line_offset(messages: &[MessageItem], index: usize) -> usize {
    messages.iter().take(index).map(message_line_count).sum()
}

fn message_total_lines(messages: &[MessageItem]) -> usize {
    messages.iter().map(message_line_count).sum()
}

fn message_line_count(message: &MessageItem) -> usize {
    let lines = message.body.lines().count();
    lines.max(1)
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
    let overlay_active =
        state.draft_modal.is_open || state.command_palette.is_open || state.log_view.is_open;
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

    let (message_text, scroll_offset, scroll_horizontal) = build_message_text(state);
    let message_title = message_view_title(state);

    let message_block = Block::default()
        .title(message_title)
        .borders(Borders::ALL)
        .border_style(focus_border_style(message_focused));
    let message_inner = message_block.inner(layout.messages);

    let scrollbar_width = if message_inner.width > 1 { 1 } else { 0 };
    let scrollbar_height = if message_inner.height > 1 { 1 } else { 0 };
    let text_area = Rect {
        x: message_inner.x,
        y: message_inner.y,
        width: message_inner.width.saturating_sub(scrollbar_width).max(1),
        height: message_inner.height.saturating_sub(scrollbar_height).max(1),
    };

    let message_view = Paragraph::new(message_text).scroll((scroll_offset, scroll_horizontal));

    let composer = Paragraph::new(state.input.text.as_str()).block(
        Block::default()
            .title("Composer")
            .borders(Borders::ALL)
            .border_style(focus_border_style(composer_focused)),
    );

    frame.render_stateful_widget(chat_list, layout.chat_list, &mut chat_state);
    frame.render_widget(message_block, layout.messages);
    frame.render_widget(message_view, text_area);

    let mut vertical_scroll_state = ScrollbarState::new(message_total_lines(&state.messages))
        .position(state.message_view.scroll_offset);
    if scrollbar_width > 0 {
        let vertical_area = Rect {
            x: text_area.x + text_area.width,
            y: text_area.y,
            width: scrollbar_width,
            height: text_area.height,
        };
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            vertical_area,
            &mut vertical_scroll_state,
        );
    }

    let mut horizontal_scroll_state = ScrollbarState::new(message_max_line_width(state))
        .position(state.message_view.scroll_horizontal);
    if scrollbar_height > 0 {
        let horizontal_area = Rect {
            x: text_area.x,
            y: text_area.y + text_area.height,
            width: text_area.width,
            height: scrollbar_height,
        };
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::HorizontalBottom),
            horizontal_area,
            &mut horizontal_scroll_state,
        );
    }
    frame.render_widget(composer, layout.composer);

    if state.draft_modal.is_open {
        draw_draft_modal(frame, state, area);
    }

    if state.command_palette.is_open {
        draw_command_palette(frame, state, area);
    }

    if state.log_view.is_open {
        draw_log_window(frame, state, area);
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

fn build_message_text(state: &UiState) -> (String, u16, u16) {
    if state.messages.is_empty() {
        return ("No messages".to_string(), 0, 0);
    }

    let lines = build_message_lines(state);

    let max_scroll = message_max_scroll(state);
    let scroll_offset = state
        .message_view
        .scroll_offset
        .min(max_scroll)
        .min(u16::MAX as usize) as u16;

    let max_horizontal = message_max_horizontal_scroll(state);
    let scroll_horizontal = state
        .message_view
        .scroll_horizontal
        .min(max_horizontal)
        .min(u16::MAX as usize) as u16;

    (lines.join("\n"), scroll_offset, scroll_horizontal)
}

fn build_message_lines(state: &UiState) -> Vec<String> {
    let search_matches = &state.message_view.search.matches;
    let mut lines = Vec::new();
    for (idx, message) in state.messages.iter().enumerate() {
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
        let header = format!(
            "{} [{}{}] {}{}: ",
            cursor_marker, selected_marker, match_marker, timestamp, message.author
        );
        let mut body_lines = message.body.lines();
        if let Some(first_line) = body_lines.next() {
            lines.push(format!("{}{}", header, first_line));
            let indent = " ".repeat(header.chars().count());
            for line in body_lines {
                lines.push(format!("{}{}", indent, line));
            }
        } else {
            lines.push(header);
        }
    }
    lines
}

fn message_max_line_width(state: &UiState) -> usize {
    build_message_lines(state)
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

fn build_log_text(state: &UiState) -> (String, u16) {
    if state.logs.is_empty() {
        return ("No logs".to_string(), 0);
    }

    let max_scroll = log_view_max_scroll(state);
    let scroll_offset = state
        .log_view
        .scroll_offset
        .min(max_scroll)
        .min(u16::MAX as usize) as u16;

    (state.logs.join("\n"), scroll_offset)
}

fn draw_log_window(frame: &mut Frame, state: &UiState, area: Rect) {
    let log_area = log_window_area(area);
    frame.render_widget(Clear, log_area);

    let (log_text, scroll_offset) = build_log_text(state);
    let logs = Paragraph::new(log_text).scroll((scroll_offset, 0)).block(
        Block::default()
            .title("Logs")
            .borders(Borders::ALL)
            .border_style(focus_border_style(true)),
    );

    frame.render_widget(logs, log_area);
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

pub fn log_window_page_size(area: Rect) -> usize {
    let log_area = log_window_area(area);
    let inner = Block::default().borders(Borders::ALL).inner(log_area);
    inner.height.max(1) as usize
}

fn log_window_area(area: Rect) -> Rect {
    centered_rect(area, 90, 90)
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
        assert_eq!(message_viewport_page_size(area, DEFAULT_CHAT_LIST_WIDTH), 4);
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

    #[test]
    fn message_max_scroll_accounts_for_multiline_messages() {
        let mut state = UiState::default();
        state.message_view.page_size = 2;
        state.messages = vec![
            MessageItem {
                id: 1,
                author: "Ada".to_string(),
                timestamp: "09:12".to_string(),
                body: "first\nsecond\nthird".to_string(),
            },
            MessageItem {
                id: 2,
                author: "You".to_string(),
                timestamp: "09:13".to_string(),
                body: "last".to_string(),
            },
        ];

        assert_eq!(message_line_offset(&state.messages, 1), 3);
        assert_eq!(message_max_scroll(&state), 2);
    }
}
