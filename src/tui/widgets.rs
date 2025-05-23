// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
pub mod filetree;
mod message;
mod shortcuts;
mod task_status;
mod terminal;
mod toolbar;

use crate::tui::App;
use ratatui::layout::{Margin, Offset};
use ratatui::prelude::StatefulWidget;
use ratatui::widgets::Widget;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation},
};
use shortcuts::ShortcutsWidget;
use toolbar::ToolbarWidget;
use tui_widget_list::{ListBuilder, ListView, ScrollAxis};

use self::filetree::FileTreeWidget;
use self::message::MessageWidget;
use self::task_status::TaskStatusWidget;
use self::terminal::TerminalWidget;

use super::app::FocusedComponent;

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

        // Task block
        let task_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Task name
                Constraint::Length(1), // Status of task
            ])
            .split(
                layout
                    .task_area
                    .inner(Margin::new(2, 0))
                    .offset(Offset { x: 0, y: 1 }),
            );

        let _task_status_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(10), // Status of task
                Constraint::Min(20),  // Task name
                Constraint::Min(10),  // Empty
            ])
            .split(task_layout[1]);

        Block::bordered()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .title(" Current Task ")
            .title_alignment(Alignment::Right)
            .title_style(theme.primary_style())
            .padding(Padding::horizontal(1))
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style(false))
            .render(layout.task_area, buf);

        Paragraph::new(self.current_task_text()).render(task_layout[0], buf);
        //        let progress_value = self.model.task_status.current_tokens as f64
        //            / f64::max(
        //                self.model.task_status.current_tokens as f64,
        //                f64::max(1.0, self.model.task_status.max_tokens as f64),
        //            );
        //
        //        LineGauge::default()
        //            .filled_style(Style::default().fg(Color::Blue))
        //            .unfilled_style(Style::default().fg(Color::DarkGray))
        //            .line_set(symbols::line::ROUNDED)
        //            .label(format!(
        //                "{}/{}",
        //                format_num!(".2s", self.model.task_status.current_tokens),
        //                format_num!(".2s", self.model.task_status.max_tokens)
        //            ))
        //            .ratio(progress_value)
        //            .render(task_status_layout[0], buf);
        //        Paragraph::new("API Cost: $1.7681")
        //            .right_aligned()
        //            .render(task_status_layout[1], buf);

        // Chat history
        let chat_block = Block::bordered()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .padding(Padding::bottom(1))
            .title(" History ")
            .title_alignment(Alignment::Right)
            .title_style(theme.primary_style())
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style(matches!(self.ui.focus, FocusedComponent::History)));

        let chat_len = self.model.messages.len();
        self.ui.history_scroll_state = self.ui.history_scroll_state.content_length(chat_len);
        let builder = ListBuilder::new(|context| {
            let item = MessageWidget::new(
                &self.model.messages[context.index],
                &theme,
                context.is_selected,
                self.ui.history_opened_state.contains(&context.index)
                    || context.index == self.model.messages.len() - 1,
                layout.chat_area.width,
            );
            let main_axis_size = item.main_axis_size();
            (item, main_axis_size)
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
            .scroll_padding(2);

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
            .set_placeholder_style(theme.inactive_style());
        self.ui
            .textarea
            .set_placeholder_text("Type your message here...");

        // Render the textarea
        self.ui.textarea.render(layout.input_area, buf);

        // File tree
        FileTreeWidget.render(layout.tree_area, buf, &mut self.ui.tree_state);

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
