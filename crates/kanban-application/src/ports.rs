use std::future::Future;

use kanban_core::Result;

use crate::{BoardColumnRecord, BoardRecord};

/// Persistence port used only by the shared application service.
///
/// The concrete Turso implementation is adapted inside `kanban-server`, which
/// keeps the storage crate out of every other product adapter.
pub trait ApplicationStore: Clone + Send + Sync + 'static {
    fn list_boards(
        &self,
        include_archived: bool,
    ) -> impl Future<Output = Result<Vec<BoardRecord>>> + Send;

    fn list_board_columns(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<Vec<BoardColumnRecord>>> + Send;
}
