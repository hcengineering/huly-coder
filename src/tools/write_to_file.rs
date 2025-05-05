use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tools::{create_patch, normalize_path, workspace_to_string};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteToFileToolArgs {
    pub path: String,
    pub content: String,
}

pub struct WriteToFileTool {
    pub workspace: PathBuf,
}

impl WriteToFileTool {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for WriteToFileTool {
    const NAME: &'static str = "write_to_file";

    type Error = std::io::Error;
    type Args = WriteToFileToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Request to write content to a file at the specified path. If the file exists, it will be overwritten \
                with the provided content. If the file doesn't exist, it will be created. This tool will automatically \
                create any directories needed to write the file."
            }.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": format!("The path of the file to write to (relative to the current working directory {})", workspace_to_string(&self.workspace)),
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file. ALWAYS provide the COMPLETE intended content of the file, \
                                        without any truncation or omissions. You MUST include ALL parts of the file, \
                                        even if they haven't been modified."
                    }
                },
                "required": ["path", "content"]
            })

        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = normalize_path(&self.workspace, &args.path);
        tracing::info!("Write to file '{}'", path);
        let diff = create_patch("", &args.content);
        fs::create_dir_all(Path::new(&path).parent().unwrap())?;
        fs::write(path, args.content)?;
        Ok(format!(
            "The user made the following updates to your content:\n\n{}",
            diff
        ))
    }
}
