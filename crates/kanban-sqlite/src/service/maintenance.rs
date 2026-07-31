use crate::db::{
    DatabaseConnection, connect_existing_quiescent_read_only, connect_existing_read_only,
    connect_file, default_pragmas, maintenance_lock_blocks, maintenance_lock_path,
    open_database_with_exclusive_authority, runtime_lock_blocks, runtime_lock_path,
};

use super::{
    BackupResult, BlockedReasonCount, CheckpointResult, DatabaseReplaceGuard, DatabaseRuntimeGuard,
    DoctorDerivedStoreReport, DoctorIssue, DoctorReport, MaintenanceResult, QueueStats,
    RunLogPathStatus, StaleClaimRecord, StatusCount, board_id, count_dependency_cycles,
    derived_store_statuses_conn, run_log_path_status_for_db_dir, storage,
};

use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::Duration,
};

use kanban_core::{Clock, KanbanError, Result, SystemClock};

use kanban_indexer::{
    DERIVED_STORE_SEEDS, OUTBOX_DERIVED_STORE_SEEDS, OutboxTarget, derived_store_for_name,
};
use kanban_local::DatabaseLifecycleExclusiveGuard;

use rusqlite::{Connection, OptionalExtension, params};

const SIGNAL_LEDGER_TABLES: [&str; 2] = ["signal_observations", "signals"];

const LABEL_ONTOLOGY_LEDGER_TABLES: [&str; 5] = [
    "label_ontology_observations",
    "label_ontology_signals",
    "label_ontology_actions",
    "label_ontology_action_atom_effects",
    "label_ontology_action_signals",
];

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

    let unplanned_active_tasks = count_unplanned_active_tasks(&conn, Some(&board_id))?;
    let active_parents_with_incomplete_required_steps =
        count_active_parents_with_incomplete_required_steps(&conn, Some(&board_id))?;

    Ok(QueueStats {
        board_id,
        generated_at,
        status_counts,
        stale_claims,
        blocked_reasons,
        unplanned_active_tasks,
        active_parents_with_incomplete_required_steps,
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
    let missing_tables = doctor_missing_required_tables(conn, migration_version, user_version)?;
    let ontology_ledger_issues = doctor_missing_ontology_table_issues(&missing_tables);
    let (ontology_ledger_errors, ontology_ledger_warnings) =
        doctor_issue_counts(&ontology_ledger_issues);
    let consistency_issues = doctor_missing_signal_table_issues(&missing_tables);
    let (consistency_errors, consistency_warnings) = doctor_issue_counts(&consistency_issues);
    if migration_version != Some(user_version) || !missing_tables.is_empty() {
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
            unplanned_active_tasks: 0,
            active_parents_with_incomplete_required_steps: 0,
            outbox_pending: 0,
            outbox_running: 0,
            outbox_failed: 0,
            derived_dirty_stores: 0,
            derived_error_stores: 0,
            derived_stores: Vec::new(),
            consistency_errors,
            consistency_warnings,
            consistency_issues,
            ontology_ledger_errors,
            ontology_ledger_warnings,
            ontology_ledger_issues,
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
    let unplanned_active_tasks = count_unplanned_active_tasks(conn, None)?;
    let active_parents_with_incomplete_required_steps =
        count_active_parents_with_incomplete_required_steps(conn, None)?;
    let derived_stores = doctor_derived_store_reports(conn)?;
    let outbox_pending = count_table_status(conn, "index_outbox", "pending")?;
    let outbox_running = count_table_status(conn, "index_outbox", "running")?;
    let outbox_failed = count_table_status(conn, "index_outbox", "failed")?;
    let derived_dirty_stores = derived_stores.iter().filter(|store| store.dirty).count() as i64;
    let derived_error_stores = derived_stores
        .iter()
        .filter(|store| store.last_error.is_some() || store.failed_outbox > 0)
        .count() as i64;
    let mut consistency_issues = doctor_consistency_issues(conn)?;
    consistency_issues.extend(doctor_projection_issues(conn)?);
    consistency_issues.extend(doctor_foreign_key_issues(conn)?);
    let (consistency_errors, consistency_warnings) = doctor_issue_counts(&consistency_issues);
    let ontology_ledger_issues = doctor_ontology_ledger_issues(conn)?;
    let (ontology_ledger_errors, ontology_ledger_warnings) =
        doctor_issue_counts(&ontology_ledger_issues);
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
        && derived_error_stores == 0
        && consistency_errors == 0
        && ontology_ledger_errors == 0;
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
        unplanned_active_tasks,
        active_parents_with_incomplete_required_steps,
        outbox_pending,
        outbox_running,
        outbox_failed,
        derived_dirty_stores,
        derived_error_stores,
        derived_stores,
        consistency_errors,
        consistency_warnings,
        consistency_issues,
        ontology_ledger_errors,
        ontology_ledger_warnings,
        ontology_ledger_issues,
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
        "SELECT COUNT(*) FROM index_outbox \
         WHERE status=?1 AND target IN (?2, 'all') AND projection_store IS NULL",
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
    begin_database_replace_with_hook(path.as_ref(), |_| Ok(()))
}

fn begin_database_replace_with_hook(
    path: &Path,
    after_marker_published: impl FnOnce(&Path) -> Result<()>,
) -> Result<DatabaseReplaceGuard> {
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
    if path.exists() && !path.is_file() {
        return Err(KanbanError::InvalidInput(format!(
            "database path is not a file: {}",
            path.display()
        )));
    }

    // Acquisition order is a stable namespace fence, the current inode's
    // exclusive lifecycle authority, then legacy database-range/sentinel
    // guards. The composite authority prevents shared lifecycle re-entry.
    create_lock_file(&lock_path, "maintenance", path)?;
    let mut guard = DatabaseReplaceGuard {
        lock_path,
        staged_authority: None,
        current_authority: None,
    };
    after_marker_published(&guard.lock_path)?;
    let exclusive = DatabaseLifecycleExclusiveGuard::acquire_or_create_for_replace(path)
        .map_err(crate::db::lifecycle_storage)?;
    let authority_marker = maintenance_lock_path(exclusive.path());
    if authority_marker != guard.lock_path {
        return Err(KanbanError::Conflict(format!(
            "database namespace changed after maintenance marker publish: {}",
            path.display()
        )));
    }
    match fs::symlink_metadata(&guard.lock_path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(KanbanError::Conflict(
                "database maintenance namespace fence is missing".to_owned(),
            ));
        }
        Err(error) => return Err(KanbanError::Storage(error.to_string())),
    }
    let created_current_authority = exclusive.created_authority_file();
    let store_names = DERIVED_STORE_SEEDS
        .iter()
        .map(|seed| seed.store_name)
        .collect::<Vec<_>>();
    guard.current_authority = Some(
        exclusive
            .into_derived_store_authority(&store_names)
            .map_err(crate::db::lifecycle_storage)?,
    );
    if !created_current_authority && let Err(error) = assert_database_idle_for_replace(&mut guard) {
        drop(guard);
        return Err(error);
    }
    Ok(guard)
}

impl DatabaseReplaceGuard {
    /// Revalidates every database namespace currently bound to this guard.
    ///
    /// Before publish this checks the current database and, once fenced, the
    /// staged database. After [`Self::rebind_after_namespace_publish`] it
    /// checks the previous and newly canonical database paths instead.
    pub fn validate_database_identities(&self) -> Result<()> {
        self.current_authority
            .as_ref()
            .ok_or_else(|| {
                KanbanError::Conflict("current database lifecycle authority is not held".to_owned())
            })?
            .validate_path_identity()
            .map_err(crate::db::lifecycle_storage)?;
        if let Some(staged) = &self.staged_authority {
            staged
                .validate_path_identity()
                .map_err(crate::db::lifecycle_storage)?;
        }
        Ok(())
    }

    /// Fences a fully closed staged SQLite inode before a later atomic publish.
    ///
    /// This phase intentionally does not perform the namespace replacement.
    /// Callers must acquire this seam before renaming the staged file so both
    /// the previous and replacement inodes remain exclusive across publish.
    pub fn fence_staged_database_for_replace(&mut self, staged_path: &Path) -> Result<()> {
        if self.staged_authority.is_some() {
            return Err(KanbanError::Conflict(
                "a staged database lifecycle authority is already held".to_owned(),
            ));
        }
        if !self.lock_path.exists() {
            return Err(KanbanError::Conflict(
                "database maintenance namespace fence is missing".to_owned(),
            ));
        }
        self.current_authority
            .as_ref()
            .ok_or_else(|| {
                KanbanError::Conflict("current database lifecycle authority is not held".to_owned())
            })?
            .validate_path_identity()
            .map_err(crate::db::lifecycle_storage)?;
        let staged = DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(staged_path)
            .map_err(crate::db::lifecycle_storage)?;
        staged
            .validate_path_identity()
            .map_err(crate::db::lifecycle_storage)?;
        self.staged_authority = Some(staged);
        Ok(())
    }

    /// Rebinds both held inode authorities after a caller-controlled publish.
    ///
    /// `previous_path` must identify the pre-publish current inode and
    /// `canonical_path` must identify the pre-fenced staged inode. Both
    /// mappings are validated before either stored namespace witness changes.
    /// A failure keeps every lock and the maintenance marker held.
    pub fn rebind_after_namespace_publish(
        &mut self,
        previous_path: &Path,
        canonical_path: &Path,
    ) -> Result<()> {
        let current = self.current_authority.as_mut().ok_or_else(|| {
            KanbanError::Conflict("current database lifecycle authority is not held".to_owned())
        })?;
        let staged = self.staged_authority.as_mut().ok_or_else(|| {
            KanbanError::Conflict("staged database lifecycle authority is not held".to_owned())
        })?;

        current
            .validate_identity_at(previous_path)
            .map_err(crate::db::lifecycle_storage)?;
        staged
            .validate_identity_at(canonical_path)
            .map_err(crate::db::lifecycle_storage)?;
        current
            .rebind_after_rename(previous_path)
            .map_err(crate::db::lifecycle_storage)?;
        staged
            .rebind_after_rename(canonical_path)
            .map_err(crate::db::lifecycle_storage)?;
        current
            .validate_path_identity()
            .map_err(crate::db::lifecycle_storage)?;
        staged
            .validate_path_identity()
            .map_err(crate::db::lifecycle_storage)?;
        Ok(())
    }
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

pub(crate) fn count_unplanned_active_tasks(
    conn: &Connection,
    board_id: Option<&str>,
) -> Result<i64> {
    let board_filter = board_id.map_or("".to_owned(), |_| " AND t.board_id=?1".to_owned());
    let sql = format!(
        "SELECT COUNT(*) FROM tasks t WHERE t.status NOT IN ('done','archived') AND t.archived_at IS NULL{board_filter} AND NOT EXISTS(SELECT 1 FROM task_steps s WHERE s.board_id=t.board_id AND s.parent_task_id=t.id) AND NOT EXISTS(SELECT 1 FROM task_execution_plans ep WHERE ep.board_id=t.board_id AND ep.task_id=t.id AND ep.state='not_required')"
    );
    match board_id {
        Some(board_id) => conn
            .query_row(&sql, [board_id], |row| row.get(0))
            .map_err(storage),
        None => conn.query_row(&sql, [], |row| row.get(0)).map_err(storage),
    }
}

pub(crate) fn count_active_parents_with_incomplete_required_steps(
    conn: &Connection,
    board_id: Option<&str>,
) -> Result<i64> {
    let board_filter = board_id.map_or("".to_owned(), |_| " AND t.board_id=?1".to_owned());
    let sql = format!(
        "SELECT COUNT(*) FROM tasks t WHERE t.status NOT IN ('done','archived') AND t.archived_at IS NULL{board_filter} AND EXISTS(SELECT 1 FROM task_steps s WHERE s.board_id=t.board_id AND s.parent_task_id=t.id AND s.required=1 AND s.status NOT IN ('done','skipped'))"
    );
    match board_id {
        Some(board_id) => conn
            .query_row(&sql, [board_id], |row| row.get(0))
            .map_err(storage),
        None => conn.query_row(&sql, [], |row| row.get(0)).map_err(storage),
    }
}

pub(crate) fn assert_database_idle_for_replace(guard: &mut DatabaseReplaceGuard) -> Result<()> {
    let authority = guard.current_authority.take().ok_or_else(|| {
        KanbanError::Conflict("current database lifecycle authority is not held".to_owned())
    })?;
    let path = authority.path().to_path_buf();
    let connection = open_database_with_exclusive_authority(authority)?;
    let result = assert_database_idle_with_connection(&connection, &path);
    match connection.close() {
        Ok(authority) => {
            guard.current_authority = Some(authority);
            result
        }
        Err((connection, error)) => {
            drop(connection);
            Err(KanbanError::Storage(format!(
                "failed to close replacement inspection connection for {}: {error}",
                path.display()
            )))
        }
    }
}

fn assert_database_idle_with_connection(conn: &Connection, path: &Path) -> Result<()> {
    default_pragmas(conn)?;
    conn.busy_timeout(Duration::from_millis(0))
        .map_err(storage)?;
    if !table_exists(conn, "schema_migrations")? {
        return Err(KanbanError::InvalidInput(format!(
            "database is not initialized; refusing replacement: {}",
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
    // Acquire SQLite's reserved write lock before the final idle checks. A
    // raw writer that bypasses the lifecycle byte lock can otherwise commit
    // running work between the initial counts and BEGIN IMMEDIATE.
    conn.execute_batch("BEGIN IMMEDIATE;").map_err(|error| {
        KanbanError::InvalidInput(format!(
            "database is busy; stop kanban serve/dispatch before import --replace: {} ({error})",
            path.display()
        ))
    })?;
    let result = (|| {
        if table_exists(conn, "projection_maintenance_owner")? {
            let now = SystemClock.now_ms();
            let active_owner = conn
                .query_row(
                    "SELECT owner
                     FROM projection_maintenance_owner
                     WHERE singleton=1
                       AND owner IS NOT NULL
                       AND lease_token IS NOT NULL
                       AND lease_expires_at>?1",
                    [now],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(storage)?;
            if let Some(owner) = active_owner {
                return Err(KanbanError::InvalidInput(format!(
                    "database has active projection maintenance owner {owner}; stop maintenance before import --replace: {}",
                    path.display()
                )));
            }
        }
        let running_tasks = count_table_status(conn, "tasks", "running")?;
        let running_runs = count_table_status(conn, "task_runs", "running")?;
        if running_tasks > 0 || running_runs > 0 {
            return Err(KanbanError::InvalidInput(format!(
                "database has running work; stop kanban serve/dispatch before import --replace: {}",
                path.display()
            )));
        }
        Ok(())
    })();
    let commit = conn.execute_batch("COMMIT;").map_err(|error| {
        KanbanError::InvalidInput(format!(
            "database is busy; stop kanban serve/dispatch before import --replace: {} ({error})",
            path.display()
        ))
    });
    result.and(commit)
}

pub(crate) fn count_table_status(conn: &Connection, table: &str, status: &str) -> Result<i64> {
    if !table_exists(conn, table)? {
        return Ok(0);
    }
    conn.query_row(
        &format!("SELECT COUNT(*) FROM {table} WHERE status=?1"),
        [status],
        |row| row.get(0),
    )
    .map_err(storage)
}

pub(crate) fn connect_existing_file(path: &Path) -> Result<DatabaseConnection> {
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

pub(crate) fn connect_existing_database(path: &Path) -> Result<DatabaseConnection> {
    let conn = connect_existing_file(path)?;
    validate_initialized_database(path, conn)
}

pub(crate) fn connect_existing_database_quiescent_read_only(
    path: &Path,
) -> Result<DatabaseConnection> {
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
    let conn = connect_existing_quiescent_read_only(path)?;
    validate_initialized_database(path, conn)
}

fn validate_initialized_database(
    path: &Path,
    conn: DatabaseConnection,
) -> Result<DatabaseConnection> {
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

fn doctor_missing_required_tables(
    conn: &Connection,
    migration_version: Option<i64>,
    user_version: i64,
) -> Result<Vec<&'static str>> {
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
    if migration_version.unwrap_or(0) >= 9 || user_version >= 9 {
        required_tables.push("label_semantic_proposals");
    }
    if migration_version.unwrap_or(0) >= 12 || user_version >= 12 {
        required_tables.extend(LABEL_ONTOLOGY_LEDGER_TABLES);
    }
    if migration_version.unwrap_or(0) >= 22 || user_version >= 22 {
        required_tables.extend(["task_subtasks", "task_execution_plans"]);
    }
    if migration_version.unwrap_or(0) >= 23 || user_version >= 23 {
        required_tables.push("task_steps");
    }
    if migration_version.unwrap_or(0) >= 24 || user_version >= 24 {
        required_tables.extend(SIGNAL_LEDGER_TABLES);
    }
    if migration_version.unwrap_or(0) >= 26 || user_version >= 26 {
        required_tables.extend([
            "projection_database",
            "projection_store_state",
            "projection_deliveries",
        ]);
    }
    let mut missing = Vec::new();
    for table in required_tables {
        if !table_exists(conn, table)? {
            missing.push(table);
        }
    }
    Ok(missing)
}

fn doctor_projection_issues(conn: &Connection) -> Result<Vec<DoctorIssue>> {
    if !table_exists(conn, "projection_database")?
        || !table_exists(conn, "projection_store_state")?
        || !table_exists(conn, "projection_deliveries")?
    {
        return Ok(Vec::new());
    }
    let mut issues = Vec::new();
    let now = SystemClock.now_ms();
    let identity_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_database \
             WHERE singleton=1 AND protocol_version=2 \
               AND database_instance_id LIKE 'db_%'",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if identity_count != 1 {
        issues.push(doctor_issue(
            "error",
            "projection_database_identity_invalid",
            "projection v2 requires exactly one protocol-v2 database identity",
            Vec::new(),
        ));
    }
    let state_mismatch_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_store_state s \
             LEFT JOIN projection_database d \
               ON d.database_instance_id=s.database_instance_id \
              AND d.protocol_version=s.protocol_version \
             WHERE d.singleton IS NULL OR s.protocol_version!=2",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if state_mismatch_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_store_identity_mismatch",
            format!(
                "{state_mismatch_count} projection store state row(s) do not match the database identity"
            ),
            Vec::new(),
        ));
    }
    let missing_store_state_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM derived_store_state legacy
             LEFT JOIN projection_store_state projection
               ON projection.store_name=legacy.store_name
             WHERE projection.store_name IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if missing_store_state_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_store_state_missing",
            format!("{missing_store_state_count} derived store(s) have no projection-v2 state row"),
            Vec::new(),
        ));
    }
    let board_mismatch_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_deliveries d \
             LEFT JOIN task_events e ON e.id=d.source_event_id \
             LEFT JOIN entities entity ON entity.uri=d.entity_uri \
             WHERE (e.id IS NOT NULL AND e.board_id!=d.board_id) \
                OR (entity.board_id IS NOT NULL AND entity.board_id!=d.board_id)",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if board_mismatch_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_delivery_board_mismatch",
            format!(
                "{board_mismatch_count} projection delivery row(s) violate mandatory board scope"
            ),
            Vec::new(),
        ));
    }
    let relation_board_mismatch_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entity_relations r
             JOIN entities subject ON subject.uri=r.subject_uri
             JOIN entities object ON object.uri=r.object_uri
             WHERE subject.board_id IS NOT NULL
               AND object.board_id IS NOT NULL
               AND subject.board_id!=object.board_id",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if relation_board_mismatch_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_relation_board_mismatch",
            format!(
                "{relation_board_mismatch_count} canonical relation(s) cross mandatory board scope"
            ),
            Vec::new(),
        ));
    }
    let claim_mismatch_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_deliveries d \
             LEFT JOIN projection_store_state s ON s.store_name=d.store_name \
             WHERE d.status='running' AND (\
               s.store_name IS NULL OR s.lease_token IS NULL OR s.lease_expires_at IS NULL \
               OR d.claim_lease_token IS NOT s.lease_token \
               OR d.claim_fence_epoch IS NOT s.fence_epoch \
               OR d.claim_expires_at>s.lease_expires_at\
             )",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if claim_mismatch_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_claim_fence_mismatch",
            format!(
                "{claim_mismatch_count} running projection claim(s) exceed or mismatch the store lease"
            ),
            Vec::new(),
        ));
    }
    let failed_delivery_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_deliveries WHERE status='failed'",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if failed_delivery_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_delivery_failed",
            format!("{failed_delivery_count} projection delivery item(s) are failed"),
            Vec::new(),
        ));
    }
    let error_store_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_store_state
             WHERE lifecycle_status='error' OR last_error IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if error_store_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_store_error",
            format!("{error_store_count} projection store(s) report a maintenance error"),
            Vec::new(),
        ));
    }
    let expired_claim_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_deliveries
             WHERE status='running' AND claim_expires_at<=?1",
            [now],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if expired_claim_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_claim_expired",
            format!("{expired_claim_count} projection delivery claim(s) are expired"),
            Vec::new(),
        ));
    }
    let discontinuous_checkpoint_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_store_state s
             WHERE s.checkpoint_cursor != COALESCE((
               SELECT MAX(done.cursor) FROM projection_deliveries done
               WHERE done.store_name=s.store_name AND done.status='done'
                 AND done.cursor < COALESCE((
                   SELECT MIN(open.cursor) FROM projection_deliveries open
                   WHERE open.store_name=s.store_name AND open.status!='done'
                 ),9223372036854775807)
             ),0)",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if discontinuous_checkpoint_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_checkpoint_discontinuous",
            format!(
                "{discontinuous_checkpoint_count} projection store checkpoint(s) are not a continuous done prefix"
            ),
            Vec::new(),
        ));
    }
    let active_mismatch_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM projection_store_state \
             WHERE control_plane='v2' \
               AND (active_generation IS NULL OR active_fingerprint IS NULL \
                    OR active_fence_epoch IS NULL OR active_snapshot_cursor IS NULL \
                    OR active_provider IS NULL OR active_provider_fingerprint IS NULL \
                    OR active_canonical_count IS NULL OR active_canonical_digest IS NULL \
                    OR active_delivery_count IS NULL OR active_delivery_digest IS NULL)",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if active_mismatch_count != 0 {
        issues.push(doctor_issue(
            "error",
            "projection_v2_active_generation_missing",
            format!(
                "{active_mismatch_count} v2-controlled store(s) have no complete active generation"
            ),
            Vec::new(),
        ));
    }
    let corpus_columns_present: bool = conn
        .query_row(
            "SELECT COUNT(*)=12
             FROM pragma_table_info('projection_store_state')
             WHERE name IN (
               'active_corpus_schema','active_corpus_fingerprint',
               'active_embedding_model','active_embedding_dimensions',
               'previous_corpus_schema','previous_corpus_fingerprint',
               'previous_embedding_model','previous_embedding_dimensions',
               'building_corpus_schema','building_corpus_fingerprint',
               'building_embedding_model','building_embedding_dimensions'
             )",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if corpus_columns_present {
        let corpus_binding_invalid_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projection_store_state
                 WHERE (
                   store_name IN ('lancedb_chunks','lancedb_label_atoms')
                   AND (
                     NOT (
                       (
                         active_generation IS NULL
                         AND active_corpus_schema IS NULL
                         AND active_corpus_fingerprint IS NULL
                         AND active_embedding_model IS NULL
                         AND active_embedding_dimensions IS NULL
                       )
                       OR (
                         active_generation IS NOT NULL
                         AND active_corpus_schema IS NOT NULL
                         AND active_corpus_schema=CASE store_name
                           WHEN 'lancedb_chunks' THEN 'task-chunks-v2'
                           ELSE 'label-atoms-v2'
                         END
                         AND active_corpus_fingerprint IS NOT NULL
                         AND length(trim(active_corpus_fingerprint))>0
                         AND active_embedding_model IS NOT NULL
                         AND length(trim(active_embedding_model))>0
                         AND active_embedding_dimensions IS NOT NULL
                         AND active_embedding_dimensions>0
                       )
                     )
                     OR NOT (
                       (
                         previous_generation IS NULL
                         AND previous_corpus_schema IS NULL
                         AND previous_corpus_fingerprint IS NULL
                         AND previous_embedding_model IS NULL
                         AND previous_embedding_dimensions IS NULL
                       )
                       OR (
                         previous_generation IS NOT NULL
                         AND previous_corpus_schema IS NOT NULL
                         AND previous_corpus_schema=CASE store_name
                           WHEN 'lancedb_chunks' THEN 'task-chunks-v2'
                           ELSE 'label-atoms-v2'
                         END
                         AND previous_corpus_fingerprint IS NOT NULL
                         AND length(trim(previous_corpus_fingerprint))>0
                         AND previous_embedding_model IS NOT NULL
                         AND length(trim(previous_embedding_model))>0
                         AND previous_embedding_dimensions IS NOT NULL
                         AND previous_embedding_dimensions>0
                       )
                     )
                     OR NOT (
                       (
                         building_generation IS NULL
                         AND building_corpus_schema IS NULL
                         AND building_corpus_fingerprint IS NULL
                         AND building_embedding_model IS NULL
                         AND building_embedding_dimensions IS NULL
                       )
                       OR (
                         building_generation IS NOT NULL
                         AND building_corpus_schema IS NOT NULL
                         AND building_corpus_schema=CASE store_name
                           WHEN 'lancedb_chunks' THEN 'task-chunks-v2'
                           ELSE 'label-atoms-v2'
                         END
                         AND building_corpus_fingerprint IS NOT NULL
                         AND length(trim(building_corpus_fingerprint))>0
                         AND building_embedding_model IS NOT NULL
                         AND length(trim(building_embedding_model))>0
                         AND building_embedding_dimensions IS NOT NULL
                         AND building_embedding_dimensions>0
                       )
                     )
                   )
                 )
                 OR (
                   store_name NOT IN ('lancedb_chunks','lancedb_label_atoms')
                   AND (
                     active_corpus_schema IS NOT NULL
                     OR active_corpus_fingerprint IS NOT NULL
                     OR active_embedding_model IS NOT NULL
                     OR active_embedding_dimensions IS NOT NULL
                     OR previous_corpus_schema IS NOT NULL
                     OR previous_corpus_fingerprint IS NOT NULL
                     OR previous_embedding_model IS NOT NULL
                     OR previous_embedding_dimensions IS NOT NULL
                     OR building_corpus_schema IS NOT NULL
                     OR building_corpus_fingerprint IS NOT NULL
                     OR building_embedding_model IS NOT NULL
                     OR building_embedding_dimensions IS NOT NULL
                   )
                 )",
                [],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if corpus_binding_invalid_count != 0 {
            issues.push(doctor_issue(
                "error",
                "projection_corpus_binding_invalid",
                format!(
                    "{corpus_binding_invalid_count} projection store(s) have a missing, incomplete, or unexpected corpus/model/dimension binding"
                ),
                Vec::new(),
            ));
        }
    }
    Ok(issues)
}

fn doctor_missing_ontology_table_issues(missing_tables: &[&'static str]) -> Vec<DoctorIssue> {
    missing_tables
        .iter()
        .filter(|table| LABEL_ONTOLOGY_LEDGER_TABLES.contains(table))
        .map(|table| {
            doctor_issue(
                "error",
                "label_ontology_missing_table",
                format!("required label ontology ledger table is missing: {table}"),
                vec![(*table).to_owned()],
            )
        })
        .collect()
}

fn doctor_missing_signal_table_issues(missing_tables: &[&'static str]) -> Vec<DoctorIssue> {
    missing_tables
        .iter()
        .filter(|table| SIGNAL_LEDGER_TABLES.contains(table))
        .map(|table| {
            doctor_issue(
                "error",
                "signal_ledger_missing_table",
                format!("required signal ledger table is missing: {table}"),
                vec![(*table).to_owned()],
            )
        })
        .collect()
}

fn doctor_consistency_issues(conn: &Connection) -> Result<Vec<DoctorIssue>> {
    let mut issues = Vec::new();
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT tl.task_id, tl.label_id, tl.board_id, t.board_id \
         FROM task_labels tl \
         JOIN tasks t ON t.id=tl.task_id \
         WHERE tl.board_id<>t.board_id",
        |row| {
            let task_id: String = row.get(0)?;
            let label_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_label_task_board_mismatch",
                "task_labels",
                format!("{task_id}:{label_id}"),
                row_board,
                "tasks",
                task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT tl.task_id, tl.label_id, tl.board_id, l.board_id \
         FROM task_labels tl \
         JOIN labels l ON l.id=tl.label_id \
         WHERE tl.board_id<>l.board_id",
        |row| {
            let task_id: String = row.get(0)?;
            let label_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_label_label_board_mismatch",
                "task_labels",
                format!("{task_id}:{label_id}"),
                row_board,
                "labels",
                label_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT d.parent_task_id, d.child_task_id, d.board_id, p.board_id \
         FROM task_dependencies d \
         JOIN tasks p ON p.id=d.parent_task_id \
         WHERE d.board_id<>p.board_id",
        |row| {
            let parent_task_id: String = row.get(0)?;
            let child_task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_dependency_parent_board_mismatch",
                "task_dependencies",
                format!("{parent_task_id}->{child_task_id}"),
                row_board,
                "tasks",
                parent_task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT d.parent_task_id, d.child_task_id, d.board_id, c.board_id \
         FROM task_dependencies d \
         JOIN tasks c ON c.id=d.child_task_id \
         WHERE d.board_id<>c.board_id",
        |row| {
            let parent_task_id: String = row.get(0)?;
            let child_task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_dependency_child_board_mismatch",
                "task_dependencies",
                format!("{parent_task_id}->{child_task_id}"),
                row_board,
                "tasks",
                child_task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.parent_task_id, s.board_id, p.board_id \
         FROM task_steps s \
         JOIN tasks p ON p.id=s.parent_task_id \
         WHERE s.board_id<>p.board_id",
        |row| {
            let step_id: String = row.get(0)?;
            let parent_task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_step_parent_board_mismatch",
                "task_steps",
                step_id,
                row_board,
                "tasks",
                parent_task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.linked_task_id, s.board_id, linked.board_id \
         FROM task_steps s \
         JOIN tasks linked ON linked.id=s.linked_task_id \
         WHERE s.linked_task_id IS NOT NULL AND s.board_id<>linked.board_id",
        |row| {
            let step_id: String = row.get(0)?;
            let linked_task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_step_linked_task_board_mismatch",
                "task_steps",
                step_id,
                row_board,
                "tasks",
                linked_task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT p.task_id, p.board_id, t.board_id \
         FROM task_execution_plans p \
         JOIN tasks t ON t.id=p.task_id \
         WHERE p.board_id<>t.board_id",
        |row| {
            let task_id: String = row.get(0)?;
            let row_board: String = row.get(1)?;
            let referenced_board: String = row.get(2)?;
            Ok(relationship_board_mismatch_issue(
                "task_execution_plan_task_board_mismatch",
                "task_execution_plans",
                task_id.clone(),
                row_board,
                "tasks",
                task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT r.id, r.task_id, r.board_id, t.board_id \
         FROM task_runs r \
         JOIN tasks t ON t.id=r.task_id \
         WHERE r.board_id<>t.board_id",
        |row| {
            let run_id: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_run_task_board_mismatch",
                "task_runs",
                run_id,
                row_board,
                "tasks",
                task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT c.id, c.task_id, c.board_id, t.board_id \
         FROM task_comments c \
         JOIN tasks t ON t.id=c.task_id \
         WHERE c.board_id<>t.board_id",
        |row| {
            let comment_id: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_comment_task_board_mismatch",
                "task_comments",
                comment_id,
                row_board,
                "tasks",
                task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT o.id, o.task_id, o.board_id, t.board_id \
         FROM signal_observations o \
         JOIN tasks t ON t.id=o.task_id \
         WHERE o.task_id IS NOT NULL AND o.board_id<>t.board_id",
        |row| {
            let observation_id: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "signal_observation_task_board_mismatch",
                "signal_observations",
                observation_id,
                row_board,
                "tasks",
                task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT o.id, o.run_id, o.board_id, r.board_id \
         FROM signal_observations o \
         JOIN task_runs r ON r.id=o.run_id \
         WHERE o.run_id IS NOT NULL AND o.board_id<>r.board_id",
        |row| {
            let observation_id: String = row.get(0)?;
            let run_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "signal_observation_run_board_mismatch",
                "signal_observations",
                observation_id,
                row_board,
                "task_runs",
                run_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT o.id, o.comment_id, o.board_id, c.board_id \
         FROM signal_observations o \
         JOIN task_comments c ON c.id=o.comment_id \
         WHERE o.comment_id IS NOT NULL AND o.board_id<>c.board_id",
        |row| {
            let observation_id: String = row.get(0)?;
            let comment_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "signal_observation_comment_board_mismatch",
                "signal_observations",
                observation_id,
                row_board,
                "task_comments",
                comment_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.observation_id, s.board_id, o.board_id \
         FROM signals s \
         JOIN signal_observations o ON o.id=s.observation_id \
         WHERE s.board_id<>o.board_id",
        |row| {
            let signal_id: String = row.get(0)?;
            let observation_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "signal_observation_board_mismatch",
                "signals",
                signal_id,
                row_board,
                "signal_observations",
                observation_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.superseded_by_signal_id, s.board_id, replacement.board_id \
         FROM signals s \
         JOIN signals replacement ON replacement.id=s.superseded_by_signal_id \
         WHERE s.superseded_by_signal_id IS NOT NULL AND s.board_id<>replacement.board_id",
        |row| {
            let signal_id: String = row.get(0)?;
            let replacement_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "signal_supersede_board_mismatch",
                "signals",
                signal_id,
                row_board,
                "signals",
                replacement_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(doctor_link_cycle_issues(
        conn,
        "SELECT id, superseded_by_signal_id FROM signals \
         WHERE superseded_by_signal_id IS NOT NULL",
        "signal_supersede_cycle",
        "signal supersede cycle",
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT e.event_id, e.task_id, e.board_id, t.board_id \
         FROM task_events e \
         JOIN tasks t ON t.id=e.task_id \
         WHERE e.task_id IS NOT NULL AND e.board_id<>t.board_id",
        |row| {
            let event_id: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_event_task_board_mismatch",
                "task_events",
                event_id,
                row_board,
                "tasks",
                task_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT e.event_id, e.run_id, e.board_id, r.board_id \
         FROM task_events e \
         JOIN task_runs r ON r.id=e.run_id \
         WHERE e.run_id IS NOT NULL AND e.board_id<>r.board_id",
        |row| {
            let event_id: String = row.get(0)?;
            let run_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_event_run_board_mismatch",
                "task_events",
                event_id,
                row_board,
                "task_runs",
                run_id,
                referenced_board,
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.task_id, a.board_id, t.board_id \
         FROM task_attachments a \
         JOIN tasks t ON t.id=a.task_id \
         WHERE a.board_id<>t.board_id",
        |row| {
            let attachment_id: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            let row_board: String = row.get(2)?;
            let referenced_board: String = row.get(3)?;
            Ok(relationship_board_mismatch_issue(
                "task_attachment_task_board_mismatch",
                "task_attachments",
                attachment_id,
                row_board,
                "tasks",
                task_id,
                referenced_board,
            ))
        },
    )?);
    Ok(issues)
}

fn doctor_foreign_key_issues(conn: &Connection) -> Result<Vec<DoctorIssue>> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check").map_err(storage)?;
    let rows = stmt
        .query_map([], |row| {
            let table: String = row.get(0)?;
            let rowid: Option<i64> = row.get(1)?;
            let parent: String = row.get(2)?;
            let fk_index: i64 = row.get(3)?;
            let rowid = rowid
                .map(|value| value.to_string())
                .unwrap_or_else(|| "without-rowid".to_owned());
            Ok(doctor_issue(
                "error",
                "sqlite_foreign_key_violation",
                format!(
                    "foreign key violation: table={table} rowid={rowid} parent={parent} fk_index={fk_index}"
                ),
                vec![format!("{table}:{rowid}"), parent],
            ))
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn relationship_board_mismatch_issue(
    code: &str,
    table: &str,
    row_key: String,
    row_board: String,
    referenced_table: &str,
    referenced_id: String,
    referenced_board: String,
) -> DoctorIssue {
    doctor_issue(
        "error",
        code,
        format!(
            "cross-board relationship mismatch: table={table} row={row_key} row_board={row_board} referenced={referenced_table}:{referenced_id} referenced_board={referenced_board}"
        ),
        vec![
            format!("{table}:{row_key}"),
            row_board,
            format!("{referenced_table}:{referenced_id}"),
            referenced_board,
        ],
    )
}

fn doctor_ontology_ledger_issues(conn: &Connection) -> Result<Vec<DoctorIssue>> {
    let mut issues = Vec::new();
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT o.id, o.task_id \
         FROM label_ontology_observations o \
         LEFT JOIN tasks t ON t.id=o.task_id \
         WHERE t.id IS NULL",
        |row| {
            let observation_id: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_observation_task_missing",
                format!(
                    "label ontology observation {observation_id} references missing task {task_id}"
                ),
                vec![observation_id, task_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT o.id, o.task_id \
         FROM label_ontology_observations o \
         JOIN tasks t ON t.id=o.task_id \
         WHERE o.board_id<>t.board_id",
        |row| {
            let observation_id: String = row.get(0)?;
            let task_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_observation_task_board_mismatch",
                format!(
                    "label ontology observation {observation_id} board does not match task {task_id}"
                ),
                vec![observation_id, task_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.observation_id \
         FROM label_ontology_signals s \
         LEFT JOIN label_ontology_observations o ON o.id=s.observation_id \
         WHERE o.id IS NULL",
        |row| {
            let signal_id: String = row.get(0)?;
            let observation_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_signal_observation_missing",
                format!(
                    "label ontology signal {signal_id} references missing observation {observation_id}"
                ),
                vec![signal_id, observation_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.observation_id \
         FROM label_ontology_signals s \
         JOIN label_ontology_observations o ON o.id=s.observation_id \
         WHERE s.board_id<>o.board_id",
        |row| {
            let signal_id: String = row.get(0)?;
            let observation_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_signal_observation_board_mismatch",
                format!(
                    "label ontology signal {signal_id} board does not match observation {observation_id}"
                ),
                vec![signal_id, observation_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.target_label_id \
         FROM label_ontology_signals s \
         LEFT JOIN labels l ON l.id=s.target_label_id \
         WHERE s.target_label_id IS NOT NULL AND l.id IS NULL",
        |row| {
            let signal_id: String = row.get(0)?;
            let label_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_signal_target_label_missing",
                format!(
                    "label ontology signal {signal_id} references missing target label {label_id}"
                ),
                vec![signal_id, label_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.target_label_id \
         FROM label_ontology_signals s \
         JOIN labels l ON l.id=s.target_label_id \
         WHERE s.target_label_id IS NOT NULL AND s.board_id<>l.board_id",
        |row| {
            let signal_id: String = row.get(0)?;
            let label_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_signal_target_label_board_mismatch",
                format!(
                    "label ontology signal {signal_id} board does not match target label {label_id}"
                ),
                vec![signal_id, label_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.superseded_by_signal_id \
         FROM label_ontology_signals s \
         LEFT JOIN label_ontology_signals r ON r.id=s.superseded_by_signal_id \
         WHERE s.superseded_by_signal_id IS NOT NULL AND r.id IS NULL",
        |row| {
            let signal_id: String = row.get(0)?;
            let superseding_signal_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_signal_supersede_missing",
                format!(
                    "label ontology signal {signal_id} references missing superseding signal {superseding_signal_id}"
                ),
                vec![signal_id, superseding_signal_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT s.id, s.superseded_by_signal_id \
         FROM label_ontology_signals s \
         JOIN label_ontology_signals r ON r.id=s.superseded_by_signal_id \
         WHERE s.superseded_by_signal_id IS NOT NULL AND s.board_id<>r.board_id",
        |row| {
            let signal_id: String = row.get(0)?;
            let superseding_signal_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_signal_supersede_board_mismatch",
                format!(
                    "label ontology signal {signal_id} board does not match superseding signal {superseding_signal_id}"
                ),
                vec![signal_id, superseding_signal_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.parent_action_id \
         FROM label_ontology_actions a \
         LEFT JOIN label_ontology_actions p ON p.id=a.parent_action_id \
         WHERE a.parent_action_id IS NOT NULL AND p.id IS NULL",
        |row| {
            let action_id: String = row.get(0)?;
            let parent_action_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_parent_missing",
                format!(
                    "label ontology action {action_id} references missing parent action {parent_action_id}"
                ),
                vec![action_id, parent_action_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.parent_action_id \
         FROM label_ontology_actions a \
         JOIN label_ontology_actions p ON p.id=a.parent_action_id \
         WHERE a.parent_action_id IS NOT NULL AND a.board_id<>p.board_id",
        |row| {
            let action_id: String = row.get(0)?;
            let parent_action_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_parent_board_mismatch",
                format!(
                    "label ontology action {action_id} board does not match parent action {parent_action_id}"
                ),
                vec![action_id, parent_action_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.target_label_id \
         FROM label_ontology_actions a \
         LEFT JOIN labels l ON l.id=a.target_label_id \
         WHERE a.target_label_id IS NOT NULL AND l.id IS NULL",
        |row| {
            let action_id: String = row.get(0)?;
            let label_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_target_label_missing",
                format!(
                    "label ontology action {action_id} references missing target label {label_id}"
                ),
                vec![action_id, label_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.target_label_id \
         FROM label_ontology_actions a \
         JOIN labels l ON l.id=a.target_label_id \
         WHERE a.target_label_id IS NOT NULL AND a.board_id<>l.board_id",
        |row| {
            let action_id: String = row.get(0)?;
            let label_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_target_label_board_mismatch",
                format!(
                    "label ontology action {action_id} board does not match target label {label_id}"
                ),
                vec![action_id, label_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.result_label_id \
         FROM label_ontology_actions a \
         LEFT JOIN labels l ON l.id=a.result_label_id \
         WHERE a.result_label_id IS NOT NULL AND l.id IS NULL",
        |row| {
            let action_id: String = row.get(0)?;
            let label_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_result_label_missing",
                format!(
                    "label ontology action {action_id} references missing result label {label_id}"
                ),
                vec![action_id, label_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.result_label_id \
         FROM label_ontology_actions a \
         JOIN labels l ON l.id=a.result_label_id \
         WHERE a.result_label_id IS NOT NULL AND a.board_id<>l.board_id",
        |row| {
            let action_id: String = row.get(0)?;
            let label_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_result_label_board_mismatch",
                format!(
                    "label ontology action {action_id} board does not match result label {label_id}"
                ),
                vec![action_id, label_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.result_proposal_id \
         FROM label_ontology_actions a \
         LEFT JOIN label_semantic_proposals p ON p.id=a.result_proposal_id \
         WHERE a.result_proposal_id IS NOT NULL AND p.id IS NULL",
        |row| {
            let action_id: String = row.get(0)?;
            let proposal_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_proposal_missing",
                format!(
                    "label ontology action {action_id} references missing proposal {proposal_id}"
                ),
                vec![action_id, proposal_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.result_proposal_id \
         FROM label_ontology_actions a \
         JOIN label_semantic_proposals p ON p.id=a.result_proposal_id \
         WHERE a.result_proposal_id IS NOT NULL AND a.board_id<>p.board_id",
        |row| {
            let action_id: String = row.get(0)?;
            let proposal_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_proposal_board_mismatch",
                format!(
                    "label ontology action {action_id} board does not match proposal {proposal_id}"
                ),
                vec![action_id, proposal_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT a.id, a.result_atom_id \
         FROM label_ontology_actions a \
         LEFT JOIN label_atoms atom ON atom.id=a.result_atom_id \
         WHERE a.result_atom_id IS NOT NULL AND atom.id IS NULL",
        |row| {
            let action_id: String = row.get(0)?;
            let atom_id: String = row.get(1)?;
            Ok(doctor_issue(
                "warning",
                "label_ontology_action_result_atom_missing",
                format!(
                    "label ontology action {action_id} references missing rebuildable atom {atom_id}"
                ),
                vec![action_id, atom_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT x.action_id, x.signal_id \
         FROM label_ontology_action_signals x \
         LEFT JOIN label_ontology_actions a ON a.id=x.action_id \
         LEFT JOIN label_ontology_signals s ON s.id=x.signal_id \
         WHERE a.id IS NULL OR s.id IS NULL",
        |row| {
            let action_id: String = row.get(0)?;
            let signal_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_signal_orphan",
                format!(
                    "label ontology action-signal link references missing action {action_id} or signal {signal_id}"
                ),
                vec![action_id, signal_id],
            ))
        },
    )?);
    issues.extend(query_doctor_issue_rows(
        conn,
        "SELECT x.action_id, x.signal_id \
         FROM label_ontology_action_signals x \
         JOIN label_ontology_actions a ON a.id=x.action_id \
         JOIN label_ontology_signals s ON s.id=x.signal_id \
         WHERE x.board_id<>a.board_id OR x.board_id<>s.board_id",
        |row| {
            let action_id: String = row.get(0)?;
            let signal_id: String = row.get(1)?;
            Ok(doctor_issue(
                "error",
                "label_ontology_action_signal_board_mismatch",
                format!(
                    "label ontology action-signal link board does not match action {action_id} or signal {signal_id}"
                ),
                vec![action_id, signal_id],
            ))
        },
    )?);
    issues.extend(doctor_link_cycle_issues(
        conn,
        "SELECT id, superseded_by_signal_id FROM label_ontology_signals \
         WHERE superseded_by_signal_id IS NOT NULL",
        "label_ontology_signal_supersede_cycle",
        "label ontology signal supersede cycle",
    )?);
    issues.extend(doctor_link_cycle_issues(
        conn,
        "SELECT id, parent_action_id FROM label_ontology_actions \
         WHERE parent_action_id IS NOT NULL",
        "label_ontology_action_parent_cycle",
        "label ontology action parent cycle",
    )?);
    Ok(issues)
}

fn query_doctor_issue_rows<F>(
    conn: &Connection,
    sql: &str,
    mut mapper: F,
) -> Result<Vec<DoctorIssue>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<DoctorIssue>,
{
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let rows = stmt.query_map([], |row| mapper(row)).map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn doctor_link_cycle_issues(
    conn: &Connection,
    sql: &str,
    code: &str,
    message: &str,
) -> Result<Vec<DoctorIssue>> {
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage)?;
    let links = rows
        .collect::<std::result::Result<HashMap<_, _>, _>>()
        .map_err(storage)?;
    let mut issues = Vec::new();
    let mut emitted = HashSet::new();
    for start in links.keys() {
        let mut path = Vec::new();
        let mut path_index = HashMap::new();
        let mut current = start.as_str();
        while let Some(next) = links.get(current) {
            if let Some(cycle_start) = path_index.get(current).copied() {
                let cycle = path[cycle_start..].to_vec();
                let mut dedup_key = cycle.clone();
                dedup_key.sort();
                if emitted.insert(dedup_key.join("\0")) {
                    issues.push(doctor_issue(
                        "error",
                        code,
                        format!("{message}: {}", cycle.join(" -> ")),
                        cycle,
                    ));
                }
                break;
            }
            path_index.insert(current.to_owned(), path.len());
            path.push(current.to_owned());
            current = next;
        }
    }
    Ok(issues)
}

fn doctor_issue(
    severity: &str,
    code: &str,
    message: impl Into<String>,
    record_ids: Vec<String>,
) -> DoctorIssue {
    DoctorIssue {
        severity: severity.to_owned(),
        code: code.to_owned(),
        message: message.into(),
        record_ids,
    }
}

fn doctor_issue_counts(issues: &[DoctorIssue]) -> (i64, i64) {
    let errors = issues
        .iter()
        .filter(|issue| issue.severity == "error")
        .count() as i64;
    let warnings = issues
        .iter()
        .filter(|issue| issue.severity == "warning")
        .count() as i64;
    (errors, warnings)
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use std::io;

    unsafe extern "C" fn deny_table_probe_reads(
        _: *mut std::ffi::c_void,
        action: std::ffi::c_int,
        _: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
    ) -> std::ffi::c_int {
        if action == rusqlite::ffi::SQLITE_READ {
            rusqlite::ffi::SQLITE_DENY
        } else {
            rusqlite::ffi::SQLITE_OK
        }
    }

    unsafe extern "C" fn deny_tasks_table_probe_after_prefix(
        context: *mut std::ffi::c_void,
        action: std::ffi::c_int,
        arg1: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
    ) -> std::ffi::c_int {
        if action == rusqlite::ffi::SQLITE_READ
            && !arg1.is_null()
            && unsafe { std::ffi::CStr::from_ptr(arg1).to_bytes() } == b"sqlite_master"
        {
            let probes = unsafe { &mut *(context.cast::<usize>()) };
            *probes += 1;
            if *probes > 4 {
                return rusqlite::ffi::SQLITE_DENY;
            }
        }
        rusqlite::ffi::SQLITE_OK
    }

    unsafe extern "C" fn require_transaction_before_status_reads(
        context: *mut std::ffi::c_void,
        action: std::ffi::c_int,
        arg1: *const std::ffi::c_char,
        arg2: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
        _: *const std::ffi::c_char,
    ) -> std::ffi::c_int {
        let state = unsafe { &mut *(context.cast::<bool>()) };
        if action == rusqlite::ffi::SQLITE_TRANSACTION {
            if !arg1.is_null() && unsafe { std::ffi::CStr::from_ptr(arg1).to_bytes() } == b"BEGIN" {
                *state = true;
            }
        }
        if action == rusqlite::ffi::SQLITE_READ
            && !arg2.is_null()
            && unsafe { std::ffi::CStr::from_ptr(arg2).to_bytes() } == b"status"
            && !*state
        {
            return rusqlite::ffi::SQLITE_DENY;
        }
        rusqlite::ffi::SQLITE_OK
    }

    #[test]
    fn replace_does_not_publish_or_retain_authority_when_inspection_pragmas_fail() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("database.db");
        let setup = Connection::open(&path).unwrap();
        setup
            .execute_batch("CREATE TABLE replacement_probe(value TEXT); INSERT INTO replacement_probe VALUES ('before');")
            .unwrap();
        drop(setup);
        let bytes_before = std::fs::read(&path).unwrap();

        // This raw external connection deliberately bypasses the lifecycle
        // protocol to model a conflicting SQLite writer. It keeps the
        // inspection connection from switching the database to WAL mode.
        let blocker = Connection::open(&path).unwrap();
        blocker
            .execute_batch("PRAGMA journal_mode=DELETE; BEGIN EXCLUSIVE;")
            .unwrap();

        let error = begin_database_replace(&path).unwrap_err();

        assert!(matches!(error, KanbanError::Storage(_)), "error: {error}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes_before);
        assert!(
            !maintenance_lock_path(&path).exists(),
            "failed inspection must not publish a replacement fence"
        );
        drop(blocker);
        drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&path).unwrap());
    }

    #[test]
    fn replace_inspection_propagates_table_probe_errors() {
        let conn = Connection::open_in_memory().unwrap();
        default_pragmas(&conn).unwrap();
        conn.execute_batch("CREATE TABLE schema_migrations(version INTEGER);")
            .unwrap();
        // SAFETY: the callback has no state and stays installed only for this
        // connection, which is dropped before the test returns.
        assert_eq!(
            unsafe {
                rusqlite::ffi::sqlite3_set_authorizer(
                    conn.handle(),
                    Some(deny_table_probe_reads),
                    std::ptr::null_mut(),
                )
            },
            rusqlite::ffi::SQLITE_OK
        );

        let error =
            assert_database_idle_with_connection(&conn, Path::new("database.db")).unwrap_err();

        assert!(matches!(error, KanbanError::Storage(_)), "error: {error}");
    }

    #[test]
    fn replace_inspection_fails_closed_when_tasks_probe_errors_after_prefix() {
        let conn = Connection::open_in_memory().unwrap();
        default_pragmas(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER);
             CREATE TABLE projection_maintenance_owner(
                 singleton INTEGER PRIMARY KEY,
                 owner TEXT,
                 lease_token TEXT,
                 lease_expires_at INTEGER
             );
             CREATE TABLE tasks(status TEXT);
             CREATE TABLE task_runs(status TEXT);",
        )
        .unwrap();
        let mut probes = 0usize;
        // SAFETY: the callback and probe counter remain valid until the
        // connection is dropped, immediately after this assertion.
        assert_eq!(
            unsafe {
                rusqlite::ffi::sqlite3_set_authorizer(
                    conn.handle(),
                    Some(deny_tasks_table_probe_after_prefix),
                    (&mut probes as *mut usize).cast(),
                )
            },
            rusqlite::ffi::SQLITE_OK
        );

        let error =
            assert_database_idle_with_connection(&conn, Path::new("database.db")).unwrap_err();

        assert!(matches!(error, KanbanError::Storage(_)), "error: {error}");
        assert!(probes > 4, "expected tasks/task_runs table probe failure");
    }

    #[test]
    fn replace_rechecks_statuses_after_acquiring_sqlite_write_lock() {
        let conn = Connection::open_in_memory().unwrap();
        default_pragmas(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER);
             CREATE TABLE tasks(status TEXT);
             CREATE TABLE task_runs(status TEXT);",
        )
        .unwrap();
        let mut transaction_started = false;
        assert_eq!(
            unsafe {
                rusqlite::ffi::sqlite3_set_authorizer(
                    conn.handle(),
                    Some(require_transaction_before_status_reads),
                    (&mut transaction_started as *mut bool).cast(),
                )
            },
            rusqlite::ffi::SQLITE_OK
        );

        assert_database_idle_with_connection(&conn, Path::new("database.db")).unwrap();
        assert!(transaction_started);
    }

    #[cfg(unix)]
    #[test]
    fn replace_rejects_database_alias_retargeted_after_marker_publish() {
        use std::os::unix::fs::symlink;

        let tempdir = tempfile::tempdir().unwrap();
        let first = tempdir.path().join("first.db");
        let second = tempdir.path().join("second.db");
        let alias = tempdir.path().join("database.db");
        std::fs::write(&first, b"").unwrap();
        std::fs::write(&second, b"").unwrap();
        symlink(&first, &alias).unwrap();
        let first_marker = maintenance_lock_path(&first);
        let second_marker = maintenance_lock_path(&second);

        let error = begin_database_replace_with_hook(&alias, |_| {
            std::fs::remove_file(&alias)
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
            symlink(&second, &alias).map_err(|error| KanbanError::Storage(error.to_string()))
        })
        .unwrap_err();

        assert!(error.to_string().contains("namespace changed"));
        assert!(!first_marker.exists());
        assert!(!second_marker.exists());
        drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&first).unwrap());
        drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&second).unwrap());
    }
}
