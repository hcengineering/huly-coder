use std::fmt::Display;

// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use rig::message::Message;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum AgentState {
    #[default]
    Paused,
    WaitingResponse,
    Thinking,
    WaitingUserPrompt,
    Error(String),
    Completed(bool),
    ToolCall(String, serde_json::Value),
}

impl AgentState {
    pub fn is_paused(&self) -> bool {
        matches!(
            self,
            Self::Paused | Self::Completed(true) | Self::Error(_) | Self::WaitingUserPrompt
        )
    }

    #[inline]
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed(is_finished) if *is_finished)
    }
}

impl Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Paused => write!(f, "Paused"),
            Self::WaitingResponse => write!(f, "WaitingResponse"),
            Self::Thinking => write!(f, "Thinking"),
            Self::WaitingUserPrompt => write!(f, "WaitingUserPrompt"),
            Self::Error(_) => write!(f, "Error"),
            Self::Completed(is_finished) => write!(f, "Completed({})", is_finished),
            Self::ToolCall(name, _) => write!(f, "ToolCall[{}]", name),
        }
    }
}

/// Status of a command tool call
#[derive(Clone, Debug, Default)]
pub struct AgentCommandStatus {
    pub command_id: usize,
    pub command: Option<String>,
    pub output: String,
    pub is_active: bool,
}

/// Events that are sent from the agent to UI
#[derive(Clone, Debug)]
pub enum AgentOutputEvent {
    AddMessage(Message),
    UpdateMessage(Message),
    NewTask,
    CommandStatus(Vec<AgentCommandStatus>),
    AgentStatus(u32, u32, AgentState),
    HighlightFile(String, bool),
}

/// Controls events that are sent to the agent
#[derive(Clone, Debug)]
pub enum AgentControlEvent {
    SendMessage(String),
    CancelTask,
    NewTask,
}
