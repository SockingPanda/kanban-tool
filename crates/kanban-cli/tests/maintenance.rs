mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir};
use kanban_sqlite::db::maintenance_lock_path;
use pretty_assertions::assert_eq;
#[cfg(feature = "tantivy-backend")]
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_run_reports_store_failure_as_closed_result() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_store_failure_result")?;
    kanban(&temp.path, &["init"])?.success()?;
    let database_instance_id = kanban_sqlite::db::connect_file(&temp.path)?.query_row(
        "SELECT database_instance_id FROM projection_database WHERE singleton=1",
        [],
        |row| row.get::<_, String>(0),
    )?;
    let store_root = kanban_local::projection_store_root_path(
        &temp.path,
        &database_instance_id,
        "tantivy_tasks",
    )?;
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
        vec![
            "--json",
            "maintenance",
            "rebuild",
            "tantivy_tasks",
            "--dry-run",
        ],
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

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_rebuild_dry_run_is_read_only_and_reports_the_exact_intent() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_rebuild_dry_run")?;
    kanban(&temp.path, &["init"])?.success()?;
    let before = kanban_sqlite::api::projection_status(&temp.path)?;
    let before_outbox = kanban_sqlite::api::list_outbox(
        &temp.path,
        kanban_sqlite::api::OutboxListOptions {
            status: None,
            limit: 1_000,
        },
    )?;
    checkpoint_sqlite_for_read_only_snapshot(&temp.path)?;
    let before_tree = exact_tree_snapshot(&temp.dir)?;

    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "dry-run-owner",
            "--json",
            "maintenance",
            "rebuild",
            "tantivy_tasks",
            "--dry-run",
        ],
    )?
    .success_json()?;

    assert_eq!(output["data"]["owner"], "dry-run-owner");
    assert_eq!(output["data"]["stores"].as_array().map(Vec::len), Some(1));
    assert_eq!(output["data"]["stores"][0]["store_name"], "tantivy_tasks");
    assert_eq!(
        output["data"]["stores"][0]["result"],
        serde_json::json!({
            "status": "succeeded",
            "action": "dry_run_rebuild",
            "processed": 0
        })
    );
    let all = kanban(
        &temp.path,
        &[
            "--actor",
            "dry-run-owner",
            "--json",
            "maintenance",
            "rebuild",
            "--all",
            "--dry-run",
        ],
    )?
    .success_json()?;
    assert!(
        all["data"]["stores"]
            .as_array()
            .is_some_and(|stores| !stores.is_empty())
    );
    assert_eq!(
        exact_tree_snapshot(&temp.dir)?,
        before_tree,
        "fresh and all-store dry-runs must not change the database, WAL/SHM sidecars, lifecycle files, or any derived root"
    );
    assert_eq!(kanban_sqlite::api::projection_status(&temp.path)?, before);
    assert_eq!(
        kanban_sqlite::api::list_outbox(
            &temp.path,
            kanban_sqlite::api::OutboxListOptions {
                status: None,
                limit: 1_000,
            },
        )?,
        before_outbox
    );
    let owner = kanban_sqlite::db::connect_file(&temp.path)?.query_row(
        "SELECT owner,lease_token,lease_expires_at
         FROM projection_maintenance_owner WHERE singleton=1",
        [],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        },
    )?;
    assert_eq!(owner, (None, None, None));
    assert!(
        !temp.dir.join("index/v2/databases").exists(),
        "dry-run must not create the Projection v2 physical namespace"
    );
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_rebuild_requires_explicit_resume_for_unfinished_generation() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_rebuild_explicit_resume")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban_sqlite::db::connect_file(&temp.path)?.execute(
        "UPDATE projection_store_state
         SET building_generation='gen_resume_fixture',
             building_fingerprint='fnv64:resume-fixture',
             building_fence_epoch=1,
             building_provider='tantivy',
             building_provider_fingerprint='tantivy-tasks-v2',
             building_canonical_count=0,
             building_canonical_digest='fnv64:resume-canonical',
             building_delivery_count=0,
             building_delivery_digest='fnv64:resume-delivery',
             building_phase='snapshotting',
             lifecycle_status='rebuilding',
             fence_epoch=1
         WHERE store_name='tantivy_tasks'",
        [],
    )?;
    checkpoint_sqlite_for_read_only_snapshot(&temp.path)?;
    let before_tree = exact_tree_snapshot(&temp.dir)?;

    kanban(
        &temp.path,
        &[
            "--actor",
            "resume-owner",
            "--json",
            "maintenance",
            "rebuild",
            "tantivy_tasks",
            "--dry-run",
        ],
    )?
    .json_failure_containing("use --resume")?;

    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "resume-owner",
            "--json",
            "maintenance",
            "rebuild",
            "tantivy_tasks",
            "--dry-run",
            "--resume",
        ],
    )?
    .success_json()?;
    assert_eq!(
        output["data"]["stores"][0]["result"]["action"],
        "dry_run_resume"
    );
    assert_eq!(
        exact_tree_snapshot(&temp.dir)?,
        before_tree,
        "fresh rejection and explicit resume dry-run must preserve the exact database and physical tree"
    );
    assert_eq!(
        kanban_sqlite::api::projection_status(&temp.path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .and_then(|store| store.building_generation),
        Some("gen_resume_fixture".to_owned())
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn maintenance_cleanup_legacy_is_digest_bound_resumable_and_restorable() -> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_cleanup_legacy")?;
    kanban(&temp.path, &["init"])?.success()?;
    let legacy_file = temp.dir.join("index/v1/tasks/segment/doc");
    let database_scoped_v2 = temp
        .dir
        .join("index/v2/databases/db_keep/tantivy_tasks/generations/gen_keep");
    std::fs::create_dir_all(
        legacy_file
            .parent()
            .context("legacy file must have a parent")?,
    )?;
    std::fs::write(&legacy_file, b"legacy-task-index")?;
    std::fs::create_dir_all(&database_scoped_v2)?;
    std::fs::write(database_scoped_v2.join("keep"), b"v2")?;
    let backup_parent = tempfile::Builder::new()
        .prefix("kb-cli-maintenance-cleanup-backup-")
        .tempdir()
        .context("create independent backup parent")?;
    let backup_dir = backup_parent.path().join("projection-v1-backup");
    checkpoint_sqlite_for_read_only_snapshot(&temp.path)?;
    let before_inventory_tree = exact_tree_snapshot(&temp.dir)?;

    let inventory = kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "inventory",
        ],
    )?
    .success_json()?;
    assert_eq!(
        exact_tree_snapshot(&temp.dir)?,
        before_inventory_tree,
        "cleanup inventory must be strictly read-only across SQLite sidecars and every derived root"
    );
    assert_eq!(inventory["data"]["action"], "inventory");
    assert_eq!(inventory["data"]["dry_run"], true);
    assert_eq!(inventory["data"]["resumed"], false);
    assert_eq!(inventory["data"]["backup_dir"], serde_json::Value::Null);
    let digest = inventory["data"]["inventory_digest"]
        .as_str()
        .context("inventory digest")?;
    assert!(digest.starts_with("sha256:"));
    assert_eq!(inventory["data"]["roots"].as_array().map(Vec::len), Some(5));
    assert!(legacy_file.is_file());
    assert!(!backup_dir.exists());

    kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
            "--expected-inventory-digest",
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        ],
    )?
    .json_failure_containing("inventory digest mismatch")?;
    assert!(legacy_file.is_file());
    assert!(!backup_dir.exists());

    let applied = kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
            "--expected-inventory-digest",
            digest,
        ],
    )?
    .success_json()?;
    assert_eq!(applied["data"]["action"], "apply");
    assert_eq!(applied["data"]["dry_run"], false);
    assert_eq!(applied["data"]["resumed"], false);
    assert!(!legacy_file.exists());
    assert!(backup_dir.join("roots/tantivy_v1/segment/doc").is_file());
    assert!(
        database_scoped_v2.join("keep").is_file(),
        "cleanup must never walk or move the DB-scoped Projection v2 namespace"
    );

    kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
            "--expected-inventory-digest",
            digest,
        ],
    )?
    .json_failure_containing("use --resume")?;
    let resumed = kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
            "--expected-inventory-digest",
            digest,
            "--resume",
        ],
    )?
    .success_json()?;
    assert_eq!(resumed["data"]["action"], "apply");
    assert_eq!(resumed["data"]["resumed"], true);

    let verified = kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "verify",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
        ],
    )?
    .success_json()?;
    assert_eq!(verified["data"]["action"], "verify");
    assert_eq!(verified["data"]["inventory_digest"], digest);

    let restored = kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "restore",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
        ],
    )?
    .success_json()?;
    assert_eq!(restored["data"]["action"], "restore");
    assert!(legacy_file.is_file());
    assert_eq!(std::fs::read(&legacy_file)?, b"legacy-task-index");
    assert!(database_scoped_v2.join("keep").is_file());

    let owner = kanban_sqlite::db::connect_file(&temp.path)?.query_row(
        "SELECT owner,lease_token,lease_expires_at
         FROM projection_maintenance_owner WHERE singleton=1",
        [],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        },
    )?;
    assert_eq!(owner, (None, None, None));
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn maintenance_cleanup_legacy_preserves_structured_validation_and_storage_errors()
-> anyhow::Result<()> {
    let temp = TempDb::new("maintenance_cleanup_legacy_error_contract")?;
    kanban(&temp.path, &["init"])?.success()?;
    let legacy_file = temp.dir.join("index/v1/tasks/segment/doc");
    std::fs::create_dir_all(
        legacy_file
            .parent()
            .context("legacy file must have a parent")?,
    )?;
    std::fs::write(&legacy_file, b"legacy-task-index")?;
    let backup_parent = tempfile::Builder::new()
        .prefix("kb-cli-maintenance-cleanup-errors-")
        .tempdir()
        .context("create independent backup parent")?;
    let backup_dir = backup_parent.path().join("projection-v1-backup");
    let inventory = kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "inventory",
        ],
    )?
    .success_json()?;
    let digest = inventory["data"]["inventory_digest"]
        .as_str()
        .context("inventory digest")?;

    kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
            "--expected-inventory-digest",
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        ],
    )?
    .json_failure_contract_containing(2, "invalid_input", "inventory digest mismatch")?;

    let overlapping_backup = temp.dir.join("cleanup-backup");
    kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            overlapping_backup
                .to_str()
                .context("UTF-8 overlapping backup path")?,
            "--expected-inventory-digest",
            digest,
        ],
    )?
    .json_failure_contract_containing(
        2,
        "invalid_input",
        "backup path overlaps managed data",
    )?;

    kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
            "--expected-inventory-digest",
            digest,
            "--resume",
        ],
    )?
    .json_failure_contract_containing(2, "invalid_input", "no backup state to resume")?;

    kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
            "--expected-inventory-digest",
            digest,
        ],
    )?
    .success_json()?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "apply",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
            "--expected-inventory-digest",
            digest,
        ],
    )?
    .json_failure_contract_containing(2, "invalid_input", "use --resume")?;

    std::fs::write(backup_dir.join("journal.toml"), b"not = [valid toml")?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "cleanup-owner",
            "--json",
            "maintenance",
            "cleanup-legacy",
            "verify",
            "--backup-dir",
            backup_dir.to_str().context("UTF-8 backup path")?,
        ],
    )?
    .json_failure_contract_containing(1, "storage_error", "journal decoding failed")?;
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

#[derive(PartialEq, Eq)]
struct ExactBytes(Vec<u8>);

impl std::fmt::Debug for ExactBytes {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let digest = self.0.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            hash.wrapping_mul(0x100000001b3) ^ u64::from(*byte)
        });
        formatter
            .debug_struct("ExactBytes")
            .field("len", &self.0.len())
            .field("fnv64", &format_args!("{digest:016x}"))
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ExactTreeSnapshot(Vec<(PathBuf, &'static str, ExactBytes)>);

fn exact_tree_snapshot(root: &Path) -> anyhow::Result<ExactTreeSnapshot> {
    fn visit(
        root: &Path,
        path: &Path,
        entries: &mut Vec<(PathBuf, &'static str, ExactBytes)>,
    ) -> anyhow::Result<()> {
        let metadata = std::fs::symlink_metadata(path)?;
        let relative = path
            .strip_prefix(root)
            .context("snapshot entry must remain below its root")?
            .to_path_buf();
        if metadata.file_type().is_symlink() {
            let target = std::fs::read_link(path)?;
            entries.push((
                relative,
                "symlink",
                ExactBytes(target.as_os_str().as_encoded_bytes().to_vec()),
            ));
        } else if metadata.is_file() {
            entries.push((relative, "file", ExactBytes(std::fs::read(path)?)));
        } else if metadata.is_dir() {
            entries.push((relative, "directory", ExactBytes(Vec::new())));
            let mut children = std::fs::read_dir(path)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<std::io::Result<Vec<_>>>()?;
            children.sort();
            for child in children {
                visit(root, &child, entries)?;
            }
        } else {
            anyhow::bail!(
                "unsupported filesystem entry in snapshot: {}",
                path.display()
            );
        }
        Ok(())
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries)?;
    Ok(ExactTreeSnapshot(entries))
}

fn checkpoint_sqlite_for_read_only_snapshot(path: &Path) -> anyhow::Result<()> {
    let conn = kanban_sqlite::db::connect_file(path)?;
    let _: (i64, i64, i64) = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    drop(conn);
    Ok(())
}
