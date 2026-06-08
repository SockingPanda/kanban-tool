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
    fn neighbors(
        &self,
        entity_uri: &EntityUri,
        predicate: Option<Predicate>,
        limit: usize,
    ) -> Result<Vec<Relation>, GraphError>;
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

    fn neighbors(
        &self,
        _entity_uri: &EntityUri,
        _predicate: Option<Predicate>,
        _limit: usize,
    ) -> Result<Vec<Relation>, GraphError> {
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
