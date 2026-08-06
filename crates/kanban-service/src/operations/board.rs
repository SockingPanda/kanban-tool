mod archive;
mod columns;
mod create;
mod get;
mod list;

pub use archive::{ArchiveBoardCommand, ArchiveBoardRecord};
pub use create::{CreateBoardCommand, CreateBoardRecord};

fn application_board(board: crate::domain::BoardRecord) -> crate::BoardRecord {
    crate::BoardRecord {
        id: board.id,
        slug: board.slug,
        name: board.name,
        description: board.description,
        created_at: board.created_at,
        updated_at: board.updated_at,
        archived_at: board.archived_at,
    }
}
