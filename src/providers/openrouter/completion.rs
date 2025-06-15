//! Modified version of rig::agent::providers::openrouter
use serde::Deserialize;

use crate::providers::openrouter::merge;

use super::client::{ApiErrorResponse, ApiResponse, Client, Usage};

use rig::{
    completion::{self, CompletionError, CompletionRequest},
    message::{ImageMediaType, MimeType},
    providers::openai::Message,
    OneOrMany,
};
use serde_json::{json, Value};

use rig::providers::openai::AssistantContent;

// ================================================================
// OpenRouter Completion API
// ================================================================
/// The `qwen/qwq-32b` model. Find more models at <https://openrouter.ai/models>.
pub const QWEN_QWQ_32B: &str = "qwen/qwq-32b";
/// The `anthropic/claude-3.7-sonnet` model. Find more models at <https://openrouter.ai/models>.
pub const CLAUDE_3_7_SONNET: &str = "anthropic/claude-3.7-sonnet";
/// The `perplexity/sonar-pro` model. Find more models at <https://openrouter.ai/models>.
pub const PERPLEXITY_SONAR_PRO: &str = "perplexity/sonar-pro";
/// The `google/gemini-2.0-flash-001` model. Find more models at <https://openrouter.ai/models>.
pub const GEMINI_FLASH_2_0: &str = "google/gemini-2.0-flash-001";

/// A openrouter completion object.
///
/// For more information, see this link: <https://docs.openrouter.xyz/reference/create_chat_completion_v1_chat_completions_post>
#[derive(Debug, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub system_fingerprint: Option<String>,
    pub usage: Option<Usage>,
}

impl From<ApiErrorResponse> for CompletionError {
    fn from(err: ApiErrorResponse) -> Self {
        CompletionError::ProviderError(err.message)
    }
}

impl TryFrom<CompletionResponse> for completion::CompletionResponse<CompletionResponse> {
    type Error = CompletionError;

    fn try_from(response: CompletionResponse) -> Result<Self, Self::Error> {
        let choice = response.choices.first().ok_or_else(|| {
            CompletionError::ResponseError("Response contained no choices".to_owned())
        })?;

        let content = match &choice.message {
            Message::Assistant {
                content,
                tool_calls,
                ..
            } => {
                let mut content = content
                    .iter()
                    .map(|c| match c {
                        AssistantContent::Text { text } => completion::AssistantContent::text(text),
                        AssistantContent::Refusal { refusal } => {
                            completion::AssistantContent::text(refusal)
                        }
                    })
                    .collect::<Vec<_>>();

                content.extend(
                    tool_calls
                        .iter()
                        .map(|call| {
                            completion::AssistantContent::tool_call(
                                &call.function.name,
                                &call.function.name,
                                call.function.arguments.clone(),
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                Ok(content)
            }
            _ => Err(CompletionError::ResponseError(
                "Response did not contain a valid message or tool call".into(),
            )),
        }?;

        let choice = OneOrMany::many(content).map_err(|_| {
            CompletionError::ResponseError(
                "Response contained no message or tool call (empty)".to_owned(),
            )
        })?;

        Ok(completion::CompletionResponse {
            choice,
            raw_response: response,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: usize,
    pub native_finish_reason: Option<String>,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Clone)]
pub struct CompletionModel {
    pub(crate) client: Client,
    /// Name of the model (e.g.: deepseek-ai/DeepSeek-R1)
    pub model: String,
}

fn user_text_to_json(content: rig::message::UserContent) -> serde_json::Value {
    match content {
        rig::message::UserContent::Text(text) => json!({
            "role": "user",
            "content": text.text,
        }),
        _ => unreachable!(),
    }
}

fn user_content_to_json(
    content: rig::message::UserContent,
) -> Result<serde_json::Value, CompletionError> {
    match content {
        rig::message::UserContent::Text(text) => Ok(json!({
            "type": "text",
            "text": text.text
        })),
        rig::message::UserContent::Image(image) => Ok(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{};base64,{}", image.media_type.unwrap_or(ImageMediaType::PNG).to_mime_type(), image.data),
            }
        })),
        rig::message::UserContent::Audio(_) => Err(CompletionError::RequestError(
            "Audio is not supported".into(),
        )),
        rig::message::UserContent::Document(_) => Err(CompletionError::RequestError(
            "Document is not supported".into(),
        )),
        rig::message::UserContent::ToolResult(_) => unreachable!(),
    }
}

fn tool_content_to_json(
    content: Vec<rig::message::UserContent>,
) -> Result<serde_json::Value, CompletionError> {
    let mut str_content = String::new();
    let mut tool_id = String::new();

    for content in content.into_iter() {
        match content {
            rig::message::UserContent::ToolResult(tool_result) => {
                tool_id = tool_result.id;
                str_content = tool_result
                    .content
                    .iter()
                    .map(|c| match c {
                        rig::message::ToolResultContent::Text(text) => text.text.clone(),
                        // ignore image content
                        _ => "".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join("");
            }
            _ => unreachable!(),
        }
    }
    Ok(json!({
        "role": "tool",
        "content": str_content,
        "tool_call_id": tool_id,
    }))
}

impl CompletionModel {
    pub fn new(client: Client, model: &str) -> Self {
        Self {
            client,
            model: model.to_string(),
        }
    }

    pub(crate) fn create_completion_request(
        &self,
        completion_request: CompletionRequest,
    ) -> Result<Value, CompletionError> {
        // Add preamble to chat history (if available)
        let mut full_history: Vec<serde_json::Value> = match &completion_request.preamble {
            Some(preamble) => vec![json!({
                "role": "system",
                "content": preamble,
            })],
            None => vec![],
        };

        // Convert existing chat history
        for message in completion_request.chat_history.into_iter() {
            match message {
                rig::message::Message::User { content } => {
                    if content.len() == 1
                        && matches!(content.first(), rig::message::UserContent::Text(_))
                    {
                        full_history.push(user_text_to_json(content.first()));
                    } else if content
                        .iter()
                        .any(|c| matches!(c, rig::message::UserContent::ToolResult(_)))
                    {
                        let (tool_content, user_content) =
                            content.into_iter().partition::<Vec<_>, _>(|c| {
                                matches!(c, rig::message::UserContent::ToolResult(_))
                            });
                        full_history.push(tool_content_to_json(tool_content.clone())?);
                        for tool_content in tool_content.into_iter() {
                            match tool_content {
                                rig::message::UserContent::ToolResult(result) => {
                                    for tool_result_content in result.content.into_iter() {
                                        match tool_result_content {
                                            rig::message::ToolResultContent::Image(image) => {
                                                full_history.push(json!({
                                                    "role": "user",
                                                    "content": [{
                                                        "type": "image_url",
                                                        "image_url": {
                                                            "url": format!("data:{};base64,{}", image.media_type.unwrap_or(ImageMediaType::PNG).to_mime_type(), image.data),
                                                        }
                                                    }]
                                                }));
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                _ => unreachable!(),
                            }
                        }
                        if !user_content.is_empty() {
                            if user_content.len() == 1 {
                                full_history
                                    .push(user_text_to_json(user_content.first().unwrap().clone()));
                            } else {
                                let user_content = user_content
                                    .into_iter()
                                    .map(user_content_to_json)
                                    .collect::<Result<Vec<_>, _>>()?;
                                full_history
                                    .push(json!({ "role": "user", "content": user_content}));
                            }
                        }
                    } else {
                        let content = content
                            .into_iter()
                            .map(user_content_to_json)
                            .collect::<Result<Vec<_>, _>>()?;
                        full_history.push(json!({ "role": "user", "content": content}));
                    }
                }
                rig::message::Message::Assistant { content } => {
                    for content in content {
                        match content {
                            rig::message::AssistantContent::Text(text) => {
                                full_history.push(json!({
                                    "role": "assistant",
                                    "content": text.text
                                }));
                            }
                            rig::message::AssistantContent::ToolCall(tool_call) => {
                                full_history.push(json!({
                                    "role": "assistant",
                                    "content": null,
                                    "tool_calls": [{
                                        "id": tool_call.id,
                                        "type": "function",
                                        "function": {
                                            "name": tool_call.function.name,
                                            "arguments": tool_call.function.arguments.to_string()
                                        }
                                    }]
                                }));
                            }
                        }
                    }
                }
            };
        }

        let tools = completion_request
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": tool
                })
            })
            .collect::<Vec<_>>();
        let request = json!({
            "model": self.model,
            "messages": full_history,
            "tools": tools,
            "temperature": completion_request.temperature,
        });

        let request = if let Some(params) = completion_request.additional_params {
            merge(request, params)
        } else {
            request
        };
        //println!("!!!!! {} ", request);
        Ok(request)
    }
}

impl completion::CompletionModel for CompletionModel {
    type Response = CompletionResponse;

    async fn completion(
        &self,
        completion_request: CompletionRequest,
    ) -> Result<completion::CompletionResponse<CompletionResponse>, CompletionError> {
        let request = self.create_completion_request(completion_request)?;

        let response = self
            .client
            .post("/chat/completions")
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            match response.json::<ApiResponse<CompletionResponse>>().await? {
                ApiResponse::Ok(response) => {
                    tracing::info!(target: "rig",
                        "OpenRouter completion token usage: {:?}",
                        response.usage.clone().map(|usage| format!("{usage}")).unwrap_or("N/A".to_string())
                    );

                    response.try_into()
                }
                ApiResponse::Err(err) => Err(CompletionError::ProviderError(err.message)),
            }
        } else {
            Err(CompletionError::ProviderError(response.text().await?))
        }
    }
}
