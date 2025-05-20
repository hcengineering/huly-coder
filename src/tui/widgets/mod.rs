// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
pub mod filetree;
mod message;
mod task_status;

use crate::tui::App;
use ratatui::layout::{Margin, Offset};
use ratatui::prelude::StatefulWidget;
use ratatui::widgets::{ScrollbarState, Widget};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation},
};
use tui_widget_list::{ListBuilder, ListView, ScrollAxis};

use self::filetree::FileTreeWidget;
use self::message::MessageWidget;
use self::task_status::TaskStatusWidget;

use super::app::FocusedComponent;

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let theme = self.theme;

        buf.set_style(area, Style::default().bg(theme.background).fg(theme.text));

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Toolbar at top
                Constraint::Min(5),    // Main content
                Constraint::Length(2), // Status bar at bottom
            ])
            .split(area);

        // #region: toolbar
        Block::bordered()
            .borders(Borders::BOTTOM)
            .border_type(BorderType::QuadrantOutside)
            .border_style(Style::default().fg(theme.background).bg(theme.panel_shadow))
            .style(theme.panel_style())
            .padding(Padding::horizontal(1))
            .render(main_layout[0], buf);

        let toolbar_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(15),  // Title
                Constraint::Ratio(2, 3), // Context length
                Constraint::Ratio(1, 3), // Model
            ])
            .split(main_layout[0]);

        Span::styled(" Huly Coder ", Style::default().fg(theme.focus))
            .render(toolbar_layout[0], buf);

        let toolbar_text = Line::from(vec![
            Span::styled(
                format!("{:?}", self.config.provider),
                Style::default().fg(theme.highlight),
            ),
            Span::styled(" | ", Style::default().fg(theme.focus)),
            Span::styled(&self.config.model, Style::default().fg(theme.text)),
            Span::styled(" ", Style::default().fg(theme.text)),
        ]);

        Paragraph::new(toolbar_text)
            .alignment(Alignment::Right)
            .render(toolbar_layout[2], buf);
        // #endregion

        // Split main content into left and right panels
        let content_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Ratio(2, 3), // Left panel (chat + input)
                Constraint::Ratio(1, 3), // Right panel (file tree + terminal)
            ])
            .split(main_layout[1]);

        // Left panel (chat history + input)
        let left_panel = if let Some(error_message) = &self.model.last_error {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4), // Task panel
                    Constraint::Fill(3),   // Chat history
                    Constraint::Max(u16::min(10, (error_message.lines().count() + 2) as u16)), // Error message
                    Constraint::Length(1), // Task progress status
                    Constraint::Length(3), // Input field
                ])
                .split(content_layout[0])
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4), // Task panel
                    Constraint::Min(3),    // Chat history
                    Constraint::Length(1), // Task progress status
                    Constraint::Length(3), // Input field
                ])
                .split(content_layout[0])
        };

        // Task block
        let task_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Task name
                Constraint::Length(1), // Status of task
            ])
            .split(
                left_panel[0]
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
            .render(left_panel[0], buf);

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
                left_panel[1].width,
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

        list.render(left_panel[1], buf, &mut self.ui.history_state);
        //render_scrollbar(left_panel[1], buf, &mut self.ui.history_scroll_state);
        self.ui
            .widget_areas
            .insert(FocusedComponent::History, left_panel[1]);

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
            Paragraph::new(error.clone())
                .block(error_block)
                .style(theme.error_style())
                .render(left_panel[2], buf);
        }
        // Task progress status
        TaskStatusWidget.render(
            left_panel[if self.model.last_error.is_some() {
                3
            } else {
                2
            }],
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
        self.ui
            .textarea
            .set_placeholder_style(theme.inactive_style());
        self.ui
            .textarea
            .set_placeholder_text("Type your message here...");

        // Render the textarea
        let input_layout_idx = if self.model.last_error.is_some() {
            4
        } else {
            3
        };
        self.ui.textarea.render(left_panel[input_layout_idx], buf);
        self.ui
            .widget_areas
            .insert(FocusedComponent::Input, left_panel[input_layout_idx]);

        // Right panel (file tree + terminal)
        let right_panel = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Ratio(1, 2), // File tree
                Constraint::Ratio(1, 2), // Terminal output
            ])
            .split(content_layout[1]);

        // File tree
        FileTreeWidget.render(right_panel[0], buf, &mut self.ui.tree_state);
        self.ui
            .widget_areas
            .insert(FocusedComponent::Tree, right_panel[0]);

        // Terminal output
        let terminal_block = Block::bordered()
            .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
            .padding(Padding::horizontal(1))
            .title(" Terminal ")
            .title_alignment(Alignment::Right)
            .title_style(theme.primary_style())
            .border_type(BorderType::Rounded)
            .border_style(theme.border_style(matches!(self.ui.focus, FocusedComponent::Terminal)));

        let mut terminal_lines = vec![];
        if !self.model.execute_command.command.is_empty() {
            terminal_lines.push(Line::styled(
                format!("> {}", self.model.execute_command.command),
                theme.primary_style(),
            ));
        }
        if !self.model.execute_command.output.is_empty() {
            let output = self.model.execute_command.output.replace("\\n", "\n");
            output.lines().for_each(|line| {
                terminal_lines.push(Line::styled(line.to_string(), theme.inactive_text_style()))
            });
        }

        self.ui.terminal_scroll_state = self
            .ui
            .terminal_scroll_state
            .content_length(terminal_lines.len())
            .position(self.ui.terminal_scroll_position.into());
        render_scrollbar(right_panel[1], buf, &mut self.ui.terminal_scroll_state);
        Paragraph::new(terminal_lines)
            .block(terminal_block)
            .style(theme.text_style())
            .scroll((self.ui.terminal_scroll_position, 0))
            .render(right_panel[1], buf);
        self.ui
            .widget_areas
            .insert(FocusedComponent::Terminal, right_panel[1]);

        // Status bar with shortcuts
        let status_block = Block::bordered()
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
            .block(status_block)
            .alignment(Alignment::Right)
            .render(main_layout[2], buf);

        //        let popup = Popup::new("Press any key to exit")
        //            .title("tui-popup demo")
        //            .border_set(symbols::border::ROUNDED)
        //            .border_style(Style::new().bold())
        //            .style(Style::new().white().on_blue());
        //        popup.render(area, buf);
    }
}

fn render_scrollbar(area: Rect, buf: &mut Buffer, state: &mut ScrollbarState) {
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(None)
        .thumb_symbol("▐");
    scrollbar.render(
        area.inner(Margin {
            vertical: 1,
            horizontal: 1,
        }),
        buf,
        state,
    );
}
