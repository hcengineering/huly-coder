use std::fs;
use std::path::{Path, PathBuf};

use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadFileToolArgs {
    pub path: String,
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

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";

    type Error = std::io::Error;
    type Args = ReadFileToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
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

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = if Path::new(&args.path).is_absolute() {
            Path::new(&args.path).to_path_buf()
        } else {
            self.workspace_dir.join(&args.path)
        };
        tracing::info!("Reading file {}", path.display());
        fs::read_to_string(path)
    }
}
