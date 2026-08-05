use kanban_application::{BoardList, BoardRecord};
use kanban_core::{Board, Result};

use crate::adapter::{TursoApplicationStore, store_error};

impl BoardList for TursoApplicationStore {
    async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
        self.store
            .list_boards(include_archived)
            .await
            .map(|boards| {
                boards
                    .into_iter()
                    .map(|board| Board {
                        id: board.id,
                        slug: board.slug,
                        name: board.name,
                        description: board.description,
                        created_at: board.created_at,
                        updated_at: board.updated_at,
                        archived_at: board.archived_at,
                    })
                    .collect()
            })
            .map_err(store_error)
    }
}
