use kanban_application::{BoardGet, BoardRecord};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};

use super::application_board;

impl BoardGet for TursoApplicationStore {
    async fn get_board(&self, selector: &str) -> Result<BoardRecord> {
        self.store
            .get_board(selector)
            .await
            .map(application_board)
            .map_err(store_error)
    }
}
