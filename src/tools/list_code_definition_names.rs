use std::path::PathBuf;

use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::tools::workspace_to_string;

#[derive(Debug, thiserror::Error)]
pub enum ListCodeDefinitionNamesError {
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListCodeDefinitionNamesToolArgs {
    pub path: String,
}

pub struct ListCodeDefinitionNamesTool {
    pub workspace: PathBuf,
}

impl ListCodeDefinitionNamesTool {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for ListCodeDefinitionNamesTool {
    const NAME: &'static str = "list_code_definition_names";

    type Error = ListCodeDefinitionNamesError;
    type Args = ListCodeDefinitionNamesToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Request to list definition names (classes, functions, methods, etc.) used in source code files at the \
                top level of the specified directory. This tool provides insights into the codebase structure and important \
                constructs, encapsulating high-level concepts and relationships that are crucial for understanding the overall architecture."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": formatdoc!{"The path of the directory (relative to the current working directory {}) \
                                                    to list top level source code definitions for.", workspace_to_string(&self.workspace)},
                    }
                },
                "required": ["path"]
            })

        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok("".to_string())
    }
}
