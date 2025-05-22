// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::path::PathBuf;
use std::sync::Arc;

use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{mpsc, RwLock};

use crate::agent::event::AgentCommandStatus;
use crate::agent::AgentOutputEvent;
use crate::tools::{workspace_to_string, AgentToolError};

use super::ProcessRegistry;

const COMMAND_TIMEOUT: u64 = 300; // 30 secs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteCommandToolArgs {
    pub command: String,
}

pub struct ExecuteCommandTool {
    workspace: PathBuf,
    process_registry: Arc<RwLock<ProcessRegistry>>,
    sender: mpsc::UnboundedSender<AgentOutputEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCommandResultToolArgs {
    pub command_id: usize,
}

pub struct GetCommandResultTool {
    process_registry: Arc<RwLock<ProcessRegistry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminateCommandToolArgs {
    pub command_id: usize,
}

pub struct TerminateCommandTool {
    process_registry: Arc<RwLock<ProcessRegistry>>,
}

impl ExecuteCommandTool {
    pub fn new(
        workspace: PathBuf,
        process_registry: Arc<RwLock<ProcessRegistry>>,
        sender: mpsc::UnboundedSender<AgentOutputEvent>,
    ) -> Self {
        Self {
            workspace,
            process_registry,
            sender,
        }
    }
}

impl GetCommandResultTool {
    pub fn new(process_registry: Arc<RwLock<ProcessRegistry>>) -> Self {
        Self { process_registry }
    }
}

impl TerminateCommandTool {
    pub fn new(process_registry: Arc<RwLock<ProcessRegistry>>) -> Self {
        Self { process_registry }
    }
}

impl Tool for ExecuteCommandTool {
    const NAME: &'static str = "execute_command";

    type Error = AgentToolError;
    type Args = ExecuteCommandToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Request to execute a CLI command on the system. Use this when you need to perform system operations or \
                run specific commands to accomplish any step in the user's task. You must tailor your command to the \
                user's system and provide a clear explanation of what the command does. For command chaining, use the \
                appropriate chaining syntax for the user's shell. Prefer to execute complex CLI commands over creating \
                executable scripts, as they are more flexible and easier to run. \
                Returns the command ID, exit status, and command output upon completion.\
                For running commands, returns the ID, partial output, and a \"Command is run\" indicator.\
                If the command is still running, it will return the ID and the output of the last command.\
                Commands will be executed in the current working directory: {workspace_dir}",
                workspace_dir = workspace_to_string(&self.workspace)}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The CLI command to execute. This should be valid for the current operating system.\
                                        Ensure the command is properly formatted and does not contain any harmful instructions.",
                    },
                },
                "required": ["command"]
            })

        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Executing command '{}'", args.command);
        let command_id = self
            .process_registry
            .write()
            .await
            .execute_command(&args.command, &workspace_to_string(&self.workspace))
            .await?;
        let mut command_output = String::new();
        for _ in 0..COMMAND_TIMEOUT {
            self.process_registry.write().await.poll();
            if let Some((exit_status, output)) =
                self.process_registry.read().await.get_process(command_id)
            {
                self.sender
                    .send(AgentOutputEvent::CommandStatus(vec![AgentCommandStatus {
                        command_id,
                        command: Some(args.command.clone()),
                        output: output.to_string(),
                        is_active: exit_status.is_none(),
                    }]))
                    .ok();
                if let Some(exit_status) = exit_status {
                    return Ok(format!(
                        "Command ID: {}\nExit Status: Exited({})\nOutput:\n{}",
                        command_id,
                        exit_status.code().unwrap_or_default(),
                        output
                    ));
                }
                command_output = output.to_string();
            } else {
                return Err(AgentToolError::Other(anyhow::anyhow!(
                    "Command '{}' not found",
                    args.command
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        Ok(format!(
            "Command ID: {}\nCommand is run\nOutput:\n{}",
            command_id, command_output
        ))
    }
}

impl Tool for GetCommandResultTool {
    const NAME: &'static str = "get_command_result";

    type Error = AgentToolError;
    type Args = GetCommandResultToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Retrieves the complete result of a previously executed command by `execute_command` that may still be running.\
                ## Example usage:
                When you need to check the final output of a long-running process that was previously started.\
            "}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command_id": {
                        "type": "number",
                        "description": "The identifier of the command returned by the `execute_command` tool",
                    },
                },
                "required": ["command_id"]
            })

        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Get command result '{}'", args.command_id);
        if let Some((exit_status, output)) = self
            .process_registry
            .read()
            .await
            .get_process(args.command_id)
        {
            if let Some(exit_status) = exit_status {
                Ok(format!(
                    "Command ID: {}\nExit Status: Exited({})\nOutput:\n{}",
                    args.command_id,
                    exit_status.code().unwrap_or_default(),
                    output
                ))
            } else {
                Ok(format!(
                    "Command ID: {}\nCommand Still Running\nOutput:\n{}",
                    args.command_id, output
                ))
            }
        } else {
            Err(AgentToolError::Other(anyhow::anyhow!(
                "Command '{}' not found",
                args.command_id
            )))
        }
    }
}

impl Tool for TerminateCommandTool {
    const NAME: &'static str = "terminate_command";

    type Error = AgentToolError;
    type Args = TerminateCommandToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: "Terminate the command execution with the given ID. command_id is the ID returned by the `execute_command` tool.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command_id": {
                        "type": "number",
                        "description": "ID of command to terminate.",
                    },
                },
                "required": ["command_id"]
            })

        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Terminate command '{}'", args.command_id);
        self.process_registry
            .write()
            .await
            .stop_process(args.command_id)?;
        Ok(format!(
            "Command with ID {} successfully terminated.",
            args.command_id
        ))
    }
}
