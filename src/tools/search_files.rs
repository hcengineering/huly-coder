use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use grep_printer::StandardBuilder;
use grep_regex::RegexMatcher;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use serde_json::json;
use walkdir::WalkDir;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum SearchFilesError {
    #[error("Search file error: {0}")]
    SearchError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
}

pub struct SearchFilesTool {
    pub workspace_dir: PathBuf,
}

impl SearchFilesTool {
    pub fn new(workspace_dir: &str) -> Self {
        Self {
            workspace_dir: Path::new(workspace_dir).to_path_buf(),
        }
    }
}

impl ClineTool for SearchFilesTool {
    const NAME: &'static str = "search_files";

    type Error = SearchFilesError;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Request to perform a regex search across files in a specified directory, providing context-rich results. \
                This tool searches for patterns or specific content across multiple files, displaying each match with encapsulating context."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": formatdoc!{"The path of the directory to search in (relative to the current working directory {}). \
                                                   This directory will be recursively searched.", self.workspace_dir.as_path().to_str().unwrap()},
                    },
                    "regex": {
                        "type": "string",
                        "description": "The regular expression pattern to search for. Uses Rust regex syntax."
                    },
                    "file_pattern": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g., '*.ts' for TypeScript files). If not provided, it will search all files (*)."
                    }
                },
                "required": ["path", "regex"]
            })

        }
    }

    async fn call(&self, args: &HashMap<String, String>) -> Result<String, Self::Error> {
        if let Some(path) = args.get("path") {
            if let Some(regex) = args.get("regex") {
                //                let regex = Regex::new(regex_str).map_err(|e| {
                //                    SearchFilesError::ParametersError(format!("invalid regex: {}", e))
                //                })?;
                let path = if Path::new(path).is_absolute() {
                    Path::new(path).to_path_buf()
                } else {
                    self.workspace_dir.join(path)
                };
                let matcher = RegexMatcher::new_line_matcher(regex).map_err(|e| {
                    SearchFilesError::ParametersError(format!("invalid regex: {}", e))
                })?;
                tracing::info!("Search for path '{}' and regex {}", path.display(), regex);
                let mut searcher = SearcherBuilder::new()
                    .binary_detection(BinaryDetection::quit(b'\x00'))
                    .build();

                let mut buffer = Vec::new();
                let writer = Cursor::new(&mut buffer);
                let mut printer = StandardBuilder::new().build_no_color(writer);

                for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let _ = searcher.search_path(
                        &matcher,
                        entry.path(),
                        printer.sink_with_path(&matcher, entry.path()),
                    );
                }
                Ok(String::from_utf8(buffer).unwrap())
            } else {
                Err(SearchFilesError::ParametersError("regex".to_string()))
            }
        } else {
            Err(SearchFilesError::ParametersError("path".to_string()))
        }
    }

    fn usage(&self) -> &str {
        indoc! {"
            <search_files>
            <path>Directory path here</path>
            <regex>Your regex pattern here</regex>
            <file_pattern>file pattern here (optional)</file_pattern>
            </search_files>
        "}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_files() {
        let tool = SearchFilesTool::new(".");
        let res = tool
            .call(&HashMap::from([
                ("path".to_string(), "src".to_string()),
                ("regex".to_string(), ".*Tool.*".to_string()),
            ]))
            .await
            .ok()
            .unwrap();
        assert!(!res.is_empty());
    }
}
