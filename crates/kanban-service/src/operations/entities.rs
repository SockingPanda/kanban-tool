use kanban_core::{Clock, KanbanError, Result};

use crate::store_operations::StoreEntityListOptions;
use crate::{EntityRecord, EntityUpsertInput, KanbanService};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntityListOptions {
    pub board: Option<String>,
    pub kind: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityUpsertCommand {
    pub uri: String,
    pub kind: String,
    pub source_table: String,
    pub source_id: String,
    pub board: Option<String>,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub archived_at: Option<i64>,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_entities(&self, options: EntityListOptions) -> Result<Vec<EntityRecord>> {
        if options.limit == 0 || options.limit > 1_000 {
            return Err(KanbanError::InvalidInput(
                "entity limit must be between 1 and 1000".to_owned(),
            ));
        }
        if options
            .board
            .as_deref()
            .is_some_and(|board| board.trim().is_empty())
        {
            return Err(KanbanError::InvalidInput(
                "board cannot be empty".to_owned(),
            ));
        }
        self.store
            .list_entities(StoreEntityListOptions {
                board: options.board,
                kind: options.kind,
                limit: options.limit,
            })
            .await
            .map_err(crate::error::store_error)
            .map(|entities| entities.into_iter().map(application_entity).collect())
    }

    pub async fn get_entity(&self, uri: &str) -> Result<EntityRecord> {
        let uri = uri.trim();
        if !uri.starts_with("kb://") || uri.len() <= 5 {
            return Err(KanbanError::InvalidInput(
                "entity uri must start with kb://".to_owned(),
            ));
        }
        self.store
            .get_entity(uri)
            .await
            .map_err(crate::error::store_error)
            .map(application_entity)
    }

    pub async fn upsert_entity(&self, command: EntityUpsertCommand) -> Result<EntityRecord> {
        if command.uri.trim().is_empty() || command.kind.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "entity uri and kind are required".to_owned(),
            ));
        }
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
            .map_err(crate::error::store_error)
            .map(application_entity)
    }
}

fn application_entity(entity: crate::domain::EntityRecord) -> EntityRecord {
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
