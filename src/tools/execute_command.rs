use std::path::PathBuf;
use std::process::Command;

use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::workspace_to_string;

#[derive(Debug, thiserror::Error)]
pub enum ExecuteCommandError {
    #[error("Execute command error: {0}")]
    ExecuteError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteCommandToolArgs {
    pub command: String,
    pub requires_approval: bool,
}

pub struct ExecuteCommandTool {
    pub workspace: PathBuf,
}

impl ExecuteCommandTool {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for ExecuteCommandTool {
    const NAME: &'static str = "execute_command";

    type Error = ExecuteCommandError;
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
                    "requires_approval": {
                        "type": "boolean",
                        "description": "A boolean indicating whether this command requires explicit user approval before \
                                        execution in case the user has auto-approve mode enabled. Set to 'true' for potentially \
                                        impactful operations like installing/uninstalling packages, deleting/overwriting files, \
                                        system configuration changes, network operations, or any commands that could have unintended side effects. \
                                        Set to 'false' for safe operations like reading files/directories, running development servers, building projects, \
                                        and other non-destructive operations."
                    }
                },
                "required": ["command", "requires_approval"]
            })

        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Executing command '{}'", args.command);
        let mut cmd = if cfg!(target_os = "windows") {
            Command::new("cmd")
        } else {
            Command::new("bash")
        };
        cmd.current_dir(workspace_to_string(&self.workspace))
            .arg(if cfg!(target_os = "windows") {
                "/C"
            } else {
                "-c"
            })
            .arg(args.command)
            .output()
            .map(|output| {
                format!(
                    "{}\n{}",
                    String::from_utf8(output.stderr).unwrap_or_else(|_| "".to_string()),
                    String::from_utf8(output.stdout).unwrap_or_else(|_| "".to_string())
                )
            })
            .map_err(ExecuteCommandError::ExecuteError)
    }
}
