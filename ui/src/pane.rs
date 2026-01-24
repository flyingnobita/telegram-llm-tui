use ratatui::{
    layout::Rect,
    text::Text,
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct PaneMetrics {
    pub line_count: usize,
    pub max_line_width: usize,
}

impl PaneMetrics {
    pub fn from_lines(lines: &[String]) -> Self {
        let mut max_line_width = 0;
        for line in lines {
            max_line_width = max_line_width.max(line.chars().count());
        }
        let line_count = lines.len().max(1);
        Self {
            line_count,
            max_line_width,
        }
    }

    pub fn from_text(text: &str) -> Self {
        let mut line_count = 0;
        let mut max_line_width = 0;
        for line in text.lines() {
            line_count += 1;
            max_line_width = max_line_width.max(line.chars().count());
        }
        if line_count == 0 {
            line_count = 1;
        }
        Self {
            line_count,
            max_line_width,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PaneViewport {
    pub width: u16,
    pub height: u16,
}

impl PaneViewport {
    pub fn from_rect(rect: Rect) -> Self {
        Self {
            width: rect.width,
            height: rect.height,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PaneConfig {
    pub enable_vertical_scroll: bool,
    pub enable_horizontal_scroll: bool,
    pub show_vertical_scrollbar: bool,
    pub show_horizontal_scrollbar: bool,
    pub enable_wrap: bool,
}

impl PaneConfig {
    pub fn chat_list_pane() -> Self {
        Self {
            enable_vertical_scroll: true,
            enable_horizontal_scroll: true,
            show_vertical_scrollbar: true,
            show_horizontal_scrollbar: true,
            enable_wrap: false,
        }
    }

    pub fn message_pane() -> Self {
        Self {
            enable_vertical_scroll: true,
            enable_horizontal_scroll: true,
            show_vertical_scrollbar: true,
            show_horizontal_scrollbar: true,
            enable_wrap: false,
        }
    }

    pub fn composer_pane() -> Self {
        Self {
            enable_vertical_scroll: false,
            enable_horizontal_scroll: false,
            show_vertical_scrollbar: false,
            show_horizontal_scrollbar: false,
            enable_wrap: false,
        }
    }

    pub fn log_pane() -> Self {
        Self {
            enable_vertical_scroll: true,
            enable_horizontal_scroll: false,
            show_vertical_scrollbar: true,
            show_horizontal_scrollbar: false,
            enable_wrap: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PaneLayout {
    pub inner: Rect,
    pub text_area: Rect,
    pub vertical_scrollbar: Option<Rect>,
    pub horizontal_scrollbar: Option<Rect>,
}

#[derive(Debug, Clone)]
pub struct PaneState {
    pub scroll_vertical: usize,
    pub scroll_horizontal: usize,
    pub viewport: PaneViewport,
    pub page_size: usize,
}

impl Default for PaneState {
    fn default() -> Self {
        Self {
            scroll_vertical: 0,
            scroll_horizontal: 0,
            viewport: PaneViewport::default(),
            page_size: 1,
        }
    }
}

impl PaneState {
    pub fn set_viewport(&mut self, viewport: PaneViewport, page_size: usize) {
        self.viewport = viewport;
        self.page_size = page_size.max(1);
    }

    pub fn max_vertical_scroll(&self, metrics: PaneMetrics, config: PaneConfig) -> usize {
        if !config.enable_vertical_scroll {
            return 0;
        }
        let page_size = self.page_size.max(1);
        metrics.line_count.saturating_sub(page_size)
    }

    pub fn max_horizontal_scroll(&self, metrics: PaneMetrics, config: PaneConfig) -> usize {
        if !config.enable_horizontal_scroll {
            return 0;
        }
        let viewport_width = self.viewport.width.max(1) as usize;
        metrics.max_line_width.saturating_sub(viewport_width)
    }

    pub fn clamp_offsets(&mut self, metrics: PaneMetrics, config: PaneConfig) {
        if !config.enable_vertical_scroll {
            self.scroll_vertical = 0;
        } else {
            let max_scroll = self.max_vertical_scroll(metrics, config);
            self.scroll_vertical = self.scroll_vertical.min(max_scroll);
        }

        if !config.enable_horizontal_scroll {
            self.scroll_horizontal = 0;
        } else {
            let max_scroll = self.max_horizontal_scroll(metrics, config);
            self.scroll_horizontal = self.scroll_horizontal.min(max_scroll);
        }
    }

    pub fn clamped_offsets(&self, metrics: PaneMetrics, config: PaneConfig) -> (u16, u16) {
        let vertical = if config.enable_vertical_scroll {
            self.scroll_vertical
                .min(self.max_vertical_scroll(metrics, config))
        } else {
            0
        };
        let horizontal = if config.enable_horizontal_scroll {
            self.scroll_horizontal
                .min(self.max_horizontal_scroll(metrics, config))
        } else {
            0
        };
        (
            vertical.min(u16::MAX as usize) as u16,
            horizontal.min(u16::MAX as usize) as u16,
        )
    }
}

pub fn pane_layout(area: Rect, block: &Block, config: PaneConfig) -> PaneLayout {
    let inner = block.inner(area);
    let show_vertical =
        config.show_vertical_scrollbar && config.enable_vertical_scroll && inner.width > 1;
    let show_horizontal =
        config.show_horizontal_scrollbar && config.enable_horizontal_scroll && inner.height > 1;

    let text_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner
            .width
            .saturating_sub(if show_vertical { 1 } else { 0 })
            .max(1),
        height: inner
            .height
            .saturating_sub(if show_horizontal { 1 } else { 0 })
            .max(1),
    };

    let vertical_scrollbar = if show_vertical {
        Some(Rect {
            x: text_area.x + text_area.width,
            y: text_area.y,
            width: 1,
            height: text_area.height,
        })
    } else {
        None
    };

    let horizontal_scrollbar = if show_horizontal {
        Some(Rect {
            x: text_area.x,
            y: text_area.y + text_area.height,
            width: text_area.width,
            height: 1,
        })
    } else {
        None
    };

    PaneLayout {
        inner,
        text_area,
        vertical_scrollbar,
        horizontal_scrollbar,
    }
}

pub fn render_pane<'a>(
    frame: &mut Frame,
    area: Rect,
    block: Block<'a>,
    text: impl Into<Text<'a>>,
    state: &PaneState,
    metrics: PaneMetrics,
    config: PaneConfig,
) -> PaneLayout {
    let layout = pane_layout(area, &block, config);
    let (scroll_vertical, scroll_horizontal) = state.clamped_offsets(metrics, config);
    let paragraph = Paragraph::new(text).scroll((scroll_vertical, scroll_horizontal));

    frame.render_widget(block, area);
    frame.render_widget(paragraph, layout.text_area);
    render_scrollbars(frame, layout, state, metrics, config);

    layout
}

pub fn render_scrollbars(
    frame: &mut Frame,
    layout: PaneLayout,
    state: &PaneState,
    metrics: PaneMetrics,
    config: PaneConfig,
) {
    let (scroll_vertical, scroll_horizontal) = state.clamped_offsets(metrics, config);
    if let Some(scroll_area) = layout.vertical_scrollbar {
        let mut scroll_state =
            ScrollbarState::new(metrics.line_count).position(scroll_vertical as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            scroll_area,
            &mut scroll_state,
        );
    }

    if let Some(scroll_area) = layout.horizontal_scrollbar {
        let mut scroll_state =
            ScrollbarState::new(metrics.max_line_width).position(scroll_horizontal as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::HorizontalBottom),
            scroll_area,
            &mut scroll_state,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Borders;
    use ratatui::Terminal;

    #[test]
    fn max_vertical_scroll_handles_empty_and_multiline() {
        let state = PaneState {
            page_size: 3,
            ..PaneState::default()
        };
        let config = PaneConfig {
            enable_vertical_scroll: true,
            enable_wrap: false,
            ..PaneConfig::default()
        };

        assert_eq!(
            state.max_vertical_scroll(
                PaneMetrics {
                    line_count: 0,
                    max_line_width: 0,
                },
                config
            ),
            0
        );
        assert_eq!(
            state.max_vertical_scroll(
                PaneMetrics {
                    line_count: 1,
                    max_line_width: 0,
                },
                config
            ),
            0
        );
        assert_eq!(
            state.max_vertical_scroll(
                PaneMetrics {
                    line_count: 5,
                    max_line_width: 0,
                },
                config
            ),
            2
        );
    }

    #[test]
    fn max_horizontal_scroll_respects_viewport_width() {
        let state = PaneState {
            viewport: PaneViewport {
                width: 10,
                height: 3,
            },
            ..PaneState::default()
        };
        let config = PaneConfig {
            enable_horizontal_scroll: true,
            ..PaneConfig::default()
        };

        assert_eq!(
            state.max_horizontal_scroll(
                PaneMetrics {
                    line_count: 1,
                    max_line_width: 5,
                },
                config
            ),
            0
        );

        let state = PaneState {
            viewport: PaneViewport {
                width: 4,
                height: 3,
            },
            ..PaneState::default()
        };

        assert_eq!(
            state.max_horizontal_scroll(
                PaneMetrics {
                    line_count: 1,
                    max_line_width: 5,
                },
                config
            ),
            1
        );
    }

    #[test]
    fn clamp_offsets_handles_shrinking_content() {
        let mut state = PaneState {
            scroll_vertical: 5,
            scroll_horizontal: 7,
            viewport: PaneViewport {
                width: 4,
                height: 3,
            },
            page_size: 2,
        };
        let config = PaneConfig {
            enable_vertical_scroll: true,
            enable_horizontal_scroll: true,
            enable_wrap: false,
            ..PaneConfig::default()
        };

        state.clamp_offsets(
            PaneMetrics {
                line_count: 3,
                max_line_width: 5,
            },
            config,
        );

        assert_eq!(state.scroll_vertical, 1);
        assert_eq!(state.scroll_horizontal, 1);
    }

    #[test]
    fn render_pane_returns_text_area_with_scrollbar_spacing() {
        let backend = TestBackend::new(10, 5);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        let layout = std::cell::RefCell::new(None);

        terminal
            .draw(|frame| {
                let area = frame.size();
                let block = Block::default().borders(Borders::ALL);
                let state = PaneState::default();
                let metrics = PaneMetrics {
                    line_count: 3,
                    max_line_width: 5,
                };
                let config = PaneConfig::message_pane();
                let result = render_pane(frame, area, block, "hello", &state, metrics, config);
                *layout.borrow_mut() = Some(result);
            })
            .expect("render pane");

        let layout = layout.into_inner().expect("layout result");
        let inner = Block::default()
            .borders(Borders::ALL)
            .inner(Rect::new(0, 0, 10, 5));
        assert_eq!(layout.text_area.width, inner.width.saturating_sub(1).max(1));
        assert_eq!(
            layout.text_area.height,
            inner.height.saturating_sub(1).max(1)
        );
    }

    #[test]
    fn pane_layout_skips_scrollbars_when_disabled_or_too_small() {
        let block = Block::default().borders(Borders::ALL);
        let area = Rect::new(0, 0, 1, 1);
        let config = PaneConfig::message_pane();
        let layout = pane_layout(area, &block, config);
        assert!(layout.vertical_scrollbar.is_none());
        assert!(layout.horizontal_scrollbar.is_none());

        let area = Rect::new(0, 0, 10, 5);
        let config = PaneConfig {
            enable_vertical_scroll: true,
            enable_horizontal_scroll: true,
            show_vertical_scrollbar: false,
            show_horizontal_scrollbar: false,
            enable_wrap: false,
        };
        let layout = pane_layout(area, &block, config);
        assert!(layout.vertical_scrollbar.is_none());
        assert!(layout.horizontal_scrollbar.is_none());
    }
}
