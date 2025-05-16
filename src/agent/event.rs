// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use rig::message::Message;

#[derive(Clone, Debug, Default)]
pub struct AgentTaskStatus {
    pub current_tokens: u32,
    pub max_tokens: u32,
    pub processing: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AgentCommandStatus {
    pub command: String,
    pub output: String,
}

#[derive(Clone, Debug)]
pub enum AgentOutputEvent {
    AddMessage(Message),
    UpdateMessage(Message),
    NewTask,
    Error(String),
    ExecuteCommand(AgentCommandStatus),
    TaskStatus(AgentTaskStatus),
    HighlightFile(String, bool),
}

#[derive(Clone, Debug)]
pub enum AgentControlEvent {
    SendMessage(String),
    CancelTask,
    NewTask,
}
