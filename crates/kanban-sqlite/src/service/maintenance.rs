use crate::{
    connect_file, default_pragmas, maintenance_lock_blocks, maintenance_lock_path,
    runtime_lock_blocks, runtime_lock_path,
};

use super::{
    BackupResult, BlockedReasonCount, CheckpointResult, DatabaseReplaceGuard, DatabaseRuntimeGuard,
    DoctorDerivedStoreReport, DoctorReport, MaintenanceResult, QueueStats, RunLogPathStatus,
    StaleClaimRecord, StatusCount, board_id, count_dependency_cycles, derived_store_statuses_conn,
    run_log_path_status_for_db_dir, storage,
};

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::Duration,
};

use kanban_core::{Clock, KanbanError, Result, SystemClock};

use kanban_indexer::{OUTBOX_DERIVED_STORE_SEEDS, OutboxTarget, derived_store_for_name};

use rusqlite::{Connection, OptionalExtension, params};

pub fn queue_stats(path: impl AsRef<Path>, board: &str) -> Result<QueueStats> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let generated_at = SystemClock.now_ms();
    let mut status_stmt = conn
        .prepare(
            "SELECT status, COUNT(*) FROM tasks WHERE board_id=?1 GROUP BY status ORDER BY status",
        )
        .map_err(storage)?;
    let status_counts = status_stmt
        .query_map([&board_id], |row| {
            Ok(StatusCount {
                status: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;

    let mut stale_stmt = conn
        .prepare(
            "SELECT id,seq,title,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries \
             FROM tasks WHERE board_id=?1 AND status='running' AND claim_expires_at <= ?2 \
             ORDER BY claim_expires_at ASC, updated_at ASC",
        )
        .map_err(storage)?;
    let stale_claims = stale_stmt
        .query_map(params![&board_id, generated_at], |row| {
            Ok(StaleClaimRecord {
                task_id: row.get(0)?,
                seq: row.get(1)?,
                title: row.get(2)?,
                claim_owner: row.get(3)?,
                claim_expires_at: row.get(4)?,
                last_heartbeat_at: row.get(5)?,
                current_run_id: row.get(6)?,
                retry_count: row.get(7)?,
                max_retries: row.get(8)?,
            })
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;

    let mut blocked_stmt = conn
        .prepare(
            "SELECT COALESCE(NULLIF(status_reason, ''), 'unspecified') AS reason, COUNT(*) \
             FROM tasks WHERE board_id=?1 AND status='blocked' \
             GROUP BY reason ORDER BY COUNT(*) DESC, reason ASC",
        )
        .map_err(storage)?;
    let blocked_reasons = blocked_stmt
        .query_map([&board_id], |row| {
            Ok(BlockedReasonCount {
                reason: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;

    Ok(QueueStats {
        board_id,
        generated_at,
        status_counts,
        stale_claims,
        blocked_reasons,
    })
}

pub fn doctor_database(path: impl AsRef<Path>) -> Result<DoctorReport> {
    let path = path.as_ref();
    let conn = connect_existing_file(path)?;
    doctor_report_conn(&conn, path.parent())
}

pub(crate) fn doctor_report_conn(conn: &Connection, db_dir: Option<&Path>) -> Result<DoctorReport> {
    let now = SystemClock.now_ms();
    let integrity_check: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(storage)?;
    let has_migrations_table = table_exists(conn, "schema_migrations")?;
    let migration_version = if has_migrations_table {
        conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .optional()
        .map_err(storage)?
        .flatten()
    } else {
        None
    };
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(storage)?;
    if migration_version != Some(user_version)
        || !doctor_tables_present(conn, migration_version, user_version)?
    {
        return Ok(DoctorReport {
            ok: false,
            integrity_check,
            migration_version,
            user_version,
            expired_running_tasks: 0,
            running_tasks_without_active_run: 0,
            orphan_running_runs: 0,
            dependency_cycles: 0,
            archived_dependency_edges: 0,
            missing_run_logs: 0,
            suspicious_run_log_paths: 0,
            executable_dependency_violations: 0,
            executable_spec_violations: 0,
            executable_schedule_violations: 0,
            outbox_pending: 0,
            outbox_running: 0,
            outbox_failed: 0,
            derived_dirty_stores: 0,
            derived_error_stores: 0,
            derived_stores: Vec::new(),
        });
    }
    let expired_running_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE status='running' AND claim_expires_at <= ?1",
            [now],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let running_tasks_without_active_run: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks t WHERE t.status='running' AND (t.current_run_id IS NULL OR NOT EXISTS (SELECT 1 FROM task_runs r WHERE r.id=t.current_run_id AND r.task_id=t.id AND r.status='running' AND r.claim_token=t.claim_token))",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let orphan_running_runs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_runs r WHERE r.status='running' AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.id=r.task_id AND t.status='running' AND t.current_run_id=r.id AND t.claim_token=r.claim_token)",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let dependency_cycles = count_dependency_cycles(conn)?;
    let archived_dependency_edges: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM task_dependencies d \
             JOIN tasks p ON p.id=d.parent_task_id \
             JOIN tasks c ON c.id=d.child_task_id \
             WHERE c.status='archived' AND p.status!='archived'",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let (missing_run_logs, suspicious_run_log_paths) = count_run_log_path_findings(conn, db_dir)?;
    let executable_dependency_violations = count_executable_dependency_violations(conn)?;
    let executable_spec_violations = count_executable_spec_violations(conn)?;
    let executable_schedule_violations = count_executable_schedule_violations(conn, now)?;
    let derived_stores = doctor_derived_store_reports(conn)?;
    let outbox_pending = count_table_status(conn, "index_outbox", "pending")?;
    let outbox_running = count_table_status(conn, "index_outbox", "running")?;
    let outbox_failed = count_table_status(conn, "index_outbox", "failed")?;
    let derived_dirty_stores = derived_stores.iter().filter(|store| store.dirty).count() as i64;
    let derived_error_stores = derived_stores
        .iter()
        .filter(|store| store.last_error.is_some() || store.failed_outbox > 0)
        .count() as i64;
    let ok = integrity_check == "ok"
        && migration_version == Some(user_version)
        && expired_running_tasks == 0
        && running_tasks_without_active_run == 0
        && orphan_running_runs == 0
        && dependency_cycles == 0
        && archived_dependency_edges == 0
        && missing_run_logs == 0
        && suspicious_run_log_paths == 0
        && executable_dependency_violations == 0
        && executable_spec_violations == 0
        && executable_schedule_violations == 0
        && outbox_failed == 0
        && derived_error_stores == 0;
    Ok(DoctorReport {
        ok,
        integrity_check,
        migration_version,
        user_version,
        expired_running_tasks,
        running_tasks_without_active_run,
        orphan_running_runs,
        dependency_cycles,
        archived_dependency_edges,
        missing_run_logs,
        suspicious_run_log_paths,
        executable_dependency_violations,
        executable_spec_violations,
        executable_schedule_violations,
        outbox_pending,
        outbox_running,
        outbox_failed,
        derived_dirty_stores,
        derived_error_stores,
        derived_stores,
    })
}

pub(crate) fn doctor_derived_store_reports(
    conn: &Connection,
) -> Result<Vec<DoctorDerivedStoreReport>> {
    if !table_exists(conn, "derived_store_state")? {
        return Ok(Vec::new());
    }
    let stores = derived_store_statuses_conn(conn)?;
    stores
        .into_iter()
        .map(|store| {
            let seed = derived_store_for_name(&store.store_name).ok_or_else(|| {
                KanbanError::Storage(format!("unknown derived store: {}", store.store_name))
            })?;
            let outbox_backed = OUTBOX_DERIVED_STORE_SEEDS
                .iter()
                .any(|outbox_seed| outbox_seed.store_name == store.store_name);
            Ok(DoctorDerivedStoreReport {
                store_name: store.store_name,
                schema_version: store.schema_version,
                last_event_id: store.last_event_id,
                dirty: store.dirty,
                last_error: store.last_error,
                pending_outbox: if outbox_backed {
                    count_outbox_for_target(conn, seed.target, "pending")?
                } else {
                    0
                },
                running_outbox: if outbox_backed {
                    count_outbox_for_target(conn, seed.target, "running")?
                } else {
                    0
                },
                failed_outbox: if outbox_backed {
                    count_outbox_for_target(conn, seed.target, "failed")?
                } else {
                    0
                },
            })
        })
        .collect()
}

pub(crate) fn count_outbox_for_target(
    conn: &Connection,
    target: OutboxTarget,
    status: &str,
) -> Result<i64> {
    if !table_exists(conn, "index_outbox")? {
        return Ok(0);
    }
    conn.query_row(
        "SELECT COUNT(*) FROM index_outbox WHERE status=?1 AND target IN (?2, 'all')",
        params![status, target.as_str()],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub fn checkpoint_database(path: impl AsRef<Path>) -> Result<CheckpointResult> {
    let conn = connect_existing_database(path.as_ref())?;
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
        Ok(CheckpointResult {
            busy: row.get(0)?,
            log_frames: row.get(1)?,
            checkpointed_frames: row.get(2)?,
        })
    })
    .map_err(storage)
}

pub fn vacuum_database(path: impl AsRef<Path>) -> Result<MaintenanceResult> {
    let conn = connect_existing_database(path.as_ref())?;
    conn.execute_batch("VACUUM").map_err(storage)?;
    Ok(MaintenanceResult { ok: true })
}

pub fn backup_database(path: impl AsRef<Path>, out_path: impl AsRef<Path>) -> Result<BackupResult> {
    let out_path = out_path.as_ref();
    if out_path.exists() {
        return Err(KanbanError::InvalidInput(format!(
            "backup target already exists: {}",
            out_path.display()
        )));
    }
    let conn = connect_existing_database(path.as_ref())?;
    if let Some(parent) = out_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| KanbanError::Storage(error.to_string()))?;
    }
    checkpoint_database(path.as_ref())?;
    conn.execute("VACUUM main INTO ?1", [out_path.to_string_lossy().as_ref()])
        .map_err(storage)?;
    Ok(BackupResult {
        out_path: out_path.to_path_buf(),
    })
}

pub fn begin_database_replace(path: impl AsRef<Path>) -> Result<DatabaseReplaceGuard> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| KanbanError::Storage(error.to_string()))?;
    }
    let lock_path = maintenance_lock_path(path);
    if maintenance_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database is locked for maintenance: {}",
            path.display()
        )));
    }
    let runtime_lock = runtime_lock_path(path);
    if runtime_lock_blocks(&runtime_lock)? {
        return Err(KanbanError::InvalidInput(format!(
            "database has active serve/dispatch runtime; stop kanban serve/dispatch before import --replace: {}",
            path.display()
        )));
    }
    create_lock_file(&lock_path, "maintenance", path)?;
    let guard = DatabaseReplaceGuard { lock_path };
    if path.exists() && !path.is_file() {
        drop(guard);
        return Err(KanbanError::InvalidInput(format!(
            "database path is not a file: {}",
            path.display()
        )));
    }
    if path.exists()
        && path.is_file()
        && let Err(error) = assert_database_idle_for_replace(path)
    {
        drop(guard);
        return Err(error);
    }
    Ok(guard)
}

pub fn begin_database_runtime(path: impl AsRef<Path>) -> Result<DatabaseRuntimeGuard> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| KanbanError::Storage(error.to_string()))?;
    }
    let lock_path = runtime_lock_path(path);
    if runtime_lock_blocks(&lock_path)? {
        return Err(KanbanError::InvalidInput(format!(
            "database already has an active serve/dispatch runtime: {}",
            path.display()
        )));
    }
    create_lock_file(&lock_path, "runtime", path)?;
    Ok(DatabaseRuntimeGuard { lock_path })
}

pub(crate) fn create_lock_file(lock_path: &Path, kind: &str, db_path: &Path) -> Result<()> {
    let lock_result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path);
    let mut lock_file = match lock_result {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(KanbanError::InvalidInput(format!(
                "database is locked for {kind}: {}",
                db_path.display()
            )));
        }
        Err(error) => return Err(KanbanError::Storage(error.to_string())),
    };
    writeln!(lock_file, "pid={}", std::process::id())
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    writeln!(lock_file, "kind={kind}").map_err(|error| KanbanError::Storage(error.to_string()))
}

pub(crate) fn count_run_log_path_findings(
    conn: &Connection,
    db_dir: Option<&Path>,
) -> Result<(i64, i64)> {
    let mut stmt = conn
        .prepare("SELECT id, log_path FROM task_runs WHERE log_path IS NOT NULL")
        .map_err(storage)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage)?;
    let mut missing = 0;
    let mut suspicious = 0;
    for row in rows {
        let (run_id, path) = row.map_err(storage)?;
        match run_log_path_status_for_db_dir(db_dir, &run_id, &path) {
            RunLogPathStatus::Present(_) => {}
            RunLogPathStatus::Missing(_) => missing += 1,
            RunLogPathStatus::Suspicious { .. } => suspicious += 1,
        }
    }
    Ok((missing, suspicious))
}

pub(crate) fn count_executable_dependency_violations(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(DISTINCT t.id) \
         FROM tasks t \
         JOIN task_dependencies d ON d.child_task_id=t.id \
         JOIN tasks p ON p.id=d.parent_task_id \
         WHERE t.status IN ('ready', 'running') AND p.status NOT IN ('done','archived')",
        [],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub(crate) fn count_executable_spec_violations(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM tasks \
         WHERE status IN ('ready', 'running') \
           AND (description IS NULL OR length(trim(description)) = 0)",
        [],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub(crate) fn count_executable_schedule_violations(conn: &Connection, now: i64) -> Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM tasks \
         WHERE status IN ('ready', 'running') AND scheduled_at IS NOT NULL AND scheduled_at > ?1",
        [now],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub(crate) fn assert_database_idle_for_replace(path: &Path) -> Result<()> {
    let conn = match Connection::open(path) {
        Ok(conn) => conn,
        Err(_) => return Ok(()),
    };
    if default_pragmas(&conn).is_err() {
        return Ok(());
    }
    conn.busy_timeout(Duration::from_millis(0))
        .map_err(storage)?;
    if !table_exists(&conn, "schema_migrations").unwrap_or(false) {
        return Ok(());
    }
    let running_tasks = count_table_status(&conn, "tasks", "running")?;
    let running_runs = count_table_status(&conn, "task_runs", "running")?;
    if running_tasks > 0 || running_runs > 0 {
        return Err(KanbanError::InvalidInput(format!(
            "database has running work; stop kanban serve/dispatch before import --replace: {}",
            path.display()
        )));
    }
    let checkpoint = conn
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok(CheckpointResult {
                busy: row.get(0)?,
                log_frames: row.get(1)?,
                checkpointed_frames: row.get(2)?,
            })
        })
        .map_err(storage)?;
    if checkpoint.busy != 0 {
        return Err(KanbanError::InvalidInput(format!(
            "database is busy; stop kanban serve/dispatch before import --replace: {}",
            path.display()
        )));
    }
    conn.execute_batch("BEGIN IMMEDIATE; COMMIT;")
        .map_err(|error| {
            KanbanError::InvalidInput(format!(
                "database is busy; stop kanban serve/dispatch before import --replace: {} ({error})",
                path.display()
            ))
        })?;
    Ok(())
}

pub(crate) fn count_table_status(conn: &Connection, table: &str, status: &str) -> Result<i64> {
    if !table_exists(conn, table).unwrap_or(false) {
        return Ok(0);
    }
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE status=?1"),
        [status],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub(crate) fn connect_existing_file(path: &Path) -> Result<Connection> {
    if !path.exists() {
        return Err(KanbanError::InvalidInput(format!(
            "database does not exist: {}",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(KanbanError::InvalidInput(format!(
            "database path is not a file: {}",
            path.display()
        )));
    }
    connect_file(path)
}

pub(crate) fn connect_existing_database(path: &Path) -> Result<Connection> {
    let conn = connect_existing_file(path)?;
    if !table_exists(&conn, "schema_migrations")? {
        return Err(KanbanError::InvalidInput(format!(
            "database is not initialized: {}",
            path.display()
        )));
    }
    let migration_version = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .optional()
        .map_err(storage)?
        .flatten();
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(storage)?;
    if migration_version.is_none() || user_version == 0 {
        return Err(KanbanError::InvalidInput(format!(
            "database is not initialized: {}",
            path.display()
        )));
    }
    Ok(conn)
}

pub(crate) fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub(crate) fn doctor_tables_present(
    conn: &Connection,
    migration_version: Option<i64>,
    user_version: i64,
) -> Result<bool> {
    let mut required_tables = vec!["tasks", "task_dependencies", "task_runs"];
    if migration_version.unwrap_or(0) >= 2 || user_version >= 2 {
        required_tables.extend([
            "entities",
            "relation_predicates",
            "entity_relations",
            "index_outbox",
            "derived_store_state",
        ]);
    }
    if migration_version.unwrap_or(0) >= 7 || user_version >= 7 {
        required_tables.extend(["label_semantics", "label_atoms"]);
    }
    if migration_version.unwrap_or(0) >= 8 || user_version >= 8 {
        required_tables.push("label_atom_index_boards");
    }
    for table in required_tables {
        if !table_exists(conn, table)? {
            return Ok(false);
        }
    }
    Ok(true)
}
