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
use anyhow::Context;
use anyhow::Result;
use futures::StreamExt;
use itertools::Itertools;
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
use tokio::sync::RwLockReadGuard;

use self::event::AgentState;
use self::utils::*;

pub struct Agent {
    config: Config,
    sender: mpsc::UnboundedSender<AgentOutputEvent>,
    memory: Arc<RwLock<MemoryManager>>,
    process_registry: Arc<RwLock<ProcessRegistry>>,
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

fn count_tokens(text: &str) -> u32 {
    text.len() as u32 / 4
}

fn pending_tool_id<'a>(messages: RwLockReadGuard<'a, Vec<Message>>) -> Option<String> {
    messages.last().and_then(|message| match message {
        Message::User { .. } => None,
        Message::Assistant { content } => match content.first() {
            AssistantContent::Text(_) => None,
            AssistantContent::ToolCall(tool_call) => Some(tool_call.id.clone()),
        },
    })
}

struct AgentContext {
    config: Config,
    messages: Arc<RwLock<Vec<Message>>>,
    state: Arc<RwLock<AgentState>>,
    sender: mpsc::UnboundedSender<AgentOutputEvent>,
    process_registry: Arc<RwLock<ProcessRegistry>>,
    memory_index: Arc<RwLock<InMemoryVectorIndex<rig_fastembed::EmbeddingModel, Entity>>>,
    system_prompt_token_count: u32,
    current_input_tokens: u32,
    current_completion_tokens: u32,
}

impl Agent {
    pub fn new(config: Config, sender: mpsc::UnboundedSender<AgentOutputEvent>) -> Self {
        Self {
            config,
            sender,
            memory: Arc::new(RwLock::new(MemoryManager::new(false))),
            process_registry: Arc::new(RwLock::new(ProcessRegistry::default())),
        }
    }

    pub async fn init_memory_index(
        &mut self,
    ) -> InMemoryVectorIndex<rig_fastembed::EmbeddingModel, Entity> {
        let documents = self.memory.read().await.entities().clone();
        let client = rig_fastembed::Client::new();
        let model = client.embedding_model(&rig_fastembed::FastembedModel::AllMiniLML6V2);
        let embeddings = EmbeddingsBuilder::new(model.clone())
            .documents(documents)
            .unwrap()
            .build()
            .await
            .unwrap();
        InMemoryVectorStore::from_documents(embeddings.into_iter()).index(model)
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
                    mcp_client
                        .open()
                        .await
                        .with_context(|| format!("Failed to open MCP client at {}", config.url))?;
                    mcp_client.initialize().await.with_context(|| {
                        format!("Failed initialize MCP client at {}", config.url)
                    })?;
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
        tools_tokens: &mut u32,
    ) -> Result<rig::agent::Agent<M>>
    where
        M: CompletionModel,
    {
        agent_builder = agent_builder
            .preamble(&context.system_prompt)
            .temperature(0.0);
        let mcp_config = context.config.mcp.as_ref();
        agent_builder = Self::add_static_tools(agent_builder, context);
        agent_builder = Self::add_mcp_tools(agent_builder, mcp_config).await?;
        let agent = agent_builder.build();
        *tools_tokens = count_tokens(
            &agent
                .tools
                .documents()
                .await
                .unwrap()
                .iter()
                .map(|d| &d.text)
                .join("\n"),
        );
        Ok(agent)
    }

    async fn build_agent(
        context: BuildAgentContext<'_>,
        tools_tokens: &mut u32,
    ) -> Result<Box<dyn HulyAgent>> {
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
                    Self::configure_agent(agent_builder, context, tools_tokens).await?,
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
                    Self::configure_agent(agent_builder, context, tools_tokens).await?,
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
                    Self::configure_agent(agent_builder, context, tools_tokens).await?,
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
                    Self::configure_agent(agent_builder, context, tools_tokens).await?,
                ))
            }
        }
    }

    pub async fn run(
        &mut self,
        receiver: mpsc::UnboundedReceiver<AgentControlEvent>,
        messages: Vec<Message>,
        memory_index: InMemoryVectorIndex<rig_fastembed::EmbeddingModel, Entity>,
    ) {
        tracing::info!(
            "Run agent: {:?} : {}",
            self.config.provider,
            self.config.model
        );
        let system_prompt =
            prepare_system_prompt(&self.config.workspace, &self.config.user_instructions).await;
        let system_prompt_token_count = count_tokens(&system_prompt);
        let mut tools_tokens = 0;

        let agent = Self::build_agent(
            BuildAgentContext {
                config: &self.config,
                system_prompt,
                memory: self.memory.clone(),
                process_registry: self.process_registry.clone(),
                sender: self.sender.clone(),
            },
            &mut tools_tokens,
        )
        .await
        .unwrap();

        // This is workaround to calculate tokens from system prompt and tools for providers like LMStudio
        let system_prompt_token_count = system_prompt_token_count + tools_tokens / 2;
        // restore state from messages
        let state = if messages.is_empty() {
            AgentState::WaitingUserPrompt
        } else {
            match messages.last().unwrap() {
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
        };
        tracing::info!("initial state: {:?}", state);
        self.sender
            .send(AgentOutputEvent::AgentStatus(0, 0, state.clone()))
            .unwrap();

        let messages = Arc::new(RwLock::new(messages));
        let memory_index = Arc::new(RwLock::new(memory_index));
        let sender = self.sender.clone();
        let state = Arc::new(RwLock::new(state));

        let events_context = AgentContext {
            config: self.config.clone(),
            messages: messages.clone(),
            state: state.clone(),
            sender: self.sender.clone(),
            process_registry: self.process_registry.clone(),
            memory_index: memory_index.clone(),
            current_completion_tokens: 0,
            current_input_tokens: 0,
            system_prompt_token_count,
        };

        let process_context = AgentContext {
            config: self.config.clone(),
            messages: messages.clone(),
            state: state.clone(),
            sender: self.sender.clone(),
            process_registry: self.process_registry.clone(),
            memory_index: memory_index.clone(),
            current_completion_tokens: 0,
            current_input_tokens: 0,
            system_prompt_token_count,
        };

        tokio::select! {
           _ = handle_control_events(events_context, receiver) => {}
           _ = process_messages(process_context, agent) => {}
           _ = handle_process_registry(self.process_registry.clone(), self.sender.clone()) => {}
           _ = sender.closed() => {}
        }

        tracing::info!("Stop agent");
    }
}

impl AgentContext {
    async fn add_message(&mut self, message: Message) {
        self.sender
            .send(AgentOutputEvent::AddMessage(message.clone()))
            .unwrap();
        let mut messages = self.messages.write().await;
        if let Message::User { .. } = &message {
            // clear previous messages from env details
            messages.iter_mut().for_each(|m| {
                if let Message::User { content, .. } = m {
                    if content.len() > 1 {
                        *content = OneOrMany::one(content.first());
                    }
                }
            });
        }
        messages.push(message);
    }

    async fn send_message(&mut self, message: String) {
        let message = if let Some(tool_id) = pending_tool_id(self.messages.read().await) {
            Message::User {
                content: OneOrMany::one(UserContent::tool_result(
                    tool_id,
                    OneOrMany::one(ToolResultContent::text(message)),
                )),
            }
        } else {
            Message::user(message)
        };
        self.add_message(self.add_env_message(message).await).await;
        self.set_state(AgentState::WaitingResponse).await;
    }

    async fn add_env_message(&self, mut message: Message) -> Message {
        add_env_message(
            &mut message,
            self.memory_index.clone(),
            &self.config.workspace,
            self.process_registry.clone(),
        )
        .await;
        message
    }

    async fn set_state(&mut self, state: AgentState) {
        let mut cur_state = self.state.write().await;
        tracing::info!("Agent state trasition: {}->{}", cur_state, state);
        *cur_state = state.clone();
        if !self.sender.is_closed() {
            self.sender
                .send(AgentOutputEvent::AgentStatus(
                    self.current_input_tokens,
                    self.current_completion_tokens,
                    state,
                ))
                .unwrap();
        }
    }

    async fn is_last_user_message(&self) -> bool {
        self.messages
            .read()
            .await
            .last()
            .is_some_and(|m| matches!(m, Message::User { .. }))
    }

    async fn chat_histoty(&self) -> Vec<Message> {
        let messages = self.messages.read().await;
        messages[..messages.len() - 1].to_vec()
    }

    async fn persist_history(&self) {
        tracing::debug!("persist_history");
        let messages = self.messages.read().await;
        persist_history(&messages);
    }

    async fn update_last_message(&mut self, message: Message) {
        let mut messages = self.messages.write().await;
        let last_idx = messages.len() - 1;
        self.sender
            .send(AgentOutputEvent::UpdateMessage(message.clone()))
            .unwrap();
        messages[last_idx] = message;
    }

    async fn count_aproximate_tokens(&self) -> u32 {
        let messages = self.messages.read().await;
        self.system_prompt_token_count
            + messages
                .iter()
                .map(|m| match m {
                    Message::User { content } => content
                        .iter()
                        .map(|c| match c {
                            UserContent::Text(text) => count_tokens(&text.text),
                            UserContent::ToolResult(tool_result) => tool_result
                                .content
                                .iter()
                                .map(|t| match t {
                                    ToolResultContent::Text(text) => count_tokens(&text.text),
                                    _ => 0,
                                })
                                .sum::<u32>(),
                            _ => 0,
                        })
                        .sum::<u32>(),
                    Message::Assistant { content } => content
                        .iter()
                        .map(|c| match c {
                            AssistantContent::Text(text) => count_tokens(&text.text),
                            AssistantContent::ToolCall(tool_call) => {
                                count_tokens(&serde_json::to_string(tool_call).unwrap())
                            }
                        })
                        .sum::<u32>(),
                })
                .sum::<u32>()
    }
}

async fn handle_control_events(
    mut ctx: AgentContext,
    mut receiver: mpsc::UnboundedReceiver<AgentControlEvent>,
) {
    while let Some(event) = receiver.recv().await {
        match event {
            AgentControlEvent::SendMessage(message) => {
                tracing::info!("Send message: {}", message);
                ctx.send_message(message).await;
            }
            AgentControlEvent::CancelTask => {
                tracing::info!("Cancel current task");
                if !ctx.state.read().await.is_paused() {
                    ctx.set_state(AgentState::Paused).await;
                } else if !ctx.state.read().await.is_completed()
                    && !ctx.messages.read().await.is_empty()
                {
                    ctx.set_state(AgentState::WaitingResponse).await;
                }
            }
            AgentControlEvent::NewTask => {
                tracing::info!("New task");
                ctx.messages.write().await.clear();
                ctx.set_state(AgentState::WaitingUserPrompt).await;
                ctx.sender.send(AgentOutputEvent::NewTask).ok();
                ctx.persist_history().await;
            }
            AgentControlEvent::TerminalData(idx, data) => {
                tracing::info!("Terminal input data");
                ctx.process_registry.read().await.send_data(idx, data);
            }
        }
    }
}

async fn process_messages(mut ctx: AgentContext, mut agent: Box<dyn HulyAgent>) {
    loop {
        if ctx.state.read().await.is_paused() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            continue;
        }

        if !ctx.is_last_user_message().await {
            ctx.set_state(AgentState::WaitingUserPrompt).await;
            continue;
        }

        if let Err(e) = send_messages(&mut ctx, &mut agent).await {
            ctx.persist_history().await;
            tracing::error!("Error processing messages: {}", e);
            ctx.set_state(AgentState::Error(format!("{e}"))).await;
        }
    }

    async fn send_messages(
        ctx: &mut AgentContext,
        agent: &mut Box<dyn HulyAgent>,
    ) -> Result<(), AgentError> {
        let last_message = ctx.messages.read().await.last().unwrap().clone();
        let mut stream = agent
            .send_messages(last_message.clone(), ctx.chat_histoty().await)
            .await?;
        tracing::trace!("Sending messages to model: {:?}", last_message);
        ctx.set_state(AgentState::WaitingResponse).await;

        let mut assistant_content = String::new();

        while let Some(result) = stream.next().await {
            //tracing::trace!("Received response from model: {:?}", result);
            let result = result?;
            if ctx.state.read().await.is_paused() {
                tracing::info!("Agent is paused, skip receiving response");
                break;
            }
            match result {
                AssistantContent::Text(text) => {
                    if matches!(*ctx.state.read().await, AgentState::Thinking) {
                        ctx.set_state(AgentState::Thinking).await;
                    }
                    let is_empty = assistant_content.is_empty();
                    assistant_content.push_str(&text.text);
                    if is_empty {
                        ctx.add_message(Message::assistant(text.text)).await;
                    } else {
                        ctx.update_last_message(Message::assistant(&assistant_content))
                            .await;
                    }
                }
                AssistantContent::ToolCall(tool_call) => {
                    ctx.set_state(AgentState::ToolCall(
                        tool_call.function.name.clone(),
                        tool_call.function.arguments.clone(),
                    ))
                    .await;
                    assistant_content = String::new();
                    ctx.add_message(Message::Assistant {
                        content: OneOrMany::one(AssistantContent::ToolCall(tool_call.clone())),
                    })
                    .await;
                    let (mut tool_result, is_error) = match agent
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
                    if tool_result.is_empty() || tool_result == "\"\"" {
                        tool_result = format!(
                            "The [{}] tool executed successfully but returned no results.",
                            tool_call.function.name
                        );
                    }
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
                                    ctx.sender
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
                    let result_message = Message::User {
                        content: OneOrMany::one(UserContent::tool_result(
                            tool_call.id.clone(),
                            OneOrMany::one(ToolResultContent::text(tool_result)),
                        )),
                    };
                    if tool_call.function.name == AttemptCompletionTool::NAME {
                        ctx.set_state(AgentState::Completed(false)).await;
                        tracing::info!("Stop task with success");
                        ctx.persist_history().await;
                    } else {
                        ctx.add_message(ctx.add_env_message(result_message).await)
                            .await;
                    }
                }
            }
        }

        let response: CompletionResponse<
            Option<rig::providers::openai::StreamingCompletionResponse>,
        > = From::from(stream);
        if let Some(raw_response) = response.raw_response {
            let usage = raw_response.usage;
            tracing::info!("Usage: {:?}", usage);
            if usage.total_tokens > 0 {
                ctx.current_input_tokens = usage.prompt_tokens as u32;
                ctx.current_completion_tokens = (usage.total_tokens - usage.prompt_tokens) as u32;
            } else {
                // try to calculate aproximate tokens
                ctx.current_input_tokens = ctx.count_aproximate_tokens().await;
                ctx.current_completion_tokens = 0;
            }
        }
        if matches!(*ctx.state.read().await, AgentState::Completed(false)) {
            ctx.set_state(AgentState::Completed(true)).await;
        } else if !ctx.is_last_user_message().await && !ctx.state.read().await.is_completed() {
            ctx.set_state(AgentState::WaitingUserPrompt).await;
        }
        ctx.persist_history().await;
        Ok(())
    }
}

async fn handle_process_registry(
    process_registry: Arc<RwLock<ProcessRegistry>>,
    sender: mpsc::UnboundedSender<AgentOutputEvent>,
) {
    loop {
        let mut process_registry = process_registry.write().await;
        let modified_command_states = process_registry.poll();
        if !modified_command_states.is_empty() {
            sender
                .send(AgentOutputEvent::CommandStatus(modified_command_states))
                .ok();
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
