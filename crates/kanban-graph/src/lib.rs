use kanban_entity::{EntityUri, Predicate, Relation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphStoreStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
}

pub trait RelationGraph {
    fn status(&self) -> GraphStoreStatus;
    fn init(&self) -> Result<(), GraphError>;
    fn upsert(&self, relations: &[Relation]) -> Result<(), GraphError>;
    fn delete(&self, entity_uri: &EntityUri) -> Result<(), GraphError>;
    fn rebuild(&self, relations: &[Relation]) -> Result<(), GraphError>;
    fn replace_entities(
        &self,
        entity_uris: &[EntityUri],
        relations: &[Relation],
    ) -> Result<(), GraphError> {
        for entity_uri in entity_uris {
            self.delete(entity_uri)?;
        }
        self.upsert(relations)
    }
    fn neighbors(
        &self,
        entity_uri: &EntityUri,
        predicate: Option<Predicate>,
        limit: usize,
    ) -> Result<Vec<Relation>, GraphError>;
    fn query(&self, sparql: &str, limit: usize) -> Result<Vec<GraphQueryRow>, GraphError>;
}

#[derive(Debug, Clone, Default)]
pub struct DisabledGraphStore;

impl RelationGraph for DisabledGraphStore {
    fn status(&self) -> GraphStoreStatus {
        GraphStoreStatus {
            backend: "disabled".to_owned(),
            enabled: false,
            message: "Graph store is disabled; SQLite-derived relations remain the source contract"
                .to_owned(),
        }
    }

    fn init(&self) -> Result<(), GraphError> {
        Err(GraphError::Disabled)
    }

    fn upsert(&self, _relations: &[Relation]) -> Result<(), GraphError> {
        Err(GraphError::Disabled)
    }

    fn delete(&self, _entity_uri: &EntityUri) -> Result<(), GraphError> {
        Err(GraphError::Disabled)
    }

    fn rebuild(&self, _relations: &[Relation]) -> Result<(), GraphError> {
        Err(GraphError::Disabled)
    }

    fn neighbors(
        &self,
        _entity_uri: &EntityUri,
        _predicate: Option<Predicate>,
        _limit: usize,
    ) -> Result<Vec<Relation>, GraphError> {
        Ok(Vec::new())
    }

    fn query(&self, _sparql: &str, _limit: usize) -> Result<Vec<GraphQueryRow>, GraphError> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("graph store is disabled")]
    Disabled,
    #[error("graph store error: {0}")]
    Store(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQueryRow {
    pub bindings: Vec<GraphQueryBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphQueryBinding {
    pub name: String,
    pub value: String,
}
