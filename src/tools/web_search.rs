// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use indoc::formatdoc;
use itertools::Itertools;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::WebSearchProvider;

use super::AgentToolError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchToolArgs {
    pub query: String,
    #[serde(default)]
    pub count: u16,
    #[serde(default)]
    pub offset: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearxResultItem {
    pub title: String,
    pub url: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearxResult {
    pub results: Vec<SearxResultItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BraveWebResultItem {
    pub title: String,
    pub url: String,
    pub description: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct BraveWebResult {
    pub results: Vec<BraveWebResultItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BraveResult {
    pub web: BraveWebResult,
}

pub struct WebSearchTool {
    config: WebSearchProvider,
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn new(config: WebSearchProvider) -> Self {
        Self {
            config,
            client: reqwest::ClientBuilder::new().build().unwrap(),
        }
    }
}

impl Tool for WebSearchTool {
    const NAME: &'static str = "web_search";

    type Error = AgentToolError;
    type Args = WebSearchToolArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: formatdoc! {"\
                Performs a web search using the Web Search API, ideal for general queries, news, articles, and online content.\
                Use this for broad information gathering, recent events, or when you need diverse web sources.\
                Supports pagination, content filtering, and freshness controls.\
                Maximum 20 results per request, with offset for pagination.\
            "},
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (max 400 chars, 50 words)",
                    },
                    "count": {
                        "type": "number",
                        "description": "Number of results (1-20, default 10)",
                        "default": 10
                    },
                    "offset": {
                        "type": "number",
                        "description": "Pagination offset (max 9, default 0)",
                        "default": 0
                    },
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!("Perform web search '{:?}'", args);
        match &self.config {
            WebSearchProvider::Searx(search_config) => {
                let url = format!(
                    "{}/search?q={}&pageno={}&format=json",
                    search_config.url,
                    utf8_percent_encode(&args.query, NON_ALPHANUMERIC),
                    args.offset + 1
                );
                let response = self.client.get(url).send().await?;
                let body = response.text().await?;

                let json: SearxResult = serde_json::from_str(&body)?;
                let converter = htmd::HtmlToMarkdownBuilder::new().build();
                let result = json
                    .results
                    .into_iter()
                    .map(|item| {
                        format!(
                            "Title: {}\nDescription: {}\nURL: {}",
                            item.title,
                            converter.convert(&item.content).unwrap_or(item.content),
                            item.url
                        )
                    })
                    .join("\n\n");
                Ok(result)
            }
            WebSearchProvider::Brave(search_config) => {
                let url = format!(
                    "https://api.search.brave.com/res/v1/web/search?q={}&count={}&offset={}",
                    utf8_percent_encode(&args.query, NON_ALPHANUMERIC),
                    if args.count == 0 { 10 } else { args.count },
                    args.offset
                );
                tracing::info!("Perform Brave web search '{}'", url);
                let response = self
                    .client
                    .get(url)
                    .header("Accept", "application/json")
                    .header("X-Subscription-Token", &search_config.api_key)
                    .send()
                    .await?;
                if response.status() != 200 {
                    return Err(AgentToolError::Other(anyhow::anyhow!(
                        "Unexpected status code: {}: {}",
                        response.status(),
                        response.text().await.unwrap()
                    )));
                }
                let body = response
                    .text()
                    .await
                    .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
                let json: BraveResult = serde_json::from_str(&body)?;
                let converter = htmd::HtmlToMarkdownBuilder::new().build();
                let result = json
                    .web
                    .results
                    .into_iter()
                    .map(|item| {
                        format!(
                            "Title: {}\nDescription: {}\nURL: {}",
                            item.title,
                            converter
                                .convert(&item.description)
                                .unwrap_or(item.description),
                            item.url
                        )
                    })
                    .join("\n\n");
                Ok(result)
            }
        }
    }
}
