use kanban_service::{BoardCreate, BoardRecord, CreateBoardRecord};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};

use super::application_board;

impl BoardCreate for TursoApplicationStore {
    async fn create_board(&self, input: CreateBoardRecord) -> Result<BoardRecord> {
        self.store
            .create_board(kanban_store_turso::CreateBoardInput {
                id: input.id,
                slug: input.slug,
                name: input.name,
                description: input.description,
                actor: input.actor,
                event_id: input.event_id,
                created_at: input.created_at,
            })
            .await
            .map(application_board)
            .map_err(store_error)
    }
}
