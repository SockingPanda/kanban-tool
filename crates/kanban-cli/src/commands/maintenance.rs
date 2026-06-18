use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use kanban_sqlite::{
    backup_database, begin_database_replace, checkpoint_database, export_jsonl, import_jsonl,
    init_database, queue_stats, vacuum_database,
};

use crate::args::{BackupArgs, ExportArgs, ImportArgs};
use crate::output::print_or_json;

pub(crate) fn import_command(
    db_path: &Path,
    actor: &str,
    args: ImportArgs,
) -> Result<kanban_sqlite::ImportResult> {
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

pub(crate) fn handle_doctor(db_path: &PathBuf, json: bool) -> Result<()> {
    let report = kanban_sqlite::doctor_database(db_path)?;
    print_or_json(json, &report, || {
        format!(
            "ok={} integrity={} migration={:?} user_version={} expired_running={} running_without_run={} orphan_running_runs={} dependency_cycles={} archived_dependency_edges={} missing_run_logs={} suspicious_run_log_paths={} executable_dependency_violations={} executable_spec_violations={} executable_schedule_violations={} outbox_pending={} outbox_running={} outbox_failed={} derived_dirty_stores={} derived_error_stores={} ontology_ledger_errors={} ontology_ledger_warnings={}",
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
            report.ontology_ledger_errors,
            report.ontology_ledger_warnings
        )
    })
}

pub(crate) fn handle_stats(db_path: &PathBuf, board: &str, json: bool) -> Result<()> {
    let stats = queue_stats(db_path, board)?;
    print_or_json(json, &stats, || {
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

pub(crate) fn handle_backup(db_path: &PathBuf, args: BackupArgs, json: bool) -> Result<()> {
    let result = backup_database(db_path, args.out)?;
    print_or_json(json, &result, || {
        format!("Backup written to {}", result.out_path.display())
    })
}

pub(crate) fn handle_export(
    db_path: &PathBuf,
    board: &str,
    args: ExportArgs,
    json: bool,
) -> Result<()> {
    if args.format != "jsonl" {
        bail!("unsupported export format: {}", args.format);
    }
    let result = export_jsonl(db_path, board, args.out)?;
    print_or_json(json, &result, || {
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
        bail!("import input does not exist: {}", args.input.display());
    }
    if !args.replace {
        bail!("import requires --replace");
    }
    let result = import_command(db_path, actor, args)?;
    print_or_json(json, &result, || {
        format!(
            "Imported {} record(s) from {}",
            result.records,
            result.input_path.display()
        )
    })
}

pub(crate) fn handle_checkpoint(db_path: &PathBuf, json: bool) -> Result<()> {
    let result = checkpoint_database(db_path)?;
    print_or_json(json, &result, || {
        format!(
            "checkpoint busy={} log_frames={} checkpointed_frames={}",
            result.busy, result.log_frames, result.checkpointed_frames
        )
    })
}

pub(crate) fn handle_vacuum(db_path: &PathBuf, json: bool) -> Result<()> {
    let result = vacuum_database(db_path)?;
    print_or_json(json, &result, || "Vacuum complete".to_owned())
}
