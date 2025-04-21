use std::path::{Path, PathBuf};

use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListFilesToolArgs {
    pub path: String,
    pub recursive: Option<bool>,
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

impl Tool for ListFilesTool {
    const NAME: &'static str = "list_files";

    type Error = std::io::Error;
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
                                                   working directory {})", self.workspace_dir.as_path().to_str().unwrap()},
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
        let path = if Path::new(&args.path).is_absolute() {
            Path::new(&args.path).to_path_buf()
        } else {
            self.workspace_dir.join(args.path)
        };
        let recursive = args.recursive.unwrap_or(false);
        tracing::info!("List files in '{}'", path.display());
        let mut files: Vec<String> = Vec::default();
        for entry in walkdir::WalkDir::new(path.clone())
            .max_depth(if recursive { usize::MAX } else { 1 })
            .follow_links(false)
            .same_file_system(true)
            .into_iter()
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
        Ok(files.join("\n"))
    }
}
