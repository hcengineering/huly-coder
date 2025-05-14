#[cfg(test)]
mod tests {
    use crate::tools::memory::*;
    use serde_json::json;

    fn setup() -> MemoryManager {
        MemoryManager::new(true) // Use memory-only mode for tests
    }

    #[test]
    fn test_create_entities() {
        let mut manager = setup();
        let args = json!({
            "entities": [{
                "name": "test_entity",
                "entityType": "test",
                "observations": ["test observation"]
            }]
        });

        let result = manager.call_tool("create_entities", args).unwrap();
        let entities: Vec<Entity> = serde_json::from_str(&result).unwrap();

        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "test_entity");
        assert_eq!(entities[0].entity_type, "test");
        assert_eq!(entities[0].observations, vec!["test observation"]);
    }

    #[test]
    fn test_create_relations() {
        let mut manager = setup();

        // First create some entities
        let entities_args = json!({
            "entities": [
                {
                    "name": "entity1",
                    "entityType": "test",
                    "observations": []
                },
                {
                    "name": "entity2",
                    "entityType": "test",
                    "observations": []
                }
            ]
        });
        manager.call_tool("create_entities", entities_args).unwrap();

        // Then create a relation between them
        let relations_args = json!({
            "relations": [{
                "from": "entity1",
                "to": "entity2",
                "relationType": "test_relation"
            }]
        });

        let result = manager
            .call_tool("create_relations", relations_args)
            .unwrap();
        let relations: Vec<Relation> = serde_json::from_str(&result).unwrap();

        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].from, "entity1");
        assert_eq!(relations[0].to, "entity2");
        assert_eq!(relations[0].relation_type, "test_relation");
    }

    #[test]
    fn test_add_observations() {
        let mut manager = setup();

        // First create an entity
        let entity_args = json!({
            "entities": [{
                "name": "test_entity",
                "entityType": "test",
                "observations": []
            }]
        });
        manager.call_tool("create_entities", entity_args).unwrap();

        // Then add observations
        let obs_args = json!({
            "observations": [{
                "entityName": "test_entity",
                "observations": ["new observation"]
            }]
        });

        let result = manager.call_tool("add_observations", obs_args).unwrap();
        let added: Vec<AddObservationsResult> = serde_json::from_str(&result).unwrap();

        assert_eq!(added.len(), 1);
        assert_eq!(added[0].entity_name, "test_entity");
        assert_eq!(added[0].added_observations, vec!["new observation"]);
    }

    #[test]
    fn test_delete_entities() {
        let mut manager = setup();

        // First create an entity
        let entity_args = json!({
            "entities": [{
                "name": "test_entity",
                "entityType": "test",
                "observations": []
            }]
        });
        manager.call_tool("create_entities", entity_args).unwrap();

        // Then delete it
        let delete_args = json!({
            "entityNames": ["test_entity"]
        });

        let result = manager.call_tool("delete_entities", delete_args).unwrap();
        assert_eq!(result, "Entities deleted successfully");

        // Verify it's gone by reading the graph
        let graph: KnowledgeGraph =
            serde_json::from_str(&manager.call_tool("read_graph", json!({})).unwrap()).unwrap();
        assert_eq!(graph.entities.len(), 0);
    }

    #[test]
    fn test_delete_observations() {
        let mut manager = setup();

        // Create entity with observation
        let entity_args = json!({
            "entities": [{
                "name": "test_entity",
                "entityType": "test",
                "observations": ["test observation"]
            }]
        });
        manager.call_tool("create_entities", entity_args).unwrap();

        // Delete the observation
        let delete_args = json!({
            "deletions": [{
                "entityName": "test_entity",
                "observations": ["test observation"]
            }]
        });

        let result = manager
            .call_tool("delete_observations", delete_args)
            .unwrap();
        assert_eq!(result, "Observations deleted successfully");

        // Verify observation is gone
        let graph: KnowledgeGraph =
            serde_json::from_str(&manager.call_tool("read_graph", json!({})).unwrap()).unwrap();
        assert_eq!(graph.entities[0].observations.len(), 0);
    }

    #[test]
    fn test_delete_relations() {
        let mut manager = setup();

        // Create entities and relation
        let entities_args = json!({
            "entities": [
                {
                    "name": "entity1",
                    "entityType": "test",
                    "observations": []
                },
                {
                    "name": "entity2",
                    "entityType": "test",
                    "observations": []
                }
            ]
        });
        manager.call_tool("create_entities", entities_args).unwrap();

        let relations_args = json!({
            "relations": [{
                "from": "entity1",
                "to": "entity2",
                "relationType": "test_relation"
            }]
        });
        manager
            .call_tool("create_relations", relations_args.clone())
            .unwrap();

        // Delete the relation
        let result = manager
            .call_tool("delete_relations", relations_args)
            .unwrap();
        assert_eq!(result, "Relations deleted successfully");

        // Verify relation is gone
        let graph: KnowledgeGraph =
            serde_json::from_str(&manager.call_tool("read_graph", json!({})).unwrap()).unwrap();
        assert_eq!(graph.relations.len(), 0);
    }

    #[test]
    fn test_search_nodes() {
        let mut manager = setup();

        // Create an entity
        let entity_args = json!({
            "entities": [{
                "name": "searchable_entity",
                "entityType": "test",
                "observations": ["searchable observation"]
            }]
        });
        manager.call_tool("create_entities", entity_args).unwrap();

        // Search for it
        let search_args = json!({
            "query": "observation"
        });

        let result = manager.call_tool("search_nodes", search_args).unwrap();
        let graph: KnowledgeGraph = serde_json::from_str(&result).unwrap();

        assert_eq!(graph.entities.len(), 1);
        assert_eq!(graph.entities[0].name, "searchable_entity");
    }

    #[test]
    fn test_open_nodes() {
        let mut manager = setup();

        // Create an entity
        let entity_args = json!({
            "entities": [{
                "name": "test_entity",
                "entityType": "test",
                "observations": ["test observation"]
            }]
        });
        manager.call_tool("create_entities", entity_args).unwrap();

        // Open it
        let open_args = json!({
            "names": ["test_entity"]
        });

        let result = manager.call_tool("open_nodes", open_args).unwrap();
        let entities: Vec<Entity> = serde_json::from_str(&result).unwrap();

        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "test_entity");
    }

    #[test]
    fn test_unknown_tool() {
        let mut manager = setup();
        let result = manager.call_tool("nonexistent_tool", json!({}));
        assert!(result.is_err());
    }
}
