use kanban_core::{Clock, KanbanError, Result};

use crate::store_operations::StoreRelationListOptions;
use crate::{
    KanbanService, RelationDeleteInput, RelationPredicateInput, RelationPredicateRecord,
    RelationRecord, RelationUpsertInput,
};

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

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_relation_predicates(&self) -> Result<Vec<RelationPredicateRecord>> {
        self.application
            .store
            .store
            .list_relation_predicates()
            .await
            .map_err(crate::adapter::store_error)
            .map(|predicates| predicates.into_iter().map(application_predicate).collect())
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
        self.application
            .store
            .store
            .upsert_relation_predicate(RelationPredicateInput {
                name: command.name,
                domain_kind: command.domain_kind,
                range_kind: command.range_kind,
                cardinality: command.cardinality,
                authoritative_store: command.authoritative_store,
                description: command.description,
            })
            .await
            .map_err(crate::adapter::store_error)
            .map(application_predicate)
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
        self.application
            .store
            .store
            .list_relations(StoreRelationListOptions {
                board: options.board,
                subject_uri: options.subject_uri,
                object_uri: options.object_uri,
                predicate: options.predicate,
                limit: options.limit,
            })
            .await
            .map_err(crate::adapter::store_error)
            .map(|relations| relations.into_iter().map(application_relation).collect())
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
        self.application
            .store
            .store
            .upsert_relation(RelationUpsertInput {
                subject_uri: command.subject_uri,
                predicate: command.predicate,
                object_uri: command.object_uri,
                graph_uri: command.graph_uri,
                board: command.board,
                authoritative_store: command.authoritative_store,
                source_table: command.source_table,
                source_id: command.source_id,
                source_event_id: command.source_event_id,
                metadata_json: command.metadata_json,
            })
            .await
            .map_err(crate::adapter::store_error)
            .map(application_relation)
    }

    pub async fn delete_relation(&self, command: RelationDeleteCommand) -> Result<bool> {
        self.application
            .store
            .store
            .delete_relation(RelationDeleteInput {
                subject_uri: command.subject_uri,
                predicate: command.predicate,
                object_uri: command.object_uri,
                graph_uri: command.graph_uri,
                board: command.board,
            })
            .await
            .map_err(crate::adapter::store_error)
    }
}

fn application_predicate(
    predicate: crate::domain::RelationPredicateRecord,
) -> RelationPredicateRecord {
    RelationPredicateRecord {
        name: predicate.name,
        domain_kind: predicate.domain_kind,
        range_kind: predicate.range_kind,
        cardinality: predicate.cardinality,
        authoritative_store: predicate.authoritative_store,
        description: predicate.description,
        created_at: predicate.created_at,
    }
}

pub(crate) fn application_relation(relation: crate::domain::RelationRecord) -> RelationRecord {
    RelationRecord {
        id: relation.id,
        subject_uri: relation.subject_uri,
        predicate: relation.predicate,
        object_uri: relation.object_uri,
        graph_uri: relation.graph_uri,
        board_id: relation.board_id,
        authoritative_store: relation.authoritative_store,
        source_table: relation.source_table,
        source_id: relation.source_id,
        source_event_id: relation.source_event_id,
        metadata_json: relation.metadata_json,
        created_at: relation.created_at,
        updated_at: relation.updated_at,
    }
}
