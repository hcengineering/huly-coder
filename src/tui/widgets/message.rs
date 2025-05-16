// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use itertools::Itertools;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, StatefulWidget, Widget};
use rig::message::{AssistantContent, Message, ToolResultContent, UserContent};
use rig::tool::Tool;

use crate::tools::attempt_completion::AttemptCompletionTool;
use crate::tui::{ratskin, Theme};

#[derive(Debug, Clone)]
pub struct MessageWidget<'a> {
    theme: &'a Theme,
    lines: Vec<Line<'a>>,
    is_complete: bool,
    in_progress: bool,
    is_opened: bool,
    throbber_state: throbber_widgets_tui::ThrobberState,
    pub is_selected: bool,
}

impl<'a> MessageWidget<'a> {
    pub fn new(
        message: &'a Message,
        theme: &'a Theme,
        is_selected: bool,
        is_opened: bool,
        width: u16,
        in_progress: bool,
        throbber_state: throbber_widgets_tui::ThrobberState,
    ) -> Self {
        let mut this = Self {
            theme,
            is_selected,
            is_opened,
            in_progress,
            is_complete: false,
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
                    let suffix = if self.is_opened { "⯆" } else { "▶" };
                    line.spans.push(Span::raw("tool_result: "));
                    line.spans.push(Span::styled(
                        format!(
                            "{} {}",
                            if is_success { "SUCCESS" } else { "ERROR" },
                            suffix
                        ),
                        self.theme.tool_result_style(is_success),
                    ));
                    if self.is_opened {
                        self.lines.push(line);
                        line = Line::default();
                        let parts = textwrap::wrap(&content, textwrap::Options::new(width.into()));
                        for part in parts.iter() {
                            self.lines.push(Line::raw(part.to_string()));
                        }
                    }
                }
            }
            Message::Assistant { content } => {
                for item in content.iter() {
                    if let AssistantContent::Text(txt) = item {
                        line.spans.push(Span::styled(
                            "Assistant",
                            Style::default().fg(self.theme.assistant),
                        ));
                        line.spans.push(Span::raw(": "));
                        let text = if let Some(start_idx) =
                            txt.text.find("<think>").or(txt.text.find("<thinking>"))
                        {
                            let end_idx =
                                txt.text.find("</think>").or(txt.text.find("</thinking>"));
                            let suffix = if end_idx.is_some() && !self.is_opened {
                                "▶"
                            } else {
                                "⯆"
                            };

                            line.spans.push(Span::styled(
                                format!("THINKING {} ", suffix),
                                self.theme.tool_call_style(),
                            ));
                            if end_idx.is_some_and(|_| !self.is_opened) {
                                let end_idx = end_idx.unwrap();
                                let end_idx =
                                    txt.text[end_idx + 1..].find(">").unwrap() + end_idx + 2;
                                &txt.text[end_idx..]
                            } else {
                                &txt.text[txt.text[start_idx + 1..].find(">").unwrap() + 2..]
                            }
                        } else {
                            &txt.text
                        };
                        self.lines.push(line);
                        line = Line::default();
                        let ratskin = ratskin::RatSkin::default();
                        self.lines
                            .append(&mut ratskin.parse_text(text.trim_end(), width));
                        //                        let parts = textwrap::wrap(
                        //                            text.trim_end(),
                        //                            textwrap::Options::new(width.into()).initial_indent("           "),
                        //                        );
                        //                        let first = parts.first().unwrap().to_string().trim_start().to_string();
                        //                        line.spans.push(Span::raw(first));
                        //                        for part in parts.iter().skip(1) {
                        //                            self.lines.push(line);
                        //                            line = Line::default();
                        //                            line.spans.push(Span::raw(part.to_string()));
                        //                        }
                    }
                    if let AssistantContent::ToolCall(tool_call) = item {
                        self.is_complete = tool_call.function.name == AttemptCompletionTool::NAME;
                        let args = tool_call.function.arguments.as_object();
                        if self.is_complete {
                            let result = args
                                .unwrap()
                                .get("result")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            let parts =
                                textwrap::wrap(result, textwrap::Options::new(width.into()));
                            for part in parts.iter() {
                                self.lines.push(Line::styled(
                                    part.to_string(),
                                    Style::default().fg(self.theme.assistant),
                                ));
                            }
                        } else {
                            line.spans.push(Span::styled(
                                "Assistant",
                                Style::default().fg(self.theme.assistant),
                            ));
                            line.spans.push(Span::raw(": "));

                            let tool_str = if let Some(args) = args {
                                let tool_params = if args.contains_key("path") {
                                    format!("path: {}", args.get("path").unwrap().as_str().unwrap())
                                } else if args.contains_key("result") {
                                    format!("result: {}", args.get("result").unwrap())
                                } else if args.contains_key("command") {
                                    format!("command: {}", args.get("command").unwrap())
                                } else if args.contains_key("question") {
                                    format!("question: {}", args.get("question").unwrap())
                                } else if args.contains_key("query") {
                                    format!("query: {}", args.get("query").unwrap())
                                } else if args.contains_key("url") {
                                    format!("url: {}", args.get("url").unwrap())
                                } else {
                                    args.iter()
                                        .next()
                                        .map(|(key, value)| format!("{}: {}", key, value))
                                        .unwrap_or_default()
                                };
                                format!("{}({})", tool_call.function.name, tool_params)
                            } else {
                                tool_call.function.name.to_string()
                            };

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
                                line.spans.push(Span::styled(
                                    part.to_string(),
                                    self.theme.tool_call_style(),
                                ));
                            }
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
        if self.is_complete {
            self.lines.len() as u16 + 2
        } else {
            self.lines.len() as u16
        }
    }
}

impl Widget for MessageWidget<'_> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let mut block = Block::new();
        if self.is_complete {
            block = block
                .borders(Borders::TOP | Borders::BOTTOM)
                .title_top(Line::styled("──", self.theme.border_style(false)))
                .title_top(Line::from("Task Complete").left_aligned())
                .title_style(Style::default().fg(self.theme.assistant));
        }
        block = block
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
