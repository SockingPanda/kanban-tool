use crate::common::*;

use serde_json::json;

#[test]
fn label_ontology_migration_creates_ledger_tables_and_json_constraints() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_migration_creates_ledger_tables_and_json_constraints")?;

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 12);
    for table in [
        "label_ontology_observations",
        "label_ontology_signals",
        "label_ontology_actions",
        "label_ontology_action_signals",
    ] {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |row| row.get(0),
        )?;
        assert_eq!(count, 1, "missing table {table}");
    }

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
            validation_status: LabelOntologyValidationStatus::Failed,
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
            validation_status: LabelOntologyValidationStatus::Failed,
            validation_json: "{}".to_owned(),
        },
    ))?;
    assert!(error.to_string().contains("canonical mutation evidence"));

    Ok(())
}

#[test]
fn label_ontology_passed_validation_rejects_empty_or_untyped_evidence() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_passed_validation_rejects_empty_or_untyped_evidence")?;
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
    assert!(error.to_string().contains("structured validation evidence"));

    Ok(())
}

#[test]
fn label_ontology_atom_apply_records_provenance_and_validation_resolves_signal()
-> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_atom_apply_records_provenance_and_validation_resolves_signal")?;
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
            validation_status: LabelOntologyValidationStatus::Passed,
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
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: json!({
                "cases": [{
                    "signal_id": signal_id,
                    "passed": true,
                    "after": {"state": "selected"}
                }]
            })
            .to_string(),
        },
    )?;

    assert_eq!(validation.action_type, LabelOntologyActionType::Validate);
    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Passed
    );
    let validation_json: serde_json::Value = serde_json::from_str(&validation.validation_json)?;
    assert_eq!(validation_json["summary"]["case_count"], 1);
    assert_eq!(validation_json["summary"]["stale_count"], 0);
    assert_eq!(validation_json["cases"][0]["signal_id"], signal_id);
    assert_eq!(validation_json["cases"][0]["comparable"], true);
    assert_eq!(validation_json["cases"][0]["before"]["score"], 0.08);
    assert_eq!(validation_json["manual"]["cases"][0]["passed"], true);
    let resolved = get_label_ontology_signal(&temp.path, &signal_id)?;
    assert_eq!(resolved.signal.status, LabelOntologySignalStatus::Resolved);
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
            reason: "Already resolved signals should not be validated again.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: json!({"cases": []}).to_string(),
        },
    ))?;
    assert!(
        repeated_validation
            .to_string()
            .contains("invalid transition")
    );

    Ok(())
}

#[test]
fn label_ontology_validation_treats_label_binding_changes_as_stale() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_validation_treats_label_binding_changes_as_stale")?;
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
        CreateTask::ready("Add ontology validation stale-label guard"),
    )?;
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-label-stale")]),
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
    kanban_sqlite::add_task_label(&temp.path, "default", "tester", &task.id, "backend")?;
    let rerecorded = record_label_ontology_observation(
        &temp.path,
        "default",
        &task.id,
        sample_record_input(vec![sample_signal_input("cli-label-stale-rerecord")]),
    )?;
    assert_ne!(
        rerecorded.capture_fingerprint,
        observation.capture_fingerprint
    );

    let stale_validation = result_err(validate_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: LabelOntologyActor {
                name: "validator".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            parent_action_id: apply_action.id,
            signal_ids: Vec::new(),
            reason: "Validation must reject stale task-label snapshot.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: json!({
                "cases": [{
                    "signal_id": signal_id,
                    "passed": true,
                    "after": {"state": "selected"}
                }]
            })
            .to_string(),
        },
    ))?;
    assert!(
        stale_validation
            .to_string()
            .contains("passed validation requires comparable")
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
        .find(|action| action.action_type == LabelOntologyActionType::BootstrapLabel)
        .context("bootstrap action")?;
    assert_eq!(bootstrap.signal_ids, vec![signal_id.clone()]);
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
    let semantics = get_label_semantics(&temp.path, "default", result_label_id)?;
    assert_eq!(semantics.label_name, "ontology-ledger");

    Ok(())
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

fn seed_label_semantic_proposal(
    path: &Path,
    board_id: &str,
    task_id: &str,
    name: &str,
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
            "Label ontology ledger work",
            json!(["records ontology observations and signals"]).to_string(),
            json!(["label ontology ledger migration"]).to_string(),
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
         canonical_after_hash, change_json, validation_status, validation_json, created_by,
         created_by_type, agent_type, created_at)
         VALUES (?1, ?2, NULL, 'add_positive_atom', 'missing canonical evidence',
         NULL, NULL, NULL, NULL, NULL, NULL, NULL, '{}', 'pending', '{}', 'tester', 'user',
         NULL, 1)",
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
