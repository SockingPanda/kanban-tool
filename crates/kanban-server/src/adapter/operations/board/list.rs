use kanban_service::{BoardList, BoardRecord};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};

use super::application_board;

impl BoardList for TursoApplicationStore {
    async fn list_boards(&self, include_archived: bool) -> Result<Vec<BoardRecord>> {
        self.store
            .list_boards(include_archived)
            .await
            .map(|boards| boards.into_iter().map(application_board).collect())
            .map_err(store_error)
    }
}
