use crate::common::*;

use serde_json::json;

#[test]
fn label_ontology_migration_creates_ledger_tables_and_json_constraints() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_migration_creates_ledger_tables_and_json_constraints")?;

    init_database(&temp.path, "tester")?;

    let conn = connect_file(&temp.path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 14);
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
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

    let validation = validate_label_ontology_action_with_trusted_evidence(
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
fn label_ontology_jsonl_export_import_round_trips_ledger_and_self_refs() -> anyhow::Result<()> {
    let source =
        TempDb::new("label_ontology_jsonl_export_import_round_trips_ledger_and_self_refs_source")?;
    let fixture = seed_portable_ontology_ledger(&source)?;
    let export_path = source.dir.join("ontology.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let export = std::fs::read_to_string(&export_path)?;
    for record_type in [
        "label_ontology_observation",
        "label_ontology_signal",
        "label_ontology_action",
        "label_ontology_action_signal",
    ] {
        assert!(
            export.contains(&format!("\"type\":\"{record_type}\"")),
            "missing {record_type} in export:\n{export}"
        );
    }

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
    assert_eq!(observation_count, 1);
    assert_eq!(signal_count, 2);
    assert_eq!(action_count, 4);
    assert_eq!(link_count, 4);

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

    let observation_candidates: String = conn.query_row(
        "SELECT agent_candidates_json FROM label_ontology_observations WHERE id=?1",
        [&fixture.observation_id],
        |row| row.get(0),
    )?;
    assert!(observation_candidates.contains("adds CLI command surface"));
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
    let error = result_err(import_jsonl(&target.path, &invalid_export, true))?;

    assert!(
        error
            .to_string()
            .contains("label ontology action-signal board mismatch"),
        "error: {error}"
    );
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
    claim_task(&temp.path, "default", "worker", &fixture.task.id, 300_000)?;

    let validation = validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        passed_validation_input(&fixture, "Status-only task drift must remain comparable."),
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

    let validation = validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        passed_validation_input(&fixture, "Label binding drift must remain comparable."),
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
fn label_ontology_typed_validation_rejects_dirty_atom_index() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_typed_validation_rejects_dirty_atom_index")?;
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Dirty atom index cannot pass automated validation.",
        ),
    ))?;
    assert!(error.to_string().contains("dirty atom index"));

    Ok(())
}

#[cfg(feature = "vector-lancedb")]
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

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_ontology_trusted_collector_rechecks_index_generation() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_trusted_collector_rechecks_index_generation")?;
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Tool-collected validation must reject stale generation evidence.",
        ),
    ))?;
    assert!(error.to_string().contains("generation changed"));
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
fn label_ontology_typed_validation_positive_atom_requires_result_atom_evidence()
-> anyhow::Result<()> {
    let temp =
        TempDb::new("label_ontology_typed_validation_positive_atom_requires_result_atom_evidence")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_validation_fixture(
        &temp,
        "Add ontology validation result atom guard",
        "cli-result-atom-evidence",
    )?;
    let mut validation_json = typed_positive_fixture_json(&fixture);
    validation_json["cases"][0]["after"]["evidence_atoms"] = json!([]);

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Positive atom validation must cite the applied result atom.",
        ),
    ))?;
    assert!(error.to_string().contains("result atom"));

    Ok(())
}

#[test]
fn label_ontology_typed_validation_negative_atom_accepts_negative_evidence_atoms()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_typed_validation_negative_atom_accepts_negative_evidence_atoms",
    )?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom evidence",
        "cli-negative-evidence-success",
    )?;
    let validation_json = typed_negative_fixture_json(&fixture, true);

    let validation = validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Negative atom validation should accept true negative evidence.",
        ),
    )?;

    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Passed
    );
    let resolved = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    assert_eq!(resolved.signal.status, LabelOntologySignalStatus::Resolved);

    Ok(())
}

#[test]
fn label_ontology_typed_validation_negative_atom_rejects_positive_evidence_slot()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_typed_validation_negative_atom_rejects_positive_evidence_slot",
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Negative atom validation must cite negative evidence atoms.",
        ),
    ))?;
    assert!(error.to_string().contains("negative_evidence_atoms"));

    Ok(())
}

#[test]
fn label_ontology_typed_validation_negative_atom_requires_suppression_proof() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_typed_validation_negative_atom_requires_suppression_proof")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom suppression",
        "cli-negative-suppression",
    )?;
    let mut validation_json = typed_negative_fixture_json(&fixture, true);
    validation_json["cases"][0]["after"]["target"]["selected"] = json!(true);
    validation_json["cases"][0]["after"]["target"]["score"] = json!(0.82);

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Negative atom validation must prove target suppression.",
        ),
    ))?;
    assert!(error.to_string().contains("selected=false"));

    Ok(())
}

#[test]
fn label_ontology_typed_validation_negative_atom_rejects_missing_controls_without_waiver()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "label_ontology_typed_validation_negative_atom_rejects_missing_controls_without_waiver",
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Negative atom validation must include controls or waiver.",
        ),
    ))?;
    assert!(error.to_string().contains("positive control"));

    Ok(())
}

#[test]
fn label_ontology_typed_validation_negative_atom_accepts_waiver_reason() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_typed_validation_negative_atom_accepts_waiver_reason")?;
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

    let validation = validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Negative atom validation should accept explicit positive-control waiver.",
        ),
    )?;

    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Passed
    );

    Ok(())
}

#[test]
fn label_ontology_typed_validation_negative_atom_rejects_empty_waiver_reason() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_typed_validation_negative_atom_rejects_empty_waiver_reason")?;
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Negative atom validation must require a non-empty waiver reason.",
        ),
    ))?;
    assert!(error.to_string().contains("non-empty reason"));

    Ok(())
}

#[test]
fn label_ontology_typed_validation_negative_atom_protects_positive_controls() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("label_ontology_typed_validation_negative_atom_protects_positive_controls")?;
    init_database(&temp.path, "tester")?;
    let fixture = seed_negative_validation_fixture(
        &temp,
        "Add ontology validation negative atom controls",
        "cli-negative-positive-control",
    )?;
    let validation_json = typed_negative_fixture_json(&fixture, false);

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
            &fixture.apply_action_id,
            validation_json,
            "Negative atom validation must protect positive control tasks.",
        ),
    ))?;
    assert!(error.to_string().contains("positive control"));

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
    let validation =
        validate_label_ontology_action_with_trusted_evidence(&temp.path, "default", input)?;

    assert_eq!(
        validation.validation_status,
        LabelOntologyValidationStatus::Failed
    );
    let signal = get_label_ontology_signal(&temp.path, &fixture.signal_id)?;
    assert_eq!(signal.signal.status, LabelOntologySignalStatus::Confirmed);

    Ok(())
}

#[test]
fn label_ontology_typed_validation_bootstrap_enforces_score_threshold() -> anyhow::Result<()> {
    let temp = TempDb::new("label_ontology_typed_validation_bootstrap_enforces_score_threshold")?;
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

    let error = result_err(validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        validation_input_from_json(
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
            "Bootstrap validation must meet the source-task score threshold.",
        ),
    ))?;
    assert!(error.to_string().contains("bootstrap label"));

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

    let validation = validate_label_ontology_action_with_trusted_evidence(
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

    let validation = validate_label_ontology_action_with_trusted_evidence(
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
    let semantics = get_label_semantics(&temp.path, "default", result_label_id)?;
    assert_eq!(semantics.label_name, "ontology-ledger");
    let atom = semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;
    let explain = explain_label_atom(&temp.path, "default", &atom.id)?;
    assert!(!explain.legacy_untracked);
    let atom_provenance = explain
        .provenance_actions
        .iter()
        .find(|action| {
            action.action.action_type == LabelOntologyActionType::BootstrapLabel
                && action.action.result_atom_id.as_deref() == Some(atom.id.as_str())
                && action.action.result_proposal_id.as_deref() == Some(proposal_id.as_str())
        })
        .context("proposal atom provenance action")?;
    assert_eq!(atom_provenance.action.signal_ids, vec![signal_id.clone()]);
    assert_eq!(atom_provenance.action.created_by, "ontology-agent");
    assert_eq!(
        atom_provenance.action.parent_action_id.as_deref(),
        Some(bootstrap.id.as_str())
    );

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

struct PortableOntologyLedgerFixture {
    observation_id: String,
    source_signal_id: String,
    duplicate_signal_id: String,
    apply_action_id: String,
    validation_action_id: String,
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
    let validation = validate_label_ontology_action_with_trusted_evidence(
        &temp.path,
        "default",
        LabelOntologyValidationInput {
            actor: validation_actor(),
            parent_action_id: apply_action.id.clone(),
            signal_ids: Vec::new(),
            reason: "Portable ontology validation passed.".to_owned(),
            validation_status: LabelOntologyValidationStatus::Passed,
            validation_json: typed_positive_validation_json(
                &source_signal_id,
                &label.id,
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
    Ok(PortableOntologyLedgerFixture {
        observation_id: observation.id,
        source_signal_id,
        duplicate_signal_id,
        apply_action_id: apply_action.id,
        validation_action_id: validation.id,
    })
}

fn write_reordered_ontology_export(input_path: &Path, output_path: &Path) -> anyhow::Result<()> {
    let mut non_ontology = Vec::new();
    let mut observations = Vec::new();
    let mut signals = Vec::new();
    let mut actions = Vec::new();
    let mut links = Vec::new();
    for line in std::fs::read_to_string(input_path)?.lines() {
        let value: serde_json::Value = serde_json::from_str(line)?;
        match value["type"].as_str() {
            Some("label_ontology_observation") => observations.push(line.to_owned()),
            Some("label_ontology_signal") => signals.push(line.to_owned()),
            Some("label_ontology_action") => actions.push(line.to_owned()),
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
