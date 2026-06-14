use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionCandidateKind {
    TaskRef,
    DependencyTaskRef,
    Board,
    Status,
    CommentKind,
}

pub fn completion_candidates(
    path: impl AsRef<Path>,
    board: &str,
    kind: CompletionCandidateKind,
    prefix: Option<&str>,
) -> Vec<String> {
    let mut candidates = match kind {
        CompletionCandidateKind::Status => status_candidates(),
        CompletionCandidateKind::CommentKind => comment_kind_candidates(),
        CompletionCandidateKind::Board => db_candidates(path.as_ref(), board_candidates),
        CompletionCandidateKind::TaskRef | CompletionCandidateKind::DependencyTaskRef => {
            db_candidates(path.as_ref(), |conn| task_ref_candidates(conn, board))
        }
    };
    if let Some(prefix) = prefix.map(str::trim).filter(|prefix| !prefix.is_empty()) {
        candidates.retain(|candidate| candidate.starts_with(prefix));
    }
    candidates
}

fn status_candidates() -> Vec<String> {
    [
        "triage",
        "todo",
        "scheduled",
        "ready",
        "running",
        "blocked",
        "review",
        "done",
        "archived",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn comment_kind_candidates() -> Vec<String> {
    ["text", "system", "worker", "decision"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn db_candidates(
    path: &Path,
    f: impl FnOnce(&Connection) -> rusqlite::Result<Vec<String>>,
) -> Vec<String> {
    if !path.exists() {
        return Vec::new();
    }
    let Ok(conn) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Vec::new();
    };
    f(&conn).unwrap_or_default()
}

fn board_candidates(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT slug FROM boards WHERE archived_at IS NULL ORDER BY created_at ASC, slug ASC",
    )?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    rows.collect()
}

fn task_ref_candidates(conn: &Connection, active_board: &str) -> rusqlite::Result<Vec<String>> {
    let active_board_slug = active_board_slug(conn, active_board)?;
    let mut stmt = conn.prepare(
        "SELECT tasks.id, boards.slug, tasks.seq \
         FROM tasks \
         JOIN boards ON boards.id=tasks.board_id \
         WHERE boards.archived_at IS NULL AND tasks.status!='archived' \
         ORDER BY boards.slug ASC, tasks.seq ASC \
         LIMIT 500",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut candidates = Vec::new();
    for row in rows {
        let (task_id, board_slug, seq) = row?;
        candidates.push(format!("{board_slug}#{seq}"));
        if active_board_slug.as_deref() == Some(board_slug.as_str()) {
            candidates.push(format!("#{seq}"));
        }
        candidates.push(task_id);
    }
    Ok(candidates)
}

fn active_board_slug(conn: &Connection, active_board: &str) -> rusqlite::Result<Option<String>> {
    if active_board.starts_with("b_") {
        conn.query_row(
            "SELECT slug FROM boards WHERE id=?1 AND archived_at IS NULL",
            params![active_board],
            |row| row.get(0),
        )
        .optional()
    } else {
        conn.query_row(
            "SELECT slug FROM boards WHERE slug=?1 AND archived_at IS NULL",
            params![active_board],
            |row| row.get(0),
        )
        .optional()
    }
}
