use crate::connect_file;

use super::{MAX_TASK_LIST_LIMIT, SqlFilter, all, all_values, required_row, validate_page_bounds};

use std::path::Path;

use kanban_core::{KanbanError, Result};

use rusqlite::{Connection, Row, types::Value};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityListOptions {
    pub kind: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxListOptions {
    pub status: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRecord {
    pub uri: String,
    pub kind: String,
    pub source_table: String,
    pub source_id: String,
    pub board_id: Option<String>,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexOutboxRecord {
    pub id: i64,
    pub source_event_id: Option<i64>,
    pub target: String,
    pub entity_uri: String,
    pub action: String,
    pub payload_json: String,
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedStoreStatusRecord {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_rebuild_at: Option<i64>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorDerivedStoreReport {
    pub store_name: String,
    pub schema_version: i64,
    pub last_event_id: i64,
    pub dirty: bool,
    pub last_error: Option<String>,
    pub pending_outbox: i64,
    pub running_outbox: i64,
    pub failed_outbox: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub record_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub integrity_check: String,
    pub migration_version: Option<i64>,
    pub user_version: i64,
    pub expired_running_tasks: i64,
    pub running_tasks_without_active_run: i64,
    pub orphan_running_runs: i64,
    pub dependency_cycles: i64,
    pub archived_dependency_edges: i64,
    pub missing_run_logs: i64,
    pub suspicious_run_log_paths: i64,
    pub executable_dependency_violations: i64,
    pub executable_spec_violations: i64,
    pub executable_schedule_violations: i64,
    pub outbox_pending: i64,
    pub outbox_running: i64,
    pub outbox_failed: i64,
    pub derived_dirty_stores: i64,
    pub derived_error_stores: i64,
    pub derived_stores: Vec<DoctorDerivedStoreReport>,
    pub ontology_ledger_errors: i64,
    pub ontology_ledger_warnings: i64,
    pub ontology_ledger_issues: Vec<DoctorIssue>,
}

pub fn list_entities(
    path: impl AsRef<Path>,
    options: EntityListOptions,
) -> Result<Vec<EntityRecord>> {
    validate_page_bounds(options.limit, MAX_TASK_LIST_LIMIT, 0)?;
    let conn = connect_file(path.as_ref())?;
    let mut filter = SqlFilter::new();
    if let Some(kind) = options
        .kind
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        filter.and("kind=?", kind)?;
    }
    let mut params = filter.params().to_vec();
    params.push(Value::Integer(
        options.limit.try_into().expect("validated limit"),
    ));
    let sql = format!(
        "SELECT uri,kind,source_table,source_id,board_id,task_id,title,summary,content_hash,created_at,updated_at,archived_at \
         FROM entities {} ORDER BY updated_at DESC, uri ASC LIMIT ?",
        filter.where_sql()
    );
    all_values(&conn, &sql, &params, entity_from_row)
}

pub fn get_entity(path: impl AsRef<Path>, uri: &str) -> Result<EntityRecord> {
    let conn = connect_file(path.as_ref())?;
    required_row(
        &conn,
        "SELECT uri,kind,source_table,source_id,board_id,task_id,title,summary,content_hash,created_at,updated_at,archived_at \
         FROM entities WHERE uri=?1",
        [uri],
        entity_from_row,
        || KanbanError::NotFound(format!("entity {uri}")),
    )
}

pub fn list_outbox(
    path: impl AsRef<Path>,
    options: OutboxListOptions,
) -> Result<Vec<IndexOutboxRecord>> {
    validate_page_bounds(options.limit, MAX_TASK_LIST_LIMIT, 0)?;
    let conn = connect_file(path.as_ref())?;
    let mut filter = SqlFilter::new();
    if let Some(status) = options
        .status
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        filter.and("status=?", status)?;
    }
    let mut params = filter.params().to_vec();
    params.push(Value::Integer(
        options.limit.try_into().expect("validated limit"),
    ));
    let sql = format!(
        "SELECT id,source_event_id,target,entity_uri,action,payload_json,status,attempts,last_error,created_at,updated_at \
         FROM index_outbox {} ORDER BY id ASC LIMIT ?",
        filter.where_sql()
    );
    all_values(&conn, &sql, &params, outbox_from_row)
}

pub fn derived_store_statuses(path: impl AsRef<Path>) -> Result<Vec<DerivedStoreStatusRecord>> {
    let conn = connect_file(path.as_ref())?;
    derived_store_statuses_conn(&conn)
}

pub(crate) fn derived_store_statuses_conn(
    conn: &Connection,
) -> Result<Vec<DerivedStoreStatusRecord>> {
    all(
        conn,
        "SELECT store_name,schema_version,last_event_id,dirty,last_rebuild_at,last_sync_at,last_error,updated_at \
         FROM derived_store_state ORDER BY store_name ASC",
        [],
        derived_store_status_from_row,
    )
}

fn entity_from_row(row: &Row<'_>) -> rusqlite::Result<EntityRecord> {
    Ok(EntityRecord {
        uri: row.get(0)?,
        kind: row.get(1)?,
        source_table: row.get(2)?,
        source_id: row.get(3)?,
        board_id: row.get(4)?,
        task_id: row.get(5)?,
        title: row.get(6)?,
        summary: row.get(7)?,
        content_hash: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        archived_at: row.get(11)?,
    })
}

pub(crate) fn outbox_from_row(row: &Row<'_>) -> rusqlite::Result<IndexOutboxRecord> {
    Ok(IndexOutboxRecord {
        id: row.get(0)?,
        source_event_id: row.get(1)?,
        target: row.get(2)?,
        entity_uri: row.get(3)?,
        action: row.get(4)?,
        payload_json: row.get(5)?,
        status: row.get(6)?,
        attempts: row.get(7)?,
        last_error: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

pub(crate) fn derived_store_status_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<DerivedStoreStatusRecord> {
    let dirty: i64 = row.get(3)?;
    Ok(DerivedStoreStatusRecord {
        store_name: row.get(0)?,
        schema_version: row.get(1)?,
        last_event_id: row.get(2)?,
        dirty: dirty != 0,
        last_rebuild_at: row.get(4)?,
        last_sync_at: row.get(5)?,
        last_error: row.get(6)?,
        updated_at: row.get(7)?,
    })
}
