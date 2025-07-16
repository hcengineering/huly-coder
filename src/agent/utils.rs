// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use itertools::Itertools;
use rig::message::{Message, UserContent};
use rig::vector_store::in_memory_store::InMemoryVectorIndex;
use rig::vector_store::VectorStoreIndex;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::templates::{ENV_DETAILS, SYSTEM_PROMPT};
use crate::tools::execute_command::ProcessRegistry;
use crate::tools::memory::{self, Entity};
use crate::HISTORY_PATH;

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

pub async fn prepare_system_prompt(config: &Config) -> String {
    let workspace_dir = config
        .workspace
        .as_os_str()
        .to_str()
        .unwrap()
        .replace("\\", "/");
    subst::substitute(
        SYSTEM_PROMPT,
        &HashMap::from([
            ("WORKSPACE_DIR", workspace_dir.as_str()),
            ("USER_NAME", &config.appearance.user_name),
            ("OS_NAME", std::env::consts::OS),
            ("OS_SHELL_EXECUTABLE", &get_shell_path()),
            ("USER_HOME_DIR", dirs::home_dir().unwrap().to_str().unwrap()),
            ("USER_INSTRUCTION", &config.user_instructions),
        ]),
    )
    .unwrap()
}

pub async fn add_env_message<'a>(
    msg: &'a mut Message,
    memory_index: Arc<
        tokio::sync::RwLock<InMemoryVectorIndex<rig_fastembed::EmbeddingModel, memory::Entity>>,
    >,
    data_dir: &'a Path,
    workspace: &'a Path,
    process_registry: Arc<RwLock<ProcessRegistry>>,
) {
    let workspace = workspace.as_os_str().to_str().unwrap().replace("\\", "/");
    let mut files: Vec<String> = Vec::default();

    for entry in ignore::WalkBuilder::new(&workspace)
        .filter_entry(|e| e.file_name() != "node_modules")
        .max_depth(Some(2))
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
        let memory_index = memory_index.read().await;
        let txt = match text {
            UserContent::Text(text) => &text.text.to_string(),
            UserContent::ToolResult(tool_result) => match tool_result.content.first() {
                rig::message::ToolResultContent::Text(text) => &text.text.to_string(),
                rig::message::ToolResultContent::Image(_) => "",
            },
            _ => "",
        };
        if !txt.is_empty() {
            let res: Vec<(f64, String, Entity)> = memory_index.top_n(txt, 10).await.unwrap();
            let result: Vec<_> = res.into_iter().map(|(_, _, entity)| entity).collect();
            memory_entries = serde_yaml::to_string(&result).unwrap();
        }

        let commands = process_registry
            .read()
            .await
            .processes()
            .map(|(id, status, command)| {
                format!(
                    "| {}    | {}                 | `{}` |",
                    id,
                    if let Some(exit_status) = status {
                        format!("Exited({})", exit_status)
                    } else {
                        "Running".to_string()
                    },
                    command
                )
            })
            .join("\n");
        let env_content = subst::substitute(
            ENV_DETAILS,
            &HashMap::from([
                ("TIME", chrono::Local::now().to_rfc2822().as_str()),
                ("WORKING_DIR", &workspace),
                ("MEMORY_ENTRIES", &memory_entries),
                ("COMMANDS", &commands),
                ("FILES", files),
            ]),
        )
        .unwrap();
        fs::write(data_dir.join("env.txt"), &env_content).unwrap();
        content.push(UserContent::text(env_content));
    }
}

pub fn persist_history(data_dir: &Path, messages: &[Message]) {
    fs::write(
        data_dir.join(HISTORY_PATH),
        serde_json::to_string_pretty(messages).unwrap(),
    )
    .unwrap();
}
