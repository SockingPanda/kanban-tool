mod archive;
mod columns;
mod create;
mod get;
mod list;

use kanban_core::Board;
use kanban_store_turso::BoardRecord;

pub(super) fn application_board(board: BoardRecord) -> Board {
    Board {
        id: board.id,
        slug: board.slug,
        name: board.name,
        description: board.description,
        created_at: board.created_at,
        updated_at: board.updated_at,
        archived_at: board.archived_at,
    }
}
