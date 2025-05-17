// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use heck::ToTitleCase;
use rig::tool::Tool;

use crate::tools::ask_followup_question::AskFollowupQuestionTool;
use crate::tools::attempt_completion::AttemptCompletionTool;
use crate::tools::execute_command::ExecuteCommandTool;
use crate::tools::list_files::ListFilesTool;
use crate::tools::memory::{
    MemoryAddObservationsTool, MemoryCreateEntitiesTool, MemoryCreateRelationsTool,
    MemoryDeleteEntitiesTool, MemoryDeleteObservationsTool, MemoryDeleteRelationsTool,
    MemoryOpenNodesTool, MemoryReadGraphTool, MemorySearchNodesTool,
};
use crate::tools::read_file::ReadFileTool;
use crate::tools::replace_in_file::ReplaceInFileTool;
use crate::tools::search_files::SearchFilesTool;
use crate::tools::web_fetch::WebFetchTool;
use crate::tools::web_search::WebSearchTool;
use crate::tools::write_to_file::WriteToFileTool;

fn array_info<'a>(name: &'a str, child_name: &'a str, args: &'a serde_json::Value) -> &'a str {
    args.get(name)
        .and_then(|v| {
            v.as_array().and_then(|a| {
                if a.is_empty() {
                    Some("")
                } else {
                    a.first().and_then(|f| {
                        if child_name.is_empty() {
                            f.as_str()
                        } else {
                            Some(array_info(child_name, "", f))
                        }
                    })
                }
            })
        })
        .unwrap_or_default()
}

pub fn get_tool_call_info(name: &str, args: &serde_json::Value) -> (String, String) {
    let title = name.to_string().to_title_case();
    let path = args
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let regex = args
        .get("regex")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let (icon, info) = match name {
        AskFollowupQuestionTool::NAME => ("🛠️", "Ask followup question".to_string()),
        AttemptCompletionTool::NAME => ("✅️", "Task completed".to_string()),
        ExecuteCommandTool::NAME => (
            "🖥️️",
            format!(
                "Execute command '{}'",
                args.get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
            ),
        ),
        ListFilesTool::NAME => ("📁", format!("List files in {}", path)),
        ReadFileTool::NAME => ("📁", format!("Read file {}", path)),
        ReplaceInFileTool::NAME => ("📁", format!("Replace in file {}", path)),
        SearchFilesTool::NAME => (
            "📁",
            format!("Search files with regex '{}' in {}", regex, path),
        ),
        WriteToFileTool::NAME => ("📁", format!("Write to file {}", path)),
        // web related
        WebFetchTool::NAME => (
            "🌍",
            format!(
                "Fetch URL {}",
                args.get("url").and_then(|v| v.as_str()).unwrap_or_default()
            ),
        ),
        WebSearchTool::NAME => (
            "🌍",
            format!(
                "Search web with query '{}'",
                args.get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
            ),
        ),
        // Memory related
        MemoryCreateEntitiesTool::NAME => (
            "🧠",
            format!("Create entities: {}", array_info("entities", "name", args)),
        ),
        MemoryCreateRelationsTool::NAME => (
            "🧠",
            format!(
                "Create relations: {}",
                array_info("relations", "relationType", args)
            ),
        ),
        MemoryAddObservationsTool::NAME => (
            "🧠",
            format!(
                "Add observations: {}",
                array_info("observations", "entityName", args)
            ),
        ),
        MemoryDeleteEntitiesTool::NAME => (
            "🧠",
            format!("Delete entities: {}", array_info("entityNames", "", args)),
        ),
        MemoryDeleteObservationsTool::NAME => (
            "🧠",
            format!(
                "Delete observations: {}",
                array_info("deletions", "entityName", args)
            ),
        ),
        MemoryDeleteRelationsTool::NAME => (
            "🧠",
            format!(
                "Delete relations: {}",
                array_info("relations", "relationType", args)
            ),
        ),
        MemoryReadGraphTool::NAME => ("🧠", "Read knowledge graph".to_string()),
        MemorySearchNodesTool::NAME => (
            "🧠",
            format!(
                "Search nodes with query '{}'",
                args.get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
            ),
        ),
        MemoryOpenNodesTool::NAME => (
            "🧠",
            format!("Open nodes: {}", array_info("names", "", args)),
        ),
        // MCP
        _ => ("🛠️", format!("MCP Tool: {}", title)),
    };
    (icon.to_string(), info)
}
