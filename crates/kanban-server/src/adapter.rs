use kanban_application::{
    ApplicationStore, BoardColumnRecord as ApplicationBoardColumn, BoardRecord,
};
use kanban_core::{Board, KanbanError, Result, TaskStatus};
use kanban_store_turso::{StoreError, TursoStore};

#[derive(Clone)]
pub(crate) struct TursoApplicationStore {
    store: TursoStore,
}

impl TursoApplicationStore {
    pub(crate) fn new(store: TursoStore) -> Self {
        Self { store }
    }
}

impl ApplicationStore for TursoApplicationStore {
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

fn store_error(error: StoreError) -> KanbanError {
    match error {
        StoreError::BoardNotFound(selector) => KanbanError::NotFound(format!("board {selector}")),
        other => KanbanError::Storage(other.to_string()),
    }
}
