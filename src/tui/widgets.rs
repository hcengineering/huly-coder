// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
pub mod filetree;
mod message;
mod shortcuts;
mod task_info;
mod task_status;
mod terminal;
mod toolbar;

use crate::agent::event::AgentState;
use crate::tui::widgets::message::create_messages;
use crate::tui::App;
use ratatui::prelude::StatefulWidget;
use ratatui::style::Stylize;
use ratatui::symbols;
use ratatui::text::Text;
use ratatui::widgets::Widget;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation},
};

use rig::tool::Tool;
use shortcuts::ShortcutsWidget;
use task_info::TaskInfoWidget;
use toolbar::ToolbarWidget;
use tui_widget_list::{ListBuilder, ListView, ScrollAxis};

use self::filetree::FileTreeWidget;
use self::message::MessageWidget;
use self::task_status::TaskStatusWidget;
use self::terminal::TerminalWidget;

use super::app::FocusedComponent;
use super::tool_info;

struct LayoutRects {
    toolbar_area: Rect,
    task_area: Rect,
    chat_area: Rect,
    error_area: Rect,
    status_area: Rect,
    input_area: Rect,
    tree_area: Rect,
    terminal_area: Rect,
    shortcuts_area: Rect,
}

fn build_layout(
    area: Rect,
    errors: &Option<String>,
    textarea: &tui_textarea::TextArea,
) -> LayoutRects {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Toolbar at top
            Constraint::Min(5),    // Main content
            Constraint::Length(2), // Status bar at bottom
        ])
        .split(area);
    let toolbar_area = main_layout[0];
    let content_area = main_layout[1];
    let shortcuts_area = main_layout[2];

    // Split main content into left and right panels
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(2, 3), // Left panel (chat + input)
            Constraint::Ratio(1, 3), // Right panel (file tree + terminal)
        ])
        .split(content_area);

    let left_area = content_layout[0];
    let right_area = content_layout[1];
    let error_lines_count = errors
        .as_ref()
        .map(|s| textwrap::wrap(s, left_area.width as usize - 5).len())
        .unwrap_or(0);

    let input_text_lines_count = textarea
        .lines()
        .iter()
        .map(|s| textwrap::wrap(s, left_area.width as usize - 5).len())
        .sum::<usize>() as u16;
    // Left panel (chat history + input)
    let (task_area, chat_area, error_area, status_area, input_area) = if error_lines_count > 0 {
        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),                                 // Task panel
                Constraint::Fill(3),                                   // Chat history
                Constraint::Max(10.min(error_lines_count as u16)),     // Error message
                Constraint::Length(1),                                 // Task progress status
                Constraint::Length(6.min(input_text_lines_count) + 1), // Input field
            ])
            .split(left_area);
        (rects[0], rects[1], rects[2], rects[3], rects[4])
    } else {
        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),                                 // Task panel
                Constraint::Min(3),                                    // Chat history
                Constraint::Length(1),                                 // Task progress status
                Constraint::Length(6.min(input_text_lines_count) + 1), // Input field
            ])
            .split(left_area);
        (rects[0], rects[1], Rect::default(), rects[2], rects[3])
    };

    // Right panel (file tree + terminal)
    let right_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 2), // File tree
            Constraint::Ratio(1, 2), // Terminal output
        ])
        .split(right_area);

    let tree_area = right_layout[0];
    let terminal_area = right_layout[1];

    LayoutRects {
        toolbar_area,
        task_area,
        chat_area,
        error_area,
        status_area,
        input_area,
        tree_area,
        terminal_area,
        shortcuts_area,
    }
}

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme;

        buf.set_style(area, Style::default().bg(theme.background).fg(theme.text));

        let layout = build_layout(area, &self.model.last_error, &self.ui.textarea);

        ToolbarWidget.render(layout.toolbar_area, buf, &theme, &self.config);

        // Task info block
        TaskInfoWidget.render(
            layout.task_area,
            buf,
            &theme,
            &self.model.agent_status,
            &self.current_task_text(),
        );

        // Chat history
        let chat_block = Block::bordered()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .padding(Padding::bottom(1))
            .title(" History ")
            .title_alignment(Alignment::Right)
            .title_style(theme.text_style())
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style(matches!(self.ui.focus, FocusedComponent::History)));

        let mut messages: Vec<MessageWidget> = Vec::new();
        let mut virt_idx = 0;
        for (idx, message) in self.model.messages.iter().enumerate() {
            for item in create_messages(
                message,
                &theme,
                layout.chat_area.width - 2,
                layout.chat_area.height as usize - 4,
                idx == self.model.messages.len() - 1
                    || self.ui.history_opened_state.contains(&virt_idx),
            ) {
                messages.push(item);
                virt_idx += 1;
            }
        }
        let chat_len = messages.len();

        if self.ui.history_follow_last && chat_len >= 1 {
            self.ui.history_follow_last = false;
            self.ui.history_state.select(Some(chat_len - 1));
        }

        if self
            .ui
            .history_state
            .selected
            .is_some_and(|idx| idx >= chat_len)
            && chat_len >= 1
        {
            self.ui.history_state.selected = Some(chat_len - 1);
        } else if chat_len == 0 {
            self.ui.history_state.selected = None;
        }

        let builder = ListBuilder::new(|context| {
            if let Some(mut item) = messages.get(context.index).cloned() {
                item.is_selected = context.is_selected;
                let main_axis_size = item.main_axis_size();
                (item, main_axis_size)
            } else {
                (MessageWidget::new(&theme, vec![], false), 0)
            }
        });
        let list = ListView::new(builder, chat_len)
            .block(chat_block)
            .scroll_axis(ScrollAxis::Vertical)
            .scrollbar(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .track_symbol(None)
                    .thumb_symbol("▐"),
            )
            .infinite_scrolling(false)
            .scroll_padding(0);

        list.render(layout.chat_area, buf, &mut self.ui.history_state);

        // Error message
        if let Some(error) = self.model.last_error.as_ref() {
            let error_block = Block::bordered()
                .borders(Borders::TOP | Borders::LEFT)
                .title(" Error ")
                .padding(Padding::horizontal(1))
                .title_alignment(Alignment::Right)
                .title_style(theme.error_style())
                .border_type(BorderType::Rounded)
                .border_style(theme.error_style());
            Paragraph::new(
                textwrap::wrap(error, layout.chat_area.width as usize - 5)
                    .iter()
                    .map(|s| Line::raw(s.to_string()))
                    .collect::<Vec<_>>(),
            )
            .block(error_block)
            .style(theme.error_style())
            .render(layout.error_area, buf);
        }

        // Task progress status
        TaskStatusWidget.render(
            layout.status_area,
            buf,
            &self.model.agent_status.state,
            &theme,
            &self.ui.throbber_state,
        );

        // Input field
        let input_block = Block::bordered()
            .borders(Borders::TOP | Borders::LEFT)
            .padding(Padding::horizontal(1))
            .title_alignment(Alignment::Right)
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style(matches!(self.ui.focus, FocusedComponent::Input)));

        // Create a TextArea with the App's input text
        self.ui.textarea.set_block(input_block);
        self.ui.textarea.set_style(theme.text_style());
        self.ui.textarea.set_wrap(true);
        self.ui.textarea.set_cursor_line_style(theme.text_style());
        self.ui
            .textarea
            .set_placeholder_style(Style::default().fg(theme.inactive_text));
        self.ui
            .textarea
            .set_placeholder_text("Type your message here...");

        // Render the textarea
        self.ui.textarea.render(layout.input_area, buf);

        // File tree
        FileTreeWidget.render(layout.tree_area, buf, &mut self.ui.tree_state, &theme);

        TerminalWidget.render(
            layout.terminal_area,
            buf,
            matches!(self.ui.focus, FocusedComponent::Terminal),
            &mut self.ui.terminal_state,
            &self.model.terminal_statuses,
            &theme,
            &self.ui.throbber_state,
        );

        // Status bar with shortcuts
        ShortcutsWidget.render(layout.shortcuts_area, buf, &theme);

        // render popup dialog if need tool confirmation
        if let AgentState::ToolCall(tool_call, true) = &self.model.agent_status.state {
            if tool_call.function.name
                != crate::tools::ask_followup_question::AskFollowupQuestionTool::NAME
            {
                let tool_name = tool_call.function.name.clone();
                let tool_args = tool_call.function.arguments.clone();
                let (icon, info) = tool_info::get_tool_call_info(&tool_name, &tool_args);
                let mut text = Text::default();
                text.push_line("");
                text.push_line("  Agent want execute tool:  ");
                text.push_line("");
                text.push_line(Line::default().spans(vec!["  ", &icon, &info, "   "]));
                text.push_line("");
                text.push_line(Line::styled(
                    "  Enter - Approve | Esc - Deny | a - Always Approve  ",
                    Style::default(),
                ));
                let popup = tui_popup::Popup::new(text)
                    .title(format!(" Confirm tool execution: {}", tool_name))
                    .style(Style::new().white().on_blue())
                    .border_set(symbols::border::ROUNDED);
                popup.render(area, buf);
            }
        }

        //#region: focus areas
        self.ui
            .widget_areas
            .insert(FocusedComponent::Input, layout.input_area);
        self.ui
            .widget_areas
            .insert(FocusedComponent::History, layout.chat_area);
        self.ui
            .widget_areas
            .insert(FocusedComponent::Tree, layout.tree_area);
        self.ui
            .widget_areas
            .insert(FocusedComponent::Terminal, layout.terminal_area);
        // #endregion
    }
}
