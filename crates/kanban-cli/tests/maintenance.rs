mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir};
use kanban_sqlite::maintenance_lock_path;
use pretty_assertions::assert_eq;
use std::path::Path;
#[test]
fn doctor_reports_integrity_and_expired_runs() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_reports_integrity_migration_and_expired_running_tasks")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "doctor expired",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    kanban(
        &temp.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
    )?
    .success_json()?;
    std::thread::sleep(std::time::Duration::from_millis(5));

    let doctor = kanban(&temp.path, &["--json", "doctor"])?.success_json()?;

    assert_eq!(doctor["data"]["integrity_check"], "ok");
    assert_eq!(doctor["data"]["migration_version"], 8);
    assert_eq!(doctor["data"]["user_version"], 8);
    assert_eq!(doctor["data"]["expired_running_tasks"], 1);
    assert_eq!(doctor["data"]["dependency_cycles"], 0);
    assert_eq!(doctor["data"]["archived_dependency_edges"], 0);
    assert_eq!(doctor["data"]["missing_run_logs"], 0);
    assert_eq!(doctor["data"]["executable_dependency_violations"], 0);
    assert_eq!(doctor["data"]["executable_spec_violations"], 0);
    assert_eq!(doctor["data"]["executable_schedule_violations"], 0);
    assert_eq!(doctor["data"]["outbox_pending"], 6);
    assert_eq!(doctor["data"]["outbox_running"], 0);
    assert_eq!(doctor["data"]["outbox_failed"], 0);
    assert_eq!(doctor["data"]["derived_dirty_stores"], 3);
    assert_eq!(doctor["data"]["derived_error_stores"], 0);
    assert_eq!(
        doctor["data"]["derived_stores"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        4
    );
    assert!(
        doctor["data"]["derived_stores"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|store| store["store_name"] == "tantivy_tasks"
                && store["dirty"] == true
                && store["pending_outbox"] == 2
                && store["failed_outbox"] == 0)
    );
    assert!(
        doctor["data"]["derived_stores"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|store| store["store_name"] == "lancedb_label_atoms"
                && store["dirty"] == false
                && store["pending_outbox"] == 0
                && store["running_outbox"] == 0
                && store["failed_outbox"] == 0)
    );
    assert_eq!(doctor["data"]["ok"], false);
    Ok(())
}

#[test]
fn maintenance_rejects_missing_database() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_commands_reject_missing_database")?;
    let backup_path = temp.dir.join("backup.sqlite");
    let nested_backup_path = temp.dir.join("new/subdir/backup.sqlite");
    let export_path = temp.dir.join("board.jsonl");
    let cases = [
        vec!["--json", "doctor"],
        vec!["--json", "checkpoint"],
        vec!["--json", "vacuum"],
        vec![
            "--json",
            "backup",
            "--out",
            backup_path.to_str().context("expected UTF-8 path")?,
        ],
        vec![
            "--json",
            "backup",
            "--out",
            nested_backup_path.to_str().context("expected UTF-8 path")?,
        ],
        vec![
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    ];

    for args in cases {
        kanban(&temp.path, &args)?.failure_containing("database does not exist")?;
        assert!(
            !temp.path.exists(),
            "maintenance command should not create missing DB for args {args:?}"
        );
    }
    assert!(!backup_path.exists());
    assert!(
        !nested_backup_path
            .parent()
            .context("expected parent directory")?
            .exists()
    );
    assert!(!export_path.exists());
    Ok(())
}

#[test]
fn maintenance_lock_uses_canonical_path() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_lock_uses_canonical_database_path")?;
    kanban(&temp.path, &["init"])?.success()?;
    let lock_path = maintenance_lock_path(&temp.path);
    std::fs::write(&lock_path, format!("pid={}", std::process::id()))?;

    kanban_in_dir(Path::new("kb.db"), &["--json", "doctor"], &temp.dir)?
        .failure_containing("database is locked for maintenance")?;
    std::fs::remove_file(lock_path)?;
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn maintenance_lock_removes_dead_pid() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_lock_with_dead_pid_is_removed")?;
    kanban(&temp.path, &["init"])?.success()?;
    let lock_path = maintenance_lock_path(&temp.path);
    std::fs::write(&lock_path, "pid=999999999")?;

    kanban(&temp.path, &["--json", "doctor"])?.success_json()?;
    assert!(!lock_path.exists());
    Ok(())
}
