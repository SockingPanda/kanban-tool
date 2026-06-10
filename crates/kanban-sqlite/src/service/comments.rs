use crate::connect_file;

use super::{
    CommentRecord, active_board_id_for_task, insert_event, resolve_task,
    resolve_task_without_active_board, storage, with_immediate_tx,
};

use std::path::Path;

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_typed_id};

use rusqlite::{Row, params};

use serde_json::json;

pub fn create_comment(
    path: impl AsRef<Path>,
    task_ref: &str,
    author: &str,
    body: &str,
    kind: Option<&str>,
) -> Result<CommentRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_task(&conn, task_ref)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        let author = author.trim();
        if author.is_empty() {
            return Err(KanbanError::InvalidInput(
                "comment author is required".into(),
            ));
        }
        let body = body.trim();
        if body.is_empty() {
            return Err(KanbanError::InvalidInput("comment body is required".into()));
        }
        let kind = kind.unwrap_or("text").trim();
        if !matches!(kind, "text" | "system" | "worker") {
            return Err(KanbanError::InvalidInput("invalid comment kind".into()));
        }
        let id = new_typed_id("c");
        conn.execute(
            "INSERT INTO task_comments(id, board_id, task_id, author, body, kind, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, board_id, task.id, author, body, kind, now],
        )
        .map_err(storage)?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.comment.created",
            author,
            &json!({"comment_id": id, "kind": kind}).to_string(),
            now,
        )?;
        Ok(CommentRecord {
            id,
            board_id,
            task_id: task.id,
            author: author.to_owned(),
            body: body.to_owned(),
            kind: kind.to_owned(),
            created_at: now,
        })
    })
}

pub fn list_comments(path: impl AsRef<Path>, task_ref: &str) -> Result<Vec<CommentRecord>> {
    let conn = connect_file(path.as_ref())?;
    let task = resolve_task_without_active_board(&conn, task_ref)?;
    let board_id = task.board_id.clone();
    let mut stmt = conn
        .prepare(
            "SELECT id, board_id, task_id, author, body, kind, created_at \
             FROM task_comments WHERE board_id=?1 AND task_id=?2 ORDER BY created_at ASC, id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(params![board_id, task.id], comment_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

pub(crate) fn comment_from_row(row: &Row<'_>) -> rusqlite::Result<CommentRecord> {
    Ok(CommentRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        author: row.get(3)?,
        body: row.get(4)?,
        kind: row.get(5)?,
        created_at: row.get(6)?,
    })
}
