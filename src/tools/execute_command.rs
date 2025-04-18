use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use serde_json::json;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum ExecuteCommandError {
    #[error("Execute command error: {0}")]
    ExecuteError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

pub struct ExecuteCommandTool {
    pub workspace_dir: PathBuf,
}

impl ExecuteCommandTool {
    pub fn new(workspace_dir: &str) -> Self {
        Self {
            workspace_dir: Path::new(workspace_dir).to_path_buf(),
        }
    }
}

impl ClineTool for ExecuteCommandTool {
    const NAME: &'static str = "execute_command";

    type Error = ExecuteCommandError;

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
                workspace_dir = self.workspace_dir.display()}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The CLI command to execute. This should be valid for the current operating system.\
                                        Ensure the command is properly formatted and does not contain any harmful instructions.",
                    },
                    "requires_approval": {
                        "type": "string",
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

    async fn call(&self, args: &HashMap<String, String>) -> Result<String, Self::Error> {
        if let Some(command) = args.get("command") {
            let _requires_approval = args
                .get("requires_approval")
                .unwrap_or(&"false".to_string())
                == "true";
            tracing::info!("Executing command '{}'", command);
            let mut cmd = if cfg!(target_os = "windows") {
                Command::new("cmd")
            } else {
                Command::new("bash")
            };
            cmd.current_dir(self.workspace_dir.clone())
                .arg(if cfg!(target_os = "windows") {
                    "/C"
                } else {
                    "-c"
                })
                .arg(command)
                .output()
                .map(|output| {
                    if output.status.success() {
                        String::from_utf8(output.stdout).unwrap_or_else(|_| "".to_string())
                    } else {
                        String::from_utf8(output.stderr).unwrap_or_else(|_| "".to_string())
                    }
                })
                .map_err(ExecuteCommandError::ExecuteError)
        } else {
            Err(ExecuteCommandError::ParametersError("command".to_string()))
        }
    }

    fn usage(&self) -> &str {
        indoc! {"
            <execute_command>
            <command>Your command here</command>
            <requires_approval>true or false</requires_approval>
            </execute_command>
        "}
    }
}
