use kanban_application::{ArchiveBoardRecord, BoardArchive, BoardRecord};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};

use super::application_board;

impl BoardArchive for TursoApplicationStore {
    async fn archive_board(&self, board: &str, input: ArchiveBoardRecord) -> Result<BoardRecord> {
        self.store
            .archive_board(
                board,
                kanban_store_turso::ArchiveBoardInput {
                    actor: input.actor,
                    event_id: input.event_id,
                    archived_at: input.archived_at,
                },
            )
            .await
            .map(application_board)
            .map_err(store_error)
    }
}
