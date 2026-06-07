use std::{path::Path, process::Command};

use kanban_sqlite::maintenance_lock_path;

#[test]
fn serve_help_includes_default_localhost_bind_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_kb"))
        .args(["serve", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--host"), "{stdout}");
    assert!(stdout.contains("127.0.0.1"), "{stdout}");
    assert!(stdout.contains("--port"), "{stdout}");
    assert!(stdout.contains("8721"), "{stdout}");
}

#[test]
fn serve_rejects_non_loopback_host_without_creating_database() {
    let temp = TempDb::new("serve_rejects_non_loopback_host_without_creating_database");

    kb(&temp.path, &["serve", "--host", "0.0.0.0", "--port", "0"])
        .failure_containing("kb serve only supports loopback hosts");
    assert!(!temp.path.exists());
}

#[test]
fn task_update_sets_and_clears_scheduled_at_and_due_at() {
    let temp = TempDb::new("task_update_sets_and_clears_scheduled_at_and_due_at");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli update dates",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();

    let updated = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--scheduled-at",
            "1767225600000",
            "--due-at",
            "1767312000000",
        ],
    )
    .success_json();
    assert_eq!(updated["data"]["scheduled_at"], 1_767_225_600_000_i64);
    assert_eq!(updated["data"]["due_at"], 1_767_312_000_000_i64);

    let cleared = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--clear-scheduled-at",
            "--clear-due-at",
        ],
    )
    .success_json();
    assert!(cleared["data"]["scheduled_at"].is_null());
    assert!(cleared["data"]["due_at"].is_null());
}

#[test]
fn task_complete_alias_finishes_like_done() {
    let temp = TempDb::new("task_complete_alias_finishes_like_done");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli complete alias",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    let claim = kb(&temp.path, &["--json", "task", "claim", task_id]).success_json();
    let token = claim["data"]["claim_token"].as_str().unwrap();

    let completed = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "complete",
            task_id,
            "--claim-token",
            token,
        ],
    )
    .success_json();
    assert_eq!(completed["data"]["status"], "done");
}

#[test]
fn task_reclaim_expired_alias_matches_bare_reclaim() {
    let bare = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_bare");
    let explicit = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_explicit");

    for temp in [&bare, &explicit] {
        kb(&temp.path, &["init"]).success();
        let created = kb(
            &temp.path,
            &[
                "--json",
                "task",
                "create",
                "cli reclaim alias",
                "--description",
                "ready spec",
            ],
        )
        .success_json();
        let task_id = created["data"]["id"].as_str().unwrap();
        kb(
            &temp.path,
            &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
        )
        .success_json();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let bare_result = kb(&bare.path, &["--json", "task", "reclaim"]).success_json();
    let explicit_result =
        kb(&explicit.path, &["--json", "task", "reclaim", "--expired"]).success_json();

    assert_eq!(bare_result, explicit_result);
    assert_eq!(explicit_result["data"]["reclaimed"], 1);
}

#[test]
fn doctor_reports_integrity_migration_and_expired_running_tasks() {
    let temp = TempDb::new("doctor_reports_integrity_migration_and_expired_running_tasks");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "doctor expired",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    kb(
        &temp.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
    )
    .success_json();
    std::thread::sleep(std::time::Duration::from_millis(5));

    let doctor = kb(&temp.path, &["--json", "doctor"]).success_json();

    assert_eq!(doctor["data"]["integrity_check"], "ok");
    assert_eq!(doctor["data"]["migration_version"], 1);
    assert_eq!(doctor["data"]["user_version"], 1);
    assert_eq!(doctor["data"]["expired_running_tasks"], 1);
    assert_eq!(doctor["data"]["dependency_cycles"], 0);
    assert_eq!(doctor["data"]["archived_dependency_edges"], 0);
    assert_eq!(doctor["data"]["missing_run_logs"], 0);
    assert_eq!(doctor["data"]["executable_dependency_violations"], 0);
    assert_eq!(doctor["data"]["executable_spec_violations"], 0);
    assert_eq!(doctor["data"]["executable_schedule_violations"], 0);
    assert_eq!(doctor["data"]["ok"], false);
}

#[test]
fn dispatch_loop_uses_worker_profile_config_and_respects_assignee_routing() {
    let temp =
        TempDb::new("dispatch_loop_uses_worker_profile_config_and_respects_assignee_routing");
    kb(&temp.path, &["init"]).success();
    let backend = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "backend work",
            "--description",
            "ready spec",
            "--assignee",
            "backend",
        ],
    )
    .success_json();
    let frontend = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "frontend work",
            "--description",
            "ready spec",
            "--assignee",
            "frontend",
        ],
    )
    .success_json();
    let config = temp.dir.join("workers.toml");
    let logs = temp.dir.join("logs");
    std::fs::write(
        &config,
        format!(
            "[workers.backend]\ncommand = \"sh -c 'true'\"\nclaim_ttl_ms = 60000\nheartbeat_interval_ms = 10\non_success = \"done\"\non_failure = \"blocked\"\nlog_dir = \"{}\"\n",
            logs.display()
        ),
    )
    .unwrap();

    let result = kb(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--worker-profile",
            "backend",
            "--profile-config",
            config.to_str().unwrap(),
            "--max-iterations",
            "1",
        ],
    )
    .success_json();

    assert_eq!(result["data"]["iterations"], 1);
    assert_eq!(result["data"]["claimed"], 1);
    let backend_task = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            backend["data"]["id"].as_str().unwrap(),
        ],
    )
    .success_json();
    let frontend_task = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            frontend["data"]["id"].as_str().unwrap(),
        ],
    )
    .success_json();
    assert_eq!(backend_task["data"]["status"], "done");
    assert_eq!(frontend_task["data"]["status"], "ready");
}

#[test]
fn retry_policy_and_run_log_commands_support_operator_recovery() {
    let temp = TempDb::new("retry_policy_and_run_log_commands_support_operator_recovery");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "retry policy",
            "--description",
            "ready spec",
            "--max-retries",
            "2",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    assert_eq!(created["data"]["max_retries"], 2);

    let updated = kb(
        &temp.path,
        &["--json", "task", "update", task_id, "--clear-max-retries"],
    )
    .success_json();
    assert!(updated["data"]["max_retries"].is_null());

    let reset = kb(
        &temp.path,
        &["--json", "task", "update", task_id, "--max-retries", "1"],
    )
    .success_json();
    assert_eq!(reset["data"]["max_retries"], 1);

    let logs = temp.dir.join("logs");
    let dispatch = kb(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--once",
            "--command",
            "printf 'operator log\\n'",
            "--log-dir",
            logs.to_str().unwrap(),
        ],
    )
    .success_json();
    let run_id = dispatch["data"]["run_id"].as_str().unwrap();

    let run = kb(&temp.path, &["--json", "run", "show", run_id]).success_json();
    assert_eq!(run["data"]["id"], run_id);
    assert!(run["data"].get("claim_token").is_some());

    let log = kb(&temp.path, &["--json", "run", "logs", run_id]).success_json();
    assert_eq!(log["data"]["run_id"], run_id);
    assert_eq!(log["data"]["content"], "operator log\n");
    assert_eq!(log["data"]["truncated"], false);
}

#[test]
fn stats_command_reports_stale_claims_and_blocked_reasons() {
    let temp = TempDb::new("stats_command_reports_stale_claims_and_blocked_reasons");
    kb(&temp.path, &["init"]).success();
    let stale = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "stale cli",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let stale_id = stale["data"]["id"].as_str().unwrap();
    kb(
        &temp.path,
        &["--json", "task", "claim", stale_id, "--ttl-ms", "1"],
    )
    .success_json();
    let blocked = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "blocked cli",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let blocked_id = blocked["data"]["id"].as_str().unwrap();
    kb(
        &temp.path,
        &[
            "--json",
            "task",
            "block",
            blocked_id,
            "operator needed",
            "--force",
        ],
    )
    .success_json();
    std::thread::sleep(std::time::Duration::from_millis(5));

    let stats = kb(&temp.path, &["--json", "stats"]).success_json();

    assert_eq!(stats["data"]["stale_claims"][0]["task_id"], stale_id);
    assert_eq!(
        stats["data"]["blocked_reasons"][0]["reason"],
        "operator needed"
    );
    assert_eq!(stats["data"]["blocked_reasons"][0]["count"], 1);
}

#[test]
fn maintenance_commands_backup_checkpoint_vacuum_export_and_import_jsonl() {
    let source = TempDb::new("maintenance_commands_source");
    kb(&source.path, &["init"]).success();
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
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
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
    )
    .success_json();
    let run_id = dispatch["data"]["run_id"].as_str().unwrap();
    let run = kb(&source.path, &["--json", "run", "show", run_id]).success_json();
    let log_path = run["data"]["log_path"].as_str().unwrap();
    assert!(Path::new(log_path).is_absolute());
    assert!(log_path.starts_with(source.dir.to_str().unwrap()));

    let checkpoint = kb(&source.path, &["--json", "checkpoint"]).success_json();
    assert_eq!(checkpoint["data"]["busy"], 0);
    kb(&source.path, &["--json", "vacuum"]).success_json();

    let backup_path = source.dir.join("backup.sqlite");
    let backup = kb(
        &source.path,
        &["--json", "backup", "--out", backup_path.to_str().unwrap()],
    )
    .success_json();
    assert_eq!(backup["data"]["out_path"], backup_path.to_str().unwrap());
    assert!(backup_path.exists());

    let export_path = source.dir.join("board.jsonl");
    let export = kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();
    assert!(export["data"]["records"].as_u64().unwrap() >= 2);
    let export_content = std::fs::read_to_string(&export_path).unwrap();
    assert!(export_content.contains(task_id));
    assert!(export_content.contains(r#""log_path":null"#));

    let target = TempDb::new("maintenance_commands_target");
    let imported = kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .success_json();
    assert_eq!(imported["data"]["records"], export["data"]["records"]);
    let tasks = kb(&target.path, &["--json", "task", "list"]).success_json();
    assert_eq!(tasks["data"][0]["id"], task_id);
    assert_eq!(tasks["data"][0]["title"], "release smoke");
    let doctor = kb(&target.path, &["--json", "doctor"]).success_json();
    assert_eq!(doctor["data"]["ok"], true);
    assert_eq!(doctor["data"]["missing_run_logs"], 0);
    assert_eq!(doctor["data"]["executable_dependency_violations"], 0);
    assert_eq!(doctor["data"]["executable_spec_violations"], 0);
    assert_eq!(doctor["data"]["executable_schedule_violations"], 0);

    let invalid_export_path = source.dir.join("invalid-board.jsonl");
    std::fs::write(
        &invalid_export_path,
        export_content.replace(r#""log_path":null"#, r#""log_path":"/missing/kb-run.log""#),
    )
    .unwrap();
    let rejected = TempDb::new("maintenance_commands_rejected_import");
    kb(
        &rejected.path,
        &[
            "--json",
            "import",
            "--input",
            invalid_export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("imported data failed doctor checks");
    assert!(!rejected.path.exists());
}

#[test]
fn maintenance_commands_reject_missing_database() {
    let temp = TempDb::new("maintenance_commands_reject_missing_database");
    let backup_path = temp.dir.join("backup.sqlite");
    let nested_backup_path = temp.dir.join("new/subdir/backup.sqlite");
    let export_path = temp.dir.join("board.jsonl");
    let cases = [
        vec!["--json", "doctor"],
        vec!["--json", "checkpoint"],
        vec!["--json", "vacuum"],
        vec!["--json", "backup", "--out", backup_path.to_str().unwrap()],
        vec![
            "--json",
            "backup",
            "--out",
            nested_backup_path.to_str().unwrap(),
        ],
        vec!["--json", "export", "--out", export_path.to_str().unwrap()],
    ];

    for args in cases {
        kb(&temp.path, &args).failure_containing("database does not exist");
        assert!(
            !temp.path.exists(),
            "maintenance command should not create missing DB for args {args:?}"
        );
    }
    assert!(!backup_path.exists());
    assert!(!nested_backup_path.parent().unwrap().exists());
    assert!(!export_path.exists());
}

#[test]
fn import_rejects_missing_input_without_creating_database() {
    let temp = TempDb::new("import_rejects_missing_input_without_creating_database");
    let input_path = temp.dir.join("missing.jsonl");

    kb(
        &temp.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("import input does not exist");
    assert!(!temp.path.exists());
}

#[test]
fn import_requires_replace_without_creating_database() {
    let temp = TempDb::new("import_requires_replace_without_creating_database");
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
    )
    .unwrap();

    kb(
        &temp.path,
        &["--json", "import", "--input", input_path.to_str().unwrap()],
    )
    .failure_containing("import requires --replace");
    assert!(!temp.path.exists());
}

#[test]
fn import_replace_restores_over_corrupt_existing_database() {
    let source = TempDb::new("import_replace_restores_over_corrupt_existing_database_source");
    kb(&source.path, &["init"]).success();
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
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();

    let target = TempDb::new("import_replace_restores_over_corrupt_existing_database_target");
    std::fs::write(&target.path, b"not a sqlite database").unwrap();
    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .success_json();

    let imported = kb(&target.path, &["--json", "task", "show", task_id]).success_json();
    assert_eq!(imported["data"]["title"], "restore me");
    assert_eq!(imported["data"]["status"], "ready");
}

#[test]
fn import_replace_rejects_maintenance_locked_database() {
    let source = TempDb::new("import_replace_rejects_maintenance_locked_database_source");
    kb(&source.path, &["init"]).success();
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
    )
    .success_json();
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();

    let target = TempDb::new("import_replace_rejects_maintenance_locked_database_target");
    kb(&target.path, &["init"]).success();
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
    )
    .success_json();
    let existing_id = existing["data"]["id"].as_str().unwrap();
    let lock_path = maintenance_lock_path(&target.path);
    std::fs::write(&lock_path, "locked").unwrap();

    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("database is locked for maintenance");
    std::fs::remove_file(lock_path).unwrap();

    let existing_after = kb(&target.path, &["--json", "task", "show", existing_id]).success_json();
    assert_eq!(existing_after["data"]["title"], "keep existing");
}

#[test]
fn import_replace_rejects_directory_database_path() {
    let source = TempDb::new("import_replace_rejects_directory_database_path_source");
    kb(&source.path, &["init"]).success();
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();

    let target = TempDb::new("import_replace_rejects_directory_database_path_target");
    let db_dir = target.dir.join("db-dir");
    std::fs::create_dir(&db_dir).unwrap();

    kb(
        &db_dir,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("database path is not a file");
    assert!(db_dir.is_dir());
}

#[test]
fn import_replace_rejects_running_work_in_target_database() {
    let source = TempDb::new("import_replace_rejects_running_work_source");
    kb(&source.path, &["init"]).success();
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();

    let target = TempDb::new("import_replace_rejects_running_work_target");
    kb(&target.path, &["init"]).success();
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
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    kb(&target.path, &["--json", "task", "claim", task_id]).success_json();

    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("database has running work");
    let running = kb(&target.path, &["--json", "task", "show", task_id]).success_json();
    assert_eq!(running["data"]["status"], "running");
}

#[test]
fn maintenance_lock_uses_canonical_database_path() {
    let temp = TempDb::new("maintenance_lock_uses_canonical_database_path");
    kb(&temp.path, &["init"]).success();
    let lock_path = maintenance_lock_path(&temp.path);
    std::fs::write(&lock_path, format!("pid={}", std::process::id())).unwrap();

    kb_in_dir(Path::new("kb.db"), &["--json", "doctor"], &temp.dir)
        .failure_containing("database is locked for maintenance");
    std::fs::remove_file(lock_path).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn maintenance_lock_with_dead_pid_is_removed() {
    let temp = TempDb::new("maintenance_lock_with_dead_pid_is_removed");
    kb(&temp.path, &["init"]).success();
    let lock_path = maintenance_lock_path(&temp.path);
    std::fs::write(&lock_path, "pid=999999999").unwrap();

    kb(&temp.path, &["--json", "doctor"]).success_json();
    assert!(!lock_path.exists());
}

#[test]
fn export_scrubs_active_running_claims_for_roundtrip_import() {
    let source = TempDb::new("export_scrubs_active_running_claims_for_roundtrip_import");
    kb(&source.path, &["init"]).success();
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
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    kb(
        &source.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "600000"],
    )
    .success_json();
    let export_path = source.dir.join("active-claim.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();
    let export_content = std::fs::read_to_string(&export_path).unwrap();
    assert!(export_content.contains(r#""status":"ready""#));
    assert!(export_content.contains(r#""current_run_id":null"#));
    assert!(export_content.contains(r#""claim_token":null"#));

    let target = TempDb::new("export_scrubs_active_running_claims_target");
    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .success_json();
    let imported = kb(&target.path, &["--json", "task", "show", task_id]).success_json();
    assert_eq!(imported["data"]["status"], "ready");
    assert!(imported["data"]["claim_token"].is_null());
    assert!(imported["data"]["current_run_id"].is_null());
    assert!(imported["data"]["started_at"].is_null());
    let events = kb(&target.path, &["--json", "events", task_id]).success_json();
    assert!(
        events["data"].as_array().unwrap().iter().any(|event| {
            event["kind"] == "task.export_sanitized" && event["run_id"].as_str().is_some()
        }),
        "events: {events}"
    );
}

#[test]
fn export_rejects_existing_output_file() {
    let temp = TempDb::new("export_rejects_existing_output_file");
    kb(&temp.path, &["init"]).success();
    let export_path = temp.dir.join("board.jsonl");
    std::fs::write(&export_path, "keepme").unwrap();

    kb(
        &temp.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .failure_containing("export target already exists");
    assert_eq!(std::fs::read_to_string(&export_path).unwrap(), "keepme");
}

#[test]
fn import_rejects_executable_status_invariant_violations() {
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
        ));
        let input_path = temp.dir.join("invalid.jsonl");
        std::fs::write(&input_path, jsonl).unwrap();

        kb(
            &temp.path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().unwrap(),
                "--replace",
            ],
        )
        .failure_containing("imported data failed doctor checks");
        assert!(!temp.path.exists(), "{doctor_field}");
    }
}

#[test]
fn import_rejects_empty_or_unusable_snapshots_without_creating_database() {
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
        let temp = TempDb::new(&format!("import_rejects_unusable_snapshot_{name}"));
        let input_path = temp.dir.join("unusable.jsonl");
        std::fs::write(&input_path, jsonl).unwrap();

        kb(
            &temp.path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().unwrap(),
                "--replace",
            ],
        )
        .failure_containing(error);
        assert!(!temp.path.exists(), "{name}");
    }
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

fn kb(db_path: &Path, args: &[&str]) -> CmdResult {
    kb_in_dir(db_path, args, Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn kb_in_dir(db_path: &Path, args: &[&str], current_dir: &Path) -> CmdResult {
    let output = Command::new(env!("CARGO_BIN_EXE_kb"))
        .current_dir(current_dir)
        .arg("--db")
        .arg(db_path)
        .args(args)
        .output()
        .unwrap();
    CmdResult { output }
}

struct CmdResult {
    output: std::process::Output,
}

impl CmdResult {
    fn success(self) {
        assert!(
            self.output.status.success(),
            "status: {:?}\nstdout:\n{}\nstderr:\n{}",
            self.output.status.code(),
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
    }

    fn success_json(self) -> serde_json::Value {
        assert!(
            self.output.status.success(),
            "status: {:?}\nstdout:\n{}\nstderr:\n{}",
            self.output.status.code(),
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        serde_json::from_slice(&self.output.stdout).unwrap()
    }

    fn failure_containing(self, expected: &str) {
        assert!(
            !self.output.status.success(),
            "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        assert!(
            stderr.contains(expected),
            "expected stderr to contain {expected:?}, got:\n{stderr}"
        );
    }
}

struct TempDb {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!("kb-cli-v05-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kb.db");
        Self { dir, path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
