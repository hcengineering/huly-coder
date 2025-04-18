use std::collections::HashMap;

use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use serde_json::json;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum AccessMcpResourceError {
    #[error("Access MCP Resource error: {0}")]
    AccessError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

pub struct AccessMcpResourceTool;

impl ClineTool for AccessMcpResourceTool {
    const NAME: &'static str = "access_mcp_resource";

    type Error = AccessMcpResourceError;

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

    async fn call(&self, args: &HashMap<String, String>) -> Result<String, Self::Error> {
        // TODO: implement this
        if let Some(_server_name) = args.get("server_name") {
            Ok("".to_string())
        } else {
            Err(AccessMcpResourceError::ParametersError(
                "server_name".to_string(),
            ))
        }
    }

    fn usage(&self) -> &str {
        indoc! {"
            <access_mcp_resource>
            <server_name>server name here</server_name>
            <uri>resource URI here</uri>
            </access_mcp_resource>
        "}
    }
}
