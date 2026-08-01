use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use fs_err as fs;
use kanban_contract::cli_operator::{
    CliExportOutput, CliExportResult, CliImportOutput, CliImportResult,
};
use kanban_core::KanbanError;
use kanban_local::{
    DatabaseFileIdentity, database_file_identity, database_file_identity_from_file,
    durable_quarantine_entry, open_database_file_for_identity,
};
use kanban_sqlite::api::lifecycle::{
    begin_database_replace, publish_staged_database, resume_staged_database_replace,
};
use kanban_sqlite::api::{
    MaintenanceMode, MaintenanceRebuildIntent, MaintenanceRunOptions, MaintenanceSession,
    ProjectionRuntimeAvailability, backup_database, checkpoint_database, export_jsonl,
    export_jsonl_to_writer, import_jsonl, maintenance_apply_legacy_projection_cleanup,
    maintenance_inventory_legacy_projections, maintenance_plan_rebuild_all,
    maintenance_plan_rebuild_store, maintenance_rebuild_all, maintenance_rebuild_store,
    maintenance_restore_legacy_projection_cleanup, maintenance_resume_rebuild_store,
    maintenance_run_once, maintenance_status, maintenance_verify_legacy_projection_cleanup,
    queue_stats, vacuum_database,
};
use kanban_sqlite::init::init_database;

use crate::args::{
    BackupArgs, DoctorArgs, ExportArgs, ExportFormatArg, ImportArgs, MaintenanceCommand,
    MaintenanceLegacyCleanupCommand,
};
use crate::commands::common::{invalid_input, is_stdio_path};
use crate::output::{print_contract_or_human, print_human};

pub(crate) struct ImportCommandOutcome {
    result: kanban_sqlite::api::ImportResult,
    resumed: bool,
}

pub(crate) fn import_command(
    db_path: &Path,
    actor: &str,
    args: ImportArgs,
) -> Result<ImportCommandOutcome> {
    import_command_with_quarantine_hook(db_path, actor, args, quarantine_completed_journal)
}

fn import_command_with_quarantine_hook<Quarantine>(
    db_path: &Path,
    actor: &str,
    args: ImportArgs,
    mut quarantine: Quarantine,
) -> Result<ImportCommandOutcome>
where
    Quarantine: FnMut(&Path) -> Result<()>,
{
    let discoverable_journal_path = replacement_journal_path(db_path)?;
    let discoverable_journal_exists = match fs::symlink_metadata(&discoverable_journal_path) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect replacement journal {}",
                    discoverable_journal_path.display()
                )
            });
        }
    };
    if discoverable_journal_exists {
        let completed = replacement_journal_is_completed(&discoverable_journal_path)?;
        let mut replace_guard = begin_database_replace(db_path)?;
        if !completed {
            resume_staged_database_replace(&mut replace_guard, &discoverable_journal_path)
                .with_context(|| {
                    format!(
                        "failed to resume database replacement from {}",
                        discoverable_journal_path.display()
                    )
                })?;
            return Ok(ImportCommandOutcome {
                result: kanban_sqlite::api::ImportResult {
                    input_path: args.input,
                    records: 0,
                },
                resumed: true,
            });
        }
        // A completed phase is still untrusted until the held lifecycle
        // guard validates every retained identity. Only then may the old
        // journal be quarantined before starting a fresh replacement.
        resume_staged_database_replace(&mut replace_guard, &discoverable_journal_path)
            .with_context(|| {
                format!(
                    "completed database replacement journal failed validation: {}",
                    discoverable_journal_path.display()
                )
            })?;
        // The completed journal is trusted only after the held guard has
        // validated every physical identity. Quarantine it before checking a
        // fresh import input so an invalid input cannot leave discoverable
        // completed evidence in place.
        quarantine(&discoverable_journal_path)?;
    }
    if !args.input.is_file() {
        return Err(invalid_input(format!(
            "import input does not exist: {}",
            args.input.display()
        )));
    }
    let temp_path = temporary_import_db_path(db_path)?;
    let restore_path = temporary_restore_db_path(db_path)?;
    let replaced_path = temporary_replaced_db_path(db_path)?;
    let journal_path = if discoverable_journal_exists {
        replacement_journal_path(db_path)?
    } else {
        discoverable_journal_path
    };
    let result: Result<ImportCommandOutcome> = (|| {
        let mut replace_guard = begin_database_replace(db_path)?;
        let _init = init_database(&temp_path, actor)
            .with_context(|| format!("failed to initialize/open {}", temp_path.display()))?;
        let result = import_jsonl(&temp_path, &args.input, args.replace)?;
        backup_database(&temp_path, &restore_path)?;
        publish_staged_database(
            &mut replace_guard,
            db_path,
            &restore_path,
            &replaced_path,
            &journal_path,
        )?;
        Ok(ImportCommandOutcome {
            result,
            resumed: false,
        })
    })();
    remove_sqlite_file_family(&temp_path);
    if !journal_path.exists() {
        remove_sqlite_file_family(&restore_path);
    }
    match result {
        Err(error) if journal_path.exists() => Err(error.context(format!(
            "database replacement is incomplete; recovery evidence retained (journal={}, staged={}, previous={})",
            journal_path.display(),
            restore_path.display(),
            replaced_path.display(),
        ))),
        Ok(outcome) => Ok(outcome),
        Err(error) => Err(error),
    }
}

fn quarantine_completed_journal(path: &Path) -> Result<()> {
    durable_quarantine_entry(path).with_context(|| {
        format!(
            "failed to preserve completed replacement journal {}",
            path.display()
        )
    })?;
    Ok(())
}

pub(crate) fn import_dry_run_command(
    db_path: &Path,
    actor: &str,
    args: &ImportArgs,
) -> Result<kanban_sqlite::api::ImportResult> {
    let temp_path = temporary_import_db_path(db_path)?;
    let result = (|| {
        let _init = init_database(&temp_path, actor)
            .with_context(|| format!("failed to initialize/open {}", temp_path.display()))?;
        let result = import_jsonl(&temp_path, &args.input, true)?;
        Ok(result)
    })();
    remove_sqlite_file_family(&temp_path);
    result
}

fn temporary_import_db_path(db_path: &Path) -> Result<PathBuf> {
    temporary_sibling_db_path(db_path, "import")
}

fn temporary_restore_db_path(db_path: &Path) -> Result<PathBuf> {
    temporary_sibling_db_path(db_path, "restore")
}

fn temporary_replaced_db_path(db_path: &Path) -> Result<PathBuf> {
    temporary_sibling_db_path(db_path, "replaced")
}

fn replacement_journal_path(db_path: &Path) -> Result<PathBuf> {
    let file_name = db_path
        .file_name()
        .ok_or_else(|| invalid_input("database path has no filename for replacement journal"))?
        .to_str()
        .ok_or_else(|| {
            invalid_input(
                "database filename must be valid UTF-8 for deterministic replacement recovery",
            )
        })?;
    let parent = canonical_database_parent(db_path)?;
    Ok(parent.join(format!(".{file_name}.replace.journal")))
}

fn replacement_journal_is_completed(path: &Path) -> Result<bool> {
    replacement_journal_is_completed_with_hook(path, || Ok(()))
}

fn replacement_journal_is_completed_with_hook<Hook>(
    path: &Path,
    mut after_open: Hook,
) -> Result<bool>
where
    Hook: FnMut() -> Result<()>,
{
    // Never discover a recovery journal by following a symlink. Read from a
    // descriptor opened with the native no-follow primitive and compare its
    // identity with the path before/after reading so a concurrent entry
    // replacement fails closed instead of selecting an attacker-controlled
    // JSON file.
    let mut file =
        open_database_file_for_identity(path).map_err(|error| {
            match fs::symlink_metadata(path) {
                Ok(metadata) if !metadata.is_file() => KanbanError::InvalidInput(format!(
                    "replacement journal is not a regular file: {}",
                    path.display()
                )),
                _ => KanbanError::InvalidInput(format!(
                    "replacement journal could not be opened: {} ({error})",
                    path.display()
                )),
            }
        })?;
    let opened_identity = database_file_identity_from_file(&file).map_err(|error| {
        KanbanError::InvalidInput(format!(
            "replacement journal identity is unavailable: {} ({error})",
            path.display()
        ))
    })?;
    after_open()?;
    let before_identity = database_file_identity(path).map_err(|error| {
        KanbanError::InvalidInput(format!(
            "replacement journal could not be rechecked: {} ({error})",
            path.display()
        ))
    })?;
    if !journal_metadata_matches(&opened_identity, &before_identity) {
        return Err(KanbanError::InvalidInput(format!(
            "replacement journal path changed while being opened: {}",
            path.display()
        ))
        .into());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|error| {
        KanbanError::InvalidInput(format!(
            "replacement journal could not be read: {} ({error})",
            path.display()
        ))
    })?;
    let after_identity = database_file_identity(path).map_err(|error| {
        KanbanError::InvalidInput(format!(
            "replacement journal could not be rechecked: {} ({error})",
            path.display()
        ))
    })?;
    if !journal_metadata_matches(&opened_identity, &after_identity) {
        return Err(KanbanError::InvalidInput(format!(
            "replacement journal path changed while being read: {}",
            path.display()
        ))
        .into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        KanbanError::InvalidInput(format!(
            "invalid replacement journal JSON: {} ({error})",
            path.display()
        ))
    })?;
    Ok(value.get("phase").and_then(serde_json::Value::as_str) == Some("completed"))
}

fn journal_metadata_matches(left: &DatabaseFileIdentity, right: &DatabaseFileIdentity) -> bool {
    left == right
}

fn temporary_sibling_db_path(db_path: &Path, label: &str) -> Result<PathBuf> {
    let parent = canonical_database_parent(db_path)?;
    let file_name = db_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("kb.db");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_nanos();
    Ok(parent.join(format!(
        ".{file_name}.{label}.{}.{}.tmp",
        std::process::id(),
        nanos
    )))
}

fn canonical_database_parent(db_path: &Path) -> Result<PathBuf> {
    let parent = db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create database parent {}", parent.display()))?;
    fs::canonicalize(parent).with_context(|| {
        format!(
            "failed to canonicalize database parent {}",
            parent.display()
        )
    })
}

fn remove_sqlite_file_family(path: &Path) {
    let _ = fs::remove_file(path);
    remove_sqlite_sidecars(path);
}

fn remove_sqlite_sidecars(path: &Path) {
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
}

pub(crate) fn handle_doctor(db_path: &PathBuf, args: DoctorArgs, json: bool) -> Result<()> {
    let report = kanban_sqlite::api::doctor_database(db_path)?;
    if args.strict_derived {
        let projection = maintenance_status(db_path)?;
        let unhealthy = projection.stores.iter().filter(|store| {
            store.lifecycle_status != "ready"
                || store.runtime_availability != ProjectionRuntimeAvailability::Available
                || store.fallback_reason.is_some()
                || store.active_generation.is_none()
        });
        let unhealthy = unhealthy
            .map(|store| store.store_name.as_str())
            .collect::<Vec<_>>();
        if !unhealthy.is_empty() {
            return Err(kanban_core::KanbanError::InvalidInput(format!(
                "failed doctor checks: strict derived stores are not ready: {}",
                unhealthy.join(",")
            ))
            .into());
        }
    }
    if json {
        let output =
            kanban_contract::CliDoctorOutput::new(kanban_server::doctor_report_from_record(report));
        print_contract_or_human(true, &output, String::new)
    } else {
        print_human(|| {
            format!(
                "ok={} integrity={} migration={:?} user_version={} expired_running={} running_without_run={} orphan_running_runs={} dependency_cycles={} archived_dependency_edges={} missing_run_logs={} suspicious_run_log_paths={} executable_dependency_violations={} executable_spec_violations={} executable_schedule_violations={} outbox_pending={} outbox_running={} outbox_failed={} derived_dirty_stores={} derived_error_stores={} consistency_errors={} consistency_warnings={} ontology_ledger_errors={} ontology_ledger_warnings={}",
                report.ok,
                report.integrity_check,
                report.migration_version,
                report.user_version,
                report.expired_running_tasks,
                report.running_tasks_without_active_run,
                report.orphan_running_runs,
                report.dependency_cycles,
                report.archived_dependency_edges,
                report.missing_run_logs,
                report.suspicious_run_log_paths,
                report.executable_dependency_violations,
                report.executable_spec_violations,
                report.executable_schedule_violations,
                report.outbox_pending,
                report.outbox_running,
                report.outbox_failed,
                report.derived_dirty_stores,
                report.derived_error_stores,
                report.consistency_errors,
                report.consistency_warnings,
                report.ontology_ledger_errors,
                report.ontology_ledger_warnings
            )
        })
    }
}

pub(crate) fn handle_maintenance(
    db_path: &PathBuf,
    actor: &str,
    command: MaintenanceCommand,
    json: bool,
) -> Result<()> {
    match command {
        MaintenanceCommand::Status => {
            let status = maintenance_status(db_path)?;
            let status: kanban_contract::CliMaintenanceStatus =
                serde_json::from_value(serde_json::to_value(status)?)?;
            let output = kanban_contract::CliMaintenanceStatusOutput::new(status);
            print_contract_or_human(json, &output, || {
                let stores = output
                    .data
                    .stores
                    .iter()
                    .map(|store| {
                        format!(
                            "{}={} availability={:?} generation={:?} cursor={} pending={} fallback={:?}",
                            store.store_name,
                            store.lifecycle_status,
                            store.runtime_availability,
                            store.active_generation,
                            store.checkpoint_cursor,
                            store.pending + store.running + store.failed + store.legacy_done,
                            store.fallback_reason
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                format!(
                    "database={} protocol={} owner={:?} stores=[{}]",
                    output.data.database_instance_id,
                    output.data.protocol_version,
                    output.data.maintenance_owner.owner,
                    stores
                )
            })
        }
        MaintenanceCommand::Run(args) => {
            if !args.once && args.poll_interval_ms == 0 {
                return Err(invalid_input(
                    "maintenance run --poll-interval-ms must be greater than zero",
                ));
            }
            let report = if args.once {
                Some(maintenance_run_once(
                    db_path,
                    actor,
                    MaintenanceRunOptions::default(),
                )?)
            } else {
                let runtime =
                    tokio::runtime::Runtime::new().context("failed to start tokio runtime")?;
                runtime.block_on(run_continuous_maintenance(
                    db_path,
                    actor,
                    std::time::Duration::from_millis(args.poll_interval_ms),
                    MaintenanceRunOptions::default(),
                ))?
            };
            match report {
                Some(report) => print_maintenance_report(report, json),
                None => Ok(()),
            }
        }
        MaintenanceCommand::Rebuild(args) => {
            let intent = if args.resume {
                MaintenanceRebuildIntent::Resume
            } else {
                MaintenanceRebuildIntent::Fresh
            };
            let report = if args.dry_run {
                if args.all {
                    maintenance_plan_rebuild_all(db_path, actor)?
                } else {
                    maintenance_plan_rebuild_store(
                        db_path,
                        actor,
                        args.store.as_deref().expect("clap target group"),
                        intent,
                    )?
                }
            } else if args.all {
                maintenance_rebuild_all(db_path, actor, MaintenanceRunOptions::default())?
            } else if args.resume {
                maintenance_resume_rebuild_store(
                    db_path,
                    actor,
                    args.store.as_deref().expect("clap target group"),
                    MaintenanceRunOptions::default(),
                )?
            } else {
                maintenance_rebuild_store(
                    db_path,
                    actor,
                    args.store.as_deref().expect("clap target group"),
                    MaintenanceRunOptions::default(),
                )?
            };
            print_maintenance_report(report, json)
        }
        MaintenanceCommand::CleanupLegacy { command } => {
            let report = match command {
                MaintenanceLegacyCleanupCommand::Inventory => {
                    maintenance_inventory_legacy_projections(db_path)?
                }
                MaintenanceLegacyCleanupCommand::Apply(args) => {
                    maintenance_apply_legacy_projection_cleanup(
                        db_path,
                        actor,
                        &args.expected_inventory_digest,
                        &args.backup_dir,
                        args.resume,
                        MaintenanceRunOptions::default(),
                    )?
                }
                MaintenanceLegacyCleanupCommand::Verify(args) => {
                    maintenance_verify_legacy_projection_cleanup(
                        db_path,
                        actor,
                        &args.backup_dir,
                        MaintenanceRunOptions::default(),
                    )?
                }
                MaintenanceLegacyCleanupCommand::Restore(args) => {
                    maintenance_restore_legacy_projection_cleanup(
                        db_path,
                        actor,
                        &args.backup_dir,
                        MaintenanceRunOptions::default(),
                    )?
                }
            };
            print_legacy_cleanup_report(report, json)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaintenanceWait {
    IntervalElapsed,
    Shutdown,
    LeaseLost,
}

async fn run_continuous_maintenance(
    db_path: &Path,
    actor: &str,
    poll_interval: std::time::Duration,
    options: MaintenanceRunOptions,
) -> Result<Option<kanban_sqlite::api::MaintenanceRunReport>> {
    let retry_interval = poll_interval.min(std::time::Duration::from_secs(1));
    let mut last_report = None;
    let mut shutdown = tokio::spawn(tokio::signal::ctrl_c());
    loop {
        let mut session = loop {
            match MaintenanceSession::start(
                db_path,
                actor,
                MaintenanceMode::Continuous,
                options.clone(),
            ) {
                Ok(session) => break session,
                Err(KanbanError::Conflict(_)) => {
                    if wait_for_retry_or_shutdown(retry_interval, &mut shutdown).await? {
                        return Ok(last_report);
                    }
                }
                Err(error) => return Err(error.into()),
            }
        };

        loop {
            match session.run_once() {
                Ok(report) => last_report = Some(report),
                Err(error) if is_stale_maintenance_owner_error(&error) => {
                    drop(session);
                    break;
                }
                Err(error) => return Err(error.into()),
            }
            match wait_for_maintenance_interval(&mut session, poll_interval, &mut shutdown).await? {
                MaintenanceWait::IntervalElapsed => {}
                MaintenanceWait::LeaseLost => {
                    drop(session);
                    break;
                }
                MaintenanceWait::Shutdown => {
                    match session.finish() {
                        Ok(()) => {}
                        Err(error) if is_stale_maintenance_owner_error(&error) => {}
                        Err(error) => return Err(error.into()),
                    }
                    return Ok(last_report);
                }
            }
        }
    }
}

fn is_stale_maintenance_owner_error(error: &KanbanError) -> bool {
    matches!(
        error,
        KanbanError::Conflict(message)
            if message == "projection maintenance owner lease is stale"
                || message == "projection maintenance owner token is stale"
    )
}

async fn wait_for_retry_or_shutdown(
    interval: std::time::Duration,
    shutdown: &mut tokio::task::JoinHandle<std::io::Result<()>>,
) -> Result<bool> {
    tokio::select! {
        signal = &mut *shutdown => {
            signal.context("Ctrl-C listener task failed")?
                .context("failed to listen for Ctrl-C")?;
            Ok(true)
        }
        _ = tokio::time::sleep(interval) => Ok(false),
    }
}

async fn wait_for_maintenance_interval(
    session: &mut MaintenanceSession,
    interval: std::time::Duration,
    shutdown: &mut tokio::task::JoinHandle<std::io::Result<()>>,
) -> Result<MaintenanceWait> {
    let deadline = tokio::time::Instant::now() + interval;
    let heartbeat_interval =
        std::time::Duration::from_millis((session.lease_ttl_ms() / 3).clamp(1, 60_000) as u64);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(MaintenanceWait::IntervalElapsed);
        }
        let wait = (deadline - now).min(heartbeat_interval);
        tokio::select! {
            signal = &mut *shutdown => {
                signal.context("Ctrl-C listener task failed")?
                    .context("failed to listen for Ctrl-C")?;
                return Ok(MaintenanceWait::Shutdown);
            }
            _ = tokio::time::sleep(wait) => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(MaintenanceWait::IntervalElapsed);
        }
        match session.heartbeat() {
            Ok(()) => {}
            Err(error) if is_stale_maintenance_owner_error(&error) => {
                return Ok(MaintenanceWait::LeaseLost);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn print_maintenance_report(
    report: kanban_sqlite::api::MaintenanceRunReport,
    json: bool,
) -> Result<()> {
    let report: kanban_contract::CliMaintenanceRun =
        serde_json::from_value(serde_json::to_value(report)?)?;
    let output = kanban_contract::CliMaintenanceRunOutput::new(report);
    print_contract_or_human(json, &output, || {
        let stores = output
            .data
            .stores
            .iter()
            .map(|store| {
                let result = match &store.result {
                    kanban_contract::CliMaintenanceStoreResult::Succeeded { action, processed } => {
                        format!("{action} processed={processed}")
                    }
                    kanban_contract::CliMaintenanceStoreResult::Failed { kind, message } => {
                        format!("failed kind={kind:?} error={message}")
                    }
                };
                format!(
                    "{}:{} fallback={:?}",
                    store.store_name, result, store.fallback_reason
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "database={} protocol={} owner={} mode={:?} stores=[{}]",
            output.data.database_instance_id,
            output.data.protocol_version,
            output.data.owner,
            output.data.mode,
            stores
        )
    })
}

fn print_legacy_cleanup_report(
    report: kanban_sqlite::api::MaintenanceLegacyCleanupReport,
    json: bool,
) -> Result<()> {
    let action = report.action;
    let human = legacy_cleanup_human_report(&report);
    let report: kanban_contract::CliMaintenanceLegacyCleanup =
        serde_json::from_value(serde_json::to_value(report)?)?;
    let report = serde_json::to_value(report)?;
    match action {
        kanban_sqlite::api::MaintenanceLegacyCleanupAction::Inventory => {
            let report = serde_json::from_value(report)?;
            let output = kanban_contract::CliMaintenanceLegacyCleanupInventoryOutput::new(report);
            print_contract_or_human(json, &output, move || human)
        }
        kanban_sqlite::api::MaintenanceLegacyCleanupAction::Apply => {
            let report = serde_json::from_value(report)?;
            let output = kanban_contract::CliMaintenanceLegacyCleanupApplyOutput::new(report);
            print_contract_or_human(json, &output, move || human)
        }
        kanban_sqlite::api::MaintenanceLegacyCleanupAction::Verify => {
            let report = serde_json::from_value(report)?;
            let output = kanban_contract::CliMaintenanceLegacyCleanupVerifyOutput::new(report);
            print_contract_or_human(json, &output, move || human)
        }
        kanban_sqlite::api::MaintenanceLegacyCleanupAction::Restore => {
            let report = serde_json::from_value(report)?;
            let output = kanban_contract::CliMaintenanceLegacyCleanupRestoreOutput::new(report);
            print_contract_or_human(json, &output, move || human)
        }
    }
}

fn legacy_cleanup_human_report(
    report: &kanban_sqlite::api::MaintenanceLegacyCleanupReport,
) -> String {
    let present = report.roots.iter().filter(|root| root.present).count();
    let bytes = report.roots.iter().map(|root| root.byte_count).sum::<u64>();
    format!(
        "action={:?} dry_run={} resumed={} database={} digest={} roots_present={}/{} bytes={} backup={:?}",
        report.action,
        report.dry_run,
        report.resumed,
        report.database_instance_id,
        report.inventory_digest,
        present,
        report.roots.len(),
        bytes,
        report.backup_dir
    )
}

pub(crate) fn handle_stats(db_path: &PathBuf, board: &str, json: bool) -> Result<()> {
    let stats = queue_stats(db_path, board)?;
    if json {
        let output =
            kanban_contract::CliStatsOutput::new(kanban_server::queue_stats_from_record(stats)?);
        print_contract_or_human(true, &output, String::new)
    } else {
        print_human(|| {
            let stale = stats.stale_claims.len();
            let blocked = stats
                .blocked_reasons
                .iter()
                .map(|reason| format!("{}={}", reason.reason, reason.count))
                .collect::<Vec<_>>()
                .join(", ");
            format!("stale_claims={stale} blocked_reasons=[{blocked}]")
        })
    }
}

pub(crate) fn handle_backup(db_path: &PathBuf, args: BackupArgs, json: bool) -> Result<()> {
    if is_stdio_path(&args.out) {
        return Err(invalid_input(
            "backup --out requires a filesystem path; '-' is not supported because SQLite VACUUM INTO cannot write to stdout",
        ));
    }
    let json_out_path = if json {
        Some(
            args.out
                .to_str()
                .context("backup output path is not valid UTF-8")?
                .to_owned(),
        )
    } else {
        None
    };
    let result = backup_database(db_path, args.out)?;
    if let Some(out_path) = json_out_path {
        let output =
            kanban_contract::CliBackupOutput::new(kanban_contract::CliBackupResult { out_path });
        print_contract_or_human(true, &output, String::new)
    } else {
        print_human(|| format!("Backup written to {}", result.out_path.display()))
    }
}

pub(crate) fn handle_export(
    db_path: &PathBuf,
    board: &str,
    args: ExportArgs,
    json: bool,
) -> Result<()> {
    match args.format {
        ExportFormatArg::Jsonl => {}
    }
    if is_stdio_path(&args.out) {
        if json {
            return Err(invalid_input(
                "export --out - cannot be combined with --json because JSONL data and the JSON envelope would share stdout",
            ));
        }
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        export_jsonl_to_writer(db_path, board, &mut handle)?;
        handle.flush()?;
        return Ok(());
    }
    let result = export_jsonl(db_path, board, args.out)?;
    let output = CliExportOutput::new(CliExportResult {
        out_path: result.out_path.clone(),
        records: result.records,
    });
    print_contract_or_human(json, &output, || {
        format!(
            "Exported {} record(s) to {}",
            result.records,
            result.out_path.display()
        )
    })
}

pub(crate) fn handle_import(
    db_path: &Path,
    actor: &str,
    args: ImportArgs,
    json: bool,
) -> Result<()> {
    if args.dry_run {
        if !args.input.is_file() {
            return Err(invalid_input(format!(
                "import input does not exist: {}",
                args.input.display()
            )));
        }
        let result = import_dry_run_command(db_path, actor, &args)?;
        let output = CliImportOutput::new(CliImportResult {
            input_path: result.input_path,
            records: result.records,
            dry_run: true,
            resumed: false,
        });
        print_contract_or_human(json, &output, || {
            format!(
                "Dry-run import validated {} record(s) from {}",
                output.data.records,
                output.data.input_path.display()
            )
        })?;
        return Ok(());
    }
    if !args.replace {
        return Err(invalid_input("import requires --replace or --dry-run"));
    }
    let outcome = import_command(db_path, actor, args)?;
    let result = outcome.result;
    let output = CliImportOutput::new(CliImportResult {
        input_path: result.input_path,
        records: result.records,
        dry_run: false,
        resumed: outcome.resumed,
    });
    print_contract_or_human(json, &output, || {
        if output.data.resumed {
            "Resumed database replacement; input ignored (records=0, dry_run=false)".to_owned()
        } else {
            format!(
                "Imported {} record(s) from {}",
                output.data.records,
                output.data.input_path.display()
            )
        }
    })
}

pub(crate) fn handle_checkpoint(db_path: &PathBuf, json: bool) -> Result<()> {
    let result = checkpoint_database(db_path)?;
    if json {
        let output = kanban_contract::CliCheckpointOutput::new(kanban_contract::CheckpointReport {
            busy: result.busy,
            log_frames: result.log_frames,
            checkpointed_frames: result.checkpointed_frames,
        });
        print_contract_or_human(true, &output, String::new)
    } else {
        print_human(|| {
            format!(
                "checkpoint busy={} log_frames={} checkpointed_frames={}",
                result.busy, result.log_frames, result.checkpointed_frames
            )
        })
    }
}

pub(crate) fn handle_vacuum(db_path: &PathBuf, json: bool) -> Result<()> {
    let result = vacuum_database(db_path)?;
    if json {
        let output = kanban_contract::CliVacuumOutput::new(kanban_contract::CliVacuumResult {
            ok: result.ok,
        });
        print_contract_or_human(true, &output, String::new)
    } else {
        print_human(|| "Vacuum complete".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_sqlite::api::lifecycle::{begin_database_replace, publish_staged_database};
    use kanban_sqlite::init::init_database;
    use std::{
        process::Command,
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn completed_journal_guard_is_held_until_quarantine_finishes() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let canonical = tempdir.path().join("kb.db");
        let staged = tempdir.path().join(".kb.db.restore.fixture");
        let previous = tempdir.path().join(".kb.db.replaced.fixture");
        let journal = tempdir.path().join(".kb.db.replace.journal");
        let entered = tempdir.path().join("quarantine-entered");
        let start_probe = tempdir.path().join("start-probe");
        let probe_result = tempdir.path().join("probe-result");
        let missing_input = tempdir.path().join("missing-input.jsonl");

        init_database(&canonical, "cli-maintenance-race").expect("canonical init");
        init_database(&staged, "cli-maintenance-race").expect("staged init");
        let mut guard = begin_database_replace(&canonical).expect("replace guard");
        publish_staged_database(&mut guard, &canonical, &staged, &previous, &journal)
            .expect("publish completed journal");
        drop(guard);

        let canonical_for_thread = canonical.clone();
        let entered_for_thread = entered.clone();
        let start_probe_for_thread = start_probe.clone();
        let probe_result_for_thread = probe_result.clone();
        let import_thread = thread::spawn(move || {
            let result = import_command_with_quarantine_hook(
                &canonical_for_thread,
                "cli-maintenance-race",
                ImportArgs {
                    input: missing_input,
                    dry_run: false,
                    replace: true,
                },
                |path| {
                    fs::write(&entered_for_thread, b"entered").expect("signal quarantine entry");
                    wait_for_marker(&start_probe_for_thread);
                    wait_for_marker(&probe_result_for_thread);
                    let probe = fs::read_to_string(&probe_result_for_thread)
                        .expect("read competing guard probe");
                    assert_eq!(
                        probe, "blocked",
                        "competing process acquired guard before quarantine"
                    );
                    durable_quarantine_entry(path).expect("quarantine completed journal");
                    Ok(())
                },
            );
            assert!(
                result.is_err(),
                "missing import input must fail after quarantine"
            );
        });

        wait_for_marker(&entered);
        let mut probe = Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("commands::maintenance::tests::completed_journal_competing_guard_probe")
            .arg("--nocapture")
            .env("KANBAN_COMPLETED_JOURNAL_PROBE_DB", &canonical)
            .env("KANBAN_COMPLETED_JOURNAL_PROBE_RESULT", &probe_result)
            .spawn()
            .expect("spawn competing guard probe");
        fs::write(&start_probe, b"start").expect("start competing guard probe");

        let status = probe.wait().expect("wait competing guard probe");
        assert!(status.success(), "competing guard probe failed: {status}");
        import_thread.join().expect("import race test thread");

        assert!(!journal.exists(), "completed journal should be quarantined");
        assert!(
            fs::read_dir(tempdir.path())
                .expect("read quarantine directory")
                .filter_map(Result::ok)
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .contains(".kb.db.replace.journal.quarantine.")),
            "quarantine evidence should remain durable"
        );
    }

    #[test]
    fn completed_journal_competing_guard_probe() {
        let Some(db_path) = std::env::var_os("KANBAN_COMPLETED_JOURNAL_PROBE_DB") else {
            return;
        };
        let result_path = PathBuf::from(
            std::env::var_os("KANBAN_COMPLETED_JOURNAL_PROBE_RESULT").expect("probe result path"),
        );
        let result = begin_database_replace(Path::new(&db_path));
        let acquired = result.is_ok();
        if let Ok(guard) = result {
            drop(guard);
        }
        fs::write(result_path, if acquired { "acquired" } else { "blocked" })
            .expect("write competing guard probe result");
    }

    #[cfg(unix)]
    #[test]
    fn completed_journal_rejects_regular_entry_replaced_after_open() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let journal = tempdir.path().join("replace.journal");
        let replacement = tempdir.path().join("replacement.journal");
        fs::write(&journal, br#"{"phase":"completed"}"#).expect("write journal");
        fs::write(&replacement, br#"{"phase":"replaced"}"#).expect("write replacement journal");

        let error = replacement_journal_is_completed_with_hook(&journal, || {
            fs::remove_file(&journal).expect("remove original journal");
            fs::rename(&replacement, &journal).expect("install replacement journal");
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("path changed"));
        assert_eq!(
            fs::read(&journal).expect("read replacement journal"),
            br#"{"phase":"replaced"}"#
        );
    }

    fn wait_for_marker(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.exists() {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {}",
                path.display()
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
}
