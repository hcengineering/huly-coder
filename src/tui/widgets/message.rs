use itertools::Itertools;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph, StatefulWidget, Widget};
use rig::message::{AssistantContent, Message, ToolResultContent, UserContent};

use crate::tui::Theme;

#[derive(Debug, Clone)]
pub struct MessageWidget<'a> {
    theme: &'a Theme,
    lines: Vec<Line<'a>>,
    in_progress: bool,
    throbber_state: throbber_widgets_tui::ThrobberState,
    pub is_selected: bool,
}

impl<'a> MessageWidget<'a> {
    pub fn new(
        message: &'a Message,
        theme: &'a Theme,
        is_selected: bool,
        width: u16,
        in_progress: bool,
        throbber_state: throbber_widgets_tui::ThrobberState,
    ) -> Self {
        let mut this = Self {
            theme,
            is_selected,
            in_progress,
            throbber_state,
            lines: Vec::new(),
        };
        this.process_message(message, width - 4);
        this
    }

    fn process_message(&mut self, message: &'a Message, width: u16) {
        let mut line = Line::default();
        match message {
            Message::User { content } => {
                line.spans.push(Span::styled(
                    "User",
                    Style::default().fg(Color::from_u32(0x4CAF50)),
                ));
                line.spans.push(Span::raw(": "));

                if let UserContent::Text(txt) = content.first() {
                    let parts = textwrap::wrap(
                        &txt.text,
                        textwrap::Options::new(width.into()).initial_indent("      "),
                    );
                    let first = parts.first().unwrap().to_string().trim_start().to_string();
                    line.spans.push(Span::raw(first));
                    for part in parts.iter().skip(1) {
                        self.lines.push(line);
                        line = Line::default();
                        line.spans.push(Span::raw(part.to_string()));
                    }
                }
                if let UserContent::ToolResult(tool_result) = content.first() {
                    let content = tool_result
                        .content
                        .into_iter()
                        .filter_map(|content| match content {
                            ToolResultContent::Text(txt) => Some(txt.text),
                            _ => None,
                        })
                        .join("\n");
                    let is_success = !content.contains("<error>");
                    line.spans.push(Span::raw("tool_result: "));
                    line.spans.push(Span::styled(
                        if is_success {
                            "SUCCESS ▶"
                        } else {
                            "ERROR ▶"
                        },
                        self.theme.tool_result_style(is_success),
                    ));
                }
            }
            Message::Assistant { content } => {
                line.spans.push(Span::styled(
                    "Assistant",
                    Style::default().fg(Color::from_u32(0x2196F3)),
                ));
                line.spans.push(Span::raw(": "));
                for item in content.iter() {
                    if let AssistantContent::Text(txt) = item {
                        let text = if let Some(start_idx) =
                            txt.text.find("<think>").or(txt.text.find("<thinking>"))
                        {
                            let end_idx =
                                txt.text.find("</think>").or(txt.text.find("</thinking>"));
                            let suffix = if end_idx.is_some() { "▶" } else { "⯆" };

                            line.spans.push(Span::styled(
                                format!("THINKING {} ", suffix),
                                self.theme.tool_call_style(),
                            ));
                            if let Some(end_idx) = end_idx {
                                let end_idx =
                                    txt.text[end_idx + 1..].find(">").unwrap() + end_idx + 2;
                                &txt.text[end_idx..]
                            } else {
                                &txt.text[txt.text[start_idx + 1..].find(">").unwrap() + 2..]
                            }
                        } else {
                            &txt.text
                        };
                        let parts = textwrap::wrap(
                            text,
                            textwrap::Options::new(width.into()).initial_indent("           "),
                        );
                        let first = parts.first().unwrap().to_string().trim_start().to_string();
                        line.spans.push(Span::raw(first));
                        for part in parts.iter().skip(1) {
                            self.lines.push(line);
                            line = Line::default();
                            line.spans.push(Span::raw(part.to_string()));
                        }
                    }
                    if let AssistantContent::ToolCall(tool_call) = item {
                        let args = tool_call.function.arguments.as_object().unwrap();
                        let tool_params = if args.contains_key("path") {
                            format!("path: {}", args.get("path").unwrap().as_str().unwrap())
                        } else if args.contains_key("result") {
                            format!("result: {}", args.get("result").unwrap())
                        } else if args.contains_key("command") {
                            format!("command: {}", args.get("command").unwrap())
                        } else if args.contains_key("question") {
                            format!("question: {}", args.get("question").unwrap())
                        } else {
                            args.iter()
                                .next()
                                .map(|(key, value)| format!("{}: {}", key, value))
                                .unwrap_or_default()
                        };
                        let tool_str = format!("{}({})", tool_call.function.name, tool_params);

                        let parts = textwrap::wrap(
                            &tool_str,
                            textwrap::Options::new(width.into()).initial_indent("           "),
                        );
                        let first = parts.first().unwrap().to_string().trim_start().to_string();
                        line.spans
                            .push(Span::styled(first, self.theme.tool_call_style()));
                        for part in parts.iter().skip(1) {
                            self.lines.push(line);
                            line = Line::default();
                            line.spans
                                .push(Span::styled(part.to_string(), self.theme.tool_call_style()));
                        }
                    }
                }
            }
        }
        if !line.spans.is_empty() {
            self.lines.push(line);
        }
    }

    pub fn main_axis_size(&self) -> u16 {
        self.lines.len() as u16
    }
}

impl Widget for MessageWidget<'_> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .padding(Padding::new(2, 1, 0, 0))
            .style(Style::default().bg(if self.is_selected {
                self.theme.background_highlight
            } else {
                self.theme.background
            }))
            .border_style(self.theme.border_style(false));
        Paragraph::new(self.lines).block(block).render(area, buf);
        if self.in_progress {
            let simple = throbber_widgets_tui::Throbber::default();
            StatefulWidget::render(simple, area, buf, &mut self.throbber_state);
        }
    }
}
