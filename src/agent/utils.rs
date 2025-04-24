use std::collections::HashMap;
use std::fs;

use rig::message::{Message, UserContent};
use walkdir::DirEntry;

use crate::templates::{ENV_DETAILS, SYSTEM_PROMPT};

const MAX_FILES: usize = 200;

fn get_shell_path() -> String {
    if let Ok(shell) = std::env::var("SHELL") {
        shell
    } else if let Ok(comspec) = std::env::var("COMSPEC") {
        comspec
    } else {
        panic!("Could not determine shell path from environment variables")
    }
}

pub fn is_ignored(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| {
            s.starts_with(".")
                || s.contains("node_modules")
                || s.contains("target")
                || s.contains("build")
        })
        .unwrap_or(false)
}

pub async fn prepare_system_prompt(workspace_dir: &str, user_instructions: &str) -> String {
    subst::substitute(
        SYSTEM_PROMPT,
        &HashMap::from([
            ("WORKSPACE_DIR", workspace_dir),
            ("OS_NAME", std::env::consts::OS),
            ("OS_SHELL_EXECUTABLE", &get_shell_path()),
            ("USER_HOME_DIR", "."),
            ("USER_INSTRUCTION", user_instructions),
            ("MCP_SECTION", ""),
        ]),
    )
    .unwrap()
}

pub fn add_env_message<'a>(msg: &'a mut Message, workspace_dir: &'a str) {
    let mut files: Vec<String> = Vec::default();

    for entry in walkdir::WalkDir::new(workspace_dir)
        .follow_links(false)
        .same_file_system(true)
        .into_iter()
        .filter_entry(|e| !is_ignored(e))
        .filter_map(|e| e.ok())
        .take(MAX_FILES)
    {
        files.push(
            entry
                .path()
                .strip_prefix(workspace_dir)
                .unwrap()
                .to_str()
                .unwrap()
                .replace("\\", "/")
                .to_string(),
        );
    }
    let files_str = files.join("\n");
    let files = if files.is_empty() {
        "No files found."
    } else {
        &files_str
    };
    if let Message::User { content } = msg {
        content.push(UserContent::text(
            subst::substitute(
                ENV_DETAILS,
                &HashMap::from([
                    ("TIME", chrono::Local::now().to_rfc2822().as_str()),
                    ("WORKING_DIR", workspace_dir),
                    ("FILES", files),
                ]),
            )
            .unwrap(),
        ));
    }
}

pub fn persist_history(messages: &Vec<Message>) {
    fs::write(
        "history.json",
        serde_json::to_string_pretty(messages).unwrap(),
    )
    .unwrap();
}
