use kanban_core::{Clock, Result};

use crate::{ApplicationService, ApplicationStore, BoardColumnRecord};

impl<S, C> ApplicationService<S, C>
where
    S: ApplicationStore,
    C: Clock,
{
    pub async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
        self.store.list_board_columns(board).await
    }
}
