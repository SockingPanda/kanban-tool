use crate::common::*;
use rusqlite::OptionalExtension;

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
    assert_eq!(result.residual_norm, 1.0);
    assert!(!result.needs_new_label);
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
    assert_eq!(user_version, 9);
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

    let attempt = propose_task_label_with(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
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
    assert!(
        get_task(&temp.path, "default", &task.id)?.labels.is_empty(),
        "accept must not attach task_labels"
    );
    let conn = connect_file(&temp.path)?;
    assert_eq!(table_count(&conn, "task_labels")?, 0);
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
            limit: 1,
            atom_limit: 10,
            min_score: 0.01,
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
            limit: 1,
            atom_limit: 10,
            min_score: 0.01,
        },
    )?
    .proposal
    .context("proposal")?;

    assert_eq!(proposal.status, LabelProposalStatus::Rejected);
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
            limit: 1,
            atom_limit: 10,
            min_score: 0.01,
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
            limit: 1,
            atom_limit: 10,
            min_score: 0.99,
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
            limit: 3,
            atom_limit: 10,
            min_score: 0.01,
        },
    )?;

    assert!(attempt.proposal.is_none());
    assert_eq!(provider.calls()?, 0);
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
    .with_status_message("test vector store; dirty=true last_error=none; board_dirty=true")
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
            limit: 3,
            atom_limit: 10,
            min_score: 0.01,
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
    let proposal = propose_task_label_with(
        &temp.path,
        "default",
        "tester",
        &task.id,
        &provider,
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
    let proposal = propose_task_label_with(
        &source.path,
        "default",
        "tester",
        &task.id,
        &provider,
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
    Ok(())
}

fn table_count(conn: &Connection, table: &str) -> anyhow::Result<i64> {
    Ok(
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })?,
    )
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
            limit: 5,
            atom_limit: 10,
            min_score: 0.01,
        },
    )?;

    assert_eq!(result.candidates[0].label_name, "backend");
    assert_eq!(result.candidates[0].score, 1.0);
    assert!(result.candidates[0].already_applied);
    assert_eq!(result.selected_labels.len(), 1);
    assert_eq!(result.selected_labels[0].label_name, "backend");
    assert!(result.coverage > 0.99);
    assert!(result.residual_norm < 0.01);
    assert!(!result.needs_new_label);
    Ok(())
}

#[test]
fn task_label_suggestions_signal_new_label_when_enabled_index_has_no_selected_labels()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "task_label_suggestions_signal_new_label_when_enabled_index_has_no_selected_labels",
    )?;
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

    assert!(result.needs_new_label);
    assert_eq!(result.coverage, 0.0);
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
    assert_eq!(result.residual_norm, 1.0);
    assert!(!result.needs_new_label);
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
        status_message: "test vector store; dirty=true last_error=none; board_dirty=true",
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
            limit: 3,
            atom_limit: 12,
            min_score: 0.01,
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
    Ok(())
}

struct DiagnosticLabelAtomStore {
    status_message: &'static str,
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

struct ProposalValidationStore {
    embeddings: Vec<(String, Vec<f32>)>,
    atoms: Vec<(kanban_vector::LabelAtomHit, Vec<f32>)>,
    status_message: &'static str,
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

impl kanban_vector::VectorStore for ProposalValidationStore {
    fn chunk_embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: self.status_message.to_owned(),
        }
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn upsert(
        &self,
        _chunks: &[kanban_vector::EmbeddingChunk],
    ) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn query(
        &self,
        _query: &kanban_vector::VectorQuery,
    ) -> Result<Vec<kanban_vector::VectorHit>, kanban_vector::VectorError> {
        Ok(Vec::new())
    }

    fn embed_query_text(&self, text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(self.embedding_for(text))
    }

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
                hit.score = (1.0 / similarity.max(0.0001)) - 1.0;
                kanban_vector::LabelAtomVectorHit {
                    hit,
                    vector: query.include_vector.then_some(vector),
                }
            })
            .collect())
    }
}

impl kanban_vector::VectorStore for DiagnosticLabelAtomStore {
    fn chunk_embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: self.status_message.to_owned(),
        }
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn upsert(
        &self,
        _chunks: &[kanban_vector::EmbeddingChunk],
    ) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn query(
        &self,
        _query: &kanban_vector::VectorQuery,
    ) -> Result<Vec<kanban_vector::VectorHit>, kanban_vector::VectorError> {
        Ok(Vec::new())
    }

    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(vec![1.0, 0.0, 0.0])
    }

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
    queries: std::sync::Mutex<Vec<kanban_vector::LabelAtomVectorQuery>>,
}

impl ResidualRecordingLabelAtomStore {
    fn new(atoms: Vec<(kanban_vector::LabelAtomHit, Vec<f32>)>) -> Self {
        Self {
            atoms,
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

impl kanban_vector::VectorStore for ResidualRecordingLabelAtomStore {
    fn chunk_embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: "test vector store; dirty=false last_error=none; board_dirty=false".to_owned(),
        }
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn upsert(
        &self,
        _chunks: &[kanban_vector::EmbeddingChunk],
    ) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn query(
        &self,
        _query: &kanban_vector::VectorQuery,
    ) -> Result<Vec<kanban_vector::VectorHit>, kanban_vector::VectorError> {
        Ok(Vec::new())
    }

    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(vec![1.0, 1.0, 0.0])
    }

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
                hit.score = (1.0 / similarity.max(0.0001)) - 1.0;
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

struct StaticLabelAtomStore {
    hits: Vec<kanban_vector::LabelAtomHit>,
}

impl kanban_vector::VectorStore for StaticLabelAtomStore {
    fn chunk_embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: "test vector store; dirty=false last_error=none; board_dirty=false".to_owned(),
        }
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn upsert(
        &self,
        _chunks: &[kanban_vector::EmbeddingChunk],
    ) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn query(
        &self,
        _query: &kanban_vector::VectorQuery,
    ) -> Result<Vec<kanban_vector::VectorHit>, kanban_vector::VectorError> {
        Ok(Vec::new())
    }

    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(vec![1.0, 0.0, 0.0])
    }

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
        score: distance,
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
        },
    )?;

    assert_eq!(semantics.label_id, label.id);
    assert_eq!(semantics.applies_when, vec!["touches server code"]);
    assert_eq!(semantics.atoms.len(), 6);
    assert_eq!(semantics.atoms[0].kind, "name");
    assert_eq!(semantics.atoms[0].text, "backend");
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

    delete_label_semantics(&temp.path, "default", "backend")?;
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
    delete_label_semantics(&temp.path, "default", "l_foo")?;
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

        assert_eq!(report.migration_version, Some(9));
        assert_eq!(report.user_version, 9);
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
    assert!(status.message.contains("rebuilt 3 label atom(s)"));
    assert_eq!(store.upserted_label_atoms()?.len(), 3);
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
    assert_eq!(hits.len(), 2);
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
    assert_eq!(vector_hits.len(), 2);
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
    assert!(status.message.contains("dirty=true"));
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
    assert!(status.message.contains("dirty=true"));
    assert!(status.message.contains("board_dirty=true"));

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
