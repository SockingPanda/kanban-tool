use kanban_entity::EntityUri;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxTarget {
    Tantivy,
    Oxigraph,
    Lancedb,
    All,
}

impl OutboxTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tantivy => "tantivy",
            Self::Oxigraph => "oxigraph",
            Self::Lancedb => "lancedb",
            Self::All => "all",
        }
    }
}

impl fmt::Display for OutboxTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxAction {
    Upsert,
    Delete,
    Rebuild,
}

impl OutboxAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
            Self::Rebuild => "rebuild",
        }
    }
}

impl fmt::Display for OutboxAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub const OUTBOX_FANOUT_TARGETS: &[OutboxTarget] = &[
    OutboxTarget::Tantivy,
    OutboxTarget::Oxigraph,
    OutboxTarget::Lancedb,
];

pub const TANTIVY_TASKS_STORE: &str = "tantivy_tasks";
pub const OXIGRAPH_RELATIONS_STORE: &str = "oxigraph_relations";
pub const LANCEDB_CHUNKS_STORE: &str = "lancedb_chunks";
pub const LANCEDB_LABEL_ATOMS_STORE: &str = "lancedb_label_atoms";
pub const DERIVED_STORE_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedStoreSeed {
    pub store_name: &'static str,
    pub target: OutboxTarget,
    pub schema_version: i64,
}

pub const DERIVED_STORE_SEEDS: &[DerivedStoreSeed] = &[
    DerivedStoreSeed {
        store_name: TANTIVY_TASKS_STORE,
        target: OutboxTarget::Tantivy,
        schema_version: DERIVED_STORE_SCHEMA_VERSION,
    },
    DerivedStoreSeed {
        store_name: OXIGRAPH_RELATIONS_STORE,
        target: OutboxTarget::Oxigraph,
        schema_version: DERIVED_STORE_SCHEMA_VERSION,
    },
    DerivedStoreSeed {
        store_name: LANCEDB_CHUNKS_STORE,
        target: OutboxTarget::Lancedb,
        schema_version: DERIVED_STORE_SCHEMA_VERSION,
    },
    DerivedStoreSeed {
        store_name: LANCEDB_LABEL_ATOMS_STORE,
        target: OutboxTarget::Lancedb,
        schema_version: DERIVED_STORE_SCHEMA_VERSION,
    },
];

pub const OUTBOX_DERIVED_STORE_SEEDS: &[DerivedStoreSeed] = &[
    DerivedStoreSeed {
        store_name: TANTIVY_TASKS_STORE,
        target: OutboxTarget::Tantivy,
        schema_version: DERIVED_STORE_SCHEMA_VERSION,
    },
    DerivedStoreSeed {
        store_name: OXIGRAPH_RELATIONS_STORE,
        target: OutboxTarget::Oxigraph,
        schema_version: DERIVED_STORE_SCHEMA_VERSION,
    },
    DerivedStoreSeed {
        store_name: LANCEDB_CHUNKS_STORE,
        target: OutboxTarget::Lancedb,
        schema_version: DERIVED_STORE_SCHEMA_VERSION,
    },
];

pub fn derived_store_for_target(target: OutboxTarget) -> Option<&'static DerivedStoreSeed> {
    DERIVED_STORE_SEEDS
        .iter()
        .find(|seed| seed.target == target)
}

pub fn derived_store_for_name(store_name: &str) -> Option<&'static DerivedStoreSeed> {
    DERIVED_STORE_SEEDS
        .iter()
        .find(|seed| seed.store_name == store_name)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedStoreUpdate {
    pub store_name: &'static str,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_rebuild_at: Option<i64>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at: i64,
}

impl DerivedStoreUpdate {
    pub fn dirty(seed: &'static DerivedStoreSeed, now: i64) -> Self {
        Self {
            store_name: seed.store_name,
            schema_version: seed.schema_version,
            last_event_id: 0,
            dirty: true,
            last_rebuild_at: None,
            last_sync_at: None,
            last_error: None,
            updated_at: now,
        }
    }

    pub fn success(
        seed: &'static DerivedStoreSeed,
        last_event_id: Option<i64>,
        rebuilt: bool,
        now: i64,
    ) -> Self {
        Self {
            store_name: seed.store_name,
            schema_version: seed.schema_version,
            last_event_id: last_event_id.unwrap_or(0),
            dirty: false,
            last_rebuild_at: rebuilt.then_some(now),
            last_sync_at: (!rebuilt).then_some(now),
            last_error: None,
            updated_at: now,
        }
    }

    pub fn failure(seed: &'static DerivedStoreSeed, error: &str, now: i64) -> Self {
        Self {
            store_name: seed.store_name,
            schema_version: seed.schema_version,
            last_event_id: 0,
            dirty: true,
            last_rebuild_at: None,
            last_sync_at: None,
            last_error: Some(error.to_owned()),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutboxJob {
    pub id: i64,
    pub source_event_id: Option<i64>,
    pub target: OutboxTarget,
    pub entity_uri: EntityUri,
    pub action: OutboxAction,
    pub payload: serde_json::Value,
    pub attempts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedStoreStatus {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_rebuild_at: Option<i64>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at: i64,
}

pub trait StoreIndexer {
    fn target(&self) -> OutboxTarget;
    fn status(&self) -> std::result::Result<DerivedStoreStatus, IndexerError>;
    fn apply(&self, jobs: &[OutboxJob]) -> std::result::Result<DerivedStoreStatus, IndexerError>;
    fn rebuild(&self) -> std::result::Result<DerivedStoreStatus, IndexerError>;
}

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error("indexer target is disabled: {0}")]
    Disabled(OutboxTarget),
    #[error("indexer storage error: {0}")]
    Storage(String),
    #[error("indexer payload error: {0}")]
    Payload(String),
}

#[cfg(test)]
mod tests {
    use super::{
        DERIVED_STORE_SCHEMA_VERSION, LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE,
        OUTBOX_DERIVED_STORE_SEEDS, OUTBOX_FANOUT_TARGETS, OXIGRAPH_RELATIONS_STORE, OutboxTarget,
        TANTIVY_TASKS_STORE, derived_store_for_name, derived_store_for_target,
    };

    #[test]
    fn outbox_targets_map_to_committed_derived_stores() {
        assert_eq!(
            OUTBOX_FANOUT_TARGETS
                .iter()
                .map(|target| target.as_str())
                .collect::<Vec<_>>(),
            vec!["tantivy", "oxigraph", "lancedb"]
        );

        let stores = OUTBOX_FANOUT_TARGETS
            .iter()
            .map(|target| derived_store_for_target(*target).unwrap().store_name)
            .collect::<Vec<_>>();
        assert_eq!(
            stores,
            vec![
                TANTIVY_TASKS_STORE,
                OXIGRAPH_RELATIONS_STORE,
                LANCEDB_CHUNKS_STORE
            ]
        );
        assert_eq!(
            OUTBOX_DERIVED_STORE_SEEDS
                .iter()
                .map(|seed| seed.store_name)
                .collect::<Vec<_>>(),
            vec![
                TANTIVY_TASKS_STORE,
                OXIGRAPH_RELATIONS_STORE,
                LANCEDB_CHUNKS_STORE
            ]
        );
        assert_eq!(DERIVED_STORE_SCHEMA_VERSION, 1);
        assert_eq!(
            derived_store_for_name(TANTIVY_TASKS_STORE).unwrap().target,
            OutboxTarget::Tantivy
        );
        assert_eq!(
            derived_store_for_name(LANCEDB_LABEL_ATOMS_STORE)
                .unwrap()
                .target,
            OutboxTarget::Lancedb
        );
        assert!(derived_store_for_target(OutboxTarget::All).is_none());
    }
}
