use std::sync::Arc;
use std::sync::RwLock;

use crate::config::McpClientTransport;
use crate::config::McpConfig;
use crate::config::ProviderKind;
use crate::providers::HulyAgent;
use crate::tools::ask_followup_question::AskFollowupQuestionTool;
use crate::tools::attempt_completion::AttemptCompletionTool;
use crate::tools::execute_command::ExecuteCommandTool;
use crate::tools::list_files::ListFilesTool;
use crate::tools::memory;
use crate::tools::memory::Entity;
use crate::tools::memory::MemoryManager;
use crate::tools::read_file::ReadFileTool;
use crate::tools::replace_in_file::ReplaceInFileTool;
use crate::tools::search_files::SearchFilesTool;
use crate::tools::web_fetch::WebFetchTool;
use crate::tools::web_search::WebSearchTool;
use crate::tools::write_to_file::WriteToFileTool;
use crate::Config;
use anyhow::Result;
use futures::StreamExt;
use mcp_core::types::ProtocolVersion;
use rig::agent::AgentBuilder;
use rig::completion::CompletionError;
use rig::completion::CompletionModel;
use rig::completion::CompletionResponse;
use rig::embeddings::EmbeddingsBuilder;
use rig::message::AssistantContent;
use rig::message::Message;
use rig::message::ToolResultContent;
use rig::message::UserContent;
use rig::streaming::StreamingCompletionResponse;
use rig::tool::Tool;
use rig::tool::ToolError;
use rig::tool::ToolSetError;
use rig::vector_store::in_memory_store::InMemoryVectorIndex;
use rig::vector_store::in_memory_store::InMemoryVectorStore;
use rig::OneOrMany;
use tokio::sync::mpsc;

pub mod event;
pub mod utils;
pub use event::AgentControlEvent;
pub use event::AgentOutputEvent;

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
    memory: Arc<RwLock<MemoryManager>>,
    memory_index: Option<InMemoryVectorIndex<rig_fastembed::EmbeddingModel, Entity>>,
    has_completion: bool,
    pending_tool_id: Option<String>,
    current_tokens: u32,
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
        messages: Vec<Message>,
    ) -> Self {
        Self {
            config,
            receiver,
            sender,
            processing: false,
            agent: None,
            messages,
            stream: None,
            assistant_content: None,
            has_completion: false,
            pending_tool_id: None,
            current_tokens: 0,
            memory: Arc::new(RwLock::new(MemoryManager::new(false))),
            memory_index: None,
        }
    }
    async fn init_memory_index(&mut self) {
        let documents = self.memory.read().unwrap().entities().clone();
        let client = rig_fastembed::Client::new();
        let model = client.embedding_model(&rig_fastembed::FastembedModel::AllMiniLML6V2);
        let embeddings = EmbeddingsBuilder::new(model.clone())
            .documents(documents)
            .unwrap()
            .build()
            .await
            .unwrap();
        let index_store = InMemoryVectorStore::from_documents(embeddings.into_iter()).index(model);
        self.memory_index = Some(index_store);
    }

    fn add_static_tools<M>(
        agent_builder: AgentBuilder<M>,
        config: &Config,
        memory: Arc<RwLock<MemoryManager>>,
    ) -> AgentBuilder<M>
    where
        M: CompletionModel,
    {
        let mut agent_builder = agent_builder
            .tool(ReadFileTool::new(config.workspace.to_path_buf()))
            .tool(ListFilesTool::new(config.workspace.to_path_buf()))
            .tool(WriteToFileTool::new(config.workspace.to_path_buf()))
            .tool(ExecuteCommandTool::new(config.workspace.to_path_buf()))
            .tool(ReplaceInFileTool::new(config.workspace.to_path_buf()))
            .tool(SearchFilesTool::new(config.workspace.to_path_buf()))
            .tool(AskFollowupQuestionTool)
            .tool(AttemptCompletionTool);
        if let Some(web_search) = config.web_search.as_ref() {
            agent_builder = agent_builder.tool(WebSearchTool::new(web_search.clone()));
        }
        if let Some(web_fetch) = config.web_fetch.as_ref() {
            agent_builder = agent_builder.tool(WebFetchTool::new(web_fetch.clone()).unwrap());
        }
        agent_builder = memory::add_memory_tools(agent_builder, memory);

        agent_builder
    }

    async fn add_mcp_tools<M>(
        mut agent_builder: AgentBuilder<M>,
        mcp: Option<&McpConfig>,
    ) -> Result<AgentBuilder<M>>
    where
        M: CompletionModel,
    {
        let Some(mcp_config) = mcp else {
            return Ok(agent_builder);
        };

        for server_config in mcp_config.servers.values() {
            match server_config {
                McpClientTransport::Stdio(config) => {
                    let transport = mcp_core::transport::ClientStdioTransport::new(
                        &config.command,
                        &config.args.iter().map(String::as_str).collect::<Vec<_>>(),
                    )?;
                    let mcp_client = mcp_core::client::ClientBuilder::new(transport)
                        .set_protocol_version(
                            config
                                .protocol_version
                                .clone()
                                .unwrap_or(ProtocolVersion::V2025_03_26),
                        )
                        .build();
                    mcp_client.open().await?;
                    mcp_client.initialize().await?;
                    let tools_list_res = mcp_client.list_tools(None, None).await?;

                    agent_builder = tools_list_res
                        .tools
                        .into_iter()
                        .fold(agent_builder, |builder, tool| {
                            builder.mcp_tool(tool, mcp_client.clone())
                        })
                }
                McpClientTransport::Sse(config) => {
                    let mut transport =
                        mcp_core::transport::ClientSseTransport::builder(config.url.clone());
                    if let Some(bearer_token) = &config.bearer_token {
                        transport = transport.with_bearer_token(bearer_token.clone());
                    }
                    let mcp_client = mcp_core::client::ClientBuilder::new(transport.build())
                        .set_protocol_version(
                            config
                                .protocol_version
                                .clone()
                                .unwrap_or(ProtocolVersion::V2025_03_26),
                        )
                        .build();
                    match mcp_client.open().await {
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("Failed to open MCP client: {}", e);
                            continue;
                        }
                    }
                    mcp_client.initialize().await?;
                    let tools_list_res = mcp_client.list_tools(None, None).await?;

                    agent_builder = tools_list_res
                        .tools
                        .into_iter()
                        .fold(agent_builder, |builder, tool| {
                            builder.mcp_tool(tool, mcp_client.clone())
                        })
                }
            }
        }
        Ok(agent_builder)
    }

    async fn build_agent(
        config: &Config,
        memory: Arc<RwLock<MemoryManager>>,
        system_prompt: String,
    ) -> Result<Box<dyn HulyAgent>> {
        match config.provider {
            ProviderKind::OpenAI => {
                let mut agent_builder = rig::providers::openai::Client::new(
                    &config
                        .provider_api_key
                        .clone()
                        .expect("provider_api_key is required for OpenAI"),
                )
                .agent(&config.model);
                agent_builder = agent_builder.preamble(&system_prompt).temperature(0.0);
                agent_builder = Self::add_static_tools(agent_builder, config, memory);
                agent_builder = Self::add_mcp_tools(agent_builder, config.mcp.as_ref()).await?;
                Ok(Box::new(agent_builder.build()))
            }
            ProviderKind::OpenRouter => {
                let mut agent_builder = crate::providers::openrouter::Client::new(
                    &config
                        .provider_api_key
                        .clone()
                        .expect("provider_api_key is required for OpenRouter"),
                )
                .agent(&config.model);
                agent_builder = agent_builder.preamble(&system_prompt).temperature(0.0);
                agent_builder = Self::add_static_tools(agent_builder, config, memory);
                agent_builder = Self::add_mcp_tools(agent_builder, config.mcp.as_ref()).await?;
                Ok(Box::new(agent_builder.build()))
            }
            ProviderKind::LMStudio => {
                let mut agent_builder = rig::providers::openai::Client::from_url(
                    "",
                    &config
                        .provider_base_url
                        .clone()
                        .unwrap_or("http://127.0.0.1:1234/v1".to_string()),
                )
                .agent(&config.model);
                agent_builder = agent_builder.preamble(&system_prompt).temperature(0.0);
                agent_builder = Self::add_static_tools(agent_builder, config, memory);
                agent_builder = Self::add_mcp_tools(agent_builder, config.mcp.as_ref()).await?;
                Ok(Box::new(agent_builder.build()))
            }
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
            self.processing = false;
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
            self.processing = false;
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
                            if tool_json_result.starts_with("\"") {
                                // TODO: currently all tools return a string, but this should be more flexible
                                (serde_json::from_str(&tool_json_result).unwrap(), false)
                            } else {
                                // raw string response
                                (tool_json_result, false)
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error calling tool: {}", e);
                            match e {
                                ToolSetError::ToolCallError(tce) => {
                                    match tce {
                                        ToolError::ToolCallError(ce) => {
                                            (format!("The tool execution failed with the following error: <error>{}</error>", ce), true)
                                        }
                                        _ => (format!("The tool execution failed with the following error: <error>{}</error>", tce), true),
                                    }
                                }
                                _ => (format!("The tool execution failed with the following error: <error>{}</error>", e), true),
                            }
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
                    tracing::trace!("tool_result: '{}'", tool_result);
                    if tool_result.is_empty()
                        && tool_call.function.name != AttemptCompletionTool::NAME
                    {
                        tracing::info!(
                            "Stop processing because empty result from tool: {}",
                            tool_call.function.name
                        );
                        self.pending_tool_id = Some(tool_call.id);
                        self.processing = false;
                    } else {
                        if !is_error {
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
                        }
                        let mut result_message = Message::User {
                            content: OneOrMany::one(UserContent::tool_result(
                                tool_call.id,
                                OneOrMany::one(ToolResultContent::text(tool_result)),
                            )),
                        };
                        add_env_message(&mut result_message, None, &self.config.workspace).await;
                        self.add_message(result_message);
                        if tool_call.function.name == AttemptCompletionTool::NAME {
                            self.has_completion = true;
                            tracing::info!("Stop task with success");
                        }
                    }
                }
                Err(e) => {
                    self.processing = false;
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
                self.current_tokens = usage.total_tokens as u32;
                self.assistant_content = None;
                if self.has_completion {
                    self.pending_tool_id = None;
                    self.processing = false;
                } else if self
                    .messages
                    .last()
                    .is_some_and(|message| !matches!(message, Message::User { .. }))
                {
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
        self.agent = Some(
            Self::build_agent(&self.config, self.memory.clone(), system_prompt)
                .await
                .unwrap(),
        );
        self.init_memory_index().await;
        while !self.sender.is_closed() {
            if let Ok(event) = self.receiver.try_recv() {
                match event {
                    AgentControlEvent::SendMessage(message) => {
                        tracing::info!("Send message: {}", message);
                        self.send_message(message).await;
                    }
                    AgentControlEvent::CancelTask => {
                        tracing::info!("Cancel current task");
                        if self.processing {
                            self.cancel_task();
                        } else if !self.has_completion && !self.messages.is_empty() {
                            self.processing = true;
                        }
                    }
                    AgentControlEvent::NewTask => {
                        tracing::info!("New task");
                        self.messages.clear();
                        self.processing = false;
                        self.has_completion = false;
                        self.sender.send(AgentOutputEvent::NewTask).ok();
                        persist_history(&self.messages);
                    }
                }
            }
            let prev_processing = self.processing;
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
            if prev_processing != self.processing {
                self.sender
                    .send(AgentOutputEvent::TaskStatus(self.status()))
                    .ok();
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        tracing::info!("Stop agent");
    }

    async fn send_message(&mut self, message: String) {
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
        add_env_message(
            &mut message,
            self.memory_index.as_ref(),
            &self.config.workspace,
        )
        .await;
        self.add_message(message);
        self.processing = true;
        self.has_completion = false;
        self.sender
            .send(AgentOutputEvent::TaskStatus(self.status()))
            .ok();
    }

    fn status(&self) -> AgentTaskStatus {
        AgentTaskStatus {
            current_tokens: self.current_tokens,
            max_tokens: 1,
            processing: self.processing,
        }
    }

    fn cancel_task(&mut self) {
        self.processing = false;
        self.has_completion = false;
        self.sender
            .send(AgentOutputEvent::TaskStatus(self.status()))
            .ok();
    }
}
