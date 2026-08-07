use kanban_core::{Clock, Result};

use crate::{BoardColumnRecord, KanbanService};

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_board_columns(&self, board: &str) -> Result<Vec<BoardColumnRecord>> {
        self.store
            .list_board_columns(board)
            .await
            .map_err(crate::error::store_error)?
            .into_iter()
            .map(|column| {
                Ok(BoardColumnRecord {
                    id: column.id,
                    board_id: column.board_id,
                    status: column.status.parse()?,
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
