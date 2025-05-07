use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mcp_core::types::ProtocolVersion;
use serde::Deserialize;

const CONFIG_FILE: &str = "huly-coder.yaml";

#[derive(Debug, Deserialize, Clone)]
pub enum ProviderKind {
    OpenAI,
    OpenRouter,
    LMStudio,
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
pub struct Config {
    pub provider: ProviderKind,
    pub provider_api_key: Option<String>,
    pub provider_base_url: Option<String>,
    pub model: String,
    pub workspace: PathBuf,
    pub user_instructions: String,
    pub mcp: Option<McpConfig>,
}

impl Config {
    pub fn new() -> color_eyre::Result<Self> {
        let mut builder = config::Config::builder()
            .add_source(config::File::with_name(CONFIG_FILE))
            .add_source(config::Environment::with_prefix("HULY_CODER"));
        let user_config = format!(
            "{}/{}",
            dirs::home_dir().unwrap().to_str().unwrap(),
            CONFIG_FILE
        );
        if Path::new(&user_config).exists() {
            tracing::info!("Found user config at {}", user_config);
            builder = builder.add_source(config::File::with_name(&user_config));
        }
        builder
            .build()?
            .try_deserialize()
            .map_err(|e| color_eyre::eyre::ErrReport::new(e))
    }
}
