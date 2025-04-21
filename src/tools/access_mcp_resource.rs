use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessMcpResourceToolArgs {
    pub server_name: String,
    pub uri: String,
}

pub struct AccessMcpResourceTool;

impl Tool for AccessMcpResourceTool {
    const NAME: &'static str = "access_mcp_resource";

    type Error = std::io::Error;
    type Args = AccessMcpResourceToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Request to access a resource provided by a connected MCP server. Resources represent data sources that \
                can be used as context, such as files, API responses, or system information."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "server_name": {
                        "type": "string",
                        "description": "The name of the MCP server providing the resource",
                    },
                    "uri": {
                        "type": "string",
                        "description": "The URI identifying the specific resource to access"
                    },
                },
                "required": ["server_name", "uri"]
            })

        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        // TODO: implement this
        Ok("".to_string())
    }
}
