mod archive;
mod columns;
mod create;
mod get;
mod list;

use crate::state::AppState;
use axum::Router;
use kanban_core::Board;
use kanban_protocol::ApiBoard;

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

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .merge(list::router())
        .merge(columns::router())
        .merge(create::router())
        .merge(get::router())
        .merge(archive::router())
}

#[cfg(test)]
mod tests;
