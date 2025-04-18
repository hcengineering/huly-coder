use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use indoc::indoc;
use rig::completion::ToolDefinition;
use serde_json::json;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum ReadFileError {
    #[error("Read file error: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

pub struct ReadFileTool {
    pub workspace_dir: PathBuf,
}

impl ReadFileTool {
    pub fn new(workspace_dir: &str) -> Self {
        Self {
            workspace_dir: Path::new(workspace_dir).to_path_buf(),
        }
    }
}

impl ClineTool for ReadFileTool {
    const NAME: &'static str = "read_file";

    type Error = ReadFileError;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: indoc! {"\
                Request to read the contents of a file at the specified path. Use this when you need to examine the contents \
                of an existing file you do not know the contents of, for example to analyze code, review text files, \
                or extract information from configuration files. Automatically extracts raw text from PDF and DOCX files. \
                May not be suitable for other types of binary files, as it returns the raw content as a string."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": format!("The path of the file to read (relative to the current working directory {})", self.workspace_dir.as_path().to_str().unwrap())
                    }
                },
                "required": ["path"]
            })

        }
    }

    async fn call(&self, args: &HashMap<String, String>) -> Result<String, Self::Error> {
        if let Some(path) = args.get("path") {
            let path = if Path::new(path).is_absolute() {
                Path::new(path).to_path_buf()
            } else {
                self.workspace_dir.join(path)
            };
            tracing::info!("Reading file {}", path.display());
            fs::read_to_string(path).map_err(ReadFileError::ReadError)
        } else {
            Err(ReadFileError::ParametersError("path".to_string()))
        }
    }

    fn usage(&self) -> &str {
        indoc! {"
            <read_file>
            <path>File path here</path>
            </read_file>
        "}
    }
}
