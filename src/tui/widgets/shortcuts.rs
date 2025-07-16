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
        let shortcuts = [
            ("^n", "New Task"),
            ("^p", "Pause/Resume Task"),
            ("⇥", "Change Focus"),
            #[cfg(target_os = "macos")]
            ("⌥[1-4]", "Focus Panel"),
            #[cfg(not(target_os = "macos"))]
            ("Alt+[1-4]", "Focus Panel"),
            ("↑↓", "Navigate"),
            ("Enter", "Select"),
            ("^e", "Export History"),
            ("^w", "Quit"),
        ];

        let shortcuts_text = Text::from(Line::from(
            shortcuts
                .iter()
                .flat_map(|(shortcut, description)| {
                    [
                        Span::styled(
                            shortcut.to_string(),
                            Style::default().fg(theme.highlight_text),
                        ),
                        Span::styled(
                            format!(": {description}"),
                            Style::default().fg(theme.inactive_text),
                        ),
                        Span::styled(format!(" | "), Style::default().fg(theme.text)),
                    ]
                })
                .collect::<Vec<_>>(),
        ));

        Paragraph::new(shortcuts_text)
            .block(block)
            .alignment(Alignment::Right)
            .render(area, buf);
    }
}
