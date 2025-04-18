use std::collections::HashMap;
use std::env;
use std::error::Error;

use indoc::indoc;
use rig::completion::Completion;
use rig::message::{AssistantContent, Message, UserContent};
use rig::providers::openrouter;
use walkdir::DirEntry;

use self::templates::TOOL_CALL_ERROR;
use self::templates::TOOL_USAGE_ERROR;
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
use self::tools::ClineTool;
use self::tools::ClineToolDyn;
use self::tools::ToolCallIterator;
mod templates;
mod tools;

fn get_shell_path() -> String {
    if let Ok(shell) = std::env::var("SHELL") {
        shell
    } else if let Ok(comspec) = std::env::var("COMSPEC") {
        comspec
    } else {
        panic!("Could not determine shell path from environment variables")
    }
}

async fn prepare_system_prompt(
    workspace_dir: &str,
    tools: &HashMap<String, Box<dyn ClineToolDyn>>,
    user_instructions: &str,
) -> String {
    let mut tools_str = String::new();
    for tool in tools.values() {
        let definition = tool.definition("".to_string()).await;
        let mut tool_parameters = String::new();
        let parameters = definition.parameters.as_object().unwrap();
        for (name, param) in parameters.get("properties").unwrap().as_object().unwrap() {
            let required_prefix = if parameters["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v.as_str().unwrap() == name)
            {
                "required"
            } else {
                "optional"
            };
            tool_parameters = format!(
                "{}\n- {}: ({}) {}",
                tool_parameters,
                name,
                required_prefix,
                param["description"].as_str().unwrap()
            );
        }
        tools_str = format!(
            "## {}\nDescription: {}\nParameters:{}\nUsage:\n{}\n\n",
            tool.name(),
            definition.description,
            tool_parameters,
            tool.usage()
        );
    }

    subst::substitute(
        SYSTEM_PROMPT,
        &HashMap::from([
            ("WORKSPACE_DIR", workspace_dir),
            ("TOOLS", &tools_str),
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

fn prepare_env_message(workspace_dir: &str) -> Message {
    let mut files: Vec<String> = Vec::default();

    for entry in walkdir::WalkDir::new(workspace_dir)
        .follow_links(false)
        .same_file_system(true)
        .into_iter()
        .filter_entry(|e| !is_ignored(e))
        .filter_map(|e| e.ok())
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
    Message::user(
        subst::substitute(
            ENV_DETAILS,
            &HashMap::from([
                ("TIME", chrono::Local::now().to_rfc2822().as_str()),
                ("WORKING_DIR", workspace_dir),
                ("FILES", files),
            ]),
        )
        .unwrap(),
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_target(false)
        .init();
    let api_key = env::var("OPEN_ROUTER_API_KEY").expect("Env key 'OPEN_ROUTER_API_KEY' not found");
    let workspace_dir = env::var("WORKSPACE_DIR").expect("Env key 'WORKSPACE_DIR' not found");

    let user_instructions = indoc! {"
        You are dedicated software engineer working alone. You’re free to choose any technology, \
        approach, and solution. If in doubt please choose the best way you think. \
        Your goal is to build working software based on user request."
    };
    let mut tools = HashMap::<String, Box<dyn ClineToolDyn>>::new();
    tools.insert(
        ExecuteCommandTool::NAME.to_string(),
        Box::new(ExecuteCommandTool::new(&workspace_dir)),
    );
    tools.insert(
        ReadFileTool::NAME.to_string(),
        Box::new(ReadFileTool::new(&workspace_dir)),
    );
    tools.insert(
        WriteToFileTool::NAME.to_string(),
        Box::new(WriteToFileTool::new(&workspace_dir)),
    );
    tools.insert(
        ReplaceInFileTool::NAME.to_string(),
        Box::new(ReplaceInFileTool::new(&workspace_dir)),
    );
    tools.insert(
        AccessMcpResourceTool::NAME.to_string(),
        Box::new(AccessMcpResourceTool),
    );
    tools.insert(
        AskFollowupQuestionTool::NAME.to_string(),
        Box::new(AskFollowupQuestionTool),
    );
    tools.insert(
        AttemptCompletionTool::NAME.to_string(),
        Box::new(AttemptCompletionTool),
    );
    tools.insert(
        ListCodeDefinitionNamesTool::NAME.to_string(),
        Box::new(ListCodeDefinitionNamesTool::new(&workspace_dir)),
    );
    tools.insert(
        ListFilesTool::NAME.to_string(),
        Box::new(ListFilesTool::new(&workspace_dir)),
    );
    tools.insert(
        SearchFilesTool::NAME.to_string(),
        Box::new(SearchFilesTool::new(&workspace_dir)),
    );
    tools.insert(UseMcpTool::NAME.to_string(), Box::new(UseMcpTool));

    let client = openrouter::Client::new(&api_key);
    let mut messages: Vec<Message> = Vec::default();
    let system_prompt = prepare_system_prompt(&workspace_dir, &tools, user_instructions).await;
    let agent = client
        //.agent("qwen/qwen2.5-vl-32b-instruct:free")
        .agent("anthropic/claude-3.5-sonnet")
        .preamble(&system_prompt)
        .temperature(0.0)
        .build();
    let mut count = 0;
    messages.push(Message::user("create Sokoban game"));
    let mut has_completion = false;
    while count < 10 && !has_completion {
        count += 1;
        let response = agent
            .completion(
                prepare_env_message(&workspace_dir), // last message always is env
                messages.clone(),
            )
            .await?
            .send()
            .await?;
        for content in response.choice {
            let str_content = match content {
                AssistantContent::Text(text) => text.text.clone(),
                _ => "".to_string(),
            };
            messages.push(Message::assistant(&str_content));
            messages.append(&mut process_tools(&str_content, &tools).await);
            if str_content.contains("<attempt_completion>") {
                has_completion = true;
            }
        }
    }
    for message in messages.iter() {
        match message {
            Message::User { content } => {
                println!("==== User ====");
                for content in content.iter() {
                    if let UserContent::Text(text) = content {
                        println!("{}", text.text);
                    }
                }
            }
            Message::Assistant { content } => {
                println!("==== Assistant ====");
                for content in content.iter() {
                    if let AssistantContent::Text(text) = content {
                        println!("{}", text.text);
                    }
                }
            }
        }
    }
    Ok(())
}

async fn process_tools(text: &str, tools: &HashMap<String, Box<dyn ClineToolDyn>>) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::default();
    let mut tool_used = false;
    let tool_call_iterator = ToolCallIterator::new(text, tools.keys().cloned().collect());
    for (tool_name, params) in tool_call_iterator {
        if tool_used {
            messages.push(Message::user(
                subst::substitute(
                    TOOL_USAGE_ERROR,
                    &HashMap::from([("TOOL_NAME", &tool_name)]),
                )
                .unwrap(),
            ));
        } else if let Some(tool) = tools.get(&tool_name) {
            tool_used = true;
            // TODO: get from tool
            let main_arg = params
                .get("path")
                .or(params.get("command"))
                .or(params.get("server_name"))
                .unwrap();
            messages.push(Message::user(format!(
                "[{tool_name} for '{main_arg}'] Result:"
            )));
            match tool.call(params).await {
                Ok(result) => {
                    messages.push(Message::user(result));
                }
                Err(e) => {
                    messages.push(Message::user(
                        subst::substitute(
                            TOOL_CALL_ERROR,
                            &HashMap::from([("ERROR", &format!("{e}"))]),
                        )
                        .unwrap(),
                    ));
                }
            }
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_file() {
        let txt = "some test message:\n\n<thinking>\n1. some think block.\n</thinking>\n\n<read_file>\n<path>cline-agent.d</path>\n\n</read_file>\n<thinking>\nanother think block.\n</thinking>";
        let mut tools = HashMap::<String, Box<dyn ClineToolDyn>>::new();
        tools.insert(
            ReadFileTool::NAME.to_string(),
            Box::new(ReadFileTool::new("./target/debug")),
        );
        let messages = process_tools(txt, &tools).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0],
            Message::user("[read_file for 'cline-agent.d'] Result:")
        );
    }

    #[tokio::test]
    async fn test_two_tools_error() {
        let txt = "some test message:\n\n<thinking>\n1. some think block.\n</thinking>\n\n<read_file>\n<path>cline-agent.d</path>\n\n</read_file>\n\n<read_file>\n<path>cline-agent.d</path>\n\n</read_file>\n<thinking>\nanother think block.\n</thinking>";
        let mut tools = HashMap::<String, Box<dyn ClineToolDyn>>::new();
        tools.insert(
            ReadFileTool::NAME.to_string(),
            Box::new(ReadFileTool::new("./target/debug")),
        );
        let messages = process_tools(txt, &tools).await;
        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages[0],
            Message::user("[read_file for 'cline-agent.d'] Result:")
        );
        assert_eq!(
            messages[2],
            Message::user("Tool [read_file] was not executed because a tool has already been used in this message. Only one tool may be used per message. You must assess the first tool's result before proceeding to use the next tool.")
        );
    }
}
