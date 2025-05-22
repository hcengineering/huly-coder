// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use crate::agent::event::AgentCommandStatus;
use crate::tui::app::TerminalState;
use crate::tui::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
    StatefulWidget, Widget,
};

pub struct TerminalWidget;

impl TerminalWidget {
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
        is_focused: bool,
        ui_state: &mut TerminalState,
        model_state: &[AgentCommandStatus],
        theme: &Theme,
        throbber_state: &throbber_widgets_tui::ThrobberState,
    ) {
        let mut title = Line::default().left_aligned();
        if !model_state.is_empty() {
            ui_state.selected_idx = ui_state.selected_idx.clamp(0, model_state.len() - 1);
        } else {
            ui_state.selected_idx = 0;
        }

        model_state.iter().enumerate().for_each(|(idx, status)| {
            let style = if idx == ui_state.selected_idx {
                Style::default().bg(theme.focus).fg(Color::Black)
            } else {
                Style::default()
            };
            if status.is_active {
                let mut state = throbber_state.clone();
                let throbber = throbber_widgets_tui::Throbber::default()
                    .throbber_set(throbber_widgets_tui::WHITE_CIRCLE);
                state.normalize(&throbber);
                let len = throbber_widgets_tui::WHITE_CIRCLE.symbols.len() as i8;
                let progress = if 0 <= state.index() && state.index() < len {
                    throbber_widgets_tui::WHITE_CIRCLE.symbols[state.index() as usize]
                } else {
                    throbber_widgets_tui::WHITE_CIRCLE.empty
                };

                let str = format!("[{}:{}{}]", idx + 1, status.command_id, progress);
                title.spans.push(Span::styled(str, style));
            } else {
                let str = format!("[{}:{}]", idx + 1, status.command_id);
                title.spans.push(Span::styled(str, style));
            }
            title.spans.push(Span::raw(" "));
        });
        let block = Block::bordered()
            .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
            .padding(Padding::horizontal(1))
            .title(" Terminal ")
            .title_top(title)
            .title_alignment(Alignment::Right)
            .title_style(theme.primary_style())
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style(is_focused));

        let mut terminal_lines = vec![];
        if let Some(state) = model_state.get(ui_state.selected_idx) {
            terminal_lines.push(Line::styled(
                format!("> {}", state.command.clone().unwrap_or_default()),
                theme.primary_style(),
            ));
            if !state.output.is_empty() {
                let output = state.output.replace("\\n", "\n");
                output.lines().for_each(|line| {
                    terminal_lines.push(Line::styled(line.to_string(), theme.inactive_text_style()))
                });
            }
        }

        ui_state.scroll_state = ui_state
            .scroll_state
            .content_length(terminal_lines.len())
            .position(ui_state.scroll_position.into());

        Paragraph::new(terminal_lines)
            .block(block)
            .style(theme.text_style())
            .scroll((ui_state.scroll_position, 0))
            .render(area, buf);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_symbol("▐");
        scrollbar.render(
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            buf,
            &mut ui_state.scroll_state,
        );
    }
}
