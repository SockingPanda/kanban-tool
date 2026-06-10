mod common;

use anyhow::Context;
use common::{TempDb, kb, kb_in_dir};
use kanban_sqlite::maintenance_lock_path;
use pretty_assertions::assert_eq;
use std::path::Path;
#[test]
fn backup_writes_database_copy() -> anyhow::Result<()> {
    let source = initialized_database("backup_writes_database_copy")?;
    let backup_path = source.dir.join("backup.sqlite");

    let backup = kb(
        &source.path,
        &[
            "--json",
            "backup",
            "--out",
            backup_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;

    assert_eq!(
        backup["data"]["out_path"],
        backup_path.to_str().context("expected UTF-8 path")?
    );
    assert!(backup_path.exists());
    assert!(std::fs::metadata(&backup_path)?.len() > 0);
    Ok(())
}

#[test]
fn checkpoint_returns_wal_checkpoint_result() -> anyhow::Result<()> {
    let source = initialized_database("checkpoint_returns_wal_checkpoint_result")?;

    let checkpoint = kb(&source.path, &["--json", "checkpoint"])?.success_json()?;

    assert_eq!(checkpoint["data"]["busy"], 0);
    Ok(())
}

#[test]
fn vacuum_succeeds_for_initialized_database() -> anyhow::Result<()> {
    let source = initialized_database("vacuum_succeeds_for_initialized_database")?;

    kb(&source.path, &["--json", "vacuum"])?.success_json()?;
    Ok(())
}

#[test]
fn export_import_round_trips_jsonl_snapshot() -> anyhow::Result<()> {
    let source = TempDb::new("export_import_round_trips_jsonl_snapshot_source")?;
    let source_data = source_with_completed_run(&source)?;

    let (export_path, export_records, export_content) =
        export_board_snapshot(&source, &source_data.task_id)?;
    import_exported_snapshot(&export_path, export_records, &source_data.task_id)?;
    assert!(export_content.contains(&source_data.task_id));
    Ok(())
}

#[test]
fn import_rejects_missing_run_log_without_restoring_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_rejects_missing_run_log_without_restoring_database_source")?;
    let source_data = source_with_completed_run(&source)?;

    let (_, _, export_content) = export_board_snapshot(&source, &source_data.task_id)?;
    reject_import_with_missing_run_log(&source, &export_content)?;
    Ok(())
}

struct SourceData {
    task_id: String,
}

fn initialized_database(name: &str) -> anyhow::Result<TempDb> {
    let source = TempDb::new(name)?;
    kb(&source.path, &["init"])?.success()?;
    Ok(source)
}

fn source_with_completed_run(source: &TempDb) -> anyhow::Result<SourceData> {
    kb(&source.path, &["init"])?.success()?;
    let created = kb(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "release smoke",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    let dispatch = kb_in_dir(
        &source.path,
        &[
            "--json",
            "dispatch",
            "--once",
            "--command",
            "printf 'exported log\\n'",
            "--log-dir",
            "logs",
        ],
        &source.dir,
    )?
    .success_json()?;
    let run_id = dispatch["data"]["run_id"]
        .as_str()
        .context("expected JSON string")?;
    let run = kb(&source.path, &["--json", "run", "show", run_id])?.success_json()?;
    let log_path = run["data"]["log_path"]
        .as_str()
        .context("expected JSON string")?;
    assert!(Path::new(log_path).is_absolute());
    assert!(log_path.starts_with(source.dir.to_str().context("expected UTF-8 path")?));
    Ok(SourceData {
        task_id: task_id.to_owned(),
    })
}

fn export_board_snapshot(
    source: &TempDb,
    task_id: &str,
) -> anyhow::Result<(std::path::PathBuf, serde_json::Value, String)> {
    let export_path = source.dir.join("board.jsonl");
    let export = kb(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;
    assert!(
        export["data"]["records"]
            .as_u64()
            .context("expected JSON u64")?
            >= 2
    );
    let export_content = std::fs::read_to_string(&export_path)?;
    assert!(export_content.contains(task_id));
    assert!(export_content.contains(r#""log_path":null"#));
    Ok((
        export_path,
        export["data"]["records"].clone(),
        export_content,
    ))
}

fn import_exported_snapshot(
    export_path: &Path,
    export_records: serde_json::Value,
    task_id: &str,
) -> anyhow::Result<()> {
    let target = TempDb::new("maintenance_commands_target")?;
    let imported = kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .success_json()?;
    assert_eq!(imported["data"]["records"], export_records);
    let tasks = kb(&target.path, &["--json", "task", "list"])?.success_json()?;
    assert_eq!(tasks["data"][0]["id"], task_id);
    assert_eq!(tasks["data"][0]["title"], "release smoke");
    let doctor = kb(&target.path, &["--json", "doctor"])?.success_json()?;
    assert_eq!(doctor["data"]["ok"], true);
    assert_eq!(doctor["data"]["missing_run_logs"], 0);
    assert_eq!(doctor["data"]["suspicious_run_log_paths"], 0);
    assert_eq!(doctor["data"]["executable_dependency_violations"], 0);
    assert_eq!(doctor["data"]["executable_spec_violations"], 0);
    assert_eq!(doctor["data"]["executable_schedule_violations"], 0);
    Ok(())
}

fn reject_import_with_missing_run_log(source: &TempDb, export_content: &str) -> anyhow::Result<()> {
    let invalid_export_path = source.dir.join("invalid-board.jsonl");
    std::fs::write(
        &invalid_export_path,
        export_content.replace(r#""log_path":null"#, r#""log_path":"/missing/kb-run.log""#),
    )?;
    let rejected = TempDb::new("maintenance_commands_rejected_import")?;
    kb(
        &rejected.path,
        &[
            "--json",
            "import",
            "--input",
            invalid_export_path
                .to_str()
                .context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .failure_containing("imported data failed doctor checks")?;
    assert!(!rejected.path.exists());
    Ok(())
}

#[test]
fn import_rejects_missing_input_without_creating_database() -> anyhow::Result<()> {
    let temp = TempDb::new("import_rejects_missing_input_without_creating_database")?;
    let input_path = temp.dir.join("missing.jsonl");

    kb(
        &temp.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .failure_containing("import input does not exist")?;
    assert!(!temp.path.exists());
    Ok(())
}

#[test]
fn import_requires_replace_without_creating_database() -> anyhow::Result<()> {
    let temp = TempDb::new("import_requires_replace_without_creating_database")?;
    let input_path = temp.dir.join("board.jsonl");
    std::fs::write(
        &input_path,
        invalid_import_jsonl(&[task_record(
            "t_ready",
            1,
            "ready",
            "ready",
            Some("spec"),
            None,
        )]),
    )?;

    kb(
        &temp.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .failure_containing("import requires --replace")?;
    assert!(!temp.path.exists());
    Ok(())
}

#[test]
fn import_replace_restores_over_corrupt_existing_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_replace_restores_over_corrupt_existing_database_source")?;
    kb(&source.path, &["init"])?.success()?;
    let created = kb(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "restore me",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;

    let target = TempDb::new("import_replace_restores_over_corrupt_existing_database_target")?;
    std::fs::write(&target.path, b"not a sqlite database")?;
    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .success_json()?;

    let imported = kb(&target.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert_eq!(imported["data"]["title"], "restore me");
    assert_eq!(imported["data"]["status"], "ready");
    Ok(())
}

#[test]
fn import_replace_rejects_maintenance_locked_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_replace_rejects_maintenance_locked_database_source")?;
    kb(&source.path, &["init"])?.success()?;
    kb(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "incoming task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;

    let target = TempDb::new("import_replace_rejects_maintenance_locked_database_target")?;
    kb(&target.path, &["init"])?.success()?;
    let existing = kb(
        &target.path,
        &[
            "--json",
            "task",
            "create",
            "keep existing",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let existing_id = existing["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    let lock_path = maintenance_lock_path(&target.path);
    std::fs::write(&lock_path, "locked")?;

    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .failure_containing("database is locked for maintenance")?;
    std::fs::remove_file(lock_path)?;

    let existing_after =
        kb(&target.path, &["--json", "task", "show", existing_id])?.success_json()?;
    assert_eq!(existing_after["data"]["title"], "keep existing");
    Ok(())
}

#[test]
fn import_replace_rejects_directory_database_path() -> anyhow::Result<()> {
    let source = TempDb::new("import_replace_rejects_directory_database_path_source")?;
    kb(&source.path, &["init"])?.success()?;
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;

    let target = TempDb::new("import_replace_rejects_directory_database_path_target")?;
    let db_dir = target.dir.join("db-dir");
    std::fs::create_dir(&db_dir)?;

    kb(
        &db_dir,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .failure_containing("database path is not a file")?;
    assert!(db_dir.is_dir());
    Ok(())
}

#[test]
fn import_replace_rejects_running_work_in_target_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_replace_rejects_running_work_source")?;
    kb(&source.path, &["init"])?.success()?;
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;

    let target = TempDb::new("import_replace_rejects_running_work_target")?;
    kb(&target.path, &["init"])?.success()?;
    let created = kb(
        &target.path,
        &[
            "--json",
            "task",
            "create",
            "running work",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    kb(&target.path, &["--json", "task", "claim", task_id])?.success_json()?;

    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .failure_containing("database has running work")?;
    let running = kb(&target.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert_eq!(running["data"]["status"], "running");
    Ok(())
}

#[test]
fn export_scrubs_active_running_claims_for_roundtrip_import() -> anyhow::Result<()> {
    let source = TempDb::new("export_scrubs_active_running_claims_for_roundtrip_import")?;
    kb(&source.path, &["init"])?.success()?;
    let created = kb(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "active claim restore",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    kb(
        &source.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "600000"],
    )?
    .success_json()?;
    let export_path = source.dir.join("active-claim.jsonl");
    kb(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;
    let export_content = std::fs::read_to_string(&export_path)?;
    assert!(export_content.contains(r#""status":"ready""#));
    assert!(export_content.contains(r#""current_run_id":null"#));
    assert!(export_content.contains(r#""claim_token":null"#));

    let target = TempDb::new("export_scrubs_active_running_claims_target")?;
    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .success_json()?;
    let imported = kb(&target.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert_eq!(imported["data"]["status"], "ready");
    assert!(imported["data"]["claim_token"].is_null());
    assert!(imported["data"]["current_run_id"].is_null());
    assert!(imported["data"]["started_at"].is_null());
    let events = kb(&target.path, &["--json", "events", task_id])?.success_json()?;
    assert!(
        events["data"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|event| {
                event["kind"] == "task.export_sanitized" && event["run_id"].as_str().is_some()
            }),
        "events: {events}"
    );
    Ok(())
}

#[test]
fn export_rejects_existing_output_file() -> anyhow::Result<()> {
    let temp = TempDb::new("export_rejects_existing_output_file")?;
    kb(&temp.path, &["init"])?.success()?;
    let export_path = temp.dir.join("board.jsonl");
    std::fs::write(&export_path, "keepme")?;

    kb(
        &temp.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .failure_containing("export target already exists")?;
    assert_eq!(std::fs::read_to_string(&export_path)?, "keepme");
    Ok(())
}

#[test]
fn import_rejects_executable_status_invariant_violations() -> anyhow::Result<()> {
    let cases = [
        (
            "dependency",
            invalid_import_jsonl(&[
                task_record("t_parent", 1, "parent", "todo", Some("spec"), None),
                task_record("t_child", 2, "child", "ready", Some("spec"), None),
                dependency_record("t_parent", "t_child"),
            ]),
            "executable_dependency_violations",
        ),
        (
            "spec",
            invalid_import_jsonl(&[task_record(
                "t_missing_spec",
                1,
                "missing spec",
                "ready",
                None,
                None,
            )]),
            "executable_spec_violations",
        ),
        (
            "schedule",
            invalid_import_jsonl(&[task_record(
                "t_future",
                1,
                "future",
                "ready",
                Some("spec"),
                Some(4_102_444_800_000),
            )]),
            "executable_schedule_violations",
        ),
    ];

    for (name, jsonl, doctor_field) in cases {
        let temp = TempDb::new(&format!(
            "import_rejects_executable_status_invariant_violations_{name}"
        ))?;
        let input_path = temp.dir.join("invalid.jsonl");
        std::fs::write(&input_path, jsonl)?;

        kb(
            &temp.path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().context("expected UTF-8 path")?,
                "--replace",
            ],
        )?
        .failure_containing("imported data failed doctor checks")?;
        assert!(!temp.path.exists(), "{doctor_field}");
    }
    Ok(())
}

#[test]
fn import_rejects_empty_or_unusable_snapshots_without_creating_database() -> anyhow::Result<()> {
    let cases = [
        (
            "empty",
            String::new(),
            "imported data must contain at least one board",
        ),
        (
            "board_without_columns",
            board_only_import_jsonl(),
            "imported data must contain columns for every board",
        ),
    ];

    for (name, jsonl, error) in cases {
        let temp = TempDb::new(&format!("import_rejects_unusable_snapshot_{name}"))?;
        let input_path = temp.dir.join("unusable.jsonl");
        std::fs::write(&input_path, jsonl)?;

        kb(
            &temp.path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().context("expected UTF-8 path")?,
                "--replace",
            ],
        )?
        .failure_containing(error)?;
        assert!(!temp.path.exists(), "{name}");
    }
    Ok(())
}

fn invalid_import_jsonl(records: &[serde_json::Value]) -> String {
    let mut lines = board_and_column_records();
    lines.extend(records.iter().map(ToString::to_string));
    lines.join("\n")
}

fn board_only_import_jsonl() -> String {
    board_record().to_string()
}

fn board_and_column_records() -> Vec<String> {
    vec![board_record().to_string(), column_record().to_string()]
}

fn board_record() -> serde_json::Value {
    serde_json::json!({
        "type": "board",
        "data": {
            "id": "b_import",
            "slug": "default",
            "name": "Default",
            "description": null,
            "created_at": 1,
            "updated_at": 1,
            "archived_at": null
        }
    })
}

fn column_record() -> serde_json::Value {
    serde_json::json!({
        "type": "column",
        "data": {
            "id": "col_import_ready",
            "board_id": "b_import",
            "status": "ready",
            "title": "Ready",
            "position": 40,
            "hidden": 0,
            "wip_limit": null,
            "created_at": 1,
            "updated_at": 1
        }
    })
}

fn task_record(
    id: &str,
    seq: i64,
    title: &str,
    status: &str,
    description: Option<&str>,
    scheduled_at: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "type": "task",
        "data": {
            "id": id,
            "board_id": "b_import",
            "seq": seq,
            "title": title,
            "description": description,
            "status": status,
            "status_reason": null,
            "assignee": null,
            "priority": 0,
            "position": seq * 1024,
            "scheduled_at": scheduled_at,
            "due_at": null,
            "created_by": "test",
            "created_at": 1,
            "updated_at": 1,
            "started_at": null,
            "completed_at": null,
            "archived_at": null,
            "claim_token": null,
            "claim_owner": null,
            "claim_expires_at": null,
            "last_heartbeat_at": null,
            "current_run_id": null,
            "retry_count": 0,
            "max_retries": null,
            "result_summary": null,
            "result_json": null,
            "metadata_json": "{}",
            "lock_version": 0
        }
    })
}

fn dependency_record(parent: &str, child: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "dependency",
        "data": {
            "board_id": "b_import",
            "parent_task_id": parent,
            "child_task_id": child,
            "created_at": 1
        }
    })
}
