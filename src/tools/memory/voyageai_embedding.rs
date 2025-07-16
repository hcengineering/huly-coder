// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use rig::embeddings::{self, embedding, Embedding, EmbeddingError};
use serde::Deserialize;

const VOYAGEAI_URL: &str = "https://api.voyageai.com/v1/embeddings";

#[derive(Debug, Deserialize)]
struct VoyageAIEmbeddingResponse {
    pub data: Vec<VoyageAIEmbedding>,
}

#[derive(Debug, Deserialize)]
struct VoyageAIEmbedding {
    pub embedding: Vec<f64>,
}

#[derive(Debug, Clone)]
pub struct VoyageAIEmbeddingModel {
    api_key: String,
    model: String,
    dimensions: usize,
    client: reqwest::Client,
}

impl VoyageAIEmbeddingModel {
    pub fn new(api_key: String, model: String, dimensions: usize) -> Self {
        Self {
            api_key,
            model,
            dimensions,
            client: reqwest::Client::new(),
        }
    }
}

impl embedding::EmbeddingModel for VoyageAIEmbeddingModel {
    const MAX_DOCUMENTS: usize = 1024;

    fn ndims(&self) -> usize {
        self.dimensions
    }

    async fn embed_texts(
        &self,
        documents: impl IntoIterator<Item = String>,
    ) -> Result<Vec<embeddings::Embedding>, EmbeddingError> {
        let text = documents.into_iter().next().unwrap();
        let res = self
            .client
            .post(VOYAGEAI_URL)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "model": self.model,
                "output_dimension": self.dimensions,
                "input": text,
            }))
            .send()
            .await?;
        let mut res = res
            .json::<VoyageAIEmbeddingResponse>()
            .await
            .map_err(|e| EmbeddingError::ProviderError(format!("Failed to parse response: {e}")))?;

        let Some(embedding) = res.data.drain(..).next() else {
            return Err(EmbeddingError::ProviderError(format!(
                "No embedding found for text: {}",
                text
            )));
        };

        Ok(vec![Embedding {
            document: text,
            vec: embedding.embedding,
        }])
    }
}
