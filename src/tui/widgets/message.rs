// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use itertools::Itertools;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Widget};
use rig::message::{AssistantContent, Message, ToolResultContent, UserContent};
use rig::tool::Tool;

use crate::tools::ask_followup_question::AskFollowupQuestionTool;
use crate::tools::attempt_completion::AttemptCompletionTool;
use crate::tui::{ratskin, split_think_tags, tool_info, Theme};

#[derive(Debug, Clone)]
pub struct MessageWidget<'a> {
    theme: &'a Theme,
    lines: Vec<Line<'a>>,
    is_complete: bool,
    is_opened: bool,
    pub is_selected: bool,
}

fn role_prefix(role: &str, color: Color) -> Line<'_> {
    Line::default().spans(vec![
        Span::styled(role, Style::default().fg(color)),
        Span::raw(": "),
    ])
}

impl<'a> MessageWidget<'a> {
    pub fn new(
        message: &'a Message,
        theme: &'a Theme,
        is_selected: bool,
        is_opened: bool,
        width: u16,
    ) -> Self {
        let mut this = Self {
            theme,
            is_selected,
            is_opened,
            is_complete: false,
            lines: Vec::new(),
        };
        this.process_message(message, width - 4);
        this
    }

    fn format_text_wrapped(&mut self, text: &str, mut first_line: Line<'a>, width: usize) {
        let parts = textwrap::wrap(
            text,
            textwrap::Options::new(width).initial_indent(&(" ".repeat(first_line.width()))),
        );
        let first = parts.first().unwrap().to_string().trim_start().to_string();
        first_line.spans.push(Span::raw(first));
        self.lines.push(first_line);
        for part in parts.iter().skip(1) {
            self.lines
                .push(Line::default().spans(vec![Span::raw(part.to_string())]));
        }
    }

    fn format_think_block<'b>(
        text: &str,
        ratskin: &'b ratskin::RatSkin,
        width: u16,
    ) -> Vec<Line<'a>> {
        let mut lines = vec![];
        lines.push(Line::styled(
            format!(" ╭{}", "─".repeat(width as usize - 4)),
            Style::default().fg(Color::Indexed(60)),
        ));
        for mut part in ratskin.parse_text(text.trim(), width - 4) {
            part.spans.insert(
                0,
                Span::styled(" │", Style::default().fg(Color::Indexed(60))),
            );
            lines.push(part);
        }
        lines.push(Line::styled(
            format!(" ╰{}", "─".repeat(width as usize - 4)),
            Style::default().fg(Color::Indexed(60)),
        ));
        lines
    }

    fn process_message(&mut self, message: &'a Message, width: u16) {
        let ratskin = ratskin::RatSkin::default();
        let open_suffix = if !self.is_opened { "▶" } else { "⯆" };

        match message {
            Message::User { content } => {
                if let UserContent::Text(txt) = content.first() {
                    let line = role_prefix("User", self.theme.user);
                    self.format_text_wrapped(&txt.text, line, width.into());
                }
                if let UserContent::ToolResult(tool_result) = content.first() {
                    let mut line = role_prefix("User", self.theme.user);
                    let content = tool_result
                        .content
                        .into_iter()
                        .filter_map(|content| match content {
                            ToolResultContent::Text(txt) => {
                                let text = serde_json::from_str::<serde_json::Value>(&txt.text)
                                    .as_ref()
                                    .map(|v| v.as_str().unwrap_or(&txt.text).to_string())
                                    .unwrap_or(txt.text)
                                    .trim()
                                    .to_string();
                                Some(text)
                            }
                            _ => None,
                        })
                        .join("\n");
                    let is_success = !content.contains("<error>");
                    // draw sigle line without folding
                    if content.lines().count() == 1 && content.len() < width as usize {
                        line.spans.push(Span::raw("tool_result: "));
                        line.spans.push(Span::styled(
                            content,
                            self.theme.tool_result_style(is_success),
                        ));
                        self.lines.push(line);
                    } else {
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
                        self.lines.push(line);
                        if self.is_opened {
                            // TODO: code highlight
                            self.lines.extend(
                                ratskin.parse_text(&format!("```\n{}\n```", content), width),
                            );
                        }
                    }
                }
            }
            Message::Assistant { content } => {
                for item in content.iter() {
                    if let AssistantContent::Text(txt) = item {
                        for (idx, (text, is_think_block)) in
                            split_think_tags(&txt.text).into_iter().enumerate()
                        {
                            if idx == 0 {
                                if is_think_block {
                                    let mut line = role_prefix("Assistant", self.theme.assistant);
                                    line.spans.push(Span::styled(
                                        format!("THINKING {} ", &open_suffix),
                                        self.theme.tool_call_style(),
                                    ));
                                    self.lines.push(line);
                                    if self.is_opened {
                                        self.lines.append(&mut Self::format_think_block(
                                            &text, &ratskin, width,
                                        ));
                                    }
                                } else {
                                    let mut line = role_prefix("Assistant", self.theme.assistant);
                                    let parts =
                                        // add space prefix to correct first line with wrapping
                                        ratskin.parse_text(&format!("           {}", &text), width);
                                    if let Some(first_line) = parts.first() {
                                        let mut spans = first_line
                                            .spans
                                            .clone()
                                            .into_iter()
                                            .enumerate()
                                            .map(|(i, s)| {
                                                if i == 0 {
                                                    s.clone()
                                                        .style(self.theme.text_style())
                                                        .content(s.content.trim().to_string())
                                                } else {
                                                    s.style(self.theme.text_style())
                                                }
                                            })
                                            .collect::<Vec<_>>();
                                        line.spans.append(&mut spans);
                                    }
                                    self.lines.push(line);
                                    self.lines.extend(parts.into_iter().skip(1));
                                }
                            } else if is_think_block {
                                if self.is_opened {
                                    self.lines.append(&mut Self::format_think_block(
                                        &text, &ratskin, width,
                                    ));
                                } else {
                                    self.lines.push(Line::styled(
                                        format!("THINKING {} ", &open_suffix),
                                        self.theme.tool_call_style(),
                                    ));
                                }
                            } else {
                                self.lines.append(&mut ratskin.parse_text(&text, width));
                            }
                        }
                    }
                    if let AssistantContent::ToolCall(tool_call) = item {
                        self.is_complete = tool_call.function.name == AttemptCompletionTool::NAME;
                        let is_ask_question =
                            tool_call.function.name == AskFollowupQuestionTool::NAME;
                        let args = tool_call.function.arguments.as_object();
                        if self.is_complete {
                            let result = args
                                .unwrap()
                                .get("result")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            let command = args
                                .unwrap()
                                .get("command")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            let mut ratskin = ratskin::RatSkin::default();
                            if let Color::Indexed(color_idx) = self.theme.assistant {
                                ratskin
                                    .skin
                                    .set_fg(crossterm::style::Color::AnsiValue(color_idx));
                            }
                            self.lines.append(&mut ratskin.parse_text(result, width));
                            if !command.is_empty() {
                                self.lines.append(
                                    &mut ratskin
                                        .parse_text(&format!("\nCommand: '{}'", command), width),
                                );
                            }
                        } else if is_ask_question {
                            let mut line = role_prefix("Assistant", self.theme.assistant);
                            line.spans.push(Span::raw("Ask followup question"));
                            self.lines.push(line);
                            let question = args
                                .unwrap()
                                .get("question")
                                .and_then(|v| v.as_str())
                                .unwrap_or_default();
                            self.lines.append(&mut ratskin.parse_text(question, width));
                            if let Some(options) =
                                args.unwrap().get("options").and_then(|v| v.as_array())
                            {
                                for item in options {
                                    self.lines.append(&mut ratskin.parse_text(
                                        &format!("  - {}", item.as_str().unwrap_or_default()),
                                        width,
                                    ));
                                }
                            }
                        } else {
                            let mut line = role_prefix("Assistant", self.theme.assistant);

                            let (tool_icon, tool_info) = tool_info::get_tool_call_info(
                                &tool_call.function.name,
                                &tool_call.function.arguments,
                            );
                            line.spans.push(Span::raw(tool_icon));
                            line.spans.push(Span::raw(" "));
                            line.spans.push(Span::raw(tool_info));
                            line.spans.push(Span::raw(open_suffix));
                            self.lines.push(line);

                            if self.is_opened {
                                let args = serde_yaml::to_string(&tool_call.function.arguments)
                                    .unwrap_or_default();
                                textwrap::wrap(&args, textwrap::Options::new(width.into()))
                                    .iter()
                                    .for_each(|line| self.lines.push(Line::raw(line.to_string())));
                            }
                        }
                    }
                }
            }
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
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut block = Block::new();
        if self.is_complete {
            block = block
                .borders(Borders::TOP | Borders::BOTTOM)
                .title_top(Line::styled("──", self.theme.border_style(false)))
                .title_top(Line::from("Task Complete").left_aligned())
                .title_style(Style::default().fg(self.theme.assistant));
        }
        block = block
            .padding(Padding::new(1, 1, 0, 0))
            .style(Style::default().bg(if self.is_selected {
                self.theme.background_highlight
            } else {
                self.theme.background
            }))
            .border_style(self.theme.border_style(false));
        Paragraph::new(self.lines).block(block).render(area, buf);
    }
}
