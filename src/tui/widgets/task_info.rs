// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use ratatui::{
    layout::Offset,
    prelude::*,
    widgets::{Block, BorderType, Borders, LineGauge, Padding, Paragraph},
};

use crate::tui::{app::AgentStatus, Theme};

pub struct TaskInfoWidget;

impl TaskInfoWidget {
    pub fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        state: &AgentStatus,
        task: &str,
    ) {
        let task_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Task name
                Constraint::Length(1), // Status of task
            ])
            .split(area.inner(Margin::new(2, 0)).offset(Offset { x: 0, y: 1 }));

        let task_status_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(10),  // Progress
                Constraint::Length(5), // Max tokens
                Constraint::Min(20),   // Price
                Constraint::Min(4),    // Empty
            ])
            .split(task_layout[1]);

        Block::bordered()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .title(" Current Task ")
            .title_alignment(Alignment::Right)
            .title_style(theme.text_style())
            .padding(Padding::horizontal(1))
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style(false))
            .render(area, buf);

        Paragraph::new(task).render(task_layout[0], buf);

        let total_tokens = (state.current_input_tokens + state.current_completion_tokens) as f64;
        let progress_value =
            total_tokens / f64::max(total_tokens, f64::max(1.0, state.max_tokens as f64));
        let cost = state.input_price * (state.current_input_tokens as f64)
            + state.completion_price * (state.current_completion_tokens as f64);
        LineGauge::default()
            .filled_style(Style::default().fg(Color::Blue))
            .unfilled_style(Style::default().fg(Color::DarkGray))
            .line_set(symbols::line::ROUNDED)
            .label(format_num::format_num!(".2s", total_tokens))
            .ratio(progress_value)
            .render(task_status_layout[0], buf);

        Span::raw(format_num::format_num!(" .2s", state.max_tokens))
            .render(task_status_layout[1], buf);

        Paragraph::new(Line::default().spans([
            Span::styled(" │ ", theme.border_style(false)),
            Span::raw(format!(
                "API Cost: ${}",
                format_num::format_num!(".2f", cost)
            )),
        ]))
        .right_aligned()
        .render(task_status_layout[2], buf);
    }
}
