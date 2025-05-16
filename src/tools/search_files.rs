// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::io::{Cursor, ErrorKind};
use std::path::PathBuf;

use grep_printer::StandardBuilder;
use grep_regex::RegexMatcher;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::tools::{normalize_path, workspace_to_string};

use super::AgentToolError;

pub struct SearchFilesTool {
    pub workspace: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilesToolArgs {
    pub path: String,
    pub regex: String,
    pub file_pattern: Option<String>,
}

impl SearchFilesTool {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for SearchFilesTool {
    const NAME: &'static str = "search_files";

    type Error = AgentToolError;
    type Args = SearchFilesToolArgs;
    type Output = String;

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
                                                   This directory will be recursively searched.", workspace_to_string(&self.workspace)},
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

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let path = normalize_path(&self.workspace, &args.path);
        let matcher = RegexMatcher::new_line_matcher(&args.regex).map_err(|e| {
            std::io::Error::new(ErrorKind::InvalidInput, format!("invalid regex: {}", e))
        })?;
        tracing::info!("Search for path '{}' and regex {}", path, args.regex);
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .build();

        let mut buffer = Vec::new();
        let writer = Cursor::new(&mut buffer);
        let mut printer = StandardBuilder::new().build_no_color(writer);

        for entry in ignore::Walk::new(path).filter_map(|e| e.ok()) {
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let _ = searcher.search_path(
                &matcher,
                entry.path(),
                printer.sink_with_path(&matcher, entry.path()),
            );
        }
        let res = String::from_utf8(buffer).unwrap();
        if res.is_empty() {
            Ok("No results found".to_string())
        } else {
            Ok(res)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_files() {
        let tool = SearchFilesTool::new(".".into());
        let res = tool
            .call(SearchFilesToolArgs {
                path: "src".to_string(),
                regex: ".*Tool.*".to_string(),
                file_pattern: None,
            })
            .await
            .ok()
            .unwrap();
        assert!(!res.is_empty());
    }
}
