// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::tui::Theme;

pub struct ShortcutsWidget;

impl ShortcutsWidget {
    pub fn render(self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let block = Block::bordered()
            .borders(Borders::TOP)
            .border_type(BorderType::Plain)
            .border_style(theme.border_style(false))
            .style(Style::default().bg(theme.background));

        let shortcuts_text = Text::from(vec![Line::from(vec![
            Span::styled("^n", theme.highlight_style()),
            Span::styled(": New Task | ", theme.inactive_style()),
            Span::styled("^p", theme.highlight_style()),
            Span::styled(": Pause/Resume Task | ", theme.inactive_style()),
            Span::styled("⇥", theme.highlight_style()),
            Span::styled(": Change Focus | ", theme.inactive_style()),
            #[cfg(target_os = "macos")]
            Span::styled("⌥[1-4]", theme.highlight_style()),
            #[cfg(not(target_os = "macos"))]
            Span::styled("Alt+[1-4]", theme.highlight_style()),
            Span::styled(": Focus Panel | ", theme.inactive_style()),
            Span::styled("↑↓", theme.highlight_style()),
            Span::styled(": Navigate | ", theme.inactive_style()),
            Span::styled("Enter", theme.highlight_style()),
            Span::styled(": Select | ", theme.inactive_style()),
            Span::styled("^q", theme.highlight_style()),
            Span::styled(": Quit ", theme.inactive_style()),
        ])]);

        Paragraph::new(shortcuts_text)
            .block(block)
            .alignment(Alignment::Right)
            .render(area, buf);
    }
}
