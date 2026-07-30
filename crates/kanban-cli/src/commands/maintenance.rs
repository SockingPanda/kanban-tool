use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use fs_err as fs;
use kanban_contract::cli_operator::{
    CliExportOutput, CliExportResult, CliImportOutput, CliImportResult,
};
use kanban_core::KanbanError;
use kanban_sqlite::api::lifecycle::begin_database_replace;
use kanban_sqlite::api::{
    MaintenanceMode, MaintenanceRunOptions, MaintenanceSession, ProjectionRuntimeAvailability,
    backup_database, checkpoint_database, export_jsonl, export_jsonl_to_writer, import_jsonl,
    maintenance_rebuild_all, maintenance_rebuild_store, maintenance_run_once, maintenance_status,
    queue_stats, vacuum_database,
};
use kanban_sqlite::init::init_database;

use crate::args::{
    BackupArgs, DoctorArgs, ExportArgs, ExportFormatArg, ImportArgs, MaintenanceCommand,
};
use crate::commands::common::{invalid_input, is_stdio_path};
use crate::output::{print_contract_or_human, print_human};

pub(crate) fn import_command(
    db_path: &Path,
    actor: &str,
    args: ImportArgs,
) -> Result<kanban_sqlite::api::ImportResult> {
    let temp_path = temporary_import_db_path(db_path)?;
    let restore_path = temporary_restore_db_path(db_path)?;
    let replaced_path = temporary_replaced_db_path(db_path)?;
    let result = (|| {
        let _replace_guard = begin_database_replace(db_path)?;
        let _init = init_database(&temp_path, actor)
            .with_context(|| format!("failed to initialize/open {}", temp_path.display()))?;
        let result = import_jsonl(&temp_path, &args.input, args.replace)?;
        backup_database(&temp_path, &restore_path)?;
        replace_database_main_file(db_path, &restore_path, &replaced_path)?;
        Ok(result)
    })();
    remove_sqlite_file_family(&temp_path);
    remove_sqlite_file_family(&restore_path);
    result
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

fn temporary_sibling_db_path(db_path: &Path, label: &str) -> Result<PathBuf> {
    if let Some(parent) = db_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let file_name = db_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("kb.db");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_nanos();
    Ok(db_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            ".{file_name}.{label}.{}.{}.tmp",
            std::process::id(),
            nanos
        )))
}

fn remove_sqlite_file_family(path: &Path) {
    let _ = fs::remove_file(path);
    remove_sqlite_sidecars(path);
}

fn remove_sqlite_sidecars(path: &Path) {
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
}

fn replace_database_main_file(
    db_path: &Path,
    restore_path: &Path,
    replaced_path: &Path,
) -> Result<()> {
    remove_sqlite_sidecars(db_path);
    let had_existing = db_path.exists();
    if had_existing {
        fs::rename(db_path, replaced_path).with_context(|| {
            format!(
                "failed to move existing database {} out of the way",
                db_path.display()
            )
        })?;
    }
    if let Err(error) = fs::rename(restore_path, db_path) {
        if had_existing {
            let _ = fs::rename(replaced_path, db_path);
        }
        return Err(error).with_context(|| {
            format!(
                "failed to replace {} with restored import",
                db_path.display()
            )
        });
    }
    if had_existing {
        remove_sqlite_file_family(replaced_path);
    }
    remove_sqlite_sidecars(db_path);
    Ok(())
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
            let report = if args.all {
                maintenance_rebuild_all(db_path, actor, MaintenanceRunOptions::default())?
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
    if !args.input.is_file() {
        return Err(invalid_input(format!(
            "import input does not exist: {}",
            args.input.display()
        )));
    }
    if args.dry_run {
        let result = import_dry_run_command(db_path, actor, &args)?;
        let output = CliImportOutput::new(CliImportResult {
            input_path: result.input_path,
            records: result.records,
            dry_run: true,
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
    let result = import_command(db_path, actor, args)?;
    let output = CliImportOutput::new(CliImportResult {
        input_path: result.input_path,
        records: result.records,
        dry_run: false,
    });
    print_contract_or_human(json, &output, || {
        format!(
            "Imported {} record(s) from {}",
            output.data.records,
            output.data.input_path.display()
        )
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
