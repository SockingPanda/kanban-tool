use crate::common::*;

#[test]
fn doctor_resolves_legacy_relative_run_log_paths_against_database_dir() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_resolves_legacy_relative_run_log_paths_against_database_dir")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("legacy log path"),
    )?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &task.id)?;
    let log_dir = temp.dir.join("logs");
    dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf legacy".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir,
        },
    )?;
    let run = list_runs(&temp.path, "default", Some(&task.id))?[0].clone();
    let absolute_log_path = Path::new(
        run.log_path
            .as_ref()
            .ok_or_else(|| test_error("expected run log path"))?,
    );
    let relative_log_path = absolute_log_path
        .strip_prefix(&temp.dir)?
        .to_string_lossy()
        .to_string();
    connect_file(&temp.path)?.execute(
        "UPDATE task_runs SET log_path=?1 WHERE id=?2",
        params![relative_log_path, run.id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert_eq!(report.missing_run_logs, 0);
    assert!(report.ok);
    Ok(())
}

#[test]
fn doctor_counts_suspicious_run_log_paths_separately_from_missing_allowed_logs()
-> anyhow::Result<()> {
    let temp = TempDb::new("doctor_counts_suspicious_run_log_paths_separately")?;
    init_database(&temp.path, "tester")?;
    let suspicious_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("suspicious log path"),
    )?;
    let missing_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("missing log path"),
    )?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &suspicious_task.id)?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &missing_task.id)?;
    let log_dir = temp.dir.join("logs");
    let suspicious = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf suspicious".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: log_dir.clone(),
        },
    )?;
    let missing = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf missing".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 30_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir,
        },
    )?;
    assert_eq!(
        suspicious.task_id.as_deref(),
        Some(suspicious_task.id.as_str())
    );
    assert_eq!(missing.task_id.as_deref(), Some(missing_task.id.as_str()));
    let suspicious_run_id = suspicious
        .run_id
        .ok_or_else(|| test_error("expected suspicious run id"))?;
    let missing_run_id = missing
        .run_id
        .ok_or_else(|| test_error("expected missing run id"))?;
    let missing_log_path = get_run_by_id_global(&temp.path, &missing_run_id)?
        .log_path
        .ok_or_else(|| test_error("expected missing run log path"))?;
    std::fs::remove_file(missing_log_path)?;
    connect_file(&temp.path)?.execute(
        "UPDATE task_runs SET log_path=?1 WHERE id=?2",
        params!["/etc/passwd", suspicious_run_id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert_eq!(report.missing_run_logs, 1);
    assert_eq!(report.suspicious_run_log_paths, 1);
    Ok(())
}

#[test]
fn doctor_reports_partially_initialized_database_without_bailing() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_reports_partially_initialized_database_without_bailing")?;
    connect_file(&temp.path)
        ?
        .execute(
            "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL DEFAULT '', applied_at INTEGER NOT NULL)",
            [],
        )
        ?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert_eq!(report.migration_version, None);
    assert_eq!(report.user_version, 0);
    Ok(())
}

#[test]
fn doctor_reports_missing_knowledge_substrate_tables_unhealthy() -> anyhow::Result<()> {
    for table in ["index_outbox", "derived_store_state"] {
        let temp = TempDb::new(&format!(
            "doctor_reports_missing_knowledge_substrate_tables_unhealthy_{table}"
        ))?;
        init_database(&temp.path, "tester")?;
        // Construct a deliberately corrupt schema. Projection v2 adds
        // foreign-key dependants to both foundation tables, so disable FK
        // enforcement only on this fixture connection before removing the
        // parent table that doctor must diagnose.
        connect_file(&temp.path)?
            .execute_batch(&format!("PRAGMA foreign_keys=OFF; DROP TABLE {table};"))?;

        let report = doctor_database(&temp.path)?;

        assert_eq!(report.migration_version, Some(30));
        assert_eq!(report.user_version, 30);
        assert!(!report.ok, "{table} missing should make doctor unhealthy");
    }
    Ok(())
}

#[test]
fn doctor_reports_missing_signal_ledger_tables_unhealthy() -> anyhow::Result<()> {
    for table in ["signal_observations", "signals"] {
        let temp = TempDb::new(&format!(
            "doctor_reports_missing_signal_ledger_tables_unhealthy_{table}"
        ))?;
        init_database(&temp.path, "tester")?;
        connect_file(&temp.path)?.execute_batch(&format!("DROP TABLE {table};"))?;

        let report = doctor_database(&temp.path)?;

        assert_eq!(report.migration_version, Some(30));
        assert_eq!(report.user_version, 30);
        assert!(!report.ok, "{table} missing should make doctor unhealthy");
        assert_eq!(report.consistency_errors, 1);
        assert!(report.consistency_issues.iter().any(|issue| {
            issue.code == "signal_ledger_missing_table"
                && issue.record_ids == vec![table.to_owned()]
        }));
    }
    Ok(())
}

#[test]
fn doctor_ontology_reports_missing_v12_tables_unhealthy() -> anyhow::Result<()> {
    for table in [
        "label_ontology_observations",
        "label_ontology_signals",
        "label_ontology_actions",
        "label_ontology_action_atom_effects",
        "label_ontology_action_signals",
    ] {
        let temp = TempDb::new(&format!(
            "doctor_ontology_reports_missing_v12_tables_unhealthy_{table}"
        ))?;
        init_database(&temp.path, "tester")?;
        connect_file(&temp.path)?.execute_batch(&format!("DROP TABLE {table};"))?;

        let report = doctor_database(&temp.path)?;

        assert_eq!(report.migration_version, Some(30));
        assert_eq!(report.user_version, 30);
        assert!(!report.ok, "{table} missing should make doctor unhealthy");
        assert_eq!(report.ontology_ledger_errors, 1);
        assert!(report.ontology_ledger_issues.iter().any(|issue| {
            issue.code == "label_ontology_missing_table"
                && issue.record_ids == vec![table.to_owned()]
        }));
    }
    Ok(())
}

#[test]
fn doctor_detects_generic_signal_supersede_cycle() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_detects_generic_signal_supersede_cycle")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "INSERT INTO signal_observations(id, board_id, actor, evidence_json, created_at) VALUES ('obs_cycle_a', ?1, 'tester', '{}', 1)",
        [&board_id],
    )?;
    conn.execute(
        "INSERT INTO signal_observations(id, board_id, actor, evidence_json, created_at) VALUES ('obs_cycle_b', ?1, 'tester', '{}', 1)",
        [&board_id],
    )?;
    conn.execute(
        "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, severity, status, superseded_by_signal_id, created_at, updated_at) VALUES ('sig_cycle_a', ?1, 'obs_cycle_a', 'test', 'a', 'a', 'info', 'superseded', 'sig_cycle_b', 1, 1)",
        [&board_id],
    )?;
    conn.execute(
        "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, severity, status, superseded_by_signal_id, created_at, updated_at) VALUES ('sig_cycle_b', ?1, 'obs_cycle_b', 'test', 'b', 'b', 'info', 'superseded', 'sig_cycle_a', 1, 1)",
        [&board_id],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    drop(conn);

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert!(report.consistency_errors >= 1);
    let cycle_issues: Vec<_> = report
        .consistency_issues
        .iter()
        .filter(|issue| issue.code == "signal_supersede_cycle")
        .collect();
    assert_eq!(cycle_issues.len(), 1);
    assert!(
        cycle_issues[0]
            .record_ids
            .contains(&"sig_cycle_a".to_owned())
    );
    assert!(
        cycle_issues[0]
            .record_ids
            .contains(&"sig_cycle_b".to_owned())
    );
    Ok(())
}

#[test]
fn doctor_ontology_detects_cross_board_signal_rows() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_ontology_detects_cross_board_signal_rows")?;
    let fixture = seed_doctor_ontology_ledger(&temp)?;
    let conn = connect_file(&temp.path)?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys=OFF;
        DROP TRIGGER IF EXISTS trg_label_ontology_signals_board_insert;
        DROP TRIGGER IF EXISTS trg_label_ontology_signals_board_update;
        ",
    )?;
    conn.execute(
        "UPDATE label_ontology_signals SET board_id=?1 WHERE id=?2",
        params![fixture.other_board_id, fixture.signal_id],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert!(report.ontology_ledger_errors >= 1);
    assert!(report.ontology_ledger_issues.iter().any(|issue| {
        issue.code == "label_ontology_signal_observation_board_mismatch"
            && issue.record_ids.contains(&fixture.signal_id)
            && issue.record_ids.contains(&fixture.observation_id)
    }));
    Ok(())
}

#[test]
fn doctor_ontology_detects_orphan_action_signal_link() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_ontology_detects_orphan_action_signal_link")?;
    let fixture = seed_doctor_ontology_ledger(&temp)?;
    let conn = connect_file(&temp.path)?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "UPDATE label_ontology_action_signals SET action_id='loa_missing' WHERE action_id=?1",
        [&fixture.action_id],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert!(report.ontology_ledger_errors >= 1);
    assert!(report.ontology_ledger_issues.iter().any(|issue| {
        issue.code == "label_ontology_action_signal_orphan"
            && issue.record_ids.contains(&"loa_missing".to_owned())
            && issue.record_ids.contains(&fixture.signal_id)
    }));
    Ok(())
}

#[test]
fn doctor_ontology_detects_supersede_cycle() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_ontology_detects_supersede_cycle")?;
    let fixture = seed_doctor_ontology_ledger(&temp)?;
    let second_signal_id = "los_doctor_b";
    let conn = connect_file(&temp.path)?;
    insert_doctor_ontology_signal(
        &conn,
        second_signal_id,
        &fixture.observation_id,
        &fixture.board_id,
        "doctor-cycle-b",
        None,
    )?;
    conn.execute(
        "UPDATE label_ontology_signals \
         SET status='superseded', superseded_by_signal_id=?1, updated_at=2 \
         WHERE id=?2",
        params![second_signal_id, fixture.signal_id],
    )?;
    conn.execute(
        "UPDATE label_ontology_signals \
         SET status='superseded', superseded_by_signal_id=?1, updated_at=2 \
         WHERE id=?2",
        params![fixture.signal_id, second_signal_id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert!(report.ontology_ledger_errors >= 1);
    assert!(report.ontology_ledger_issues.iter().any(|issue| {
        issue.code == "label_ontology_signal_supersede_cycle"
            && issue.record_ids.contains(&fixture.signal_id)
            && issue.record_ids.contains(&second_signal_id.to_owned())
    }));
    Ok(())
}

#[test]
fn sqlite_rejects_cross_board_foundation_relationship_rows() -> anyhow::Result<()> {
    let temp = TempDb::new("sqlite_rejects_cross_board_key_relationship_rows")?;
    let fixture = seed_foundation_relationship_fixture(&temp)?;
    let conn = connect_file(&temp.path)?;

    for (name, result, expected) in [
        (
            "task_labels",
            conn.execute(
                "UPDATE task_labels SET board_id=?1 WHERE task_id=?2 AND label_id=?3",
                params![fixture.other_board_id, fixture.task_id, fixture.label_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "task_dependencies",
            conn.execute(
                "UPDATE task_dependencies SET board_id=?1 WHERE parent_task_id=?2 AND child_task_id=?3",
                params![
                    fixture.other_board_id,
                    fixture.parent_task_id,
                    fixture.child_task_id
                ],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "task_runs",
            conn.execute(
                "UPDATE task_runs SET board_id=?1 WHERE id='r_cross_board'",
                [&fixture.other_board_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "task_comments_update",
            conn.execute(
                "UPDATE task_comments SET board_id=?1 WHERE task_id=?2",
                params![fixture.other_board_id, fixture.task_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "task_comments_insert",
            conn.execute(
                "INSERT INTO task_comments(id, board_id, task_id, author, author_type, body, kind, metadata_json, created_at) \
                 VALUES ('c_cross_board_insert', ?1, ?2, 'tester', 'user', 'bad comment', 'note', '{}', 1)",
                params![fixture.other_board_id, fixture.task_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "task_attachments_update",
            conn.execute(
                "UPDATE task_attachments SET board_id=?1 WHERE id='a_cross_board'",
                [&fixture.other_board_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "task_attachments_insert",
            conn.execute(
                "INSERT INTO task_attachments(id, board_id, task_id, filename, rel_path, size_bytes, created_by, created_at) \
                 VALUES ('a_cross_board_insert', ?1, ?2, 'bad.txt', 'attachments/bad.txt', 0, 'tester', 1)",
                params![fixture.other_board_id, fixture.task_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "signals_update_observation",
            conn.execute(
                "UPDATE signals SET board_id=?1 WHERE id='sig_cross_board'",
                [&fixture.other_board_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "signals_insert_observation",
            conn.execute(
                "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, severity, status, created_at, updated_at) \
                 VALUES ('sig_cross_board_insert', ?1, 'obs_cross_board_a', 'agent_cli_friction', 'bad signal', 'bad signal', 'info', 'open', 1, 1)",
                [&fixture.other_board_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "task_events_update_task",
            conn.execute(
                "UPDATE task_events SET board_id=?1 WHERE event_id='e_cross_board'",
                [&fixture.other_board_id],
            ),
            "task_events.board_id must match task_id board_id",
        ),
        (
            "task_events_insert_task",
            conn.execute(
                "INSERT INTO task_events(event_id, board_id, task_id, kind, payload_json, created_at) \
                 VALUES ('e_cross_board_insert_task', ?1, ?2, 'test.cross_board', '{}', 1)",
                params![fixture.other_board_id, fixture.task_id],
            ),
            "task_events.board_id must match task_id board_id",
        ),
        (
            "task_events_insert_run",
            conn.execute(
                "INSERT INTO task_events(event_id, board_id, run_id, kind, payload_json, created_at) \
                 VALUES ('e_cross_board_insert_run', ?1, 'r_cross_board', 'test.cross_board', '{}', 1)",
                [&fixture.other_board_id],
            ),
            "task_events.board_id must match run_id board_id",
        ),
    ] {
        let error = result_err(result)?;
        assert!(
            error.to_string().contains(expected),
            "{name}: {error}"
        );
    }

    let report = doctor_database(&temp.path)?;
    assert!(report.ok, "{report:#?}");
    Ok(())
}

#[test]
fn task_event_nullable_refs_are_cleared_when_task_and_run_are_deleted() -> anyhow::Result<()> {
    let temp = TempDb::new("task_event_nullable_refs_are_cleared_when_task_and_run_are_deleted")?;
    let fixture = seed_foundation_relationship_fixture(&temp)?;
    let conn = connect_file(&temp.path)?;
    let before_event: (String, Option<String>, Option<String>) = conn.query_row(
        "SELECT board_id, task_id, run_id FROM task_events WHERE event_id='e_cross_board'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(before_event.1.as_deref(), Some(fixture.task_id.as_str()));
    assert_eq!(before_event.2.as_deref(), Some("r_cross_board"));

    conn.execute("DELETE FROM tasks WHERE id=?1", [&fixture.task_id])?;

    let after_event: (String, Option<String>, Option<String>) = conn.query_row(
        "SELECT board_id, task_id, run_id FROM task_events WHERE event_id='e_cross_board'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(after_event.0, before_event.0);
    assert_eq!(after_event.1, None);
    assert_eq!(after_event.2, None);
    let run_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM task_runs WHERE id='r_cross_board'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(run_count, 0);
    Ok(())
}

#[test]
fn doctor_detects_cross_board_history_relationship_rows() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_detects_cross_board_history_relationship_rows")?;
    let fixture = seed_foundation_relationship_fixture(&temp)?;
    let conn = connect_file(&temp.path)?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys=OFF;
        DROP TRIGGER IF EXISTS trg_task_events_board_insert;
        DROP TRIGGER IF EXISTS trg_task_events_board_update;
        ",
    )?;
    conn.execute(
        "UPDATE task_comments SET board_id=?1 WHERE task_id=?2",
        params![fixture.other_board_id, fixture.task_id],
    )?;
    conn.execute(
        "UPDATE task_events SET board_id=?1 WHERE event_id='e_cross_board'",
        [&fixture.other_board_id],
    )?;
    conn.execute(
        "UPDATE task_attachments SET board_id=?1 WHERE id='a_cross_board'",
        [&fixture.other_board_id],
    )?;
    conn.execute(
        "UPDATE signal_observations SET board_id=?1 WHERE id='obs_cross_board_a'",
        [&fixture.other_board_id],
    )?;
    conn.execute(
        "UPDATE signals SET board_id=?1 WHERE id='sig_cross_board_replacement'",
        [&fixture.other_board_id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert!(report.consistency_errors >= 5);
    for code in [
        "task_comment_task_board_mismatch",
        "task_event_task_board_mismatch",
        "task_attachment_task_board_mismatch",
        "signal_observation_task_board_mismatch",
        "signal_observation_board_mismatch",
    ] {
        assert!(
            report
                .consistency_issues
                .iter()
                .any(|issue| issue.code == code
                    && issue.message.contains("table=")
                    && issue.message.contains("row=")
                    && issue.message.contains("row_board=")
                    && issue.message.contains("referenced_board=")),
            "missing consistency issue {code}: {:#?}",
            report.consistency_issues
        );
    }
    Ok(())
}

#[test]
fn doctor_reports_sqlite_foreign_key_check_violations() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_reports_sqlite_foreign_key_check_violations")?;
    let task = seed_label_semantic_proposal_fk_fixture(&temp)?;
    let conn = connect_file(&temp.path)?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "UPDATE label_semantic_proposals
         SET top1_existing_label_id='l_missing_fk_parent'
         WHERE id='lp_fk_check'",
        [],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    let fk_errors: i64 =
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    assert_eq!(fk_errors, 1);
    drop(conn);

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok, "{report:#?}");
    assert!(report.consistency_errors >= 1, "{report:#?}");
    assert!(
        report.consistency_issues.iter().any(|issue| {
            issue.code == "sqlite_foreign_key_violation"
                && issue.severity == "error"
                && issue.message.contains("table=label_semantic_proposals")
                && issue.message.contains("rowid=")
                && issue.message.contains("parent=labels")
                && issue.message.contains("fk_index=")
                && issue
                    .record_ids
                    .iter()
                    .any(|record_id| record_id.starts_with("label_semantic_proposals:"))
                && issue
                    .record_ids
                    .iter()
                    .any(|record_id| record_id == "labels")
        }),
        "task={} issues={:#?}",
        task.id,
        report.consistency_issues
    );
    Ok(())
}

#[test]
fn jsonl_import_rejects_cross_board_foundation_relationship_rows() -> anyhow::Result<()> {
    let source = TempDb::new("jsonl_import_rejects_cross_board_foundation_source")?;
    let fixture = seed_foundation_relationship_fixture(&source)?;
    let default_export = source.dir.join("default.jsonl");
    let other_export = source.dir.join("other.jsonl");
    export_jsonl(&source.path, "default", &default_export)?;
    export_jsonl(&source.path, "other", &other_export)?;
    let default_jsonl = std::fs::read_to_string(&default_export)?;
    let other_jsonl = std::fs::read_to_string(&other_export)?;

    for case in FOUNDATION_RELATIONSHIP_IMPORT_CASES {
        let invalid_export = source
            .dir
            .join(format!("cross-board-{}.jsonl", case.record_type));
        let invalid_default = replace_jsonl_record_board_id(
            &default_jsonl,
            case.record_type,
            &fixture.other_board_id,
        )?;
        std::fs::write(&invalid_export, format!("{other_jsonl}{invalid_default}"))?;

        let target = TempDb::new(&format!(
            "jsonl_import_rejects_cross_board_foundation_target_{}",
            case.record_type
        ))?;
        init_database(&target.path, "tester")?;
        let sentinel = create_task(
            &target.path,
            "default",
            "tester",
            CreateTask::ready(format!("target sentinel {}", case.record_type)),
        )?;
        let before_counts = foundation_relationship_table_counts(&target.path)?;

        let error = result_err(import_jsonl(&target.path, &invalid_export, true))?;

        let message = error.to_string();
        match case.rejection {
            FoundationRelationshipImportRejection::Doctor(expected_table) => {
                assert!(
                    message.contains("imported data failed doctor checks"),
                    "{}: {message}",
                    case.record_type
                );
                assert!(
                    message.contains(expected_table),
                    "{}: {message}",
                    case.record_type
                );
                assert!(
                    message.contains("table="),
                    "{}: {message}",
                    case.record_type
                );
            }
            FoundationRelationshipImportRejection::Trigger(expected) => {
                assert!(
                    message.contains(expected),
                    "{}: {message}",
                    case.record_type
                );
            }
        }

        let after_counts = foundation_relationship_table_counts(&target.path)?;
        assert_eq!(
            after_counts, before_counts,
            "failed {} import must roll back the replace transaction",
            case.record_type
        );
        let target_tasks = list_tasks(&target.path, "default", &[], true)?;
        assert_eq!(target_tasks.len(), 1);
        assert_eq!(target_tasks[0].id, sentinel.id);
    }
    Ok(())
}

#[test]
fn jsonl_import_foreign_key_check_failure_rolls_back_replace() -> anyhow::Result<()> {
    let source = TempDb::new("jsonl_import_foreign_key_check_failure_source")?;
    seed_label_semantic_proposal_fk_fixture(&source)?;
    let export_path = source.dir.join("fk-source.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let source_jsonl = std::fs::read_to_string(&export_path)?;
    let invalid_jsonl = set_jsonl_record_field(
        &source_jsonl,
        "label_semantic_proposal",
        "top1_existing_label_id",
        serde_json::Value::String("l_missing_fk_parent".to_owned()),
    )?;
    let invalid_path = source.dir.join("fk-invalid.jsonl");
    std::fs::write(&invalid_path, invalid_jsonl)?;

    let target = TempDb::new("jsonl_import_foreign_key_check_failure_target")?;
    init_database(&target.path, "tester")?;
    let sentinel = create_task(
        &target.path,
        "default",
        "tester",
        CreateTask::ready("target sentinel fk import"),
    )?;
    let before_tasks = list_tasks(&target.path, "default", &[], true)?;

    let error = result_err(import_jsonl(&target.path, &invalid_path, true))?;

    let message = error.to_string();
    assert!(
        message.contains("imported data failed doctor checks"),
        "{message}"
    );
    assert!(
        message.contains("foreign key violation: table=label_semantic_proposals"),
        "{message}"
    );
    assert!(message.contains("parent=labels"), "{message}");
    let after_tasks = list_tasks(&target.path, "default", &[], true)?;
    assert_eq!(after_tasks, before_tasks);
    assert_eq!(after_tasks[0].id, sentinel.id);
    Ok(())
}

#[test]
fn jsonl_import_accepts_legal_foundation_relationship_round_trip() -> anyhow::Result<()> {
    let source = TempDb::new("jsonl_import_accepts_legal_foundation_relationship_source")?;
    seed_foundation_relationship_fixture(&source)?;
    let default_export = source.dir.join("default-legal.jsonl");
    export_jsonl(&source.path, "default", &default_export)?;

    let target = TempDb::new("jsonl_import_accepts_legal_foundation_relationship_target")?;
    init_database(&target.path, "tester")?;

    import_jsonl(&target.path, &default_export, true)?;
    let report = doctor_database(&target.path)?;

    assert!(report.ok, "{report:#?}");
    assert_eq!(report.consistency_errors, 0);
    assert_eq!(list_tasks(&target.path, "default", &[], true)?.len(), 3);
    assert_eq!(list_runs(&target.path, "default", None)?.len(), 1);
    assert!(!list_events(&target.path, "default", None)?.is_empty());
    assert_eq!(list_labels(&target.path, "default")?.len(), 1);

    let conn = connect_file(&target.path)?;
    assert_eq!(table_count(&conn, "task_dependencies")?, 1);
    assert_eq!(table_count(&conn, "task_comments")?, 1);
    assert_eq!(table_count(&conn, "task_labels")?, 1);
    assert_eq!(table_count(&conn, "task_attachments")?, 1);
    assert_eq!(table_count(&conn, "signal_observations")?, 2);
    assert_eq!(table_count(&conn, "signals")?, 2);
    let superseded_by: Option<String> = conn.query_row(
        "SELECT superseded_by_signal_id FROM signals WHERE id='sig_cross_board'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        superseded_by.as_deref(),
        Some("sig_cross_board_replacement")
    );
    Ok(())
}

#[test]
fn jsonl_import_reset_leaves_no_orphan_projection_corpus_binding() -> anyhow::Result<()> {
    let source = TempDb::new("jsonl_import_projection_corpus_source")?;
    init_database(&source.path, "tester")?;
    create_task(
        &source.path,
        "default",
        "tester",
        CreateTask::ready("portable canonical task"),
    )?;
    let export_path = source.dir.join("projection-corpus-reset.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;

    let target = TempDb::new("jsonl_import_projection_corpus_target")?;
    init_database(&target.path, "tester")?;
    let conn = connect_file(&target.path)?;
    conn.execute_batch(
        "UPDATE projection_store_state
         SET control_plane='v2',
             active_generation='gen_import_active',
             active_fingerprint='sha256:active',
             active_fence_epoch=3,
             active_snapshot_cursor=0,
             active_provider='fake-lance',
             active_provider_fingerprint='fake-provider-v1',
             active_canonical_count=0,
             active_canonical_digest='fnv64:active-canonical',
             active_delivery_count=0,
             active_delivery_digest='fnv64:active-delivery',
             active_corpus_schema='task-chunks-v2',
             active_corpus_fingerprint='corpus:active',
             active_embedding_model='fake-embedding-v1',
             active_embedding_dimensions=3,
             previous_generation='gen_import_previous',
             previous_fingerprint='sha256:previous',
             previous_fence_epoch=2,
             previous_snapshot_cursor=0,
             previous_provider='fake-lance',
             previous_provider_fingerprint='fake-provider-v1',
             previous_canonical_count=0,
             previous_canonical_digest='fnv64:previous-canonical',
             previous_delivery_count=0,
             previous_delivery_digest='fnv64:previous-delivery',
             previous_corpus_schema='task-chunks-v2',
             previous_corpus_fingerprint='corpus:previous',
             previous_embedding_model='fake-embedding-v1',
             previous_embedding_dimensions=3,
             building_generation='gen_import_building',
             building_fingerprint='sha256:building',
             building_fence_epoch=4,
             building_provider='fake-lance',
             building_provider_fingerprint='fake-provider-v1',
             building_canonical_count=0,
             building_canonical_digest='fnv64:building-canonical',
             building_delivery_count=0,
             building_delivery_digest='fnv64:building-delivery',
             building_phase='prepared',
             building_corpus_schema='task-chunks-v2',
             building_corpus_fingerprint='corpus:building',
             building_embedding_model='fake-embedding-v1',
             building_embedding_dimensions=3
         WHERE store_name='lancedb_chunks';",
    )?;
    drop(conn);

    import_jsonl(&target.path, &export_path, true)?;

    let conn = connect_file(&target.path)?;
    let state: (String, Option<String>, Option<String>, Option<String>, i64) = conn.query_row(
        "SELECT control_plane,active_generation,previous_generation,building_generation,
                (active_corpus_schema IS NULL
                 AND active_corpus_fingerprint IS NULL
                 AND active_embedding_model IS NULL
                 AND active_embedding_dimensions IS NULL
                 AND previous_corpus_schema IS NULL
                 AND previous_corpus_fingerprint IS NULL
                 AND previous_embedding_model IS NULL
                 AND previous_embedding_dimensions IS NULL
                 AND building_corpus_schema IS NULL
                 AND building_corpus_fingerprint IS NULL
                 AND building_embedding_model IS NULL
                 AND building_embedding_dimensions IS NULL)
         FROM projection_store_state WHERE store_name='lancedb_chunks'",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        },
    )?;
    assert_eq!(state, ("legacy".to_owned(), None, None, None, 1));
    let report = doctor_database(&target.path)?;
    assert!(
        !report
            .consistency_issues
            .iter()
            .any(|issue| { issue.code == "projection_corpus_binding_invalid" })
    );
    Ok(())
}

struct FoundationRelationshipFixture {
    other_board_id: String,
    task_id: String,
    parent_task_id: String,
    child_task_id: String,
    label_id: String,
}

struct FoundationRelationshipImportCase {
    record_type: &'static str,
    rejection: FoundationRelationshipImportRejection,
}

#[derive(Clone, Copy)]
enum FoundationRelationshipImportRejection {
    Doctor(&'static str),
    Trigger(&'static str),
}

const FOUNDATION_RELATIONSHIP_IMPORT_CASES: &[FoundationRelationshipImportCase] = &[
    FoundationRelationshipImportCase {
        record_type: "task_label",
        rejection: FoundationRelationshipImportRejection::Doctor("task_labels"),
    },
    FoundationRelationshipImportCase {
        record_type: "dependency",
        rejection: FoundationRelationshipImportRejection::Doctor("task_dependencies"),
    },
    FoundationRelationshipImportCase {
        record_type: "run",
        rejection: FoundationRelationshipImportRejection::Trigger(
            "task_events.board_id must match run_id board_id",
        ),
    },
    FoundationRelationshipImportCase {
        record_type: "comment",
        rejection: FoundationRelationshipImportRejection::Doctor("task_comments"),
    },
    FoundationRelationshipImportCase {
        record_type: "event",
        rejection: FoundationRelationshipImportRejection::Trigger(
            "task_events.board_id must match task_id board_id",
        ),
    },
    FoundationRelationshipImportCase {
        record_type: "attachment",
        rejection: FoundationRelationshipImportRejection::Doctor("task_attachments"),
    },
    FoundationRelationshipImportCase {
        record_type: "signal_observation",
        rejection: FoundationRelationshipImportRejection::Doctor("signal_observations"),
    },
    FoundationRelationshipImportCase {
        record_type: "signal",
        rejection: FoundationRelationshipImportRejection::Doctor("signals"),
    },
];

fn seed_foundation_relationship_fixture(
    temp: &TempDb,
) -> anyhow::Result<FoundationRelationshipFixture> {
    init_database(&temp.path, "tester")?;
    let other_board = create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".to_owned(),
            name: "Other".to_owned(),
            description: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("relationship row source task"),
    )?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("relationship parent task"),
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("relationship child task"),
    )?;
    let label = create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "relationship-label".to_owned(),
            color: None,
        },
    )?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    let comment = create_comment(&temp.path, &task.id, "tester", "relationship note", None)?;

    let conn = connect_file(&temp.path)?;
    conn.execute(
        "INSERT INTO task_labels(board_id, task_id, label_id, created_at) VALUES (?1, ?2, ?3, 1)",
        params![task.board_id, task.id, label.id],
    )?;
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, claim_token, claim_owner, claim_expires_at, started_at, metadata_json) \
         VALUES ('r_cross_board', ?1, ?2, 'failed', 'token', 'tester', 1, 1, '{}')",
        params![task.board_id, task.id],
    )?;
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, payload_json, created_at) \
         VALUES ('e_cross_board', ?1, ?2, 'r_cross_board', 'test.cross_board', '{}', 1)",
        params![task.board_id, task.id],
    )?;
    conn.execute(
        "INSERT INTO task_attachments(id, board_id, task_id, filename, rel_path, size_bytes, created_by, created_at) \
         VALUES ('a_cross_board', ?1, ?2, 'artifact.txt', 'attachments/artifact.txt', 0, 'tester', 1)",
        params![task.board_id, task.id],
    )?;
    conn.execute(
        "INSERT INTO signal_observations(id, board_id, task_id, task_ref_snapshot, run_id, comment_id, actor, agent_type, source, evidence_json, created_at) \
         VALUES ('obs_cross_board_a', ?1, ?2, ?3, 'r_cross_board', ?4, 'tester', 'codex', 'test', '{}', 1)",
        params![task.board_id, task.id, task.task_ref, comment.id],
    )?;
    conn.execute(
        "INSERT INTO signal_observations(id, board_id, task_id, task_ref_snapshot, run_id, comment_id, actor, agent_type, source, evidence_json, created_at) \
         VALUES ('obs_cross_board_b', ?1, ?2, ?3, 'r_cross_board', ?4, 'tester', 'codex', 'test', '{}', 2)",
        params![task.board_id, task.id, task.task_ref, comment.id],
    )?;
    conn.execute(
        "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, severity, status, created_at, updated_at) \
         VALUES ('sig_cross_board_replacement', ?1, 'obs_cross_board_b', 'agent_cli_friction', 'replacement signal', 'replacement signal', 'info', 'open', 2, 2)",
        [&task.board_id],
    )?;
    conn.execute(
        "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, severity, status, superseded_by_signal_id, created_at, updated_at) \
         VALUES ('sig_cross_board', ?1, 'obs_cross_board_a', 'agent_cli_friction', 'cross board signal', 'cross board signal', 'info', 'superseded', 'sig_cross_board_replacement', 1, 2)",
        [&task.board_id],
    )?;

    Ok(FoundationRelationshipFixture {
        other_board_id: other_board.id,
        task_id: task.id,
        parent_task_id: parent.id,
        child_task_id: child.id,
        label_id: label.id,
    })
}

fn foundation_relationship_table_counts(path: &Path) -> anyhow::Result<Vec<(&'static str, i64)>> {
    let conn = connect_file(path)?;
    [
        "boards",
        "board_columns",
        "tasks",
        "task_dependencies",
        "task_runs",
        "task_comments",
        "task_events",
        "task_attachments",
        "signal_observations",
        "signals",
        "labels",
        "task_labels",
    ]
    .into_iter()
    .map(|table| Ok((table, table_count(&conn, table)?)))
    .collect()
}

fn table_count(conn: &Connection, table: &str) -> anyhow::Result<i64> {
    Ok(
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })?,
    )
}

#[test]
fn doctor_reports_executable_status_invariant_violations() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_reports_executable_status_invariant_violations")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("unfinished parent"),
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid ready child"),
    )?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &child.id)?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    let missing_spec = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid missing spec"),
    )?;
    let future_scheduled = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("invalid future schedule"),
    )?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &missing_spec.id)?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &future_scheduled.id)?;
    let conn = connect_file(&temp.path)?;
    conn.execute("UPDATE tasks SET status='ready' WHERE id=?1", [&child.id])?;
    conn.execute(
        "UPDATE tasks SET status='ready', description=NULL WHERE id=?1",
        [&missing_spec.id],
    )?;
    conn.execute(
        "UPDATE tasks SET status='ready', scheduled_at=?1 WHERE id=?2",
        params![4_102_444_800_000_i64, future_scheduled.id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert_eq!(report.executable_dependency_violations, 1);
    assert_eq!(report.executable_spec_violations, 1);
    assert_eq!(report.executable_schedule_violations, 1);
    Ok(())
}

#[test]
fn doctor_accepts_archived_parent_for_active_child_dependency() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_accepts_archived_parent_for_active_child_dependency")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archived parent"),
    )?;
    archive_task(&temp.path, "default", "tester", &parent.id, false)?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("active child"),
    )?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;

    let report = doctor_database(&temp.path)?;

    assert_eq!(report.archived_dependency_edges, 0);
    assert_eq!(report.executable_dependency_violations, 0);
    assert!(report.ok);
    Ok(())
}

#[test]
fn doctor_counts_each_dependency_cycle_once() -> anyhow::Result<()> {
    let temp = TempDb::new("doctor_counts_each_dependency_cycle_once")?;
    init_database(&temp.path, "tester")?;
    let a = create_task(&temp.path, "default", "tester", CreateTask::ready("a"))?;
    let b = create_task(&temp.path, "default", "tester", CreateTask::ready("b"))?;
    let c = create_task(&temp.path, "default", "tester", CreateTask::ready("c"))?;
    add_dependency(&temp.path, "default", "tester", &a.id, &b.id)?;
    add_dependency(&temp.path, "default", "tester", &b.id, &c.id)?;
    connect_file(&temp.path)?.execute(
        "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) \
             VALUES (?1, ?2, ?3, 1)",
        params![a.board_id, c.id, a.id],
    )?;

    let report = doctor_database(&temp.path)?;

    assert!(!report.ok);
    assert_eq!(report.dependency_cycles, 1);
    Ok(())
}

struct DoctorOntologyLedgerFixture {
    board_id: String,
    other_board_id: String,
    observation_id: String,
    signal_id: String,
    action_id: String,
}

fn seed_doctor_ontology_ledger(temp: &TempDb) -> anyhow::Result<DoctorOntologyLedgerFixture> {
    init_database(&temp.path, "tester")?;
    let other_board = create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".to_owned(),
            name: "Other".to_owned(),
            description: None,
        },
    )?;
    let label = create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "doctor-label".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("doctor ontology ledger source task"),
    )?;
    let conn = connect_file(&temp.path)?;
    let observation_id = "lor_doctor".to_owned();
    let signal_id = "los_doctor".to_owned();
    let action_id = "loa_doctor".to_owned();
    conn.execute(
        "INSERT INTO label_ontology_observations(\
         id, board_id, task_id, task_ref_snapshot, task_snapshot_json, suggest_input_hash, \
         agent_candidates_json, suggestion_snapshot_json, final_decision_json, diagnostics_json, \
         capture_fingerprint, created_by, created_by_type, created_at) \
         VALUES (?1, ?2, ?3, ?4, '{}', 'abcdef0123456789', '[]', '{}', '{}', '[]', \
         'doctor-fixture', 'tester', 'user', 1)",
        params![observation_id, task.board_id, task.id, task.task_ref],
    )?;
    insert_doctor_ontology_signal(
        &conn,
        &signal_id,
        &observation_id,
        &task.board_id,
        "doctor-signal",
        Some(&label.id),
    )?;
    conn.execute(
        "INSERT INTO label_ontology_actions(\
         id, board_id, action_type, reason, change_json, validation_status, validation_json, \
         created_by, created_by_type, created_at) \
         VALUES (?1, ?2, 'confirm', 'confirmed by doctor fixture', '{}', 'not_required', '{}', \
         'tester', 'user', 1)",
        params![action_id, task.board_id],
    )?;
    conn.execute(
        "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at) \
         VALUES (?1, ?2, ?3, 1)",
        params![task.board_id, action_id, signal_id],
    )?;
    Ok(DoctorOntologyLedgerFixture {
        board_id: task.board_id,
        other_board_id: other_board.id,
        observation_id,
        signal_id,
        action_id,
    })
}

fn insert_doctor_ontology_signal(
    conn: &Connection,
    signal_id: &str,
    observation_id: &str,
    board_id: &str,
    signal_key: &str,
    target_label_id: Option<&str>,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO label_ontology_signals(\
         id, observation_id, board_id, kind, status, target_label_id, related_labels_json, \
         proposed_action, proposal_json, agent_selected, final_selected, rationale, signal_key, \
         created_at, updated_at) \
         VALUES (?1, ?2, ?3, 'false_negative', 'open', ?4, '[]', 'add_positive_atom', '{}', 1, 1, \
         'doctor fixture signal', ?5, 1, 1)",
        params![
            signal_id,
            observation_id,
            board_id,
            target_label_id,
            signal_key
        ],
    )?;
    Ok(())
}

fn seed_label_semantic_proposal_fk_fixture(temp: &TempDb) -> anyhow::Result<TaskRecord> {
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("label semantic proposal fk fixture"),
    )?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "INSERT INTO label_semantic_proposals(
         id, board_id, task_id, status, name, applies_when, excludes_when,
         positive_examples, negative_examples, heuristic_coverage,
         heuristic_coverage_cosine, heuristic_residual_norm, diagnostics_json,
         created_by, created_at, updated_at)
         VALUES ('lp_fk_check', ?1, ?2, 'proposed', 'fk-check',
         '[]', '[]', '[]', '[]', 0.1, 0.1, 0.9, '[]', 'tester', 1, 1)",
        params![task.board_id, task.id],
    )?;
    Ok(task)
}

fn replace_jsonl_record_board_id(
    input: &str,
    record_type: &str,
    board_id: &str,
) -> anyhow::Result<String> {
    let mut changed = false;
    let mut output = String::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut value: serde_json::Value = serde_json::from_str(line)?;
        if value["type"] == record_type {
            value["data"]["board_id"] = serde_json::Value::String(board_id.to_owned());
            changed = true;
        }
        output.push_str(&value.to_string());
        output.push('\n');
    }
    assert!(changed, "expected {record_type} record in JSONL");
    Ok(output)
}

fn set_jsonl_record_field(
    input: &str,
    record_type: &str,
    field: &str,
    field_value: serde_json::Value,
) -> anyhow::Result<String> {
    let mut changed = false;
    let mut output = String::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut value: serde_json::Value = serde_json::from_str(line)?;
        if value["type"] == record_type {
            value["data"][field] = field_value.clone();
            changed = true;
        }
        output.push_str(&value.to_string());
        output.push('\n');
    }
    assert!(changed, "expected {record_type} record in JSONL");
    Ok(output)
}

#[test]
fn database_replace_is_rejected_while_runtime_lock_is_held() -> anyhow::Result<()> {
    let temp = TempDb::new("database_replace_is_rejected_while_runtime_lock_is_held")?;
    init_database(&temp.path, "tester")?;
    let _runtime_guard = lifecycle::begin_database_runtime(&temp.path)?;

    let err = result_err(lifecycle::begin_database_replace(&temp.path))?;

    assert!(
        err.to_string().contains("running")
            || err.to_string().contains("runtime")
            || err.to_string().contains("serve/dispatch"),
        "err: {err}"
    );
    Ok(())
}
