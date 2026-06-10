use crate::connect_file;

use super::{board_id, graph_relation_snapshot_for_board};

use std::path::Path;

use kanban_core::Result;

use kanban_entity::Relation;

pub fn graph_relation_snapshot(path: impl AsRef<Path>, board: &str) -> Result<Vec<Relation>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    graph_relation_snapshot_for_board(&conn, &board_id)
}
