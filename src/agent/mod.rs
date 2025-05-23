use std::fmt::Display;
// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::sync::Arc;

use crate::config::McpClientTransport;
use crate::config::McpConfig;
use crate::config::ProviderKind;
use crate::providers::HulyAgent;
use crate::tools::ask_followup_question::AskFollowupQuestionTool;
use crate::tools::attempt_completion::AttemptCompletionTool;
use crate::tools::execute_command::tools::ExecuteCommandTool;
use crate::tools::execute_command::tools::GetCommandResultTool;
use crate::tools::execute_command::tools::TerminateCommandTool;
use crate::tools::execute_command::ProcessRegistry;
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
use tokio::sync::RwLock;

use self::event::AgentState;
use self::event::AgentStatus;
use self::utils::*;

pub struct Agent {
    config: Config,
    receiver: mpsc::UnboundedReceiver<AgentControlEvent>,
    sender: mpsc::UnboundedSender<AgentOutputEvent>,
    agent: Option<Box<dyn HulyAgent>>,
    messages: Vec<Message>,
    stream:
        Option<StreamingCompletionResponse<rig::providers::openai::StreamingCompletionResponse>>,
    assistant_content: Option<String>,
    memory: Arc<RwLock<MemoryManager>>,
    memory_index: Option<InMemoryVectorIndex<rig_fastembed::EmbeddingModel, Entity>>,
    process_registry: Arc<RwLock<ProcessRegistry>>,
    current_tokens: u32,
    state: AgentState,
}

struct BuildAgentContext<'a> {
    config: &'a Config,
    memory: Arc<RwLock<MemoryManager>>,
    process_registry: Arc<RwLock<ProcessRegistry>>,
    system_prompt: String,
    sender: mpsc::UnboundedSender<AgentOutputEvent>,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    ToolSetError(#[from] ToolSetError),
    CompletionError(#[from] CompletionError),
}

impl Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ToolSetError(e) => write!(f, "{e}"),
            Self::CompletionError(e) => write!(f, "CompletionError: {e}"),
        }
    }
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
            agent: None,
            messages,
            stream: None,
            assistant_content: None,
            current_tokens: 0,
            memory: Arc::new(RwLock::new(MemoryManager::new(false))),
            process_registry: Arc::new(RwLock::new(ProcessRegistry::default())),
            memory_index: None,
            state: AgentState::Paused,
        }
    }

    pub async fn init_memory_index(&mut self) {
        let documents = self.memory.read().await.entities().clone();
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
        context: BuildAgentContext<'_>,
    ) -> AgentBuilder<M>
    where
        M: CompletionModel,
    {
        let mut agent_builder = agent_builder
            .tool(ReadFileTool::new(context.config.workspace.to_path_buf()))
            .tool(ListFilesTool::new(context.config.workspace.to_path_buf()))
            .tool(WriteToFileTool::new(context.config.workspace.to_path_buf()))
            .tool(ExecuteCommandTool::new(
                context.config.workspace.to_path_buf(),
                context.process_registry.clone(),
                context.sender.clone(),
            ))
            .tool(GetCommandResultTool::new(context.process_registry.clone()))
            .tool(TerminateCommandTool::new(context.process_registry.clone()))
            .tool(ReplaceInFileTool::new(
                context.config.workspace.to_path_buf(),
            ))
            .tool(SearchFilesTool::new(context.config.workspace.to_path_buf()))
            .tool(AskFollowupQuestionTool)
            .tool(AttemptCompletionTool);
        if let Some(web_search) = context.config.web_search.as_ref() {
            agent_builder = agent_builder.tool(WebSearchTool::new(web_search.clone()));
        }
        if let Some(web_fetch) = context.config.web_fetch.as_ref() {
            agent_builder = agent_builder.tool(WebFetchTool::new(web_fetch.clone()).unwrap());
        }
        agent_builder = memory::add_memory_tools(agent_builder, context.memory.clone());

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

    async fn configure_agent<M>(
        mut agent_builder: AgentBuilder<M>,
        context: BuildAgentContext<'_>,
    ) -> Result<AgentBuilder<M>>
    where
        M: CompletionModel,
    {
        agent_builder = agent_builder
            .preamble(&context.system_prompt)
            .temperature(0.0);
        let mcp_config = context.config.mcp.as_ref();
        agent_builder = Self::add_static_tools(agent_builder, context);
        agent_builder = Self::add_mcp_tools(agent_builder, mcp_config).await?;
        Ok(agent_builder)
    }

    async fn build_agent(context: BuildAgentContext<'_>) -> Result<Box<dyn HulyAgent>> {
        match context.config.provider {
            ProviderKind::OpenAI => {
                let agent_builder = rig::providers::openai::Client::new(
                    &context
                        .config
                        .provider_api_key
                        .clone()
                        .expect("provider_api_key is required for OpenAI"),
                )
                .agent(&context.config.model);
                Ok(Box::new(
                    Self::configure_agent(agent_builder, context).await?.build(),
                ))
            }
            ProviderKind::Anthropic => {
                let agent_builder = rig::providers::anthropic::ClientBuilder::new(
                    &context
                        .config
                        .provider_api_key
                        .clone()
                        .expect("provider_api_key is required for Anthropic"),
                )
                .build()
                .agent(&context.config.model)
                .max_tokens(20000);
                Ok(Box::new(
                    Self::configure_agent(agent_builder, context).await?.build(),
                ))
            }
            ProviderKind::OpenRouter => {
                let agent_builder = crate::providers::openrouter::Client::new(
                    &context
                        .config
                        .provider_api_key
                        .clone()
                        .expect("provider_api_key is required for OpenRouter"),
                )
                .agent(&context.config.model);
                Ok(Box::new(
                    Self::configure_agent(agent_builder, context).await?.build(),
                ))
            }
            ProviderKind::LMStudio => {
                let agent_builder = rig::providers::openai::Client::from_url(
                    "",
                    &context
                        .config
                        .provider_base_url
                        .clone()
                        .unwrap_or("http://127.0.0.1:1234/v1".to_string()),
                )
                .agent(&context.config.model);
                Ok(Box::new(
                    Self::configure_agent(agent_builder, context).await?.build(),
                ))
            }
        }
    }

    fn pending_tool_id(&self) -> Option<String> {
        self.messages.last().and_then(|message| match message {
            Message::User { .. } => None,
            Message::Assistant { content } => match content.first() {
                AssistantContent::Text(_) => None,
                AssistantContent::ToolCall(tool_call) => Some(tool_call.id.clone()),
            },
        })
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
        if self.state.is_paused() {
            return Ok(());
        }
        let Some(agent) = self.agent.as_mut() else {
            self.set_state(AgentState::Paused);
            return Ok(());
        };

        if self.stream.is_none() && is_last_user_message(&self.messages) {
            self.stream = Some(
                agent
                    .send_messages(
                        self.messages.last().unwrap().clone(),
                        self.messages[..self.messages.len() - 1].to_vec(),
                    )
                    .await?,
            );
            tracing::trace!("Sending messages to model: {:?}", self.messages.last());
            self.set_state(AgentState::WaitingResponse);
        }

        let Some(stream) = self.stream.as_mut() else {
            self.set_state(AgentState::WaitingUserPrompt);
            return Ok(());
        };

        match stream.next().await {
            Some(result) => {
                //tracing::trace!("Received response from model: {:?}", result);

                match result {
                    Ok(AssistantContent::Text(text)) => {
                        if matches!(self.state, AgentState::Thinking) {
                            self.set_state(AgentState::Thinking);
                        }
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
                        self.set_state(AgentState::ToolCall(
                            tool_call.function.name.clone(),
                            tool_call.function.arguments.clone(),
                        ));
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
                            Ok(tool_json_result) => (tool_json_result, false),
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

                        tracing::trace!("tool_result: '{}'", tool_result);
                        if (tool_result.is_empty() || tool_result == "\"\"")
                            && tool_call.function.name != AttemptCompletionTool::NAME
                        {
                            tracing::info!(
                                "Stop processing because empty result from tool: {}",
                                tool_call.function.name
                            );
                            self.set_state(AgentState::WaitingUserPrompt);
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
                                                    tool_call.function.name
                                                        == WriteToFileTool::NAME,
                                                ))
                                                .unwrap();
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            let mut result_message = Message::User {
                                content: OneOrMany::one(UserContent::tool_result(
                                    tool_call.id.clone(),
                                    OneOrMany::one(ToolResultContent::text(tool_result)),
                                )),
                            };
                            if tool_call.function.name == AttemptCompletionTool::NAME {
                                self.set_state(AgentState::Completed(false));
                                tracing::info!("Stop task with success");
                                persist_history(&self.messages);
                            } else {
                                add_env_message(
                                    &mut result_message,
                                    None,
                                    &self.config.workspace,
                                    self.process_registry.clone(),
                                )
                                .await;
                                self.add_message(result_message);
                            }
                        }
                    }
                    Err(e) => {
                        self.stream = None;
                        return Err(e.into());
                    }
                }
            }
            None => {
                let response: CompletionResponse<
                    Option<rig::providers::openai::StreamingCompletionResponse>,
                > = From::from(self.stream.take().unwrap());
                if let Some(raw_response) = response.raw_response {
                    let usage = raw_response.usage;
                    tracing::info!("Usage: {:?}", usage);
                    self.current_tokens = usage.total_tokens as u32;
                }
                self.assistant_content = None;
                if matches!(self.state, AgentState::Completed(false)) {
                    self.set_state(AgentState::Completed(true));
                } else if !is_last_user_message(&self.messages) && !self.state.is_completed() {
                    self.set_state(AgentState::WaitingUserPrompt);
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
            Self::build_agent(BuildAgentContext {
                config: &self.config,
                system_prompt,
                memory: self.memory.clone(),
                process_registry: self.process_registry.clone(),
                sender: self.sender.clone(),
            })
            .await
            .unwrap(),
        );
        // restore state from messages
        self.set_state(if self.messages.is_empty() {
            AgentState::WaitingUserPrompt
        } else {
            match self.messages.last().unwrap() {
                Message::User { .. } => AgentState::Paused,
                Message::Assistant { content } => match content.first() {
                    AssistantContent::Text(_) => AgentState::WaitingUserPrompt,
                    AssistantContent::ToolCall(tool_call) => {
                        if tool_call.function.name == AttemptCompletionTool::NAME {
                            AgentState::Completed(true)
                        } else {
                            AgentState::WaitingUserPrompt
                        }
                    }
                },
            }
        });
        while !self.sender.is_closed() {
            let modified_command_states = self.process_registry.write().await.poll();
            if !modified_command_states.is_empty() {
                self.sender
                    .send(AgentOutputEvent::CommandStatus(modified_command_states))
                    .ok();
            }

            if let Ok(event) = self.receiver.try_recv() {
                match event {
                    AgentControlEvent::SendMessage(message) => {
                        tracing::info!("Send message: {}", message);
                        self.send_message(message).await;
                    }
                    AgentControlEvent::CancelTask => {
                        tracing::info!("Cancel current task");
                        if !self.state.is_paused() {
                            self.set_state(AgentState::Paused);
                        } else if !self.state.is_completed() && !self.messages.is_empty() {
                            self.set_state(AgentState::WaitingResponse);
                        }
                    }
                    AgentControlEvent::NewTask => {
                        tracing::info!("New task");
                        self.messages.clear();
                        self.set_state(AgentState::WaitingUserPrompt);
                        self.sender.send(AgentOutputEvent::NewTask).ok();
                        persist_history(&self.messages);
                    }
                }
            }
            if let Err(e) = self.process_messages().await {
                tracing::debug!("persist_history");
                persist_history(&self.messages);
                tracing::error!("Error processing messages: {}", e);
                self.set_state(AgentState::Error(format!("{e}")));
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        tracing::info!("Stop agent");
    }

    async fn send_message(&mut self, message: String) {
        let mut message = if let Some(tool_id) = self.pending_tool_id() {
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
            self.process_registry.clone(),
        )
        .await;
        self.add_message(message);
        self.set_state(AgentState::WaitingResponse);
    }

    fn set_state(&mut self, state: AgentState) {
        tracing::info!("Agent state trasition: {}->{}", self.state, state);
        self.state = state;
        if !self.sender.is_closed() {
            self.sender
                .send(AgentOutputEvent::AgentStatus(AgentStatus {
                    current_tokens: self.current_tokens,
                    max_tokens: 1,
                    state: self.state.clone(),
                }))
                .unwrap();
        }
    }
}
