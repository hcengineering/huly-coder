///
/// Based on https://github.com/modelcontextprotocol/servers/tree/main/src/memory MCP server
///
// Copyright © 2025 Huly Labs. Use of this source code is governed by the MIT license.
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indicium::simple::{Indexable, SearchIndex};
use rig::agent::AgentBuilder;
use rig::completion::{CompletionModel, ToolDefinition};
use rig::tool::Tool;
use rig::Embed;
use serde::{Deserialize, Serialize};

use super::AgentToolError;

#[cfg(test)]
mod tests;

const TOOLS_STR: &str = include_str!("tools.json");
const MEMORY_PATH: &str = "memory.yaml";

#[derive(Clone, Deserialize)]
struct JsonToolDefinition {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: serde_json::Value,
}

macro_rules! create_tool {
    ($func_name:ident, $tool_name:ident) => {
        paste::paste! {
            pub struct [<$func_name Tool>] {
                manager: Arc<tokio::sync::RwLock<MemoryManager>>,
            }

            impl [<$func_name Tool>] {
                pub(self) fn new(manager: Arc<tokio::sync::RwLock<MemoryManager>>) -> Self {
                    Self { manager }
                }
            }

            impl Tool for [<$func_name Tool>] {
                const NAME: &'static str = stringify!($tool_name);

                type Error = AgentToolError;
                type Args = serde_json::Value;
                type Output = String;

                async fn definition(&self, _prompt: String) -> ToolDefinition {
                    let defs = serde_json::from_str::<Vec<JsonToolDefinition>>(TOOLS_STR)
                        .unwrap()
                        .into_iter()
                        .find(|it| it.name == self.name())
                        .unwrap();
                    ToolDefinition {
                        name: self.name(),
                        description: defs.description,
                        parameters: defs.input_schema,
                    }
                }

                async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
                    self.manager.write().await.call_tool(&self.name(), args)
                }

                fn name(&self) -> String {
                    Self::NAME.to_string()
                }
            }
        }
    };
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Observation {
    #[serde(rename = "entityName")]
    pub entity_name: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AddObservationsResult {
    #[serde(rename = "entityName")]
    entity_name: String,
    #[serde(rename = "addedObservations")]
    added_observations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
    pub name: String,
    #[serde(rename = "entityType")]
    pub entity_type: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Relation {
    pub from: String,
    pub to: String,
    #[serde(rename = "relationType")]
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KnowledgeGraph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
}

pub struct MemoryManager {
    memory_only: bool,
    knowledge_graph: KnowledgeGraph,
    search_index: SearchIndex<usize>,
    data_dir: PathBuf,
}

impl Embed for Entity {
    fn embed(
        &self,
        embedder: &mut rig::embeddings::TextEmbedder,
    ) -> Result<(), rig::embeddings::EmbedError> {
        embedder.embed(self.name.clone());
        self.observations
            .iter()
            .for_each(|t| embedder.embed(t.clone()));
        Ok(())
    }
}

impl Indexable for Entity {
    fn strings(&self) -> Vec<String> {
        let mut res = vec![self.name.clone()];
        res.extend(self.observations.clone());
        res
    }
}

impl MemoryManager {
    pub fn new(data_dir: &str, memory_only: bool) -> Self {
        let knowledge_graph = if !memory_only {
            serde_yaml::from_str(
                &fs::read_to_string(Path::new(data_dir).join(MEMORY_PATH)).unwrap_or_default(),
            )
            .unwrap_or(KnowledgeGraph {
                entities: Vec::new(),
                relations: Vec::new(),
            })
        } else {
            KnowledgeGraph {
                entities: Vec::new(),
                relations: Vec::new(),
            }
        };

        let mut search_index = SearchIndex::default();
        knowledge_graph
            .entities
            .iter()
            .enumerate()
            .for_each(|(i, entity)| {
                search_index.insert(&i, entity);
            });

        Self {
            memory_only,
            knowledge_graph,
            search_index,
            data_dir: PathBuf::from(data_dir),
        }
    }

    pub fn entities(&self) -> &Vec<Entity> {
        &self.knowledge_graph.entities
    }

    pub fn call_tool(
        &mut self,
        toolname: &str,
        args: serde_json::Value,
    ) -> Result<String, AgentToolError> {
        match toolname {
            "create_entities" => {
                let mut entities: Vec<Entity> = serde_json::from_value(args["entities"].clone())?;
                entities.retain(|entity| {
                    !self
                        .knowledge_graph
                        .entities
                        .iter()
                        .any(|it| it.name == entity.name)
                });
                self.knowledge_graph.entities.extend(entities.clone());
                self.save();
                Ok(serde_json::to_string_pretty(&entities)?)
            }
            "create_relations" => {
                let mut relations: Vec<Relation> =
                    serde_json::from_value(args["relations"].clone())?;
                relations.retain(|relation| {
                    !self.knowledge_graph.relations.iter().any(|it| {
                        it.from == relation.from
                            && it.to == relation.to
                            && it.relation_type == relation.relation_type
                    })
                });
                self.knowledge_graph.relations.extend(relations.clone());
                self.save();
                Ok(serde_json::to_string_pretty(&relations)?)
            }
            "add_observations" => {
                let observations: Vec<Observation> =
                    serde_json::from_value(args["observations"].clone())?;
                let result = observations
                    .into_iter()
                    .map(|mut observation| {
                        let Some(entity) = self
                            .knowledge_graph
                            .entities
                            .iter_mut()
                            .find(|entity| entity.name == observation.entity_name)
                        else {
                            return Err(AgentToolError::Other(anyhow::anyhow!(
                                "Entity '{}' not found",
                                observation.entity_name
                            )));
                        };
                        observation
                            .observations
                            .retain(|it| !entity.observations.contains(it));
                        entity.observations.extend(observation.observations.clone());
                        Ok(AddObservationsResult {
                            entity_name: entity.name.clone(),
                            added_observations: observation.observations,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.save();
                Ok(serde_json::to_string_pretty(&result)?)
            }
            "delete_entities" => {
                let entity_names: Vec<String> =
                    serde_json::from_value(args["entityNames"].clone())?;
                for entity_name in entity_names {
                    self.knowledge_graph
                        .entities
                        .retain(|entity| entity.name != entity_name);
                    self.knowledge_graph.relations.retain(|relation| {
                        relation.from != entity_name || relation.to != entity_name
                    });
                }
                self.save();
                Ok("Entities deleted successfully".to_string())
            }
            "delete_observations" => {
                let observations: Vec<Observation> =
                    serde_json::from_value(args["deletions"].clone())?;
                for observation in observations {
                    if let Some(entity) = self
                        .knowledge_graph
                        .entities
                        .iter_mut()
                        .find(|entity| entity.name == observation.entity_name)
                    {
                        entity.observations.retain(|it| {
                            !observation
                                .observations
                                .iter()
                                .any(|observation_to_delete| it == observation_to_delete)
                        });
                    }
                }
                self.save();
                Ok("Observations deleted successfully".to_string())
            }
            "delete_relations" => {
                let relations: Vec<Relation> = serde_json::from_value(args["relations"].clone())?;
                for relation in relations {
                    self.knowledge_graph.relations.retain(|it| {
                        !(it.from == relation.from
                            && it.to == relation.to
                            && it.relation_type == relation.relation_type)
                    });
                }
                self.save();
                Ok("Relations deleted successfully".to_string())
            }
            "read_graph" => Ok(serde_json::to_string(&self.knowledge_graph).unwrap()),
            "search_nodes" => {
                let query = args["query"].as_str().unwrap();
                let indicies = self.search_index.search(query);
                let entities = self
                    .knowledge_graph
                    .entities
                    .iter()
                    .enumerate()
                    .filter_map(|(i, entity)| {
                        if indicies.contains(&&i) {
                            Some(entity.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                let entry_names = entities
                    .iter()
                    .map(|entity| entity.name.clone())
                    .collect::<HashSet<String>>();

                let relations = self
                    .knowledge_graph
                    .relations
                    .iter()
                    .filter_map(|relation| {
                        if entry_names.contains(&relation.from)
                            || entry_names.contains(&relation.to)
                        {
                            Some(relation.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                let result = KnowledgeGraph {
                    entities,
                    relations,
                };
                Ok(serde_json::to_string(&result).unwrap())
            }
            "open_nodes" => {
                let names: Vec<String> = serde_json::from_value(args["names"].clone())?;
                let entities: Vec<Entity> = self
                    .knowledge_graph
                    .entities
                    .iter()
                    .filter(|entity| names.contains(&entity.name))
                    .cloned()
                    .collect();
                Ok(serde_json::to_string(&entities).unwrap())
            }
            _ => Err(AgentToolError::Other(anyhow::anyhow!(
                "Unknown tool: {}",
                toolname
            ))),
        }
    }

    pub fn save(&mut self) {
        self.search_index.clear();
        self.knowledge_graph
            .entities
            .iter()
            .enumerate()
            .for_each(|(i, entity)| {
                self.search_index.insert(&i, entity);
            });
        if !self.memory_only {
            fs::write(
                self.data_dir.join(MEMORY_PATH),
                serde_yaml::to_string(&self.knowledge_graph).unwrap(),
            )
            .unwrap();
        }
    }
}

create_tool!(MemoryCreateEntities, create_entities);
create_tool!(MemoryCreateRelations, create_relations);
create_tool!(MemoryAddObservations, add_observations);
create_tool!(MemoryDeleteEntities, delete_entities);
create_tool!(MemoryDeleteObservations, delete_observations);
create_tool!(MemoryDeleteRelations, delete_relations);
create_tool!(MemoryReadGraph, read_graph);
create_tool!(MemorySearchNodes, search_nodes);
create_tool!(MemoryOpenNodes, open_nodes);

pub(crate) fn add_memory_tools<M>(
    agent_builder: AgentBuilder<M>,
    memory: Arc<tokio::sync::RwLock<MemoryManager>>,
) -> AgentBuilder<M>
where
    M: CompletionModel,
{
    agent_builder
        .tool(MemoryCreateEntitiesTool::new(memory.clone()))
        .tool(MemoryCreateRelationsTool::new(memory.clone()))
        .tool(MemoryAddObservationsTool::new(memory.clone()))
        .tool(MemoryDeleteEntitiesTool::new(memory.clone()))
        .tool(MemoryDeleteObservationsTool::new(memory.clone()))
        .tool(MemoryDeleteRelationsTool::new(memory.clone()))
        .tool(MemoryReadGraphTool::new(memory.clone()))
        .tool(MemorySearchNodesTool::new(memory.clone()))
        .tool(MemoryOpenNodesTool::new(memory.clone()))
}
