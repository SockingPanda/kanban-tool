use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};

use crate::{ApplicationService, ApplicationStore, EntityRecord};

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

pub trait EntityQuery: ApplicationStore {
    fn list_entities(
        &self,
        options: EntityListOptions,
    ) -> impl Future<Output = Result<Vec<EntityRecord>>> + Send;
    fn get_entity(&self, uri: &str) -> impl Future<Output = Result<EntityRecord>> + Send;
    fn upsert_entity(
        &self,
        command: EntityUpsertCommand,
    ) -> impl Future<Output = Result<EntityRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: EntityQuery,
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
        self.store.list_entities(options).await
    }

    pub async fn get_entity(&self, uri: &str) -> Result<EntityRecord> {
        let uri = uri.trim();
        if !uri.starts_with("kb://") || uri.len() <= 5 {
            return Err(KanbanError::InvalidInput(
                "entity uri must start with kb://".to_owned(),
            ));
        }
        self.store.get_entity(uri).await
    }

    pub async fn upsert_entity(&self, command: EntityUpsertCommand) -> Result<EntityRecord> {
        if command.uri.trim().is_empty() || command.kind.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "entity uri and kind are required".to_owned(),
            ));
        }
        self.store.upsert_entity(command).await
    }
}
