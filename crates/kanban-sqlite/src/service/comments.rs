use crate::connect_file;

use super::{
    CommentRecord, CreateComment, active_board_id_for_task, all,
    comment_identity::{normalize_comment_agent_type, normalize_comment_author_type},
    comment_metadata::normalize_comment_metadata_json,
    exec, insert_event, resolve_task, resolve_task_without_active_board, with_immediate_tx,
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
    create_comment_with_options(
        path,
        task_ref,
        CreateComment {
            author: author.to_owned(),
            body: body.to_owned(),
            kind: kind.map(str::to_owned),
            author_type: None,
            agent_type: None,
            metadata_json: None,
        },
    )
}

pub fn create_comment_with_options(
    path: impl AsRef<Path>,
    task_ref: &str,
    input: CreateComment,
) -> Result<CommentRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = active_board_id_for_task(&conn, task_ref)?;
        let task = resolve_task(&conn, &board_id, task_ref)?;
        let author = input.author.trim();
        if author.is_empty() {
            return Err(KanbanError::InvalidInput(
                "comment author is required".into(),
            ));
        }
        let body = input.body.trim();
        if body.is_empty() {
            return Err(KanbanError::InvalidInput("comment body is required".into()));
        }
        let kind = input.kind.as_deref().unwrap_or("note").trim();
        if !matches!(kind, "note" | "decision" | "signal") {
            return Err(KanbanError::InvalidInput("invalid comment kind".into()));
        }
        let author_type = normalize_comment_author_type(input.author_type.as_deref(), kind)?;
        let agent_type = normalize_comment_agent_type(input.agent_type.as_deref(), author_type)?;
        let metadata_json = normalize_comment_metadata_json(kind, input.metadata_json.as_deref())?;
        let id = new_typed_id("c");
        exec(
            &conn,
            "INSERT INTO task_comments(id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                board_id,
                task.id,
                author,
                author_type,
                agent_type,
                body,
                kind,
                metadata_json,
                now
            ],
        )?;
        insert_event(
            &conn,
            &board_id,
            Some(&task.id),
            None,
            "task.comment.created",
            author,
            &json!({"comment_id": id, "kind": kind, "author_type": author_type, "agent_type": agent_type}).to_string(),
            now,
        )?;
        Ok(CommentRecord {
            id,
            board_id,
            task_id: task.id,
            author: author.to_owned(),
            author_type: author_type.to_owned(),
            agent_type: agent_type.map(str::to_owned),
            body: body.to_owned(),
            kind: kind.to_owned(),
            metadata_json,
            created_at: now,
        })
    })
}

pub fn list_comments(path: impl AsRef<Path>, task_ref: &str) -> Result<Vec<CommentRecord>> {
    let conn = connect_file(path.as_ref())?;
    let task = resolve_task_without_active_board(&conn, task_ref)?;
    let board_id = task.board_id.clone();
    all(
        &conn,
        "SELECT id, board_id, task_id, author, author_type, agent_type, body, kind, metadata_json, created_at \
         FROM task_comments WHERE board_id=?1 AND task_id=?2 ORDER BY created_at ASC, id ASC",
        params![board_id, task.id],
        comment_from_row,
    )
}

pub(crate) fn comment_from_row(row: &Row<'_>) -> rusqlite::Result<CommentRecord> {
    Ok(CommentRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        author: row.get(3)?,
        author_type: row.get(4)?,
        agent_type: row.get(5)?,
        body: row.get(6)?,
        kind: row.get(7)?,
        metadata_json: row.get(8)?,
        created_at: row.get(9)?,
    })
}
