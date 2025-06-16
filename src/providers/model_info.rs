// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use std::{fs, path::Path};

use serde::Deserialize;

use crate::config::Config;

const OPENROUTER_MODELS_FILE: &str = "openrouter_models.json";
const ANTHROPIC_MODELS: &str = include_str!("anthropic_models.json");
const OPENAI_MODELS: &str = include_str!("openai_models.json");

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub input_price: f64,
    pub completion_price: f64,
    pub max_tokens: u32,
}

#[derive(Deserialize)]
struct LMStudioModelInfo {
    pub id: String,
    pub loaded_context_length: Option<u32>,
    pub max_context_length: u32,
}

#[derive(Deserialize)]
struct OpenRouterPriceInfo {
    pub prompt: String,
    pub completion: String,
}

#[derive(Deserialize)]
struct OpenRouterModelInfo {
    pub id: String,
    pub pricing: OpenRouterPriceInfo,
    pub context_length: u32,
}

#[derive(Deserialize)]
struct AnthropicModelInfo {
    pub model_id: String,
    pub input_price: f64,
    pub output_price: f64,
    pub max_context_tokens: u32,
}

#[derive(Deserialize)]
struct OpenAIModelInfo {
    pub model_id: String,
    pub input_price: f64,
    pub output_price: f64,
    pub max_context_tokens: u32,
}

pub async fn model_info(data_dir: &str, config: &Config) -> color_eyre::Result<ModelInfo> {
    let openrouter_models_file = Path::new(data_dir).join(OPENROUTER_MODELS_FILE);
    match config.provider {
        crate::config::ProviderKind::OpenAI => {
            let models: Vec<OpenAIModelInfo> = serde_json::from_str(OPENAI_MODELS)?;
            models
                .iter()
                .find(|model| config.model.contains(&model.model_id))
                .map(|model| ModelInfo {
                    input_price: model.input_price,
                    completion_price: model.output_price,
                    max_tokens: model.max_context_tokens,
                })
                .ok_or_else(|| color_eyre::eyre::eyre!("Model not found"))
        }
        crate::config::ProviderKind::OpenRouter => {
            let models: Vec<OpenRouterModelInfo> =
                serde_json::from_value(if openrouter_models_file.exists() {
                    let data = fs::read_to_string(openrouter_models_file)?;
                    serde_json::from_str(&data)?
                } else {
                    let mut data = reqwest::get("https://openrouter.ai/api/v1/models")
                        .await?
                        .json::<serde_json::Value>()
                        .await?;
                    let data = data["data"].take();
                    fs::write(openrouter_models_file, data.to_string())?;
                    data
                })?;
            models
                .iter()
                .find(|model| model.id == config.model)
                .map(|model| ModelInfo {
                    input_price: model.pricing.prompt.parse::<f64>().unwrap_or(0.0),
                    completion_price: model.pricing.completion.parse::<f64>().unwrap_or(0.0),
                    max_tokens: model.context_length,
                })
                .ok_or_else(|| color_eyre::eyre::eyre!("Model not found"))
        }
        crate::config::ProviderKind::LMStudio => {
            let url = config
                .provider_base_url
                .clone()
                .unwrap_or("http://127.0.0.1:1234/v1".to_string())
                .replace("/v1", "/api/v0/models");
            let mut data = reqwest::get(url).await?.json::<serde_json::Value>().await?;
            let models: Vec<LMStudioModelInfo> = serde_json::from_value(data["data"].take())?;
            models
                .iter()
                .find(|model| model.id == config.model)
                .map(|model| ModelInfo {
                    input_price: 0.0,
                    completion_price: 0.0,
                    max_tokens: model
                        .loaded_context_length
                        .unwrap_or(model.max_context_length),
                })
                .ok_or_else(|| color_eyre::eyre::eyre!("Model not found"))
        }
        crate::config::ProviderKind::Anthropic => {
            let models: Vec<AnthropicModelInfo> = serde_json::from_str(ANTHROPIC_MODELS)?;
            models
                .iter()
                .find(|model| config.model.contains(&model.model_id))
                .map(|model| ModelInfo {
                    input_price: model.input_price,
                    completion_price: model.output_price,
                    max_tokens: model.max_context_tokens,
                })
                .ok_or_else(|| color_eyre::eyre::eyre!("Model not found"))
        }
    }
}
