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
    pub model: String,
    pub workspace: String,
    pub user_instructions: String,
}

impl Config {
    pub fn load() -> color_eyre::Result<Self> {
        let config = serde_yaml::from_str::<Self>(&std::fs::read_to_string(CONFIG_FILE)?)?;
        Ok(config)
    }
}
