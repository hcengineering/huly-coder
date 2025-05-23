// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use crate::agent::event::AgentState;
use crate::tui::{tool_info, Theme};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

pub struct TaskStatusWidget;

impl TaskStatusWidget {
    pub fn render(
        self,
        area: Rect,
        buf: &mut Buffer,
        state: &AgentState,
        theme: &Theme,
        throbber_state: &throbber_widgets_tui::ThrobberState,
    ) {
        let block = Block::bordered()
            .borders(Borders::LEFT | Borders::RIGHT)
            .border_set(symbols::border::FULL)
            .border_style(theme.border_style(false))
            .style(Style::default().bg(theme.border));
        let max_len = area.width.saturating_sub(5) as usize;
        let (icon, message) = match state {
            AgentState::ToolCall(tool, args) => {
                let (icon, info) = tool_info::get_tool_call_info(tool, args);
                (Span::raw(icon), Span::raw(info))
            }
            AgentState::Paused => (Span::from("⏸️"), Span::raw("Agent paused")),
            AgentState::WaitingResponse => {
                let simple = throbber_widgets_tui::Throbber::default()
                    .throbber_set(throbber_widgets_tui::CLOCK);
                (
                    simple.to_symbol_span(throbber_state),
                    Span::raw("Waiting response from model..."),
                )
            }
            AgentState::Thinking => {
                let simple = throbber_widgets_tui::Throbber::default()
                    .throbber_set(throbber_widgets_tui::CLOCK);
                (
                    simple.to_symbol_span(throbber_state),
                    Span::raw("Thinking..."),
                )
            }
            AgentState::WaitingUserPrompt => (Span::from("✍️"), Span::raw("Waiting user prompt")),
            AgentState::Error(msg) => (
                Span::from("⚠️"),
                Span::raw(if msg.len() > max_len {
                    format!("{}...", &msg[..max_len - 3])
                } else {
                    msg.to_string()
                }),
            ),
            AgentState::Completed(_) => (Span::from("✅"), Span::raw("Completed")),
        };

        Paragraph::new(Line::default().spans(vec![icon, Span::raw(" "), message]))
            .block(block)
            .render(area, buf);
    }
}
