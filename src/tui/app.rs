// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::vec;

use crate::agent::event::{AgentCommandStatus, AgentState};
use crate::config::Config;

use crate::providers::model_info::ModelInfo;
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
pub struct AgentStatus {
    pub current_input_tokens: u32,
    pub current_completion_tokens: u32,
    pub max_tokens: u32,
    pub input_price: f64,
    pub completion_price: f64,
    pub state: AgentState,
}

#[derive(Debug, Default)]
pub struct ModelState {
    pub messages: Vec<Message>,
    pub agent_status: AgentStatus,
    pub terminal_statuses: Vec<AgentCommandStatus>,
    pub last_error: Option<String>,
}

#[derive(Debug, Default)]
pub struct TerminalState {
    pub selected_idx: usize,
    pub scroll_state: ScrollbarState,
    pub scroll_position: u16,
}

#[derive(Debug)]
pub struct UiState<'a> {
    pub textarea: TextArea<'a>,
    pub focus: FocusedComponent,
    pub tree_state: FileTreeState,
    pub history_state: ListState,
    pub history_opened_state: HashSet<usize>,
    pub throbber_state: throbber_widgets_tui::ThrobberState,
    pub widget_areas: HashMap<FocusedComponent, Rect>,
    pub terminal_state: TerminalState,
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
            tree_state: FileTreeState::new(workspace),
            history_state: ListState::default(),
            history_opened_state: HashSet::default(),
            throbber_state: throbber_widgets_tui::ThrobberState::default(),
            widget_areas: HashMap::default(),
            terminal_state: TerminalState::default(),
        }
    }
}

impl ModelState {
    pub fn new(messages: Vec<Message>, model_info: ModelInfo) -> Self {
        let mut model_state = ModelState {
            messages,
            ..Default::default()
        };
        model_state.agent_status.input_price = model_info.input_price;
        model_state.agent_status.completion_price = model_info.completion_price;
        model_state.agent_status.max_tokens = model_info.max_tokens;
        model_state
    }
}

impl App<'_> {
    pub fn new(
        config: Config,
        model_info: ModelInfo,
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
            model: ModelState::new(messages, model_info),
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
                                    Self::handle_terminal_input(
                                        &mut self.ui.terminal_state,
                                        &event,
                                    );
                                    if key_event.kind == KeyEventKind::Press {
                                        let input_data = match key_event.code {
                                            KeyCode::Char(ch) => {
                                                if ch == 'c'
                                                    && key_event.modifiers == KeyModifiers::CONTROL
                                                {
                                                    vec![3]
                                                } else {
                                                    vec![ch as u8]
                                                }
                                            }
                                            KeyCode::Enter => {
                                                vec![b'\n']
                                            }
                                            KeyCode::Down
                                                if key_event.modifiers == KeyModifiers::ALT =>
                                            {
                                                vec![b'\x1b', b'[', b'B']
                                            }
                                            KeyCode::Up
                                                if key_event.modifiers == KeyModifiers::ALT =>
                                            {
                                                vec![b'\x1b', b'[', b'A']
                                            }
                                            _ => {
                                                vec![]
                                            }
                                        };
                                        if !input_data.is_empty() {
                                            tracing::trace!(
                                                "Sending data to terminal: {} {:?}",
                                                self.ui.terminal_state.selected_idx,
                                                input_data
                                            );
                                            self.agent_sender
                                                .send(AgentControlEvent::TerminalData(
                                                    self.ui.terminal_state.selected_idx + 1,
                                                    input_data,
                                                ))
                                                .unwrap()
                                        }
                                    }
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
                                    Self::handle_terminal_input(
                                        &mut self.ui.terminal_state,
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
                        AgentOutputEvent::CommandStatus(status) => {
                            for state in status {
                                if let Some(st) = self
                                    .model
                                    .terminal_statuses
                                    .iter_mut()
                                    .find(|t| t.command_id == state.command_id)
                                {
                                    st.is_active = state.is_active;
                                    st.output = state.output;
                                } else {
                                    self.model.terminal_statuses.push(state);
                                    self.ui.terminal_state.scroll_position = 0;
                                    self.ui.terminal_state.selected_idx =
                                        self.model.terminal_statuses.len() - 1;
                                }
                            }
                        }
                        AgentOutputEvent::AgentStatus(
                            current_input_tokens,
                            current_completion_tokens,
                            state,
                        ) => {
                            tracing::info!("agent_state: {}", state);
                            // if the task is no longer processing, focus input
                            if !self.model.agent_status.state.is_paused() && state.is_paused() {
                                self.ui.focus = FocusedComponent::Input;
                            }
                            self.model.agent_status.state = state;
                            self.model.agent_status.current_input_tokens = current_input_tokens;
                            self.model.agent_status.current_completion_tokens =
                                current_completion_tokens;
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

    fn handle_terminal_input(terminal_state: &mut TerminalState, event: &crossterm::event::Event) {
        if let crossterm::event::Event::Key(key_event) = event {
            if key_event.kind == KeyEventKind::Press {
                match key_event.code {
                    KeyCode::Char(ch) if ch.is_ascii_digit() => {
                        let idx = ch.to_digit(10).unwrap() as usize;
                        terminal_state.selected_idx = idx - 1;
                        terminal_state.scroll_position = 0;
                    }
                    KeyCode::Down if key_event.modifiers != KeyModifiers::ALT => {
                        terminal_state.scroll_position =
                            terminal_state.scroll_position.saturating_add(1);
                    }
                    KeyCode::Up if key_event.modifiers != KeyModifiers::ALT => {
                        terminal_state.scroll_position =
                            terminal_state.scroll_position.saturating_sub(1);
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
            KeyCode::Char('q') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(AppEvent::Quit)
            }
            KeyCode::Char('n') | KeyCode::Char('N')
                if key_event.modifiers == KeyModifiers::CONTROL =>
            {
                self.agent_sender.send(AgentControlEvent::NewTask).unwrap()
            }
            KeyCode::Char('p') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.model.last_error = None;
                self.agent_sender
                    .send(AgentControlEvent::CancelTask)
                    .unwrap()
            }
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
