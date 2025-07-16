// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use rig::{
    embeddings::EmbeddingModel,
    embeddings::{self},
    vector_store::{in_memory_store::InMemoryVectorStore, VectorStoreIndex},
    OneOrMany,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{
    config::{Config, EmbeddingProvider},
    tools::memory::{voyageai_embedding::VoyageAIEmbeddingModel, Entity, MemoryManager},
};

pub enum MemoryEmbeddingModel {
    Fastembed(rig_fastembed::EmbeddingModel),
    VoyageAI(VoyageAIEmbeddingModel),
}

#[derive(Serialize, Deserialize, Default)]
pub struct MemoryVectorStorage {
    embeddings: Vec<(String, OneOrMany<embeddings::Embedding>)>,
}

fn to_texts(entity: &Entity) -> String {
    format!("{}\n{}", entity.name, entity.observations.join("\n"))
}

impl MemoryEmbeddingModel {
    pub async fn embeddings(
        &self,
        document: &Entity,
    ) -> color_eyre::Result<OneOrMany<embeddings::Embedding>> {
        let txt = to_texts(document);
        match self {
            Self::Fastembed(model) => {
                let embedding = model.embed_text(&txt).await?;
                Ok(OneOrMany::one(embedding))
            }
            Self::VoyageAI(model) => {
                let embedding = model.embed_text(&txt).await?;
                Ok(OneOrMany::one(embedding))
            }
        }
    }

    async fn search(
        &self,
        vector_store: InMemoryVectorStore<Entity>,
        query: &str,
        limit: usize,
    ) -> color_eyre::Result<Vec<Entity>> {
        match self {
            Self::Fastembed(model) => {
                let res: Vec<(f64, String, Entity)> = vector_store
                    .index(model.clone())
                    .top_n(query, limit)
                    .await?;
                Ok(res.into_iter().map(|(_, _, entity)| entity).collect())
            }
            Self::VoyageAI(model) => {
                let res: Vec<(f64, String, Entity)> = vector_store
                    .index(model.clone())
                    .top_n(query, limit)
                    .await?;
                Ok(res.into_iter().map(|(_, _, entity)| entity).collect())
            }
        }
    }
}

pub struct MemoryIndexer {
    embedding_storage_path: PathBuf,
    embedding_provider: EmbeddingProvider,
    vector_store: InMemoryVectorStore<Entity>,
    embedding_model: Option<MemoryEmbeddingModel>,
}

impl MemoryIndexer {
    pub fn new(data_dir: &Path, config: &Config) -> Self {
        Self {
            embedding_storage_path: data_dir.join("memory_embeddings.json"),
            embedding_provider: config.memory_embedding.clone(),
            vector_store: InMemoryVectorStore::default(),
            embedding_model: None,
        }
    }

    pub async fn init(&mut self, memory: Arc<RwLock<MemoryManager>>) -> color_eyre::Result<()> {
        let embedding_storage = if self.embedding_storage_path.exists() {
            serde_json::from_str(&std::fs::read_to_string(&self.embedding_storage_path)?)?
        } else {
            MemoryVectorStorage::default()
        };
        match &self.embedding_provider {
            EmbeddingProvider::Fastembed => {
                let client = rig_fastembed::Client::new();
                let model = client.embedding_model(&rig_fastembed::FastembedModel::AllMiniLML6V2);
                self.embedding_model = Some(MemoryEmbeddingModel::Fastembed(model));
            }
            EmbeddingProvider::VoyageAi {
                api_key,
                model,
                dimensions,
            } => {
                let model =
                    VoyageAIEmbeddingModel::new(api_key.clone(), model.clone(), *dimensions);
                self.embedding_model = Some(MemoryEmbeddingModel::VoyageAI(model));
            }
        }
        let documents = memory.read().await.entities().clone();
        let Some(model) = self.embedding_model.as_ref() else {
            return Ok(());
        };

        for document in documents.iter() {
            if let Some((_, emb)) = embedding_storage
                .embeddings
                .iter()
                .find(|(id, _)| id == &document.name)
            {
                self.vector_store.add_documents_with_ids(vec![(
                    document.name.clone(),
                    document.clone(),
                    emb.clone(),
                )]);
            } else {
                self.vector_store.add_documents_with_ids(vec![(
                    document.name.clone(),
                    document.clone(),
                    model.embeddings(document).await?,
                )]);
            }
        }

        self.save_embeddings().await?;
        Ok(())
    }

    pub async fn index(&mut self, entities: Vec<Entity>) -> Result<()> {
        if let Some(model) = &self.embedding_model {
            for entity in &entities {
                let Ok(embeddings) = model.embeddings(entity).await else {
                    tracing::warn!("Failed to get embeddings for entity {}", entity.name);
                    continue;
                };
                self.vector_store.add_documents_with_ids(vec![(
                    entity.name.clone(),
                    entity.clone(),
                    embeddings,
                )]);
            }
        }
        self.save_embeddings().await.map_err(|e| anyhow!(e))?;
        Ok(())
    }

    pub async fn search(&self, query: &str, limit: usize) -> color_eyre::Result<Vec<Entity>> {
        if let Some(model) = &self.embedding_model {
            model.search(self.vector_store.clone(), query, limit).await
        } else {
            Ok(Vec::new())
        }
    }

    async fn save_embeddings(&self) -> color_eyre::Result<()> {
        let embeddings = self
            .vector_store
            .iter()
            .map(|(id, embeddings)| (id.clone(), embeddings.1.clone()))
            .collect::<Vec<_>>();
        let storage = MemoryVectorStorage { embeddings };
        std::fs::write(
            &self.embedding_storage_path,
            serde_json::to_string(&storage)?,
        )
        .unwrap();
        Ok(())
    }
}
