// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use mcp_core::types::ProtocolVersion;
use serde::Deserialize;

const CONFIG_FILE: &str = "huly-coder.yaml";
const LOCAL_CONFIG_FILE: &str = "huly-coder-local.yaml";
const DOCKER_LOCAL_CONFIG_FILE: &str = "data/huly-coder-local.yaml";

#[derive(Debug, Deserialize, Clone)]
pub enum ProviderKind {
    OpenAI,
    OpenRouter,
    LMStudio,
    Anthropic,
}

#[derive(Debug, Deserialize, Clone)]
pub struct McpClientStdioTransport {
    pub command: String,
    pub args: Vec<String>,
    pub protocol_version: Option<ProtocolVersion>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct McpClientSseTransport {
    pub url: String,
    pub bearer_token: Option<String>,
    pub protocol_version: Option<ProtocolVersion>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpClientTransport {
    Stdio(McpClientStdioTransport),
    Sse(McpClientSseTransport),
}

#[derive(Debug, Deserialize, Clone)]
pub struct McpConfig {
    pub servers: HashMap<String, McpClientTransport>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WebSearchSearxConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WebSearchBraveConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum WebSearchProvider {
    Searx(WebSearchSearxConfig),
    Brave(WebSearchBraveConfig),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum WebFetchProvider {
    Direct,
    Chrome,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    FullAutonomous,
    ManualApproval,
    DenyAll,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub provider: ProviderKind,
    pub provider_api_key: Option<String>,
    pub provider_base_url: Option<String>,
    pub model: String,
    pub permission_mode: PermissionMode,
    pub workspace: PathBuf,
    pub user_instructions: String,
    pub mcp: Option<McpConfig>,
    pub web_search: Option<WebSearchProvider>,
    pub web_fetch: Option<WebFetchProvider>,
}

impl Config {
    pub fn new() -> color_eyre::Result<Self> {
        let mut builder = config::Config::builder()
            .add_source(config::File::with_name(CONFIG_FILE))
            .add_source(config::Environment::with_prefix("HULY_CODER"));

        if Path::new(LOCAL_CONFIG_FILE).exists() {
            tracing::info!("Found local config at {}", LOCAL_CONFIG_FILE);
            builder = builder.add_source(config::File::with_name(LOCAL_CONFIG_FILE));
        }

        // Docker related local config file that stored in /data directory
        if Path::new(DOCKER_LOCAL_CONFIG_FILE).exists() {
            tracing::info!("Found local config at {}", DOCKER_LOCAL_CONFIG_FILE);
            builder = builder.add_source(config::File::with_name(DOCKER_LOCAL_CONFIG_FILE));
        }

        let user_config = format!(
            "{}/{}",
            dirs::home_dir().unwrap().to_str().unwrap(),
            CONFIG_FILE
        );
        if Path::new(&user_config).exists() {
            tracing::info!("Found user config at {}", user_config);
            builder = builder.add_source(config::File::with_name(&user_config));
        }
        if env::var("DOCKER_RUN").is_ok() {
            builder = builder.set_override("permission_mode", "full_autonomous")?;
        }
        builder
            .build()?
            .try_deserialize()
            .map_err(|e| color_eyre::eyre::ErrReport::new(e))
    }
}
