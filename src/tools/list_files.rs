// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
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
    pub max_depth: Option<usize>,
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
                Request to list files and directories within the specified directory. If max_depth equals 1 or not provided, \
                it will only list the top-level contents. If max_depth is greater than 1, it will list the contents of the directory \
                and its subdirectories up to the specified depth. Do not use this tool to confirm the existence of files you may have created,\
                as the user will let you know if the files were created successfully or not."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": formatdoc!{"The path of the directory to list contents for (relative to the current \
                                                   working directory {})", workspace_to_string(&self.workspace)},
                    },
                    "max_depth": {
                        "type": "number",
                        "description": "Max depth to list files (default: 1)",
                    }
                },
                "required": ["path"]
            })

        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = normalize_path(&self.workspace, &args.path);
        let max_depth = args.max_depth.unwrap_or(1);
        let mut files: Vec<String> = Vec::default();
        for entry in ignore::WalkBuilder::new(path.clone())
            .max_depth(Some(max_depth))
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
