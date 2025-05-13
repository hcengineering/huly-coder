use std::path::PathBuf;

use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::tools::workspace_to_string;

use super::{normalize_path, AgentToolError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFilesToolArgs {
    pub path: String,
    pub recursive: Option<bool>,
}

pub struct ListFilesTool {
    pub workspace: PathBuf,
}

impl ListFilesTool {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for ListFilesTool {
    const NAME: &'static str = "list_files";

    type Error = AgentToolError;
    type Args = ListFilesToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Request to list files and directories within the specified directory. If recursive is true, it will list \
                all files and directories recursively. If recursive is false or not provided, it will only list the top-level contents. \
                Do not use this tool to confirm the existence of files you may have created, as the user will let you know \
                if the files were created successfully or not."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": formatdoc!{"The path of the directory to list contents for (relative to the current \
                                                   working directory {})", workspace_to_string(&self.workspace)},
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Whether to list files recursively. Use true for recursive listing, false or omit for top-level only."
                    }
                },
                "required": ["path"]
            })

        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = normalize_path(&self.workspace, &args.path);
        let recursive = args.recursive.unwrap_or(false);
        let mut files: Vec<String> = Vec::default();
        for entry in ignore::WalkBuilder::new(path.clone())
            .max_depth(if recursive { None } else { Some(1) })
            .filter_entry(|e| e.file_name() != "node_modules")
            .build()
            .filter_map(|e| e.ok())
        {
            files.push(
                entry
                    .path()
                    .strip_prefix(&path)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace("\\", "/")
                    .to_string(),
            );
        }
        let res = files.join("\n");
        if res.is_empty() {
            Ok("No results found".to_string())
        } else {
            Ok(res)
        }
    }
}
