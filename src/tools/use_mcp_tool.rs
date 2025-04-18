use std::collections::HashMap;

use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use serde_json::json;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum UseMcpToolError {
    #[error("Use MCP tool error: {0}")]
    UseMcpError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

pub struct UseMcpTool;

impl ClineTool for UseMcpTool {
    const NAME: &'static str = "use_mcp_tool";

    type Error = UseMcpToolError;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Request to use a tool provided by a connected MCP server. Each MCP server can provide multiple tools with \
                different capabilities. Tools have defined input schemas that specify required and optional parameters."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "server_name": {
                        "type": "string",
                        "description": "The name of the MCP server providing the tool",
                    },
                    "tool_name": {
                        "type": "string",
                        "description": "The name of the tool to execute"
                    },
                    "arguments": {
                        "type": "string",
                        "description": "A JSON object containing the tool's input parameters, following the tool's input schema"
                    }
                },
                "required": ["server_name", "tool_name", "arguments"]
            })

        }
    }

    async fn call(&self, args: &HashMap<String, String>) -> Result<String, Self::Error> {
        if let Some(_server_name) = args.get("server_name") {
            Ok("".to_string())
        } else {
            Err(UseMcpToolError::ParametersError("server_name".to_string()))
        }
    }

    fn usage(&self) -> &str {
        indoc! {r#"
            <use_mcp_tool>
            <server_name>server name here</server_name>
            <tool_name>tool name here</tool_name>
            <arguments>
            {
              "param1": "value1",
              "param2": "value2"
            }
            </arguments>
            </use_mcp_tool>
        "#}
    }
}
