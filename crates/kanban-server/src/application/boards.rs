pub(crate) mod archive;
pub(crate) mod columns;
pub(crate) mod create;
pub(crate) mod get;
pub(crate) mod list;
use kanban_protocol::ApiBoard;
use kanban_service::Board;
pub(super) fn api_board(board: Board) -> ApiBoard {
    ApiBoard {
        id: board.id,
        slug: board.slug,
        name: board.name,
        description: board.description,
        created_at: board.created_at,
        updated_at: board.updated_at,
        archived_at: board.archived_at,
    }
}
