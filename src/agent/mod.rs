use crate::config::ProviderKind;
use crate::providers::HulyAgent;
use crate::tools::access_mcp_resource::AccessMcpResourceTool;
use crate::tools::ask_followup_question::AskFollowupQuestionTool;
use crate::tools::attempt_completion::AttemptCompletionTool;
use crate::tools::execute_command::ExecuteCommandTool;
use crate::tools::list_code_definition_names::ListCodeDefinitionNamesTool;
use crate::tools::list_files::ListFilesTool;
use crate::tools::read_file::ReadFileTool;
use crate::tools::replace_in_file::ReplaceInFileTool;
use crate::tools::search_files::SearchFilesTool;
use crate::tools::use_mcp_tool::UseMcpTool;
use crate::tools::write_to_file::WriteToFileTool;
use crate::Config;
use futures::StreamExt;
use rig::completion::CompletionError;
use rig::completion::CompletionResponse;
use rig::message::AssistantContent;
use rig::message::Message;
use rig::message::ToolResultContent;
use rig::message::UserContent;
use rig::streaming::StreamingCompletionResponse;
use rig::tool::Tool;
use rig::tool::ToolSetError;
use rig::OneOrMany;
use tokio::sync::mpsc;

pub mod event;
mod utils;
pub use event::AgentControlEvent;
pub use event::AgentOutputEvent;
pub use utils::is_ignored;

use self::event::AgentCommandStatus;
use self::event::AgentTaskStatus;
use self::utils::*;

pub struct Agent {
    config: Config,
    processing: bool,
    receiver: mpsc::UnboundedReceiver<AgentControlEvent>,
    sender: mpsc::UnboundedSender<AgentOutputEvent>,
    agent: Option<Box<dyn HulyAgent>>,
    messages: Vec<Message>,
    stream:
        Option<StreamingCompletionResponse<rig::providers::openai::StreamingCompletionResponse>>,
    assistant_content: Option<String>,
    has_completion: bool,
    pending_tool_id: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("ToolSetError: {0}")]
    ToolSetError(#[from] ToolSetError),
    #[error("CompletionError: {0}")]
    CompletionError(#[from] CompletionError),
}

impl Agent {
    pub fn new(
        config: Config,
        receiver: mpsc::UnboundedReceiver<AgentControlEvent>,
        sender: mpsc::UnboundedSender<AgentOutputEvent>,
    ) -> Self {
        Self {
            config,
            receiver,
            sender,
            processing: false,
            agent: None,
            messages: Vec::new(),
            stream: None,
            assistant_content: None,
            has_completion: false,
            pending_tool_id: None,
        }
    }

    fn build_agent(config: &Config, system_prompt: String) -> Box<dyn HulyAgent> {
        match config.provider {
            ProviderKind::OpenAI => Box::new(
                rig::providers::openai::Client::from_env()
                    .agent(&config.model)
                    .preamble(&system_prompt)
                    .tool(ReadFileTool::new(&config.workspace))
                    .tool(ListFilesTool::new(&config.workspace))
                    .tool(WriteToFileTool::new(&config.workspace))
                    .tool(ExecuteCommandTool::new(&config.workspace))
                    .tool(ListCodeDefinitionNamesTool::new(&config.workspace))
                    .tool(ReplaceInFileTool::new(&config.workspace))
                    .tool(SearchFilesTool::new(&config.workspace))
                    .tool(AccessMcpResourceTool)
                    .tool(UseMcpTool)
                    .tool(AskFollowupQuestionTool)
                    .tool(AttemptCompletionTool)
                    .temperature(0.0)
                    .build(),
            ),
            ProviderKind::OpenRouter => Box::new(
                crate::providers::openrouter::Client::from_env()
                    .agent(&config.model)
                    .preamble(&system_prompt)
                    .tool(ReadFileTool::new(&config.workspace))
                    .tool(ListFilesTool::new(&config.workspace))
                    .tool(WriteToFileTool::new(&config.workspace))
                    .tool(ExecuteCommandTool::new(&config.workspace))
                    .tool(ListCodeDefinitionNamesTool::new(&config.workspace))
                    .tool(ReplaceInFileTool::new(&config.workspace))
                    .tool(SearchFilesTool::new(&config.workspace))
                    .tool(AccessMcpResourceTool)
                    .tool(UseMcpTool)
                    .tool(AskFollowupQuestionTool)
                    .tool(AttemptCompletionTool)
                    .temperature(0.0)
                    .build(),
            ),
            ProviderKind::LMStudio => Box::new(
                rig::providers::openai::Client::from_url("", "http://127.0.0.1:1234/v1")
                    .agent(&config.model)
                    .preamble(&system_prompt)
                    .tool(ReadFileTool::new(&config.workspace))
                    .tool(ListFilesTool::new(&config.workspace))
                    .tool(WriteToFileTool::new(&config.workspace))
                    .tool(ExecuteCommandTool::new(&config.workspace))
                    .tool(ListCodeDefinitionNamesTool::new(&config.workspace))
                    .tool(ReplaceInFileTool::new(&config.workspace))
                    .tool(SearchFilesTool::new(&config.workspace))
                    .tool(AccessMcpResourceTool)
                    .tool(UseMcpTool)
                    .tool(AskFollowupQuestionTool)
                    .tool(AttemptCompletionTool)
                    .temperature(0.0)
                    .build(),
            ),
        }
    }

    fn add_message(&mut self, message: Message) {
        self.sender
            .send(AgentOutputEvent::AddMessage(message.clone()))
            .unwrap();
        self.messages.push(message);
    }

    fn update_last_message(&mut self, message: Message) {
        let last_idx = self.messages.len() - 1;
        self.sender
            .send(AgentOutputEvent::UpdateMessage(message.clone()))
            .unwrap();
        self.messages[last_idx] = message;
    }

    async fn process_messages(&mut self) -> Result<(), AgentError> {
        if !self.processing {
            return Ok(());
        }
        let Some(agent) = self.agent.as_mut() else {
            return Ok(());
        };

        if self.stream.is_none() && !self.messages.is_empty() {
            self.stream = Some(
                agent
                    .send_messages(
                        self.messages.last().unwrap().clone(),
                        self.messages[..self.messages.len() - 1].to_vec(),
                    )
                    .await?,
            );
        }

        let Some(stream) = self.stream.as_mut() else {
            return Ok(());
        };

        match stream.next().await {
            Some(result) => match result {
                Ok(AssistantContent::Text(text)) => {
                    if self.assistant_content.is_none() {
                        self.assistant_content = Some(text.text.clone());
                        self.add_message(Message::assistant(text.text.clone()));
                    } else {
                        self.assistant_content
                            .as_mut()
                            .unwrap()
                            .push_str(&text.text);
                        self.update_last_message(Message::assistant(
                            self.assistant_content.as_ref().unwrap(),
                        ));
                    }
                }
                Ok(AssistantContent::ToolCall(tool_call)) => {
                    tracing::info!("Tool call: {}", tool_call.function.name);
                    self.assistant_content = None;
                    self.add_message(Message::Assistant {
                        content: OneOrMany::one(AssistantContent::ToolCall(tool_call.clone())),
                    });
                    let (tool_result, is_error) = match self
                        .agent
                        .as_mut()
                        .unwrap()
                        .tools()
                        .call(
                            &tool_call.function.name,
                            tool_call.function.arguments.to_string(),
                        )
                        .await
                    {
                        Ok(tool_json_result) => {
                            // TODO: currently all tools return a string, but this should be more flexible
                            (serde_json::from_str(&tool_json_result).unwrap(), false)
                        }
                        Err(e) => {
                            tracing::error!("Error calling tool: {}", e);
                            (format!("Tool called with error: {}", e), true)
                        }
                    };

                    if tool_call.function.name == ExecuteCommandTool::NAME {
                        let command = tool_call.function.arguments.as_object().unwrap()["command"]
                            .as_str()
                            .unwrap();
                        self.sender
                            .send(AgentOutputEvent::ExecuteCommand(AgentCommandStatus {
                                command: command.to_string(),
                                output: tool_result.clone(),
                            }))
                            .unwrap();
                    }
                    tracing::info!("tool_result: '{}'", tool_result);
                    if tool_result.is_empty() {
                        tracing::info!(
                            "Stop processing because empty result from tool: {}",
                            tool_call.function.name
                        );
                        self.pending_tool_id = Some(tool_call.id);
                        self.processing = false;
                    } else if !is_error {
                        match tool_call.function.name.as_str() {
                            ReadFileTool::NAME
                            | WriteToFileTool::NAME
                            | ListFilesTool::NAME
                            | ReplaceInFileTool::NAME => {
                                if let Some(path) = tool_call
                                    .function
                                    .arguments
                                    .as_object()
                                    .unwrap()
                                    .get("path")
                                {
                                    self.sender
                                        .send(AgentOutputEvent::HighlightFile(
                                            path.as_str().unwrap().to_string(),
                                            tool_call.function.name == WriteToFileTool::NAME,
                                        ))
                                        .unwrap();
                                }
                            }
                            _ => {}
                        }
                        let mut result_message = Message::User {
                            content: OneOrMany::one(UserContent::tool_result(
                                tool_call.id,
                                OneOrMany::one(ToolResultContent::text(tool_result)),
                            )),
                        };
                        add_env_message(&mut result_message, &self.config.workspace);
                        self.add_message(result_message);
                        if tool_call.function.name == AttemptCompletionTool::NAME {
                            self.has_completion = true;
                            tracing::info!("Stop task with success");
                        }
                    }
                }
                Err(e) => {
                    self.stream = None;
                    return Err(e.into());
                }
            },
            None => {
                let response: CompletionResponse<
                    Option<rig::providers::openai::StreamingCompletionResponse>,
                > = From::from(self.stream.take().unwrap());
                let usage = response.raw_response.unwrap().usage;
                tracing::info!("Usage: {:?}", usage);
                self.sender
                    .send(AgentOutputEvent::TaskStatus(AgentTaskStatus {
                        current_tokens: usage.total_tokens as u32,
                        max_tokens: 1,
                    }))
                    .unwrap();
                self.assistant_content = None;
                if self.has_completion {
                    self.pending_tool_id = None;
                    self.processing = false;
                }
                tracing::debug!("persist_history");
                persist_history(&self.messages);
            }
        }
        Ok(())
    }

    pub async fn run(&mut self) {
        tracing::info!(
            "Run agent: {:?} : {}",
            self.config.provider,
            self.config.model
        );
        let system_prompt =
            prepare_system_prompt(&self.config.workspace, &self.config.user_instructions).await;
        self.agent = Some(Self::build_agent(&self.config, system_prompt));
        while !self.sender.is_closed() {
            if let Ok(event) = self.receiver.try_recv() {
                match event {
                    AgentControlEvent::SendMessage(message) => {
                        tracing::info!("Send message: {}", message);
                        self.send_message(message);
                    }
                    AgentControlEvent::CancelTask => {
                        tracing::info!("Cancel current task");
                        self.cancel_task();
                    }
                }
            }
            if let Err(e) = self.process_messages().await {
                tracing::debug!("persist_history");
                persist_history(&self.messages);
                self.sender
                    .send(AgentOutputEvent::Error(format!(
                        "Send message error: {}",
                        e
                    )))
                    .ok();
                tracing::error!("Error processing messages: {}", e);
                self.processing = false;
                self.has_completion = false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        tracing::info!("Stop agent");
    }

    fn send_message(&mut self, message: String) {
        let mut message = if let Some(tool_id) = self.pending_tool_id.take() {
            Message::User {
                content: OneOrMany::one(UserContent::tool_result(
                    tool_id,
                    OneOrMany::one(ToolResultContent::text(message)),
                )),
            }
        } else {
            Message::user(message)
        };
        add_env_message(&mut message, &self.config.workspace);
        self.add_message(message);
        self.processing = true;
        self.has_completion = false;
    }

    fn cancel_task(&mut self) {
        self.processing = false;
        self.has_completion = false;
    }
}
