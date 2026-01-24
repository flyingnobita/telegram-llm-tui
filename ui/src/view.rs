use std::collections::BTreeSet;

use ratatui::text::{Line, Span};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use textwrap::Options;

use crate::input::InputState;
use crate::pane::{
    pane_layout, render_pane, render_scrollbars, PaneConfig, PaneMetrics, PaneState, PaneViewport,
};

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
    ChatSearch,
    #[default]
    Messages,
    Composer,
    Search,
}

#[derive(Debug, Clone, Default)]
pub struct ChatSearchState {
    pub is_open: bool,
    pub query: InputState,
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
    pub pane: PaneState,
    pub cursor: Option<usize>,
    pub selected_ids: BTreeSet<i64>,
    pub search: MessageSearchState,
}

impl Default for MessageViewState {
    fn default() -> Self {
        let pane = PaneState {
            page_size: 8,
            ..PaneState::default()
        };
        Self {
            pane,
            cursor: None,
            selected_ids: BTreeSet::new(),
            search: MessageSearchState::default(),
        }
    }
}

impl MessageViewState {
    pub fn reconcile(&mut self, messages: &[MessageItem]) {
        let existing_ids: BTreeSet<i64> = messages.iter().map(|message| message.id).collect();
        self.selected_ids.retain(|id| existing_ids.contains(id));

        if messages.is_empty() {
            self.cursor = None;
            self.pane.scroll_vertical = 0;
            self.pane.scroll_horizontal = 0;
        } else {
            let max_index = messages.len().saturating_sub(1);
            self.cursor = Some(self.cursor.unwrap_or(max_index).min(max_index));
            let metrics = PaneMetrics {
                line_count: message_total_lines(messages),
                max_line_width: message_max_line_width_for(messages),
            };
            self.pane.clamp_offsets(metrics, PaneConfig::message_pane());
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
    pub pane: PaneState,
    pub selection: Option<(usize, usize)>, // (anchor, cursor)
    pub sticky_scroll: bool,
}

impl Default for LogViewState {
    fn default() -> Self {
        let pane = PaneState {
            page_size: 8,
            ..PaneState::default()
        };
        Self {
            is_open: false,
            pane,
            selection: None,
            sticky_scroll: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UiState {
    pub focus: UiFocus,
    pub input: InputState,
    pub chats: Vec<ChatListItem>,
    pub chat_search: ChatSearchState,
    pub messages: Vec<MessageItem>,
    pub message_view: MessageViewState,
    pub draft_modal: DraftModalState,
    pub command_palette: CommandPaletteState,
    pub logs: Vec<String>,
    pub log_view: LogViewState,
    pub chat_list_pane: PaneState,
    pub composer_pane: PaneState,
    pub chat_list_width: u16,
    pub composer_cursor_visible: bool,
    pub status_message: Option<String>,
    pub selected_prompt_kit: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            focus: UiFocus::default(),
            input: InputState::default(),
            chats: Vec::new(),
            chat_search: ChatSearchState::default(),
            messages: Vec::new(),
            message_view: MessageViewState::default(),
            draft_modal: DraftModalState::default(),
            command_palette: CommandPaletteState::default(),
            logs: Vec::new(),
            log_view: LogViewState::default(),
            chat_list_pane: PaneState::default(),
            composer_pane: PaneState::default(),
            chat_list_width: DEFAULT_CHAT_LIST_WIDTH,
            composer_cursor_visible: true,
            status_message: None,
            selected_prompt_kit: "reply".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LayoutAreas {
    chat_list: Rect,
    messages: Rect,
    composer: Rect,
    key_hints: Rect,
}

pub fn update_layout(state: &mut UiState, area: Rect) {
    let layout = layout_areas(area, state.chat_list_width);

    // Update Chat List Pane
    let chat_block = Block::default().borders(Borders::ALL);
    let chat_layout = pane_layout(layout.chat_list, &chat_block, PaneConfig::chat_list_pane());
    state.chat_list_pane.set_viewport(
        PaneViewport::from_rect(chat_layout.text_area),
        chat_layout.text_area.height as usize,
    );

    // Update Message Pane
    let msg_block = Block::default().borders(Borders::ALL);
    let msg_layout = pane_layout(layout.messages, &msg_block, PaneConfig::message_pane());
    state.message_view.pane.set_viewport(
        PaneViewport::from_rect(msg_layout.text_area),
        msg_layout.text_area.height as usize,
    );

    // Update Composer Pane
    let comp_block = Block::default().borders(Borders::ALL);
    let comp_layout = pane_layout(layout.composer, &comp_block, PaneConfig::composer_pane());
    state.composer_pane.set_viewport(
        PaneViewport::from_rect(comp_layout.text_area),
        comp_layout.text_area.height as usize,
    );

    // Update Log Window Pane
    if state.log_view.is_open {
        let log_area = log_window_area(area);
        let log_block = Block::default().borders(Borders::ALL);
        let log_layout = pane_layout(log_area, &log_block, PaneConfig::log_pane());
        state.log_view.pane.set_viewport(
            PaneViewport::from_rect(log_layout.text_area),
            log_layout.text_area.height as usize,
        );
    }
}

fn layout_areas(area: Rect, chat_list_width: u16) -> LayoutAreas {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
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
        key_hints: rows[2],
    }
}

pub fn message_viewport_area(area: Rect, chat_list_width: u16) -> Rect {
    layout_areas(area, chat_list_width).messages
}

pub fn composer_viewport_area(area: Rect, chat_list_width: u16) -> Rect {
    layout_areas(area, chat_list_width).composer
}

pub fn message_viewport_page_size(area: Rect, chat_list_width: u16) -> usize {
    let message_area = message_viewport_area(area, chat_list_width);
    let block = Block::default().borders(Borders::ALL);
    let layout = pane_layout(message_area, &block, PaneConfig::message_pane());
    layout.text_area.height.max(1) as usize
}

pub fn message_viewport_width(area: Rect, chat_list_width: u16) -> u16 {
    let message_area = message_viewport_area(area, chat_list_width);
    let block = Block::default().borders(Borders::ALL);
    let layout = pane_layout(message_area, &block, PaneConfig::message_pane());
    layout.text_area.width.max(1)
}

pub fn composer_text_area(area: Rect, chat_list_width: u16) -> Rect {
    let composer_area = composer_viewport_area(area, chat_list_width);
    let block = Block::default().borders(Borders::ALL);
    let layout = pane_layout(composer_area, &block, PaneConfig::composer_pane());
    layout.text_area
}

pub fn chat_list_text_area(area: Rect, chat_list_width: u16) -> Rect {
    let chat_area = layout_areas(area, chat_list_width).chat_list;
    let block = Block::default().borders(Borders::ALL);
    let layout = pane_layout(chat_area, &block, PaneConfig::chat_list_pane());
    layout.text_area
}

pub fn chat_list_metrics(state: &UiState) -> PaneMetrics {
    let labels: Vec<String> = if state.chats.is_empty() {
        vec!["No chats".to_string()]
    } else {
        state.chats.iter().map(ChatListItem::label).collect()
    };
    PaneMetrics::from_lines(&labels)
}

pub fn chat_list_max_vertical_scroll(state: &UiState) -> usize {
    state
        .chat_list_pane
        .max_vertical_scroll(chat_list_metrics(state), PaneConfig::chat_list_pane())
}

pub fn chat_list_max_horizontal_scroll(state: &UiState) -> usize {
    state
        .chat_list_pane
        .max_horizontal_scroll(chat_list_metrics(state), PaneConfig::chat_list_pane())
}

pub fn clamp_chat_list_scroll(state: &mut UiState) {
    let metrics = chat_list_metrics(state);
    state
        .chat_list_pane
        .clamp_offsets(metrics, PaneConfig::chat_list_pane());
}

pub fn ensure_chat_list_selection_visible(state: &mut UiState) {
    let Some(selected) = state.chats.iter().position(|chat| chat.is_selected) else {
        return;
    };
    let page_size = state.chat_list_pane.page_size.max(1);
    let scroll = state.chat_list_pane.scroll_vertical;
    if selected < scroll {
        state.chat_list_pane.scroll_vertical = selected;
    } else if selected >= scroll + page_size {
        state.chat_list_pane.scroll_vertical = selected + 1 - page_size;
    }
}

pub fn log_view_max_scroll(state: &UiState) -> usize {
    let width = (state.log_view.pane.viewport.width as usize).max(1);
    let effective_width = if width < 10 { 80 } else { width };
    let metrics = log_pane_metrics(state, effective_width);
    state
        .log_view
        .pane
        .max_vertical_scroll(metrics, PaneConfig::log_pane())
}

pub fn log_view_max_horizontal_scroll(state: &UiState) -> usize {
    let width = (state.log_view.pane.viewport.width as usize).max(1);
    let effective_width = if width < 10 { 80 } else { width };
    let metrics = log_pane_metrics(state, effective_width);
    state
        .log_view
        .pane
        .max_horizontal_scroll(metrics, PaneConfig::log_pane())
}

pub fn message_max_scroll(state: &UiState) -> usize {
    let metrics = PaneMetrics {
        line_count: message_total_lines(&state.messages),
        max_line_width: 0,
    };
    state
        .message_view
        .pane
        .max_vertical_scroll(metrics, PaneConfig::message_pane())
}

pub fn message_max_horizontal_scroll(state: &UiState) -> usize {
    let metrics = PaneMetrics {
        line_count: 0,
        max_line_width: message_max_line_width(state),
    };
    state
        .message_view
        .pane
        .max_horizontal_scroll(metrics, PaneConfig::message_pane())
}

pub(crate) fn message_line_offset(messages: &[MessageItem], index: usize) -> usize {
    messages.iter().take(index).map(message_line_count).sum()
}

fn message_total_lines(messages: &[MessageItem]) -> usize {
    messages.iter().map(message_line_count).sum()
}

pub(crate) fn message_line_count(message: &MessageItem) -> usize {
    let lines = message.body.lines().count();
    lines.max(1)
}

fn message_max_line_width_for(messages: &[MessageItem]) -> usize {
    let mut max_width = 0;
    for message in messages {
        let timestamp = if message.timestamp.is_empty() {
            String::new()
        } else {
            format!("[{}] ", message.timestamp)
        };
        let header = format!("> [  ] {}{}: ", timestamp, message.author);
        let header_width = header.chars().count();
        let mut body_lines = message.body.lines();
        if let Some(first_line) = body_lines.next() {
            max_width = max_width.max(header_width + first_line.chars().count());
            for line in body_lines {
                max_width = max_width.max(header_width + line.chars().count());
            }
        } else {
            max_width = max_width.max(header_width);
        }
    }
    max_width
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

fn is_chat_focus(focus: UiFocus) -> bool {
    matches!(focus, UiFocus::Chats | UiFocus::ChatSearch)
}

pub fn draw(frame: &mut Frame, state: &UiState) {
    let area = frame.size();
    let layout = layout_areas(area, state.chat_list_width);
    let overlay_active =
        state.draft_modal.is_open || state.command_palette.is_open || state.log_view.is_open;
    let chat_focused = !overlay_active && is_chat_focus(state.focus);
    let message_focused = !overlay_active && is_message_focus(state.focus);
    let composer_focused = !overlay_active && state.focus == UiFocus::Composer;
    let composer_cursor_visible = composer_focused && state.composer_cursor_visible;

    frame.render_widget(Clear, layout.messages);

    let chat_labels: Vec<String> = if state.chats.is_empty() {
        vec!["No chats".to_string()]
    } else {
        state.chats.iter().map(ChatListItem::label).collect()
    };
    let chat_metrics = PaneMetrics::from_lines(&chat_labels);
    let scroll_offset = state.chat_list_pane.scroll_horizontal;
    let chat_items: Vec<ListItem> = chat_labels
        .into_iter()
        .map(|label| ListItem::new(apply_horizontal_scroll(&label, scroll_offset)))
        .collect();

    let mut chat_state = ListState::default();
    let selected_chat = state.chats.iter().position(|chat| chat.is_selected);
    chat_state.select(selected_chat);
    *chat_state.offset_mut() = state.chat_list_pane.scroll_vertical;

    let chat_title = if state.chat_search.is_open || !state.chat_search.query.text.is_empty() {
        if state.chat_search.query.text.is_empty() {
            "Chats (search)".to_string()
        } else {
            format!("Chats (search: {})", state.chat_search.query.text)
        }
    } else {
        "Chats".to_string()
    };

    let chat_block = Block::default()
        .title(chat_title)
        .borders(Borders::ALL)
        .border_style(focus_border_style(chat_focused));
    let chat_list =
        List::new(chat_items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let message_content = message_pane_content(state);
    let message_title = message_view_title(state);

    let message_block = Block::default()
        .title(message_title)
        .borders(Borders::ALL)
        .border_style(focus_border_style(message_focused));

    let chat_layout = pane_layout(layout.chat_list, &chat_block, PaneConfig::chat_list_pane());
    frame.render_widget(chat_block, layout.chat_list);
    frame.render_stateful_widget(chat_list, chat_layout.text_area, &mut chat_state);
    render_scrollbars(
        frame,
        chat_layout,
        &state.chat_list_pane,
        chat_metrics,
        PaneConfig::chat_list_pane(),
    );
    render_pane(
        frame,
        layout.messages,
        message_block,
        message_content.text.as_str(),
        &state.message_view.pane,
        message_content.metrics,
        PaneConfig::message_pane(),
    );

    let composer_block = Block::default()
        .title("Composer")
        .borders(Borders::ALL)
        .border_style(focus_border_style(composer_focused));
    let composer_content = composer_pane_content(state, composer_cursor_visible);
    render_pane(
        frame,
        layout.composer,
        composer_block,
        composer_content.text.as_str(),
        &state.composer_pane,
        composer_content.metrics,
        PaneConfig::composer_pane(),
    );

    draw_key_hints(frame, state, layout.key_hints);

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

struct PaneContent {
    text: String,
    metrics: PaneMetrics,
}

fn message_pane_content(state: &UiState) -> PaneContent {
    let lines = if state.messages.is_empty() {
        vec!["No messages".to_string()]
    } else {
        build_message_lines(state)
    };
    let metrics = PaneMetrics::from_lines(&lines);
    PaneContent {
        text: lines.join("\n"),
        metrics,
    }
}

fn composer_pane_content(state: &UiState, show_cursor: bool) -> PaneContent {
    let mut input = state.input.clone();
    input.clamp_cursor();
    let mut text = input.text;
    if show_cursor {
        if input.cursor <= text.len() {
            text.insert(input.cursor, '_');
        } else {
            text.push('_');
        }
    }
    let metrics = PaneMetrics::from_text(text.as_str());
    PaneContent { text, metrics }
}

fn message_max_line_width(state: &UiState) -> usize {
    message_max_line_width_for(&state.messages)
}

fn log_pane_content(state: &UiState, width: usize) -> (Vec<String>, PaneMetrics) {
    if state.logs.is_empty() {
        return (
            vec!["No logs".to_string()],
            PaneMetrics::from_text("No logs"),
        );
    }

    let width = width.max(1);
    let options = Options::new(width).break_words(true);
    let mut wrapped_lines = Vec::new();

    for log in &state.logs {
        let lines = textwrap::wrap(log, &options);
        for line in lines {
            wrapped_lines.push(line.into_owned());
        }
    }

    if wrapped_lines.is_empty() {
        // Should not happen if logs not empty, but safety check
        wrapped_lines.push(String::new());
    }

    let metrics = PaneMetrics {
        line_count: wrapped_lines.len(),
        max_line_width: width, // Since we wrapped to width
    };

    (wrapped_lines, metrics)
}

pub fn get_selected_log_text(state: &UiState) -> Option<String> {
    let (anchor, cursor) = state.log_view.selection?;
    let width = (state.log_view.pane.viewport.width as usize).max(1);
    let effective_width = if width < 10 { 80 } else { width };
    let (lines, _) = log_pane_content(state, effective_width);

    let min = anchor.min(cursor);
    let max = anchor.max(cursor);

    let selected_lines: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i >= min && *i <= max)
        .map(|(_, line)| line.as_str())
        .collect();

    if selected_lines.is_empty() {
        None
    } else {
        Some(selected_lines.join("\n"))
    }
}

pub fn log_pane_metrics(state: &UiState, width: usize) -> PaneMetrics {
    if state.logs.is_empty() {
        return PaneMetrics::from_text("No logs");
    }
    let width = width.max(1);
    let options = Options::new(width).break_words(true);
    let mut line_count = 0;

    for log in &state.logs {
        line_count += textwrap::wrap(log, &options).len();
    }

    PaneMetrics {
        line_count,
        max_line_width: width,
    }
}

fn draw_log_window(frame: &mut Frame, state: &UiState, area: Rect) {
    let log_area = log_window_area(area);
    frame.render_widget(Clear, log_area);

    let block_inner_width = log_area.width.saturating_sub(2).max(1) as usize;
    let (wrapped_lines, metrics) = log_pane_content(state, block_inner_width);

    let mut styled_lines = Vec::new();
    let selection_range = state.log_view.selection.map(|(anchor, cursor)| {
        let min = anchor.min(cursor);
        let max = anchor.max(cursor);
        (min, max)
    });

    for (idx, line) in wrapped_lines.iter().enumerate() {
        let is_selected = if let Some((min, max)) = selection_range {
            idx >= min && idx <= max
        } else {
            false
        };

        if is_selected {
            styled_lines.push(Line::styled(
                line.clone(),
                Style::default().add_modifier(Modifier::REVERSED),
            ));
        } else {
            styled_lines.push(Line::from(line.clone()));
        }
    }

    let log_block = Block::default()
        .title("Logs")
        .borders(Borders::ALL)
        .border_style(focus_border_style(true));

    render_pane(
        frame,
        log_area,
        log_block,
        ratatui::text::Text::from(styled_lines),
        &state.log_view.pane,
        metrics,
        PaneConfig::log_pane(),
    );
}

// Helper for calculating area if not defined (it was referenced in tui.rs imports but not found in viewed code?
// Ah wait, tui.rs imported log_window_text_area, not log_window_area?
// view.rs view output check:
// Line 697: let log_area = log_window_area(area);
// I need to ensure log_window_area exists or is defined.
// Looking at previous view_file output...
// It was not shown in the first 800 lines? or I missed it?
// Let's assume it exists or I need to find it.
// Wait, I saw `draw_log_window` use it.
// Ah, `log_window_area` is NOT in the shown lines (1-800).
// It must be further down.
// I will not replace it if I can't confirm it exists, but I am replacing `draw_log_window` which CALLS it.
// So I should just keep calling it.

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

fn draw_key_hints(frame: &mut Frame, state: &UiState, area: Rect) {
    if let Some(msg) = &state.status_message {
        let span = Span::styled(
            msg,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_widget(Paragraph::new(Line::from(span)), area);
        return;
    }

    let hints = key_hints_for_focus(state.focus);
    let mut spans = Vec::new();
    for (i, (key, desc)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" | "));
        }
        spans.push(Span::styled(
            format!(" {} ", key),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray)
                .fg(Color::White),
        ));
        spans.push(Span::raw(format!(" {} ", desc)));
    }
    // Add global hints
    if !hints.is_empty() {
        spans.push(Span::raw(" | "));
    }
    spans.push(Span::styled(
        " Tab ",
        Style::default()
            .add_modifier(Modifier::BOLD)
            .bg(Color::DarkGray)
            .fg(Color::White),
    ));
    spans.push(Span::raw(" Next Pane "));

    spans.push(Span::raw(" | "));
    spans.push(Span::styled(
        " Ctrl+l ",
        Style::default()
            .add_modifier(Modifier::BOLD)
            .bg(Color::DarkGray)
            .fg(Color::White),
    ));
    spans.push(Span::raw(" Log "));

    spans.push(Span::raw(" | "));
    spans.push(Span::styled(
        " Ctrl+q ",
        Style::default()
            .add_modifier(Modifier::BOLD)
            .bg(Color::DarkGray)
            .fg(Color::White),
    ));
    spans.push(Span::raw(" Quit "));

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::Reset));
    frame.render_widget(paragraph, area);
}

fn key_hints_for_focus(focus: UiFocus) -> Vec<(&'static str, &'static str)> {
    match focus {
        UiFocus::Chats => vec![("j/k", "Nav"), ("Enter", "Select"), ("/", "Search")],
        UiFocus::ChatSearch => vec![("Esc", "Cancel"), ("Enter", "Done"), ("Up/Down", "Nav")],
        UiFocus::Messages => vec![
            ("j/k", "Scroll"),
            ("Space", "Select"),
            ("/", "Search"),
            ("i", "Composer"),
            ("Ctrl+e", "Export"),
        ],
        UiFocus::Composer => vec![("Esc", "Unfocus"), ("Enter", "Send")],
        UiFocus::Search => vec![("Esc", "Cancel"), ("Enter", "Jump"), ("Up/Down", "Nav")],
    }
}

pub fn log_window_page_size(area: Rect) -> usize {
    let text_area = log_window_text_area(area);
    text_area.height.max(1) as usize
}

fn log_window_area(area: Rect) -> Rect {
    centered_rect(area, 90, 90)
}

pub fn log_window_text_area(area: Rect) -> Rect {
    let log_area = log_window_area(area);
    let block = Block::default().borders(Borders::ALL);
    let layout = pane_layout(log_area, &block, PaneConfig::log_pane());
    layout.text_area
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
        assert_eq!(message_viewport_page_size(area, DEFAULT_CHAT_LIST_WIDTH), 3);
    }

    #[test]
    fn chat_list_max_horizontal_scroll_respects_viewport() {
        let mut state = UiState {
            chats: vec![ChatListItem {
                id: 1,
                title: "Long Chat Title".to_string(),
                unread: 0,
                is_selected: true,
            }],
            ..Default::default()
        };
        state.chat_list_pane.viewport.width = 8;

        assert_eq!(chat_list_max_horizontal_scroll(&state), 7);
    }

    #[test]
    fn message_max_scroll_accounts_for_multiline_messages() {
        let mut state = UiState::default();
        state.message_view.pane.page_size = 2;
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

    #[test]
    fn key_hints_change_based_on_focus() {
        let chats = key_hints_for_focus(UiFocus::Chats);
        assert!(chats.iter().any(|(k, _)| *k == "Enter"));
        assert!(!chats.iter().any(|(k, _)| *k == "i"));

        let messages = key_hints_for_focus(UiFocus::Messages);
        assert!(messages.iter().any(|(k, _)| *k == "Space"));
        assert!(messages.iter().any(|(k, _)| *k == "i"));

        let composer = key_hints_for_focus(UiFocus::Composer);
        assert!(composer.iter().any(|(k, _)| *k == "Esc"));
    }
    #[test]
    fn get_selected_log_text_extracts_correct_range() {
        let mut state = UiState {
            logs: vec![
                "Log line 1".to_string(),
                "Log line 2".to_string(),
                "Log line 3".to_string(),
            ],
            log_view: LogViewState {
                is_open: true,
                selection: Some((0, 1)), // Select first two lines
                ..LogViewState::default()
            },
            ..UiState::default()
        };
        // Set viewport width distinct from wrapping default to test behavior
        state.log_view.pane.viewport.width = 100;

        let selected = super::get_selected_log_text(&state);
        assert_eq!(selected, Some("Log line 1\nLog line 2".to_string()));

        // Test single line selection
        state.log_view.selection = Some((1, 1));
        let selected = super::get_selected_log_text(&state);
        assert_eq!(selected, Some("Log line 2".to_string()));

        // Test reverse selection (cursor < anchor)
        state.log_view.selection = Some((2, 0));
        let selected = super::get_selected_log_text(&state);
        assert_eq!(
            selected,
            Some("Log line 1\nLog line 2\nLog line 3".to_string())
        );
    }
}
