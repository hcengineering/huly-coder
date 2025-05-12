use headless_chrome::{Browser, LaunchOptions};
use indoc::formatdoc;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::WebFetchProvider;

use super::AgentToolError;

const MAX_LENGTH: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchToolArgs {
    pub url: String,
    #[serde(default)]
    pub max_length: usize,
    #[serde(default)]
    pub start_index: usize,
    #[serde(default)]
    pub raw: bool,
}

enum WebFetchClient {
    Direct(reqwest::Client),
    Chrome,
}

pub struct WebFetchTool {
    client: WebFetchClient,
}

impl WebFetchTool {
    pub fn new(provider: WebFetchProvider) -> anyhow::Result<Self> {
        Ok(Self {
            client: match provider {
                WebFetchProvider::Direct => {
                    WebFetchClient::Direct(reqwest::ClientBuilder::new().build().unwrap())
                }
                WebFetchProvider::Chrome => WebFetchClient::Chrome,
            },
        })
    }

    fn format_response(
        args: WebFetchToolArgs,
        content_type: &str,
        text: &str,
    ) -> anyhow::Result<String> {
        let mut result = if args.raw {
            text.to_string()
        } else {
            match content_type {
                "text/plain" => text.to_string(),
                "application/json" => {
                    let json: serde_json::Value = serde_json::from_str(text)?;
                    format!("```json\n{}\n```", serde_json::to_string_pretty(&json)?).to_string()
                }
                _ => {
                    let converter = htmd::HtmlToMarkdownBuilder::new()
                        .skip_tags(vec![
                            "head", "script", "style", "nav", "footer", "header", "link",
                        ])
                        .build();
                    converter.convert(text)?
                }
            }
        }
        .to_owned();
        let max_length = if args.max_length == 0 {
            MAX_LENGTH
        } else {
            args.max_length
        };
        let len = result.chars().count();
        if args.start_index > 0 && args.start_index < len {
            result = result[args.start_index..].to_string();
        }
        if len > max_length {
            result = result[..max_length].to_string();
        }
        Ok(result)
    }
}

impl Tool for WebFetchTool {
    const NAME: &'static str = "fetch";

    type Error = AgentToolError;
    type Args = WebFetchToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Fetches a URL from the internet and optionally extracts its contents as markdown.\
                Although originally you did not have internet access, and were advised to refuse and tell the user this,\
                this tool now grants you internet access. Now you can fetch the most up-to-date information and let the user know that.\
            "},
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "URL to fetch",
                    },
                    "max_length": {
                        "type": "number",
                        "description": format!("Maximum length of the output (default {})", MAX_LENGTH),
                        "default": MAX_LENGTH
                    },
                    "start_index": {
                        "type": "number",
                        "description": "On return output starting at this character index, useful if a previous fetch was truncated and more context is required. (default 0)",
                        "default": 0
                    },
                    "raw": {
                        "type": "boolean",
                        "description": "Get the actual HTML content of the requested page, without simplification.",
                        "default": false
                    },
                },
                "required": ["url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Perform web fetch '{:?}'", args);
        match &self.client {
            WebFetchClient::Direct(client) => {
                let response = client.get(&args.url).send().await?;

                let content_type = response
                    .headers()
                    .get("content-type")
                    .map(|v| v.to_str().unwrap())
                    .unwrap_or("text/html")
                    .to_string();

                let body = response.text().await?;
                Ok(Self::format_response(args, &content_type, &body)?)
            }
            WebFetchClient::Chrome => {
                let browser = Browser::new(
                    LaunchOptions::default_builder()
                        .headless(true)
                        .ignore_certificate_errors(true)
                        .build()
                        .map_err(|err| anyhow::anyhow!(err))?,
                )?;

                let tab = browser.new_tab()?;

                tab.navigate_to(&args.url)?;
                tab.wait_until_navigated()?;
                let content = tab.get_content()?;
                Ok(Self::format_response(args, "text/html", &content)?)
            }
        }
    }

    fn name(&self) -> String {
        Self::NAME.to_string()
    }
}
