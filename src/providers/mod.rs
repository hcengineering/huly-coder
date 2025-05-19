// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use async_trait::async_trait;
use rig::agent::Agent;
use rig::completion::CompletionError;
use rig::message::Message;
use rig::streaming::{StreamingCompletion, StreamingCompletionResponse};
use rig::tool::ToolSet;

pub mod openrouter;

#[async_trait]
pub trait HulyAgent: Send + Sync {
    async fn send_messages(
        &self,
        prompt: Message,
        chat_history: Vec<Message>,
    ) -> Result<
        StreamingCompletionResponse<rig::providers::openai::StreamingCompletionResponse>,
        CompletionError,
    >;

    fn tools(&self) -> &ToolSet;
}

#[async_trait]
impl HulyAgent for Agent<openrouter::CompletionModel> {
    async fn send_messages(
        &self,
        prompt: Message,
        chat_history: Vec<Message>,
    ) -> Result<
        StreamingCompletionResponse<rig::providers::openai::StreamingCompletionResponse>,
        CompletionError,
    > {
        self.stream_completion(prompt, chat_history)
            .await?
            .stream()
            .await
    }

    fn tools(&self) -> &ToolSet {
        &self.tools
    }
}

#[async_trait]
impl HulyAgent for Agent<rig::providers::openai::CompletionModel> {
    async fn send_messages(
        &self,
        prompt: Message,
        chat_history: Vec<Message>,
    ) -> Result<
        StreamingCompletionResponse<rig::providers::openai::StreamingCompletionResponse>,
        CompletionError,
    > {
        self.stream_completion(prompt, chat_history)
            .await?
            .stream()
            .await
    }

    fn tools(&self) -> &ToolSet {
        &self.tools
    }
}

#[async_trait]
impl HulyAgent for Agent<rig::providers::anthropic::completion::CompletionModel> {
    async fn send_messages(
        &self,
        prompt: Message,
        chat_history: Vec<Message>,
    ) -> Result<
        StreamingCompletionResponse<rig::providers::openai::StreamingCompletionResponse>,
        CompletionError,
    > {
        self.stream_completion(prompt, chat_history)
            .await?
            .stream()
            .await
    }

    fn tools(&self) -> &ToolSet {
        &self.tools
    }
}
