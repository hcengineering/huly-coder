// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

use crate::{config::Config, tui::Theme};

pub struct ToolbarWidget;

impl ToolbarWidget {
    pub fn render(self, area: Rect, buf: &mut Buffer, theme: &Theme, config: &Config) {
        Block::bordered()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::QuadrantOutside)
            .border_style(Style::default().fg(theme.background).bg(theme.panel_shadow))
            .style(theme.panel_style())
            .padding(Padding::horizontal(1))
            .render(area, buf);

        let toolbar_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20),  // Title
                Constraint::Ratio(2, 3), // Context length
                Constraint::Ratio(1, 3), // Model
            ])
            .split(area);

        Line::default()
            .spans([
                Span::styled(" Huly Coder ", Style::default().fg(theme.focus)),
                Span::styled(
                    format!("v{}", env!("CARGO_PKG_VERSION")),
                    Style::default().fg(theme.inactive_text),
                ),
            ])
            .render(toolbar_layout[0], buf);

        let toolbar_text = Line::from(vec![
            Span::styled(
                format!("{:?}", config.provider),
                Style::default().fg(theme.highlight),
            ),
            Span::styled(" | ", Style::default().fg(theme.focus)),
            Span::styled(&config.model, Style::default().fg(theme.text)),
            Span::styled(" | ", Style::default().fg(theme.focus)),
            Span::styled(
                format!("{:?}", config.permission_mode),
                Style::default().fg(theme.text),
            ),
            Span::styled(" ", Style::default().fg(theme.text)),
        ]);

        Paragraph::new(toolbar_text)
            .alignment(Alignment::Right)
            .render(toolbar_layout[2], buf);
    }
}
