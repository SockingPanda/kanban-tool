use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, RelationPredicateRecord, RelationRecord};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelationListOptions {
    pub board: Option<String>,
    pub subject_uri: Option<String>,
    pub object_uri: Option<String>,
    pub predicate: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationPredicateCommand {
    pub name: String,
    pub domain_kind: Option<String>,
    pub range_kind: Option<String>,
    pub cardinality: String,
    pub authoritative_store: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationUpsertCommand {
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub graph_uri: String,
    pub board: Option<String>,
    pub authoritative_store: String,
    pub source_table: Option<String>,
    pub source_id: Option<String>,
    pub source_event_id: Option<i64>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationDeleteCommand {
    pub subject_uri: String,
    pub predicate: String,
    pub object_uri: String,
    pub graph_uri: String,
    pub board: Option<String>,
}

pub trait RelationQuery: ApplicationStore {
    fn list_relation_predicates(
        &self,
    ) -> impl Future<Output = Result<Vec<RelationPredicateRecord>>> + Send;
    fn upsert_relation_predicate(
        &self,
        command: RelationPredicateCommand,
    ) -> impl Future<Output = Result<RelationPredicateRecord>> + Send;
    fn list_relations(
        &self,
        options: RelationListOptions,
    ) -> impl Future<Output = Result<Vec<RelationRecord>>> + Send;
    fn upsert_relation(
        &self,
        command: RelationUpsertCommand,
    ) -> impl Future<Output = Result<RelationRecord>> + Send;
    fn delete_relation(
        &self,
        command: RelationDeleteCommand,
    ) -> impl Future<Output = Result<bool>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: RelationQuery,
    C: Clock,
{
    pub async fn list_relation_predicates(&self) -> Result<Vec<RelationPredicateRecord>> {
        self.store.list_relation_predicates().await
    }

    pub async fn upsert_relation_predicate(
        &self,
        command: RelationPredicateCommand,
    ) -> Result<RelationPredicateRecord> {
        if command.name.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "predicate name is required".to_owned(),
            ));
        }
        self.store.upsert_relation_predicate(command).await
    }

    pub async fn list_relations(
        &self,
        options: RelationListOptions,
    ) -> Result<Vec<RelationRecord>> {
        if options.limit == 0 || options.limit > 1_000 {
            return Err(KanbanError::InvalidInput(
                "relation limit must be between 1 and 1000".to_owned(),
            ));
        }
        self.store.list_relations(options).await
    }

    pub async fn upsert_relation(&self, command: RelationUpsertCommand) -> Result<RelationRecord> {
        if command.subject_uri.trim().is_empty()
            || command.object_uri.trim().is_empty()
            || command.predicate.trim().is_empty()
        {
            return Err(KanbanError::InvalidInput(
                "relation subject, predicate and object are required".to_owned(),
            ));
        }
        self.store.upsert_relation(command).await
    }

    pub async fn delete_relation(&self, command: RelationDeleteCommand) -> Result<bool> {
        self.store.delete_relation(command).await
    }
}
