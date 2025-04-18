use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::tools::create_patch;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum WriteToFileError {
    #[error("Write file error: {0}")]
    WriteError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
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

impl ClineTool for WriteToFileTool {
    const NAME: &'static str = "write_to_file";

    type Error = WriteToFileError;

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

    async fn call(&self, args: &HashMap<String, String>) -> Result<String, Self::Error> {
        if let Some(path) = args.get("path") {
            if let Some(content) = args.get("content") {
                let path = if Path::new(path).is_absolute() {
                    Path::new(path).to_path_buf()
                } else {
                    self.workspace_dir.join(path)
                };
                tracing::info!("Write to file '{}'", path.display());
                let diff = create_patch("", content);
                fs::create_dir_all(path.parent().unwrap()).map_err(WriteToFileError::WriteError)?;
                fs::write(path, content).map_err(WriteToFileError::WriteError)?;
                Ok(format!(
                    "The user made the following updates to your content:\n\n{}",
                    diff
                ))
            } else {
                Err(WriteToFileError::ParametersError("content".to_string()))
            }
        } else {
            Err(WriteToFileError::ParametersError("path".to_string()))
        }
    }

    fn usage(&self) -> &str {
        indoc! {"
            <write_to_file>
            <path>File path here</path>
            <content>
            Your file content here
            </content>
            </write_to_file>
        "}
    }
}
