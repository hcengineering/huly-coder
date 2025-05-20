// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::agent::event::{AgentCommandStatus, AgentState, AgentStatus};
use crate::config::Config;

use crate::{
    agent::{self, AgentControlEvent, AgentOutputEvent},
    tui::{
        event::{AppEvent, UiEvent, UiEventMultiplexer},
        Theme,
    },
};
use crossterm::event::KeyEventKind;
use ratatui::layout::Position;
use ratatui::prelude::Rect;
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    widgets::ScrollbarState,
    DefaultTerminal,
};
use rig::message::{Message, UserContent};
use tokio::sync::mpsc;
use tui_textarea::TextArea;
use tui_widget_list::ListState;

use super::filetree::FileTreeState;

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum FocusedComponent {
    /// Input text area field
    Input,
    /// History component
    History,
    /// File tree component
    Tree,
    /// Terminal output
    Terminal,
}

impl From<u8> for FocusedComponent {
    fn from(idx: u8) -> Self {
        match idx {
            0 => FocusedComponent::Input,
            1 => FocusedComponent::History,
            2 => FocusedComponent::Tree,
            3 => FocusedComponent::Terminal,
            _ => FocusedComponent::Input,
        }
    }
}

#[derive(Debug, Default)]
pub struct ModelState {
    pub messages: Vec<Message>,
    pub agent_status: AgentStatus,
    pub execute_command: AgentCommandStatus,
    pub last_error: Option<String>,
}

#[derive(Debug)]
pub struct UiState<'a> {
    pub textarea: TextArea<'a>,
    pub focus: FocusedComponent,
    pub terminal_scroll_state: ScrollbarState,
    pub terminal_scroll_position: u16,
    pub tree_state: FileTreeState,
    pub history_scroll_state: ScrollbarState,
    pub history_state: ListState,
    pub history_opened_state: HashSet<usize>,
    pub throbber_state: throbber_widgets_tui::ThrobberState,
    pub widget_areas: HashMap<FocusedComponent, Rect>,
}

#[derive(Debug)]
pub struct App<'a> {
    pub config: Config,
    pub running: bool,
    pub events: UiEventMultiplexer,
    pub agent_sender: mpsc::UnboundedSender<agent::AgentControlEvent>,
    pub theme: Theme,
    pub model: ModelState,
    pub ui: UiState<'a>,
}

impl UiState<'_> {
    fn new(workspace: PathBuf) -> Self {
        Self {
            textarea: TextArea::default(),
            focus: FocusedComponent::Input,
            history_scroll_state: ScrollbarState::default(),
            terminal_scroll_state: ScrollbarState::default(),
            terminal_scroll_position: 0,
            tree_state: FileTreeState::new(workspace),
            history_state: ListState::default(),
            history_opened_state: HashSet::default(),
            throbber_state: throbber_widgets_tui::ThrobberState::default(),
            widget_areas: HashMap::default(),
        }
    }
}

impl App<'_> {
    pub fn new(
        config: Config,
        sender: mpsc::UnboundedSender<agent::AgentControlEvent>,
        receiver: mpsc::UnboundedReceiver<agent::AgentOutputEvent>,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            ui: UiState::new(config.workspace.clone()),
            config,
            running: true,
            events: UiEventMultiplexer::new(receiver),
            agent_sender: sender,
            theme: Theme::default(),
            model: ModelState {
                messages,
                ..Default::default()
            },
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        if !self.model.messages.is_empty() {
            self.ui
                .history_state
                .select(Some(self.model.messages.len() - 1));
        }
        while self.running {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;

            match self.events.next().await? {
                UiEvent::Tick => self.tick(),
                UiEvent::Crossterm(event) => match event {
                    crossterm::event::Event::Key(key_event) => {
                        if !self.handle_global_key_events(key_event)? {
                            match self.ui.focus {
                                FocusedComponent::Input => {
                                    if Self::handle_text_input(&mut self.ui.textarea, &event)
                                        && !self.ui.textarea.is_empty()
                                    {
                                        self.ui.textarea.select_all();
                                        self.ui.textarea.cut();
                                        self.model.last_error = None;
                                        self.agent_sender
                                            .send(agent::AgentControlEvent::SendMessage(
                                                self.ui.textarea.yank_text(),
                                            ))
                                            .unwrap();
                                    }
                                }
                                FocusedComponent::Tree => {
                                    Self::handle_tree_input(&mut self.ui.tree_state, &event);
                                }
                                FocusedComponent::History => {
                                    Self::handle_list_input(
                                        &mut self.ui.history_state,
                                        &mut self.ui.history_opened_state,
                                        &event,
                                    );
                                }
                                FocusedComponent::Terminal => {
                                    Self::handle_scroll_input(
                                        &mut self.ui.terminal_scroll_position,
                                        &event,
                                    );
                                }
                            }
                        }
                    }
                    crossterm::event::Event::Mouse(mouse_event) => {
                        if matches!(mouse_event.kind, crossterm::event::MouseEventKind::Down(_)) {
                            let focus = self.ui.widget_areas.iter().find_map(|(k, v)| {
                                if v.contains(Position {
                                    x: mouse_event.column,
                                    y: mouse_event.row,
                                }) {
                                    Some(k)
                                } else {
                                    None
                                }
                            });
                            if let Some(focus) = focus {
                                self.ui.focus = focus.clone();
                                self.ui.tree_state.focused =
                                    matches!(self.ui.focus, FocusedComponent::Tree);
                            }
                            match self.ui.focus {
                                FocusedComponent::Input => {
                                    Self::handle_text_input(&mut self.ui.textarea, &event);
                                }
                                FocusedComponent::Tree => {
                                    Self::handle_tree_input(&mut self.ui.tree_state, &event);
                                }
                                FocusedComponent::History => {
                                    Self::handle_list_input(
                                        &mut self.ui.history_state,
                                        &mut self.ui.history_opened_state,
                                        &event,
                                    );
                                }
                                FocusedComponent::Terminal => {
                                    Self::handle_scroll_input(
                                        &mut self.ui.terminal_scroll_position,
                                        &event,
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                },
                UiEvent::App(app_event) => match app_event {
                    AppEvent::Quit => self.quit(),
                    AppEvent::Agent(evt) => match evt {
                        AgentOutputEvent::NewTask => {
                            self.model.messages.clear();
                            self.ui.history_state.select(None);
                            self.ui.history_opened_state.clear();
                            self.ui.focus = FocusedComponent::Input;
                        }
                        AgentOutputEvent::AddMessage(message) => {
                            self.model.messages.push(message);
                            self.ui
                                .history_state
                                .select(Some(self.model.messages.len() - 1));
                        }
                        AgentOutputEvent::UpdateMessage(message) => {
                            if !self.model.messages.is_empty() {
                                let len = self.model.messages.len() - 1;
                                self.model.messages[len] = message;
                                self.ui.history_state.select(Some(len));
                            }
                        }
                        AgentOutputEvent::ExecuteCommand(status) => {
                            self.model.execute_command = status;
                        }
                        AgentOutputEvent::AgentStatus(status) => {
                            tracing::info!("agent_status: {:?}", status);
                            // if the task is no longer processing, focus input
                            if !self.model.agent_status.state.is_paused()
                                && status.state.is_paused()
                            {
                                self.ui.focus = FocusedComponent::Input;
                            }
                            self.model.agent_status = status;
                            if let AgentState::Error(msg) = &self.model.agent_status.state {
                                self.model.last_error = Some(msg.clone());
                            }
                        }
                        AgentOutputEvent::HighlightFile(path, is_new) => {
                            if is_new {
                                self.ui.tree_state.update_items();
                            }
                            self.ui.tree_state.highlight_file(path);
                        }
                    },
                },
            }
        }
        Ok(())
    }

    fn handle_text_input(text_area: &mut TextArea, event: &crossterm::event::Event) -> bool {
        if let crossterm::event::Event::Key(key_event) = event {
            if key_event.kind == KeyEventKind::Press
                && key_event.code == KeyCode::Enter
                && key_event.modifiers != KeyModifiers::SHIFT
            {
                return true;
            }
        }
        text_area.input(event.clone());
        false
    }

    fn handle_list_input(
        state: &mut ListState,
        opened_state: &mut HashSet<usize>,
        event: &crossterm::event::Event,
    ) {
        if let crossterm::event::Event::Key(key_event) = event {
            if key_event.kind == KeyEventKind::Press {
                match key_event.code {
                    KeyCode::Down => {
                        state.next();
                    }
                    KeyCode::Up => {
                        state.previous();
                    }
                    KeyCode::Enter => {
                        if let Some(selected) = state.selected {
                            if !opened_state.remove(&selected) {
                                opened_state.insert(selected);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_scroll_input(scroll_position: &mut u16, event: &crossterm::event::Event) {
        if let crossterm::event::Event::Key(key_event) = event {
            if key_event.kind == KeyEventKind::Press {
                match key_event.code {
                    KeyCode::Down => {
                        *scroll_position = scroll_position.saturating_add(1);
                    }
                    KeyCode::Up => {
                        *scroll_position = scroll_position.saturating_sub(1);
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_tree_input(state: &mut FileTreeState, event: &crossterm::event::Event) {
        if let crossterm::event::Event::Key(key_event) = event {
            if key_event.kind == KeyEventKind::Press {
                state.highlighted = false;
                match key_event.code {
                    KeyCode::Down => {
                        state.tree_state.key_down();
                    }
                    KeyCode::Up => {
                        state.tree_state.key_up();
                    }
                    KeyCode::Right => {
                        state.tree_state.key_right();
                    }
                    KeyCode::Left => {
                        state.tree_state.key_left();
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn handle_global_key_events(&mut self, key_event: KeyEvent) -> color_eyre::Result<bool> {
        if key_event.kind != KeyEventKind::Press {
            return Ok(false);
        }

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Char('c')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Char('n') | KeyCode::Char('N')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                self.agent_sender.send(AgentControlEvent::NewTask).unwrap()
            }
            KeyCode::Char('p') if key_event.modifiers == KeyModifiers::CONTROL => self
                .agent_sender
                .send(AgentControlEvent::CancelTask)
                .unwrap(),
            KeyCode::BackTab => {
                let mut focus = self.ui.focus.clone() as u8;
                if focus == 0 {
                    focus = FocusedComponent::Terminal as u8;
                } else {
                    focus -= 1;
                }
                self.ui.focus = focus.into();
            }
            KeyCode::Tab => {
                self.ui.focus = (self.ui.focus.clone() as u8 + 1u8).into();
            }
            KeyCode::Char('1') | KeyCode::Char('2') | KeyCode::Char('3') | KeyCode::Char('4')
                if key_event.modifiers == KeyModifiers::ALT =>
            {
                match key_event.code {
                    KeyCode::Char('1') => self.ui.focus = FocusedComponent::Input,
                    KeyCode::Char('2') => self.ui.focus = FocusedComponent::History,
                    KeyCode::Char('3') => self.ui.focus = FocusedComponent::Tree,
                    KeyCode::Char('4') => self.ui.focus = FocusedComponent::Terminal,
                    _ => {}
                };
            }
            _ => {
                return Ok(false);
            }
        }
        self.ui.tree_state.focused = matches!(self.ui.focus, FocusedComponent::Tree);
        Ok(true)
    }

    pub fn current_task_text(&self) -> String {
        if let Some(Message::User { content }) = self.model.messages.first() {
            if let UserContent::Text(txt) = content.first() {
                return txt.text;
            }
        }
        "".to_string()
    }

    pub fn tick(&mut self) {
        self.ui.throbber_state.calc_next();
    }

    pub fn quit(&mut self) {
        self.running = false;
    }
}
