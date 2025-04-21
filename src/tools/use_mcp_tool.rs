use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub struct UseMcpTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseMcpToolArgs {
    pub server_name: String,
    pub tool_name: String,
    pub arguments: String,
}

impl Tool for UseMcpTool {
    const NAME: &'static str = "use_mcp_tool";

    type Error = std::io::Error;
    type Args = UseMcpToolArgs;
    type Output = String;

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

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok("".to_string())
    }
}
