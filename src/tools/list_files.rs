use std::collections::HashMap;
use std::path::{Path, PathBuf};

use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use serde_json::json;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum ListFilesError {
    #[error("List files error: {0}")]
    ListError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

pub struct ListFilesTool {
    pub workspace_dir: PathBuf,
}

impl ListFilesTool {
    pub fn new(workspace_dir: &str) -> Self {
        Self {
            workspace_dir: Path::new(workspace_dir).to_path_buf(),
        }
    }
}

impl ClineTool for ListFilesTool {
    const NAME: &'static str = "list_files";

    type Error = ListFilesError;

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
                                                   working directory {})", self.workspace_dir.as_path().to_str().unwrap()},
                    },
                    "recursive": {
                        "type": "string",
                        "description": "Whether to list files recursively. Use true for recursive listing, false or omit for top-level only."
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
            let recursive = args.get("recursive").unwrap_or(&"false".to_string()) == "true";

            tracing::info!("List files in '{}'", path.display());
            let mut files: Vec<String> = Vec::default();
            for entry in walkdir::WalkDir::new(path.clone())
                .max_depth(if recursive { usize::MAX } else { 1 })
                .follow_links(false)
                .same_file_system(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                files.push(entry
                        .path()
                        .strip_prefix(&path)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace("\\", "/").to_string());
            }
            Ok(files.join("\n"))
        } else {
            Err(ListFilesError::ParametersError("path".to_string()))
        }
    }

    fn usage(&self) -> &str {
        indoc! {"
            <list_files>
            <path>Directory path here</path>
            <recursive>true or false (optional)</recursive>
            </list_files>
        "}
    }
}
