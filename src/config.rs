use std::path::Path;

use serde::Deserialize;

const CONFIG_FILE: &str = "huly-coder.yaml";

#[derive(Debug, Deserialize, Clone)]
pub enum ProviderKind {
    OpenAI,
    OpenRouter,
    LMStudio,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub provider: ProviderKind,
    pub provider_api_key: Option<String>,
    pub provider_base_url: Option<String>,
    pub model: String,
    pub workspace: String,
    pub user_instructions: String,
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
