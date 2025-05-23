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
                Constraint::Length(15),  // Title
                Constraint::Ratio(2, 3), // Context length
                Constraint::Ratio(1, 3), // Model
            ])
            .split(area);

        Span::styled(" Huly Coder ", Style::default().fg(theme.focus))
            .render(toolbar_layout[0], buf);

        let toolbar_text = Line::from(vec![
            Span::styled(
                format!("{:?}", config.provider),
                Style::default().fg(theme.highlight),
            ),
            Span::styled(" | ", Style::default().fg(theme.focus)),
            Span::styled(&config.model, Style::default().fg(theme.text)),
            Span::styled(" ", Style::default().fg(theme.text)),
        ]);

        Paragraph::new(toolbar_text)
            .alignment(Alignment::Right)
            .render(toolbar_layout[2], buf);
    }
}
