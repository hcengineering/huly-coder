// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use rig::message::{Message, UserContent};
use rig::vector_store::in_memory_store::InMemoryVectorIndex;
use rig::vector_store::VectorStoreIndex;

use crate::templates::{ENV_DETAILS, SYSTEM_PROMPT};
use crate::tools::memory::Entity;

pub const MAX_FILES: usize = 10000;

fn get_shell_path() -> String {
    if let Ok(shell) = std::env::var("SHELL") {
        shell
    } else if let Ok(comspec) = std::env::var("COMSPEC") {
        comspec
    } else {
        panic!("Could not determine shell path from environment variables")
    }
}

pub async fn prepare_system_prompt(workspace_dir: &Path, user_instructions: &str) -> String {
    let workspace_dir = workspace_dir
        .as_os_str()
        .to_str()
        .unwrap()
        .replace("\\", "/");
    subst::substitute(
        SYSTEM_PROMPT,
        &HashMap::from([
            ("WORKSPACE_DIR", workspace_dir.as_str()),
            ("OS_NAME", std::env::consts::OS),
            ("OS_SHELL_EXECUTABLE", &get_shell_path()),
            ("USER_HOME_DIR", dirs::home_dir().unwrap().to_str().unwrap()),
            ("USER_INSTRUCTION", user_instructions),
        ]),
    )
    .unwrap()
}

pub async fn add_env_message<'a>(
    msg: &'a mut Message,
    memory_index: Option<&'a InMemoryVectorIndex<rig_fastembed::EmbeddingModel, Entity>>,
    workspace: &'a Path,
) {
    let workspace = workspace.as_os_str().to_str().unwrap().replace("\\", "/");
    let mut files: Vec<String> = Vec::default();

    for entry in ignore::WalkBuilder::new(&workspace)
        .filter_entry(|e| e.file_name() != "node_modules")
        .build()
        .filter_map(|e| e.ok())
        .take(MAX_FILES)
    {
        files.push(
            entry
                .path()
                .to_str()
                .unwrap()
                .replace("\\", "/")
                .strip_prefix(&workspace)
                .unwrap()
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
        let text = content.first();
        let mut memory_entries = String::new();
        if let Some(memory_index) = memory_index {
            if let UserContent::Text(text) = text {
                let res: Vec<(f64, String, Entity)> =
                    memory_index.top_n(&text.text, 10).await.unwrap();
                let result: Vec<_> = res.into_iter().map(|(_, _, entity)| entity).collect();
                memory_entries = serde_yaml::to_string(&result).unwrap();
            }
        }
        content.push(UserContent::text(
            subst::substitute(
                ENV_DETAILS,
                &HashMap::from([
                    ("TIME", chrono::Local::now().to_rfc2822().as_str()),
                    ("WORKING_DIR", &workspace),
                    ("MEMORY_ENTRIES", &memory_entries),
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
