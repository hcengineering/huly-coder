use std::collections::HashMap;
use std::error::Error;
use std::{env, fs};

use futures_util::StreamExt;
use indoc::indoc;
use rig::message::{AssistantContent, Message, ToolResultContent, UserContent};
use rig::streaming::StreamingCompletion;
use rig::tool::Tool;
use rig::OneOrMany;
use walkdir::DirEntry;

use self::providers::openrouter;
use self::templates::{ENV_DETAILS, SYSTEM_PROMPT};
use self::tools::access_mcp_resource::AccessMcpResourceTool;
use self::tools::ask_followup_question::AskFollowupQuestionTool;
use self::tools::attempt_completion::AttemptCompletionTool;
use self::tools::execute_command::ExecuteCommandTool;
use self::tools::list_code_definition_names::ListCodeDefinitionNamesTool;
use self::tools::list_files::ListFilesTool;
use self::tools::read_file::ReadFileTool;
use self::tools::replace_in_file::ReplaceInFileTool;
use self::tools::search_files::SearchFilesTool;
use self::tools::use_mcp_tool::UseMcpTool;
use self::tools::write_to_file::WriteToFileTool;
mod providers;
mod templates;
mod tools;

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

async fn prepare_system_prompt(workspace_dir: &str, user_instructions: &str) -> String {
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

fn is_ignored(entry: &DirEntry) -> bool {
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

fn add_env_message<'a>(msg: &'a mut Message, workspace_dir: &'a str) {
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

fn log_last_message(messages: &Vec<Message>) {
    let Some(last_message) = messages.last() else {
        return;
    };
    match last_message {
        Message::User { content } => {
            println!("==== User ====");
            for content in content.iter() {
                match content {
                    UserContent::ToolResult(tool_result) => {
                        println!("Tool result: {}", tool_result.id);
                    }
                    UserContent::Text(text) => {
                        println!(
                            "Text: {}",
                            text.text.chars().into_iter().take(100).collect::<String>()
                        );
                    }
                    UserContent::Audio(audio) => {
                        println!("Audio: {:?}", audio.media_type);
                    }
                    UserContent::Document(doc) => {
                        println!("Document: {:?}", doc.media_type);
                    }
                    UserContent::Image(img) => {
                        println!("Image: {:?}", img.media_type);
                    }
                }
            }
        }
        Message::Assistant { content } => {
            println!("==== Assistant ====");
            for content in content.iter() {
                match content {
                    AssistantContent::ToolCall(tool_call) => {
                        println!("Tool call: {}", tool_call.function.name);
                    }
                    AssistantContent::Text(text) => {
                        println!(
                            "Text: {}",
                            text.text.chars().into_iter().take(100).collect::<String>()
                        );
                    }
                }
            }
        }
    }
}

fn persist_history(messages: &Vec<Message>) {
    fs::write(
        "history.json",
        serde_json::to_string_pretty(messages).unwrap(),
    )
    .unwrap();
}

fn load_history() -> Vec<Message> {
    let history = fs::read_to_string("history.json").unwrap();
    serde_json::from_str(&history).unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();
    let api_key = env::var("OPEN_ROUTER_API_KEY").expect("Env key 'OPEN_ROUTER_API_KEY' not found");
    //let workspace_dir = env::var("WORKSPACE_DIR").expect("Env key 'WORKSPACE_DIR' not found");
    let workspace_dir = "D:\\work\\test4".to_string();

    let user_instructions = indoc! {"
        You are dedicated software engineer working alone. You’re free to choose any technology, \
        approach, and solution. If in doubt please choose the best way you think. \
        Your goal is to build working software based on user request."
    };

    let client = openrouter::Client::new(&api_key);
    let mut messages: Vec<Message> = Vec::default();
    let system_prompt = prepare_system_prompt(&workspace_dir, user_instructions).await;
    let agent = client
        //.agent("qwen/qwen2.5-vl-32b-instruct:free")
        .agent("anthropic/claude-3.5-sonnet")
        .preamble(&system_prompt)
        .tool(ReadFileTool::new(&workspace_dir))
        .tool(ListFilesTool::new(&workspace_dir))
        .tool(WriteToFileTool::new(&workspace_dir))
        .tool(ExecuteCommandTool::new(&workspace_dir))
        .tool(ListCodeDefinitionNamesTool::new(&workspace_dir))
        .tool(ReplaceInFileTool::new(&workspace_dir))
        .tool(SearchFilesTool::new(&workspace_dir))
        .tool(AccessMcpResourceTool)
        .tool(UseMcpTool)
        .tool(AskFollowupQuestionTool)
        .tool(AttemptCompletionTool)
        .temperature(0.0)
        .build();
    let mut count = 0;
    log_last_message(&messages);
    let mut has_completion = false;
    let mut message = Message::user("add npm and build the project");
    while count < 5 && !has_completion {
        count += 1;
        add_env_message(&mut message, &workspace_dir);
        let mut stream = agent
            .stream_completion(message.clone(), messages.clone())
            .await?
            .stream()
            .await?;
        messages.push(message.clone());
        let mut assistant_content = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(AssistantContent::Text(text)) => {
                    assistant_content.push_str(&text.text);
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
                Ok(AssistantContent::ToolCall(tool_call)) => {
                    if !assistant_content.is_empty() {
                        messages.push(Message::assistant(&assistant_content));
                        assistant_content.clear();
                        log_last_message(&messages);
                    }
                    messages.push(Message::Assistant {
                        content: OneOrMany::one(AssistantContent::ToolCall(tool_call.clone())),
                    });
                    log_last_message(&messages);
                    let res = agent
                        .tools
                        .call(
                            &tool_call.function.name,
                            tool_call.function.arguments.to_string(),
                        )
                        .await
                        .map_err(|e| std::io::Error::other(e.to_string()))?;
                    message = Message::User {
                        content: OneOrMany::one(UserContent::tool_result(
                            tool_call.id,
                            OneOrMany::one(ToolResultContent::text(res)),
                        )),
                    };
                    add_env_message(&mut message, &workspace_dir);
                    log_last_message(&messages);
                    if tool_call.function.name == AttemptCompletionTool::NAME {
                        has_completion = true;
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    break;
                }
            }
        }
        if !assistant_content.is_empty() {
            messages.push(Message::assistant(&assistant_content));
            log_last_message(&messages);
        }

        persist_history(&messages);
    }
    Ok(())
}
