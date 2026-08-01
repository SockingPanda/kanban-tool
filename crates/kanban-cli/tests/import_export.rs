mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir};
use kanban_sqlite::api::lifecycle::{begin_database_replace, publish_staged_database};
use kanban_sqlite::db::maintenance_lock_path;
use kanban_sqlite::init::init_database;
use pretty_assertions::assert_eq;
use std::path::Path;

fn mark_no_plan_required(db_path: &Path, task_id: &str) -> anyhow::Result<()> {
    kanban_sqlite::api::mark_execution_plan_not_required(
        db_path,
        "default",
        "cli-import-export-test",
        task_id,
        "import/export fixture does not need steps",
    )?;
    Ok(())
}

#[test]
fn backup_writes_database_copy() -> anyhow::Result<()> {
    let source = initialized_database("backup_writes_database_copy")?;
    let backup_path = source.dir.join("backup.sqlite");

    let backup = kanban(
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
fn backup_rejects_stdout_target() -> anyhow::Result<()> {
    let source = initialized_database("backup_rejects_stdout_target")?;

    kanban(&source.path, &["--json", "backup", "--out", "-"])?
        .json_failure_containing("backup --out requires a filesystem path")?;
    assert!(!source.dir.join("-").exists());
    Ok(())
}

#[test]
fn checkpoint_returns_wal_checkpoint_result() -> anyhow::Result<()> {
    let source = initialized_database("checkpoint_returns_wal_checkpoint_result")?;

    let checkpoint = kanban(&source.path, &["--json", "checkpoint"])?.success_json()?;

    assert_eq!(checkpoint["data"]["busy"], 0);
    Ok(())
}

#[test]
fn vacuum_succeeds_for_initialized_database() -> anyhow::Result<()> {
    let source = initialized_database("vacuum_succeeds_for_initialized_database")?;

    kanban(&source.path, &["--json", "vacuum"])?.success_json()?;
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
fn export_stdout_streams_jsonl_without_status_noise() -> anyhow::Result<()> {
    let source = TempDb::new("export_stdout_streams_jsonl_without_status_noise")?;
    let source_data = source_with_completed_run(&source)?;

    let result = kanban(&source.path, &["export", "--out", "-"])?;
    let stdout = String::from_utf8(result.output.stdout)?;
    let stderr = String::from_utf8(result.output.stderr)?;

    assert!(result.output.status.success(), "export failed: {stderr}");
    assert_eq!(stderr, "");
    assert!(stdout.contains(&source_data.task_id));
    assert!(stdout.contains(r#""type":"board""#));
    assert!(stdout.contains(r#""log_path":null"#));
    assert!(!stdout.contains("Exported "));
    assert!(!source.dir.join("-").exists());
    Ok(())
}

#[test]
fn export_stdout_rejects_json_envelope_mode() -> anyhow::Result<()> {
    let source = initialized_database("export_stdout_rejects_json_envelope_mode")?;

    kanban(&source.path, &["--json", "export", "--out", "-"])?
        .json_failure_containing("export --out - cannot be combined with --json")?;
    assert!(!source.dir.join("-").exists());
    Ok(())
}

#[test]
fn import_rejects_nonportable_run_log_without_restoring_database() -> anyhow::Result<()> {
    let source =
        TempDb::new("import_rejects_nonportable_run_log_without_restoring_database_source")?;
    let source_data = source_with_completed_run(&source)?;

    let (_, _, export_content) = export_board_snapshot(&source, &source_data.task_id)?;
    reject_import_with_nonportable_run_log(&source, &export_content)?;
    Ok(())
}

struct SourceData {
    task_id: String,
}

fn initialized_database(name: &str) -> anyhow::Result<TempDb> {
    let source = TempDb::new(name)?;
    kanban(&source.path, &["init"])?.success()?;
    Ok(source)
}

fn completed_replacement_fixture(
    name: &str,
) -> anyhow::Result<(
    TempDb,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
)> {
    let target = TempDb::new(name)?;
    kanban(&target.path, &["init"])?.success()?;
    let staged = target.dir.join(".kb.db.restore.fixture");
    let previous = target.dir.join(".kb.db.replaced.fixture");
    let journal = target.dir.join(".kb.db.replace.journal");
    init_database(&staged, "cli-completed-validation")?;
    let mut guard = begin_database_replace(&target.path)?;
    publish_staged_database(&mut guard, &target.path, &staged, &previous, &journal)?;
    drop(guard);
    Ok((target, staged, previous, journal))
}

fn source_with_completed_run(source: &TempDb) -> anyhow::Result<SourceData> {
    kanban(&source.path, &["init"])?.success()?;
    let created = kanban(
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
    mark_no_plan_required(&source.path, task_id)?;
    let dispatch = kanban_in_dir(
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
    let run = kanban_sqlite::api::get_run_by_id_global(&source.path, run_id)?;
    let log_path = run.log_path.as_deref().context("completed run log path")?;
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
    let export = kanban(
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
    let imported = kanban(
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
    assert_eq!(imported["data"]["dry_run"], false);
    assert_eq!(imported["data"]["records"], export_records);
    let tasks = kanban(&target.path, &["--json", "task", "list"])?.success_json()?;
    assert_eq!(tasks["data"][0]["id"], task_id);
    assert_eq!(tasks["data"][0]["title"], "release smoke");
    let doctor = kanban(&target.path, &["--json", "doctor"])?.success_json()?;
    assert_eq!(doctor["data"]["ok"], true);
    assert_eq!(doctor["data"]["missing_run_logs"], 0);
    assert_eq!(doctor["data"]["suspicious_run_log_paths"], 0);
    assert_eq!(doctor["data"]["executable_dependency_violations"], 0);
    assert_eq!(doctor["data"]["executable_spec_violations"], 0);
    assert_eq!(doctor["data"]["executable_schedule_violations"], 0);
    Ok(())
}

fn reject_import_with_nonportable_run_log(
    source: &TempDb,
    export_content: &str,
) -> anyhow::Result<()> {
    let invalid_export_path = source.dir.join("invalid-board.jsonl");
    std::fs::write(
        &invalid_export_path,
        export_content.replace(r#""log_path":null"#, r#""log_path":"/missing/kb-run.log""#),
    )?;
    let rejected = TempDb::new("maintenance_commands_rejected_import")?;
    let result = kanban(
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
    )?;
    assert_eq!(result.output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&result.output.stderr), "");
    let json: serde_json::Value = serde_json::from_slice(&result.output.stdout)?;
    assert_eq!(json["error"]["code"], "invalid_input");
    assert_eq!(json["error"]["exit_code"], 2);
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("run import row violates its contract"),
        "{json}"
    );
    assert!(!rejected.path.exists());
    Ok(())
}

#[test]
fn import_rejects_missing_input_without_creating_database() -> anyhow::Result<()> {
    let temp = TempDb::new("import_rejects_missing_input_without_creating_database")?;
    let input_path = temp.dir.join("missing.jsonl");

    kanban(
        &temp.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("import input does not exist")?;
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

    kanban(
        &temp.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .json_failure_containing("import requires --replace or --dry-run")?;
    assert!(!temp.path.exists());
    Ok(())
}

#[test]
fn import_dry_run_validates_without_creating_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_dry_run_validates_without_creating_database_source")?;
    kanban(&source.path, &["init"])?.success()?;
    let created = kanban(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "dry-run import source",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&source.path, task_id)?;
    let export_path = source.dir.join("dry-run.jsonl");
    let exported = kanban(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;

    let target = TempDb::new("import_dry_run_validates_without_creating_database_target")?;
    let dry_run = kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--dry-run",
        ],
    )?
    .success_json()?;

    assert_eq!(dry_run["data"]["records"], exported["data"]["records"]);
    assert_eq!(dry_run["data"]["dry_run"], true);
    assert_eq!(
        dry_run["data"]["input_path"],
        export_path.to_str().context("expected UTF-8 path")?
    );
    assert!(!target.path.exists());
    Ok(())
}

#[test]
fn import_dry_run_does_not_replace_existing_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_dry_run_does_not_replace_existing_database_source")?;
    kanban(&source.path, &["init"])?.success()?;
    let incoming = kanban(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "incoming dry-run task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let incoming_id = incoming["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&source.path, incoming_id)?;
    let export_path = source.dir.join("incoming.jsonl");
    kanban(
        &source.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .success_json()?;

    let target = TempDb::new("import_dry_run_does_not_replace_existing_database_target")?;
    kanban(&target.path, &["init"])?.success()?;
    let existing = kanban(
        &target.path,
        &[
            "--json",
            "task",
            "create",
            "existing task survives",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let existing_id = existing["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&target.path, existing_id)?;

    let dry_run = kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--dry-run",
        ],
    )?
    .success_json()?;
    assert_eq!(dry_run["data"]["dry_run"], true);

    let tasks = kanban(&target.path, &["--json", "task", "list"])?.success_json()?;
    assert_eq!(
        tasks["data"]
            .as_array()
            .context("expected tasks array")?
            .len(),
        1
    );
    assert_eq!(tasks["data"][0]["id"], existing_id);
    assert_eq!(tasks["data"][0]["title"], "existing task survives");
    Ok(())
}

// These are real CLI entry points: dry-run must remain side-effect free, while replace must roll
// back the pre-existing target when record-level compatibility detection fails.
#[test]
fn import_dry_run_rejects_hybrid_parent_record_without_creating_database() -> anyhow::Result<()> {
    let target = TempDb::new("import_dry_run_rejects_hybrid_parent_record")?;
    let input_path = target.dir.join("hybrid.jsonl");
    std::fs::write(&input_path, hybrid_parent_import_jsonl())?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--dry-run",
        ],
    )?
    .json_failure_containing("cannot contain both natural and parent storage-native keys")?;
    assert!(!target.path.exists());
    Ok(())
}

#[test]
fn import_replace_rejects_hybrid_parent_record_without_replacing_database() -> anyhow::Result<()> {
    let target = initialized_database(
        "import_replace_rejects_hybrid_parent_record_without_replacing_database",
    )?;
    let existing = kanban(
        &target.path,
        &[
            "--json",
            "task",
            "create",
            "existing task survives hybrid import",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let existing_id = existing["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&target.path, existing_id)?;
    let input_path = target.dir.join("hybrid.jsonl");
    std::fs::write(&input_path, hybrid_parent_import_jsonl())?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("cannot contain both natural and parent storage-native keys")?;

    let retained =
        kanban(&target.path, &["--json", "task", "show", existing_id])?.success_json()?;
    assert_eq!(
        retained["data"]["title"],
        "existing task survives hybrid import"
    );
    Ok(())
}

#[test]
fn import_replace_rejects_parent_snapshot_with_natural_setting_without_replacing_database()
-> anyhow::Result<()> {
    let target = initialized_database(
        "import_replace_rejects_parent_snapshot_with_natural_setting_without_replacing_database",
    )?;
    let existing = kanban(
        &target.path,
        &[
            "--json",
            "task",
            "create",
            "existing task survives cross-record import",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let existing_id = existing["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&target.path, existing_id)?;
    let input_path = target.dir.join("cross-record-mixed.jsonl");
    std::fs::write(
        &input_path,
        parent_snapshot_with_natural_setting_import_jsonl(),
    )?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("cannot mix natural and parent storage-native records")?;

    let retained =
        kanban(&target.path, &["--json", "task", "show", existing_id])?.success_json()?;
    assert_eq!(
        retained["data"]["title"],
        "existing task survives cross-record import"
    );
    Ok(())
}

#[test]
fn import_replace_restores_over_corrupt_existing_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_replace_restores_over_corrupt_existing_database_source")?;
    kanban(&source.path, &["init"])?.success()?;
    let created = kanban(
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
    mark_no_plan_required(&source.path, task_id)?;
    let export_path = source.dir.join("restore.jsonl");
    kanban(
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
    kanban(
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

    let imported = kanban(&target.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert_eq!(imported["data"]["title"], "restore me");
    assert_eq!(imported["data"]["status"], "ready");
    Ok(())
}

#[cfg(unix)]
#[test]
fn import_replace_rejects_symlinked_discoverable_journal() -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let target = TempDb::new("import_replace_rejects_symlinked_discoverable_journal")?;
    kanban(&target.path, &["init"])?.success()?;
    let input_path = target.dir.join("input.jsonl");
    std::fs::write(&input_path, b"not parsed after journal discovery")?;
    let target_json = target.dir.join("journal-target.json");
    let journal_path = target.dir.join(".kb.db.replace.journal");
    std::fs::write(&target_json, br#"{"phase":"completed"}"#)?;
    symlink(&target_json, &journal_path)?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_code_containing(2, "regular file")?;
    assert!(target.path.is_file());
    assert!(std::fs::symlink_metadata(&journal_path)?.is_symlink());
    Ok(())
}

#[test]
fn import_replace_rejects_malformed_discoverable_journal_as_invalid_input() -> anyhow::Result<()> {
    let target = TempDb::new("import_replace_rejects_malformed_discoverable_journal")?;
    kanban(&target.path, &["init"])?.success()?;
    let input_path = target.dir.join("input.jsonl");
    std::fs::write(&input_path, b"not parsed after journal discovery")?;
    let journal_path = target.dir.join(".kb.db.replace.journal");
    std::fs::write(&journal_path, b"{not valid json")?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_code_containing(2, "invalid replacement journal JSON")?;
    assert!(journal_path.is_file());
    Ok(())
}

#[cfg(unix)]
#[test]
fn import_replace_rejects_non_utf8_database_basename_without_journal_collision()
-> anyhow::Result<()> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let target = TempDb::new("import_replace_rejects_non_utf8_database_basename")?;
    let input_path = target.dir.join("input.jsonl");
    std::fs::write(&input_path, b"not parsed after path validation")?;
    for suffix in [&b"\xff.db"[..], &b"\xfe.db"[..]] {
        let db_path = target.dir.join(OsString::from_vec(
            (*b"kb-")
                .into_iter()
                .chain(suffix.iter().copied())
                .collect(),
        ));
        kanban(
            &db_path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().context("expected UTF-8 input path")?,
                "--replace",
            ],
        )?
        .json_failure_code_containing(2, "valid UTF-8")?;
        let entries = std::fs::read_dir(&target.dir)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .contains("replace.journal")
            })
            .collect::<Vec<_>>();
        assert!(
            entries.is_empty(),
            "unexpected journal for rejected basename"
        );
    }
    Ok(())
}

#[test]
fn import_replace_auto_resumes_discoverable_incomplete_journal() -> anyhow::Result<()> {
    let target = TempDb::new("import_replace_auto_resumes_discoverable_incomplete_journal")?;
    kanban(&target.path, &["init"])?.success()?;
    let staged = target.dir.join(".kb.db.restore.test");
    let previous = target.dir.join(".kb.db.replaced.test");
    let journal = target.dir.join(".kb.db.replace.journal");
    init_database(&staged, "cli-auto-resume")?;

    let mut guard = begin_database_replace(&target.path)?;
    publish_staged_database(&mut guard, &target.path, &staged, &previous, &journal)?;
    drop(guard);
    // Recreate the physical PreviousPublished state while preserving the
    // published staged inode. The CLI must discover this deterministic
    // journal and resume before attempting to parse its new input.
    std::fs::hard_link(&target.path, &staged)?;
    std::fs::remove_file(&target.path)?;
    let mut journal_json: serde_json::Value = serde_json::from_slice(&std::fs::read(&journal)?)?;
    journal_json["phase"] = serde_json::Value::String("previous_published".to_owned());
    std::fs::write(&journal, serde_json::to_vec_pretty(&journal_json)?)?;
    // Recovery discovery must run before validating a new import input.
    let input_path = target.dir.join("not-consumed.jsonl");

    let resumed = kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .success_json()?;
    assert_eq!(resumed["data"]["resumed"], true);
    assert_eq!(resumed["data"]["records"], 0);
    assert!(target.path.is_file());
    assert!(!staged.exists());
    assert!(previous.is_file());
    assert!(std::fs::read_to_string(&journal)?.contains("\"phase\": \"completed\""));
    Ok(())
}

#[cfg(unix)]
#[test]
fn import_replace_auto_resumes_across_symlinked_parent_aliases() -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;

    let root = TempDb::new("import_replace_auto_resumes_across_symlinked_parent_aliases")?;
    // Exercise both directions: the interrupted publication is recorded via
    // the real parent and resumed through its alias, then recorded through the
    // alias and resumed through the real parent. Service paths are normalized
    // to the same canonical parent in both cases.
    for (case_index, (start_via_alias, resume_via_alias)) in
        [(false, true), (true, false)].into_iter().enumerate()
    {
        let case_dir = root.dir.join(format!("case-{case_index}"));
        let real_dir = case_dir.join("real");
        let alias_dir = case_dir.join("alias");
        std::fs::create_dir_all(&real_dir)?;
        symlink(&real_dir, &alias_dir)?;

        let real_db = real_dir.join("kb.db");
        let start_db = if start_via_alias {
            alias_dir.join("kb.db")
        } else {
            real_db.clone()
        };
        let resume_db = if resume_via_alias {
            alias_dir.join("kb.db")
        } else {
            real_db.clone()
        };
        kanban(&start_db, &["init"])?.success()?;

        let staged = real_dir.join(".kb.db.restore.alias");
        let previous = real_dir.join(".kb.db.replaced.alias");
        let journal = real_dir.join(".kb.db.replace.journal");
        init_database(&staged, "cli-alias-resume")?;
        let mut guard = begin_database_replace(&start_db)?;
        publish_staged_database(&mut guard, &start_db, &staged, &previous, &journal)?;
        drop(guard);

        // Recreate a crash after the previous database was anchored but before
        // the staged inode was moved back to the canonical path.
        std::fs::hard_link(&real_db, &staged)?;
        std::fs::remove_file(&real_db)?;
        let mut journal_json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&journal)?)?;
        journal_json["phase"] = serde_json::Value::String("previous_published".to_owned());
        std::fs::write(&journal, serde_json::to_vec_pretty(&journal_json)?)?;

        let missing_input = case_dir.join("ignored-after-resume.jsonl");
        let resumed = kanban(
            &resume_db,
            &[
                "--json",
                "import",
                "--input",
                missing_input
                    .to_str()
                    .context("expected UTF-8 input path")?,
                "--replace",
            ],
        )?
        .success_json()?;
        assert_eq!(resumed["data"]["resumed"], true);
        assert_eq!(resumed["data"]["records"], 0);
        assert_eq!(resumed["data"]["dry_run"], false);
        assert!(real_db.is_file());
        assert!(!staged.exists());
        assert!(previous.is_file());
        assert!(std::fs::read_to_string(&journal)?.contains("\"phase\": \"completed\""));
    }
    Ok(())
}

#[test]
fn import_replace_rejects_completed_journal_with_missing_canonical() -> anyhow::Result<()> {
    let (target, _staged, previous, journal) = completed_replacement_fixture(
        "import_replace_rejects_completed_journal_with_missing_canonical",
    )?;
    std::fs::remove_file(&target.path)?;
    let input_path = target.dir.join("not-consumed.jsonl");
    std::fs::write(&input_path, b"not parsed after completed validation")?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("completed replacement canonical identity")?;
    assert!(!target.path.exists());
    assert!(previous.is_file());
    assert!(journal.is_file());
    Ok(())
}

#[test]
fn import_replace_rejects_completed_journal_with_replaced_canonical() -> anyhow::Result<()> {
    let (target, _staged, previous, journal) = completed_replacement_fixture(
        "import_replace_rejects_completed_journal_with_replaced_canonical",
    )?;
    let replacement = target.dir.join("untrusted-canonical.db");
    init_database(&replacement, "untrusted-replacement")?;
    std::fs::remove_file(&target.path)?;
    std::fs::rename(&replacement, &target.path)?;
    let input_path = target.dir.join("not-consumed.jsonl");
    std::fs::write(&input_path, b"not parsed after completed validation")?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("completed replacement canonical identity")?;
    assert!(target.path.is_file());
    assert!(previous.is_file());
    assert!(journal.is_file());
    Ok(())
}

#[test]
fn import_replace_rejects_completed_journal_with_replaced_previous() -> anyhow::Result<()> {
    let (target, _staged, previous, journal) = completed_replacement_fixture(
        "import_replace_rejects_completed_journal_with_replaced_previous",
    )?;
    std::fs::write(&previous, b"untrusted previous replacement")?;
    let input_path = target.dir.join("not-consumed.jsonl");
    std::fs::write(&input_path, b"not parsed after completed validation")?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("completed replacement previous identity")?;
    assert_eq!(std::fs::read(&previous)?, b"untrusted previous replacement");
    assert!(target.path.is_file());
    assert!(journal.is_file());
    Ok(())
}

#[test]
fn import_replace_rejects_completed_journal_with_retained_staged_entry() -> anyhow::Result<()> {
    let (target, staged, previous, journal) = completed_replacement_fixture(
        "import_replace_rejects_completed_journal_with_retained_staged_entry",
    )?;
    std::fs::write(&staged, b"untrusted retained staged evidence")?;
    let input_path = target.dir.join("not-consumed.jsonl");
    std::fs::write(&input_path, b"not parsed after completed validation")?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("staged database")?;
    assert_eq!(
        std::fs::read(&staged)?,
        b"untrusted retained staged evidence"
    );
    assert!(target.path.is_file());
    assert!(previous.is_file());
    assert!(journal.is_file());
    Ok(())
}

#[test]
fn import_replace_validated_completed_journal_still_validates_new_input() -> anyhow::Result<()> {
    let (target, _staged, previous, journal) = completed_replacement_fixture(
        "import_replace_validated_completed_journal_still_validates_new_input",
    )?;
    let missing_input = target.dir.join("missing-after-completed.jsonl");

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            missing_input
                .to_str()
                .context("expected UTF-8 input path")?,
            "--replace",
        ],
    )?
    .json_failure_code_containing(2, "import input does not exist")?;
    assert!(target.path.is_file());
    assert!(previous.is_file());
    assert!(
        !journal.exists(),
        "validated completed journal must quarantine"
    );
    Ok(())
}

#[test]
fn import_replace_rejects_maintenance_locked_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_replace_rejects_maintenance_locked_database_source")?;
    kanban(&source.path, &["init"])?.success()?;
    let incoming = kanban(
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
    let incoming_id = incoming["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    mark_no_plan_required(&source.path, incoming_id)?;
    let export_path = source.dir.join("restore.jsonl");
    kanban(
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
    kanban(&target.path, &["init"])?.success()?;
    let existing = kanban(
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
    mark_no_plan_required(&target.path, existing_id)?;
    let lock_path = maintenance_lock_path(&target.path);
    std::fs::write(&lock_path, "locked")?;

    kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("database is locked for maintenance")?;
    std::fs::remove_file(lock_path)?;

    let existing_after =
        kanban(&target.path, &["--json", "task", "show", existing_id])?.success_json()?;
    assert_eq!(existing_after["data"]["title"], "keep existing");
    Ok(())
}

#[test]
fn import_replace_rejects_directory_database_path() -> anyhow::Result<()> {
    let source = TempDb::new("import_replace_rejects_directory_database_path_source")?;
    kanban(&source.path, &["init"])?.success()?;
    let export_path = source.dir.join("restore.jsonl");
    kanban(
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

    kanban(
        &db_dir,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?
    .json_failure_containing("database path is not a file")?;
    assert!(db_dir.is_dir());
    Ok(())
}

#[test]
fn import_replace_rejects_running_work_in_target_database() -> anyhow::Result<()> {
    let source = TempDb::new("import_replace_rejects_running_work_source")?;
    kanban(&source.path, &["init"])?.success()?;
    let export_path = source.dir.join("restore.jsonl");
    kanban(
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
    kanban(&target.path, &["init"])?.success()?;
    let created = kanban(
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
    mark_no_plan_required(&target.path, task_id)?;
    kanban(&target.path, &["--json", "task", "claim", task_id])?.success_json()?;

    let import_result = kanban(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().context("expected UTF-8 path")?,
            "--replace",
        ],
    )?;
    assert!(
        !import_result.output.status.success(),
        "import unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&import_result.output.stdout),
        String::from_utf8_lossy(&import_result.output.stderr)
    );
    let stderr = String::from_utf8_lossy(&import_result.output.stderr);
    let stdout = String::from_utf8_lossy(&import_result.output.stdout);
    let failure_output = format!("{stderr}\n{stdout}");
    assert!(
        failure_output.contains("database has running work; stop kanban serve/dispatch"),
        "{failure_output}"
    );
    assert!(
        !failure_output.contains("stop kb serve/dispatch"),
        "{failure_output}"
    );
    let running = kanban(&target.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert_eq!(running["data"]["status"], "running");
    Ok(())
}

#[test]
fn export_scrubs_active_running_claims_for_roundtrip_import() -> anyhow::Result<()> {
    let source = TempDb::new("export_scrubs_active_running_claims_for_roundtrip_import")?;
    kanban(&source.path, &["init"])?.success()?;
    let created = kanban(
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
    mark_no_plan_required(&source.path, task_id)?;
    kanban(
        &source.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "600000"],
    )?
    .success_json()?;
    let export_path = source.dir.join("active-claim.jsonl");
    kanban(
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
    assert!(export_content.contains(r#""actor":"kanban export""#));
    assert!(!export_content.contains(r#""actor":"kb export""#));

    let target = TempDb::new("export_scrubs_active_running_claims_target")?;
    kanban(
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
    let imported = kanban(&target.path, &["--json", "task", "show", task_id])?.success_json()?;
    assert_eq!(imported["data"]["status"], "ready");
    assert!(imported["data"]["claim_token"].is_null());
    assert!(imported["data"]["current_run_id"].is_null());
    assert!(imported["data"]["started_at"].is_null());
    let events = kanban(&target.path, &["--json", "events", task_id])?.success_json()?;
    assert!(
        events["data"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|event| {
                event["kind"] == "task.export_sanitized"
                    && event["run_id"].as_str().is_some()
                    && event["actor"] == "kanban export"
            }),
        "events: {events}"
    );
    assert!(
        !events["data"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|event| event["actor"] == "kb export"),
        "events: {events}"
    );
    Ok(())
}

#[test]
fn export_rejects_existing_output_file() -> anyhow::Result<()> {
    let temp = TempDb::new("export_rejects_existing_output_file")?;
    kanban(&temp.path, &["init"])?.success()?;
    let export_path = temp.dir.join("board.jsonl");
    std::fs::write(&export_path, "keepme")?;

    kanban(
        &temp.path,
        &[
            "--json",
            "export",
            "--out",
            export_path.to_str().context("expected UTF-8 path")?,
        ],
    )?
    .json_failure_containing("export target already exists")?;
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

        kanban(
            &temp.path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().context("expected UTF-8 path")?,
                "--replace",
            ],
        )?
        .json_failure_containing("imported data failed doctor checks")?;
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

        kanban(
            &temp.path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().context("expected UTF-8 path")?,
                "--replace",
            ],
        )?
        .json_failure_containing(error)?;
        assert!(!temp.path.exists(), "{name}");
    }
    Ok(())
}

fn invalid_import_jsonl(records: &[serde_json::Value]) -> String {
    let mut lines = board_and_column_records();
    lines.extend(records.iter().map(ToString::to_string));
    lines.join("\n")
}

fn hybrid_parent_import_jsonl() -> String {
    let mut column = column_record();
    column["data"]["hidden"] = serde_json::json!(0);
    let mut task = task_record(
        "t_hybrid",
        1,
        "hybrid parent task",
        "todo",
        Some("specified"),
        None,
    );
    let data = task["data"].as_object_mut().expect("task data");
    let result = data.remove("result").expect("task result");
    data.insert("result_json".into(), result);
    let metadata = data.remove("metadata").expect("task metadata");
    data.insert("metadata_json".into(), metadata.to_string().into());
    data.insert(
        "metadata".into(),
        serde_json::json!({"source": "ambiguous natural value"}),
    );

    [board_record(), column, task]
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn parent_snapshot_with_natural_setting_import_jsonl() -> String {
    let mut column = column_record();
    column["data"]["hidden"] = serde_json::json!(0);
    let setting = serde_json::json!({
        "type": "setting",
        "data": {
            "key": "contract.fixture",
            "value": {"enabled": true},
            "updated_at": 2
        }
    });

    [board_record(), column, setting]
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n")
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
            "hidden": false,
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
            "result": null,
            "metadata": {},
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
