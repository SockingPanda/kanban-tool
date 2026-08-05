use kanban_application::{BoardColumnRecord as ApplicationBoardColumn, BoardColumns};
use kanban_core::{Result, TaskStatus};

use crate::adapter::{TursoApplicationStore, store_error};

impl BoardColumns for TursoApplicationStore {
    async fn list_board_columns(&self, board: &str) -> Result<Vec<ApplicationBoardColumn>> {
        self.store
            .list_board_columns(board)
            .await
            .map_err(store_error)?
            .into_iter()
            .map(|column| {
                Ok(ApplicationBoardColumn {
                    id: column.id,
                    board_id: column.board_id,
                    status: column.status.parse::<TaskStatus>()?,
                    title: column.title,
                    position: column.position,
                    hidden: column.hidden,
                    wip_limit: column.wip_limit,
                    created_at: column.created_at,
                    updated_at: column.updated_at,
                })
            })
            .collect()
    }
}
