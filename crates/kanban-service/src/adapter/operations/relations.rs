use crate::dto::{RelationPredicateRecord, RelationRecord};
use crate::operations::{
    RelationDeleteCommand, RelationListOptions, RelationPredicateCommand, RelationQuery,
    RelationUpsertCommand,
};
use crate::{
    RelationDeleteInput, RelationPredicateInput, RelationUpsertInput, StoreRelationListOptions,
};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};

impl RelationQuery for TursoApplicationStore {
    async fn list_relation_predicates(&self) -> Result<Vec<RelationPredicateRecord>> {
        self.store
            .list_relation_predicates()
            .await
            .map_err(store_error)
            .map(|predicates| predicates.into_iter().map(application_predicate).collect())
    }

    async fn upsert_relation_predicate(
        &self,
        command: RelationPredicateCommand,
    ) -> Result<RelationPredicateRecord> {
        self.store
            .upsert_relation_predicate(RelationPredicateInput {
                name: command.name,
                domain_kind: command.domain_kind,
                range_kind: command.range_kind,
                cardinality: command.cardinality,
                authoritative_store: command.authoritative_store,
                description: command.description,
            })
            .await
            .map_err(store_error)
            .map(application_predicate)
    }

    async fn list_relations(&self, options: RelationListOptions) -> Result<Vec<RelationRecord>> {
        self.store
            .list_relations(StoreRelationListOptions {
                board: options.board,
                subject_uri: options.subject_uri,
                object_uri: options.object_uri,
                predicate: options.predicate,
                limit: options.limit,
            })
            .await
            .map_err(store_error)
            .map(|relations| relations.into_iter().map(application_relation).collect())
    }

    async fn upsert_relation(&self, command: RelationUpsertCommand) -> Result<RelationRecord> {
        self.store
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
            .map_err(store_error)
            .map(application_relation)
    }

    async fn delete_relation(&self, command: RelationDeleteCommand) -> Result<bool> {
        self.store
            .delete_relation(RelationDeleteInput {
                subject_uri: command.subject_uri,
                predicate: command.predicate,
                object_uri: command.object_uri,
                graph_uri: command.graph_uri,
                board: command.board,
            })
            .await
            .map_err(store_error)
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

fn application_relation(relation: crate::domain::RelationRecord) -> RelationRecord {
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

pub(crate) fn application_relation_record(
    relation: crate::domain::RelationRecord,
) -> RelationRecord {
    application_relation(relation)
}
