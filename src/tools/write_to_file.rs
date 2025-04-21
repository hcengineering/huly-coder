use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tools::create_patch;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteToFileToolArgs {
    pub path: String,
    pub content: String,
}

pub struct WriteToFileTool {
    pub workspace_dir: PathBuf,
}

impl WriteToFileTool {
    pub fn new(workspace_dir: &str) -> Self {
        Self {
            workspace_dir: Path::new(workspace_dir).to_path_buf(),
        }
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
                        "description": format!("The path of the file to write to (relative to the current working directory {})", self.workspace_dir.as_path().to_str().unwrap()),
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
        let path = if Path::new(&args.path).is_absolute() {
            Path::new(&args.path).to_path_buf()
        } else {
            self.workspace_dir.join(args.path)
        };
        tracing::info!("Write to file '{}'", path.display());
        let diff = create_patch("", &args.content);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, args.content)?;
        Ok(format!(
            "The user made the following updates to your content:\n\n{}",
            diff
        ))
    }
}
