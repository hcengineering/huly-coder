use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use indoc::{formatdoc, indoc};
use rig::completion::ToolDefinition;
use serde_json::json;

use crate::tools::create_patch;

use super::ClineTool;

#[derive(Debug, thiserror::Error)]
pub enum ReplaceInFileError {
    #[error("Replace in file error: {0}")]
    ReplaceError(#[from] std::io::Error),
    #[error("Incorrect parameters error: {0}")]
    ParametersError(String),
    #[error("Search string not found: {0}")]
    SearchNotFound(String),
}

pub struct ReplaceInFileTool {
    pub workspace_dir: PathBuf,
}

impl ReplaceInFileTool {
    pub fn new(workspace_dir: &str) -> Self {
        Self {
            workspace_dir: Path::new(workspace_dir).to_path_buf(),
        }
    }
}

impl ClineTool for ReplaceInFileTool {
    const NAME: &'static str = "replace_in_file";

    type Error = ReplaceInFileError;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Request to replace sections of content in an existing file using SEARCH/REPLACE blocks that define exact \
                changes to specific parts of the file. This tool should be used when you need to make targeted changes \
                to specific parts of a file."}.to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": format!("The path of the file to modify (relative to the current working directory {})", self.workspace_dir.as_path().to_str().unwrap()),
                    },
                    "diff": {
                        "type": "string",
                        "description": indoc!{"
                            One or more SEARCH/REPLACE blocks following this exact format:
                              ```
                              <<<<<<< SEARCH
                              [exact content to find]
                              =======
                              [new content to replace with]
                              >>>>>>> REPLACE
                              ```
                              Critical rules:
                              1. SEARCH content must match the associated file section to find EXACTLY:
                                 * Match character-for-character including whitespace, indentation, line endings
                                 * Include all comments, docstrings, etc.
                              2. SEARCH/REPLACE blocks will ONLY replace the first match occurrence.
                                 * Including multiple unique SEARCH/REPLACE blocks if you need to make multiple changes.
                                 * Include *just* enough lines in each SEARCH section to uniquely match each set of lines that need to change.
                                 * When using multiple SEARCH/REPLACE blocks, list them in the order they appear in the file.
                              3. Keep SEARCH/REPLACE blocks concise:
                                 * Break large SEARCH/REPLACE blocks into a series of smaller blocks that each change a small portion of the file.
                                 * Include just the changing lines, and a few surrounding lines if needed for uniqueness.
                                 * Do not include long runs of unchanging lines in SEARCH/REPLACE blocks.
                                 * Each line must be complete. Never truncate lines mid-way through as this can cause matching failures.
                              4. Special operations:
                                 * To move code: Use two SEARCH/REPLACE blocks (one to delete from original + one to insert at new location)
                                 * To delete code: Use empty REPLACE section
                        "}
                    }
                },
                "required": ["path", "diff"]
            })

        }
    }

    async fn call(&self, args: &HashMap<String, String>) -> Result<String, Self::Error> {
        if let Some(path) = args.get("path") {
            if let Some(content) = args.get("diff") {
                let path = if Path::new(path).is_absolute() {
                    Path::new(path).to_path_buf()
                } else {
                    self.workspace_dir.join(path)
                };
                tracing::info!("Replace in file '{}'", path.display());
                let replace_diffs = parse_replace_diff(content)?;
                let original_content =
                    fs::read_to_string(path.clone()).map_err(ReplaceInFileError::ReplaceError)?;
                let mut modified_content = original_content.clone();
                for replace_diff in replace_diffs {
                    let search = &replace_diff.search;
                    let replace = &replace_diff.replace;
                    let start = original_content.find(search);
                    if let Some(start) = start {
                        let end = start + search.len();
                        modified_content.replace_range(start..end, replace);
                    } else {
                        return Err(ReplaceInFileError::SearchNotFound(replace_diff.search));
                    }
                }
                let diff = create_patch(&original_content, &modified_content);
                fs::write(path, modified_content).map_err(ReplaceInFileError::ReplaceError)?;
                Ok(format!(
                    "The user made the following updates to your content:\n\n{}",
                    diff
                ))
            } else {
                Err(ReplaceInFileError::ParametersError("diff".to_string()))
            }
        } else {
            Err(ReplaceInFileError::ParametersError("path".to_string()))
        }
    }

    fn usage(&self) -> &str {
        indoc! {"
            <replace_in_file>
            <path>File path here</path>
            <diff>
            Search and replace blocks here
            </diff>
            </replace_in_file>
        "}
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ReplaceDiffBlock {
    pub search: String,
    pub replace: String,
}

fn parse_replace_diff(diff: &str) -> Result<Vec<ReplaceDiffBlock>, ReplaceInFileError> {
    let mut diff_blocks = Vec::new();
    let mut current_block = ReplaceDiffBlock::default();
    let mut start_search = false;
    let mut start_replace = false;
    for line in diff.lines() {
        if line == "<<<<<<< SEARCH" {
            start_search = true;
            start_replace = false;
        } else if start_search && line == "=======" {
            start_replace = true;
            start_search = false;
        } else if line == ">>>>>>> REPLACE" {
            start_search = false;
            start_replace = false;
            diff_blocks.push(current_block);
            current_block = ReplaceDiffBlock::default();
        } else if start_search {
            current_block.search.push_str(line);
            current_block.search.push('\n');
        } else if start_replace {
            current_block.replace.push_str(line);
            current_block.replace.push('\n');
        }
    }
    Ok(diff_blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_replace() {
        let diff = r#"
<<<<<<< SEARCH
import React from 'react';
=======
import React, { useState } from 'react';
>>>>>>> REPLACE

<<<<<<< SEARCH
function handleSubmit() {
  saveData();
  setLoading(false);
}

=======
>>>>>>> REPLACE

<<<<<<< SEARCH
return (
  <div>
=======
function handleSubmit() {
  saveData();
  setLoading(false);
}

return (
  <div>
>>>>>>> REPLACE
"#;
        let diff_blocks = parse_replace_diff(diff).unwrap();
        assert_eq!(3, diff_blocks.len());
        assert_eq!(
            diff_blocks[0],
            ReplaceDiffBlock {
                search: "import React from 'react';\n".to_string(),
                replace: "import React, { useState } from 'react';\n".to_string()
            }
        );
    }
}
