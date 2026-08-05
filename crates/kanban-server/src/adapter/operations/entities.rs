use kanban_service::dto::EntityRecord;
use kanban_service::operations::{EntityListOptions, EntityQuery, EntityUpsertCommand};
use kanban_core::Result;
use kanban_store_turso::{EntityListOptions as StoreEntityListOptions, EntityUpsertInput};

use crate::adapter::{TursoApplicationStore, store_error};

impl EntityQuery for TursoApplicationStore {
    async fn list_entities(&self, options: EntityListOptions) -> Result<Vec<EntityRecord>> {
        self.store
            .list_entities(StoreEntityListOptions {
                board: options.board,
                kind: options.kind,
                limit: options.limit,
            })
            .await
            .map_err(store_error)
            .map(|entities| entities.into_iter().map(application_entity).collect())
    }

    async fn get_entity(&self, uri: &str) -> Result<EntityRecord> {
        self.store
            .get_entity(uri)
            .await
            .map_err(store_error)
            .map(application_entity)
    }

    async fn upsert_entity(&self, command: EntityUpsertCommand) -> Result<EntityRecord> {
        self.store
            .upsert_entity(EntityUpsertInput {
                uri: command.uri,
                kind: command.kind,
                source_table: command.source_table,
                source_id: command.source_id,
                board: command.board,
                task_id: command.task_id,
                title: command.title,
                summary: command.summary,
                content_hash: command.content_hash,
                archived_at: command.archived_at,
            })
            .await
            .map_err(store_error)
            .map(application_entity)
    }
}

fn application_entity(entity: kanban_store_turso::EntityRecord) -> EntityRecord {
    EntityRecord {
        uri: entity.uri,
        kind: entity.kind,
        source_table: entity.source_table,
        source_id: entity.source_id,
        board_id: entity.board_id,
        task_id: entity.task_id,
        title: entity.title,
        summary: entity.summary,
        content_hash: entity.content_hash,
        created_at: entity.created_at,
        updated_at: entity.updated_at,
        archived_at: entity.archived_at,
    }
}
