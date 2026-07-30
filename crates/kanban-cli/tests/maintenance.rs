mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir};
use kanban_sqlite::db::maintenance_lock_path;
use pretty_assertions::assert_eq;
#[cfg(feature = "tantivy-backend")]
use std::fs;
use std::path::Path;

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_run_reports_store_failure_as_closed_result() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_store_failure_result")?;
    kanban(&temp.path, &["init"])?.success()?;
    let store_root = temp.dir.join("index/v2/tantivy_tasks");
    fs::create_dir_all(&store_root)?;
    fs::write(store_root.join("generations"), b"not-a-directory")?;

    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "failure-fixture",
            "--json",
            "maintenance",
            "run",
            "--once",
        ],
    )?
    .success_json()?;
    let result = &output["data"]["stores"][0]["result"];
    assert_eq!(result["status"], "failed");
    assert_eq!(result["kind"], "backend");
    assert!(
        result["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
    );
    let status = kanban(&temp.path, &["--json", "maintenance", "status"])?.success_json()?;
    assert_eq!(
        status["data"]["maintenance_owner"]["owner"],
        serde_json::Value::Null
    );
    Ok(())
}

fn mark_no_plan_required(db_path: &Path, task_id: &str) -> anyhow::Result<()> {
    kanban_sqlite::api::mark_execution_plan_not_required(
        db_path,
        "default",
        "cli-maintenance-test",
        task_id,
        "maintenance fixture does not need steps",
    )?;
    Ok(())
}

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
    mark_no_plan_required(&temp.path, task_id)?;
    kanban(
        &temp.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
    )?
    .success_json()?;
    std::thread::sleep(std::time::Duration::from_millis(5));

    let doctor = kanban(&temp.path, &["--json", "doctor"])?.success_json()?;

    assert_eq!(doctor["data"]["integrity_check"], "ok");
    assert_eq!(doctor["data"]["migration_version"], 28);
    assert_eq!(doctor["data"]["user_version"], 28);
    assert_eq!(doctor["data"]["expired_running_tasks"], 1);
    assert_eq!(doctor["data"]["dependency_cycles"], 0);
    assert_eq!(doctor["data"]["archived_dependency_edges"], 0);
    assert_eq!(doctor["data"]["missing_run_logs"], 0);
    assert_eq!(doctor["data"]["executable_dependency_violations"], 0);
    assert_eq!(doctor["data"]["executable_spec_violations"], 0);
    assert_eq!(doctor["data"]["executable_schedule_violations"], 0);
    assert_eq!(doctor["data"]["unplanned_active_tasks"], 0);
    assert_eq!(
        doctor["data"]["active_parents_with_incomplete_required_steps"],
        0
    );
    assert_eq!(doctor["data"]["outbox_pending"], 12);
    assert_eq!(doctor["data"]["outbox_running"], 0);
    assert_eq!(doctor["data"]["outbox_failed"], 0);
    assert_eq!(doctor["data"]["derived_dirty_stores"], 3);
    assert_eq!(doctor["data"]["derived_error_stores"], 0);
    assert_eq!(doctor["data"]["consistency_errors"], 0);
    assert_eq!(doctor["data"]["consistency_warnings"], 0);
    assert_eq!(
        doctor["data"]["consistency_issues"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        0
    );
    assert_eq!(doctor["data"]["ontology_ledger_errors"], 0);
    assert_eq!(doctor["data"]["ontology_ledger_warnings"], 0);
    assert_eq!(
        doctor["data"]["ontology_ledger_issues"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        0
    );
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
                && store["pending_outbox"] == 4
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
        vec!["--json", "maintenance", "status"],
        vec!["--json", "maintenance", "run", "--once"],
        vec!["--json", "maintenance", "rebuild", "tantivy_tasks"],
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
        kanban(&temp.path, &args)?.json_failure_containing("database does not exist")?;
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
fn maintenance_continuous_partial_capability_refuses_without_claiming_owner() -> anyhow::Result<()>
{
    let temp = TempDb::new("maintenance_continuous_partial_capability")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &["--json", "maintenance", "run", "--poll-interval-ms", "20"],
    )?
    .json_failure_code_containing(
        2,
        "continuous maintenance requires capabilities for every projection store",
    )?;

    let owner = kanban_sqlite::db::connect_file(&temp.path)?.query_row(
        "SELECT owner,lease_token,lease_expires_at,capabilities_json,build_identity
         FROM projection_maintenance_owner WHERE singleton=1",
        [],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        },
    )?;
    assert_eq!(owner, (None, None, None, "[]".to_owned(), None));
    Ok(())
}

#[test]
fn doctor_strict_derived_fails_closed_on_bootstrap_store() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_strict_derived_bootstrap")?;
    kanban(&temp.path, &["init"])?.success()?;

    kanban(&temp.path, &["--json", "doctor", "--strict-derived"])?
        .json_failure_code_containing(8, "strict derived stores are not ready")?;
    Ok(())
}

#[cfg(feature = "oxigraph-backend")]
#[test]
fn maintenance_rebuild_targets_oxigraph_v2_store() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_rebuild_oxigraph_v2")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &["--actor", "tester", "maintenance", "run", "--once"],
    )?
    .success()?;

    let rebuilt = kanban(
        &temp.path,
        &[
            "--actor",
            "tester",
            "--json",
            "maintenance",
            "rebuild",
            "oxigraph_relations",
        ],
    )?
    .success_json()?;
    let stores = rebuilt["data"]["stores"]
        .as_array()
        .context("maintenance rebuild stores")?;
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0]["store_name"], "oxigraph_relations");
    assert_eq!(stores[0]["result"]["status"], "succeeded");
    assert_eq!(stores[0]["result"]["action"], "generation_published");
    assert_eq!(stores[0]["lifecycle_status"], "ready");
    Ok(())
}

#[test]
fn maintenance_lock_uses_canonical_path() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_lock_uses_canonical_database_path")?;
    kanban(&temp.path, &["init"])?.success()?;
    let lock_path = maintenance_lock_path(&temp.path);
    std::fs::write(&lock_path, format!("pid={}", std::process::id()))?;

    kanban_in_dir(Path::new("kb.db"), &["--json", "doctor"], &temp.dir)?
        .json_failure_containing("database is locked for maintenance")?;
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
