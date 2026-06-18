use crate::connect_file;

use super::{
    BoardColumnRecord, BoardListOptions, BoardRecord, CreateBoard, all, exec, exec_one, exists,
    insert_event, required_row, with_immediate_tx,
};

use std::path::Path;

use kanban_core::{Clock, KanbanError, Result, SystemClock, TaskStatus, new_typed_id};

use rusqlite::{Connection, Row, params};

use serde_json::json;

pub fn list_boards(path: impl AsRef<Path>, options: BoardListOptions) -> Result<Vec<BoardRecord>> {
    let conn = connect_file(path.as_ref())?;
    let archived_filter = if options.include_archived {
        ""
    } else {
        "WHERE archived_at IS NULL"
    };
    all(
        &conn,
        &format!(
            "SELECT id,slug,name,description,created_at,updated_at,archived_at \
             FROM boards {archived_filter} ORDER BY created_at ASC, slug ASC",
        ),
        [],
        board_from_row,
    )
}

pub fn create_board(
    path: impl AsRef<Path>,
    actor: &str,
    input: CreateBoard,
) -> Result<BoardRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    let slug = input.slug.trim().to_owned();
    validate_board_slug(&slug)?;
    let name = input.name.trim().to_owned();
    if name.is_empty() {
        return Err(KanbanError::InvalidInput("board name is required".into()));
    }
    let description = input
        .description
        .map(|description| description.trim().to_owned())
        .filter(|description| !description.is_empty());
    let id = new_typed_id("b");
    with_immediate_tx(&conn, || {
        let slug_exists = exists(&conn, "SELECT 1 FROM boards WHERE slug=?1", [&slug])?;
        if slug_exists {
            return Err(KanbanError::InvalidInput(format!(
                "board slug already exists: {slug}"
            )));
        }
        exec(
            &conn,
            "INSERT INTO boards(id, slug, name, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, slug, name, description, now],
        )?;
        ensure_default_columns_conn(&conn, &id, now)?;
        insert_event(
            &conn,
            &id,
            None,
            None,
            "board.created",
            actor,
            &json!({ "slug": slug }).to_string(),
            now,
        )?;
        get_board_conn_any(&conn, &id)
    })
}

pub fn archive_board(path: impl AsRef<Path>, slug_or_id: &str, actor: &str) -> Result<BoardRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board = get_board_conn(&conn, slug_or_id)?;
        let has_running_work = exists(
            &conn,
            "SELECT 1 WHERE EXISTS (SELECT 1 FROM tasks WHERE board_id=?1 AND status='running') OR EXISTS (SELECT 1 FROM task_runs WHERE board_id=?1 AND status='running')",
            [&board.id],
        )?;
        if has_running_work {
            return Err(KanbanError::InvalidTransition(
                "cannot archive board with running work".into(),
            ));
        }
        exec_one(
            &conn,
            "UPDATE boards SET archived_at=?1, updated_at=?1 WHERE id=?2 AND archived_at IS NULL",
            params![now, board.id],
            || KanbanError::InvalidTransition("cannot archive board".into()),
        )?;
        insert_event(
            &conn,
            &board.id,
            None,
            None,
            "board.archived",
            actor,
            "{}",
            now,
        )?;
        get_board_conn_any(&conn, &board.id)
    })
}

pub fn get_board(path: impl AsRef<Path>, slug_or_id: &str) -> Result<BoardRecord> {
    let conn = connect_file(path.as_ref())?;
    get_board_conn(&conn, slug_or_id)
}

pub fn get_board_including_archived(
    path: impl AsRef<Path>,
    slug_or_id: &str,
) -> Result<BoardRecord> {
    let conn = connect_file(path.as_ref())?;
    get_board_conn_any(&conn, slug_or_id)
}

pub fn list_board_columns(
    path: impl AsRef<Path>,
    board_slug_or_id: &str,
) -> Result<Vec<BoardColumnRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board_slug_or_id)?;
    all(
        &conn,
        "SELECT id,board_id,status,title,position,hidden,wip_limit,created_at,updated_at \
         FROM board_columns WHERE board_id=?1 ORDER BY position ASC",
        [board_id],
        board_column_from_row,
    )
}

pub(crate) fn get_board_conn(conn: &Connection, slug_or_id: &str) -> Result<BoardRecord> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id,slug,name,description,created_at,updated_at,archived_at FROM boards WHERE id=?1 AND archived_at IS NULL"
    } else {
        "SELECT id,slug,name,description,created_at,updated_at,archived_at FROM boards WHERE slug=?1 AND archived_at IS NULL"
    };
    required_row(conn, sql, [slug_or_id], board_from_row, || {
        KanbanError::NotFound(format!("board {slug_or_id}"))
    })
}

pub(crate) fn get_board_conn_any(conn: &Connection, slug_or_id: &str) -> Result<BoardRecord> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id,slug,name,description,created_at,updated_at,archived_at FROM boards WHERE id=?1"
    } else {
        "SELECT id,slug,name,description,created_at,updated_at,archived_at FROM boards WHERE slug=?1"
    };
    required_row(conn, sql, [slug_or_id], board_from_row, || {
        KanbanError::NotFound(format!("board {slug_or_id}"))
    })
}

pub(crate) fn board_from_row(row: &Row<'_>) -> rusqlite::Result<BoardRecord> {
    Ok(BoardRecord {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        archived_at: row.get(6)?,
    })
}

pub(crate) fn board_column_from_row(row: &Row<'_>) -> rusqlite::Result<BoardColumnRecord> {
    let status: String = row.get(2)?;
    let hidden: i64 = row.get(5)?;
    Ok(BoardColumnRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        status: TaskStatus::try_from(status.as_str())
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?,
        title: row.get(3)?,
        position: row.get(4)?,
        hidden: hidden != 0,
        wip_limit: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

pub(crate) fn board_id(conn: &Connection, slug_or_id: &str) -> Result<String> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id FROM boards WHERE id=?1 AND archived_at IS NULL"
    } else {
        "SELECT id FROM boards WHERE slug=?1 AND archived_at IS NULL"
    };
    required_row(
        conn,
        sql,
        [slug_or_id],
        |r| r.get(0),
        || KanbanError::NotFound(format!("board {slug_or_id}")),
    )
}

pub(crate) fn board_id_any(conn: &Connection, slug_or_id: &str) -> Result<String> {
    let sql = if slug_or_id.starts_with("b_") {
        "SELECT id FROM boards WHERE id=?1"
    } else {
        "SELECT id FROM boards WHERE slug=?1"
    };
    required_row(
        conn,
        sql,
        [slug_or_id],
        |r| r.get(0),
        || KanbanError::NotFound(format!("board {slug_or_id}")),
    )
}

pub(crate) fn ensure_board_active(conn: &Connection, board_id: &str) -> Result<()> {
    let active = exists(
        conn,
        "SELECT 1 FROM boards WHERE id=?1 AND archived_at IS NULL",
        [board_id],
    )?;
    if active {
        Ok(())
    } else {
        Err(KanbanError::NotFound(format!("board {board_id}")))
    }
}

pub(crate) fn ensure_default_columns_conn(
    conn: &Connection,
    board_id: &str,
    now_ms: i64,
) -> Result<()> {
    let defaults = [
        ("triage", "Triage", 10, 0),
        ("todo", "Todo", 20, 0),
        ("scheduled", "Scheduled", 30, 0),
        ("ready", "Ready", 40, 0),
        ("running", "Running", 50, 0),
        ("blocked", "Blocked", 60, 0),
        ("review", "Review", 70, 0),
        ("done", "Done", 80, 0),
        ("archived", "Archived", 90, 1),
    ];
    for (status, title, position, hidden) in defaults {
        let id = format!("col_{}_{}", board_id.trim_start_matches("b_"), status);
        exec(
            conn,
            "INSERT OR IGNORE INTO board_columns(id, board_id, status, title, position, hidden, wip_limit, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?7)",
            params![id, board_id, status, title, position, hidden, now_ms],
        )?;
    }
    Ok(())
}

pub(crate) fn validate_board_slug(slug: &str) -> Result<()> {
    if slug.is_empty() {
        return Err(KanbanError::InvalidInput("slug is required".into()));
    }
    let reserved = ["b_", "t_", "r_", "c_", "a_", "l_", "col_", "e_"];
    if slug.len() > 64
        || reserved.iter().any(|prefix| slug.starts_with(prefix))
        || !slug
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || !slug.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(KanbanError::InvalidInput(format!(
            "invalid board slug: {slug}"
        )));
    }
    Ok(())
}
