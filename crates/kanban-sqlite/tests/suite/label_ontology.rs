use crate::common::*;

use serde_json::json;

#[test]
fn label_ontology_migration_creates_ledger_tables_and_json_constraints() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_migration_creates_ledger_tables_and_json_constraints")?;

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 23);
    for table in [
        "label_ontology_observations",
        "label_ontology_signals",
        "label_ontology_actions",
        "label_ontology_action_atom_effects",
        "label_ontology_action_signals",
    ] {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;
        assert_eq!(count, 1, "missing table {table}");
    }
    let unique_create_proposal_index: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_label_ontology_actions_unique_create_proposal'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(unique_create_proposal_index, 1);

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("ontology JSON constraint target"),
    )?;
    let board_id = task.board_id.clone();
    let err = result_err(conn.execute(
        "INSERT INTO label_ontology_observations(\
         id, board_id, task_id, task_ref_snapshot, task_snapshot_json, agent_candidates_json, \
         suggestion_snapshot_json, final_decision_json, diagnostics_json, capture_fingerprint, \
         created_by, created_by_type, created_at) \
         VALUES ('lor_bad', ?1, ?2, ?3, '{bad', '[]', '{}', '{}', '[]', 'bad-json', 'tester', 'user', 1)",
        params![board_id, task.id, task.task_ref],
    ))?;
    assert!(err.to_string().contains("CHECK"));

    Ok(())
}

#[test]
fn label_ontology_action_atom_effects_use_board_scoped_action_fk() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_action_atom_effects_use_board_scoped_action_fk")?;
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
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO label_ontology_actions(
         id, board_id, action_type, reason, change_json, validation_status, validation_json,
         created_by, created_by_type, created_at)
         VALUES ('loa_effect_fk', ?1, 'update_semantics', 'effect fk test', '{}',
         'not_required', '{}', 'tester', 'user', 1)",
        [&board_id],
    )?;
    conn.execute(
        "INSERT INTO label_ontology_action_atom_effects(
         board_id, action_id, label_id_snapshot, atom_id_snapshot, atom_content_hash,
         polarity, kind, text, effect, created_at)
         VALUES (?1, 'loa_effect_fk', 'l_snapshot', 'la_snapshot', 'hash-added',
         'positive', 'applies_when', 'same board effect', 'added', 2)",
        [&board_id],
    )?;

    let error = result_err(conn.execute(
        "INSERT INTO label_ontology_action_atom_effects(
         board_id, action_id, label_id_snapshot, atom_id_snapshot, atom_content_hash,
         polarity, kind, text, effect, created_at)
         VALUES (?1, 'loa_effect_fk', 'l_snapshot', 'la_snapshot2', 'hash-other-board',
         'positive', 'applies_when', 'other board effect', 'added', 3)",
        [&other_board.id],
    ))?;
    assert!(
        error.to_string().contains("FOREIGN KEY constraint failed"),
        "error: {error}"
    );
    let fk_error_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    assert_eq!(fk_error_count, 0);
    Ok(())
}

#[test]
fn label_ontology_schema_rejects_cross_board_ontology_links() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_schema_rejects_cross_board_ontology_links")?;
    let fixture = seed_ontology_link_schema_fixture(&temp)?;
    let conn = connect_file(&temp.path)?;

    for (name, result, expected) in [
        (
            "proposal_task_insert",
            conn.execute(
                "INSERT INTO label_semantic_proposals(
                 id, board_id, task_id, status, name, applies_when, excludes_when,
                 positive_examples, negative_examples, heuristic_coverage,
                 heuristic_coverage_cosine, heuristic_residual_norm, diagnostics_json,
                 created_by, created_at, updated_at)
                 VALUES ('lp_schema_bad_task', ?1, ?2, 'proposed', 'bad-task',
                 '[]', '[]', '[]', '[]', 0.1, 0.1, 0.9, '[]', 'tester', 1, 1)",
                params![fixture.other_board_id, fixture.task_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "proposal_resolved_label_insert",
            conn.execute(
                "INSERT INTO label_semantic_proposals(
                 id, board_id, task_id, status, name, resolved_label_id,
                 applies_when, excludes_when, positive_examples, negative_examples,
                 heuristic_coverage, heuristic_coverage_cosine, heuristic_residual_norm,
                 diagnostics_json, created_by, created_at, updated_at)
                 VALUES ('lp_schema_bad_resolved', ?1, ?2, 'proposed', 'bad-resolved',
                 ?3, '[]', '[]', '[]', '[]', 0.1, 0.1, 0.9, '[]', 'tester', 1, 1)",
                params![
                    fixture.board_id,
                    fixture.task_id,
                    fixture.other_label_id
                ],
            ),
            "label_semantic_proposals.board_id must match resolved_label_id board_id",
        ),
        (
            "proposal_resolved_label_update",
            conn.execute(
                "UPDATE label_semantic_proposals SET resolved_label_id=?1 WHERE id=?2",
                params![fixture.other_label_id, fixture.proposal_id],
            ),
            "label_semantic_proposals.board_id must match resolved_label_id board_id",
        ),
        (
            "signal_observation_insert",
            conn.execute(
                "INSERT INTO label_ontology_signals(
                 id, observation_id, board_id, kind, status, related_labels_json,
                 proposed_action, proposal_json, agent_selected, final_selected,
                 rationale, signal_key, created_at, updated_at)
                 VALUES ('los_schema_bad_observation', ?1, ?2, 'false_negative',
                 'open', '[]', 'add_positive_atom', '{}', 1, 1,
                 'bad observation board', 'schema-bad-observation', 1, 1)",
                params![fixture.observation_id, fixture.other_board_id],
            ),
            "label_ontology_signals.board_id must match observation_id board_id",
        ),
        (
            "signal_target_label_insert",
            conn.execute(
                "INSERT INTO label_ontology_signals(
                 id, observation_id, board_id, kind, status, target_label_id,
                 related_labels_json, proposed_action, proposal_json, agent_selected,
                 final_selected, rationale, signal_key, created_at, updated_at)
                 VALUES ('los_schema_bad_target', ?1, ?2, 'false_negative',
                 'open', ?3, '[]', 'add_positive_atom', '{}', 1, 1,
                 'bad target label board', 'schema-bad-target', 1, 1)",
                params![
                    fixture.observation_id,
                    fixture.board_id,
                    fixture.other_label_id
                ],
            ),
            "label_ontology_signals.board_id must match target_label_id board_id",
        ),
        (
            "signal_target_label_update",
            conn.execute(
                "UPDATE label_ontology_signals SET target_label_id=?1 WHERE id=?2",
                params![fixture.other_label_id, fixture.signal_id],
            ),
            "label_ontology_signals.board_id must match target_label_id board_id",
        ),
        (
            "signal_supersede_insert",
            conn.execute(
                "INSERT INTO label_ontology_signals(
                 id, observation_id, board_id, kind, status, superseded_by_signal_id,
                 related_labels_json, proposed_action, proposal_json, agent_selected,
                 final_selected, rationale, signal_key, created_at, updated_at)
                 VALUES ('los_schema_bad_supersede', ?1, ?2, 'false_negative',
                 'open', ?3, '[]', 'add_positive_atom', '{}', 1, 1,
                 'bad supersede board', 'schema-bad-supersede', 1, 1)",
                params![
                    fixture.observation_id,
                    fixture.board_id,
                    fixture.other_signal_id
                ],
            ),
            "label_ontology_signals.board_id must match superseded_by_signal_id board_id",
        ),
        (
            "signal_supersede_update",
            conn.execute(
                "UPDATE label_ontology_signals SET superseded_by_signal_id=?1 WHERE id=?2",
                params![fixture.other_signal_id, fixture.signal_id],
            ),
            "label_ontology_signals.board_id must match superseded_by_signal_id board_id",
        ),
        (
            "action_parent_insert",
            conn.execute(
                "INSERT INTO label_ontology_actions(
                 id, board_id, action_type, reason, parent_action_id, change_json,
                 validation_requirement, validation_status, validation_json,
                 created_by, created_by_type, created_at)
                 VALUES ('loa_schema_bad_parent', ?1, 'confirm', 'bad parent board',
                 ?2, '{}', 'none', 'not_required', '{}', 'tester', 'user', 1)",
                params![fixture.board_id, fixture.other_action_id],
            ),
            "label_ontology_actions.board_id must match parent_action_id board_id",
        ),
        (
            "action_parent_update",
            conn.execute(
                "UPDATE label_ontology_actions SET parent_action_id=?1 WHERE id=?2",
                params![fixture.other_action_id, fixture.action_id],
            ),
            "label_ontology_actions.board_id must match parent_action_id board_id",
        ),
        (
            "action_target_label_insert",
            conn.execute(
                "INSERT INTO label_ontology_actions(
                 id, board_id, action_type, reason, target_label_id, change_json,
                 validation_requirement, validation_status, validation_json,
                 created_by, created_by_type, created_at)
                 VALUES ('loa_schema_bad_target', ?1, 'confirm', 'bad target board',
                 ?2, '{}', 'none', 'not_required', '{}', 'tester', 'user', 1)",
                params![fixture.board_id, fixture.other_label_id],
            ),
            "label_ontology_actions.board_id must match target_label_id board_id",
        ),
        (
            "action_target_label_update",
            conn.execute(
                "UPDATE label_ontology_actions SET target_label_id=?1 WHERE id=?2",
                params![fixture.other_label_id, fixture.action_id],
            ),
            "label_ontology_actions.board_id must match target_label_id board_id",
        ),
        (
            "action_result_label_insert",
            conn.execute(
                "INSERT INTO label_ontology_actions(
                 id, board_id, action_type, reason, result_label_id, change_json,
                 validation_requirement, validation_status, validation_json,
                 created_by, created_by_type, created_at)
                 VALUES ('loa_schema_bad_result_label', ?1, 'confirm', 'bad result label board',
                 ?2, '{}', 'none', 'not_required', '{}', 'tester', 'user', 1)",
                params![fixture.board_id, fixture.other_label_id],
            ),
            "label_ontology_actions.board_id must match result_label_id board_id",
        ),
        (
            "action_result_label_update",
            conn.execute(
                "UPDATE label_ontology_actions SET result_label_id=?1 WHERE id=?2",
                params![fixture.other_label_id, fixture.action_id],
            ),
            "label_ontology_actions.board_id must match result_label_id board_id",
        ),
        (
            "action_result_proposal_insert",
            conn.execute(
                "INSERT INTO label_ontology_actions(
                 id, board_id, action_type, reason, result_proposal_id, change_json,
                 validation_requirement, validation_status, validation_json,
                 created_by, created_by_type, created_at)
                 VALUES ('loa_schema_bad_result_proposal', ?1, 'confirm',
                 'bad result proposal board', ?2, '{}', 'none', 'not_required',
                 '{}', 'tester', 'user', 1)",
                params![fixture.board_id, fixture.other_proposal_id],
            ),
            "label_ontology_actions.board_id must match result_proposal_id board_id",
        ),
        (
            "action_result_proposal_update",
            conn.execute(
                "UPDATE label_ontology_actions SET result_proposal_id=?1 WHERE id=?2",
                params![fixture.other_proposal_id, fixture.action_id],
            ),
            "label_ontology_actions.board_id must match result_proposal_id board_id",
        ),
        (
            "action_signal_insert",
            conn.execute(
                "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at)
                 VALUES (?1, ?2, ?3, 1)",
                params![fixture.board_id, fixture.action_id, fixture.other_signal_id],
            ),
            "FOREIGN KEY constraint failed",
        ),
        (
            "action_signal_update",
            conn.execute(
                "UPDATE label_ontology_action_signals SET signal_id=?1
                 WHERE action_id=?2 AND signal_id=?3",
                params![
                    fixture.other_signal_id,
                    fixture.action_id,
                    fixture.signal_id
                ],
            ),
            "FOREIGN KEY constraint failed",
        ),
    ] {
        let error = result_err(result)?;
        assert!(
            error.to_string().contains(expected),
            "{name}: {error}"
        );
    }

    let fk_error_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    assert_eq!(fk_error_count, 0);
    Ok(())
}

#[test]
fn label_ontology_records_observation_signals_and_preserves_board_scope() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_records_observation_signals_and_preserves_board_scope")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".to_owned(),
            name: "Other".to_owned(),
            description: None,
        },
    )?;
    let cli = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        kanban_sqlite::CreateTask {
            title: "Add ontology CLI commands".to_owned(),
            description: Some("Record label ontology signals from a task labeling run".to_owned()),
            ..CreateTask::ready("unused")
        },
    )?;
    create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("unrelated board task"),
    )?;

    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            actor: LabelOntologyActor {
                name: "label-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            agent_candidates_json: json!([
                {"label": "cli", "confidence": 0.92, "reason": "adds CLI command surface"}
            ])
            .to_string(),
            suggestion_snapshot_json: json!({
                "result": {"selected_labels": [], "candidates": []}
            })
            .to_string(),
            final_decision_json: json!({"accepted_labels": ["cli"]}).to_string(),
            suggest_coverage: Some(0.61),
            suggest_coverage_cosine: Some(0.74),
            suggest_residual_norm: Some(0.39),
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics_json: "[]".to_owned(),
            capture_fingerprint: Some("cli-false-negative-run".to_owned()),
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::FalseNegative,
                target_label_ref: Some("cli".to_owned()),
                related_labels_json: "[]".to_owned(),
                proposed_action: LabelOntologyProposedAction::AddPositiveAtom,
                candidate_atom: Some(LabelOntologyCandidateAtomInput {
                    polarity: "positive".to_owned(),
                    kind: "applies_when".to_owned(),
                    text: "extends CLI subcommands, arguments, help output, or JSON behavior"
                        .to_owned(),
                }),
                proposed_label_name: None,
                proposal_json: "{}".to_owned(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Candidate),
                suggest_score: Some(0.08),
                suggest_rank: Some(4),
                final_selected: true,
                rationale: "The task expands the CLI surface although suggest scored cli weakly."
                    .to_owned(),
                confidence: Some(0.91),
                signal_key: Some("cli-false-negative".to_owned()),
            }],
        },
    )?;

    assert!(observation.id.starts_with("lor_"));
    assert_eq!(observation.task_id, task.id);
    assert_eq!(observation.task_ref_snapshot, task.task_ref);
    assert_eq!(observation.signals.len(), 1);
    let signal = &observation.signals[0];
    assert!(signal.id.starts_with("los_"));
    assert_eq!(signal.status, LabelOntologySignalStatus::Open);
    assert_eq!(signal.target_label_id.as_deref(), Some(cli.id.as_str()));
    assert_eq!(signal.target_label_name_snapshot.as_deref(), Some("cli"));
    let candidate_hash = signal
        .candidate_content_hash
        .as_deref()
        .context("candidate content hash")?;
    assert_eq!(candidate_hash.len(), 16);
    assert!(candidate_hash.chars().all(|ch| ch.is_ascii_hexdigit()));

    let default_signals = list_label_ontology_signals(
        &temp.path,
        "default",
        LabelOntologySignalListOptions::default(),
    )?;
    assert_eq!(default_signals.len(), 1);
    assert_eq!(default_signals[0].id, signal.id);

    let other_signals = list_label_ontology_signals(
        &temp.path,
        "other",
        LabelOntologySignalListOptions::default(),
    )?;
    assert!(other_signals.is_empty());

    let detail = get_label_ontology_signal(&temp.path, &signal.id)?;
    assert_eq!(detail.signal.id, signal.id);
    assert_eq!(detail.observation.id, observation.id);
    assert!(detail.actions.is_empty());

    let stored_suggest_input_hash: Option<String> = connect_file(&temp.path)?.query_row(
        "SELECT suggest_input_hash FROM label_ontology_observations WHERE id=?1",
        [&observation.id],
        |row| row.get(0),
    )?;
    let stored_suggest_input_hash =
        stored_suggest_input_hash.context("stored suggest_input_hash")?;
    assert_eq!(stored_suggest_input_hash.len(), 16);
    assert!(
        stored_suggest_input_hash
            .chars()
            .all(|ch| ch.is_ascii_hexdigit())
    );
    let task_snapshot: serde_json::Value = serde_json::from_str(&observation.task_snapshot_json)?;
    assert_ne!(
        task_snapshot["content_hash"].as_str(),
        Some(stored_suggest_input_hash.as_str())
    );

    Ok(())
}

#[test]
fn label_ontology_quality_distinguishes_signal_counts_from_rates() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_quality_distinguishes_signal_counts_from_rates")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let cli_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add CLI ontology quality report"),
    )?;
    let docs_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Document ontology quality cohort"),
    )?;

    for (fingerprint, signal_key) in [
        ("cli-quality-run-a", "cli-quality-signal-a"),
        ("cli-quality-run-b", "cli-quality-signal-b"),
    ] {
        let mut input = sample_record_input(vec![LabelOntologySignalInput {
            kind: LabelOntologySignalKind::FalseNegative,
            target_label_ref: Some("cli".to_owned()),
            related_labels_json: "[]".to_owned(),
            proposed_action: LabelOntologyProposedAction::AddPositiveAtom,
            candidate_atom: Some(LabelOntologyCandidateAtomInput {
                polarity: "positive".to_owned(),
                kind: "applies_when".to_owned(),
                text: "extends CLI subcommands or machine-readable output".to_owned(),
            }),
            proposed_label_name: None,
            proposal_json: "{}".to_owned(),
            agent_selected: true,
            suggest_state: Some(LabelOntologySuggestState::Absent),
            suggest_score: None,
            suggest_rank: None,
            final_selected: true,
            rationale: "The task is clearly CLI work, but suggest did not select cli.".to_owned(),
            confidence: Some(0.9),
            signal_key: Some(signal_key.to_owned()),
        }]);
        input.capture_fingerprint = Some(fingerprint.to_owned());
        record_label_ontology_observation(&temp.path, "default", &cli_task.id, input)?;
    }

    let signal_only_report = label_ontology_quality_report(
        &temp.path,
        "default",
        LabelOntologyQualityOptions { sample_limit: 10 },
    )?;
    assert_eq!(signal_only_report.denominator.observation_count, 2);
    assert_eq!(signal_only_report.denominator.distinct_task_count, 1);
    assert_eq!(
        signal_only_report.denominator.agreement_observation_count,
        0
    );
    assert_eq!(signal_only_report.disagreement.signal_count, 2);
    assert_eq!(signal_only_report.disagreement.distinct_task_count, 1);
    assert_eq!(
        signal_only_report
            .disagreement
            .by_kind
            .get("false_negative"),
        Some(&2)
    );
    assert_eq!(
        signal_only_report.disagreement.by_status.get("open"),
        Some(&2)
    );
    assert_eq!(signal_only_report.rates.disagreement_task_rate, None);
    assert!(!signal_only_report.precision_recall.available);
    assert!(
        signal_only_report
            .warnings
            .iter()
            .any(|warning| warning.contains("no agreement observations"))
    );

    insert_agreement_observation_fixture(&temp, &docs_task, "agreement-docs-quality")?;
    let truth_counts_before = ontology_quality_truth_counts(&temp.path)?;
    let report = label_ontology_quality_report(
        &temp.path,
        "default",
        LabelOntologyQualityOptions { sample_limit: 10 },
    )?;
    let truth_counts_after = ontology_quality_truth_counts(&temp.path)?;
    assert_eq!(truth_counts_after, truth_counts_before);

    assert_eq!(report.denominator.source, "label_ontology_observations");
    assert_eq!(report.denominator.observation_count, 3);
    assert_eq!(report.denominator.distinct_task_count, 2);
    assert_eq!(report.denominator.agreement_observation_count, 1);
    assert_eq!(report.denominator.agreement_task_count, 1);
    assert_eq!(report.disagreement.signal_count, 2);
    assert_eq!(report.disagreement.distinct_task_count, 1);
    assert_eq!(report.rates.disagreement_task_rate, Some(0.5));
    assert!(!report.precision_recall.available);
    assert!(report.precision_recall.reason.contains("expected labels"));
    assert_eq!(
        report.denominator.sample_task_refs,
        vec![cli_task.task_ref.clone(), docs_task.task_ref.clone()]
    );

    Ok(())
}

#[test]
fn label_ontology_record_derives_metrics_from_snapshot_and_preserves_canonical_state()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_record_derives_metrics_from_snapshot_and_preserves_canonical_state",
    )?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Capture ontology record metrics from suggest snapshot"),
    )?;

    let labels_before = list_labels(&temp.path, "default")?;
    let task_labels_before = get_task(&temp.path, "default", &task.id)?.labels;
    let atoms_before = list_label_atoms(&temp.path, "default")?;
    let mut input = sample_record_input(vec![sample_signal_input("snapshot-derived")]);
    input.suggestion_snapshot_json = json!({
        "selected_labels": [],
        "candidates": [],
        "coverage": 0.42,
        "coverage_cosine": 0.37,
        "residual_norm": 0.58,
        "needs_new_label": true,
        "degraded": true,
        "diagnostics": ["vector_store_disabled"]
    })
    .to_string();
    input.suggest_coverage = None;
    input.suggest_coverage_cosine = None;
    input.suggest_residual_norm = None;
    input.suggest_needs_new_label = false;
    input.suggest_degraded = false;
    input.diagnostics_json = json!([]).to_string();
    input.capture_fingerprint = Some("snapshot-derived-capture".to_owned());

    let observation =
        record_label_ontology_observation(&temp.path, "default", &task.id, input.clone())?;

    assert_score_near(observation.suggest_coverage, 0.42);
    assert_score_near(observation.suggest_coverage_cosine, 0.37);
    assert_score_near(observation.suggest_residual_norm, 0.58);
    assert!(observation.suggest_needs_new_label);
    assert!(observation.suggest_degraded);
    let diagnostics: serde_json::Value = serde_json::from_str(&observation.diagnostics_json)?;
    assert_eq!(diagnostics, json!(["vector_store_disabled"]));
    assert_eq!(observation.capture_fingerprint, "snapshot-derived-capture");

    assert_eq!(list_labels(&temp.path, "default")?, labels_before);
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.labels,
        task_labels_before
    );
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);

    let duplicate_error = result_err(record_label_ontology_observation(
        &temp.path, "default", &task.id, input,
    ))?;
    assert!(
        duplicate_error.to_string().contains("capture_fingerprint")
            || duplicate_error.to_string().contains("UNIQUE"),
        "error: {duplicate_error}"
    );
    let signals = list_label_ontology_signals(
        &temp.path,
        "default",
        LabelOntologySignalListOptions::default(),
    )?;
    assert_eq!(signals.len(), 1);

    Ok(())
}

#[test]
fn label_ontology_record_rejects_conflicting_snapshot_metrics() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_record_rejects_conflicting_snapshot_metrics")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject contradictory ontology record metrics"),
    )?;
    let snapshot = json!({
        "coverage": 0.42,
        "coverage_cosine": 0.37,
        "residual_norm": 0.58,
        "diagnostics": ["derived-diagnostic"]
    })
    .to_string();

    let mut coverage_conflict = sample_record_input(vec![sample_signal_input("coverage-conflict")]);
    coverage_conflict.suggestion_snapshot_json = snapshot.clone();
    coverage_conflict.suggest_coverage = Some(0.99);
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        coverage_conflict,
    ))?;
    assert!(
        error
            .to_string()
            .contains("suggest_coverage conflicts with suggestion_snapshot_json.coverage"),
        "error: {error}"
    );

    let mut diagnostics_conflict =
        sample_record_input(vec![sample_signal_input("diagnostics-conflict")]);
    diagnostics_conflict.suggestion_snapshot_json = snapshot;
    diagnostics_conflict.suggest_coverage = None;
    diagnostics_conflict.suggest_coverage_cosine = None;
    diagnostics_conflict.suggest_residual_norm = None;
    diagnostics_conflict.diagnostics_json = json!(["manual-diagnostic"]).to_string();
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        diagnostics_conflict,
    ))?;
    assert!(
        error
            .to_string()
            .contains("diagnostics_json conflicts with suggestion_snapshot_json.diagnostics"),
        "error: {error}"
    );

    assert!(
        list_label_ontology_signals(
            &temp.path,
            "default",
            LabelOntologySignalListOptions::default(),
        )?
        .is_empty()
    );

    Ok(())
}

#[test]
fn label_ontology_signal_input_rejects_atom_polarity_kind_mismatches() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_signal_input_rejects_atom_polarity_kind_mismatches")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject mismatched ontology candidate atoms"),
    )?;

    let mut negative_applies_when = sample_signal_input("negative-applies-when");
    negative_applies_when.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "negative".to_owned(),
        kind: "applies_when".to_owned(),
        text: "does not touch CLI behavior".to_owned(),
    });
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![negative_applies_when]),
    ))?;
    assert!(error.to_string().contains("candidate atom polarity"));

    let mut positive_excludes_when = sample_signal_input("positive-excludes-when");
    positive_excludes_when.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "positive".to_owned(),
        kind: "excludes_when".to_owned(),
        text: "UI-only polish".to_owned(),
    });
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![positive_excludes_when]),
    ))?;
    assert!(error.to_string().contains("candidate atom polarity"));

    Ok(())
}

#[test]
fn label_ontology_review_groups_by_label_with_distinct_task_sorting() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_review_groups_by_label_with_distinct_task_sorting")?;
    let fixture = seed_label_ontology_review_fixture(&temp)?;

    let groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::Label,
            include_all: false,
            limit: 10,
        },
    )?;

    assert_eq!(groups.len(), 3);
    let cli = &groups[0];
    assert_eq!(cli.group_by, LabelOntologyReviewGroupBy::Label);
    assert_eq!(cli.label_name.as_deref(), Some("cli"));
    assert_eq!(cli.task_count, 2);
    assert_eq!(cli.signal_count, 2);
    assert_eq!(cli.open_count, 1);
    assert_eq!(cli.confirmed_count, 1);
    assert_eq!(cli.degraded_count, 1);
    assert_eq!(cli.action_count, 1);
    assert!(cli.action_ids.contains(&fixture.confirm_action_id));
    assert_eq!(cli.sample_task_refs.len(), 2);
    assert_eq!(cli.candidate_atom_variants.len(), 1);
    assert_score_near(cli.average_score, 0.3);
    assert_score_near(cli.median_score, 0.3);

    let docs = groups
        .iter()
        .find(|group| group.label_name.as_deref() == Some("docs"))
        .context("docs group")?;
    assert_eq!(docs.label_name.as_deref(), Some("docs"));
    assert_eq!(docs.task_count, 1);
    assert_eq!(docs.signal_count, 2);
    assert!(
        docs.signal_count > cli.signal_count - 1,
        "raw signal count alone must not outrank distinct task count"
    );
    Ok(())
}

#[test]
fn label_ontology_review_groups_by_candidate_atom() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_review_groups_by_candidate_atom")?;
    let fixture = seed_label_ontology_review_fixture(&temp)?;

    let groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::CandidateAtom,
            include_all: false,
            limit: 10,
        },
    )?;

    let cli_atom = groups
        .iter()
        .find(|group| group.candidate_text.as_deref() == Some("adds CLI commands"))
        .context("cli candidate atom group")?;
    assert_eq!(cli_atom.task_count, 2);
    assert_eq!(cli_atom.signal_count, 2);
    assert_eq!(cli_atom.labels.len(), 1);
    assert_eq!(cli_atom.labels[0].name.as_deref(), Some("cli"));
    assert!(cli_atom.signal_ids.contains(&fixture.cli_open_signal_id));
    assert!(
        cli_atom
            .signal_ids
            .contains(&fixture.cli_confirmed_signal_id)
    );
    assert!(cli_atom.action_ids.contains(&fixture.confirm_action_id));
    Ok(())
}

#[test]
fn label_ontology_review_clusters_duplicate_signals_without_mutating_atoms() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_review_clusters_duplicate_signals_without_mutating_atoms")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task_a = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Cluster signal source A"),
    )?;
    let task_b = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Cluster signal source B"),
    )?;

    let before_atoms = list_label_atoms(&temp.path, "default")?;
    let observation_a = record_label_ontology_observation(
        &temp.path,
        "default",
        &task_a.id,
        review_record_input(vec![review_label_signal(
            "cluster-a",
            "cli",
            "Adds CLI commands!",
            0.2,
        )]),
    )?;
    let observation_b = record_label_ontology_observation(
        &temp.path,
        "default",
        &task_b.id,
        review_record_input(vec![review_label_signal(
            "cluster-b",
            "cli",
            "adds cli commands",
            0.3,
        )]),
    )?;

    let default_groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::CandidateAtom,
            include_all: false,
            limit: 10,
        },
    )?;
    assert_eq!(default_groups.len(), 2);

    let groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::Cluster,
            include_all: false,
            limit: 10,
        },
    )?;

    assert_eq!(groups.len(), 1);
    let cluster = &groups[0];
    assert_eq!(cluster.group_by, LabelOntologyReviewGroupBy::Cluster);
    assert_eq!(
        cluster.cluster_key.as_deref(),
        Some(
            "candidate:kind:false_negative|action:add_positive_atom|target:cli|proposed:none|text:adds cli commands"
        )
    );
    assert_eq!(
        cluster.cluster_reason.as_deref(),
        Some("normalized_candidate_text")
    );
    assert_eq!(cluster.task_count, 2);
    assert_eq!(cluster.signal_count, 2);
    assert!(cluster.signal_ids.contains(&observation_a.signals[0].id));
    assert!(cluster.signal_ids.contains(&observation_b.signals[0].id));
    assert_eq!(cluster.candidate_atom_variants.len(), 2);
    assert_eq!(list_label_atoms(&temp.path, "default")?, before_atoms);
    Ok(())
}

#[test]
fn label_ontology_review_cluster_separates_same_text_across_label_and_action_scope()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_review_cluster_separates_same_text_across_label_and_action_scope",
    )?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "docs".to_owned(),
            color: None,
        },
    )?;
    let cli_task_a = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("CLI scoped cluster source A"),
    )?;
    let cli_task_b = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("CLI scoped cluster source B"),
    )?;
    let docs_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Docs scoped cluster source"),
    )?;
    let cli_observe_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("CLI observe scoped cluster source"),
    )?;

    let cli_a = record_label_ontology_observation(
        &temp.path,
        "default",
        &cli_task_a.id,
        review_record_input(vec![review_label_signal(
            "same-text-cli-a",
            "cli",
            "Shared Boundary",
            0.21,
        )]),
    )?;
    let cli_b = record_label_ontology_observation(
        &temp.path,
        "default",
        &cli_task_b.id,
        review_record_input(vec![review_label_signal(
            "same-text-cli-b",
            "cli",
            "shared boundary",
            0.22,
        )]),
    )?;
    let docs = record_label_ontology_observation(
        &temp.path,
        "default",
        &docs_task.id,
        review_record_input(vec![review_label_signal(
            "same-text-docs",
            "docs",
            "shared boundary",
            0.23,
        )]),
    )?;
    let mut cli_observe_signal =
        review_label_signal("same-text-cli-observe", "cli", "shared boundary", 0.24);
    cli_observe_signal.proposed_action = LabelOntologyProposedAction::Observe;
    let cli_observe = record_label_ontology_observation(
        &temp.path,
        "default",
        &cli_observe_task.id,
        review_record_input(vec![cli_observe_signal]),
    )?;

    let groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::Cluster,
            include_all: false,
            limit: 10,
        },
    )?;

    assert_eq!(groups.len(), 3);
    let cli_group = groups
        .iter()
        .find(|group| group.signal_ids.contains(&cli_a.signals[0].id))
        .context("cli add atom cluster")?;
    assert_eq!(cli_group.signal_count, 2);
    assert!(cli_group.signal_ids.contains(&cli_b.signals[0].id));
    assert!(!cli_group.signal_ids.contains(&docs.signals[0].id));
    assert!(!cli_group.signal_ids.contains(&cli_observe.signals[0].id));

    let docs_group = groups
        .iter()
        .find(|group| group.signal_ids.contains(&docs.signals[0].id))
        .context("docs add atom cluster")?;
    assert_eq!(docs_group.signal_count, 1);
    assert_ne!(cli_group.cluster_key, docs_group.cluster_key);

    let observe_group = groups
        .iter()
        .find(|group| group.signal_ids.contains(&cli_observe.signals[0].id))
        .context("cli observe cluster")?;
    assert_eq!(observe_group.signal_count, 1);
    assert_ne!(cli_group.cluster_key, observe_group.cluster_key);
    assert_ne!(docs_group.cluster_key, observe_group.cluster_key);

    Ok(())
}

#[test]
fn label_ontology_review_cluster_is_repeatable_read_only_projection() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_review_cluster_is_repeatable_read_only_projection")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task_a = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Cluster readonly source A"),
    )?;
    let task_b = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Cluster readonly source B"),
    )?;
    let observation_a = record_label_ontology_observation(
        &temp.path,
        "default",
        &task_a.id,
        review_record_input(vec![review_label_signal(
            "readonly-cluster-a",
            "cli",
            "Readonly Cluster",
            0.25,
        )]),
    )?;
    let observation_b = record_label_ontology_observation(
        &temp.path,
        "default",
        &task_b.id,
        review_record_input(vec![review_label_signal(
            "readonly-cluster-b",
            "cli",
            "readonly cluster",
            0.35,
        )]),
    )?;

    let before_atoms = list_label_atoms(&temp.path, "default")?;
    let board = get_board(&temp.path, "default")?;
    let before_action_count: i64 = connect_file(&temp.path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions WHERE board_id=?1",
        [&board.id],
        |row| row.get(0),
    )?;
    let before_signals = list_label_ontology_signals(
        &temp.path,
        "default",
        LabelOntologySignalListOptions::default(),
    )?;
    assert!(
        before_signals
            .iter()
            .all(|signal| signal.status == LabelOntologySignalStatus::Open)
    );

    let first = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::Cluster,
            include_all: false,
            limit: 10,
        },
    )?;
    let second = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::Cluster,
            include_all: false,
            limit: 10,
        },
    )?;

    assert_eq!(first, second);
    assert_eq!(first.len(), 1);
    assert!(first[0].signal_ids.contains(&observation_a.signals[0].id));
    assert!(first[0].signal_ids.contains(&observation_b.signals[0].id));
    assert_eq!(list_label_atoms(&temp.path, "default")?, before_atoms);
    assert_eq!(
        connect_file(&temp.path)?.query_row(
            "SELECT COUNT(*) FROM label_ontology_actions WHERE board_id=?1",
            [&board.id],
            |row| row.get::<_, i64>(0),
        )?,
        before_action_count
    );
    assert_eq!(
        list_label_ontology_signals(
            &temp.path,
            "default",
            LabelOntologySignalListOptions::default(),
        )?,
        before_signals
    );

    Ok(())
}

#[test]
fn label_ontology_review_candidate_atom_fallback_separates_empty_candidates() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_review_candidate_atom_fallback_separates_empty_candidates")?;
    let fixture = seed_label_ontology_review_fixture(&temp)?;
    let gap_task_same = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Ontology gap source C"),
    )?;
    let gap_task_other = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Decision comment gap source"),
    )?;

    let same_gap_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &gap_task_same.id,
        review_record_input(vec![review_gap_signal(
            "gap-open-same",
            "Ontology Ledger",
            0.11,
        )]),
    )?;
    let other_gap_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &gap_task_other.id,
        review_record_input(vec![review_gap_signal(
            "gap-open-other",
            "Decision Comments",
            0.13,
        )]),
    )?;

    let groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::CandidateAtom,
            include_all: false,
            limit: 10,
        },
    )?;

    let ontology_gap = groups
        .iter()
        .find(|group| group.signal_ids.contains(&fixture.open_gap_signal_id))
        .context("ontology ledger empty-candidate group")?;
    assert!(ontology_gap.key.contains("no-candidate-atom"));
    assert!(ontology_gap.key.contains("vocabulary_gap"));
    assert!(ontology_gap.key.contains("bootstrap_label"));
    assert!(ontology_gap.key.contains("ontology ledger"));
    assert_eq!(ontology_gap.task_count, 2);
    assert_eq!(ontology_gap.signal_count, 2);
    assert!(
        ontology_gap
            .signal_ids
            .contains(&same_gap_observation.signals[0].id)
    );

    let other_gap = groups
        .iter()
        .find(|group| {
            group
                .signal_ids
                .contains(&other_gap_observation.signals[0].id)
        })
        .context("decision comments empty-candidate group")?;
    assert!(other_gap.key.contains("decision comments"));
    assert_eq!(other_gap.task_count, 1);
    assert_eq!(other_gap.signal_count, 1);
    assert_ne!(ontology_gap.key, other_gap.key);

    let cli_atom = groups
        .iter()
        .find(|group| group.candidate_text.as_deref() == Some("adds CLI commands"))
        .context("cli candidate atom group")?;
    assert_eq!(cli_atom.task_count, 2);
    assert_eq!(cli_atom.signal_count, 2);
    Ok(())
}

#[test]
fn label_ontology_review_candidate_atom_fallback_separates_target_kind_and_action()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_review_candidate_atom_fallback_separates_target_kind_and_action",
    )?;
    seed_label_ontology_review_fixture(&temp)?;
    let cli_update_a = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("CLI empty candidate update source A"),
    )?;
    let cli_update_b = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("CLI empty candidate update source B"),
    )?;
    let docs_update = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Docs empty candidate update source"),
    )?;
    let cli_observe = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("CLI empty candidate observe source"),
    )?;

    let cli_update_a_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &cli_update_a.id,
        review_record_input(vec![review_empty_target_signal(
            "empty-cli-update-a",
            "cli",
            LabelOntologySignalKind::BoundaryIssue,
            LabelOntologyProposedAction::UpdateSemantics,
            0.31,
        )]),
    )?;
    let cli_update_b_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &cli_update_b.id,
        review_record_input(vec![review_empty_target_signal(
            "empty-cli-update-b",
            "cli",
            LabelOntologySignalKind::BoundaryIssue,
            LabelOntologyProposedAction::UpdateSemantics,
            0.33,
        )]),
    )?;
    let docs_update_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &docs_update.id,
        review_record_input(vec![review_empty_target_signal(
            "empty-docs-update",
            "docs",
            LabelOntologySignalKind::BoundaryIssue,
            LabelOntologyProposedAction::UpdateSemantics,
            0.8,
        )]),
    )?;
    let cli_observe_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &cli_observe.id,
        review_record_input(vec![review_empty_target_signal(
            "empty-cli-observe",
            "cli",
            LabelOntologySignalKind::NameIssue,
            LabelOntologyProposedAction::Observe,
            0.41,
        )]),
    )?;
    let cli_update_b_signal_id = cli_update_b_observation.signals[0].id.clone();
    let confirm_action = create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![cli_update_b_signal_id.clone()],
            "confirm empty candidate update group",
        ),
    )?;

    let groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::CandidateAtom,
            include_all: false,
            limit: 20,
        },
    )?;

    let cli_update_group = groups
        .iter()
        .find(|group| {
            group
                .signal_ids
                .contains(&cli_update_a_observation.signals[0].id)
        })
        .context("cli update empty-candidate group")?;
    assert!(cli_update_group.key.contains("boundary_issue"));
    assert!(cli_update_group.key.contains("update_semantics"));
    assert_eq!(cli_update_group.labels.len(), 1);
    assert_eq!(cli_update_group.labels[0].name.as_deref(), Some("cli"));
    assert_eq!(cli_update_group.task_count, 2);
    assert_eq!(cli_update_group.signal_count, 2);
    assert_eq!(cli_update_group.open_count, 1);
    assert_eq!(cli_update_group.confirmed_count, 1);
    assert_eq!(cli_update_group.action_count, 1);
    assert!(cli_update_group.action_ids.contains(&confirm_action.id));
    assert!(cli_update_group.candidate_atom_variants.is_empty());

    let docs_update_group = groups
        .iter()
        .find(|group| {
            group
                .signal_ids
                .contains(&docs_update_observation.signals[0].id)
        })
        .context("docs update empty-candidate group")?;
    assert_eq!(docs_update_group.labels[0].name.as_deref(), Some("docs"));
    assert_eq!(docs_update_group.task_count, 1);
    assert_eq!(docs_update_group.signal_count, 1);
    assert_ne!(cli_update_group.key, docs_update_group.key);

    let cli_observe_group = groups
        .iter()
        .find(|group| {
            group
                .signal_ids
                .contains(&cli_observe_observation.signals[0].id)
        })
        .context("cli observe empty-candidate group")?;
    assert!(cli_observe_group.key.contains("name_issue"));
    assert!(cli_observe_group.key.contains("observe"));
    assert_eq!(cli_observe_group.labels[0].name.as_deref(), Some("cli"));
    assert_eq!(cli_observe_group.task_count, 1);
    assert_eq!(cli_observe_group.signal_count, 1);
    assert_ne!(cli_update_group.key, cli_observe_group.key);
    assert_ne!(docs_update_group.key, cli_observe_group.key);

    Ok(())
}

#[test]
fn label_ontology_review_groups_by_proposed_label_and_include_all() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_review_groups_by_proposed_label_and_include_all")?;
    let fixture = seed_label_ontology_review_fixture(&temp)?;

    let default_groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::ProposedLabel,
            include_all: false,
            limit: 10,
        },
    )?;
    assert!(
        default_groups
            .iter()
            .all(|group| !group.signal_ids.contains(&fixture.rejected_gap_signal_id))
    );

    let groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::ProposedLabel,
            include_all: true,
            limit: 10,
        },
    )?;

    let proposed = groups
        .iter()
        .find(|group| group.proposed_label_name_normalized.as_deref() == Some("ontology ledger"))
        .context("proposed label group")?;
    assert_eq!(proposed.task_count, 2);
    assert_eq!(proposed.signal_count, 2);
    assert_eq!(proposed.open_count, 1);
    assert_eq!(proposed.rejected_count, 1);
    assert!(proposed.signal_ids.contains(&fixture.open_gap_signal_id));
    assert!(
        proposed
            .signal_ids
            .contains(&fixture.rejected_gap_signal_id)
    );
    Ok(())
}

#[test]
fn label_ontology_review_returns_empty_groups() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_review_returns_empty_groups")?;
    init_database(&temp.path, "tester")?;

    let groups = review_label_ontology(
        &temp.path,
        "default",
        LabelOntologyReviewOptions {
            group_by: LabelOntologyReviewGroupBy::Label,
            include_all: false,
            limit: 10,
        },
    )?;

    assert!(groups.is_empty());
    Ok(())
}

#[test]
fn task_ontology_summary_counts_statuses_stale_degraded_and_actions() -> anyhow::Result<()> {
    let temp = TempDb::new("task_ontology_summary_counts_statuses_stale_degraded_and_actions")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Ontology summary source"),
    )?;
    let stale_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        review_record_input(vec![review_label_signal(
            "summary-stale-open",
            "cli",
            "old task wording should be stale",
            0.2,
        )]),
    )?;
    update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("Ontology summary source after edit".to_owned()),
            description: None,
            assignee: None,
            priority: None,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: None,
            expected_lock_version: None,
        },
    )?;
    let open_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        review_record_input(vec![review_label_signal(
            "summary-open",
            "cli",
            "current open task wording",
            0.3,
        )]),
    )?;
    let mut degraded_record = review_record_input(vec![review_label_signal(
        "summary-confirmed",
        "cli",
        "degraded confirmation",
        0.4,
    )]);
    degraded_record.suggest_degraded = true;
    let confirmed_observation =
        record_label_ontology_observation(&temp.path, "default", &task.id, degraded_record)?;
    let rejected_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        review_record_input(vec![review_label_signal(
            "summary-rejected",
            "cli",
            "rejected signal",
            0.5,
        )]),
    )?;
    let superseded_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        review_record_input(vec![review_label_signal(
            "summary-superseded",
            "cli",
            "superseded signal",
            0.6,
        )]),
    )?;
    let resolved_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        review_record_input(vec![review_label_signal(
            "summary-resolved",
            "cli",
            "resolved signal",
            0.7,
        )]),
    )?;

    let stale_signal = stale_observation.signals[0].id.clone();
    let open_signal = open_observation.signals[0].id.clone();
    let confirmed_signal = confirmed_observation.signals[0].id.clone();
    let rejected_signal = rejected_observation.signals[0].id.clone();
    let superseded_signal = superseded_observation.signals[0].id.clone();
    let resolved_signal = resolved_observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![confirmed_signal.clone()],
            "confirm degraded signal",
        ),
    )?;
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Reject,
            vec![rejected_signal.clone()],
            "reject weak signal",
        ),
    )?;
    create_label_ontology_action(
        &temp.path,
        "default",
        supersede_input(
            vec![superseded_signal.clone()],
            &open_signal,
            "supersede duplicate signal",
        ),
    )?;
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::ResolveNoChange,
            vec![resolved_signal.clone()],
            "resolve without ontology change",
        ),
    )?;

    let summary =
        task_ontology_summary(&temp.path, "default", &task.id)?.context("ontology summary")?;

    assert_eq!(summary.signal_count, 6);
    assert_eq!(summary.open_count, 2);
    assert_eq!(summary.confirmed_count, 1);
    assert_eq!(summary.resolved_count, 1);
    assert_eq!(summary.rejected_count, 1);
    assert_eq!(summary.superseded_count, 1);
    assert_eq!(summary.degraded_count, 1);
    assert_eq!(summary.stale_count, 1);
    assert_eq!(summary.legacy_incomparable_count, 0);
    assert_eq!(summary.incomparable_count, 2);
    assert_eq!(summary.action_count, 4);
    assert!(summary.oldest_open_confirmed_signal_at.is_some());
    assert!(summary.oldest_open_confirmed_signal_age_ms.is_some());
    assert!(summary.latest_signal_at.is_some());
    assert!(summary.latest_action_at.is_some());
    assert!(summary.sample_signals.len() <= 5);
    assert!(
        summary
            .sample_signals
            .iter()
            .any(|signal| signal.id == stale_signal && signal.stale)
    );
    assert!(
        summary
            .sample_signals
            .iter()
            .any(|signal| signal.id == confirmed_signal && signal.degraded)
    );
    Ok(())
}

#[test]
fn task_ontology_summary_returns_none_without_signals() -> anyhow::Result<()> {
    let temp = TempDb::new("task_ontology_summary_returns_none_without_signals")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Task without ontology"),
    )?;

    assert!(task_ontology_summary(&temp.path, "default", &task.id)?.is_none());
    Ok(())
}

#[test]
fn label_ontology_signal_input_enforces_proposed_action_requirements() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_signal_input_enforces_proposed_action_requirements")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject incomplete ontology proposed actions"),
    )?;

    let mut missing_candidate = sample_signal_input("missing-positive-atom-candidate");
    missing_candidate.candidate_atom = None;
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![missing_candidate]),
    ))?;
    assert!(error.to_string().contains("add_positive_atom"));

    let mut wrong_negative_atom = sample_signal_input("wrong-negative-atom-candidate");
    wrong_negative_atom.proposed_action = LabelOntologyProposedAction::AddNegativeAtom;
    wrong_negative_atom.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "positive".to_owned(),
        kind: "positive_example".to_owned(),
        text: "adds CLI JSON behavior".to_owned(),
    });
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![wrong_negative_atom]),
    ))?;
    assert!(error.to_string().contains("add_negative_atom"));

    let mut missing_label_name = sample_signal_input("missing-bootstrap-label-name");
    missing_label_name.kind = LabelOntologySignalKind::VocabularyGap;
    missing_label_name.target_label_ref = None;
    missing_label_name.proposed_action = LabelOntologyProposedAction::BootstrapLabel;
    missing_label_name.candidate_atom = None;
    missing_label_name.proposed_label_name = None;
    missing_label_name.proposal_json = json!({"description": "Ontology ledger work"}).to_string();
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![missing_label_name]),
    ))?;
    assert!(error.to_string().contains("bootstrap_label"));

    let mut merge_without_related = sample_signal_input("merge-without-related-labels");
    merge_without_related.proposed_action = LabelOntologyProposedAction::MergeLabels;
    merge_without_related.candidate_atom = None;
    merge_without_related.related_labels_json = "[]".to_owned();
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![merge_without_related]),
    ))?;
    assert!(error.to_string().contains("merge_labels"));

    Ok(())
}

#[test]
fn label_ontology_signal_input_rejects_invalid_metrics() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_signal_input_rejects_invalid_metrics")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject invalid ontology signal metrics"),
    )?;

    let mut bad_coverage =
        sample_record_input(vec![sample_signal_input("bad-observation-coverage")]);
    bad_coverage.suggest_coverage = Some(1.25);
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        bad_coverage,
    ))?;
    assert!(error.to_string().contains("suggest_coverage"));

    let mut bad_rank = sample_signal_input("bad-suggest-rank");
    bad_rank.suggest_rank = Some(0);
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![bad_rank]),
    ))?;
    assert!(error.to_string().contains("suggest_rank"));

    let mut bad_score = sample_signal_input("bad-suggest-score");
    bad_score.suggest_score = Some(f64::INFINITY);
    let error = result_err(record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![bad_score]),
    ))?;
    assert!(error.to_string().contains("suggest_score"));

    Ok(())
}

#[test]
fn label_ontology_lifecycle_actions_update_status_and_link_actions() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_lifecycle_actions_update_status_and_link_actions")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology CLI lifecycle commands"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![
            sample_signal_input("cli-false-negative"),
            sample_signal_input("cli-false-negative-duplicate"),
        ]),
    )?;
    let source = &observation.signals[0];
    let duplicate = &observation.signals[1];

    let confirm = create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![source.id.clone()],
            "Reviewer agrees this is a real CLI false negative.",
        ),
    )?;

    assert!(confirm.id.starts_with("loa_"));
    assert_eq!(confirm.signal_ids, vec![source.id.clone()]);
    let confirmed = get_label_ontology_signal(&temp.path, &source.id)?;
    assert_eq!(
        confirmed.signal.status,
        LabelOntologySignalStatus::Confirmed
    );
    assert_eq!(confirmed.actions.len(), 1);
    assert_eq!(confirmed.actions[0].id, confirm.id);

    let resolved = create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::ResolveNoChange,
            vec![source.id.clone()],
            "Existing semantics already covered this after review.",
        ),
    )?;
    let resolved_detail = get_label_ontology_signal(&temp.path, &source.id)?;
    assert_eq!(
        resolved_detail.signal.status,
        LabelOntologySignalStatus::Resolved
    );
    assert!(resolved_detail.signal.closed_at.is_some());
    assert_eq!(resolved_detail.actions.len(), 2);
    assert_eq!(resolved_detail.actions[1].id, resolved.id);

    let err = result_err(create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![source.id.clone()],
            "Cannot confirm a resolved signal.",
        ),
    ))?;
    assert!(err.to_string().contains("invalid transition"));

    let mut supersede = action_input(
        LabelOntologyActionType::Supersede,
        vec![duplicate.id.clone()],
        "Duplicate of the confirmed CLI false-negative signal.",
    );
    supersede.superseded_by_signal_id = Some(source.id.clone());
    let supersede_action = create_label_ontology_action(&temp.path, "default", supersede)?;
    let duplicate_detail = get_label_ontology_signal(&temp.path, &duplicate.id)?;
    assert_eq!(
        duplicate_detail.signal.status,
        LabelOntologySignalStatus::Superseded
    );
    assert_eq!(
        duplicate_detail.signal.superseded_by_signal_id.as_deref(),
        Some(source.id.as_str())
    );
    assert_eq!(duplicate_detail.actions[0].id, supersede_action.id);

    Ok(())
}

#[test]
fn label_ontology_supersede_rejects_cycles() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_supersede_rejects_cycles")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology supersede cycle guard"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![
            sample_signal_input("cycle-a"),
            sample_signal_input("cycle-b"),
            sample_signal_input("cycle-c"),
            sample_signal_input("cycle-d"),
        ]),
    )?;
    let signal_a = observation.signals[0].id.clone();
    let signal_b = observation.signals[1].id.clone();
    let signal_c = observation.signals[2].id.clone();
    let signal_d = observation.signals[3].id.clone();

    create_label_ontology_action(
        &temp.path,
        "default",
        supersede_input(vec![signal_a.clone()], &signal_b, "A is replaced by B."),
    )?;

    let two_node_cycle = result_err(create_label_ontology_action(
        &temp.path,
        "default",
        supersede_input(
            vec![signal_b.clone()],
            &signal_a,
            "B cannot be replaced by A because A already points at B.",
        ),
    ))?;
    assert!(two_node_cycle.to_string().contains("supersede cycle"));

    create_label_ontology_action(
        &temp.path,
        "default",
        supersede_input(vec![signal_b.clone()], &signal_c, "B is replaced by C."),
    )?;

    let longer_cycle = result_err(create_label_ontology_action(
        &temp.path,
        "default",
        supersede_input(
            vec![signal_c.clone()],
            &signal_a,
            "C cannot be replaced by A because A points through B back to C.",
        ),
    ))?;
    assert!(longer_cycle.to_string().contains("supersede cycle"));

    let batch_cycle = result_err(create_label_ontology_action(
        &temp.path,
        "default",
        supersede_input(
            vec![signal_c.clone(), signal_d],
            &signal_a,
            "Batch supersede cannot include a signal already in the replacement chain.",
        ),
    ))?;
    assert!(batch_cycle.to_string().contains("supersede cycle"));

    Ok(())
}

#[test]
fn label_ontology_generic_action_rejects_canonical_mutation_types() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_generic_action_rejects_canonical_mutation_types")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology mutation guard"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-generic-mutation")]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;

    let mut mutation = action_input(
        LabelOntologyActionType::AddPositiveAtom,
        vec![signal_id],
        "The generic endpoint must not record canonical mutations.",
    );
    mutation.target_label_ref = Some("cli".to_owned());
    mutation.result_atom_id = Some("la_fabricated".to_owned());
    mutation.result_atom_content_hash = Some("deadbeefdeadbeef".to_owned());
    mutation.canonical_before_hash = Some("before".to_owned());
    mutation.canonical_after_hash = Some("after".to_owned());
    mutation.change_json = Some(json!({"fabricated": true}).to_string());
    mutation.validation_status = Some(LabelOntologyValidationStatus::Pending);

    let error = result_err(create_label_ontology_action(
        &temp.path, "default", mutation,
    ))?;
    assert!(
        error
            .to_string()
            .contains("dedicated canonical mutation endpoint")
    );

    Ok(())
}

#[test]
fn label_ontology_legacy_structure_plan_actions_remain_readable_and_importable()
-> anyhow::Result<()> {
    let source =
        TempDb::new("label_ontology_legacy_structure_plan_actions_remain_readable_and_importable")?;
    init_database(&source.path, "tester")?;
    create_label(
        &source.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &source.path,
        "default",
        "tester",
        CreateTask::ready("Legacy structure plan history remains readable"),
    )?;
    kanban_sqlite::add_task_labels_with_options(
        &source.path,
        "default",
        "tester",
        &task.id,
        &["cli".to_owned()],
        false,
    )?;
    let mut rename_signal = sample_signal_input("rename-cli-to-command-surface");
    rename_signal.proposed_action = LabelOntologyProposedAction::RenameLabel;
    rename_signal.candidate_atom = None;
    rename_signal.proposed_label_name = Some("command surface".to_owned());
    rename_signal.proposal_json = json!({
        "from": "cli",
        "to": "command surface",
        "reason": "Historical structure plan fixture."
    })
    .to_string();
    let observation = record_label_ontology_observation(
        &source.path,
        "default",
        &task.id,
        sample_record_input(vec![rename_signal]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &source.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer confirmed historical structure signal.",
        ),
    )?;
    let before_labels = structure_label_names(&source.path)?;
    let before_bindings = structure_task_label_rows(&source.path)?;

    let conn = connect_file(&source.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    let target_label_id: String = conn.query_row(
        "SELECT id FROM labels WHERE board_id=?1 AND name='cli'",
        params![board_id.as_str()],
        |row| row.get(0),
    )?;
    let action_id = "loa_legacy_structure_plan";
    let change_json = json!({
        "phase": "planned_structure_change",
        "canonical_mutation_applied": false,
        "change_type": "rename_label",
        "target_label": {
            "id": target_label_id,
            "name": "cli",
        },
        "after": {
            "proposed_label_name": "command surface",
        },
    })
    .to_string();
    let validation_json = json!({
        "state": "pending_structure_change_plan",
        "trusted_validation_required_before_apply": true,
    })
    .to_string();
    conn.execute(
        "INSERT INTO label_ontology_actions(
            id, board_id, parent_action_id, action_type, reason,
            target_label_id, result_label_id, result_atom_id, result_atom_content_hash,
            result_proposal_id, canonical_before_hash, canonical_after_hash,
            change_json, validation_requirement, validation_status, validation_json,
            created_by, created_by_type, agent_type, created_at
        ) VALUES (
            ?1, ?2, NULL, 'rename_label', ?3,
            ?4, NULL, NULL, NULL,
            NULL, 'legacy-before', 'legacy-after',
            ?5, 'unsupported', 'pending', ?6,
            'legacy-agent', 'agent', 'fixture', 42
        )",
        params![
            action_id,
            board_id.as_str(),
            "Historical structure plan kept for read compatibility.",
            target_label_id.as_str(),
            change_json,
            validation_json,
        ],
    )?;
    conn.execute(
        "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at)
         VALUES (?1, ?2, ?3, 42)",
        params![board_id.as_str(), action_id, signal_id.as_str()],
    )?;

    let detail = get_label_ontology_signal(&source.path, &signal_id)?;
    assert_eq!(detail.signal.status, LabelOntologySignalStatus::Confirmed);
    let legacy_action = detail
        .actions
        .iter()
        .find(|action| action.id == action_id)
        .context("legacy structure action")?;
    assert_eq!(
        legacy_action.action_type,
        LabelOntologyActionType::RenameLabel
    );
    assert_eq!(
        legacy_action.validation_requirement,
        LabelOntologyValidationRequirement::Unsupported
    );
    assert_eq!(
        legacy_action.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Unsupported
    );
    assert_eq!(legacy_action.signal_ids, vec![signal_id.clone()]);
    assert_eq!(structure_label_names(&source.path)?, before_labels);
    assert_eq!(structure_task_label_rows(&source.path)?, before_bindings);

    let export_path = source.dir.join("legacy-structure-plan.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let target = TempDb::new("label_ontology_legacy_structure_plan_import_target")?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &export_path, true)?;
    let imported_detail = get_label_ontology_signal(&target.path, &signal_id)?;
    let imported_action = imported_detail
        .actions
        .iter()
        .find(|action| action.id == action_id)
        .context("imported legacy structure action")?;
    assert_eq!(
        imported_action.validation_requirement,
        LabelOntologyValidationRequirement::Unsupported
    );
    assert_eq!(
        imported_action.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Unsupported
    );

    Ok(())
}

#[test]
fn label_ontology_generic_action_rejects_fabricated_provenance_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_generic_action_rejects_fabricated_provenance_fields")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology provenance guard"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-generic-provenance")]),
    )?;
    let signal_id = observation.signals[0].id.clone();

    let mut confirm = action_input(
        LabelOntologyActionType::Confirm,
        vec![signal_id],
        "The generic endpoint must not accept fabricated provenance.",
    );
    confirm.result_atom_id = Some("la_fabricated".to_owned());
    confirm.result_atom_content_hash = Some("deadbeefdeadbeef".to_owned());
    confirm.canonical_before_hash = Some("before".to_owned());
    confirm.canonical_after_hash = Some("after".to_owned());
    confirm.change_json = Some(json!({"fabricated": true}).to_string());
    confirm.validation_json = Some(json!({"fabricated": true}).to_string());

    let error = result_err(create_label_ontology_action(&temp.path, "default", confirm))?;
    assert!(
        error
            .to_string()
            .contains("cannot set canonical mutation provenance")
    );

    Ok(())
}

#[test]
fn label_ontology_validation_rejects_non_mutation_parent() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_validation_rejects_non_mutation_parent")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology validation parent guard"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-validation-lifecycle-parent")]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    let confirm = create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;

    let error = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: confirm.id,
            signal_ids: vec![signal_id],
            reason: "Lifecycle actions cannot be validated as canonical mutations.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: "{}".to_owned(),
        },
    ))?;
    assert!(error.to_string().contains("canonical mutation action"));

    Ok(())
}

#[test]
fn label_ontology_validation_rejects_parent_without_pending_canonical_evidence()
-> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_validation_rejects_parent_without_pending_canonical_evidence")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology validation evidence guard"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-validation-missing-evidence")]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;
    let bare_parent =
        seed_pending_mutation_action_without_evidence(&temp.path, &task.board_id, &signal_id)?;

    let error = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: bare_parent,
            signal_ids: vec![signal_id],
            reason: "Pending mutation without canonical evidence cannot be validated.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: "{}".to_owned(),
        },
    ))?;
    assert!(error.to_string().contains("canonical mutation evidence"));

    Ok(())
}

#[test]
fn label_ontology_external_passed_validation_rejects_trusted_update_semantics_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_passed_validation_rejects_trusted_update_semantics_payload",
    )?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let seed_semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            description: Some("Command-line interface behavior".to_owned()),
            applies_when: vec!["changes CLI user-visible behavior".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject untyped semantics validation"),
    )?;
    let update_signal = review_empty_target_signal(
        "cli-validation-update-semantics-unsupported-policy",
        "cli",
        LabelOntologySignalKind::BoundaryIssue,
        LabelOntologyProposedAction::UpdateSemantics,
        0.42,
    );
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![update_signal]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed semantics description adjustment.",
        ),
    )?;

    kanban_sqlite::upsert_label_semantics_with_options(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            expected_semantics_hash: Some(seed_semantics.semantics_hash.clone()),
            description: Some("Command-line interface and CLI JSON behavior".to_owned()),
            ..UpsertLabelSemantics::default()
        },
        kanban_sqlite::LabelSemanticsMutationOptions {
            actor: reviewer_actor(),
            reason: Some("Clarify CLI semantics description.".to_owned()),
            source_signal_ids: vec![signal_id.clone()],
            context_json: None,
        },
    )?;
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let update_action = detail
        .actions
        .iter()
        .find(|action| action.action_type == LabelOntologyActionType::UpdateSemantics)
        .context("update_semantics action")?;
    let validation_json = json!({
        "evidence_type": "trusted_automated",
        "embedding_model": "test-embedding-v1",
        "solver_options": {"candidate_limit": 24, "atom_limit": 64},
        "index": {"status": "ready", "dirty": false, "generation": 7},
        "cases": [{
            "signal_id": signal_id,
            "case_type": "update_semantics",
            "passed": true,
            "before": {},
            "after": {"degraded": false}
        }]
    });

    let error = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: update_action.id.clone(),
            signal_ids: Vec::new(),
            reason: "Passed validation requires a typed update semantics policy.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: validation_json.to_string(),
        },
    ))?;
    assert!(
        error
            .to_string()
            .contains("passed validation is unsupported")
    );
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    assert_eq!(detail.signal.status, LabelOntologySignalStatus::Confirmed);

    Ok(())
}

#[test]
fn label_ontology_external_passed_validation_rejects_untyped_evidence() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_external_passed_validation_rejects_untyped_evidence")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology validation evidence policy"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-validation-empty-evidence")]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;
    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed false-negative support for CLI surface changes.".to_owned(),
        },
    )?;

    let error = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: apply_action.id,
            signal_ids: Vec::new(),
            reason: "Passed validation must provide typed evidence.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: "{}".to_owned(),
        },
    ))?;
    assert!(
        error
            .to_string()
            .contains("trusted evidence collected by the kanban tool")
    );

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_fake_automated_passed_evidence() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_external_validation_rejects_fake_automated_passed_evidence")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Reject fake automated ontology validation",
        "cli-fake-automated-validation",
    )?;
    let mut fake_json = typed_positive_fixture_json(&fixture);
    fake_json["evidence_type"] = json!("automated");

    let error = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            fake_json,
            "Hand-written automated evidence must not close ontology signals.",
        ),
    ))?;
    assert!(
        error
            .to_string()
            .contains("trusted evidence collected by the kanban tool"),
        "{error}"
    );
    let detail = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    assert_eq!(detail.signal.status, LabelOntologySignalStatus::Confirmed);

    Ok(())
}

#[test]
fn label_ontology_atom_apply_records_provenance_and_external_validation_diagnostics()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_atom_apply_records_provenance_and_external_validation_diagnostics",
    )?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology CLI apply atom command"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![
            sample_signal_input("cli-apply-atom"),
            sample_signal_input("unrelated-cli-signal"),
        ]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    let unrelated_signal_id = observation.signals[1].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;

    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed false-negative support for CLI surface changes.".to_owned(),
        },
    )?;

    assert_eq!(
        apply_action.action_type,
        LabelOntologyActionType::AddPositiveAtom
    );
    assert_eq!(
        apply_action.validation_status,
        LabelOntologyValidationStatus::Pending
    );
    assert_eq!(apply_action.signal_ids, vec![signal_id.clone()]);
    assert!(
        apply_action
            .result_atom_id
            .as_deref()
            .unwrap()
            .starts_with("la_")
    );
    assert_eq!(
        apply_action
            .result_atom_content_hash
            .as_deref()
            .unwrap()
            .len(),
        16
    );
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &apply_action.id, Some("added"))?,
        1
    );
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &apply_action.id, Some("removed"))?,
        0
    );
    let semantics = get_label_semantics(&temp.path, "default", "cli")?;
    assert!(
        semantics
            .applies_when
            .iter()
            .any(|atom| atom.contains("CLI subcommands"))
    );
    let confirmed_detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    assert_eq!(
        confirmed_detail.signal.status,
        LabelOntologySignalStatus::Confirmed
    );
    assert_eq!(confirmed_detail.actions.len(), 2);

    let unrelated_validation = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: LabelOntologyActor {
                name: "validator".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            parent_action_id: apply_action.id.clone(),
            signal_ids: vec![unrelated_signal_id.clone()],
            reason: "This unrelated signal must not be resolved by this validation.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Failed,
            validation_json: json!({"cases": []}).to_string(),
        },
    ))?;
    assert!(
        unrelated_validation
            .to_string()
            .contains("is not linked to parent action")
    );

    let validation = validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: LabelOntologyActor {
                name: "validator".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            parent_action_id: apply_action.id.clone(),
            signal_ids: Vec::new(),
            reason: "Source task now selects cli with the new atom as evidence.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Failed,
            validation_json: typed_positive_validation_json(
                &signal_id,
                apply_action
                    .target_label_id
                    .as_deref()
                    .context("target label")?,
                apply_action
                    .result_atom_id
                    .as_deref()
                    .context("result atom id")?,
                apply_action
                    .result_atom_content_hash
                    .as_deref()
                    .context("result atom hash")?,
            )
            .to_string(),
        },
    )?;

    assert_eq!(validation.action_type, LabelOntologyActionType::Validate);
    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Failed
    );
    let validation_json: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(validation_json["summary"]["case_count"], 1);
    assert_eq!(validation_json["summary"]["stale_count"], 0);
    assert_eq!(validation_json["cases"][0]["signal_id"], signal_id);
    assert_eq!(validation_json["cases"][0]["comparable"], true);
    assert_eq!(validation_json["cases"][0]["before"]["score"], 0.08);
    assert_eq!(validation_json["manual"]["cases"][0]["passed"], true);
    assert_eq!(
        validation_json["cases"][0]["after"]["manual_case_ref"]["source"],
        "manual.cases"
    );
    assert_eq!(
        validation_json["cases"][0]["after"]["manual_case_ref"]["index"],
        0
    );
    assert_eq!(
        validation_json["cases"][0]["after"]["manual_case_ref"]["signal_id"],
        signal_id
    );
    assert!(
        validation_json["cases"][0]["after"].get("manual").is_none(),
        "generated validation case must reference top-level manual payload instead of duplicating it"
    );
    let resolved = get_label_ontology_signal(&temp.path, &signal_id)?;
    assert_eq!(resolved.signal.status, LabelOntologySignalStatus::Confirmed);
    assert_eq!(resolved.actions.len(), 3);

    let repeated_validation = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: LabelOntologyActor {
                name: "validator".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            parent_action_id: apply_action.id.clone(),
            signal_ids: vec![signal_id.clone()],
            reason: "External typed JSON must not pass validation.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: json!({"cases": []}).to_string(),
        },
    ))?;
    assert!(
        repeated_validation
            .to_string()
            .contains("trusted evidence collected by the kanban tool")
    );

    Ok(())
}

#[test]
fn label_ontology_validation_effective_outcome_reduces_requirement_and_latest_attempt()
-> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_validation_effective_outcome_reduces_requirement_and_latest")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Validate ontology effective outcome reducer",
        "cli-validation-effective-outcome",
    )?;

    let detail = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    let confirm_action = detail
        .actions
        .iter()
        .find(|action| action.action_type == LabelOntologyActionType::Confirm)
        .context("confirm action")?;
    assert_eq!(
        confirm_action.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::NotRequired
    );
    assert_eq!(confirm_action.validation_latest_attempt_id, None);
    let required_parent = detail
        .actions
        .iter()
        .find(|action| action.id == fixture.apply_action_id)
        .context("required apply action")?;
    assert_eq!(
        required_parent.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Pending
    );
    assert_eq!(required_parent.validation_latest_attempt_id, None);

    let failed = validate_label_ontology_action(
        &temp.path,
        "default",
        failed_validation_input(&fixture, "External failed attempt should be latest failed."),
    )?;
    let detail = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    let required_parent = detail
        .actions
        .iter()
        .find(|action| action.id == fixture.apply_action_id)
        .context("required apply action after failed attempt")?;
    assert_eq!(
        required_parent.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Failed
    );
    assert_eq!(
        required_parent.validation_latest_attempt_id.as_deref(),
        Some(failed.id.as_str())
    );
    assert_eq!(detail.signal.status, LabelOntologySignalStatus::Confirmed);

    let mut partial_input = failed_validation_input(
        &fixture,
        "External partial attempt should become latest partial.",
    );
    partial_input.validation_status = LabelOntologyValidationStatus::Partial;
    let partial = validate_label_ontology_action(&temp.path, "default", partial_input)?;
    let detail = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    let required_parent = detail
        .actions
        .iter()
        .find(|action| action.id == fixture.apply_action_id)
        .context("required apply action after partial attempt")?;
    assert_eq!(
        required_parent.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Partial
    );
    assert_eq!(
        required_parent.validation_latest_attempt_id.as_deref(),
        Some(partial.id.as_str())
    );
    assert_eq!(detail.signal.status, LabelOntologySignalStatus::Confirmed);

    let passed_id = seed_validation_action_with(SeedValidationAction {
        temp: &temp,
        id: "loa_effective_passed_latest",
        board_id: &fixture.task.board_id,
        parent_action_id: &fixture.apply_action_id,
        signal_ids: std::slice::from_ref(&fixture.signal_id),
        status: LabelOntologyValidationStatus::Passed,
        validation_json: json!({
            "manual": typed_positive_fixture_json(&fixture),
            "cases": [{
                "signal_id": &fixture.signal_id,
                "passed": true
            }],
            "summary": {
                "status": "passed",
                "case_count": 1
            }
        }),
        created_at: 9_999_999_999_999,
    })?;
    let detail = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    let required_parent = detail
        .actions
        .iter()
        .find(|action| action.id == fixture.apply_action_id)
        .context("required apply action after passed attempt")?;
    assert_eq!(
        required_parent.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Passed
    );
    assert_eq!(
        required_parent.validation_latest_attempt_id.as_deref(),
        Some(passed_id.as_str())
    );

    let update_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Record unsupported validation outcome"),
    )?;
    let update_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &update_task.id,
        sample_record_input(vec![review_empty_target_signal(
            "cli-validation-unsupported-outcome",
            "cli",
            LabelOntologySignalKind::BoundaryIssue,
            LabelOntologyProposedAction::UpdateSemantics,
            0.44,
        )]),
    )?;
    let update_signal_id = update_observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![update_signal_id.clone()],
            "Confirmed update semantics signal.",
        ),
    )?;
    let before_semantics = get_label_semantics(&temp.path, "default", "cli")?;
    kanban_sqlite::upsert_label_semantics_with_options(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            expected_semantics_hash: Some(before_semantics.semantics_hash),
            description: Some("Command-line interface and CLI JSON behavior".to_owned()),
            ..UpsertLabelSemantics::default()
        },
        kanban_sqlite::LabelSemanticsMutationOptions {
            actor: reviewer_actor(),
            reason: Some("Clarify CLI semantics from a confirmed source signal.".to_owned()),
            source_signal_ids: vec![update_signal_id.clone()],
            context_json: None,
        },
    )?;
    let detail = get_label_ontology_signal(&temp.path, &update_signal_id)?;
    let unsupported_parent = detail
        .actions
        .iter()
        .find(|action| action.action_type == LabelOntologyActionType::UpdateSemantics)
        .context("unsupported update_semantics action")?;
    assert_eq!(
        unsupported_parent.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Unsupported
    );
    assert_eq!(unsupported_parent.validation_latest_attempt_id, None);

    let unsupported_partial = validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: unsupported_parent.id.clone(),
            signal_ids: Vec::new(),
            reason: "External partial diagnostics are allowed for unsupported policies.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Partial,
            validation_json: json!({"cases": []}).to_string(),
        },
    )?;
    let detail = get_label_ontology_signal(&temp.path, &update_signal_id)?;
    let unsupported_parent = detail
        .actions
        .iter()
        .find(|action| action.action_type == LabelOntologyActionType::UpdateSemantics)
        .context("unsupported update_semantics action after partial attempt")?;
    assert_eq!(
        unsupported_parent.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Unsupported
    );
    assert_eq!(
        unsupported_parent.validation_latest_attempt_id.as_deref(),
        Some(unsupported_partial.id.as_str())
    );
    assert_eq!(detail.signal.status, LabelOntologySignalStatus::Confirmed);

    let unsupported_passed = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: unsupported_parent.id.clone(),
            signal_ids: Vec::new(),
            reason: "Unsupported policy cannot be passed.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: typed_positive_fixture_json(&fixture).to_string(),
        },
    ))?;
    assert!(
        unsupported_passed
            .to_string()
            .contains("passed validation is unsupported"),
        "{unsupported_passed}"
    );

    Ok(())
}

#[test]
fn label_ontology_revert_positive_atom_restores_before_hash_and_records_action()
-> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_revert_positive_atom_restores_before_hash_and_records_action")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            description: Some("Command-line interface behavior".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    let before_semantics = get_label_semantics(&temp.path, "default", "cli")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology CLI revert action"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-revert-positive-atom")]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;
    clear_label_atom_dirty_flags(&temp.path, "default")?;

    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed false-negative support for CLI surface changes.".to_owned(),
        },
    )?;
    assert!(label_atom_board_dirty(&temp.path, "default")?);
    clear_label_atom_dirty_flags(&temp.path, "default")?;
    assert!(!label_atom_board_dirty(&temp.path, "default")?);

    let revert_action = revert_label_ontology_mutation(
        &temp.path,
        "default",
        LabelOntologyRevertInput {
            actor: reviewer_actor(),
            target_action_id: apply_action.id.clone(),
            expected_current_hash: apply_action.canonical_after_hash.clone(),
            reason: "Revert test-only ontology mutation.".to_owned(),
        },
    )?;

    assert_eq!(
        revert_action.action_type,
        LabelOntologyActionType::RevertOntologyMutation
    );
    assert_eq!(
        revert_action.parent_action_id.as_deref(),
        Some(apply_action.id.as_str())
    );
    assert_eq!(
        revert_action.validation_status,
        LabelOntologyValidationStatus::Pending
    );
    assert_eq!(
        revert_action.canonical_before_hash,
        apply_action.canonical_after_hash
    );
    assert_eq!(
        revert_action.canonical_after_hash,
        apply_action.canonical_before_hash
    );
    assert_eq!(revert_action.signal_ids, vec![signal_id.clone()]);
    assert_eq!(
        ontology_action_atom_effect_hashes(&temp.path, &revert_action.id, "removed")?,
        vec![
            apply_action
                .result_atom_content_hash
                .clone()
                .context("applied atom hash")?
        ]
    );
    let change: serde_json::Value = serde_json::from_str(&revert_action.change_json)?;
    assert_eq!(change["reverted_action_id"], apply_action.id);
    assert_eq!(change["reverted_action_type"], "add_positive_atom");
    assert_eq!(change["index_dirty"], true);
    let restored_semantics = get_label_semantics(&temp.path, "default", "cli")?;
    assert_eq!(
        restored_semantics.semantics_hash,
        before_semantics.semantics_hash
    );
    assert_eq!(restored_semantics.description, before_semantics.description);
    assert_eq!(
        restored_semantics.applies_when,
        before_semantics.applies_when
    );
    assert_eq!(
        restored_semantics.excludes_when,
        before_semantics.excludes_when
    );
    assert_eq!(
        restored_semantics.positive_examples,
        before_semantics.positive_examples
    );
    assert_eq!(
        restored_semantics.negative_examples,
        before_semantics.negative_examples
    );
    assert_eq!(
        restored_semantics
            .atoms
            .iter()
            .map(|atom| (
                atom.polarity.as_str(),
                atom.kind.as_str(),
                atom.text.as_str(),
                atom.ordinal,
                atom.content_hash.as_str(),
            ))
            .collect::<Vec<_>>(),
        before_semantics
            .atoms
            .iter()
            .map(|atom| (
                atom.polarity.as_str(),
                atom.kind.as_str(),
                atom.text.as_str(),
                atom.ordinal,
                atom.content_hash.as_str(),
            ))
            .collect::<Vec<_>>()
    );
    let reverted_atom_id = apply_action
        .result_atom_id
        .as_deref()
        .context("applied action atom id")?;
    assert!(
        !list_label_atoms(&temp.path, "default")?
            .iter()
            .any(|atom| atom.id == reverted_atom_id)
    );
    assert!(label_atom_board_dirty(&temp.path, "default")?);

    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let action_types = detail
        .actions
        .iter()
        .map(|action| action.action_type)
        .collect::<Vec<_>>();
    assert!(action_types.contains(&LabelOntologyActionType::Confirm));
    assert!(action_types.contains(&LabelOntologyActionType::AddPositiveAtom));
    assert!(action_types.contains(&LabelOntologyActionType::RevertOntologyMutation));

    let original_action = detail
        .actions
        .iter()
        .find(|action| action.id == apply_action.id)
        .context("original action remains in history")?;
    assert_eq!(
        original_action.action_type,
        LabelOntologyActionType::AddPositiveAtom
    );
    let atom_explain = explain_label_atom(&temp.path, "default", reverted_atom_id)?;
    assert!(
        atom_explain.atom.is_none(),
        "reverted atom should no longer be canonical"
    );
    let explain_action_types = atom_explain
        .provenance_actions
        .iter()
        .map(|provenance| provenance.action.action_type)
        .collect::<Vec<_>>();
    assert!(
        atom_explain
            .provenance_actions
            .iter()
            .all(|provenance| provenance.matched_by == "atom_effect"),
        "{:?}",
        atom_explain.provenance_actions
    );
    assert!(
        explain_action_types.contains(&LabelOntologyActionType::AddPositiveAtom),
        "{explain_action_types:?}"
    );
    assert!(
        explain_action_types.contains(&LabelOntologyActionType::RevertOntologyMutation),
        "{explain_action_types:?}"
    );
    assert!(
        atom_explain.supporting_signals.iter().any(|support| {
            support.signal.id == signal_id
                && support.signal.status == LabelOntologySignalStatus::Confirmed
        }),
        "atom explain should keep the source signal history"
    );
    let atom_hash = apply_action
        .result_atom_content_hash
        .as_deref()
        .context("applied action atom hash")?;
    let atom_explain_by_hash = explain_label_atom(&temp.path, "default", atom_hash)?;
    assert!(
        atom_explain_by_hash.atom.is_none(),
        "reverted atom content hash should no longer resolve to a canonical atom"
    );
    assert!(
        atom_explain_by_hash
            .provenance_actions
            .iter()
            .all(|provenance| provenance.matched_by == "atom_effect"),
        "{:?}",
        atom_explain_by_hash.provenance_actions
    );

    let validation_action = validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: reviewer_actor(),
            parent_action_id: revert_action.id.clone(),
            signal_ids: vec![signal_id.clone()],
            reason: "External validation records that revert needs a future trusted check."
                .to_owned(),
            validation_status: LabelOntologyValidationStatus::Failed,
            validation_json: json!({
                "evidence_type": "external_attestation",
                "cases": []
            })
            .to_string(),
        },
    )?;
    assert_eq!(
        validation_action.action_type,
        LabelOntologyActionType::Validate
    );
    assert_eq!(
        validation_action.parent_action_id.as_deref(),
        Some(revert_action.id.as_str())
    );
    assert_eq!(
        validation_action.validation_status,
        LabelOntologyValidationStatus::Failed
    );
    assert_eq!(
        get_label_ontology_signal(&temp.path, &signal_id)?
            .signal
            .status,
        LabelOntologySignalStatus::Confirmed
    );

    Ok(())
}

#[test]
fn label_ontology_revert_legacy_child_action_records_warning() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_revert_legacy_child_action_records_warning")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Seed legacy per-atom revert target"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-legacy-child-revert")]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed legacy child action fixture.",
        ),
    )?;
    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![signal_id],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Create a canonical mutation that will be shaped like a legacy child row."
                .to_owned(),
        },
    )?;
    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO label_ontology_actions(
         id, board_id, action_type, reason, change_json, validation_status, validation_json,
         created_by, created_by_type, created_at)
         VALUES ('loa_legacy_parent_root', ?1, 'update_semantics', 'legacy parent fixture',
         '{}', 'not_required', '{}', 'fixture', 'agent', 1)",
        [&board_id],
    )?;
    conn.execute(
        "UPDATE label_ontology_actions SET parent_action_id='loa_legacy_parent_root' WHERE id=?1",
        [&apply_action.id],
    )?;
    conn.execute(
        "DELETE FROM label_ontology_action_atom_effects WHERE action_id=?1",
        [&apply_action.id],
    )?;

    let revert_action = revert_label_ontology_mutation(
        &temp.path,
        "default",
        LabelOntologyRevertInput {
            actor: reviewer_actor(),
            target_action_id: apply_action.id,
            expected_current_hash: apply_action.canonical_after_hash,
            reason: "Revert legacy per-atom child action fixture.".to_owned(),
        },
    )?;

    let change: serde_json::Value = serde_json::from_str(&revert_action.change_json)?;
    assert!(
        change["legacy_warning"]
            .as_str()
            .unwrap_or_default()
            .contains("legacy per-atom ontology action"),
        "{change}"
    );
    Ok(())
}

#[test]
fn label_ontology_revert_negative_atom_restores_before_hash_and_explain_chain() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_revert_negative_atom_restores_before_hash_and_explain_chain")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            description: Some("Command-line interface behavior".to_owned()),
            applies_when: vec!["changes CLI user-visible behavior".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let before_semantics = get_label_semantics(&temp.path, "default", "cli")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject docs-only task as CLI label match"),
    )?;
    let mut negative_signal = sample_signal_input("cli-revert-negative-atom");
    negative_signal.kind = LabelOntologySignalKind::FalsePositive;
    negative_signal.proposed_action = LabelOntologyProposedAction::AddNegativeAtom;
    negative_signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "negative".to_owned(),
        kind: "excludes_when".to_owned(),
        text: "only changes release notes or prose without touching CLI behavior".to_owned(),
    });
    negative_signal.rationale =
        "The source task was a false positive for cli and needs suppressing evidence.".to_owned();
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![negative_signal]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed false-positive suppressing evidence.",
        ),
    )?;

    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "excludes_when".to_owned(),
            text: "only changes release notes or prose without touching CLI behavior".to_owned(),
            reason: "Confirmed negative boundary for CLI label.".to_owned(),
        },
    )?;
    assert_eq!(
        apply_action.action_type,
        LabelOntologyActionType::AddNegativeAtom
    );
    let applied_atom_id = apply_action
        .result_atom_id
        .as_deref()
        .context("negative atom id")?;

    let revert_action = revert_label_ontology_mutation(
        &temp.path,
        "default",
        LabelOntologyRevertInput {
            actor: reviewer_actor(),
            target_action_id: apply_action.id.clone(),
            expected_current_hash: apply_action.canonical_after_hash.clone(),
            reason: "Revert negative boundary test mutation.".to_owned(),
        },
    )?;

    assert_eq!(
        revert_action.action_type,
        LabelOntologyActionType::RevertOntologyMutation
    );
    assert_eq!(
        revert_action.parent_action_id.as_deref(),
        Some(apply_action.id.as_str())
    );
    assert_eq!(
        revert_action.canonical_after_hash,
        apply_action.canonical_before_hash
    );
    assert_eq!(revert_action.signal_ids, vec![signal_id.clone()]);
    assert_eq!(
        ontology_action_atom_effect_hashes(&temp.path, &revert_action.id, "removed")?,
        vec![
            apply_action
                .result_atom_content_hash
                .clone()
                .context("negative atom hash")?
        ]
    );
    assert!(label_atom_board_dirty(&temp.path, "default")?);
    let restored_semantics = get_label_semantics(&temp.path, "default", "cli")?;
    assert_semantics_content_eq(&restored_semantics, &before_semantics);
    assert!(
        !list_label_atoms(&temp.path, "default")?
            .iter()
            .any(|atom| atom.id == applied_atom_id)
    );

    let atom_explain = explain_label_atom(&temp.path, "default", applied_atom_id)?;
    let explain_action_types = atom_explain
        .provenance_actions
        .iter()
        .map(|provenance| provenance.action.action_type)
        .collect::<Vec<_>>();
    assert!(
        explain_action_types.contains(&LabelOntologyActionType::AddNegativeAtom),
        "{explain_action_types:?}"
    );
    assert!(
        explain_action_types.contains(&LabelOntologyActionType::RevertOntologyMutation),
        "{explain_action_types:?}"
    );

    Ok(())
}

#[test]
fn label_ontology_revert_update_semantics_restores_before_hash_and_keeps_atom_history()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_revert_update_semantics_restores_before_hash_and_keeps_atom_history",
    )?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let seed_semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            description: Some("Command-line interface behavior".to_owned()),
            applies_when: vec!["changes CLI user-visible behavior".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let stable_atom = seed_semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("stable applies_when atom")?
        .clone();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Update CLI semantics description"),
    )?;
    let update_signal = review_empty_target_signal(
        "cli-revert-update-semantics",
        "cli",
        LabelOntologySignalKind::BoundaryIssue,
        LabelOntologyProposedAction::UpdateSemantics,
        0.42,
    );
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![update_signal]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed semantics description adjustment.",
        ),
    )?;

    let changed_semantics = kanban_sqlite::upsert_label_semantics_with_options(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            expected_semantics_hash: Some(seed_semantics.semantics_hash.clone()),
            description: Some("Command-line interface and CLI JSON behavior".to_owned()),
            ..UpsertLabelSemantics::default()
        },
        kanban_sqlite::LabelSemanticsMutationOptions {
            actor: reviewer_actor(),
            reason: Some("Clarify CLI semantics description.".to_owned()),
            source_signal_ids: vec![signal_id.clone()],
            context_json: None,
        },
    )?;
    assert_ne!(
        changed_semantics.semantics_hash,
        seed_semantics.semantics_hash
    );
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let update_action = detail
        .actions
        .iter()
        .find(|action| {
            action.action_type == LabelOntologyActionType::UpdateSemantics
                && action.canonical_after_hash.as_deref()
                    == Some(changed_semantics.semantics_hash.as_str())
        })
        .context("root update_semantics action")?
        .clone();
    assert_eq!(update_action.result_atom_id, None);
    assert_eq!(update_action.result_atom_content_hash, None);
    let update_effect_count: i64 = connect_file(&temp.path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_action_atom_effects WHERE action_id=?1",
        [&update_action.id],
        |row| row.get(0),
    )?;
    assert_eq!(update_effect_count, 0);

    let revert_action = revert_label_ontology_mutation(
        &temp.path,
        "default",
        LabelOntologyRevertInput {
            actor: reviewer_actor(),
            target_action_id: update_action.id.clone(),
            expected_current_hash: update_action.canonical_after_hash.clone(),
            reason: "Revert semantics description adjustment.".to_owned(),
        },
    )?;

    assert_eq!(
        revert_action.action_type,
        LabelOntologyActionType::RevertOntologyMutation
    );
    assert_eq!(
        revert_action.parent_action_id.as_deref(),
        Some(update_action.id.as_str())
    );
    assert_eq!(
        revert_action.canonical_after_hash,
        update_action.canonical_before_hash
    );
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &revert_action.id, None)?,
        0
    );
    assert_semantics_content_eq(
        &get_label_semantics(&temp.path, "default", "cli")?,
        &seed_semantics,
    );
    assert!(
        list_label_atoms(&temp.path, "default")?
            .iter()
            .any(|atom| atom.content_hash == stable_atom.content_hash)
    );

    Ok(())
}

#[test]
fn label_ontology_revert_rejects_expected_hash_mismatch_without_new_action() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_revert_rejects_expected_hash_mismatch_without_new_action")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Revert expected hash mismatch source"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-revert-wrong-expected-hash")]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed expected hash mismatch fixture.",
        ),
    )?;
    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![signal_id],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed atom for expected hash mismatch test.".to_owned(),
        },
    )?;
    let action_count_before = ontology_action_count(&temp.path)?;
    let current_semantics = get_label_semantics(&temp.path, "default", "cli")?;

    let error = result_err(revert_label_ontology_mutation(
        &temp.path,
        "default",
        LabelOntologyRevertInput {
            actor: reviewer_actor(),
            target_action_id: apply_action.id,
            expected_current_hash: Some("definitely-not-the-current-hash".to_owned()),
            reason: "Wrong expected hash should reject before writing action.".to_owned(),
        },
    ))?;

    assert!(
        error
            .to_string()
            .contains("expected_current_hash does not match"),
        "{error}"
    );
    assert_eq!(ontology_action_count(&temp.path)?, action_count_before);
    assert_semantics_content_eq(
        &get_label_semantics(&temp.path, "default", "cli")?,
        &current_semantics,
    );

    Ok(())
}

#[test]
fn label_ontology_revert_rejects_stale_current_hash() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_revert_rejects_stale_current_hash")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add stale revert ontology fixture"),
    )?;
    let mut second_signal = sample_signal_input("cli-revert-stale-second");
    second_signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "positive".to_owned(),
        kind: "positive_example".to_owned(),
        text: "adds a new kanban label ontology command".to_owned(),
    });
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![
            sample_signal_input("cli-revert-stale-first"),
            second_signal,
        ]),
    )?;
    let first_signal_id = observation.signals[0].id.clone();
    let second_signal_id = observation.signals[1].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![first_signal_id.clone(), second_signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;
    let first_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![first_signal_id],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed first atom.".to_owned(),
        },
    )?;
    apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![second_signal_id],
            label_ref: "cli".to_owned(),
            kind: "positive_example".to_owned(),
            text: "adds a new kanban label ontology command".to_owned(),
            reason: "Confirmed second atom.".to_owned(),
        },
    )?;

    let error = result_err(revert_label_ontology_mutation(
        &temp.path,
        "default",
        LabelOntologyRevertInput {
            actor: reviewer_actor(),
            target_action_id: first_action.id,
            expected_current_hash: None,
            reason: "Stale revert should be rejected.".to_owned(),
        },
    ))?;
    assert!(
        error
            .to_string()
            .contains("canonical ontology state changed"),
        "{error}"
    );

    Ok(())
}

#[test]
fn label_ontology_apply_existing_positive_atom_records_provenance_only_action() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_apply_existing_positive_atom_records_provenance_only_action")?;
    let fixture = seed_existing_atom_apply_fixture(
        &temp,
        ExistingAtomApplyKind::Positive,
        vec!["existing-positive-atom"],
    )?;
    let atoms_before = list_label_atoms(&temp.path, "default")?;
    let semantics_before = get_label_semantics(&temp.path, "default", "cli")?;
    clear_label_atom_dirty_flags(&temp.path, "default")?;
    assert!(!label_atom_store_dirty(&temp.path)?);
    assert!(!label_atom_board_dirty(&temp.path, "default")?);
    let add_action_count_before = add_atom_action_count(&temp.path)?;

    let action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![fixture.signal_ids[0].clone()],
            label_ref: "cli".to_owned(),
            kind: fixture.atom_kind.clone(),
            text: fixture.atom_text.clone(),
            reason: "Link confirmed signal to existing positive atom.".to_owned(),
        },
    )?;

    assert_existing_atom_adoption_action(&action, &fixture, "add_positive_atom")?;
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);
    assert_eq!(
        get_label_semantics(&temp.path, "default", "cli")?,
        semantics_before
    );
    assert_eq!(add_atom_action_count(&temp.path)?, add_action_count_before);
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &action.id, None)?,
        0
    );
    assert!(!label_atom_store_dirty(&temp.path)?);
    assert!(!label_atom_board_dirty(&temp.path, "default")?);

    let explain = explain_label_atom(&temp.path, "default", &fixture.atom_id)?;
    assert!(!explain.legacy_untracked);
    assert!(explain.provenance_actions.iter().any(|provenance| {
        provenance.action.id == action.id
            && provenance.action.action_type == LabelOntologyActionType::AdoptExistingAtom
            && provenance.matched_by == "legacy_result_atom_id"
    }));
    assert!(
        explain
            .supporting_signals
            .iter()
            .any(|support| support.signal.id == fixture.signal_ids[0])
    );

    Ok(())
}

#[test]
fn label_ontology_apply_existing_negative_atom_records_provenance_only_action() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_apply_existing_negative_atom_records_provenance_only_action")?;
    let fixture = seed_existing_atom_apply_fixture(
        &temp,
        ExistingAtomApplyKind::Negative,
        vec!["existing-negative-atom"],
    )?;
    let atoms_before = list_label_atoms(&temp.path, "default")?;
    clear_label_atom_dirty_flags(&temp.path, "default")?;
    let add_action_count_before = add_atom_action_count(&temp.path)?;

    let action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![fixture.signal_ids[0].clone()],
            label_ref: "cli".to_owned(),
            kind: fixture.atom_kind.clone(),
            text: fixture.atom_text.clone(),
            reason: "Link confirmed signal to existing negative atom.".to_owned(),
        },
    )?;

    assert_existing_atom_adoption_action(&action, &fixture, "add_negative_atom")?;
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);
    assert_eq!(add_atom_action_count(&temp.path)?, add_action_count_before);
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &action.id, None)?,
        0
    );
    assert!(!label_atom_store_dirty(&temp.path)?);
    assert!(!label_atom_board_dirty(&temp.path, "default")?);

    Ok(())
}

#[test]
fn label_ontology_apply_existing_atom_repeatedly_keeps_canonical_state_clean() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_apply_existing_atom_repeatedly_keeps_canonical_state_clean")?;
    let fixture = seed_existing_atom_apply_fixture(
        &temp,
        ExistingAtomApplyKind::Positive,
        vec!["existing-atom-first", "existing-atom-second"],
    )?;
    let atoms_before = list_label_atoms(&temp.path, "default")?;
    let semantics_before = get_label_semantics(&temp.path, "default", "cli")?;
    clear_label_atom_dirty_flags(&temp.path, "default")?;
    let add_action_count_before = add_atom_action_count(&temp.path)?;

    let first_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![fixture.signal_ids[0].clone()],
            label_ref: "cli".to_owned(),
            kind: fixture.atom_kind.clone(),
            text: fixture.atom_text.clone(),
            reason: "Link first signal to existing atom.".to_owned(),
        },
    )?;
    let second_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![fixture.signal_ids[1].clone()],
            label_ref: "cli".to_owned(),
            kind: fixture.atom_kind.clone(),
            text: fixture.atom_text.clone(),
            reason: "Link second signal to existing atom.".to_owned(),
        },
    )?;

    assert_ne!(first_action.id, second_action.id);
    assert_existing_atom_adoption_action(&first_action, &fixture, "add_positive_atom")?;
    assert_existing_atom_adoption_action(&second_action, &fixture, "add_positive_atom")?;
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);
    assert_eq!(
        get_label_semantics(&temp.path, "default", "cli")?,
        semantics_before
    );
    assert_eq!(add_atom_action_count(&temp.path)?, add_action_count_before);
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &first_action.id, None)?,
        0
    );
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &second_action.id, None)?,
        0
    );
    assert!(!label_atom_store_dirty(&temp.path)?);
    assert!(!label_atom_board_dirty(&temp.path, "default")?);

    let explain = explain_label_atom(&temp.path, "default", &fixture.atom_id)?;
    let adoption_action_ids = explain
        .provenance_actions
        .iter()
        .filter(|provenance| {
            provenance.action.action_type == LabelOntologyActionType::AdoptExistingAtom
        })
        .map(|provenance| provenance.action.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(adoption_action_ids.contains(first_action.id.as_str()));
    assert!(adoption_action_ids.contains(second_action.id.as_str()));
    let support_signal_ids = explain
        .supporting_signals
        .iter()
        .map(|support| support.signal.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(support_signal_ids.contains(fixture.signal_ids[0].as_str()));
    assert!(support_signal_ids.contains(fixture.signal_ids[1].as_str()));

    Ok(())
}

#[test]
fn label_ontology_validation_serialization_grows_linearly_with_signal_cases() -> anyhow::Result<()>
{
    let size_1 = validated_payload_size_for_signal_count(1)?;
    let size_10 = validated_payload_size_for_signal_count(10)?;
    let size_100 = validated_payload_size_for_signal_count(100)?;

    assert!(
        size_10 < size_1 * 15,
        "10-case validation payload should stay close to linear growth: size_1={size_1}, size_10={size_10}"
    );
    assert!(
        size_100 < size_10 * 15,
        "100-case validation payload should stay close to linear growth: size_10={size_10}, size_100={size_100}"
    );

    Ok(())
}

#[test]
fn label_ontology_validation_show_preserves_new_and_legacy_payload_shapes() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_validation_show_preserves_new_and_legacy_payload_shapes")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_multi_validation_fixture(&temp, 2)?;
    let validation_json = typed_positive_multi_validation_json(&fixture);

    let validation = validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id.clone(),
            signal_ids: Vec::new(),
            reason: "Multi-signal validation should store compact case references.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Failed,
            validation_json: validation_json.to_string(),
        },
    )?;
    let compact_payload: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(compact_payload["summary"]["case_count"], 2);
    assert_eq!(
        compact_payload["manual"]["cases"][1]["signal_id"],
        fixture.signal_ids[1]
    );
    let compact_case = compact_payload["cases"]
        .as_array()
        .context("compact cases")?
        .iter()
        .find(|case| case["signal_id"] == fixture.signal_ids[1])
        .context("compact case for second signal")?;
    assert_eq!(
        compact_case["after"]["manual_case_ref"]["index"],
        manual_case_index_for_signal(&fixture, &fixture.signal_ids[1])?
    );
    assert!(
        compact_payload["cases"][1]["after"].get("manual").is_none(),
        "new persisted cases must not duplicate top-level manual payload"
    );
    let compact_detail = get_label_ontology_signal(&temp.path, &fixture.signal_ids[0])?;
    let compact_action = compact_detail
        .actions
        .iter()
        .find(|action| action.id == validation.id)
        .context("compact validation action should be visible from signal show")?;
    let compact_show_payload: serde_json::Value =
        serde_json::from_str(&compact_action.validation_json)?;
    let compact_show_case = compact_show_payload["cases"]
        .as_array()
        .context("compact show cases")?
        .iter()
        .find(|case| case["signal_id"] == fixture.signal_ids[0])
        .context("compact show case for first signal")?;
    assert_eq!(
        compact_show_case["after"]["manual_case_ref"]["signal_id"],
        fixture.signal_ids[0]
    );

    let legacy_action_id = seed_legacy_validation_action(&temp, &fixture)?;
    let legacy_detail = get_label_ontology_signal(&temp.path, &fixture.signal_ids[0])?;
    let legacy_action = legacy_detail
        .actions
        .iter()
        .find(|action| action.id == legacy_action_id)
        .context("legacy validation action should be visible from signal show")?;
    let legacy_payload: serde_json::Value = serde_json::from_str(&legacy_action.validation_json)?;
    assert_eq!(
        legacy_payload["cases"][0]["signal_id"],
        fixture.signal_ids[0]
    );
    assert_eq!(
        legacy_payload["cases"][0]["after"]["manual"]["cases"][0]["signal_id"],
        fixture.signal_ids[0]
    );

    Ok(())
}

#[test]
fn label_ontology_jsonl_export_import_round_trips_ledger_and_self_refs() -> anyhow::Result<()> {
    let source =
        TempDb::new("label_ontology_jsonl_export_import_round_trips_ledger_and_self_refs_source")?;
    let fixture = seed_portable_ontology_ledger(&source)?;
    let source_explain =
        explain_label_atom(&source.path, "default", &fixture.result_atom_content_hash)?;
    let source_explain_actions = source_explain
        .provenance_actions
        .iter()
        .map(|provenance| {
            (
                provenance.action.id.clone(),
                provenance.action.action_type,
                provenance.matched_by.clone(),
            )
        })
        .collect::<Vec<_>>();
    let source_explain_signals = source_explain
        .supporting_signals
        .iter()
        .map(|support| support.signal.id.clone())
        .collect::<Vec<_>>();
    let source_explain_validations = source_explain
        .validation_history
        .iter()
        .map(|validation| {
            (
                validation.parent_action_id.clone(),
                validation.validation_status,
            )
        })
        .collect::<Vec<_>>();
    let export_path = source.dir.join("ontology.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let export = std::fs::read_to_string(&export_path)?;
    for record_type in [
        "label_ontology_observation",
        "label_ontology_signal",
        "label_ontology_action",
        "label_ontology_action_atom_effect",
        "label_ontology_action_signal",
    ] {
        assert!(
            export.contains(&format!("\"type\":\"{record_type}\"")),
            "missing {record_type} in export:\n{export}"
        );
    }
    assert!(
        export.contains("\"validation_requirement\":\"required\""),
        "missing required validation_requirement in export:\n{export}"
    );

    let reordered_path = source.dir.join("ontology-reordered.jsonl");
    write_reordered_ontology_export(&export_path, &reordered_path)?;

    let target =
        TempDb::new("label_ontology_jsonl_export_import_round_trips_ledger_and_self_refs_target")?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &reordered_path, true)?;

    let conn = connect_file(&target.path)?;
    let observation_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM label_ontology_observations",
        [],
        |row| row.get(0),
    )?;
    let signal_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM label_ontology_signals", [], |row| {
            row.get(0)
        })?;
    let action_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM label_ontology_actions", [], |row| {
            row.get(0)
        })?;
    let link_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM label_ontology_action_signals",
        [],
        |row| row.get(0),
    )?;
    let effect_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM label_ontology_action_atom_effects",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(observation_count, 1);
    assert_eq!(signal_count, 2);
    assert_eq!(action_count, 4);
    assert_eq!(effect_count, 1);
    assert_eq!(link_count, 4);
    let fk_error_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    assert_eq!(fk_error_count, 0);

    let duplicate = get_label_ontology_signal(&target.path, &fixture.duplicate_signal_id)?;
    assert_eq!(
        duplicate.signal.superseded_by_signal_id.as_deref(),
        Some(fixture.source_signal_id.as_str())
    );
    assert_eq!(
        duplicate.signal.status,
        LabelOntologySignalStatus::Superseded
    );
    let validation_parent: Option<String> = conn.query_row(
        "SELECT parent_action_id FROM label_ontology_actions WHERE id=?1",
        [&fixture.validation_action_id],
        |row| row.get(0),
    )?;
    assert_eq!(
        validation_parent.as_deref(),
        Some(fixture.apply_action_id.as_str())
    );
    let validation_json: String = conn.query_row(
        "SELECT validation_json FROM label_ontology_actions WHERE id=?1",
        [&fixture.validation_action_id],
        |row| row.get(0),
    )?;
    let validation: serde_json::Value = serde_json::from_str(&validation_json)?;
    assert_eq!(validation["summary"]["status"], "passed");
    assert_eq!(
        validation["cases"][0]["signal_id"],
        fixture.source_signal_id
    );
    let imported_requirements = conn
        .prepare(
            "SELECT id, validation_requirement
             FROM label_ontology_actions
             WHERE id IN (?1, ?2)
             ORDER BY id ASC",
        )?
        .query_map(
            params![fixture.apply_action_id, fixture.validation_action_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(
        imported_requirements,
        vec![
            (fixture.apply_action_id.clone(), "required".to_owned()),
            (fixture.validation_action_id.clone(), "none".to_owned()),
        ]
    );
    let source_detail = get_label_ontology_signal(&source.path, &fixture.source_signal_id)?;
    let source_apply = source_detail
        .actions
        .iter()
        .find(|action| action.id == fixture.apply_action_id)
        .context("source apply action")?;
    assert_eq!(
        source_apply.validation_effective_outcome,
        LabelOntologyValidationEffectiveOutcome::Passed
    );
    assert_eq!(
        source_apply.validation_latest_attempt_id.as_deref(),
        Some(fixture.validation_action_id.as_str())
    );
    let imported_detail = get_label_ontology_signal(&target.path, &fixture.source_signal_id)?;
    let imported_apply = imported_detail
        .actions
        .iter()
        .find(|action| action.id == fixture.apply_action_id)
        .context("imported apply action")?;
    assert_eq!(
        imported_apply.validation_effective_outcome,
        source_apply.validation_effective_outcome
    );
    assert_eq!(
        imported_apply.validation_latest_attempt_id,
        source_apply.validation_latest_attempt_id
    );

    let observation_candidates: String = conn.query_row(
        "SELECT agent_candidates_json FROM label_ontology_observations WHERE id=?1",
        [&fixture.observation_id],
        |row| row.get(0),
    )?;
    assert!(observation_candidates.contains("adds CLI command surface"));
    let imported_explain =
        explain_label_atom(&target.path, "default", &fixture.result_atom_content_hash)?;
    assert_eq!(
        imported_explain.atom.as_ref().map(|atom| atom.id.as_str()),
        Some(fixture.result_atom_id.as_str())
    );
    assert_eq!(
        imported_explain
            .atom
            .as_ref()
            .map(|atom| atom.content_hash.as_str()),
        Some(fixture.result_atom_content_hash.as_str())
    );
    assert_eq!(
        imported_explain
            .provenance_actions
            .iter()
            .map(|provenance| (
                provenance.action.id.clone(),
                provenance.action.action_type,
                provenance.matched_by.clone()
            ))
            .collect::<Vec<_>>(),
        source_explain_actions
    );
    assert_eq!(
        imported_explain
            .supporting_signals
            .iter()
            .map(|support| support.signal.id.clone())
            .collect::<Vec<_>>(),
        source_explain_signals
    );
    assert_eq!(
        imported_explain
            .validation_history
            .iter()
            .map(|validation| (
                validation.parent_action_id.clone(),
                validation.validation_status
            ))
            .collect::<Vec<_>>(),
        source_explain_validations
    );
    Ok(())
}

#[test]
fn label_ontology_jsonl_import_rejects_cross_board_action_signal_link() -> anyhow::Result<()> {
    let source =
        TempDb::new("label_ontology_jsonl_import_rejects_cross_board_action_signal_link_source")?;
    let fixture = seed_portable_ontology_ledger(&source)?;
    let other_board = create_board(
        &source.path,
        "tester",
        CreateBoard {
            slug: "other".to_owned(),
            name: "Other".to_owned(),
            description: None,
        },
    )?;
    create_task(
        &source.path,
        "other",
        "tester",
        CreateTask::ready("other board export parent"),
    )?;
    let default_export = source.dir.join("default-ontology.jsonl");
    let other_export = source.dir.join("other-board.jsonl");
    let invalid_export = source.dir.join("cross-board-action-signal.jsonl");
    export_jsonl(&source.path, "default", &default_export)?;
    export_jsonl(&source.path, "other", &other_export)?;
    let invalid = replace_action_signal_board_id(
        &std::fs::read_to_string(&default_export)?,
        &fixture.apply_action_id,
        &fixture.source_signal_id,
        &other_board.id,
    )?;
    std::fs::write(
        &invalid_export,
        format!("{}{}", std::fs::read_to_string(&other_export)?, invalid),
    )?;

    let target =
        TempDb::new("label_ontology_jsonl_import_rejects_cross_board_action_signal_link_target")?;
    init_database(&target.path, "tester")?;
    let sentinel = create_task(
        &target.path,
        "default",
        "tester",
        CreateTask::ready("ontology import rollback sentinel"),
    )?;
    let before_tasks = list_tasks(&target.path, "default", &[], true)?;
    let error = result_err(import_jsonl(&target.path, &invalid_export, true))?;

    assert!(
        error
            .to_string()
            .contains("label ontology action-signal board mismatch"),
        "error: {error}"
    );
    let after_tasks = list_tasks(&target.path, "default", &[], true)?;
    assert_eq!(after_tasks, before_tasks);
    assert_eq!(after_tasks[0].id, sentinel.id);
    Ok(())
}

#[test]
fn label_ontology_jsonl_import_rejects_supersede_cycle() -> anyhow::Result<()> {
    let source = TempDb::new("label_ontology_jsonl_import_rejects_supersede_cycle_source")?;
    let fixture = seed_portable_ontology_ledger(&source)?;
    let export_path = source.dir.join("ontology.jsonl");
    let invalid_export = source.dir.join("supersede-cycle.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let invalid = replace_signal_superseded_by(
        &std::fs::read_to_string(&export_path)?,
        &fixture.source_signal_id,
        &fixture.duplicate_signal_id,
    )?;
    std::fs::write(&invalid_export, invalid)?;

    let target = TempDb::new("label_ontology_jsonl_import_rejects_supersede_cycle_target")?;
    init_database(&target.path, "tester")?;
    let error = result_err(import_jsonl(&target.path, &invalid_export, true))?;

    assert!(
        error
            .to_string()
            .contains("label ontology signal supersede cycle"),
        "error: {error}"
    );
    Ok(())
}

#[test]
fn label_ontology_validation_allows_status_only_task_drift_with_warning() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_validation_allows_status_only_task_drift_with_warning")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation status drift warning",
        "cli-status-drift",
    )?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &fixture.task.id)?;
    claim_task(&temp.path, "default", "worker", &fixture.task.id, 300_000)?;

    let validation = validate_label_ontology_action(
        &temp.path,
        "default",
        failed_validation_input(&fixture, "Status-only task drift must remain comparable."),
    )?;

    let validation_json: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(validation_json["summary"]["stale_count"], 0);
    assert_eq!(validation_json["summary"]["metadata_drift_count"], 1);
    assert_eq!(validation_json["cases"][0]["comparable"], true);
    assert_eq!(validation_json["cases"][0]["stale"], false);
    assert_json_array_contains(
        &validation_json["cases"][0]["warnings"],
        "task_metadata_drift",
    );

    Ok(())
}

#[test]
fn label_ontology_validation_allows_label_binding_drift_with_warning() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_validation_allows_label_binding_drift_with_warning")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation label drift warning",
        "cli-label-drift",
    )?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    kanban_sqlite::add_task_label(&temp.path, "default", "tester", &fixture.task.id, "backend")?;
    let rerecorded = record_label_ontology_observation(
        &temp.path,
        "default",
        &fixture.task.id,
        sample_record_input(vec![sample_signal_input("cli-label-stale-rerecord")]),
    )?;
    assert_ne!(rerecorded.id, fixture.observation_id);

    let validation = validate_label_ontology_action(
        &temp.path,
        "default",
        failed_validation_input(&fixture, "Label binding drift must remain comparable."),
    )?;

    let validation_json: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(validation_json["summary"]["stale_count"], 0);
    assert_eq!(validation_json["summary"]["metadata_drift_count"], 1);
    assert_eq!(validation_json["summary"]["label_binding_drift_count"], 1);
    assert_eq!(validation_json["cases"][0]["comparable"], true);
    assert_eq!(validation_json["cases"][0]["stale"], false);
    assert_json_array_contains(
        &validation_json["cases"][0]["warnings"],
        "label_binding_drift",
    );

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_dirty_trusted_index() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_external_validation_rejects_handwritten_dirty_trusted_index")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation dirty index guard",
        "cli-dirty-index",
    )?;
    let mut validation_json = typed_positive_fixture_json(&fixture);
    validation_json["index"] = json!({
        "status": "dirty",
        "dirty": true,
        "generation": 9
    });

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass automated validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_trusted_collector_runs_suggest_and_resolves_signal() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_trusted_collector_runs_suggest_and_resolves_signal")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation trusted collector success",
        "cli-trusted-collector-success",
    )?;
    connect_file(&temp.path)?.execute(
        "UPDATE label_ontology_observations \
         SET suggest_coverage=0.0,suggest_residual_norm=1.0 \
         WHERE id=?1",
        [&fixture.observation_id],
    )?;
    let store = RecordingVectorStore::with_embedding_model("trusted-test-model");
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;
    let status = label_atom_index_status_with(&temp.path, "default", &store)?;
    let generation = status.generation.context("label atom index generation")?;

    let validation = validate_label_ontology_action_with_trusted_suggestions(
        &temp.path,
        "default",
        LabelOntologyTrustedValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id.clone(),
            signal_ids: Vec::new(),
            reason: "Trusted collector should close the signal from real suggest evidence."
                .to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            positive_control_task_refs: Vec::new(),
            positive_control_waiver_reason: None,
        },
        &store,
        LabelSuggestionOptions {
            output_limit: 5,
            candidate_limit: 5,
            atom_limit: 5,
            max_selected_labels: 1,
            min_score: 0.0,
        },
    )?;

    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Passed
    );
    let validation_json: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(validation_json["summary"]["case_count"], 1);
    assert_eq!(
        validation_json["manual"]["evidence_type"],
        "trusted_automated"
    );
    assert_eq!(
        validation_json["manual"]["collector"]["source"],
        "label_ontology_validate_trusted"
    );
    assert_eq!(
        validation_json["manual"]["embedding_model"],
        "trusted-test-model"
    );
    assert_eq!(
        validation_json["manual"]["solver_options"]["candidate_limit"],
        5
    );
    assert_eq!(validation_json["manual"]["index"]["generation"], generation);
    let evidence_atoms = validation_json["manual"]["cases"][0]["after"]["evidence_atoms"]
        .as_array()
        .context("trusted evidence atoms")?;
    assert!(
        evidence_atoms
            .iter()
            .any(|atom| atom["atom_id"] == fixture.result_atom_id),
        "trusted evidence atoms: {evidence_atoms:?}"
    );
    assert!(
        validation_json["manual"]["cases"][0]["suggestion"]["candidates"]
            .as_array()
            .context("trusted suggestion candidates")?
            .iter()
            .any(|candidate| candidate["label_id"] == fixture.target_label_id)
    );
    let resolved = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    assert_eq!(resolved.signal.status, LabelOntologySignalStatus::Resolved);

    Ok(())
}

#[test]
fn label_ontology_trusted_negative_collector_requires_control_or_user_waiver() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_trusted_negative_collector_requires_control_or_user_waiver")?;
    init_database(&temp.path, "tester")?;
    let (fixture, _control_task) = seed_negative_validation_fixture_with_positive_control(
        &temp,
        "Add ontology validation negative typed control requirement",
        "cli-negative-typed-control-required",
    )?;
    let store = RecordingVectorStore::with_embedding_model("trusted-test-model");
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;

    let error = result_err(validate_label_ontology_action_with_trusted_suggestions(
        &temp.path,
        "default",
        LabelOntologyTrustedValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id.clone(),
            signal_ids: Vec::new(),
            reason: "Negative trusted validation should require typed controls or a user waiver."
                .to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            positive_control_task_refs: Vec::new(),
            positive_control_waiver_reason: None,
        },
        &store,
        LabelSuggestionOptions {
            output_limit: 5,
            candidate_limit: 5,
            atom_limit: 5,
            max_selected_labels: 1,
            min_score: 0.0,
        },
    ))?;

    assert!(error.to_string().contains("positive control"), "{error}");
    assert_eq!(
        store.label_atom_vector_queries()?.len(),
        0,
        "collector should reject before running suggest"
    );

    let agent_waiver = result_err(validate_label_ontology_action_with_trusted_suggestions(
        &temp.path,
        "default",
        LabelOntologyTrustedValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id.clone(),
            signal_ids: Vec::new(),
            reason: "Agent waiver should not be accepted for negative trusted validation."
                .to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            positive_control_task_refs: Vec::new(),
            positive_control_waiver_reason: Some("No stable positive control exists.".to_owned()),
        },
        &store,
        LabelSuggestionOptions {
            output_limit: 5,
            candidate_limit: 5,
            atom_limit: 5,
            max_selected_labels: 1,
            min_score: 0.0,
        },
    ))?;

    assert!(agent_waiver.to_string().contains("user"), "{agent_waiver}");

    let waiver_reason = "  No stable positive control exists.  ".to_owned();
    let user_waiver = validate_label_ontology_action_with_trusted_suggestions(
        &temp.path,
        "default",
        LabelOntologyTrustedValidationInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            parent_action_id: fixture.apply_action_id,
            signal_ids: Vec::new(),
            reason: "User waiver should be preserved in trusted validation evidence.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            positive_control_task_refs: Vec::new(),
            positive_control_waiver_reason: Some(waiver_reason.clone()),
        },
        &store,
        LabelSuggestionOptions {
            output_limit: 5,
            candidate_limit: 5,
            atom_limit: 5,
            max_selected_labels: 1,
            min_score: 0.0,
        },
    )?;
    assert_eq!(
        user_waiver.validation_status,
        LabelOntologyValidationStatus::Passed
    );
    let validation_json: serde_json::Value = serde_json::from_str(&user_waiver.validation_json)?;
    assert_eq!(
        validation_json["manual"]["cases"][0]["after"]["positive_control_waiver"]["reason"],
        waiver_reason
    );

    Ok(())
}

#[test]
fn label_ontology_trusted_controls_are_negative_atom_only() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_trusted_controls_are_negative_atom_only")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation trusted control boundary",
        "cli-trusted-control-boundary",
    )?;
    let store = RecordingVectorStore::with_embedding_model("trusted-test-model");
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;

    let error = result_err(validate_label_ontology_action_with_trusted_suggestions(
        &temp.path,
        "default",
        LabelOntologyTrustedValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id,
            signal_ids: Vec::new(),
            reason: "Positive controls should be accepted only for negative atom validation."
                .to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            positive_control_task_refs: vec![fixture.task.task_ref],
            positive_control_waiver_reason: None,
        },
        &store,
        LabelSuggestionOptions {
            output_limit: 5,
            candidate_limit: 5,
            atom_limit: 5,
            max_selected_labels: 1,
            min_score: 0.0,
        },
    ))?;

    assert!(
        error.to_string().contains("negative atom validation"),
        "{error}"
    );
    assert_eq!(
        store.label_atom_vector_queries()?.len(),
        0,
        "collector should reject unsupported control parameters before running suggest"
    );

    Ok(())
}

#[test]
fn label_ontology_trusted_negative_collector_records_positive_controls() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_trusted_negative_collector_records_positive_controls")?;
    init_database(&temp.path, "tester")?;
    let (fixture, control_task) = seed_negative_validation_fixture_with_positive_control(
        &temp,
        "Add ontology validation negative typed controls",
        "cli-negative-typed-control",
    )?;
    let store = RecordingVectorStore::with_embedding_model("trusted-test-model");
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;

    let validation = validate_label_ontology_action_with_trusted_suggestions(
        &temp.path,
        "default",
        LabelOntologyTrustedValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id.clone(),
            signal_ids: Vec::new(),
            reason: "Negative trusted validation should include tool-collected positive controls."
                .to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            positive_control_task_refs: vec![control_task.task_ref.clone()],
            positive_control_waiver_reason: None,
        },
        &store,
        LabelSuggestionOptions {
            output_limit: 5,
            candidate_limit: 5,
            atom_limit: 5,
            max_selected_labels: 1,
            min_score: 0.0,
        },
    )?;

    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Passed
    );
    let validation_json: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    let control = &validation_json["manual"]["cases"][0]["after"]["positive_controls"][0];
    assert_eq!(control["task_ref"], control_task.task_ref);
    assert_eq!(control["target_label_id"], fixture.target_label_id);
    assert_eq!(control["passed"], true);
    assert_eq!(control["regressed"], false);
    assert_eq!(control["before"]["target"]["selected"], true);
    assert_eq!(control["after"]["target"]["selected"], true);
    assert_eq!(validation_json["manual"]["solver_options"]["atom_limit"], 5);
    assert!(
        store
            .label_atom_vector_queries()?
            .iter()
            .all(|query| query.embedding_model.as_deref() == Some("trusted-test-model")),
        "collector should query all cases with the same embedding model"
    );

    Ok(())
}

#[test]
fn label_ontology_public_validation_rejects_forged_trusted_generation_evidence()
-> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_public_validation_rejects_forged_trusted_generation_evidence")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation trusted generation guard",
        "cli-trusted-generation-guard",
    )?;
    let store = RecordingVectorStore::with_embedding_model("trusted-test-model");
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;
    let status = label_atom_index_status_with(&temp.path, "default", &store)?;
    let generation = status.generation.context("label atom index generation")?;
    let mut validation_json = typed_positive_fixture_json(&fixture);
    validation_json["collector"] = json!({
        "tool": "kanban",
        "source": "label_ontology_validate_trusted",
        "collected_at": 123
    });
    validation_json["embedding_model"] = json!("trusted-test-model");
    validation_json["index"]["generation"] = json!(generation - 1);

    let error = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Tool-collected validation must reject stale generation evidence.",
        ),
    ))?;
    assert!(
        error
            .to_string()
            .contains("external attestation cannot close ontology signals"),
        "{error}"
    );
    let signal = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    assert_eq!(signal.signal.status, LabelOntologySignalStatus::Confirmed);

    Ok(())
}

#[test]
fn label_ontology_atom_apply_rejects_mismatched_target_signal() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_atom_apply_rejects_mismatched_target_signal")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject unrelated ontology source signal"),
    )?;
    let mut backend_signal = sample_signal_input("backend-target-mismatch");
    backend_signal.target_label_ref = Some("backend".to_owned());
    backend_signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "positive".to_owned(),
        kind: "applies_when".to_owned(),
        text: "extends backend persistence or service APIs".to_owned(),
    });
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![backend_signal]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer confirmed backend signal.",
        ),
    )?;

    let error = result_err(apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "This should not use a backend source signal.".to_owned(),
        },
    ))?;
    assert!(error.to_string().contains(&signal_id), "{error}");
    assert!(error.to_string().contains("target label"), "{error}");

    Ok(())
}

#[test]
fn label_ontology_atom_apply_rejects_source_signal_action_polarity_and_kind_mismatch()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_atom_apply_rejects_source_signal_action_polarity_and_kind_mismatch",
    )?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject incompatible source signal atoms"),
    )?;

    let cases = vec![
        (
            "positive-used-for-negative",
            sample_signal_input("positive-used-for-negative"),
            "excludes_when",
            "only updates backend release notes",
            "proposed action add_positive_atom does not match apply atom action add_negative_atom",
        ),
        (
            "positive-kind-mismatch",
            {
                let mut signal = sample_signal_input("positive-kind-mismatch");
                signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
                    polarity: "positive".to_owned(),
                    kind: "positive_example".to_owned(),
                    text: "adds a CLI command example".to_owned(),
                });
                signal
            },
            "applies_when",
            "changes CLI command behavior",
            "candidate atom kind",
        ),
        (
            "update-semantics-used-for-atom",
            {
                let mut signal = sample_signal_input("update-semantics-used-for-atom");
                signal.proposed_action = LabelOntologyProposedAction::UpdateSemantics;
                signal.candidate_atom = None;
                signal.rationale = "The label description needs broader wording.".to_owned();
                signal
            },
            "applies_when",
            "changes CLI command behavior",
            "proposed action update_semantics does not match apply atom action add_positive_atom",
        ),
    ];

    for (case_name, signal, apply_kind, apply_text, expected_error) in cases {
        let signal_id = record_confirmed_test_signal(&temp, &task.id, signal)?;
        let action_count_before = ontology_action_count(&temp.path)?;
        let atoms_before = list_label_atoms(&temp.path, "default")?;

        let error = result_err(apply_label_ontology_atom(
            &temp.path,
            "default",
            LabelOntologyAtomApplyInput {
                actor: reviewer_actor(),
                signal_ids: vec![signal_id.clone()],
                label_ref: "cli".to_owned(),
                kind: apply_kind.to_owned(),
                text: apply_text.to_owned(),
                reason: format!("case {case_name} must be rejected"),
            },
        ))?;
        assert!(
            error.to_string().contains(expected_error),
            "{case_name}: {error}"
        );
        assert_eq!(ontology_action_count(&temp.path)?, action_count_before);
        assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);
    }

    Ok(())
}

#[test]
fn label_ontology_atom_apply_allows_generalized_text_with_matching_contract() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_atom_apply_allows_generalized_text_with_matching_contract")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Allow generalized atom text"),
    )?;
    let mut signal = sample_signal_input("generalized-text");
    signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "positive".to_owned(),
        kind: "applies_when".to_owned(),
        text: "adds exactly the foo command flag".to_owned(),
    });
    let signal_id = record_confirmed_test_signal(&temp, &task.id, signal)?;

    let action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![signal_id],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "changes CLI commands, flags, help output, or JSON behavior".to_owned(),
            reason: "Generalize atom text while preserving action/polarity/kind contract."
                .to_owned(),
        },
    )?;

    assert_eq!(action.action_type, LabelOntologyActionType::AddPositiveAtom);
    let semantics = get_label_semantics(&temp.path, "default", "cli")?;
    assert!(
        semantics
            .applies_when
            .iter()
            .any(|atom| { atom == "changes CLI commands, flags, help output, or JSON behavior" })
    );
    Ok(())
}

#[test]
fn label_ontology_atom_apply_retarget_does_not_bypass_atom_kind_contract() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_atom_apply_retarget_does_not_bypass_atom_kind_contract")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Retarget must preserve atom contract"),
    )?;
    let mut signal = sample_signal_input("retarget-kind-mismatch");
    signal.target_label_ref = Some("backend".to_owned());
    signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "positive".to_owned(),
        kind: "positive_example".to_owned(),
        text: "adds a backend API example".to_owned(),
    });
    let signal_id = record_confirmed_test_signal(&temp, &task.id, signal)?;
    let action_count_before = ontology_action_count(&temp.path)?;

    let error = result_err(apply_label_ontology_atom_with_options(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: reviewer_actor(),
            signal_ids: vec![signal_id],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "changes CLI command behavior".to_owned(),
            reason: "Retarget cannot change atom kind.".to_owned(),
        },
        LabelOntologyRetargetOptions {
            allow_retarget: true,
            retarget_reason: Some("Reviewer retargets only the label boundary.".to_owned()),
        },
    ))?;
    assert!(error.to_string().contains("candidate atom kind"), "{error}");
    assert_eq!(ontology_action_count(&temp.path)?, action_count_before);
    assert!(list_label_atoms(&temp.path, "default")?.is_empty());
    Ok(())
}

#[test]
fn label_ontology_atom_apply_retarget_override_records_reason() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_atom_apply_retarget_override_records_reason")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let backend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Retarget ontology source signal with audit reason"),
    )?;
    let mut backend_signal = sample_signal_input("backend-retarget-override");
    backend_signal.target_label_ref = Some("backend".to_owned());
    backend_signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "positive".to_owned(),
        kind: "applies_when".to_owned(),
        text: "extends backend persistence or service APIs".to_owned(),
    });
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![backend_signal]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer confirmed backend signal.",
        ),
    )?;

    let action = apply_label_ontology_atom_with_options(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Retarget source signal after reviewer boundary decision.".to_owned(),
        },
        LabelOntologyRetargetOptions {
            allow_retarget: true,
            retarget_reason: Some(
                "Backend signal actually describes CLI boundary work.".to_owned(),
            ),
        },
    )?;

    let change: serde_json::Value = serde_json::from_str(&action.change_json)?;
    assert_eq!(
        change["retarget_override"]["reason"],
        "Backend signal actually describes CLI boundary work."
    );
    assert_eq!(
        change["retarget_override"]["signals"][0]["target_label_id"],
        backend.id
    );
    assert_eq!(change["retarget_override"]["target_label"]["name"], "cli");

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_positive_atom_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_validation_rejects_handwritten_positive_atom_payload",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation result atom guard",
        "cli-result-atom-evidence",
    )?;
    let mut validation_json = typed_positive_fixture_json(&fixture);
    validation_json["cases"][0]["after"]["evidence_atoms"] = json!([]);

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass positive atom validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_negative_atom_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_validation_rejects_handwritten_negative_atom_payload",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom evidence",
        "cli-negative-evidence-success",
    )?;
    let validation_json = typed_negative_fixture_json(&fixture, true);

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass negative atom validation.",
    )?;
    let detail = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    assert_eq!(detail.signal.status, LabelOntologySignalStatus::Confirmed);

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_negative_positive_slot_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_validation_rejects_handwritten_negative_positive_slot_payload",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom evidence slot",
        "cli-negative-evidence-slot",
    )?;
    let mut validation_json = typed_negative_fixture_json(&fixture, true);
    validation_json["cases"][0]["after"]["evidence_atoms"] =
        validation_json["cases"][0]["after"]["negative_evidence_atoms"].take();

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass negative atom validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_negative_no_suppression_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_validation_rejects_handwritten_negative_no_suppression_payload",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom suppression",
        "cli-negative-suppression",
    )?;
    let mut validation_json = typed_negative_fixture_json(&fixture, true);
    validation_json["cases"][0]["after"]["target"]["selected"] = json!(true);
    validation_json["cases"][0]["after"]["target"]["score"] = json!(0.82);

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass negative atom validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_negative_missing_control_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_validation_rejects_handwritten_negative_missing_control_payload",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom control requirement",
        "cli-negative-missing-control",
    )?;
    let mut validation_json = typed_negative_fixture_json(&fixture, true);
    let after = validation_json["cases"][0]["after"]
        .as_object_mut()
        .context("negative validation after object")?;
    after.remove("positive_controls");

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass negative atom validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_negative_waiver_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_validation_rejects_handwritten_negative_waiver_payload",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom waiver",
        "cli-negative-waiver",
    )?;
    let mut validation_json = typed_negative_fixture_json(&fixture, true);
    let after = validation_json["cases"][0]["after"]
        .as_object_mut()
        .context("negative validation after object")?;
    after.remove("positive_controls");
    after.insert(
        "positive_control_waiver".to_owned(),
        json!({"reason": "No stable positive control task exists in this disposable fixture."}),
    );

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass negative atom validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_negative_empty_waiver_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_validation_rejects_handwritten_negative_empty_waiver_payload",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom empty waiver",
        "cli-negative-empty-waiver",
    )?;
    let mut validation_json = typed_negative_fixture_json(&fixture, true);
    let after = validation_json["cases"][0]["after"]
        .as_object_mut()
        .context("negative validation after object")?;
    after.remove("positive_controls");
    after.insert(
        "positive_control_waiver".to_owned(),
        json!({"reason": "  "}),
    );

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass negative atom validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_negative_control_regression_payload()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_external_validation_rejects_handwritten_negative_control_regression_payload",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom controls",
        "cli-negative-positive-control",
    )?;
    let validation_json = typed_negative_fixture_json(&fixture, false);

    assert_external_passed_trusted_json_rejected(
        &temp,
        &fixture.apply_action_id,
        validation_json,
        "External typed JSON cannot pass negative atom validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_typed_validation_negative_atom_records_control_regression_as_failed()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_typed_validation_negative_atom_records_control_regression_as_failed",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom failed control",
        "cli-negative-control-failed",
    )?;
    let mut validation_json = typed_negative_fixture_json(&fixture, false);
    validation_json["cases"][0]["passed"] = json!(false);

    let mut input = validation_input_from_json(
        &fixture.apply_action_id,
        validation_json,
        "Negative atom validation records positive control regression as failed.",
    );
    input.validation_status = LabelOntologyValidationStatus::Failed;
    let validation = validate_label_ontology_action(&temp.path, "default", input)?;

    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Failed
    );
    let signal = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    assert_eq!(signal.signal.status, LabelOntologySignalStatus::Confirmed);

    Ok(())
}

#[test]
fn label_ontology_external_validation_rejects_handwritten_bootstrap_payload() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_external_validation_rejects_handwritten_bootstrap_payload")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology bootstrap validation threshold"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::VocabularyGap,
                target_label_ref: None,
                related_labels_json: "[]".to_owned(),
                proposed_action: LabelOntologyProposedAction::BootstrapLabel,
                candidate_atom: None,
                proposed_label_name: Some("ontology-ledger".to_owned()),
                proposal_json: json!({
                    "name": "ontology-ledger",
                    "description": "Label ontology ledger work",
                    "applies_when": ["records ontology observations and signals"]
                })
                .to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Absent),
                suggest_score: None,
                suggest_rank: None,
                final_selected: true,
                rationale: "Existing labels do not express ontology ledger storage.".to_owned(),
                confidence: Some(0.86),
                signal_key: Some("ontology-ledger-threshold".to_owned()),
            }],
            ..sample_record_input(Vec::new())
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer agrees this vocabulary gap is real.",
        ),
    )?;
    let proposal_id =
        seed_label_semantic_proposal(&temp.path, &task.board_id, &task.id, "ontology-ledger")?;
    let accepted = accept_label_proposal_with_options(
        &temp.path,
        "reviewer",
        &proposal_id,
        Some("Bootstrap label from confirmed ontology signal.".to_owned()),
        LabelProposalDecisionOptions {
            source_signal_ids: vec![signal_id.clone()],
            ontology_actor: None,
            allow_retarget: false,
            retarget_reason: None,
        },
    )?;
    let result_label_id = accepted
        .resolved_label_id
        .as_deref()
        .context("resolved label")?;
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let bootstrap = detail
        .actions
        .iter()
        .find(|action| {
            action.action_type == LabelOntologyActionType::BootstrapLabel
                && action.result_atom_id.is_none()
        })
        .context("bootstrap action")?;

    assert_external_passed_trusted_json_rejected(
        &temp,
        &bootstrap.id,
        json!({
            "evidence_type": "trusted_automated",
            "embedding_model": "test-embedding-v1",
            "solver_options": {"candidate_limit": 24, "atom_limit": 64},
            "index": {"status": "ready", "dirty": false, "generation": 7},
            "cases": [{
                "signal_id": signal_id,
                "case_type": "bootstrap_label",
                "passed": true,
                "before": {"target": {"selected": false, "score": 0.0}},
                "after": {
                    "degraded": false,
                    "target": {
                        "label_id": result_label_id,
                        "selected": false,
                        "score": 0.49
                    },
                    "evidence_atoms": [{"label_id": result_label_id}]
                }
            }]
        }),
        "External typed JSON cannot pass bootstrap validation.",
    )?;

    Ok(())
}

#[test]
fn label_ontology_validation_marks_title_description_drift_incomparable() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_validation_marks_title_description_drift_incomparable")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation input drift guard",
        "cli-input-drift",
    )?;
    update_task(
        &temp.path,
        "default",
        "tester",
        &fixture.task.id,
        TaskPatch {
            title: Some("Add ontology validation input drift guard v2".to_owned()),
            ..TaskPatch::default()
        },
    )?;

    let validation = validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id,
            signal_ids: Vec::new(),
            reason: "Failed validation can record an incomparable title drift case.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Failed,
            validation_json: "{}".to_owned(),
        },
    )?;

    let validation_json: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(validation_json["summary"]["stale_count"], 1);
    assert_eq!(validation_json["summary"]["suggest_input_drift_count"], 1);
    assert_eq!(validation_json["cases"][0]["comparable"], false);
    assert_eq!(validation_json["cases"][0]["stale"], true);
    assert_json_array_contains(
        &validation_json["cases"][0]["warnings"],
        "suggest_input_drift",
    );

    Ok(())
}

#[test]
fn label_ontology_validation_marks_missing_legacy_suggest_hash_incomparable() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_validation_marks_missing_legacy_suggest_hash_incomparable")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology legacy validation comparability",
        "cli-legacy-hash",
    )?;
    connect_file(&temp.path)?.execute(
        "UPDATE label_ontology_observations SET suggest_input_hash=NULL WHERE id=?1",
        [&fixture.observation_id],
    )?;

    let validation = validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id,
            signal_ids: Vec::new(),
            reason: "Failed validation can record legacy missing hash incompatibility.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Failed,
            validation_json: "{}".to_owned(),
        },
    )?;

    let validation_json: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(validation_json["summary"]["stale_count"], 1);
    assert_eq!(validation_json["summary"]["legacy_incomparable_count"], 1);
    assert_eq!(validation_json["cases"][0]["comparable"], false);
    assert_eq!(validation_json["cases"][0]["legacy_incomparable"], true);
    assert_json_array_contains(
        &validation_json["cases"][0]["warnings"],
        "legacy_suggest_input_hash_missing",
    );

    Ok(())
}

#[test]
fn label_ontology_proposal_accept_records_bootstrap_provenance() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_proposal_accept_records_bootstrap_provenance")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology proposal provenance"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::VocabularyGap,
                target_label_ref: None,
                related_labels_json: "[]".to_owned(),
                proposed_action: LabelOntologyProposedAction::BootstrapLabel,
                candidate_atom: None,
                proposed_label_name: Some("ontology-ledger".to_owned()),
                proposal_json: json!({
                    "name": "ontology-ledger",
                    "description": "Label ontology ledger work",
                    "applies_when": ["records ontology observations and signals"]
                })
                .to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Absent),
                suggest_score: None,
                suggest_rank: None,
                final_selected: true,
                rationale: "Existing labels do not express ontology ledger storage.".to_owned(),
                confidence: Some(0.86),
                signal_key: Some("ontology-ledger-gap".to_owned()),
            }],
            ..sample_record_input(Vec::new())
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer agrees this vocabulary gap is real.",
        ),
    )?;
    let proposal_id =
        seed_label_semantic_proposal(&temp.path, &task.board_id, &task.id, "ontology-ledger")?;

    let accepted = accept_label_proposal_with_options(
        &temp.path,
        "reviewer",
        &proposal_id,
        Some("Bootstrap label from confirmed ontology signal.".to_owned()),
        LabelProposalDecisionOptions {
            source_signal_ids: vec![signal_id.clone()],
            ontology_actor: Some(LabelOntologyActor {
                name: "ontology-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("codex".to_owned()),
            }),
            allow_retarget: false,
            retarget_reason: None,
        },
    )?;

    let result_label_id = accepted
        .resolved_label_id
        .as_deref()
        .context("resolved label")?;
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    assert_eq!(detail.signal.status, LabelOntologySignalStatus::Confirmed);
    let bootstrap = detail
        .actions
        .iter()
        .find(|action| {
            action.action_type == LabelOntologyActionType::BootstrapLabel
                && action.result_atom_id.is_none()
        })
        .context("bootstrap action")?;
    assert_eq!(bootstrap.signal_ids, vec![signal_id.clone()]);
    assert_eq!(bootstrap.created_by, "ontology-agent");
    assert_eq!(bootstrap.created_by_type, "agent");
    assert_eq!(bootstrap.agent_type.as_deref(), Some("codex"));
    assert_eq!(bootstrap.result_label_id.as_deref(), Some(result_label_id));
    assert_eq!(
        bootstrap.result_proposal_id.as_deref(),
        Some(proposal_id.as_str())
    );
    assert_eq!(
        bootstrap.validation_status,
        LabelOntologyValidationStatus::Pending
    );
    assert!(bootstrap.canonical_after_hash.as_deref().is_some());
    assert!(bootstrap.change_json.contains("ontology-ledger"));
    assert_eq!(
        bootstrap_action_count_for_proposal(&temp.path, &proposal_id)?,
        1
    );
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &bootstrap.id, Some("added"))?,
        3
    );
    let semantics = get_label_semantics(&temp.path, "default", result_label_id)?;
    assert_eq!(semantics.label_name, "ontology-ledger");
    let atom = semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;
    let effect_hashes = ontology_action_atom_effect_hashes(&temp.path, &bootstrap.id, "added")?;
    let mut expected_hashes = semantics
        .atoms
        .iter()
        .map(|atom| atom.content_hash.clone())
        .collect::<Vec<_>>();
    expected_hashes.sort();
    assert_eq!(effect_hashes, expected_hashes);
    assert!(effect_hashes.contains(&atom.content_hash));

    Ok(())
}

#[test]
fn label_ontology_proposal_bootstrap_large_semantics_uses_one_root_action() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_proposal_bootstrap_large_semantics_uses_one_root_action")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add large ontology proposal provenance"),
    )?;
    let proposed_atoms = (0..99)
        .map(|index| format!("large ontology bootstrap atom {index:03}"))
        .collect::<Vec<_>>();
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::VocabularyGap,
                target_label_ref: None,
                related_labels_json: "[]".to_owned(),
                proposed_action: LabelOntologyProposedAction::BootstrapLabel,
                candidate_atom: None,
                proposed_label_name: Some("large-ontology-ledger".to_owned()),
                proposal_json: json!({
                    "name": "large-ontology-ledger",
                    "applies_when": proposed_atoms
                })
                .to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Absent),
                suggest_score: None,
                suggest_rank: None,
                final_selected: true,
                rationale: "Existing labels do not express the large ontology bootstrap fixture."
                    .to_owned(),
                confidence: Some(0.86),
                signal_key: Some("large-ontology-ledger-gap".to_owned()),
            }],
            ..sample_record_input(Vec::new())
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer agrees this large vocabulary gap is real.",
        ),
    )?;
    let proposal_id = seed_label_semantic_proposal_with_semantics(
        &temp.path,
        &task.board_id,
        &task.id,
        "large-ontology-ledger",
        None,
        &proposed_atoms,
        &[],
    )?;

    let accepted = accept_label_proposal_with_options(
        &temp.path,
        "reviewer",
        &proposal_id,
        Some("Bootstrap large label from confirmed ontology signal.".to_owned()),
        LabelProposalDecisionOptions {
            source_signal_ids: vec![signal_id.clone()],
            ontology_actor: Some(LabelOntologyActor {
                name: "ontology-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("codex".to_owned()),
            }),
            allow_retarget: false,
            retarget_reason: None,
        },
    )?;

    let result_label_id = accepted
        .resolved_label_id
        .as_deref()
        .context("resolved label")?;
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let bootstrap = detail
        .actions
        .iter()
        .find(|action| {
            action.action_type == LabelOntologyActionType::BootstrapLabel
                && action.result_atom_id.is_none()
        })
        .context("bootstrap action")?;
    assert_eq!(
        bootstrap_action_count_for_proposal(&temp.path, &proposal_id)?,
        1
    );
    assert_eq!(
        ontology_action_atom_effect_count(&temp.path, &bootstrap.id, Some("added"))?,
        100
    );
    let conn = connect_file(&temp.path)?;
    let child_bootstrap_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions \
         WHERE parent_action_id=?1 AND action_type='bootstrap_label'",
        [&bootstrap.id],
        |row| row.get(0),
    )?;
    assert_eq!(child_bootstrap_count, 0);
    let (bootstrap_rows, bootstrap_payload_sum): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(length(change_json)), 0) \
         FROM label_ontology_actions \
         WHERE result_proposal_id=?1 AND action_type='bootstrap_label'",
        [&proposal_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let bootstrap_payload_len: i64 = conn.query_row(
        "SELECT length(change_json) FROM label_ontology_actions WHERE id=?1",
        [&bootstrap.id],
        |row| row.get(0),
    )?;
    assert_eq!(bootstrap_rows, 1);
    assert_eq!(bootstrap_payload_sum, bootstrap_payload_len);
    let semantics = get_label_semantics(&temp.path, "default", result_label_id)?;
    assert_eq!(semantics.atoms.len(), 100);
    let effect_hashes = ontology_action_atom_effect_hashes(&temp.path, &bootstrap.id, "added")?;
    let mut expected_hashes = semantics
        .atoms
        .iter()
        .map(|atom| atom.content_hash.clone())
        .collect::<Vec<_>>();
    expected_hashes.sort();
    assert_eq!(effect_hashes, expected_hashes);
    Ok(())
}

#[test]
fn label_ontology_proposal_accept_rejects_unrelated_source_signal() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_proposal_accept_rejects_unrelated_source_signal")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject unrelated proposal source signal"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::VocabularyGap,
                target_label_ref: None,
                related_labels_json: "[]".to_owned(),
                proposed_action: LabelOntologyProposedAction::BootstrapLabel,
                candidate_atom: None,
                proposed_label_name: Some("backend-ledger".to_owned()),
                proposal_json: json!({
                    "name": "backend-ledger",
                    "description": "Backend ledger work"
                })
                .to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Absent),
                suggest_score: None,
                suggest_rank: None,
                final_selected: true,
                rationale: "Existing labels do not express backend ledger storage.".to_owned(),
                confidence: Some(0.86),
                signal_key: Some("backend-ledger-gap".to_owned()),
            }],
            ..sample_record_input(Vec::new())
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer agrees this vocabulary gap is real.",
        ),
    )?;
    let proposal_id =
        seed_label_semantic_proposal(&temp.path, &task.board_id, &task.id, "ontology-ledger")?;

    let error = result_err(accept_label_proposal_with_options(
        &temp.path,
        "reviewer",
        &proposal_id,
        Some("Attempt unrelated bootstrap source signal.".to_owned()),
        LabelProposalDecisionOptions {
            source_signal_ids: vec![signal_id.clone()],
            ontology_actor: None,
            allow_retarget: false,
            retarget_reason: None,
        },
    ))?;
    assert!(error.to_string().contains(&signal_id), "{error}");
    assert!(error.to_string().contains("proposed label"), "{error}");

    Ok(())
}

#[test]
fn label_ontology_proposal_accept_retarget_override_records_reason() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_proposal_accept_retarget_override_records_reason")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Accept proposal source signal with explicit retarget override"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::VocabularyGap,
                target_label_ref: None,
                related_labels_json: "[]".to_owned(),
                proposed_action: LabelOntologyProposedAction::BootstrapLabel,
                candidate_atom: None,
                proposed_label_name: Some("backend-ledger".to_owned()),
                proposal_json: json!({
                    "name": "backend-ledger",
                    "description": "Backend ledger work"
                })
                .to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Absent),
                suggest_score: None,
                suggest_rank: None,
                final_selected: true,
                rationale: "Existing labels do not express backend ledger storage.".to_owned(),
                confidence: Some(0.86),
                signal_key: Some("backend-ledger-retarget-gap".to_owned()),
            }],
            ..sample_record_input(Vec::new())
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer agrees this vocabulary gap is real.",
        ),
    )?;
    let proposal_id =
        seed_label_semantic_proposal(&temp.path, &task.board_id, &task.id, "ontology-ledger")?;

    accept_label_proposal_with_options(
        &temp.path,
        "reviewer",
        &proposal_id,
        Some("Accept with audited retarget.".to_owned()),
        LabelProposalDecisionOptions {
            source_signal_ids: vec![signal_id.clone()],
            ontology_actor: None,
            allow_retarget: true,
            retarget_reason: Some(
                "Backend-ledger signal was reclassified as ontology-ledger vocabulary.".to_owned(),
            ),
        },
    )?;
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let bootstrap = detail
        .actions
        .iter()
        .find(|action| action.action_type == LabelOntologyActionType::BootstrapLabel)
        .context("bootstrap action")?;
    let change: serde_json::Value = serde_json::from_str(&bootstrap.change_json)?;
    assert_eq!(
        change["retarget_override"]["reason"],
        "Backend-ledger signal was reclassified as ontology-ledger vocabulary."
    );
    assert_eq!(
        change["retarget_override"]["signals"][0]["proposed_label_name"],
        "backend-ledger"
    );
    assert_eq!(
        change["retarget_override"]["proposal"]["name"],
        "ontology-ledger"
    );

    Ok(())
}

#[test]
fn label_ontology_actor_contract_rejects_invalid_agent_metadata() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_actor_contract_rejects_invalid_agent_metadata")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Review ontology actor provenance"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        LabelOntologyRecordInput {
            signals: vec![sample_signal_input("actor-contract")],
            ..sample_record_input(Vec::new())
        },
    )?;
    let signal_id = observation.signals[0].id.clone();

    let mut user_with_agent_type = action_input(
        LabelOntologyActionType::Confirm,
        vec![signal_id.clone()],
        "User actors cannot carry agent type.",
    );
    user_with_agent_type.actor.agent_type = Some("codex".to_owned());
    let error = result_err(create_label_ontology_action(
        &temp.path,
        "default",
        user_with_agent_type,
    ))?;
    assert!(error.to_string().contains("agent_type"));

    let mut agent_without_agent_type = action_input(
        LabelOntologyActionType::Confirm,
        vec![signal_id.clone()],
        "Agent actors must carry agent type.",
    );
    agent_without_agent_type.actor.actor_type = "agent".to_owned();
    let error = result_err(create_label_ontology_action(
        &temp.path,
        "default",
        agent_without_agent_type,
    ))?;
    assert!(error.to_string().contains("agent_type"));

    let mut agent_action = action_input(
        LabelOntologyActionType::Confirm,
        vec![signal_id.clone()],
        "Agent actor provenance is complete.",
    );
    agent_action.actor = LabelOntologyActor {
        name: "codex".to_owned(),
        actor_type: "agent".to_owned(),
        agent_type: Some("codex".to_owned()),
    };
    let action = create_label_ontology_action(&temp.path, "default", agent_action)?;
    assert_eq!(action.created_by, "codex");
    assert_eq!(action.created_by_type, "agent");
    assert_eq!(action.agent_type.as_deref(), Some("codex"));

    Ok(())
}

struct OntologyValidationFixture {
    task: TaskRecord,
    observation_id: String,
    signal_id: String,
    apply_action_id: String,
    target_label_id: String,
    result_atom_id: String,
    result_atom_content_hash: String,
}

struct MultiOntologyValidationFixture {
    board_id: String,
    signal_ids: Vec<String>,
    apply_action_id: String,
    target_label_id: String,
    result_atom_id: String,
    result_atom_content_hash: String,
}

struct PortableOntologyLedgerFixture {
    observation_id: String,
    source_signal_id: String,
    duplicate_signal_id: String,
    apply_action_id: String,
    validation_action_id: String,
    result_atom_id: String,
    result_atom_content_hash: String,
}

fn seed_validation_fixture(
    temp: &TempDb,
    title: &str,
    signal_key: &str,
) -> anyhow::Result<OntologyValidationFixture> {
    let label = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(&temp.path, "default", "tester", CreateTask::ready(title))?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input(signal_key)]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;
    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed false-negative support for CLI surface changes.".to_owned(),
        },
    )?;
    Ok(OntologyValidationFixture {
        task,
        observation_id: observation.id,
        signal_id,
        apply_action_id: apply_action.id,
        target_label_id: label.id,
        result_atom_id: apply_action.result_atom_id.context("result atom id")?,
        result_atom_content_hash: apply_action
            .result_atom_content_hash
            .context("result atom hash")?,
    })
}

fn seed_multi_validation_fixture(
    temp: &TempDb,
    signal_count: usize,
) -> anyhow::Result<MultiOntologyValidationFixture> {
    let label = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Validate ontology serialization with many source signals"),
    )?;
    let signals = (0..signal_count)
        .map(|index| {
            sample_signal_input(&format!("validation-serialization-{signal_count}-{index}"))
        })
        .collect::<Vec<_>>();
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(signals),
    )?;
    let signal_ids = observation
        .signals
        .iter()
        .map(|signal| signal.id.clone())
        .collect::<Vec<_>>();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            signal_ids.clone(),
            "Confirmed all serialization fixture signals.",
        ),
    )?;
    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: signal_ids.clone(),
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed false-negative support from multiple source signals.".to_owned(),
        },
    )?;
    Ok(MultiOntologyValidationFixture {
        board_id: task.board_id,
        signal_ids,
        apply_action_id: apply_action.id,
        target_label_id: label.id,
        result_atom_id: apply_action.result_atom_id.context("result atom id")?,
        result_atom_content_hash: apply_action
            .result_atom_content_hash
            .context("result atom hash")?,
    })
}

fn validated_payload_size_for_signal_count(signal_count: usize) -> anyhow::Result<usize> {
    let temp_name = format!("label_ontology_validation_payload_linear_{signal_count}");
    let temp = TempDb::new(&temp_name)?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_multi_validation_fixture(&temp, signal_count)?;
    let validation = validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: fixture.apply_action_id.clone(),
            signal_ids: Vec::new(),
            reason: format!("Validate compact serialization for {signal_count} signals."),
            validation_status: LabelOntologyValidationStatus::Failed,
            validation_json: typed_positive_multi_validation_json(&fixture).to_string(),
        },
    )?;
    let payload: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(payload["summary"]["case_count"], signal_count);
    let cases = payload["cases"].as_array().context("generated cases")?;
    for (index, case) in cases.iter().enumerate() {
        assert!(
            case["after"].get("manual").is_none(),
            "case {index} unexpectedly duplicated the top-level manual payload"
        );
        let signal_id = case["signal_id"]
            .as_str()
            .context("generated case signal id")?;
        assert_eq!(
            case["after"]["manual_case_ref"]["index"],
            manual_case_index_for_signal(&fixture, signal_id)?
        );
        assert_eq!(case["after"]["manual_case_ref"]["signal_id"], signal_id);
    }
    Ok(validation.validation_json.len())
}

fn manual_case_index_for_signal(
    fixture: &MultiOntologyValidationFixture,
    signal_id: &str,
) -> anyhow::Result<usize> {
    fixture
        .signal_ids
        .iter()
        .position(|fixture_signal_id| fixture_signal_id == signal_id)
        .ok_or_else(|| test_error(format!("missing fixture signal {signal_id}")))
}

fn seed_legacy_validation_action(
    temp: &TempDb,
    fixture: &MultiOntologyValidationFixture,
) -> anyhow::Result<String> {
    let legacy_action_id = "loa_legacy_validation_payload_shape".to_owned();
    let manual = typed_positive_multi_validation_json(fixture);
    let legacy_payload = json!({
        "manual": manual.clone(),
        "cases": [{
            "signal_id": fixture.signal_ids[0],
            "task_id": "legacy-task",
            "after": {
                "validation_status": "passed",
                "manual": manual,
            },
            "passed": true
        }],
        "summary": {
            "status": "passed",
            "case_count": 1,
            "stale_count": 0,
            "degraded_count": 0,
            "incomparable_count": 0
        }
    });
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "INSERT INTO label_ontology_actions(
         id, board_id, parent_action_id, action_type, reason, target_label_id, result_label_id,
         result_atom_id, result_atom_content_hash, result_proposal_id, canonical_before_hash,
         canonical_after_hash, change_json, validation_status, validation_json, created_by,
         created_by_type, agent_type, created_at)
         VALUES (?1, ?2, ?3, 'validate', 'legacy validation payload shape',
         NULL, NULL, NULL, NULL, NULL, NULL, NULL, '{}', 'passed', ?4, 'legacy-fixture',
         'agent', 'codex', 123456)",
        params![
            legacy_action_id,
            fixture.board_id,
            fixture.apply_action_id,
            legacy_payload.to_string(),
        ],
    )?;
    conn.execute(
        "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at)
         VALUES (?1, ?2, ?3, 123456)",
        params![fixture.board_id, legacy_action_id, fixture.signal_ids[0]],
    )?;
    Ok(legacy_action_id)
}

fn seed_portable_ontology_ledger(temp: &TempDb) -> anyhow::Result<PortableOntologyLedgerFixture> {
    init_database(&temp.path, "tester")?;
    let label = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Export portable ontology ledger"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![
            sample_signal_input("portable-source"),
            sample_signal_input("portable-duplicate"),
        ]),
    )?;
    let source_signal_id = observation.signals[0].id.clone();
    let duplicate_signal_id = observation.signals[1].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![source_signal_id.clone()],
            "Confirmed portable ontology source signal.",
        ),
    )?;
    create_label_ontology_action(
        &temp.path,
        "default",
        supersede_input(
            vec![duplicate_signal_id.clone()],
            &source_signal_id,
            "Duplicate of the source signal.",
        ),
    )?;
    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![source_signal_id.clone()],
            label_ref: label.id.clone(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
            reason: "Confirmed portable ontology atom support.".to_owned(),
        },
    )?;
    let result_atom_id = apply_action
        .result_atom_id
        .as_deref()
        .context("result atom id")?
        .to_owned();
    let result_atom_hash = apply_action
        .result_atom_content_hash
        .as_deref()
        .context("result atom hash")?
        .to_owned();
    let manual = typed_positive_validation_json(
        &source_signal_id,
        &label.id,
        &result_atom_id,
        &result_atom_hash,
    );
    let validation_action_id = seed_validation_action(
        temp,
        "loa_portable_validation",
        &task.board_id,
        &apply_action.id,
        std::slice::from_ref(&source_signal_id),
        LabelOntologyValidationStatus::Passed,
        json!({
            "manual": manual,
            "cases": [{
                "signal_id": source_signal_id,
                "task_id": task.id,
                "after": {
                    "validation_status": "passed",
                    "manual_case_ref": {
                        "source": "manual.cases",
                        "index": 0,
                        "signal_id": source_signal_id
                    }
                },
                "passed": true
            }],
            "summary": {
                "status": "passed",
                "case_count": 1,
                "stale_count": 0,
                "degraded_count": 0,
                "incomparable_count": 0
            }
        }),
    )?;
    Ok(PortableOntologyLedgerFixture {
        observation_id: observation.id,
        source_signal_id,
        duplicate_signal_id,
        apply_action_id: apply_action.id,
        validation_action_id,
        result_atom_id,
        result_atom_content_hash: result_atom_hash,
    })
}

fn write_reordered_ontology_export(input_path: &Path, output_path: &Path) -> anyhow::Result<()> {
    let mut non_ontology = Vec::new();
    let mut observations = Vec::new();
    let mut signals = Vec::new();
    let mut actions = Vec::new();
    let mut effects = Vec::new();
    let mut links = Vec::new();
    for line in std::fs::read_to_string(input_path)?.lines() {
        let value: serde_json::Value = serde_json::from_str(line)?;
        match value["type"].as_str() {
            Some("label_ontology_observation") => observations.push(line.to_owned()),
            Some("label_ontology_signal") => signals.push(line.to_owned()),
            Some("label_ontology_action") => actions.push(line.to_owned()),
            Some("label_ontology_action_atom_effect") => effects.push(line.to_owned()),
            Some("label_ontology_action_signal") => links.push(line.to_owned()),
            _ => non_ontology.push(line.to_owned()),
        }
    }
    signals.sort_by_key(|line| {
        let value: serde_json::Value = serde_json::from_str(line).expect("valid JSONL test line");
        value["data"]["superseded_by_signal_id"].is_null()
    });
    actions.sort_by_key(|line| {
        let value: serde_json::Value = serde_json::from_str(line).expect("valid JSONL test line");
        value["data"]["parent_action_id"].is_null()
    });
    let mut reordered = non_ontology;
    reordered.extend(observations);
    reordered.extend(signals);
    reordered.extend(actions);
    reordered.extend(effects);
    reordered.extend(links);
    std::fs::write(output_path, format!("{}\n", reordered.join("\n")))?;
    Ok(())
}

fn replace_action_signal_board_id(
    export: &str,
    action_id: &str,
    signal_id: &str,
    board_id: &str,
) -> anyhow::Result<String> {
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in export.lines() {
        let mut value: serde_json::Value = serde_json::from_str(line)?;
        if value["type"].as_str() == Some("label_ontology_action_signal")
            && value["data"]["action_id"].as_str() == Some(action_id)
            && value["data"]["signal_id"].as_str() == Some(signal_id)
        {
            value["data"]["board_id"] = json!(board_id);
            replaced = true;
            lines.push(serde_json::to_string(&value)?);
        } else {
            lines.push(line.to_owned());
        }
    }
    if !replaced {
        return Err(test_error("missing action-signal link to mutate"));
    }
    Ok(format!("{}\n", lines.join("\n")))
}

fn replace_signal_superseded_by(
    export: &str,
    signal_id: &str,
    replacement_signal_id: &str,
) -> anyhow::Result<String> {
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in export.lines() {
        let mut value: serde_json::Value = serde_json::from_str(line)?;
        if value["type"].as_str() == Some("label_ontology_signal")
            && value["data"]["id"].as_str() == Some(signal_id)
        {
            value["data"]["superseded_by_signal_id"] = json!(replacement_signal_id);
            replaced = true;
            lines.push(serde_json::to_string(&value)?);
        } else {
            lines.push(line.to_owned());
        }
    }
    if !replaced {
        return Err(test_error("missing signal to mutate"));
    }
    Ok(format!("{}\n", lines.join("\n")))
}

fn seed_negative_validation_fixture(
    temp: &TempDb,
    title: &str,
    signal_key: &str,
) -> anyhow::Result<OntologyValidationFixture> {
    let label = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(&temp.path, "default", "tester", CreateTask::ready(title))?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![LabelOntologySignalInput {
            kind: LabelOntologySignalKind::FalsePositive,
            target_label_ref: Some("cli".to_owned()),
            related_labels_json: "[]".to_owned(),
            proposed_action: LabelOntologyProposedAction::AddNegativeAtom,
            candidate_atom: Some(LabelOntologyCandidateAtomInput {
                polarity: "negative".to_owned(),
                kind: "excludes_when".to_owned(),
                text: "does not change user-visible CLI behavior".to_owned(),
            }),
            proposed_label_name: None,
            proposal_json: "{}".to_owned(),
            agent_selected: true,
            suggest_state: Some(LabelOntologySuggestState::Selected),
            suggest_score: Some(0.82),
            suggest_rank: Some(1),
            final_selected: false,
            rationale: "The task was a false positive for the cli label.".to_owned(),
            confidence: Some(0.9),
            signal_key: Some(signal_key.to_owned()),
        }]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;
    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "excludes_when".to_owned(),
            text: "does not change user-visible CLI behavior".to_owned(),
            reason: "Confirmed false-positive support for CLI label suppression.".to_owned(),
        },
    )?;
    Ok(OntologyValidationFixture {
        task,
        observation_id: observation.id,
        signal_id,
        apply_action_id: apply_action.id,
        target_label_id: label.id,
        result_atom_id: apply_action.result_atom_id.context("result atom id")?,
        result_atom_content_hash: apply_action
            .result_atom_content_hash
            .context("result atom hash")?,
    })
}

fn seed_negative_validation_fixture_with_positive_control(
    temp: &TempDb,
    title: &str,
    signal_key: &str,
) -> anyhow::Result<(OntologyValidationFixture, TaskRecord)> {
    let label = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            applies_when: vec!["changes CLI user-visible behavior".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let control_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Positive control changes CLI user-visible behavior"),
    )?;
    let task = create_task(&temp.path, "default", "tester", CreateTask::ready(title))?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![LabelOntologySignalInput {
            kind: LabelOntologySignalKind::FalsePositive,
            target_label_ref: Some("cli".to_owned()),
            related_labels_json: "[]".to_owned(),
            proposed_action: LabelOntologyProposedAction::AddNegativeAtom,
            candidate_atom: Some(LabelOntologyCandidateAtomInput {
                polarity: "negative".to_owned(),
                kind: "excludes_when".to_owned(),
                text: "does not change user-visible CLI behavior".to_owned(),
            }),
            proposed_label_name: None,
            proposal_json: "{}".to_owned(),
            agent_selected: true,
            suggest_state: Some(LabelOntologySuggestState::Selected),
            suggest_score: Some(0.82),
            suggest_rank: Some(1),
            final_selected: false,
            rationale: "The task was a false positive for the cli label.".to_owned(),
            confidence: Some(0.9),
            signal_key: Some(signal_key.to_owned()),
        }]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Confirmed by reviewer.",
        ),
    )?;
    let apply_action = apply_label_ontology_atom(
        &temp.path,
        "default",
        LabelOntologyAtomApplyInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "excludes_when".to_owned(),
            text: "does not change user-visible CLI behavior".to_owned(),
            reason: "Confirmed false-positive support for CLI label suppression.".to_owned(),
        },
    )?;
    Ok((
        OntologyValidationFixture {
            task,
            observation_id: observation.id,
            signal_id,
            apply_action_id: apply_action.id,
            target_label_id: label.id,
            result_atom_id: apply_action.result_atom_id.context("result atom id")?,
            result_atom_content_hash: apply_action
                .result_atom_content_hash
                .context("result atom hash")?,
        },
        control_task,
    ))
}

fn passed_validation_input(
    fixture: &OntologyValidationFixture,
    reason: &str,
) -> LabelOntologyValidationInput {
    LabelOntologyValidationInput {
        actor: validation_actor(),
        parent_action_id: fixture.apply_action_id.clone(),
        signal_ids: Vec::new(),
        reason: reason.to_owned(),
        validation_status: LabelOntologyValidationStatus::Passed,
        validation_json: typed_positive_fixture_json(fixture).to_string(),
    }
}

fn failed_validation_input(
    fixture: &OntologyValidationFixture,
    reason: &str,
) -> LabelOntologyValidationInput {
    let mut input = passed_validation_input(fixture, reason);
    input.validation_status = LabelOntologyValidationStatus::Failed;
    input
}

fn validation_input_from_json(
    parent_action_id: &str,
    validation_json: serde_json::Value,
    reason: &str,
) -> LabelOntologyValidationInput {
    LabelOntologyValidationInput {
        actor: validation_actor(),
        parent_action_id: parent_action_id.to_owned(),
        signal_ids: Vec::new(),
        reason: reason.to_owned(),
        validation_status: LabelOntologyValidationStatus::Passed,
        validation_json: validation_json.to_string(),
    }
}

fn assert_external_passed_trusted_json_rejected(
    temp: &TempDb,
    parent_action_id: &str,
    validation_json: serde_json::Value,
    reason: &str,
) -> anyhow::Result<()> {
    let error = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        validation_input_from_json(parent_action_id, validation_json, reason),
    ))?;
    assert!(
        error
            .to_string()
            .contains("trusted evidence collected by the kanban tool"),
        "{error}"
    );
    Ok(())
}

fn seed_validation_action(
    temp: &TempDb,
    id: &str,
    board_id: &str,
    parent_action_id: &str,
    signal_ids: &[String],
    status: LabelOntologyValidationStatus,
    validation_json: serde_json::Value,
) -> anyhow::Result<String> {
    seed_validation_action_with(SeedValidationAction {
        temp,
        id,
        board_id,
        parent_action_id,
        signal_ids,
        status,
        validation_json,
        created_at: 123456,
    })
}

struct SeedValidationAction<'a> {
    temp: &'a TempDb,
    id: &'a str,
    board_id: &'a str,
    parent_action_id: &'a str,
    signal_ids: &'a [String],
    status: LabelOntologyValidationStatus,
    validation_json: serde_json::Value,
    created_at: i64,
}

fn seed_validation_action_with(input: SeedValidationAction<'_>) -> anyhow::Result<String> {
    let conn = connect_file(&input.temp.path)?;
    conn.execute(
        "INSERT INTO label_ontology_actions(
         id, board_id, parent_action_id, action_type, reason, target_label_id, result_label_id,
         result_atom_id, result_atom_content_hash, result_proposal_id, canonical_before_hash,
         canonical_after_hash, change_json, validation_status, validation_json, created_by,
         created_by_type, agent_type, created_at)
         VALUES (?1, ?2, ?3, 'validate', 'seeded validation fixture',
         NULL, NULL, NULL, NULL, NULL, NULL, NULL, '{}', ?4, ?5, 'test-fixture',
         'agent', 'codex', ?6)",
        params![
            input.id,
            input.board_id,
            input.parent_action_id,
            input.status.to_string(),
            input.validation_json.to_string(),
            input.created_at,
        ],
    )?;
    for signal_id in input.signal_ids {
        conn.execute(
            "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![input.board_id, input.id, signal_id, input.created_at],
        )?;
    }
    Ok(input.id.to_owned())
}

fn typed_positive_fixture_json(fixture: &OntologyValidationFixture) -> serde_json::Value {
    typed_positive_validation_json(
        &fixture.signal_id,
        &fixture.target_label_id,
        &fixture.result_atom_id,
        &fixture.result_atom_content_hash,
    )
}

fn typed_positive_validation_json(
    signal_id: &str,
    target_label_id: &str,
    result_atom_id: &str,
    result_atom_content_hash: &str,
) -> serde_json::Value {
    json!({
        "evidence_type": "trusted_automated",
        "embedding_model": "test-embedding-v1",
        "solver_options": {"candidate_limit": 24, "atom_limit": 64},
        "index": {"status": "ready", "dirty": false, "generation": 7},
        "cases": [{
            "signal_id": signal_id,
            "case_type": "positive_atom",
            "passed": true,
            "target_label_id": target_label_id,
            "before": {
                "target": {
                    "label_id": target_label_id,
                    "selected": false,
                    "score": 0.08
                },
                "coverage": 0.61
            },
            "after": {
                "degraded": false,
                "target": {
                    "label_id": target_label_id,
                    "selected": true,
                    "score": 0.74
                },
                "coverage": 0.79,
                "evidence_atoms": [{
                    "id": result_atom_id,
                    "content_hash": result_atom_content_hash,
                    "label_id": target_label_id
                }]
            }
        }]
    })
}

fn typed_positive_multi_validation_json(
    fixture: &MultiOntologyValidationFixture,
) -> serde_json::Value {
    let cases = fixture
        .signal_ids
        .iter()
        .map(|signal_id| {
            json!({
                "signal_id": signal_id,
                "case_type": "positive_atom",
                "passed": true,
                "target_label_id": fixture.target_label_id,
                "before": {
                    "target": {
                        "label_id": fixture.target_label_id,
                        "selected": false,
                        "score": 0.08
                    },
                    "coverage": 0.61
                },
                "after": {
                    "degraded": false,
                    "target": {
                        "label_id": fixture.target_label_id,
                        "selected": true,
                        "score": 0.74
                    },
                    "coverage": 0.79,
                    "evidence_atoms": [{
                        "id": fixture.result_atom_id,
                        "content_hash": fixture.result_atom_content_hash,
                        "label_id": fixture.target_label_id
                    }]
                }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "evidence_type": "trusted_automated",
        "embedding_model": "test-embedding-v1",
        "solver_options": {"candidate_limit": 24, "atom_limit": 64},
        "index": {"status": "ready", "dirty": false, "generation": 7},
        "cases": cases
    })
}

fn typed_negative_fixture_json(
    fixture: &OntologyValidationFixture,
    positive_control_passed: bool,
) -> serde_json::Value {
    json!({
        "evidence_type": "trusted_automated",
        "embedding_model": "test-embedding-v1",
        "solver_options": {"candidate_limit": 24, "atom_limit": 64},
        "index": {"status": "ready", "dirty": false, "generation": 7},
        "cases": [{
            "signal_id": fixture.signal_id,
            "case_type": "negative_atom",
            "passed": true,
            "target_label_id": fixture.target_label_id,
            "before": {
                "target": {
                    "label_id": fixture.target_label_id,
                    "selected": true,
                    "score": 0.82
                }
            },
            "after": {
                "degraded": false,
                "target": {
                    "label_id": fixture.target_label_id,
                    "selected": false,
                    "score": 0.18
                },
                "negative_evidence_atoms": [{
                    "id": fixture.result_atom_id,
                    "content_hash": fixture.result_atom_content_hash,
                    "label_id": fixture.target_label_id
                }],
                "positive_controls": [{
                    "task_ref": "default#control",
                    "passed": positive_control_passed,
                    "regressed": !positive_control_passed
                }]
            }
        }]
    })
}

fn assert_json_array_contains(value: &serde_json::Value, expected: &str) {
    let items = value
        .as_array()
        .unwrap_or_else(|| panic!("expected JSON array, got {value}"));
    assert!(
        items.iter().any(|item| item.as_str() == Some(expected)),
        "expected {expected:?} in {items:?}"
    );
}

fn assert_score_near(actual: Option<f64>, expected: f64) {
    let actual = actual.unwrap_or_else(|| panic!("expected score near {expected}"));
    assert!(
        (actual - expected).abs() < 0.000_001,
        "expected score near {expected}, got {actual}"
    );
}

fn sample_record_input(signals: Vec<LabelOntologySignalInput>) -> LabelOntologyRecordInput {
    LabelOntologyRecordInput {
        actor: LabelOntologyActor {
            name: "label-agent".to_owned(),
            actor_type: "agent".to_owned(),
            agent_type: Some("local".to_owned()),
        },
        agent_candidates_json: json!([
            {"label": "cli", "confidence": 0.92, "reason": "adds CLI command surface"}
        ])
        .to_string(),
        suggestion_snapshot_json: json!({
            "result": {"selected_labels": [], "candidates": []}
        })
        .to_string(),
        final_decision_json: json!({"accepted_labels": ["cli"]}).to_string(),
        suggest_coverage: Some(0.61),
        suggest_coverage_cosine: Some(0.74),
        suggest_residual_norm: Some(0.39),
        suggest_needs_new_label: false,
        suggest_degraded: false,
        diagnostics_json: "[]".to_owned(),
        capture_fingerprint: None,
        signals,
    }
}

fn insert_agreement_observation_fixture(
    temp: &TempDb,
    task: &TaskRecord,
    capture_fingerprint: &str,
) -> anyhow::Result<()> {
    let conn = connect_file(&temp.path)?;
    let created_at: i64 = conn.query_row(
        "SELECT COALESCE(MAX(created_at), 0) + 1 FROM label_ontology_observations",
        [],
        |row| row.get(0),
    )?;
    let observation_id = format!("lor_{capture_fingerprint}");
    conn.execute(
        "INSERT INTO label_ontology_observations(\
         id, board_id, task_id, task_ref_snapshot, task_snapshot_json, suggest_input_hash, \
         agent_candidates_json, suggestion_snapshot_json, final_decision_json, suggest_coverage, \
         suggest_coverage_cosine, suggest_residual_norm, suggest_needs_new_label, suggest_degraded, \
         diagnostics_json, capture_fingerprint, created_by, created_by_type, agent_type, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '[]', ?7, ?8, 0.91, 0.88, 0.09, 0, 0, '[]', ?9, 'tester', 'agent', 'quality-test', ?10)",
        params![
            observation_id,
            task.board_id,
            task.id,
            task.task_ref,
            json!({
                "id": task.id,
                "board_id": task.board_id,
                "ref": task.task_ref,
                "title": task.title,
                "description": task.description,
            })
            .to_string(),
            "agreementhash000",
            json!({"selected_labels": ["docs"], "candidates": []}).to_string(),
            json!({"accepted_labels": ["docs"], "agreement": true}).to_string(),
            capture_fingerprint,
            created_at,
        ],
    )?;
    Ok(())
}

fn ontology_quality_truth_counts(
    path: &Path,
) -> anyhow::Result<(i64, i64, i64, i64, i64, i64, i64)> {
    let conn = connect_file(path)?;
    Ok((
        table_count(&conn, "labels")?,
        table_count(&conn, "task_labels")?,
        table_count(&conn, "label_semantics")?,
        table_count(&conn, "label_atoms")?,
        table_count(&conn, "label_ontology_observations")?,
        table_count(&conn, "label_ontology_signals")?,
        table_count(&conn, "label_ontology_actions")?,
    ))
}

fn table_count(conn: &Connection, table: &str) -> anyhow::Result<i64> {
    let table = match table {
        "labels" => "labels",
        "task_labels" => "task_labels",
        "label_semantics" => "label_semantics",
        "label_atoms" => "label_atoms",
        "label_ontology_observations" => "label_ontology_observations",
        "label_ontology_signals" => "label_ontology_signals",
        "label_ontology_actions" => "label_ontology_actions",
        _ => return Err(test_error(format!("unsupported table count: {table}"))),
    };
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

struct OntologyLinkSchemaFixture {
    board_id: String,
    other_board_id: String,
    task_id: String,
    observation_id: String,
    signal_id: String,
    other_signal_id: String,
    action_id: String,
    other_action_id: String,
    proposal_id: String,
    other_proposal_id: String,
    other_label_id: String,
}

fn seed_ontology_link_schema_fixture(temp: &TempDb) -> anyhow::Result<OntologyLinkSchemaFixture> {
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
            name: "schema-default".to_owned(),
            color: None,
        },
    )?;
    let other_label = create_label(
        &temp.path,
        "other",
        CreateLabel {
            name: "schema-other".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("schema default task"),
    )?;
    let other_task = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("schema other task"),
    )?;
    let conn = connect_file(&temp.path)?;
    let observation_id = "lor_schema_default".to_owned();
    let other_observation_id = "lor_schema_other".to_owned();
    let signal_id = "los_schema_default".to_owned();
    let other_signal_id = "los_schema_other".to_owned();
    let action_id = "loa_schema_default".to_owned();
    let other_action_id = "loa_schema_other".to_owned();
    let proposal_id = "lp_schema_default".to_owned();
    let other_proposal_id = "lp_schema_other".to_owned();
    for (id, row_task, fingerprint) in [
        (&observation_id, &task, "schema-default-fingerprint"),
        (
            &other_observation_id,
            &other_task,
            "schema-other-fingerprint",
        ),
    ] {
        conn.execute(
            "INSERT INTO label_ontology_observations(
             id, board_id, task_id, task_ref_snapshot, task_snapshot_json, suggest_input_hash,
             agent_candidates_json, suggestion_snapshot_json, final_decision_json,
             diagnostics_json, capture_fingerprint, created_by, created_by_type, created_at)
             VALUES (?1, ?2, ?3, ?4, '{}', 'schemahash', '[]', '{}', '{}', '[]',
             ?5, 'tester', 'user', 1)",
            params![
                id,
                row_task.board_id,
                row_task.id,
                row_task.task_ref,
                fingerprint
            ],
        )?;
    }
    for (id, observation, board_id, label_id, signal_key) in [
        (
            &signal_id,
            &observation_id,
            &task.board_id,
            Some(label.id.as_str()),
            "schema-default-signal",
        ),
        (
            &other_signal_id,
            &other_observation_id,
            &other_task.board_id,
            Some(other_label.id.as_str()),
            "schema-other-signal",
        ),
    ] {
        conn.execute(
            "INSERT INTO label_ontology_signals(
             id, observation_id, board_id, kind, status, target_label_id, related_labels_json,
             proposed_action, proposal_json, agent_selected, final_selected, rationale, signal_key,
             created_at, updated_at)
             VALUES (?1, ?2, ?3, 'false_negative', 'open', ?4, '[]',
             'add_positive_atom', '{}', 1, 1, 'schema fixture signal', ?5, 1, 1)",
            params![id, observation, board_id, label_id, signal_key],
        )?;
    }
    for (id, row_task, name) in [
        (&proposal_id, &task, "schema-default-proposal"),
        (&other_proposal_id, &other_task, "schema-other-proposal"),
    ] {
        conn.execute(
            "INSERT INTO label_semantic_proposals(
             id, board_id, task_id, status, name, applies_when, excludes_when,
             positive_examples, negative_examples, heuristic_coverage,
             heuristic_coverage_cosine, heuristic_residual_norm, diagnostics_json,
             created_by, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'proposed', ?4, '[]', '[]', '[]', '[]',
             0.1, 0.1, 0.9, '[]', 'tester', 1, 1)",
            params![id, row_task.board_id, row_task.id, name],
        )?;
    }
    for (id, board_id, label_id, proposal_id) in [
        (
            &action_id,
            &task.board_id,
            Some(label.id.as_str()),
            Some(proposal_id.as_str()),
        ),
        (
            &other_action_id,
            &other_task.board_id,
            Some(other_label.id.as_str()),
            Some(other_proposal_id.as_str()),
        ),
    ] {
        conn.execute(
            "INSERT INTO label_ontology_actions(
             id, board_id, action_type, reason, target_label_id, result_label_id,
             result_proposal_id, change_json, validation_requirement, validation_status,
             validation_json, created_by, created_by_type, created_at)
             VALUES (?1, ?2, 'create_label_proposal', 'schema fixture action',
             ?3, ?3, ?4, '{}', 'none', 'not_required', '{}', 'tester', 'user', 1)",
            params![id, board_id, label_id, proposal_id],
        )?;
    }
    conn.execute(
        "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at)
         VALUES (?1, ?2, ?3, 1)",
        params![task.board_id, action_id, signal_id],
    )?;

    Ok(OntologyLinkSchemaFixture {
        board_id: task.board_id,
        other_board_id: other_board.id,
        task_id: task.id,
        observation_id,
        signal_id,
        other_signal_id,
        action_id,
        other_action_id,
        proposal_id,
        other_proposal_id,
        other_label_id: other_label.id,
    })
}

enum ExistingAtomApplyKind {
    Positive,
    Negative,
}

struct ExistingAtomApplyFixture {
    signal_ids: Vec<String>,
    target_label_id: String,
    atom_id: String,
    atom_content_hash: String,
    atom_kind: String,
    atom_text: String,
}

fn seed_existing_atom_apply_fixture(
    temp: &TempDb,
    kind: ExistingAtomApplyKind,
    signal_keys: Vec<&str>,
) -> anyhow::Result<ExistingAtomApplyFixture> {
    init_database(&temp.path, "tester")?;
    let label = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let (atom_kind, atom_text, semantics_input, signals) = match kind {
        ExistingAtomApplyKind::Positive => {
            let atom_kind = "applies_when".to_owned();
            let atom_text =
                "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned();
            let semantics_input = UpsertLabelSemantics {
                label_ref: "cli".to_owned(),
                applies_when: vec![atom_text.clone()],
                ..UpsertLabelSemantics::default()
            };
            let signals = signal_keys
                .into_iter()
                .map(sample_signal_input)
                .collect::<Vec<_>>();
            (atom_kind, atom_text, semantics_input, signals)
        }
        ExistingAtomApplyKind::Negative => {
            let atom_kind = "excludes_when".to_owned();
            let atom_text =
                "only changes unrelated release notes without touching CLI behavior".to_owned();
            let semantics_input = UpsertLabelSemantics {
                label_ref: "cli".to_owned(),
                excludes_when: vec![atom_text.clone()],
                ..UpsertLabelSemantics::default()
            };
            let signals = signal_keys
                .into_iter()
                .map(|signal_key| {
                    let mut signal = sample_signal_input(signal_key);
                    signal.proposed_action = LabelOntologyProposedAction::AddNegativeAtom;
                    signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
                        polarity: "negative".to_owned(),
                        kind: atom_kind.clone(),
                        text: atom_text.clone(),
                    });
                    signal.rationale =
                        "The task was a false positive for cli and needs negative evidence."
                            .to_owned();
                    signal
                })
                .collect::<Vec<_>>();
            (atom_kind, atom_text, semantics_input, signals)
        }
    };
    upsert_label_semantics(&temp.path, "default", semantics_input)?;
    let atom = list_label_atoms(&temp.path, "default")?
        .into_iter()
        .find(|atom| atom.kind == atom_kind && atom.text == atom_text)
        .context("seeded atom")?;
    let signal_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Existing ontology atom provenance source"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &signal_task.id,
        sample_record_input(signals),
    )?;
    let signal_ids = observation
        .signals
        .iter()
        .map(|signal| signal.id.clone())
        .collect::<Vec<_>>();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            signal_ids.clone(),
            "Confirmed existing atom provenance signal.",
        ),
    )?;
    Ok(ExistingAtomApplyFixture {
        signal_ids,
        target_label_id: label.id,
        atom_id: atom.id,
        atom_content_hash: atom.content_hash,
        atom_kind,
        atom_text,
    })
}

fn assert_existing_atom_adoption_action(
    action: &kanban_sqlite::LabelOntologyActionRecord,
    fixture: &ExistingAtomApplyFixture,
    requested_action_type: &str,
) -> anyhow::Result<()> {
    assert_eq!(
        action.action_type,
        LabelOntologyActionType::AdoptExistingAtom
    );
    assert_eq!(
        action.validation_status,
        LabelOntologyValidationStatus::NotRequired
    );
    assert_eq!(
        action.target_label_id.as_deref(),
        Some(fixture.target_label_id.as_str())
    );
    assert_eq!(
        action.result_atom_id.as_deref(),
        Some(fixture.atom_id.as_str())
    );
    assert_eq!(
        action.result_atom_content_hash.as_deref(),
        Some(fixture.atom_content_hash.as_str())
    );
    assert_eq!(action.canonical_before_hash, action.canonical_after_hash);
    let change: serde_json::Value = serde_json::from_str(&action.change_json)?;
    assert_eq!(change["canonical_changed"], false);
    assert_eq!(change["provenance_only"], true);
    assert_eq!(change["requested_action_type"], requested_action_type);
    assert_eq!(change["added_atom"]["id"], fixture.atom_id);
    assert_eq!(
        change["added_atom"]["content_hash"],
        fixture.atom_content_hash
    );
    assert_eq!(change["before"], change["after"]);
    Ok(())
}

fn reviewer_actor() -> LabelOntologyActor {
    LabelOntologyActor {
        name: "reviewer".to_owned(),
        actor_type: "user".to_owned(),
        agent_type: None,
    }
}

fn clear_label_atom_dirty_flags(path: &Path, board: &str) -> anyhow::Result<()> {
    let board = get_board(path, board)?;
    let conn = connect_file(path)?;
    conn.execute(
        "UPDATE derived_store_state SET dirty=0, last_error=NULL \
         WHERE store_name='lancedb_label_atoms'",
        [],
    )?;
    conn.execute(
        "UPDATE label_atom_index_boards SET dirty=0, last_error=NULL \
         WHERE store_name='lancedb_label_atoms' AND board_id=?1",
        [board.id],
    )?;
    Ok(())
}

fn add_atom_action_count(path: &Path) -> anyhow::Result<i64> {
    Ok(connect_file(path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions \
         WHERE action_type IN ('add_positive_atom','add_negative_atom')",
        [],
        |row| row.get(0),
    )?)
}

fn bootstrap_action_count_for_proposal(path: &Path, proposal_id: &str) -> anyhow::Result<i64> {
    Ok(connect_file(path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions \
         WHERE action_type='bootstrap_label' AND result_proposal_id=?1",
        [proposal_id],
        |row| row.get(0),
    )?)
}

fn ontology_action_atom_effect_count(
    path: &Path,
    action_id: &str,
    effect: Option<&str>,
) -> anyhow::Result<i64> {
    let conn = connect_file(path)?;
    let count = if let Some(effect) = effect {
        conn.query_row(
            "SELECT COUNT(*) FROM label_ontology_action_atom_effects \
             WHERE action_id=?1 AND effect=?2",
            params![action_id, effect],
            |row| row.get(0),
        )?
    } else {
        conn.query_row(
            "SELECT COUNT(*) FROM label_ontology_action_atom_effects WHERE action_id=?1",
            [action_id],
            |row| row.get(0),
        )?
    };
    Ok(count)
}

fn ontology_action_atom_effect_hashes(
    path: &Path,
    action_id: &str,
    effect: &str,
) -> anyhow::Result<Vec<String>> {
    connect_file(path)?
        .prepare(
            "SELECT atom_content_hash FROM label_ontology_action_atom_effects \
             WHERE action_id=?1 AND effect=?2 ORDER BY created_at ASC, atom_content_hash ASC",
        )?
        .query_map(params![action_id, effect], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn assert_semantics_content_eq(
    actual: &kanban_sqlite::LabelSemanticsRecord,
    expected: &kanban_sqlite::LabelSemanticsRecord,
) {
    assert_eq!(actual.semantics_hash, expected.semantics_hash);
    assert_eq!(actual.description, expected.description);
    assert_eq!(actual.applies_when, expected.applies_when);
    assert_eq!(actual.excludes_when, expected.excludes_when);
    assert_eq!(actual.positive_examples, expected.positive_examples);
    assert_eq!(actual.negative_examples, expected.negative_examples);
    assert_eq!(
        actual
            .atoms
            .iter()
            .map(|atom| (
                atom.polarity.as_str(),
                atom.kind.as_str(),
                atom.text.as_str(),
                atom.ordinal,
                atom.content_hash.as_str(),
            ))
            .collect::<Vec<_>>(),
        expected
            .atoms
            .iter()
            .map(|atom| (
                atom.polarity.as_str(),
                atom.kind.as_str(),
                atom.text.as_str(),
                atom.ordinal,
                atom.content_hash.as_str(),
            ))
            .collect::<Vec<_>>()
    );
}

fn ontology_action_count(path: &Path) -> anyhow::Result<i64> {
    Ok(
        connect_file(path)?.query_row(
            "SELECT COUNT(*) FROM label_ontology_actions",
            [],
            |row| row.get(0),
        )?,
    )
}

fn structure_label_names(path: &Path) -> anyhow::Result<Vec<String>> {
    connect_file(path)?
        .prepare("SELECT name FROM labels ORDER BY name")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn structure_task_label_rows(path: &Path) -> anyhow::Result<Vec<(String, String)>> {
    connect_file(path)?
        .prepare(
            "SELECT t.title, l.name \
             FROM task_labels tl \
             JOIN tasks t ON t.id=tl.task_id \
             JOIN labels l ON l.id=tl.label_id \
             ORDER BY t.title, l.name",
        )?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn label_atom_store_dirty(path: &Path) -> anyhow::Result<bool> {
    let stores = derived_store_statuses(path)?;
    Ok(stores
        .iter()
        .find(|store| store.store_name == "lancedb_label_atoms")
        .ok_or_else(|| test_error("missing lancedb_label_atoms"))?
        .dirty)
}

fn label_atom_board_dirty(path: &Path, board: &str) -> anyhow::Result<bool> {
    let board = get_board(path, board)?;
    let count: i64 = connect_file(path)?.query_row(
        "SELECT COUNT(*) FROM label_atom_index_boards \
         WHERE store_name='lancedb_label_atoms' AND board_id=?1 AND dirty<>0",
        [board.id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

struct LabelOntologyReviewFixture {
    cli_open_signal_id: String,
    cli_confirmed_signal_id: String,
    open_gap_signal_id: String,
    rejected_gap_signal_id: String,
    confirm_action_id: String,
}

fn seed_label_ontology_review_fixture(temp: &TempDb) -> anyhow::Result<LabelOntologyReviewFixture> {
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "docs".to_owned(),
            color: None,
        },
    )?;
    let cli_task_a = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("CLI review source A"),
    )?;
    let cli_task_b = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("CLI review source B"),
    )?;
    let docs_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Docs review source"),
    )?;
    let gap_task_a = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Ontology gap source A"),
    )?;
    let gap_task_b = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Ontology gap source B"),
    )?;

    let cli_open_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &cli_task_a.id,
        review_record_input(vec![review_label_signal(
            "cli-open",
            "cli",
            "adds CLI commands",
            0.2,
        )]),
    )?;
    let mut degraded_cli_record = review_record_input(vec![review_label_signal(
        "cli-confirmed",
        "cli",
        "adds CLI commands",
        0.4,
    )]);
    degraded_cli_record.suggest_degraded = true;
    let cli_confirmed_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &cli_task_b.id,
        degraded_cli_record,
    )?;
    let docs_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &docs_task.id,
        review_record_input(vec![
            review_label_signal("docs-a", "docs", "updates docs", 0.9),
            review_label_signal("docs-b", "docs", "documents behavior", 0.8),
        ]),
    )?;
    assert_eq!(docs_observation.signals.len(), 2);
    let open_gap_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &gap_task_a.id,
        review_record_input(vec![review_gap_signal("gap-open", "Ontology Ledger", 0.1)]),
    )?;
    let rejected_gap_observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &gap_task_b.id,
        review_record_input(vec![review_gap_signal(
            "gap-rejected",
            "Ontology Ledger",
            0.12,
        )]),
    )?;

    let cli_open_signal_id = cli_open_observation.signals[0].id.clone();
    let cli_confirmed_signal_id = cli_confirmed_observation.signals[0].id.clone();
    let open_gap_signal_id = open_gap_observation.signals[0].id.clone();
    let rejected_gap_signal_id = rejected_gap_observation.signals[0].id.clone();
    let confirm_action = create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![cli_confirmed_signal_id.clone()],
            "confirm CLI review group signal",
        ),
    )?;
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Reject,
            vec![rejected_gap_signal_id.clone()],
            "reject weak proposed label signal",
        ),
    )?;

    Ok(LabelOntologyReviewFixture {
        cli_open_signal_id,
        cli_confirmed_signal_id,
        open_gap_signal_id,
        rejected_gap_signal_id,
        confirm_action_id: confirm_action.id,
    })
}

fn review_record_input(signals: Vec<LabelOntologySignalInput>) -> LabelOntologyRecordInput {
    let mut input = sample_record_input(signals);
    input.capture_fingerprint = None;
    input
}

fn review_label_signal(
    signal_key: &str,
    label: &str,
    candidate_text: &str,
    score: f64,
) -> LabelOntologySignalInput {
    let mut signal = sample_signal_input(signal_key);
    signal.target_label_ref = Some(label.to_owned());
    signal.candidate_atom = Some(LabelOntologyCandidateAtomInput {
        polarity: "positive".to_owned(),
        kind: "applies_when".to_owned(),
        text: candidate_text.to_owned(),
    });
    signal.suggest_score = Some(score);
    signal
}

fn review_gap_signal(
    signal_key: &str,
    proposed_label_name: &str,
    score: f64,
) -> LabelOntologySignalInput {
    let mut signal = sample_signal_input(signal_key);
    signal.kind = LabelOntologySignalKind::VocabularyGap;
    signal.target_label_ref = None;
    signal.proposed_action = LabelOntologyProposedAction::BootstrapLabel;
    signal.candidate_atom = None;
    signal.proposed_label_name = Some(proposed_label_name.to_owned());
    signal.proposal_json = json!({
        "name": proposed_label_name,
        "description": "Label ontology ledger work"
    })
    .to_string();
    signal.suggest_score = Some(score);
    signal
}

fn review_empty_target_signal(
    signal_key: &str,
    label: &str,
    kind: LabelOntologySignalKind,
    proposed_action: LabelOntologyProposedAction,
    score: f64,
) -> LabelOntologySignalInput {
    let mut signal = sample_signal_input(signal_key);
    signal.kind = kind;
    signal.target_label_ref = Some(label.to_owned());
    signal.related_labels_json = "[]".to_owned();
    signal.proposed_action = proposed_action;
    signal.candidate_atom = None;
    signal.proposed_label_name = None;
    signal.proposal_json = "{}".to_owned();
    signal.suggest_score = Some(score);
    signal.rationale = "Empty candidate signal used to review grouping boundaries.".to_owned();
    signal
}

fn seed_label_semantic_proposal(
    path: &Path,
    board_id: &str,
    task_id: &str,
    name: &str,
) -> anyhow::Result<String> {
    seed_label_semantic_proposal_with_semantics(
        path,
        board_id,
        task_id,
        name,
        Some("Label ontology ledger work"),
        &["records ontology observations and signals".to_owned()],
        &["label ontology ledger migration".to_owned()],
    )
}

fn seed_label_semantic_proposal_with_semantics(
    path: &Path,
    board_id: &str,
    task_id: &str,
    name: &str,
    description: Option<&str>,
    applies_when: &[String],
    positive_examples: &[String],
) -> anyhow::Result<String> {
    let conn = connect_file(path)?;
    let id = format!("lp_test_{name}");
    conn.execute(
        "INSERT INTO label_semantic_proposals(
         id, board_id, task_id, status, name, description, applies_when, excludes_when,
         positive_examples, negative_examples, heuristic_coverage, heuristic_coverage_cosine,
         heuristic_residual_norm, diagnostics_json, created_by, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'proposed', ?4, ?5, ?6, '[]', ?7, '[]', 0.1, 0.2, 0.9, '[]', 'tester', 1, 1)",
        params![
            id,
            board_id,
            task_id,
            name,
            description,
            json!(applies_when).to_string(),
            json!(positive_examples).to_string(),
        ],
    )?;
    Ok(id)
}

fn seed_pending_mutation_action_without_evidence(
    path: &Path,
    board_id: &str,
    signal_id: &str,
) -> anyhow::Result<String> {
    let conn = connect_file(path)?;
    let id = "loa_missing_evidence".to_owned();
    conn.execute(
        "INSERT INTO label_ontology_actions(
         id, board_id, parent_action_id, action_type, reason, target_label_id, result_label_id,
         result_atom_id, result_atom_content_hash, result_proposal_id, canonical_before_hash,
         canonical_after_hash, change_json, validation_requirement, validation_status,
         validation_json, created_by, created_by_type, agent_type, created_at)
         VALUES (?1, ?2, NULL, 'add_positive_atom', 'missing canonical evidence',
         NULL, NULL, NULL, NULL, NULL, NULL, NULL, '{}', 'required', 'pending', '{}',
         'tester', 'user', NULL, 1)",
        params![id, board_id],
    )?;
    conn.execute(
        "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at)
         VALUES (?1, ?2, ?3, 1)",
        params![board_id, id, signal_id],
    )?;
    Ok(id)
}

fn sample_signal_input(signal_key: &str) -> LabelOntologySignalInput {
    LabelOntologySignalInput {
        kind: LabelOntologySignalKind::FalseNegative,
        target_label_ref: Some("cli".to_owned()),
        related_labels_json: "[]".to_owned(),
        proposed_action: LabelOntologyProposedAction::AddPositiveAtom,
        candidate_atom: Some(LabelOntologyCandidateAtomInput {
            polarity: "positive".to_owned(),
            kind: "applies_when".to_owned(),
            text: "extends CLI subcommands, arguments, help output, or JSON behavior".to_owned(),
        }),
        proposed_label_name: None,
        proposal_json: "{}".to_owned(),
        agent_selected: true,
        suggest_state: Some(LabelOntologySuggestState::Candidate),
        suggest_score: Some(0.08),
        suggest_rank: Some(4),
        final_selected: true,
        rationale: "The task expands the CLI surface although suggest scored cli weakly."
            .to_owned(),
        confidence: Some(0.91),
        signal_key: Some(signal_key.to_owned()),
    }
}

fn record_confirmed_test_signal(
    temp: &TempDb,
    task_id: &str,
    signal: LabelOntologySignalInput,
) -> anyhow::Result<String> {
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        task_id,
        sample_record_input(vec![signal]),
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer confirmed source signal.",
        ),
    )?;
    Ok(signal_id)
}

fn validation_actor() -> LabelOntologyActor {
    LabelOntologyActor {
        name: "validator".to_owned(),
        actor_type: "agent".to_owned(),
        agent_type: Some("local".to_owned()),
    }
}

fn supersede_input(
    signal_ids: Vec<String>,
    replacement_signal_id: &str,
    reason: &str,
) -> LabelOntologyActionInput {
    let mut input = action_input(LabelOntologyActionType::Supersede, signal_ids, reason);
    input.superseded_by_signal_id = Some(replacement_signal_id.to_owned());
    input
}

fn action_input(
    action_type: LabelOntologyActionType,
    signal_ids: Vec<String>,
    reason: &str,
) -> LabelOntologyActionInput {
    LabelOntologyActionInput {
        actor: LabelOntologyActor {
            name: "reviewer".to_owned(),
            actor_type: "user".to_owned(),
            agent_type: None,
        },
        action_type,
        signal_ids,
        reason: reason.to_owned(),
        superseded_by_signal_id: None,
        parent_action_id: None,
        target_label_ref: None,
        result_label_ref: None,
        result_atom_id: None,
        result_atom_content_hash: None,
        result_proposal_id: None,
        canonical_before_hash: None,
        canonical_after_hash: None,
        change_json: None,
        validation_status: None,
        validation_json: None,
    }
}
