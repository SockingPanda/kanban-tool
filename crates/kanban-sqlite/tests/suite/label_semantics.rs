use crate::common::*;
use rusqlite::OptionalExtension;
use serde_json::json;

#[test]
fn task_label_suggestions_degrade_when_vector_store_disabled() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_suggestions_degrade_when_vector_store_disabled")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("label suggestion target"),
    )?;

    let result = kanban_sqlite::suggest_task_labels(
        &temp.path,
        "default",
        &task.id,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;

    assert_eq!(result.task_id, task.id);
    assert!(result.degraded);
    assert!(result.selected_labels.is_empty());
    assert_eq!(result.coverage, 0.0);
    assert_eq!(result.coverage_cosine, 0.0);
    assert_eq!(result.residual_norm, 1.0);
    assert!(!result.needs_new_label);
    assert_eq!(
        result.reason_codes,
        vec!["degraded_result", "vector_store_disabled"]
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "vector_store_disabled")
    );
    Ok(())
}

#[test]
fn label_proposal_migration_and_provider_unavailable_are_non_polluting() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_migration_and_provider_unavailable_are_non_polluting")?;
    init_database(&temp.path, "tester")?;
    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 18);
    let has_table: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='label_semantic_proposals'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(has_table, 1);
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("proposal degraded target"),
    )?;

    let attempt = propose_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;

    assert!(attempt.degraded);
    assert!(attempt.proposal.is_none());
    assert!(
        attempt
            .diagnostics
            .iter()
            .any(|code| code == "label_proposal_provider_unavailable")
    );
    assert_eq!(table_count(&conn, "labels")?, 0);
    assert_eq!(table_count(&conn, "label_semantics")?, 0);
    assert_eq!(table_count(&conn, "label_atoms")?, 0);
    assert_eq!(table_count(&conn, "task_labels")?, 0);
    Ok(())
}

#[test]
fn label_proposal_manual_candidate_accepts_without_task_binding() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_manual_candidate_accepts_without_task_binding")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("proposal manual target"),
    )?;
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "database".to_owned(),
        description: Some("Database persistence work".to_owned()),
        applies_when: vec!["touches SQLite migrations".to_owned()],
        excludes_when: vec!["frontend-only work".to_owned()],
        positive_examples: vec!["new table migration".to_owned()],
        negative_examples: vec!["CSS polish".to_owned()],
    });
    let store = ProposalValidationStore::new(vec![
        ("proposal manual target", vec![0.0, 1.0, 0.0]),
        ("database", vec![0.0, 1.0, 0.0]),
        ("Database persistence work", vec![0.0, 1.0, 0.0]),
        ("touches SQLite migrations", vec![0.0, 1.0, 0.0]),
        ("new table migration", vec![0.0, 1.0, 0.0]),
    ]);

    let attempt = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;
    let proposal = attempt.proposal.context("proposal")?;
    assert_eq!(proposal.status, LabelProposalStatus::Proposed);
    assert_eq!(proposal.heuristic_residual_norm, 1.0);

    let accepted = accept_label_proposal(
        &temp.path,
        "tester",
        &proposal.id,
        Some("覆盖不足，接受新 label".to_owned()),
    )?;

    assert_eq!(accepted.status, LabelProposalStatus::Accepted);
    let label_id = accepted.resolved_label_id.context("resolved label")?;
    let semantics = get_label_semantics(&temp.path, "default", &label_id)?;
    assert_eq!(semantics.label_name, "database");
    assert!(
        semantics
            .atoms
            .iter()
            .any(|atom| atom.kind == "applies_when")
    );
    let atom = semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;
    let conn = connect_file(&temp.path)?;
    let bootstrap_action_id: String = conn.query_row(
        "SELECT id FROM label_ontology_actions \
         WHERE action_type='bootstrap_label' AND result_proposal_id=?1",
        [&proposal.id],
        |row| row.get(0),
    )?;
    assert_eq!(
        ontology_action_atom_effect_count(&conn, &bootstrap_action_id)?,
        semantics.atoms.len() as i64
    );
    assert!(
        ontology_action_atom_effect_texts(&conn, &bootstrap_action_id, "added")?
            .contains(&atom.text)
    );
    assert!(
        get_task(&temp.path, "default", &task.id)?.labels.is_empty(),
        "accept must not attach task_labels"
    );
    assert_eq!(table_count(&conn, "task_labels")?, 0);
    Ok(())
}

#[test]
fn label_proposal_create_writes_ontology_action() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_create_writes_ontology_action")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Add ontology proposal provenance"),
    )?;
    let signal_id =
        record_confirmed_proposal_source_signal(&temp, &task.id, "ontology-ledger", "create-gap")?;
    let provider = ontology_proposal_provider("ontology-ledger");
    let store = ontology_proposal_store(&task.title, "ontology-ledger");

    let attempt = propose_task_label_with_store_and_create_options(
        &temp.path,
        "default",
        "reviewer",
        &task.id,
        &provider,
        &store,
        LabelProposalProposeOptions {
            suggestion: kanban_sqlite::LabelSuggestionOptions::default(),
            create: LabelProposalCreateOptions {
                source_signal_ids: vec![signal_id.clone()],
                ontology_actor: Some(LabelOntologyActor {
                    name: "ontology-agent".to_owned(),
                    actor_type: "agent".to_owned(),
                    agent_type: Some("codex".to_owned()),
                }),
                allow_retarget: false,
                retarget_reason: None,
            },
        },
    )?;

    let proposal = attempt.proposal.context("proposal")?;
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let create_action = detail
        .actions
        .iter()
        .find(|action| action.action_type == LabelOntologyActionType::CreateLabelProposal)
        .context("create_label_proposal action")?;
    assert_eq!(create_action.signal_ids, vec![signal_id.clone()]);
    assert_eq!(create_action.created_by, "ontology-agent");
    assert_eq!(create_action.created_by_type, "agent");
    assert_eq!(create_action.agent_type.as_deref(), Some("codex"));
    assert_eq!(
        create_action.result_proposal_id.as_deref(),
        Some(proposal.id.as_str())
    );
    assert_eq!(
        create_action.validation_status,
        LabelOntologyValidationStatus::NotRequired
    );
    let change: serde_json::Value = serde_json::from_str(&create_action.change_json)?;
    assert_eq!(change["proposal"]["id"], proposal.id);
    assert_eq!(change["proposal"]["name"], "ontology-ledger");
    assert_eq!(change["proposal"]["status"], "proposed");
    assert_eq!(change["retarget_override"], serde_json::Value::Null);

    Ok(())
}

#[test]
fn label_proposal_create_rejects_unrelated_source_signal() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_create_rejects_unrelated_source_signal")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject unrelated proposal creation source signal"),
    )?;
    let signal_id =
        record_confirmed_proposal_source_signal(&temp, &task.id, "backend-ledger", "create-bad")?;
    let provider = ontology_proposal_provider("ontology-ledger");
    let store = ontology_proposal_store(&task.title, "ontology-ledger");

    let error = result_err(propose_task_label_with_store_and_create_options(
        &temp.path,
        "default",
        "reviewer",
        &task.id,
        &provider,
        &store,
        LabelProposalProposeOptions {
            suggestion: kanban_sqlite::LabelSuggestionOptions::default(),
            create: LabelProposalCreateOptions {
                source_signal_ids: vec![signal_id.clone()],
                ontology_actor: None,
                allow_retarget: false,
                retarget_reason: None,
            },
        },
    ))?;
    assert!(error.to_string().contains(&signal_id), "{error}");
    assert!(error.to_string().contains("proposed label"), "{error}");
    assert!(
        list_label_proposals(&temp.path, "default", LabelProposalListOptions::default())?
            .is_empty()
    );

    Ok(())
}

#[test]
fn label_proposal_create_rejects_non_bootstrap_signal_even_with_retarget() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_proposal_create_rejects_non_bootstrap_signal_even_with_retarget")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject non bootstrap proposal creation source signal"),
    )?;
    let signal_id = record_confirmed_atom_source_signal(&temp, &task.id, "database")?;
    let provider = ontology_proposal_provider("ontology-ledger");
    let store = ontology_proposal_store(&task.title, "ontology-ledger");

    let error = result_err(propose_task_label_with_store_and_create_options(
        &temp.path,
        "default",
        "reviewer",
        &task.id,
        &provider,
        &store,
        LabelProposalProposeOptions {
            suggestion: kanban_sqlite::LabelSuggestionOptions::default(),
            create: LabelProposalCreateOptions {
                source_signal_ids: vec![signal_id.clone()],
                ontology_actor: None,
                allow_retarget: true,
                retarget_reason: Some("Reviewer is testing retarget guard.".to_owned()),
            },
        },
    ))?;
    assert!(error.to_string().contains(&signal_id), "{error}");
    assert!(error.to_string().contains("vocabulary_gap"), "{error}");
    assert!(
        list_label_proposals(&temp.path, "default", LabelProposalListOptions::default())?
            .is_empty()
    );

    Ok(())
}

#[test]
fn label_proposal_reject_keeps_create_source_signal_provenance() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_reject_keeps_create_source_signal_provenance")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject proposal but keep provenance"),
    )?;
    let signal_id =
        record_confirmed_proposal_source_signal(&temp, &task.id, "ontology-ledger", "reject-gap")?;
    let provider = ontology_proposal_provider("ontology-ledger");
    let store = ontology_proposal_store(&task.title, "ontology-ledger");
    let proposal = propose_task_label_with_store_and_create_options(
        &temp.path,
        "default",
        "reviewer",
        &task.id,
        &provider,
        &store,
        LabelProposalProposeOptions {
            suggestion: kanban_sqlite::LabelSuggestionOptions::default(),
            create: LabelProposalCreateOptions {
                source_signal_ids: vec![signal_id.clone()],
                ontology_actor: None,
                allow_retarget: false,
                retarget_reason: None,
            },
        },
    )?
    .proposal
    .context("proposal")?;

    let rejected = reject_label_proposal(
        &temp.path,
        "reviewer",
        &proposal.id,
        Some("Reviewer rejected the proposed vocabulary boundary.".to_owned()),
    )?;

    assert_eq!(rejected.status, LabelProposalStatus::Rejected);
    assert_eq!(
        rejected.decision_reason.as_deref(),
        Some("Reviewer rejected the proposed vocabulary boundary.")
    );
    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let create_action = detail
        .actions
        .iter()
        .find(|action| action.action_type == LabelOntologyActionType::CreateLabelProposal)
        .context("create_label_proposal action")?;
    assert_eq!(
        create_action.result_proposal_id.as_deref(),
        Some(proposal.id.as_str())
    );

    Ok(())
}

#[test]
fn label_proposal_accept_rejects_non_bootstrap_signal_even_with_retarget() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_proposal_accept_rejects_non_bootstrap_signal_even_with_retarget")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Reject non bootstrap proposal accept source signal"),
    )?;
    let signal_id = record_confirmed_atom_source_signal(&temp, &task.id, "database")?;
    let provider = ontology_proposal_provider("ontology-ledger");
    let store = ontology_proposal_store(&task.title, "ontology-ledger");
    let proposal = propose_task_label_with_store(
        &temp.path,
        "default",
        "reviewer",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?
    .proposal
    .context("proposal")?;
    let labels_before = list_labels(&temp.path, "default")?;
    let semantics_before = get_label_semantics(&temp.path, "default", "database")?;
    let atoms_before = list_label_atoms(&temp.path, "default")?;
    let conn = connect_file(&temp.path)?;
    let task_labels_before = table_count(&conn, "task_labels")?;

    let error = result_err(accept_label_proposal_with_options(
        &temp.path,
        "reviewer",
        &proposal.id,
        Some("Accept proposal after provenance review.".to_owned()),
        LabelProposalDecisionOptions {
            source_signal_ids: vec![signal_id.clone()],
            ontology_actor: None,
            allow_retarget: true,
            retarget_reason: Some("Reviewer is testing retarget guard.".to_owned()),
        },
    ))?;
    assert!(error.to_string().contains(&signal_id), "{error}");
    assert!(error.to_string().contains("vocabulary_gap"), "{error}");
    let proposal_after = get_label_proposal(&temp.path, &proposal.id)?;
    assert_eq!(proposal_after.status, LabelProposalStatus::Proposed);
    assert!(proposal_after.resolved_label_id.is_none());
    assert!(
        list_labels(&temp.path, "default")?
            .iter()
            .all(|label| label.name != proposal.name),
        "failed proposal adoption must not leave the proposal label"
    );
    assert_eq!(list_labels(&temp.path, "default")?, labels_before);
    assert_eq!(
        get_label_semantics(&temp.path, "default", "database")?,
        semantics_before
    );
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "task_labels")?, task_labels_before);

    Ok(())
}

#[test]
fn label_proposal_accept_links_creation_and_bootstrap_history() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_accept_links_creation_and_bootstrap_history")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("Accept proposal with creation provenance"),
    )?;
    let signal_id =
        record_confirmed_proposal_source_signal(&temp, &task.id, "ontology-ledger", "accept-gap")?;
    let provider = ontology_proposal_provider("ontology-ledger");
    let store = ontology_proposal_store(&task.title, "ontology-ledger");
    let proposal = propose_task_label_with_store_and_create_options(
        &temp.path,
        "default",
        "reviewer",
        &task.id,
        &provider,
        &store,
        LabelProposalProposeOptions {
            suggestion: kanban_sqlite::LabelSuggestionOptions::default(),
            create: LabelProposalCreateOptions {
                source_signal_ids: vec![signal_id.clone()],
                ontology_actor: None,
                allow_retarget: false,
                retarget_reason: None,
            },
        },
    )?
    .proposal
    .context("proposal")?;

    accept_label_proposal_with_options(
        &temp.path,
        "reviewer",
        &proposal.id,
        Some("Accept proposal after provenance review.".to_owned()),
        LabelProposalDecisionOptions {
            source_signal_ids: vec![signal_id.clone()],
            ontology_actor: None,
            allow_retarget: false,
            retarget_reason: None,
        },
    )?;

    let detail = get_label_ontology_signal(&temp.path, &signal_id)?;
    let create_action = detail
        .actions
        .iter()
        .find(|action| action.action_type == LabelOntologyActionType::CreateLabelProposal)
        .context("create_label_proposal action")?;
    let bootstrap = detail
        .actions
        .iter()
        .find(|action| {
            action.action_type == LabelOntologyActionType::BootstrapLabel
                && action.result_atom_id.is_none()
        })
        .context("bootstrap action")?;
    assert_eq!(
        bootstrap.parent_action_id.as_deref(),
        Some(create_action.id.as_str())
    );
    assert_eq!(
        bootstrap.result_proposal_id.as_deref(),
        Some(proposal.id.as_str())
    );
    assert_eq!(bootstrap.signal_ids, vec![signal_id]);

    Ok(())
}

#[test]
fn label_bootstrap_attaches_task_and_rejects_existing_semantics_replacement() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_bootstrap_attaches_task_and_rejects_existing_semantics_replacement")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bootstrap target"),
    )?;

    let first = bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            positive_examples: vec!["new table migration".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    )?;

    assert_eq!(first.task.labels.len(), 1);
    assert_eq!(first.task.labels[0].name, "database");
    assert_eq!(first.semantics.label_name, "database");
    assert_eq!(
        first.semantics.description.as_deref(),
        Some("Database persistence work")
    );
    assert!(
        first
            .semantics
            .atoms
            .iter()
            .any(|atom| atom.kind == "applies_when")
    );
    let status = kanban_sqlite::label_atom_index_status(&temp.path, "default")?;
    assert_eq!(status.board_dirty, Some(true));
    let before_atoms = first.semantics.atoms.clone();

    let error = result_err(bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database and migration work".to_owned()),
            excludes_when: vec!["UI-only polish".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    ))?;

    assert_eq!(
        error.to_string(),
        "invalid input: label bootstrap would replace existing semantics for label database; use a dedicated semantics mutation or proposal adoption path"
    );
    let after = get_label_semantics(&temp.path, "default", "database")?;
    assert_eq!(
        after.description.as_deref(),
        Some("Database persistence work")
    );
    assert_eq!(after.atoms, before_atoms);
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.labels.len(),
        1,
        "task-label binding is idempotent"
    );
    assert_eq!(table_count(&connect_file(&temp.path)?, "task_labels")?, 1);
    Ok(())
}

#[test]
fn label_bootstrap_rejects_empty_semantics_without_partial_writes() -> anyhow::Result<()> {
    let temp = TempDb::new("label_bootstrap_rejects_empty_semantics_without_partial_writes")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bootstrap rollback target"),
    )?;

    let error = result_err(bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "empty-semantic-label".to_owned(),
            description: Some("   ".to_owned()),
            applies_when: vec![" ".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    ))?;

    assert!(
        error
            .to_string()
            .contains("label bootstrap requires description or semantic examples")
    );
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "labels")?, 0);
    assert_eq!(table_count(&conn, "label_semantics")?, 0);
    assert_eq!(table_count(&conn, "label_atoms")?, 0);
    assert_eq!(table_count(&conn, "task_labels")?, 0);
    Ok(())
}

#[test]
fn label_bootstrap_snapshot_restore_deletes_unverified_new_label() -> anyhow::Result<()> {
    let temp = TempDb::new("label_bootstrap_snapshot_restore_deletes_unverified_new_label")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bootstrap restore target"),
    )?;

    let snapshot =
        snapshot_bootstrap_task_label_state(&temp.path, "default", &task.id, "database")?;
    bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            positive_examples: vec!["new table migration".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    )?;
    assert_eq!(table_count(&connect_file(&temp.path)?, "labels")?, 1);

    let restored = restore_bootstrap_task_label_state(&temp.path, "tester", &snapshot)?;

    assert!(restored.label_deleted);
    assert!(restored.task_binding_restored);
    assert!(!restored.semantics_restored);
    assert!(restored.index_marked_dirty);
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "labels")?, 0);
    assert_eq!(table_count(&conn, "label_semantics")?, 0);
    assert_eq!(table_count(&conn, "label_atoms")?, 0);
    assert_eq!(table_count(&conn, "task_labels")?, 0);
    let status = kanban_sqlite::label_atom_index_status(&temp.path, "default")?;
    assert_eq!(status.board_dirty, Some(true));
    Ok(())
}

#[test]
fn label_bootstrap_snapshot_restore_preserves_existing_unbound_label_without_semantics()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_bootstrap_snapshot_restore_preserves_existing_unbound_label_without_semantics",
    )?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bootstrap restore existing label target"),
    )?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "database".to_owned(),
            color: None,
        },
    )?;

    let snapshot =
        snapshot_bootstrap_task_label_state(&temp.path, "default", &task.id, "database")?;
    bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    )?;

    let restored = restore_bootstrap_task_label_state(&temp.path, "tester", &snapshot)?;

    assert!(!restored.label_deleted);
    assert!(restored.task_binding_restored);
    assert!(restored.semantics_restored);
    assert!(restored.index_marked_dirty);
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "labels")?, 1);
    assert_eq!(table_count(&conn, "label_semantics")?, 0);
    assert_eq!(table_count(&conn, "label_atoms")?, 0);
    assert_eq!(table_count(&conn, "task_labels")?, 0);
    let error = result_err(get_label_semantics(&temp.path, "default", "database"))?;
    assert!(error.to_string().contains("label semantics"));
    Ok(())
}

#[test]
fn label_bootstrap_snapshot_restore_preserves_existing_bound_label_without_semantics()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_bootstrap_snapshot_restore_preserves_existing_bound_label_without_semantics",
    )?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bootstrap restore existing bound label target"),
    )?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "database".to_owned(),
            color: None,
        },
    )?;
    kanban_sqlite::add_task_labels(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &["database".to_owned()],
    )?;

    let snapshot =
        snapshot_bootstrap_task_label_state(&temp.path, "default", &task.id, "database")?;
    bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    )?;

    let restored = restore_bootstrap_task_label_state(&temp.path, "tester", &snapshot)?;

    assert!(!restored.label_deleted);
    assert!(!restored.task_binding_restored);
    assert!(restored.semantics_restored);
    assert_eq!(get_task(&temp.path, "default", &task.id)?.labels.len(), 1);
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "labels")?, 1);
    assert_eq!(table_count(&conn, "label_semantics")?, 0);
    assert_eq!(table_count(&conn, "label_atoms")?, 0);
    assert_eq!(table_count(&conn, "task_labels")?, 1);
    Ok(())
}

#[test]
fn label_bootstrap_snapshot_restore_restores_existing_semantics_and_atoms_exactly()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_bootstrap_snapshot_restore_restores_existing_semantics_and_atoms_exactly",
    )?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bootstrap restore existing semantics target"),
    )?;
    bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            positive_examples: vec!["new table migration".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    )?;
    let before = get_label_semantics(&temp.path, "default", "database")?;
    let snapshot =
        snapshot_bootstrap_task_label_state(&temp.path, "default", &task.id, "database")?;

    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "database".to_owned(),
            replace: true,
            description: Some("Changed database semantics".to_owned()),
            applies_when: vec!["changed applies".to_owned()],
            excludes_when: vec!["changed excludes".to_owned()],
            positive_examples: vec!["changed positive".to_owned()],
            negative_examples: vec!["changed negative".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    assert_ne!(
        get_label_semantics(&temp.path, "default", "database")?,
        before
    );

    let restored = restore_bootstrap_task_label_state(&temp.path, "tester", &snapshot)?;

    assert!(restored.semantics_restored);
    let after = get_label_semantics(&temp.path, "default", "database")?;
    assert_eq!(after, before);
    assert_eq!(get_task(&temp.path, "default", &task.id)?.labels.len(), 1);
    let status = kanban_sqlite::label_atom_index_status(&temp.path, "default")?;
    assert_eq!(status.board_dirty, Some(true));
    Ok(())
}

#[test]
fn canonical_label_identity_create_writes_event_without_ontology_action() -> anyhow::Result<()> {
    let temp = TempDb::new("canonical_label_identity_create_writes_event_without_ontology_action")?;
    init_database(&temp.path, "tester")?;

    let label = kanban_sqlite::create_label_with_actor(
        &temp.path,
        "default",
        "tester",
        CreateLabel {
            name: "identity-only".to_owned(),
            color: Some("#112233".to_owned()),
        },
    )?;

    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "label_ontology_actions")?, 0);
    let created_events = list_events(&temp.path, "default", None)?
        .into_iter()
        .filter(|event| event.kind == "label.created")
        .collect::<Vec<_>>();
    assert_eq!(created_events.len(), 1);
    assert_eq!(created_events[0].actor.as_deref(), Some("tester"));
    assert_eq!(created_events[0].task_id, None);
    assert!(created_events[0].payload_json.contains(&label.id));
    assert!(created_events[0].payload_json.contains("identity-only"));
    assert!(created_events[0].payload_json.contains("#112233"));

    let deleted = delete_label(&temp.path, "default", "tester", "identity-only", false)?;
    assert_eq!(deleted.label.id, label.id);
    assert_eq!(table_count(&conn, "label_ontology_actions")?, 0);
    let deleted_events = list_events(&temp.path, "default", None)?
        .into_iter()
        .filter(|event| event.kind == "label.deleted")
        .collect::<Vec<_>>();
    assert_eq!(deleted_events.len(), 1);
    assert_eq!(deleted_events[0].actor.as_deref(), Some("tester"));
    Ok(())
}

#[test]
fn label_semantics_clear_requires_cas_records_root_action_effects_and_reverts() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_semantics_clear_requires_cas_records_root_action_effects_and_reverts")?;
    init_database(&temp.path, "tester")?;
    let label = create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let seed = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend service work".to_owned()),
            applies_when: vec!["touches Rust service code".to_owned()],
            excludes_when: vec!["CSS-only changes".to_owned()],
            positive_examples: vec!["add HTTP route".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let original_atom_count = seed.atoms.len() as i64;
    mark_label_atom_index_clean_for_default_board(&temp.path)?;
    let conn = connect_file(&temp.path)?;
    let action_count = table_count(&conn, "label_ontology_actions")?;
    let effect_count = table_count(&conn, "label_ontology_action_atom_effects")?;

    let mut stale_options = kanban_sqlite::LabelSemanticsMutationOptions::manual_actor("tester");
    stale_options.reason = Some("Attempt stale clear.".to_owned());
    let stale_error = result_err(clear_label_semantics_with_options(
        &temp.path,
        "default",
        "backend",
        "not-the-current-semantics-hash".to_owned(),
        stale_options,
    ))?;
    assert!(matches!(stale_error, KanbanError::Conflict(_)));
    assert_eq!(
        get_label_semantics(&temp.path, "default", "backend")?.semantics_hash,
        seed.semantics_hash
    );
    assert_eq!(table_count(&conn, "label_ontology_actions")?, action_count);
    assert_eq!(
        table_count(&conn, "label_ontology_action_atom_effects")?,
        effect_count
    );
    assert!(!label_atom_store_dirty(&temp.path)?);
    assert!(!label_atom_board_dirty(&temp.path, "default")?);

    let mut clear_options = kanban_sqlite::LabelSemanticsMutationOptions::manual_actor("tester");
    clear_options.reason = Some("Clear semantics with audited CAS.".to_owned());
    clear_label_semantics_with_options(
        &temp.path,
        "default",
        "backend",
        seed.semantics_hash.clone(),
        clear_options,
    )?;

    assert!(
        result_err(get_label_semantics(&temp.path, "default", "backend"))?
            .to_string()
            .contains("not found")
    );
    assert!(list_label_atoms(&temp.path, "default")?.is_empty());
    assert_eq!(
        table_count(&conn, "label_ontology_actions")?,
        action_count + 1
    );
    let clear_action = root_mutation_action_by_before_hash(&conn, &seed.semantics_hash)?;
    assert_eq!(
        clear_action.action_type,
        LabelOntologyActionType::UpdateSemantics.to_string()
    );
    assert_eq!(
        clear_action.target_label_id.as_deref(),
        Some(label.id.as_str())
    );
    assert_eq!(clear_action.result_label_id, None);
    assert_eq!(clear_action.result_atom_id, None);
    assert_eq!(clear_action.result_atom_content_hash, None);
    assert_eq!(
        clear_action.canonical_before_hash.as_deref(),
        Some(seed.semantics_hash.as_str())
    );
    assert_eq!(
        ontology_action_atom_effect_count(&conn, &clear_action.id)?,
        original_atom_count
    );
    let mut expected_removed = seed
        .atoms
        .iter()
        .map(|atom| atom.text.clone())
        .collect::<Vec<_>>();
    expected_removed.sort();
    assert_eq!(
        ontology_action_atom_effect_texts(&conn, &clear_action.id, "removed")?,
        expected_removed
    );
    assert!(label_atom_store_dirty(&temp.path)?);
    assert!(label_atom_board_dirty(&temp.path, "default")?);

    let revert_action = revert_label_ontology_mutation(
        &temp.path,
        "default",
        LabelOntologyRevertInput {
            actor: LabelOntologyActor {
                name: "tester".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            target_action_id: clear_action.id.clone(),
            expected_current_hash: clear_action.canonical_after_hash.clone(),
            reason: "Restore cleared semantics for contract test.".to_owned(),
        },
    )?;
    assert_eq!(
        revert_action.action_type,
        LabelOntologyActionType::RevertOntologyMutation
    );
    assert_eq!(
        revert_action.parent_action_id.as_deref(),
        Some(clear_action.id.as_str())
    );
    let restored = get_label_semantics(&temp.path, "default", "backend")?;
    assert_eq!(restored.semantics_hash, seed.semantics_hash);
    assert_eq!(restored.description, seed.description);
    assert_eq!(restored.applies_when, seed.applies_when);
    assert_eq!(restored.excludes_when, seed.excludes_when);
    assert_eq!(restored.positive_examples, seed.positive_examples);
    assert_eq!(restored.negative_examples, seed.negative_examples);
    Ok(())
}

#[test]
fn canonical_label_delete_rejects_bound_label_without_force() -> anyhow::Result<()> {
    let temp = TempDb::new("canonical_label_delete_rejects_bound_label_without_force")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("delete label guarded target"),
    )?;
    bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            positive_examples: vec!["new table migration".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    )?;

    let error = result_err(delete_label(
        &temp.path, "default", "tester", "database", false,
    ))?;

    assert!(error.to_string().contains("attached to 1 task(s)"));
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "labels")?, 1);
    assert_eq!(table_count(&conn, "task_labels")?, 1);
    assert_eq!(table_count(&conn, "label_semantics")?, 1);
    assert!(table_count(&conn, "label_atoms")? > 0);
    assert_eq!(
        get_task(&temp.path, "default", &task.id)?.labels[0].name,
        "database"
    );
    Ok(())
}

#[test]
fn canonical_label_delete_removes_unbound_label_without_force() -> anyhow::Result<()> {
    let temp = TempDb::new("canonical_label_delete_removes_unbound_label_without_force")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "retired".to_owned(),
            color: None,
        },
    )?;

    let deleted = delete_label(&temp.path, "default", "tester", "retired", false)?;

    assert!(!deleted.forced);
    assert_eq!(deleted.label.name, "retired");
    assert_eq!(deleted.removed_task_bindings, 0);
    assert!(!deleted.removed_semantics);
    assert_eq!(deleted.removed_atoms, 0);
    assert!(list_labels(&temp.path, "default")?.is_empty());
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "labels")?, 0);
    assert_eq!(table_count(&conn, "task_labels")?, 0);
    assert_eq!(table_count(&conn, "label_semantics")?, 0);
    assert_eq!(table_count(&conn, "label_atoms")?, 0);
    Ok(())
}

#[test]
fn canonical_label_delete_force_cleans_truth_and_marks_index_dirty() -> anyhow::Result<()> {
    let temp = TempDb::new("canonical_label_delete_force_cleans_truth_and_marks_index_dirty")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("delete label force target"),
    )?;
    let bootstrapped = bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            positive_examples: vec!["new table migration".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    )?;

    let force_error = result_err(delete_label(
        &temp.path, "default", "tester", "database", true,
    ))?;
    assert!(
        force_error.to_string().contains("has semantics or atoms"),
        "{force_error}"
    );
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "labels")?, 1);
    assert_eq!(table_count(&conn, "task_labels")?, 1);
    assert_eq!(table_count(&conn, "label_semantics")?, 1);
    assert!(table_count(&conn, "label_atoms")? > 0);

    let mut clear_options = kanban_sqlite::LabelSemanticsMutationOptions::manual_actor("tester");
    clear_options.reason = Some("Clear semantics before deleting identity.".to_owned());
    clear_label_semantics_with_options(
        &temp.path,
        "default",
        "database",
        bootstrapped.semantics.semantics_hash,
        clear_options,
    )?;
    let deleted = delete_label(&temp.path, "default", "tester", "database", true)?;

    assert!(deleted.forced);
    assert_eq!(deleted.label.name, "database");
    assert_eq!(deleted.removed_task_bindings, 1);
    assert!(!deleted.removed_semantics);
    assert_eq!(deleted.removed_atoms, 0);
    assert!(list_labels(&temp.path, "default")?.is_empty());
    assert!(get_task(&temp.path, "default", &task.id)?.labels.is_empty());
    assert_eq!(table_count(&conn, "labels")?, 0);
    assert_eq!(table_count(&conn, "task_labels")?, 0);
    assert_eq!(table_count(&conn, "label_semantics")?, 0);
    assert_eq!(table_count(&conn, "label_atoms")?, 0);
    let status = kanban_sqlite::label_atom_index_status(&temp.path, "default")?;
    assert_eq!(status.board_dirty, Some(true));
    let deleted_events = list_events(&temp.path, "default", None)?
        .into_iter()
        .filter(|event| event.kind == "label.deleted")
        .collect::<Vec<_>>();
    assert_eq!(deleted_events.len(), 1);
    assert!(deleted_events[0].payload_json.contains("\"forced\":true"));
    Ok(())
}

#[test]
fn label_proposal_residual_validation_passes_and_accept_keeps_task_unbound() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_proposal_residual_validation_passes_and_accept_keeps_task_unbound")?;
    init_database(&temp.path, "tester")?;
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
        kanban_sqlite::CreateTask {
            title: "API documentation update".to_owned(),
            description: Some("Server route docs are missing".to_owned()),
            ..CreateTask::ready("unused")
        },
    )?;
    let store = ProposalValidationStore::new(vec![
        (
            "API documentation update\n\nServer route docs are missing",
            vec![1.0, 0.65, 0.0],
        ),
        ("documentation", vec![0.0, 1.0, 0.0]),
        ("Documentation work", vec![0.0, 1.0, 0.0]),
        ("backend", vec![1.0, 0.0, 0.0]),
        ("server routes", vec![1.0, 0.0, 0.0]),
    ])
    .with_atoms(vec![(
        atom_hit(&backend, "positive", "applies_when", "server routes", 0.0),
        vec![1.0, 0.0, 0.0],
    )]);
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "documentation".to_owned(),
        description: Some("Documentation work".to_owned()),
        applies_when: vec!["documentation".to_owned()],
        ..LabelProposalCandidate::default()
    });

    let attempt = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 1,
            atom_limit: 10,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?;

    let proposal = attempt.proposal.context("proposal")?;
    assert_eq!(proposal.status, LabelProposalStatus::Proposed);
    assert!(
        proposal
            .diagnostics
            .iter()
            .any(|code| code == "label_proposal_residual_top1_verified")
    );
    let accepted = accept_label_proposal(&temp.path, "tester", &proposal.id, None)?;
    assert_eq!(accepted.status, LabelProposalStatus::Accepted);
    assert!(
        get_task(&temp.path, "default", &task.id)?.labels.is_empty(),
        "accepting a label proposal must not auto-attach task_labels"
    );
    Ok(())
}

#[test]
fn label_proposal_residual_validation_unavailable_after_candidate_is_non_polluting()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_proposal_residual_validation_unavailable_after_candidate_is_non_polluting",
    )?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("uncovered residual validation target"),
    )?;
    let store = ResidualValidationUnavailableStore::new();
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "workflow".to_owned(),
        description: Some("residual validation candidate".to_owned()),
        applies_when: vec!["residual validation candidate".to_owned()],
        ..LabelProposalCandidate::default()
    });

    let attempt = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;

    assert!(attempt.degraded);
    assert!(
        attempt.proposal.is_none(),
        "residual validation unavailable must not persist a proposed proposal"
    );
    assert!(attempt.heuristic_coverage < 0.55);
    assert!(
        attempt
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_empty")
    );
    assert!(
        attempt
            .diagnostics
            .iter()
            .any(|code| code == "label_proposal_residual_validation_unavailable")
    );
    assert!(
        attempt
            .diagnostics
            .iter()
            .any(|message| message.contains("residual validation atom query failed"))
    );
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "label_semantic_proposals")?, 0);
    assert_eq!(table_count(&conn, "labels")?, 0);
    assert_eq!(table_count(&conn, "label_semantics")?, 0);
    assert_eq!(table_count(&conn, "label_atoms")?, 0);
    assert_eq!(table_count(&conn, "task_labels")?, 0);
    Ok(())
}

#[test]
fn label_proposal_residual_validation_rejects_when_existing_wins() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_residual_validation_rejects_when_existing_wins")?;
    init_database(&temp.path, "tester")?;
    let backend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let docs = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "docs".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("API documentation update"),
    )?;
    let store = ProposalValidationStore::new(vec![
        ("API documentation update", vec![1.0, 0.65, 0.0]),
        ("workflow", vec![0.0, 0.8, 0.6]),
        ("Workflow classification", vec![0.0, 0.8, 0.6]),
        ("backend", vec![1.0, 0.0, 0.0]),
        ("docs", vec![0.0, 1.0, 0.0]),
    ])
    .with_atoms(vec![
        (
            atom_hit(&backend, "positive", "applies_when", "backend", 0.0),
            vec![1.0, 0.0, 0.0],
        ),
        (
            atom_hit(&docs, "positive", "applies_when", "docs", 0.0),
            vec![0.0, 1.0, 0.0],
        ),
    ]);
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "workflow".to_owned(),
        description: Some("Workflow classification".to_owned()),
        ..LabelProposalCandidate::default()
    });

    let proposal = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 1,
            atom_limit: 10,
            max_selected_labels: 1,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?
    .proposal
    .context("proposal")?;

    assert_eq!(proposal.status, LabelProposalStatus::Rejected);
    assert!(proposal.heuristic_coverage_cosine > 0.8);
    assert_eq!(proposal.top1_existing_label_name.as_deref(), Some("docs"));
    assert!(
        proposal
            .diagnostics
            .iter()
            .any(|code| code == "label_proposal_residual_top1_failed")
    );
    let error = result_err(accept_label_proposal(
        &temp.path,
        "tester",
        &proposal.id,
        None,
    ))?;
    assert!(error.to_string().contains("already rejected"));
    Ok(())
}

#[test]
fn label_proposal_residual_validation_rejects_when_margin_is_insufficient() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_proposal_residual_validation_rejects_when_margin_is_insufficient")?;
    init_database(&temp.path, "tester")?;
    let backend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let docs = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "docs".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("API documentation update"),
    )?;
    let store = ProposalValidationStore::new(vec![
        ("API documentation update", vec![1.0, 0.65, 0.0]),
        ("manuals", vec![0.0, 1.0, 0.0]),
        ("Manual writing", vec![0.0, 1.0, 0.0]),
        ("backend", vec![1.0, 0.0, 0.0]),
        ("docs", vec![0.0, 1.0, 0.2]),
    ])
    .with_atoms(vec![
        (
            atom_hit(&backend, "positive", "applies_when", "backend", 0.0),
            vec![1.0, 0.0, 0.0],
        ),
        (
            atom_hit(&docs, "positive", "applies_when", "docs", 0.0),
            vec![0.0, 1.0, 0.2],
        ),
    ]);
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "manuals".to_owned(),
        description: Some("Manual writing".to_owned()),
        ..LabelProposalCandidate::default()
    });

    let proposal = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 1,
            atom_limit: 10,
            max_selected_labels: 1,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?
    .proposal
    .context("proposal")?;

    assert_eq!(proposal.status, LabelProposalStatus::Rejected);
    assert!(
        proposal
            .diagnostics
            .iter()
            .any(|code| code == "label_proposal_residual_margin_insufficient")
    );
    Ok(())
}

#[test]
fn label_proposal_residual_validation_uses_solver_negative_suppression() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_residual_validation_uses_solver_negative_suppression")?;
    init_database(&temp.path, "tester")?;
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
        CreateTask::ready("Residual suppression target"),
    )?;
    let store = ProposalValidationStore::new(vec![
        ("Residual suppression target", vec![1.0, 0.0, 0.0]),
        ("workflow", vec![0.25, 0.968_245_86, 0.0]),
        ("candidate residual", vec![0.25, 0.968_245_86, 0.0]),
    ])
    .with_atoms(vec![
        (
            atom_hit(&backend, "positive", "applies_when", "server handler", 0.0),
            vec![0.95, 0.312_249_9, 0.0],
        ),
        (
            atom_hit(&backend, "negative", "excludes_when", "client polish", 0.0),
            vec![0.65, 0.759_934_2, 0.0],
        ),
    ]);
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "workflow".to_owned(),
        applies_when: vec!["candidate residual".to_owned()],
        ..LabelProposalCandidate::default()
    });

    let proposal = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 1,
            atom_limit: 10,
            min_score: 0.99,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?
    .proposal
    .context("proposal")?;

    assert_eq!(proposal.status, LabelProposalStatus::Rejected);
    assert_eq!(
        proposal.top1_existing_label_name.as_deref(),
        Some("backend")
    );
    assert!(
        proposal
            .diagnostics
            .iter()
            .any(|code| code == "label_proposal_residual_top1_failed")
    );
    Ok(())
}

#[test]
fn label_proposal_coverage_sufficient_does_not_call_provider_or_persist_candidate()
-> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_coverage_sufficient_does_not_call_provider_or_persist")?;
    init_database(&temp.path, "tester")?;
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
        CreateTask::ready("Backend API route"),
    )?;
    let store = ProposalValidationStore::new(vec![
        ("Backend API route", vec![1.0, 0.0, 0.0]),
        ("backend", vec![1.0, 0.0, 0.0]),
    ])
    .with_atoms(vec![(
        atom_hit(&backend, "positive", "applies_when", "backend", 0.0),
        vec![1.0, 0.0, 0.0],
    )]);
    let provider = CountingProposalProvider::new();

    let attempt = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 3,
            atom_limit: 10,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?;

    assert!(attempt.proposal.is_none());
    assert_eq!(provider.calls()?, 0);
    assert!(attempt.heuristic_coverage_cosine > 0.99);
    let proposals =
        list_label_proposals(&temp.path, "default", LabelProposalListOptions::default())?;
    assert!(proposals.is_empty());
    assert!(
        attempt
            .diagnostics
            .iter()
            .any(|code| code == "heuristic_coverage_sufficient")
    );
    Ok(())
}

#[test]
fn label_proposal_coverage_sufficient_preserves_degraded_diagnostics() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_coverage_sufficient_preserves_degraded_diagnostics")?;
    init_database(&temp.path, "tester")?;
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
        CreateTask::ready("Backend API route"),
    )?;
    let board = get_board(&temp.path, "default")?;
    connect_file(&temp.path)?.execute(
        "INSERT INTO label_atom_index_boards(\
             store_name,board_id,dirty,last_rebuild_at,last_error,updated_at\
         ) VALUES ('lancedb_label_atoms',?1,1,NULL,NULL,1)",
        [board.id],
    )?;
    let store = ProposalValidationStore::new(vec![
        ("Backend API route", vec![1.0, 0.0, 0.0]),
        ("backend", vec![1.0, 0.0, 0.0]),
    ])
    .with_status_message("test vector store; status copy changed")
    .with_status_dirty(true, true)
    .with_atoms(vec![(
        atom_hit(&backend, "positive", "applies_when", "backend", 0.0),
        vec![1.0, 0.0, 0.0],
    )]);
    let provider = CountingProposalProvider::new();

    let attempt = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 3,
            atom_limit: 10,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?;

    assert!(attempt.proposal.is_none());
    assert!(attempt.degraded);
    assert_eq!(provider.calls()?, 0);
    assert!(
        attempt
            .diagnostics
            .iter()
            .any(|code| code == "heuristic_coverage_sufficient")
    );
    assert!(
        attempt
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_dirty")
    );
    let proposals =
        list_label_proposals(&temp.path, "default", LabelProposalListOptions::default())?;
    assert!(proposals.is_empty());
    Ok(())
}

#[test]
fn label_proposal_validation_rejects_blank_or_empty_semantics() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_validation_rejects_blank_or_empty_semantics")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("proposal validation target"),
    )?;

    for candidate in [
        LabelProposalCandidate {
            name: " ".to_owned(),
            description: Some("has semantics".to_owned()),
            ..LabelProposalCandidate::default()
        },
        LabelProposalCandidate {
            name: "empty-semantics".to_owned(),
            ..LabelProposalCandidate::default()
        },
    ] {
        let provider = ManualLabelProposalProvider::new(candidate);
        let error = result_err(propose_task_label_with(
            &temp.path,
            "default",
            "tester",
            &task.id,
            &provider,
            kanban_sqlite::LabelSuggestionOptions::default(),
        ))?;
        assert!(error.to_string().contains("label proposal"));
    }
    Ok(())
}

#[test]
fn label_proposal_near_duplicate_is_persisted_rejected_and_cannot_accept() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_proposal_near_duplicate_is_persisted_rejected_and_cannot_accept")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "Back End".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("duplicate proposal target"),
    )?;
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "backend".to_owned(),
        description: Some("Duplicate backend semantics".to_owned()),
        ..LabelProposalCandidate::default()
    });

    let attempt = propose_task_label_with(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;
    let proposal = attempt.proposal.context("proposal")?;
    assert_eq!(proposal.status, LabelProposalStatus::Rejected);
    assert!(
        proposal
            .diagnostics
            .iter()
            .any(|code| code == "near_duplicate_label_conflict")
    );
    let error = result_err(accept_label_proposal(
        &temp.path,
        "tester",
        &proposal.id,
        None,
    ))?;
    assert!(error.to_string().contains("already rejected"));
    Ok(())
}

#[test]
fn label_proposal_reject_then_accept_fails_and_list_filters() -> anyhow::Result<()> {
    let temp = TempDb::new("label_proposal_reject_then_accept_fails_and_list_filters")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("proposal reject target"),
    )?;
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "ops".to_owned(),
        description: Some("Operational work".to_owned()),
        applies_when: vec!["operator workflow".to_owned()],
        ..LabelProposalCandidate::default()
    });
    let store = ProposalValidationStore::new(vec![
        ("proposal reject target", vec![0.0, 1.0, 0.0]),
        ("ops", vec![0.0, 1.0, 0.0]),
        ("Operational work", vec![0.0, 1.0, 0.0]),
        ("operator workflow", vec![0.0, 1.0, 0.0]),
    ]);
    let proposal = propose_task_label_with_store(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?
    .proposal
    .context("proposal")?;

    let rejected = reject_label_proposal(
        &temp.path,
        "tester",
        &proposal.id,
        Some("人工拒绝".to_owned()),
    )?;

    assert_eq!(rejected.status, LabelProposalStatus::Rejected);
    let error = result_err(accept_label_proposal(
        &temp.path,
        "tester",
        &proposal.id,
        None,
    ))?;
    assert!(error.to_string().contains("already rejected"));
    let listed = list_label_proposals(
        &temp.path,
        "default",
        LabelProposalListOptions {
            task_ref: Some(task.id),
            status: Some(LabelProposalStatus::Rejected),
        },
    )?;
    assert_eq!(listed.len(), 1);
    assert_eq!(
        get_label_proposal(&temp.path, &proposal.id)?.status,
        LabelProposalStatus::Rejected
    );
    Ok(())
}

#[test]
fn label_proposal_jsonl_export_import_round_trips() -> anyhow::Result<()> {
    let source = TempDb::new("label_proposal_jsonl_export_import_round_trips_source")?;
    init_database(&source.path, "tester")?;
    let task = create_task(
        &source.path,
        "default",
        "tester",
        CreateTask::ready("proposal export target"),
    )?;
    let provider = ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: "release".to_owned(),
        description: Some("Release workflow".to_owned()),
        applies_when: vec!["packaging or release gate".to_owned()],
        ..LabelProposalCandidate::default()
    });
    let store = ProposalValidationStore::new(vec![
        ("proposal export target", vec![0.0, 1.0, 0.0]),
        ("release", vec![0.0, 1.0, 0.0]),
        ("Release workflow", vec![0.0, 1.0, 0.0]),
        ("packaging or release gate", vec![0.0, 1.0, 0.0]),
    ]);
    let proposal = propose_task_label_with_store(
        &source.path,
        "default",
        "tester",
        &task.id,
        &provider,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?
    .proposal
    .context("proposal")?;
    let export_path = source.dir.join("proposal.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let export = std::fs::read_to_string(&export_path)?;
    assert!(export.contains("\"type\":\"label_semantic_proposal\""));

    let target = TempDb::new("label_proposal_jsonl_export_import_round_trips_target")?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &export_path, true)?;

    let imported = get_label_proposal(&target.path, &proposal.id)?;
    assert_eq!(imported.name, "release");
    assert_eq!(imported.status, LabelProposalStatus::Proposed);
    assert_eq!(
        imported.heuristic_coverage_cosine,
        proposal.heuristic_coverage_cosine
    );
    Ok(())
}

fn table_count(conn: &Connection, table: &str) -> anyhow::Result<i64> {
    Ok(
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })?,
    )
}

struct RootMutationActionRow {
    id: String,
    action_type: String,
    target_label_id: Option<String>,
    result_label_id: Option<String>,
    result_atom_id: Option<String>,
    result_atom_content_hash: Option<String>,
    canonical_before_hash: Option<String>,
    canonical_after_hash: Option<String>,
    change_json: String,
    validation_status: String,
    created_by: String,
}

fn single_root_mutation_action(conn: &Connection) -> anyhow::Result<RootMutationActionRow> {
    Ok(conn.query_row(
        "SELECT id,action_type,target_label_id,result_label_id,result_atom_id,\
         result_atom_content_hash,canonical_before_hash,canonical_after_hash,change_json,\
         validation_status,created_by FROM label_ontology_actions",
        [],
        |row| {
            Ok(RootMutationActionRow {
                id: row.get(0)?,
                action_type: row.get(1)?,
                target_label_id: row.get(2)?,
                result_label_id: row.get(3)?,
                result_atom_id: row.get(4)?,
                result_atom_content_hash: row.get(5)?,
                canonical_before_hash: row.get(6)?,
                canonical_after_hash: row.get(7)?,
                change_json: row.get(8)?,
                validation_status: row.get(9)?,
                created_by: row.get(10)?,
            })
        },
    )?)
}

fn root_mutation_action_by_before_hash(
    conn: &Connection,
    canonical_before_hash: &str,
) -> anyhow::Result<RootMutationActionRow> {
    Ok(conn.query_row(
        "SELECT id,action_type,target_label_id,result_label_id,result_atom_id,\
         result_atom_content_hash,canonical_before_hash,canonical_after_hash,change_json,\
         validation_status,created_by FROM label_ontology_actions \
         WHERE canonical_before_hash=?1 ORDER BY created_at DESC,id DESC LIMIT 1",
        [canonical_before_hash],
        |row| {
            Ok(RootMutationActionRow {
                id: row.get(0)?,
                action_type: row.get(1)?,
                target_label_id: row.get(2)?,
                result_label_id: row.get(3)?,
                result_atom_id: row.get(4)?,
                result_atom_content_hash: row.get(5)?,
                canonical_before_hash: row.get(6)?,
                canonical_after_hash: row.get(7)?,
                change_json: row.get(8)?,
                validation_status: row.get(9)?,
                created_by: row.get(10)?,
            })
        },
    )?)
}

fn ontology_action_atom_effect_count(conn: &Connection, action_id: &str) -> anyhow::Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM label_ontology_action_atom_effects WHERE action_id=?1",
        [action_id],
        |row| row.get(0),
    )?)
}

fn ontology_action_atom_effect_texts(
    conn: &Connection,
    action_id: &str,
    effect: &str,
) -> anyhow::Result<Vec<String>> {
    Ok(conn
        .prepare(
            "SELECT text FROM label_ontology_action_atom_effects \
             WHERE action_id=?1 AND effect=?2 ORDER BY text",
        )?
        .query_map([action_id, effect], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

#[test]
fn task_label_suggestions_use_residual_vector_queries_and_refit_coverage() -> anyhow::Result<()> {
    let temp =
        TempDb::new("task_label_suggestions_aggregate_atom_hits_and_penalize_negative_evidence")?;
    init_database(&temp.path, "tester")?;
    let backend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let frontend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "frontend".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        kanban_sqlite::CreateTask {
            title: "Fix API route".to_owned(),
            description: Some("Touches server handler code".to_owned()),
            ..CreateTask::ready("unused")
        },
    )?;
    let labeled =
        kanban_sqlite::add_task_label(&temp.path, "default", "tester", &task.id, "backend")?;

    let store = StaticLabelAtomStore {
        hits: vec![
            atom_hit(&backend, "positive", "applies_when", "server handlers", 0.0),
            atom_hit(&frontend, "positive", "name", "frontend", 0.2),
            atom_hit(&frontend, "negative", "excludes_when", "server only", 0.0),
        ],
    };
    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &labeled.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 5,
            atom_limit: 10,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?;

    assert_eq!(result.candidates[0].label_name, "backend");
    assert_eq!(result.candidates[0].score, 1.0);
    assert!(result.candidates[0].already_applied);
    assert_eq!(result.selected_labels.len(), 1);
    assert_eq!(result.selected_labels[0].label_name, "backend");
    assert!(result.coverage > 0.99);
    assert!(result.coverage_cosine > 0.99);
    assert!(result.residual_norm < 0.01);
    assert!(!result.needs_new_label);
    assert!(result.reason_codes.is_empty());
    Ok(())
}

#[test]
fn task_label_suggestions_reason_codes_for_selected_coverage_gap() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_suggestions_reason_codes_for_selected_coverage_gap")?;
    init_database(&temp.path, "tester")?;
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
        CreateTask::ready("API docs update"),
    )?;
    mark_label_atom_index_clean_for_default_board(&temp.path)?;
    let store = ResidualRecordingLabelAtomStore::new(vec![(
        atom_hit(&backend, "positive", "applies_when", "server handlers", 0.0),
        vec![1.0, 0.0, 0.0],
    )]);

    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &task.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 3,
            atom_limit: 12,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?;

    assert!(!result.degraded);
    assert_eq!(result.selected_labels.len(), 1);
    assert_eq!(result.selected_labels[0].label_name, "backend");
    assert!(result.coverage < 0.55);
    assert!(result.residual_norm <= 0.75);
    assert!(result.needs_new_label);
    assert_eq!(result.reason_codes, vec!["coverage_below_threshold"]);
    Ok(())
}

#[test]
fn task_label_suggestions_reason_codes_for_empty_selection_and_residual_gap() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("task_label_suggestions_reason_codes_for_empty_selection_and_residual_gap")?;
    init_database(&temp.path, "tester")?;
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
        CreateTask::ready("API docs update"),
    )?;
    mark_label_atom_index_clean_for_default_board(&temp.path)?;
    let store = ResidualRecordingLabelAtomStore::with_query_vector(
        vec![(
            atom_hit(&backend, "positive", "applies_when", "server handlers", 0.0),
            vec![1.0, 0.0, 0.0],
        )],
        vec![0.01, 1.0, 0.0],
    );

    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &task.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 3,
            atom_limit: 12,
            min_score: 0.001,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?;

    assert!(!result.degraded);
    assert!(result.selected_labels.is_empty());
    assert!(!result.candidates.is_empty());
    assert_eq!(result.coverage, 0.0);
    assert_eq!(result.residual_norm, 1.0);
    assert!(result.needs_new_label);
    assert_eq!(
        result.reason_codes,
        vec![
            "coverage_below_threshold",
            "no_selected_labels",
            "residual_above_threshold"
        ]
    );
    Ok(())
}

#[test]
fn task_label_suggestions_report_empty_index_as_degraded_reason() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_suggestions_report_empty_index_as_degraded_reason")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("uncovered task"),
    )?;
    let store = StaticLabelAtomStore { hits: Vec::new() };

    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &task.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;

    assert!(result.degraded);
    assert!(!result.needs_new_label);
    assert_eq!(
        result.reason_codes,
        vec!["degraded_result", "label_atom_index_empty"]
    );
    assert_eq!(result.coverage, 0.0);
    assert_eq!(result.coverage_cosine, 0.0);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_empty")
    );
    Ok(())
}

#[test]
fn task_label_suggestions_degrade_on_label_atom_vector_query_error() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_suggestions_degrade_on_label_atom_vector_query_error")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("query failure target"),
    )?;
    let store = DiagnosticLabelAtomStore {
        status_message: "test vector store; dirty=false last_error=none; board_dirty=false",
        dirty: false,
        board_dirty: false,
        query_error: Some("label atom vector query failed"),
    };

    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &task.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;

    assert!(result.degraded);
    assert!(result.selected_labels.is_empty());
    assert_eq!(result.coverage, 0.0);
    assert_eq!(result.coverage_cosine, 0.0);
    assert_eq!(result.residual_norm, 1.0);
    assert!(!result.needs_new_label);
    assert_eq!(
        result.reason_codes,
        vec!["degraded_result", "vector_query_error"]
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "vector_query_error")
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|message| message.contains("label atom vector query failed"))
    );
    Ok(())
}

#[test]
fn task_label_suggestions_report_label_atom_index_dirty_and_errors() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_suggestions_report_label_atom_index_dirty_and_errors")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("dirty index target"),
    )?;
    let board = get_board(&temp.path, "default")?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "UPDATE derived_store_state \
         SET dirty=1,last_error='global label atom failure',updated_at=1 \
         WHERE store_name='lancedb_label_atoms'",
        [],
    )?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(\
             store_name,board_id,dirty,last_rebuild_at,last_error,updated_at\
         ) VALUES ('lancedb_label_atoms',?1,1,NULL,'board label atom failure',1)",
        [board.id],
    )?;
    let store = DiagnosticLabelAtomStore {
        status_message: "test vector store; status copy changed",
        dirty: true,
        board_dirty: true,
        query_error: None,
    };

    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &task.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;

    assert!(result.degraded);
    assert!(!result.needs_new_label);
    assert_eq!(
        result.reason_codes,
        vec![
            "degraded_result",
            "label_atom_index_dirty",
            "label_atom_index_empty",
            "label_atom_index_error"
        ]
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_dirty")
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_error")
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|message| message.contains("global label atom failure"))
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|message| message.contains("board label atom failure"))
    );
    Ok(())
}

#[test]
fn task_label_suggestions_query_positive_atoms_by_residual_rounds() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_suggestions_query_positive_atoms_by_residual_rounds")?;
    init_database(&temp.path, "tester")?;
    let backend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let docs = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "docs".to_owned(),
            color: None,
        },
    )?;
    let frontend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "frontend".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("API docs update"),
    )?;
    let store = ResidualRecordingLabelAtomStore::new(vec![
        (
            atom_hit(&backend, "positive", "applies_when", "server handlers", 0.0),
            vec![1.0, 0.0, 0.0],
        ),
        (
            atom_hit(&docs, "positive", "applies_when", "documentation", 0.0),
            vec![0.0, 1.0, 0.0],
        ),
        (
            atom_hit(&frontend, "positive", "name", "frontend", 0.0),
            vec![0.0, 0.0, 1.0],
        ),
        (
            atom_hit(&frontend, "negative", "excludes_when", "server docs", 0.0),
            vec![0.0, 0.0, 1.0],
        ),
    ]);

    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &task.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 3,
            atom_limit: 12,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?;

    assert_eq!(
        result
            .selected_labels
            .iter()
            .map(|label| label.label_name.as_str())
            .collect::<Vec<_>>(),
        vec!["backend", "docs"]
    );
    assert!(result.coverage > 0.99);
    assert!(result.residual_norm < 0.01);
    assert!(!result.needs_new_label);
    let queries = store.queries()?;
    assert!(
        queries
            .iter()
            .filter(|query| query.polarity.as_deref() == Some("positive"))
            .count()
            >= 2,
        "solver should issue multiple positive residual vector queries: {queries:?}"
    );
    assert!(queries.iter().all(
        |query| query.include_vector && query.embedding_model.as_deref() == Some("test-model")
    ));
    assert!(
        queries
            .iter()
            .filter(|query| query.polarity.as_deref() == Some("positive"))
            .all(|query| (vector_norm(&query.vector) - 1.0).abs() < 0.0001)
    );
    Ok(())
}

#[test]
fn task_label_suggestions_limit_truncates_output_without_narrowing_solver() -> anyhow::Result<()> {
    let temp =
        TempDb::new("task_label_suggestions_limit_truncates_output_without_narrowing_solver")?;
    init_database(&temp.path, "tester")?;
    let backend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let docs = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "docs".to_owned(),
            color: None,
        },
    )?;
    let frontend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "frontend".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("API docs update"),
    )?;
    let store = ResidualRecordingLabelAtomStore::new(vec![
        (
            atom_hit(&backend, "positive", "applies_when", "server handlers", 0.0),
            vec![1.0, 0.0, 0.0],
        ),
        (
            atom_hit(&docs, "positive", "applies_when", "documentation", 0.0),
            vec![0.0, 1.0, 0.0],
        ),
        (
            atom_hit(&frontend, "positive", "name", "frontend", 0.0),
            vec![0.0, 0.0, 1.0],
        ),
    ]);

    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &task.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            output_limit: 1,
            atom_limit: 12,
            min_score: 0.01,
            ..kanban_sqlite::LabelSuggestionOptions::default()
        },
    )?;

    assert_eq!(result.selected_labels.len(), 1);
    assert_eq!(result.candidates.len(), 1);
    assert!(
        result.coverage > 0.99,
        "output limit must not reduce internal refit coverage: {result:?}"
    );
    assert!(result.coverage_cosine > 0.99);
    assert!(result.residual_norm < 0.01);
    let positive_queries = store
        .queries()?
        .into_iter()
        .filter(|query| query.polarity.as_deref() == Some("positive"))
        .collect::<Vec<_>>();
    assert!(
        positive_queries.len() >= 2,
        "solver should still make residual follow-up queries: {positive_queries:?}"
    );
    Ok(())
}

struct DiagnosticLabelAtomStore {
    status_message: &'static str,
    dirty: bool,
    board_dirty: bool,
    query_error: Option<&'static str>,
}

struct CountingProposalProvider {
    calls: std::sync::Mutex<usize>,
}

impl CountingProposalProvider {
    fn new() -> Self {
        Self {
            calls: std::sync::Mutex::new(0),
        }
    }

    fn calls(&self) -> anyhow::Result<usize> {
        Ok(*self
            .calls
            .lock()
            .map_err(|err| test_error(format!("calls mutex poisoned: {err}")))?)
    }
}

impl kanban_sqlite::LabelProposalProvider for CountingProposalProvider {
    fn propose_label(
        &self,
        _task: &kanban_sqlite::TaskRecord,
        _suggestions: &kanban_sqlite::LabelSuggestionResult,
    ) -> kanban_core::Result<Option<LabelProposalCandidate>> {
        *self
            .calls
            .lock()
            .map_err(|err| KanbanError::Storage(format!("calls mutex poisoned: {err}")))? += 1;
        Ok(Some(LabelProposalCandidate {
            name: "should-not-be-called".to_owned(),
            description: Some("Provider should not run when coverage is sufficient".to_owned()),
            ..LabelProposalCandidate::default()
        }))
    }
}

fn record_confirmed_proposal_source_signal(
    temp: &TempDb,
    task_ref: &str,
    proposed_label_name: &str,
    signal_key: &str,
) -> anyhow::Result<String> {
    let observation = record_label_ontology_observation(
        &temp.path,
        "default",
        task_ref,
        LabelOntologyRecordInput {
            actor: LabelOntologyActor {
                name: "label-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            agent_candidates_json: json!([
                {"label": proposed_label_name, "confidence": 0.88, "reason": "new vocabulary"}
            ])
            .to_string(),
            suggestion_snapshot_json: json!({
                "result": {"selected_labels": [], "candidates": []}
            })
            .to_string(),
            final_decision_json: json!({"accepted_labels": [proposed_label_name]}).to_string(),
            suggest_coverage: Some(0.22),
            suggest_coverage_cosine: Some(0.31),
            suggest_residual_norm: Some(0.78),
            suggest_needs_new_label: true,
            suggest_degraded: false,
            diagnostics_json: "[]".to_owned(),
            capture_fingerprint: None,
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::VocabularyGap,
                target_label_ref: None,
                related_labels_json: "[]".to_owned(),
                proposed_action: LabelOntologyProposedAction::BootstrapLabel,
                candidate_atom: None,
                proposed_label_name: Some(proposed_label_name.to_owned()),
                proposal_json: json!({
                    "name": proposed_label_name,
                    "description": "Label ontology ledger work",
                    "applies_when": ["records ontology observations and signals"]
                })
                .to_string(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Absent),
                suggest_score: None,
                suggest_rank: None,
                final_selected: true,
                rationale: "Existing labels do not express this vocabulary.".to_owned(),
                confidence: Some(0.86),
                signal_key: Some(signal_key.to_owned()),
            }],
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
    Ok(signal_id)
}

fn record_confirmed_atom_source_signal(
    temp: &TempDb,
    task_ref: &str,
    label_name: &str,
) -> anyhow::Result<String> {
    let task = get_task(&temp.path, "default", task_ref)?;
    bootstrap_task_label(
        &temp.path,
        "default",
        "tester",
        &task.id,
        BootstrapTaskLabel {
            name: label_name.to_owned(),
            description: Some("Label ontology ledger work".to_owned()),
            applies_when: vec!["records ontology observations and signals".to_owned()],
            positive_examples: vec!["create ontology ledger table".to_owned()],
            ..BootstrapTaskLabel::default()
        },
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
                {"label": label_name, "confidence": 0.88, "reason": "label evidence"}
            ])
            .to_string(),
            suggestion_snapshot_json: json!({
                "result": {"selected_labels": [label_name], "candidates": []}
            })
            .to_string(),
            final_decision_json: json!({"accepted_labels": [label_name]}).to_string(),
            suggest_coverage: Some(0.62),
            suggest_coverage_cosine: Some(0.72),
            suggest_residual_norm: Some(0.38),
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics_json: "[]".to_owned(),
            capture_fingerprint: None,
            signals: vec![LabelOntologySignalInput {
                kind: LabelOntologySignalKind::FalsePositive,
                target_label_ref: Some(label_name.to_owned()),
                related_labels_json: "[]".to_owned(),
                proposed_action: LabelOntologyProposedAction::AddNegativeAtom,
                candidate_atom: Some(LabelOntologyCandidateAtomInput {
                    polarity: "negative".to_owned(),
                    kind: "excludes_when".to_owned(),
                    text: "proposal creation provenance uses unrelated atom evidence".to_owned(),
                }),
                proposed_label_name: None,
                proposal_json: "{}".to_owned(),
                agent_selected: true,
                suggest_state: Some(LabelOntologySuggestState::Selected),
                suggest_score: Some(0.91),
                suggest_rank: Some(1),
                final_selected: false,
                rationale: "This signal is about atom tuning, not proposal bootstrap provenance."
                    .to_owned(),
                confidence: Some(0.84),
                signal_key: Some(format!("{label_name}-atom-source")),
            }],
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        action_input(
            LabelOntologyActionType::Confirm,
            vec![signal_id.clone()],
            "Reviewer confirms this atom evidence is real.",
        ),
    )?;
    Ok(signal_id)
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

fn ontology_proposal_provider(name: &str) -> ManualLabelProposalProvider {
    ManualLabelProposalProvider::new(LabelProposalCandidate {
        name: name.to_owned(),
        description: Some("Label ontology ledger work".to_owned()),
        applies_when: vec!["records ontology observations and signals".to_owned()],
        excludes_when: vec!["unrelated UI-only work".to_owned()],
        positive_examples: vec!["ontology signal provenance".to_owned()],
        negative_examples: vec!["CSS polish".to_owned()],
    })
}

fn ontology_proposal_store(task_title: &str, name: &str) -> ProposalValidationStore {
    ProposalValidationStore::new(vec![
        (task_title, vec![0.0, 1.0, 0.0]),
        (name, vec![0.0, 1.0, 0.0]),
        ("Label ontology ledger work", vec![0.0, 1.0, 0.0]),
        (
            "records ontology observations and signals",
            vec![0.0, 1.0, 0.0],
        ),
        ("ontology signal provenance", vec![0.0, 1.0, 0.0]),
    ])
}

struct ProposalValidationStore {
    embeddings: Vec<(String, Vec<f32>)>,
    atoms: Vec<(kanban_vector::LabelAtomHit, Vec<f32>)>,
    status_message: &'static str,
    dirty: bool,
    board_dirty: bool,
}

impl ProposalValidationStore {
    fn new(embeddings: Vec<(&str, Vec<f32>)>) -> Self {
        Self {
            embeddings: embeddings
                .into_iter()
                .map(|(text, vector)| (text.to_owned(), vector))
                .collect(),
            atoms: Vec::new(),
            status_message: "test vector store; dirty=false last_error=none; board_dirty=false",
            dirty: false,
            board_dirty: false,
        }
    }

    fn with_atoms(mut self, atoms: Vec<(kanban_vector::LabelAtomHit, Vec<f32>)>) -> Self {
        self.atoms = atoms;
        self
    }

    fn with_status_message(mut self, status_message: &'static str) -> Self {
        self.status_message = status_message;
        self
    }

    fn with_status_dirty(mut self, dirty: bool, board_dirty: bool) -> Self {
        self.dirty = dirty;
        self.board_dirty = board_dirty;
        self
    }

    fn embedding_for(&self, text: &str) -> Vec<f32> {
        self.embeddings
            .iter()
            .find(|(key, _vector)| key == text)
            .or_else(|| {
                self.embeddings
                    .iter()
                    .find(|(key, _vector)| text.contains(key) || key.contains(text))
            })
            .map(|(_key, vector)| vector.clone())
            .unwrap_or_else(|| vec![0.0, 0.0, 1.0])
    }
}

struct ResidualValidationUnavailableStore {
    validation_started: std::sync::Mutex<bool>,
}

impl ResidualValidationUnavailableStore {
    fn new() -> Self {
        Self {
            validation_started: std::sync::Mutex::new(false),
        }
    }

    fn mark_validation_started(&self, text: &str) -> Result<(), kanban_vector::VectorError> {
        if text.contains("residual validation candidate") {
            *self.validation_started.lock().map_err(|err| {
                kanban_vector::VectorError::Store(format!(
                    "validation_started mutex poisoned: {err}"
                ))
            })? = true;
        }
        Ok(())
    }
}

impl kanban_vector::VectorStoreBackend for ResidualValidationUnavailableStore {
    fn embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus::new(
            "test-vector",
            true,
            "test vector store; dirty=false last_error=none; board_dirty=false",
        )
    }
}

impl kanban_vector::QueryEmbeddingProvider for ResidualValidationUnavailableStore {
    fn embed_query_text(&self, text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        self.mark_validation_started(text)?;
        if text.contains("residual validation candidate") {
            Ok(vec![0.0, 1.0, 0.0])
        } else {
            Ok(vec![1.0, 0.0, 0.0])
        }
    }
}

impl kanban_vector::LabelAtomVectorStore for ResidualValidationUnavailableStore {
    fn query_label_atoms_by_vector(
        &self,
        _query: &kanban_vector::LabelAtomVectorQuery,
    ) -> Result<Vec<kanban_vector::LabelAtomVectorHit>, kanban_vector::VectorError> {
        if *self.validation_started.lock().map_err(|err| {
            kanban_vector::VectorError::Store(format!("validation_started mutex poisoned: {err}"))
        })? {
            return Err(kanban_vector::VectorError::Store(
                "residual validation atom query failed".to_owned(),
            ));
        }
        Ok(Vec::new())
    }
}

impl kanban_vector::VectorStoreBackend for ProposalValidationStore {
    fn embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: self.status_message.to_owned(),
            diagnostics: Vec::new(),
            dirty: Some(self.dirty),
            board_dirty: Some(self.board_dirty),
            generation: None,
        }
    }
}

impl kanban_vector::QueryEmbeddingProvider for ProposalValidationStore {
    fn embed_query_text(&self, text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(self.embedding_for(text))
    }
}

impl kanban_vector::LabelAtomVectorStore for ProposalValidationStore {
    fn query_label_atoms_by_vector(
        &self,
        query: &kanban_vector::LabelAtomVectorQuery,
    ) -> Result<Vec<kanban_vector::LabelAtomVectorHit>, kanban_vector::VectorError> {
        let mut hits = self
            .atoms
            .iter()
            .filter(|(hit, _vector)| {
                query
                    .board_id
                    .as_ref()
                    .is_none_or(|board_id| &hit.board_id == board_id)
                    && query
                        .embedding_model
                        .as_ref()
                        .is_none_or(|model| &hit.embedding_model == model)
                    && query
                        .polarity
                        .as_ref()
                        .is_none_or(|polarity| &hit.polarity == polarity)
            })
            .filter_map(|(hit, vector)| {
                let similarity = cosine(&query.vector, vector);
                (similarity > 0.0).then_some((similarity, hit.clone(), vector.clone()))
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| right.0.total_cmp(&left.0));
        Ok(hits
            .into_iter()
            .take(query.limit)
            .map(|(similarity, mut hit, vector)| {
                hit.distance = (1.0 / similarity.max(0.0001)) - 1.0;
                kanban_vector::LabelAtomVectorHit {
                    hit,
                    vector: query.include_vector.then_some(vector),
                }
            })
            .collect())
    }
}

impl kanban_vector::VectorStoreBackend for DiagnosticLabelAtomStore {
    fn embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: self.status_message.to_owned(),
            diagnostics: Vec::new(),
            dirty: Some(self.dirty),
            board_dirty: Some(self.board_dirty),
            generation: None,
        }
    }
}

impl kanban_vector::QueryEmbeddingProvider for DiagnosticLabelAtomStore {
    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(vec![1.0, 0.0, 0.0])
    }
}

impl kanban_vector::LabelAtomVectorStore for DiagnosticLabelAtomStore {
    fn query_label_atoms_by_vector(
        &self,
        _query: &kanban_vector::LabelAtomVectorQuery,
    ) -> Result<Vec<kanban_vector::LabelAtomVectorHit>, kanban_vector::VectorError> {
        if let Some(message) = self.query_error {
            return Err(kanban_vector::VectorError::Store(message.to_owned()));
        }
        Ok(Vec::new())
    }
}

struct ResidualRecordingLabelAtomStore {
    atoms: Vec<(kanban_vector::LabelAtomHit, Vec<f32>)>,
    query_vector: Vec<f32>,
    queries: std::sync::Mutex<Vec<kanban_vector::LabelAtomVectorQuery>>,
}

impl ResidualRecordingLabelAtomStore {
    fn new(atoms: Vec<(kanban_vector::LabelAtomHit, Vec<f32>)>) -> Self {
        Self::with_query_vector(atoms, vec![1.0, 1.0, 0.0])
    }

    fn with_query_vector(
        atoms: Vec<(kanban_vector::LabelAtomHit, Vec<f32>)>,
        query_vector: Vec<f32>,
    ) -> Self {
        Self {
            atoms,
            query_vector,
            queries: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn queries(&self) -> anyhow::Result<Vec<kanban_vector::LabelAtomVectorQuery>> {
        Ok(self
            .queries
            .lock()
            .map_err(|err| test_error(format!("queries mutex poisoned: {err}")))?
            .clone())
    }
}

impl kanban_vector::VectorStoreBackend for ResidualRecordingLabelAtomStore {
    fn embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus::new(
            "test-vector",
            true,
            "test vector store; dirty=false last_error=none; board_dirty=false",
        )
    }
}

impl kanban_vector::QueryEmbeddingProvider for ResidualRecordingLabelAtomStore {
    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(self.query_vector.clone())
    }
}

impl kanban_vector::LabelAtomVectorStore for ResidualRecordingLabelAtomStore {
    fn query_label_atoms_by_vector(
        &self,
        query: &kanban_vector::LabelAtomVectorQuery,
    ) -> Result<Vec<kanban_vector::LabelAtomVectorHit>, kanban_vector::VectorError> {
        self.queries
            .lock()
            .map_err(|err| {
                kanban_vector::VectorError::Store(format!("queries mutex poisoned: {err}"))
            })?
            .push(query.clone());
        let mut hits = self
            .atoms
            .iter()
            .filter(|(hit, _vector)| {
                query
                    .board_id
                    .as_ref()
                    .is_none_or(|board_id| &hit.board_id == board_id)
                    && query
                        .embedding_model
                        .as_ref()
                        .is_none_or(|model| &hit.embedding_model == model)
                    && query
                        .polarity
                        .as_ref()
                        .is_none_or(|polarity| &hit.polarity == polarity)
            })
            .map(|(hit, vector)| {
                let similarity = cosine(&query.vector, vector);
                (similarity, hit.clone(), vector.clone())
            })
            .filter(|(similarity, _hit, _vector)| *similarity > 0.0)
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| right.0.total_cmp(&left.0));
        Ok(hits
            .into_iter()
            .take(query.limit)
            .map(|(similarity, mut hit, vector)| {
                hit.distance = (1.0 / similarity.max(0.0001)) - 1.0;
                kanban_vector::LabelAtomVectorHit {
                    hit,
                    vector: query.include_vector.then_some(vector),
                }
            })
            .collect())
    }
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let dot = left
        .iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn vector_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn mark_label_atom_index_clean_for_default_board(path: &std::path::Path) -> anyhow::Result<()> {
    let board = get_board(path, "default")?;
    let conn = connect_file(path)?;
    conn.execute(
        "UPDATE derived_store_state SET dirty=0,last_error=NULL WHERE store_name='lancedb_label_atoms'",
        [],
    )?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(\
             store_name,board_id,dirty,last_rebuild_at,last_error,updated_at\
         ) VALUES ('lancedb_label_atoms',?1,0,1,NULL,1) \
         ON CONFLICT(store_name,board_id) DO UPDATE SET dirty=0,last_error=NULL,updated_at=excluded.updated_at",
        [&board.id],
    )?;
    Ok(())
}

struct StaticLabelAtomStore {
    hits: Vec<kanban_vector::LabelAtomHit>,
}

impl kanban_vector::VectorStoreBackend for StaticLabelAtomStore {
    fn embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus::new(
            "test-vector",
            true,
            "test vector store; dirty=false last_error=none; board_dirty=false",
        )
    }
}

impl kanban_vector::QueryEmbeddingProvider for StaticLabelAtomStore {
    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(vec![1.0, 0.0, 0.0])
    }
}

impl kanban_vector::LabelAtomVectorStore for StaticLabelAtomStore {
    fn query_label_atoms_by_vector(
        &self,
        query: &kanban_vector::LabelAtomVectorQuery,
    ) -> Result<Vec<kanban_vector::LabelAtomVectorHit>, kanban_vector::VectorError> {
        Ok(self
            .hits
            .iter()
            .filter(|hit| {
                query
                    .board_id
                    .as_ref()
                    .is_none_or(|board_id| &hit.board_id == board_id)
                    && query
                        .embedding_model
                        .as_ref()
                        .is_none_or(|model| &hit.embedding_model == model)
                    && query
                        .polarity
                        .as_ref()
                        .is_none_or(|polarity| &hit.polarity == polarity)
            })
            .take(query.limit)
            .cloned()
            .map(|hit| {
                let vector = match (hit.label_name.as_str(), hit.polarity.as_str()) {
                    ("backend", "positive") => vec![1.0, 0.0, 0.0],
                    ("frontend", _) => vec![0.0, 1.0, 0.0],
                    _ => vec![0.0, 0.0, 1.0],
                };
                kanban_vector::LabelAtomVectorHit {
                    hit,
                    vector: query.include_vector.then_some(vector),
                }
            })
            .collect())
    }
}

fn atom_hit(
    label: &kanban_sqlite::LabelRecord,
    polarity: &str,
    kind: &str,
    text: &str,
    distance: f32,
) -> kanban_vector::LabelAtomHit {
    kanban_vector::LabelAtomHit {
        atom_id: format!("la_{}_{}", label.name, kind),
        label_id: label.id.clone(),
        label_name: label.name.clone(),
        board_id: label.board_id.clone(),
        polarity: polarity.to_owned(),
        kind: kind.to_owned(),
        text: text.to_owned(),
        ordinal: 0,
        content_hash: "hash".to_owned(),
        embedding_model: "test-model".to_owned(),
        distance,
    }
}

#[test]
fn label_semantics_crud_expands_stable_atoms_and_keeps_label_binding() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_crud_expands_stable_atoms_and_keeps_label_binding")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("label target"),
    )?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "backend".into(),
            color: None,
        },
    )?;
    let labeled =
        kanban_sqlite::add_task_label(&temp.path, "default", "tester", &task.id, "backend")?;
    let label = labeled
        .labels
        .first()
        .ok_or_else(|| test_error("expected task label"))?
        .clone();

    let semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend implementation work".to_owned()),
            applies_when: vec!["  touches server code  ".to_owned(), "".to_owned()],
            positive_examples: vec!["API handlers".to_owned()],
            excludes_when: vec!["frontend only".to_owned()],
            negative_examples: vec!["CSS polish".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;

    assert_eq!(semantics.label_id, label.id);
    assert_eq!(semantics.applies_when, vec!["touches server code"]);
    assert_eq!(semantics.atoms.len(), 5);
    assert_eq!(semantics.atoms[0].kind, "description");
    assert_eq!(
        semantics.atoms[0].text,
        "label: backend\ndescription: Backend implementation work"
    );
    assert!(
        semantics
            .atoms
            .iter()
            .any(|atom| atom.polarity == "negative" && atom.kind == "negative_example")
    );
    let first_ids = semantics
        .atoms
        .iter()
        .map(|atom| atom.id.clone())
        .collect::<Vec<_>>();

    let reread = get_label_semantics(&temp.path, "default", &label.id)?;
    assert_eq!(
        reread
            .atoms
            .iter()
            .map(|atom| atom.id.clone())
            .collect::<Vec<_>>(),
        first_ids
    );

    let fresh_task = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(fresh_task.labels.len(), 1);
    assert_eq!(fresh_task.labels[0].id, label.id);

    let mut clear_options = kanban_sqlite::LabelSemanticsMutationOptions::manual_actor("tester");
    clear_options.reason = Some("Clear semantics for CRUD cleanup.".to_owned());
    clear_label_semantics_with_options(
        &temp.path,
        "default",
        "backend",
        semantics.semantics_hash.clone(),
        clear_options,
    )?;
    assert!(
        result_err(get_label_semantics(&temp.path, "default", "backend"))?
            .to_string()
            .contains("not found")
    );
    assert!(list_label_atoms(&temp.path, "default")?.is_empty());
    assert_eq!(get_task(&temp.path, "default", &task.id)?.labels.len(), 1);
    Ok(())
}

#[test]
fn label_atom_hashes_are_stable_across_reordered_sources_without_dirty_noop() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_atom_hashes_are_stable_across_reordered_sources_without_dirty_noop")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;

    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            applies_when: vec![
                "touches Rust service code".to_owned(),
                "updates SQLite repository".to_owned(),
            ],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let first_atoms = list_label_atoms(&temp.path, "default")?;
    let first_identity_by_text = first_atoms
        .iter()
        .filter(|atom| atom.kind == "applies_when")
        .map(|atom| {
            (
                atom.text.clone(),
                (atom.id.clone(), atom.content_hash.clone()),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    let board = get_board(&temp.path, "default")?;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "UPDATE derived_store_state SET dirty=0 WHERE store_name='lancedb_label_atoms'",
        [],
    )?;
    conn.execute(
        "UPDATE label_atom_index_boards SET dirty=0 \
         WHERE store_name='lancedb_label_atoms' AND board_id=?1",
        [&board.id],
    )?;
    let action_count = table_count(&conn, "label_ontology_actions")?;
    let effect_count = table_count(&conn, "label_ontology_action_atom_effects")?;

    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            applies_when: vec![
                "updates SQLite repository".to_owned(),
                "touches Rust service code".to_owned(),
            ],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let reordered_atoms = list_label_atoms(&temp.path, "default")?;
    let reordered_identity_by_text = reordered_atoms
        .iter()
        .filter(|atom| atom.kind == "applies_when")
        .map(|atom| {
            (
                atom.text.clone(),
                (atom.id.clone(), atom.content_hash.clone()),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(reordered_identity_by_text, first_identity_by_text);
    assert_eq!(table_count(&conn, "label_ontology_actions")?, action_count);
    assert_eq!(
        table_count(&conn, "label_ontology_action_atom_effects")?,
        effect_count
    );
    assert!(!label_atom_store_dirty(&temp.path)?);
    assert!(!label_atom_board_dirty(&temp.path, "default")?);
    Ok(())
}

#[test]
fn label_atom_rebuild_deduplicates_normalized_text_and_keeps_first_ordinal() -> anyhow::Result<()> {
    let temp =
        TempDb::new("label_atom_rebuild_deduplicates_normalized_text_and_keeps_first_ordinal")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;

    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            applies_when: vec![
                "touches   server code".to_owned(),
                " touches server code ".to_owned(),
                "touches server code".to_owned(),
            ],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let atoms = list_label_atoms(&temp.path, "default")?;
    let applies = atoms
        .iter()
        .filter(|atom| atom.kind == "applies_when")
        .collect::<Vec<_>>();
    assert_eq!(applies.len(), 1);
    assert_eq!(applies[0].text, "touches server code");
    assert_eq!(applies[0].ordinal, 1);
    Ok(())
}

#[test]
fn direct_label_semantics_upsert_records_update_semantics_provenance() -> anyhow::Result<()> {
    let temp = TempDb::new("direct_label_semantics_upsert_records_update_semantics_provenance")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;

    let semantics = kanban_sqlite::upsert_label_semantics_with_options(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            applies_when: vec!["touches Rust service code".to_owned()],
            ..UpsertLabelSemantics::default()
        },
        kanban_sqlite::LabelSemanticsMutationOptions::manual_actor("ontology-editor"),
    )?;
    let atom = semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;

    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "label_ontology_actions")?, 1);
    let action = single_root_mutation_action(&conn)?;
    assert_eq!(
        action.action_type,
        LabelOntologyActionType::UpdateSemantics.to_string()
    );
    assert_eq!(
        action.target_label_id.as_deref(),
        Some(semantics.label_id.as_str())
    );
    assert_eq!(action.result_label_id, None);
    assert_eq!(action.result_atom_id, None);
    assert_eq!(action.result_atom_content_hash, None);
    assert_eq!(action.created_by, "ontology-editor");
    assert_eq!(
        action.validation_status,
        LabelOntologyValidationStatus::Pending.to_string()
    );
    assert!(action.canonical_before_hash.is_some());
    assert_eq!(
        action.canonical_after_hash.as_deref(),
        Some(semantics.semantics_hash.as_str())
    );
    let change: serde_json::Value = serde_json::from_str(&action.change_json)?;
    assert!(change.get("atoms").is_none());
    assert_eq!(
        change["atom_effect_counts"],
        json!({"added": 2, "removed": 0})
    );
    assert_eq!(ontology_action_atom_effect_count(&conn, &action.id)?, 2);
    let added_texts = ontology_action_atom_effect_texts(&conn, &action.id, "added")?;
    assert!(added_texts.iter().any(|text| text == "backend"));
    assert!(added_texts.iter().any(|text| text == &atom.text));
    Ok(())
}

#[test]
fn label_semantics_root_actions_record_only_atom_effect_deltas() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_root_actions_record_only_atom_effect_deltas")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let seed = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend service work".to_owned()),
            applies_when: vec![
                "touches Rust service code".to_owned(),
                "updates API handlers".to_owned(),
            ],
            ..UpsertLabelSemantics::default()
        },
    )?;
    mark_label_atom_index_clean_for_default_board(&temp.path)?;

    let description_patch = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(seed.semantics_hash.clone()),
            description: Some("Backend service ownership".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "label_ontology_actions")?, 2);
    assert_eq!(table_count(&conn, "label_ontology_action_atom_effects")?, 2);
    let description_action_id: String = conn.query_row(
        "SELECT id FROM label_ontology_actions WHERE canonical_after_hash=?1",
        [&description_patch.semantics_hash],
        |row| row.get(0),
    )?;
    assert_eq!(
        ontology_action_atom_effect_count(&conn, &description_action_id)?,
        0
    );
    assert!(label_atom_store_dirty(&temp.path)?);
    assert!(label_atom_board_dirty(&temp.path, "default")?);

    mark_label_atom_index_clean_for_default_board(&temp.path)?;
    let action_count = table_count(&conn, "label_ontology_actions")?;
    let effect_count = table_count(&conn, "label_ontology_action_atom_effects")?;
    let no_op = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(description_patch.semantics_hash.clone()),
            description: Some("Backend service ownership".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    assert_eq!(no_op.semantics_hash, description_patch.semantics_hash);
    assert_eq!(table_count(&conn, "label_ontology_actions")?, action_count);
    assert_eq!(
        table_count(&conn, "label_ontology_action_atom_effects")?,
        effect_count
    );
    assert!(!label_atom_store_dirty(&temp.path)?);
    assert!(!label_atom_board_dirty(&temp.path, "default")?);

    let added = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(no_op.semantics_hash.clone()),
            applies_when: vec!["owns scheduler transitions".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let add_action_id: String = conn.query_row(
        "SELECT id FROM label_ontology_actions WHERE canonical_after_hash=?1",
        [&added.semantics_hash],
        |row| row.get(0),
    )?;
    assert_eq!(ontology_action_atom_effect_count(&conn, &add_action_id)?, 1);
    assert_eq!(
        ontology_action_atom_effect_texts(&conn, &add_action_id, "added")?,
        vec!["owns scheduler transitions"]
    );

    let removed = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(added.semantics_hash.clone()),
            remove_applies_when: vec!["updates API handlers".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let remove_action_id: String = conn.query_row(
        "SELECT id FROM label_ontology_actions WHERE canonical_after_hash=?1",
        [&removed.semantics_hash],
        |row| row.get(0),
    )?;
    assert_eq!(
        ontology_action_atom_effect_count(&conn, &remove_action_id)?,
        1
    );
    assert_eq!(
        ontology_action_atom_effect_texts(&conn, &remove_action_id, "removed")?,
        vec!["updates API handlers"]
    );
    Ok(())
}

#[test]
fn label_semantics_patch_preserves_missing_fields_and_records_reason() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_patch_preserves_missing_fields_and_records_reason")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let seed = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend service work".to_owned()),
            applies_when: vec!["touches Rust service code".to_owned()],
            excludes_when: vec!["CSS-only polish".to_owned()],
            positive_examples: vec!["add API handler".to_owned()],
            negative_examples: vec!["adjust spacing".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let mut options = kanban_sqlite::LabelSemanticsMutationOptions::manual_actor("editor");
    options.reason = Some("Add a CLI-facing backend boundary.".to_owned());

    let patched = kanban_sqlite::upsert_label_semantics_with_options(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(seed.semantics_hash.clone()),
            applies_when: vec!["exposes CLI JSON output".to_owned()],
            ..UpsertLabelSemantics::default()
        },
        options,
    )?;

    assert_eq!(patched.description.as_deref(), Some("Backend service work"));
    assert_eq!(
        patched.applies_when,
        vec!["touches Rust service code", "exposes CLI JSON output"]
    );
    assert_eq!(patched.excludes_when, vec!["CSS-only polish"]);
    assert_eq!(patched.positive_examples, vec!["add API handler"]);
    assert_eq!(patched.negative_examples, vec!["adjust spacing"]);
    assert_ne!(patched.semantics_hash, seed.semantics_hash);
    let conn = connect_file(&temp.path)?;
    let action_id: String = conn.query_row(
        "SELECT id FROM label_ontology_actions WHERE canonical_after_hash=?1 AND reason=?2",
        [
            patched.semantics_hash.as_str(),
            "Add a CLI-facing backend boundary.",
        ],
        |row| row.get(0),
    )?;
    assert_eq!(
        ontology_action_atom_effect_texts(&conn, &action_id, "added")?,
        vec!["exposes CLI JSON output"]
    );
    Ok(())
}

#[test]
fn label_semantics_patch_remove_atom_only_changes_target_collection() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_patch_remove_atom_only_changes_target_collection")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let seed = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend service work".to_owned()),
            applies_when: vec![
                "touches Rust service code".to_owned(),
                "updates API handlers".to_owned(),
            ],
            excludes_when: vec!["CSS-only polish".to_owned()],
            positive_examples: vec!["add API handler".to_owned()],
            negative_examples: vec!["adjust spacing".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let patched = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(seed.semantics_hash.clone()),
            remove_applies_when: vec![" touches Rust service code ".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;

    assert_eq!(patched.applies_when, vec!["updates API handlers"]);
    assert_eq!(patched.excludes_when, vec!["CSS-only polish"]);
    assert_eq!(patched.positive_examples, vec!["add API handler"]);
    assert_eq!(patched.negative_examples, vec!["adjust spacing"]);
    assert!(
        patched.atoms.iter().all(|atom| {
            atom.kind != "applies_when" || atom.text != "touches Rust service code"
        })
    );
    Ok(())
}

#[test]
fn label_semantics_replace_intent_required_to_clear_omitted_fields() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_replace_intent_required_to_clear_omitted_fields")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let seed = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend service work".to_owned()),
            applies_when: vec!["touches Rust service code".to_owned()],
            excludes_when: vec!["CSS-only polish".to_owned()],
            positive_examples: vec!["add API handler".to_owned()],
            negative_examples: vec!["adjust spacing".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let patched = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(seed.semantics_hash.clone()),
            description: Some("Backend service ownership".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    assert_eq!(
        patched.description.as_deref(),
        Some("Backend service ownership")
    );
    assert_eq!(patched.applies_when, seed.applies_when);
    assert_eq!(patched.excludes_when, seed.excludes_when);
    assert_eq!(patched.positive_examples, seed.positive_examples);
    assert_eq!(patched.negative_examples, seed.negative_examples);

    let replaced = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(patched.semantics_hash.clone()),
            replace: true,
            description: Some("Only the replacement description remains".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    assert_eq!(
        replaced.description.as_deref(),
        Some("Only the replacement description remains")
    );
    assert!(replaced.applies_when.is_empty());
    assert!(replaced.excludes_when.is_empty());
    assert!(replaced.positive_examples.is_empty());
    assert!(replaced.negative_examples.is_empty());
    assert_eq!(replaced.atoms.len(), 1);
    assert_eq!(replaced.atoms[0].kind, "description");
    Ok(())
}

#[test]
fn label_semantics_upsert_rejects_stale_expected_hash_without_mutating() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_upsert_rejects_stale_expected_hash_without_mutating")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let seed = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend service work".to_owned()),
            applies_when: vec!["touches Rust service code".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let changed = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(seed.semantics_hash.clone()),
            applies_when: vec!["updates API handlers".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let conn = connect_file(&temp.path)?;
    let action_count = table_count(&conn, "label_ontology_actions")?;

    let error = result_err(upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            expected_semantics_hash: Some(seed.semantics_hash),
            applies_when: vec!["stale writer addition".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    ))?;

    assert!(matches!(error, KanbanError::Conflict(_)));
    assert!(error.to_string().contains("hash mismatch"));
    let after = get_label_semantics(&temp.path, "default", "backend")?;
    assert_eq!(after.semantics_hash, changed.semantics_hash);
    assert_eq!(after.applies_when, changed.applies_when);
    assert!(
        !after
            .applies_when
            .iter()
            .any(|item| item == "stale writer addition")
    );
    assert_eq!(table_count(&conn, "label_ontology_actions")?, action_count);
    Ok(())
}

#[test]
fn direct_label_bootstrap_records_bootstrap_provenance_for_atoms() -> anyhow::Result<()> {
    let temp = TempDb::new("direct_label_bootstrap_records_bootstrap_provenance_for_atoms")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("bootstrap ontology provenance target"),
    )?;

    let result = bootstrap_task_label(
        &temp.path,
        "default",
        "bootstrapper",
        &task.id,
        BootstrapTaskLabel {
            name: "ontology".to_owned(),
            description: Some("Ontology provenance work".to_owned()),
            applies_when: vec!["records semantics mutations in the ledger".to_owned()],
            ..BootstrapTaskLabel::default()
        },
    )?;
    let atom = result
        .semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;

    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "label_ontology_actions")?, 1);
    let action = single_root_mutation_action(&conn)?;
    assert_eq!(
        action.action_type,
        LabelOntologyActionType::BootstrapLabel.to_string()
    );
    assert_eq!(
        action.result_label_id.as_deref(),
        Some(result.semantics.label_id.as_str())
    );
    assert_eq!(action.target_label_id, None);
    assert_eq!(action.result_atom_id, None);
    assert_eq!(action.result_atom_content_hash, None);
    assert_eq!(action.created_by, "bootstrapper");
    assert!(action.canonical_before_hash.is_some());
    assert_eq!(
        action.canonical_after_hash.as_deref(),
        Some(result.semantics.semantics_hash.as_str())
    );
    let change: serde_json::Value = serde_json::from_str(&action.change_json)?;
    assert_eq!(
        change["atom_effect_counts"],
        json!({"added": 2, "removed": 0})
    );
    assert_eq!(ontology_action_atom_effect_count(&conn, &action.id)?, 2);
    assert_eq!(
        ontology_action_atom_effect_texts(&conn, &action.id, "added")?,
        vec![
            "label: ontology\ndescription: Ontology provenance work".to_owned(),
            atom.text.clone(),
        ]
    );
    Ok(())
}

#[test]
fn label_atom_explain_hydrates_provenance_signals_and_validation() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_explain_hydrates_provenance_signals_and_validation")?;
    let fixture = seed_label_atom_explain_fixture(&temp, "Explain CLI atom provenance")?;
    seed_passed_explain_validation_action(&temp, &fixture)?;

    let explain = explain_label_atom(&temp.path, "default", &fixture.result_atom_id)?;

    assert_eq!(explain.query, fixture.result_atom_id);
    let atom = explain.atom.as_ref().context("current atom")?;
    assert_eq!(atom.id, fixture.result_atom_id);
    assert_eq!(atom.content_hash, fixture.result_atom_content_hash);
    assert_eq!(
        explain
            .current_semantics
            .as_ref()
            .context("current semantics")?
            .label_id,
        fixture.target_label_id
    );
    assert!(!explain.legacy_untracked);
    assert_eq!(explain.legacy_reason, None);
    assert_eq!(explain.provenance_actions.len(), 1);
    assert_eq!(explain.provenance_actions[0].matched_by, "atom_id");
    assert_eq!(
        explain.provenance_actions[0].action.id,
        fixture.apply_action_id
    );
    assert_eq!(explain.supporting_signals.len(), 1);
    let source = &explain.supporting_signals[0];
    assert_eq!(source.signal.id, fixture.signal_id);
    assert_eq!(source.observation.id, fixture.observation_id);
    assert_eq!(source.source_task.id, fixture.task.id);
    assert_eq!(source.task_ref_snapshot, fixture.task.task_ref);
    assert!(!source.suggest_input_stale);
    assert!(!source.suggest_degraded);
    assert!(source.warnings.is_empty());
    assert_eq!(explain.validation_history.len(), 1);
    let validation = &explain.validation_history[0];
    assert_eq!(validation.parent_action_id, fixture.apply_action_id);
    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Passed
    );
    assert_eq!(validation.summary["status"], "passed");
    assert_eq!(validation.cases.as_array().context("cases")?.len(), 1);
    Ok(())
}

#[test]
fn label_atom_explain_resolves_rebuilt_atom_by_content_hash() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_explain_resolves_rebuilt_atom_by_content_hash")?;
    let fixture = seed_label_atom_explain_fixture(&temp, "Explain rebuilt CLI atom")?;
    let rebuilt_atom_id = "la_rebuilt_explain_atom";
    connect_file(&temp.path)?.execute(
        "UPDATE label_atoms SET id=?1 WHERE id=?2",
        params![rebuilt_atom_id, fixture.result_atom_id],
    )?;

    let explain = explain_label_atom(&temp.path, "default", &fixture.result_atom_content_hash)?;

    let atom = explain.atom.as_ref().context("current atom")?;
    assert_eq!(atom.id, rebuilt_atom_id);
    assert_eq!(atom.content_hash, fixture.result_atom_content_hash);
    assert_eq!(explain.provenance_actions.len(), 1);
    assert_eq!(explain.provenance_actions[0].matched_by, "content_hash");
    assert_eq!(
        explain.provenance_actions[0].action.id,
        fixture.apply_action_id
    );
    Ok(())
}

#[test]
fn label_atom_explain_marks_existing_atom_without_provenance_as_legacy_untracked()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_atom_explain_marks_existing_atom_without_provenance_as_legacy_untracked",
    )?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "legacy".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "legacy".to_owned(),
            applies_when: vec!["legacy atom without ontology action".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    connect_file(&temp.path)?.execute("DELETE FROM label_ontology_actions", [])?;
    let atom = list_label_atoms(&temp.path, "default")?
        .into_iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;

    let explain = explain_label_atom(&temp.path, "default", &atom.id)?;

    assert_eq!(explain.atom.as_ref().context("current atom")?.id, atom.id);
    assert!(explain.provenance_actions.is_empty());
    assert!(explain.supporting_signals.is_empty());
    assert!(explain.validation_history.is_empty());
    assert!(explain.legacy_untracked);
    assert!(
        explain
            .legacy_reason
            .as_deref()
            .unwrap_or_default()
            .contains("no ontology provenance")
    );
    Ok(())
}

#[test]
fn label_atom_explain_rejects_unknown_atom_ref() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_explain_rejects_unknown_atom_ref")?;
    init_database(&temp.path, "tester")?;

    let error = result_err(explain_label_atom(&temp.path, "default", "la_missing"))?;

    assert!(error.to_string().contains("label atom la_missing"));
    Ok(())
}

#[test]
fn init_v10_backfills_stable_label_atom_hashes_and_marks_index_dirty() -> anyhow::Result<()> {
    let temp = TempDb::new("init_v10_backfills_stable_label_atom_hashes_and_marks_index_dirty")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            applies_when: vec![
                "touches   server code".to_owned(),
                "touches server code".to_owned(),
            ],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let conn = connect_file(&temp.path)?;
    conn.execute(
        "DELETE FROM label_atoms WHERE board_id=?1 AND label_id=?2",
        (&semantics.board_id, &semantics.label_id),
    )?;
    for (id, text, ordinal, content_hash) in [
        ("la_old_name", "backend", 0_i64, "old_name"),
        (
            "la_old_applies_1",
            "touches   server code",
            1_i64,
            "old_applies_1",
        ),
        (
            "la_old_applies_2",
            "touches server code",
            2_i64,
            "old_applies_2",
        ),
    ] {
        conn.execute(
            "INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at) \
             VALUES (?1,?2,?3,'positive',?4,?5,?6,?7,1,1)",
            (
                id,
                &semantics.label_id,
                &semantics.board_id,
                if ordinal == 0 { "name" } else { "applies_when" },
                text,
                ordinal,
                content_hash,
            ),
        )?;
    }
    conn.execute(
        "UPDATE derived_store_state SET dirty=0 WHERE store_name='lancedb_label_atoms'",
        [],
    )?;
    conn.execute(
        "UPDATE label_atom_index_boards SET dirty=0 \
         WHERE store_name='lancedb_label_atoms' AND board_id=?1",
        [&semantics.board_id],
    )?;
    conn.execute("DELETE FROM schema_migrations WHERE version=10", [])?;
    conn.pragma_update(None, "user_version", 9)?;
    drop(conn);

    init_database(&temp.path, "tester")?;

    let atoms = list_label_atoms(&temp.path, "default")?;
    assert!(atoms.iter().all(|atom| !atom.id.starts_with("la_old_")));
    let applies = atoms
        .iter()
        .filter(|atom| atom.kind == "applies_when")
        .collect::<Vec<_>>();
    assert_eq!(applies.len(), 1);
    assert_eq!(applies[0].text, "touches server code");
    assert_eq!(applies[0].ordinal, 1);
    assert!(label_atom_store_dirty(&temp.path)?);
    assert!(label_atom_board_dirty(&temp.path, "default")?);
    let user_version: i64 =
        connect_file(&temp.path)?.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 18);
    Ok(())
}

#[test]
fn init_retries_v10_label_atom_hash_backfill_after_recorded_migration() -> anyhow::Result<()> {
    let temp = TempDb::new("init_retries_v10_label_atom_hash_backfill_after_recorded_migration")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            applies_when: vec![
                "touches   server code".to_owned(),
                "touches server code".to_owned(),
            ],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let conn = connect_file(&temp.path)?;
    conn.execute(
        "DELETE FROM label_atoms WHERE board_id=?1 AND label_id=?2",
        (&semantics.board_id, &semantics.label_id),
    )?;
    for (id, text, ordinal, content_hash) in [
        ("la_old_name", "backend", 0_i64, "old_name"),
        (
            "la_old_applies_1",
            "touches   server code",
            1_i64,
            "old_applies_1",
        ),
        (
            "la_old_applies_2",
            "touches server code",
            2_i64,
            "old_applies_2",
        ),
    ] {
        conn.execute(
            "INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at) \
             VALUES (?1,?2,?3,'positive',?4,?5,?6,?7,1,1)",
            (
                id,
                &semantics.label_id,
                &semantics.board_id,
                if ordinal == 0 { "name" } else { "applies_when" },
                text,
                ordinal,
                content_hash,
            ),
        )?;
    }
    conn.execute(
        "UPDATE derived_store_state SET dirty=0 WHERE store_name='lancedb_label_atoms'",
        [],
    )?;
    conn.execute(
        "UPDATE label_atom_index_boards SET dirty=0 \
         WHERE store_name='lancedb_label_atoms' AND board_id=?1",
        [&semantics.board_id],
    )?;
    let v10_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE version=10",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(v10_count, 1);
    drop(conn);

    init_database(&temp.path, "tester")?;

    let atoms = list_label_atoms(&temp.path, "default")?;
    assert!(atoms.iter().all(|atom| !atom.id.starts_with("la_old_")));
    let applies = atoms
        .iter()
        .filter(|atom| atom.kind == "applies_when")
        .collect::<Vec<_>>();
    assert_eq!(applies.len(), 1);
    assert_eq!(applies[0].text, "touches server code");
    assert_eq!(applies[0].ordinal, 1);
    assert!(label_atom_store_dirty(&temp.path)?);
    assert!(label_atom_board_dirty(&temp.path, "default")?);
    Ok(())
}

#[test]
fn init_v10_label_atom_hash_backfill_rolls_back_when_dirty_mark_fails() -> anyhow::Result<()> {
    let temp = TempDb::new("init_v10_label_atom_hash_backfill_rolls_back_when_dirty_mark_fails")?;
    init_database(&temp.path, "tester")?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            applies_when: vec!["touches server code".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let conn = connect_file(&temp.path)?;
    conn.execute(
        "DELETE FROM label_atoms WHERE board_id=?1 AND label_id=?2",
        (&semantics.board_id, &semantics.label_id),
    )?;
    conn.execute(
        "INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at) \
         VALUES ('la_old_name',?1,?2,'positive','name','backend',0,'old_name',1,1)",
        (&semantics.label_id, &semantics.board_id),
    )?;
    conn.execute(
        "UPDATE derived_store_state SET dirty=0 WHERE store_name='lancedb_label_atoms'",
        [],
    )?;
    conn.execute(
        "UPDATE label_atom_index_boards SET dirty=0 \
         WHERE store_name='lancedb_label_atoms' AND board_id=?1",
        [&semantics.board_id],
    )?;
    conn.execute_batch(
        "CREATE TRIGGER fail_label_atom_dirty_mark \
         BEFORE UPDATE ON derived_store_state \
         WHEN NEW.store_name='lancedb_label_atoms' \
         BEGIN \
           SELECT RAISE(ABORT, 'forced label atom dirty failure'); \
         END;",
    )?;
    drop(conn);

    let err = result_err(init_database(&temp.path, "tester"))?;
    assert!(
        err.to_string().contains("forced label atom dirty failure"),
        "err: {err}"
    );
    let atoms = list_label_atoms(&temp.path, "default")?;
    assert!(
        atoms.iter().any(|atom| atom.id == "la_old_name"),
        "failed dirty mark must roll back atom rewrite: {atoms:?}"
    );
    assert!(!label_atom_store_dirty(&temp.path)?);
    assert!(!label_atom_board_dirty(&temp.path, "default")?);

    connect_file(&temp.path)?.execute("DROP TRIGGER fail_label_atom_dirty_mark", [])?;
    init_database(&temp.path, "tester")?;
    let repaired = list_label_atoms(&temp.path, "default")?;
    assert!(repaired.iter().all(|atom| !atom.id.starts_with("la_old_")));
    assert!(label_atom_store_dirty(&temp.path)?);
    assert!(label_atom_board_dirty(&temp.path, "default")?);
    Ok(())
}

#[test]
fn label_semantics_resolves_l_prefixed_label_name_before_id_fallback() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_resolves_l_prefixed_label_name_before_id_fallback")?;
    init_database(&temp.path, "tester")?;
    let label = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "l_foo".to_owned(),
            color: None,
        },
    )?;

    let semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "l_foo".to_owned(),
            description: Some("Name starts with an id-like prefix".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;

    assert_eq!(semantics.label_id, label.id);
    assert_eq!(semantics.label_name, "l_foo");
    let reread = get_label_semantics(&temp.path, "default", "l_foo")?;
    assert_eq!(reread.label_id, label.id);
    let mut clear_options = kanban_sqlite::LabelSemanticsMutationOptions::manual_actor("tester");
    clear_options.reason = Some("Clear semantics for id fallback check.".to_owned());
    clear_label_semantics_with_options(
        &temp.path,
        "default",
        "l_foo",
        semantics.semantics_hash,
        clear_options,
    )?;
    assert!(list_label_atoms(&temp.path, "default")?.is_empty());
    Ok(())
}

#[test]
fn label_semantics_jsonl_export_import_round_trips_truth_and_atoms() -> anyhow::Result<()> {
    let source =
        TempDb::new("label_semantics_jsonl_export_import_round_trips_truth_and_atoms_source")?;
    init_database(&source.path, "tester")?;
    create_label(
        &source.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: Some("blue".to_owned()),
        },
    )?;
    let semantics = upsert_label_semantics(
        &source.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend implementation work".to_owned()),
            applies_when: vec!["touches Rust service code".to_owned()],
            positive_examples: vec!["SQLite repository change".to_owned()],
            excludes_when: vec!["UI-only polish".to_owned()],
            negative_examples: vec!["CSS-only tweak".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;
    let source_atoms = semantics
        .atoms
        .iter()
        .map(|atom| {
            (
                atom.id.clone(),
                atom.content_hash.clone(),
                atom.text.clone(),
            )
        })
        .collect::<Vec<_>>();

    let export_path = source.dir.join("labels.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let export = std::fs::read_to_string(&export_path)?;
    assert!(export.contains("\"type\":\"label_semantics\""));
    assert!(export.contains("\"type\":\"label_atom\""));

    let target =
        TempDb::new("label_semantics_jsonl_export_import_round_trips_truth_and_atoms_target")?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &export_path, true)?;

    let imported = get_label_semantics(&target.path, "default", "backend")?;
    assert_eq!(
        imported.description.as_deref(),
        Some("Backend implementation work")
    );
    assert_eq!(imported.applies_when, vec!["touches Rust service code"]);
    assert_eq!(
        imported
            .atoms
            .iter()
            .map(|atom| (
                atom.id.clone(),
                atom.content_hash.clone(),
                atom.text.clone()
            ))
            .collect::<Vec<_>>(),
        source_atoms
    );
    assert!(
        label_atom_store_dirty(&target.path)?,
        "imported label truth must mark global label atom store dirty"
    );
    assert!(
        label_atom_board_dirty(&target.path, "default")?,
        "imported label truth must mark the imported board dirty"
    );
    Ok(())
}

#[test]
fn label_semantics_rejects_missing_or_cross_board_label() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_rejects_missing_or_cross_board_label")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "second".to_owned(),
            name: "Second".to_owned(),
            description: None,
        },
    )?;
    let second_label = create_label(
        &temp.path,
        "second",
        kanban_sqlite::CreateLabel {
            name: "shared".to_owned(),
            color: None,
        },
    )?;

    let missing = result_err(upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "missing".to_owned(),
            ..UpsertLabelSemantics::default()
        },
    ))?;
    assert!(missing.to_string().contains("not found"));

    let cross_board = result_err(upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: second_label.id,
            ..UpsertLabelSemantics::default()
        },
    ))?;
    assert!(cross_board.to_string().contains("not found"));
    Ok(())
}

#[test]
fn doctor_reports_missing_label_semantics_tables_unhealthy() -> anyhow::Result<()> {
    for table in ["label_semantics", "label_atoms", "label_atom_index_boards"] {
        let temp = TempDb::new(&format!(
            "doctor_reports_missing_label_semantics_tables_unhealthy_{table}"
        ))?;
        init_database(&temp.path, "tester")?;
        connect_file(&temp.path)?.execute_batch(&format!("DROP TABLE {table};"))?;

        let report = doctor_database(&temp.path)?;

        assert_eq!(report.migration_version, Some(18));
        assert_eq!(report.user_version, 18);
        assert!(!report.ok, "{table} missing should make doctor unhealthy");
    }
    Ok(())
}

#[test]
fn label_atom_store_is_seeded_and_not_dirtied_by_task_outbox() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_store_is_seeded_and_not_dirtied_by_task_outbox")?;
    init_database(&temp.path, "tester")?;

    let stores = derived_store_statuses(&temp.path)?;
    assert!(
        stores
            .iter()
            .any(|store| store.store_name == "lancedb_label_atoms")
    );
    assert_eq!(stores.len(), 4);

    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("task outbox should not dirty label atoms"),
    )?;
    let stores = derived_store_statuses(&temp.path)?;
    let chunks = stores
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks"))?;
    let label_atoms = stores
        .iter()
        .find(|store| store.store_name == "lancedb_label_atoms")
        .ok_or_else(|| test_error("missing lancedb_label_atoms"))?;
    assert!(chunks.dirty);
    assert!(!label_atoms.dirty);
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_atom_rebuild_status_query_and_failure_are_independent() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_rebuild_status_query_and_failure_are_independent")?;
    let init = init_database(&temp.path, "tester")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("task vector dirty"),
    )?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend work".to_owned()),
            excludes_when: vec!["frontend only".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let store = RecordingVectorStore::with_embedding_model("static-test");
    let status = rebuild_label_atom_index_with(&temp.path, "default", &store)?;
    assert!(status.message.contains("rebuilt 2 label atom(s)"));
    assert_eq!(store.upserted_label_atoms()?.len(), 2);
    let hits = query_label_atom_index_with(
        &temp.path,
        "default",
        &store,
        LabelAtomQuery {
            text: "backend".to_owned(),
            limit: 10,
            board_id: None,
            embedding_model: Some("static-test".to_owned()),
            polarity: Some("positive".to_owned()),
        },
    )?;
    assert_eq!(hits.len(), 1);
    assert!(hits.iter().all(|hit| hit.polarity == "positive"));

    let vector_hits = query_label_atom_index_by_vector_with(
        &temp.path,
        "default",
        &store,
        LabelAtomVectorQuery {
            vector: vec![1.0, 0.0, 0.0],
            limit: 10,
            board_id: None,
            embedding_model: Some("static-test".to_owned()),
            polarity: Some("positive".to_owned()),
            include_vector: true,
        },
    )?;
    assert_eq!(vector_hits.len(), 1);
    assert!(
        vector_hits
            .iter()
            .all(|hit| hit.hit.board_id == init.board_id)
    );
    assert!(
        vector_hits
            .iter()
            .all(|hit| hit.vector.as_deref() == Some([1.0, 0.0, 0.0].as_slice()))
    );
    let recorded = store.label_atom_vector_queries()?;
    assert_eq!(recorded.len(), 1);
    assert_eq!(
        recorded[0].board_id.as_deref(),
        Some(init.board_id.as_str())
    );
    assert_eq!(recorded[0].embedding_model.as_deref(), Some("static-test"));
    assert_eq!(recorded[0].polarity.as_deref(), Some("positive"));
    assert!(recorded[0].include_vector);

    let stores = derived_store_statuses(&temp.path)?;
    assert!(
        stores
            .iter()
            .find(|store| store.store_name == "lancedb_chunks")
            .ok_or_else(|| test_error("missing lancedb_chunks"))?
            .dirty,
        "task chunk outbox remains dirty"
    );
    assert!(
        !stores
            .iter()
            .find(|store| store.store_name == "lancedb_label_atoms")
            .ok_or_else(|| test_error("missing lancedb_label_atoms"))?
            .dirty
    );

    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend work updated".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    let failure = result_err(rebuild_label_atom_index_with(
        &temp.path,
        "default",
        &FailingVectorStore,
    ))?;
    assert!(failure.to_string().contains("dimension mismatch"));
    let status = label_atom_index_status_with(&temp.path, "default", &store)?;
    assert_eq!(status.dirty, Some(true));
    assert_eq!(status.board_dirty, Some(true));
    assert!(
        status
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_dirty")
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_error")
    );
    assert!(status.message.contains("dimension mismatch"));
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_atom_rebuild_keeps_global_dirty_until_all_dirty_boards_rebuild() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_rebuild_keeps_global_dirty_until_all_dirty_boards_rebuild")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "second".to_owned(),
            name: "Second".to_owned(),
            description: None,
        },
    )?;
    for (board, label) in [("default", "backend"), ("second", "frontend")] {
        create_label(
            &temp.path,
            board,
            kanban_sqlite::CreateLabel {
                name: label.to_owned(),
                color: None,
            },
        )?;
        upsert_label_semantics(
            &temp.path,
            board,
            UpsertLabelSemantics {
                label_ref: label.to_owned(),
                description: Some(format!("{label} work")),
                ..UpsertLabelSemantics::default()
            },
        )?;
    }

    let store = RecordingVectorStore::with_embedding_model("static-test");
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;
    let stores = derived_store_statuses(&temp.path)?;
    assert!(
        stores
            .iter()
            .find(|store| store.store_name == "lancedb_label_atoms")
            .ok_or_else(|| test_error("missing lancedb_label_atoms"))?
            .dirty,
        "second board remains dirty after rebuilding only default"
    );

    rebuild_label_atom_index_with(&temp.path, "second", &store)?;
    let stores = derived_store_statuses(&temp.path)?;
    assert!(
        !stores
            .iter()
            .find(|store| store.store_name == "lancedb_label_atoms")
            .ok_or_else(|| test_error("missing lancedb_label_atoms"))?
            .dirty,
        "all dirty label atom boards were rebuilt"
    );
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_semantics_jsonl_import_marks_label_atom_boards_dirty_and_rebuild_clears_per_board()
-> anyhow::Result<()> {
    let source = TempDb::new(
        "label_semantics_jsonl_import_marks_label_atom_boards_dirty_and_rebuild_clears_source",
    )?;
    init_database(&source.path, "tester")?;
    create_board(
        &source.path,
        "tester",
        CreateBoard {
            slug: "second".to_owned(),
            name: "Second".to_owned(),
            description: None,
        },
    )?;
    for (board, label) in [("default", "backend"), ("second", "frontend")] {
        create_label(
            &source.path,
            board,
            kanban_sqlite::CreateLabel {
                name: label.to_owned(),
                color: None,
            },
        )?;
        upsert_label_semantics(
            &source.path,
            board,
            UpsertLabelSemantics {
                label_ref: label.to_owned(),
                description: Some(format!("{label} work")),
                applies_when: vec![format!("{label} scope")],
                ..UpsertLabelSemantics::default()
            },
        )?;
    }

    let default_export = source.dir.join("default-labels.jsonl");
    let second_export = source.dir.join("second-labels.jsonl");
    let import_path = source.dir.join("two-board-labels.jsonl");
    export_jsonl(&source.path, "default", &default_export)?;
    export_jsonl(&source.path, "second", &second_export)?;
    let merged_export = format!(
        "{}{}",
        std::fs::read_to_string(&default_export)?,
        std::fs::read_to_string(&second_export)?
    );
    std::fs::write(&import_path, merged_export)?;

    let target = TempDb::new(
        "label_semantics_jsonl_import_marks_label_atom_boards_dirty_and_rebuild_clears_target",
    )?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &import_path, true)?;

    assert!(label_atom_store_dirty(&target.path)?);
    assert!(label_atom_board_dirty(&target.path, "default")?);
    assert!(label_atom_board_dirty(&target.path, "second")?);
    let status = label_atom_index_status_with(
        &target.path,
        "default",
        &RecordingVectorStore::with_embedding_model("static-test"),
    )?;
    assert_eq!(status.dirty, Some(true));
    assert_eq!(status.board_dirty, Some(true));
    assert!(
        status
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_dirty")
    );

    let store = RecordingVectorStore::with_embedding_model("static-test");
    rebuild_label_atom_index_with(&target.path, "default", &store)?;
    assert!(
        !label_atom_board_dirty(&target.path, "default")?,
        "rebuilt board should be clean"
    );
    assert!(
        label_atom_board_dirty(&target.path, "second")?,
        "unrebuilt imported board must remain dirty"
    );
    assert!(
        label_atom_store_dirty(&target.path)?,
        "global store remains dirty while another imported board is dirty"
    );

    rebuild_label_atom_index_with(&target.path, "second", &store)?;
    assert!(!label_atom_board_dirty(&target.path, "second")?);
    assert!(!label_atom_store_dirty(&target.path)?);
    Ok(())
}

struct LabelAtomExplainFixture {
    task: TaskRecord,
    observation_id: String,
    signal_id: String,
    apply_action_id: String,
    target_label_id: String,
    result_atom_id: String,
    result_atom_content_hash: String,
}

fn seed_label_atom_explain_fixture(
    temp: &TempDb,
    title: &str,
) -> anyhow::Result<LabelAtomExplainFixture> {
    init_database(&temp.path, "tester")?;
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
                signal_key: Some("cli-explain-false-negative".to_owned()),
            }],
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    create_label_ontology_action(
        &temp.path,
        "default",
        LabelOntologyActionInput {
            actor: LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            action_type: LabelOntologyActionType::Confirm,
            signal_ids: vec![signal_id.clone()],
            reason: "Confirmed by reviewer.".to_owned(),
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
        },
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
    Ok(LabelAtomExplainFixture {
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

fn seed_passed_explain_validation_action(
    temp: &TempDb,
    fixture: &LabelAtomExplainFixture,
) -> anyhow::Result<()> {
    let validation_json = json!({
        "manual": {
            "evidence_type": "trusted_automated",
            "embedding_model": "test-embedding-v1",
            "solver_options": {"candidate_limit": 24, "atom_limit": 64},
            "index": {"status": "ready", "dirty": false, "generation": 7},
            "cases": [{
                "signal_id": fixture.signal_id,
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
            }]
        },
        "cases": [{
            "signal_id": fixture.signal_id,
            "task_id": fixture.task.id,
            "after": {
                "validation_status": "passed",
                "manual_case_ref": {
                    "source": "manual.cases",
                    "index": 0,
                    "signal_id": fixture.signal_id
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
    });
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "INSERT INTO label_ontology_actions(
         id, board_id, parent_action_id, action_type, reason, target_label_id, result_label_id,
         result_atom_id, result_atom_content_hash, result_proposal_id, canonical_before_hash,
         canonical_after_hash, change_json, validation_status, validation_json, created_by,
         created_by_type, agent_type, created_at)
         VALUES ('loa_explain_validation', ?1, ?2, 'validate',
         'seeded atom explain validation fixture', ?3, NULL, ?4, ?5, NULL, NULL, NULL,
         '{}', 'passed', ?6, 'test-fixture', 'agent', 'codex', 123456)",
        params![
            fixture.task.board_id,
            fixture.apply_action_id,
            fixture.target_label_id,
            fixture.result_atom_id,
            fixture.result_atom_content_hash,
            validation_json.to_string(),
        ],
    )?;
    conn.execute(
        "INSERT INTO label_ontology_action_signals(board_id, action_id, signal_id, created_at)
         VALUES (?1, 'loa_explain_validation', ?2, 123456)",
        params![fixture.task.board_id, fixture.signal_id],
    )?;
    Ok(())
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
    Ok(connect_file(path)?
        .query_row(
            "SELECT dirty FROM label_atom_index_boards \
             WHERE store_name='lancedb_label_atoms' AND board_id=?1",
            [board.id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(false))
}
