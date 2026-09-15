use std::collections::BTreeMap;

use super::*;

const TASK: &str = "t_ontology_adapter";
const LABEL: &str = "adapter";

fn board_path() -> wire::BoardLabelPath {
    wire::BoardLabelPath {
        board: "default".to_owned(),
    }
}

fn task_path() -> wire::TaskLabelSurfacePath {
    wire::TaskLabelSurfacePath {
        task_id: TASK.to_owned(),
    }
}

fn label_path() -> wire::LabelSemanticsPath {
    wire::LabelSemanticsPath {
        board: "default".to_owned(),
        label_id: LABEL.to_owned(),
    }
}

fn suggestion_query(board: Option<&str>) -> wire::rpc::dto::TaskLabelSuggestionQuery {
    wire::rpc::dto::TaskLabelSuggestionQuery {
        board: board.map(str::to_owned),
        limit: 5,
        candidate_limit: 32,
        atom_limit: 80,
        max_selected_labels: 4,
        min_score: 0.15,
    }
}

async fn fixture() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("ontology.db"), "host-default")
        .await
        .unwrap();
    state
        .application()
        .create_board_label(service::CreateBoardLabelCommand {
            board: "default".to_owned(),
            name: LABEL.to_owned(),
            color: None,
        })
        .await
        .unwrap();
    state
        .application()
        .create_task(service::operations::CreateTaskCommand {
            task_id: TASK.to_owned(),
            board: "default".to_owned(),
            idempotency_key: None,
            title: "Typed ontology adapter".to_owned(),
            description: Some("retain transport fields".to_owned()),
            requested_status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata: BTreeMap::new(),
            labels: Vec::new(),
            depends_on: Vec::new(),
            actor: "fixture".to_owned(),
        })
        .await
        .unwrap();
    (directory, state)
}

fn upsert_body(actor: Option<&str>, reason: &str) -> wire::UpsertLabelSemanticsRequest {
    wire::UpsertLabelSemanticsRequest {
        actor: actor.map(str::to_owned),
        expected_semantics_hash: None,
        replace: true,
        reason: Some(reason.to_owned()),
        source_signal_ids: Vec::new(),
        description: Some(reason.to_owned()),
        applies_when: Some(vec!["typed RPC".to_owned()]),
        excludes_when: Some(Vec::new()),
        positive_examples: Some(Vec::new()),
        negative_examples: Some(Vec::new()),
        remove_applies_when: Vec::new(),
        remove_excludes_when: Vec::new(),
        remove_positive_examples: Vec::new(),
        remove_negative_examples: Vec::new(),
    }
}

fn proposal_body(actor: Option<&str>, nested: Option<&str>) -> wire::ProposeTaskLabelRequest {
    wire::ProposeTaskLabelRequest {
        proposal: Some(wire::LabelProposalCandidateWire {
            name: "manual candidate".to_owned(),
            description: None,
            applies_when: vec!["new requirement".to_owned()],
            excludes_when: Vec::new(),
            positive_examples: Vec::new(),
            negative_examples: Vec::new(),
        }),
        actor: actor.map(str::to_owned),
        source_signal_ids: Vec::new(),
        ontology_actor: nested.map(|name| wire::LabelOntologyActorWire {
            name: name.to_owned(),
            actor_type: "agent".to_owned(),
            agent_type: Some("codex".to_owned()),
        }),
        allow_retarget: false,
        retarget_reason: None,
    }
}

fn observation_body() -> wire::RecordLabelOntologyObservationRequest {
    serde_json::from_value(json!({
        "actor": {"name": "中文 agent", "type": "agent", "agent_type": "codex"},
        "agent_candidates": [{"name": LABEL, "large": u64::MAX}],
        "suggestion_snapshot": {"signed": i64::MIN},
        "final_decision": {"explicit": false},
        "suggest_coverage": 0.0,
        "suggest_coverage_cosine": 0.5,
        "suggest_residual_norm": 1.0,
        "suggest_needs_new_label": false,
        "suggest_degraded": true,
        "diagnostics": ["provider unavailable"],
        "capture_fingerprint": "adapter-capture",
        "signals": [{
            "kind": "vocabulary_gap", "target_label_ref": LABEL, "related_labels": [LABEL],
            "proposed_action": "observe", "candidate_atom": {"polarity": "positive", "kind": "applies_when", "text": "new calls"},
            "proposal": {"large": u64::MAX}, "agent_selected": true, "suggest_state": "candidate",
            "suggest_score": 0.0, "suggest_rank": 1, "final_selected": false,
            "rationale": "typed evidence", "confidence": 0.5, "signal_key": "adapter-signal"
        }]
    })).unwrap()
}

#[tokio::test]
async fn actor_body_context_defaults_and_blank_rejection_are_explicit() {
    let (_directory, state) = fixture().await;
    for (index, body_actor, context_actor, expected) in [
        (0, None, None, "http"),
        (1, None, Some(" 上下文 "), "上下文"),
        (2, Some(" 正文 "), Some("context"), "正文"),
    ] {
        let reason = format!("actor-source-{index}");
        let response = upsert_semantics(
            state.clone(),
            label_path(),
            CallContext {
                actor: context_actor.map(str::to_owned),
            },
            upsert_body(body_actor, &reason),
        )
        .await
        .unwrap();
        let changed_atom = response
            .data
            .atoms
            .iter()
            .find(|atom| atom.text == reason)
            .unwrap();
        let explanation = state
            .application()
            .explain_label_atom("default", &changed_atom.id)
            .await
            .unwrap();
        let action = explanation
            .provenance_actions
            .into_iter()
            .find(|record| record.action.reason == reason)
            .unwrap()
            .action;
        assert_eq!(action.created_by, expected);
    }
    for (body_actor, context_actor) in [(Some("  "), Some("context")), (None, Some("  "))] {
        let error = upsert_semantics(
            state.clone(),
            label_path(),
            CallContext {
                actor: context_actor.map(str::to_owned),
            },
            upsert_body(body_actor, "invalid actor"),
        )
        .await
        .unwrap_err();
        assert!(matches!(error.0, service::KanbanError::InvalidInput(_)));
    }
    for (body_actor, nested_actor, context_actor, expected) in [
        (None, None, None, "user"),
        (None, None, Some(" 上下文 "), "上下文"),
        (None, Some(" 嵌套 "), Some("context"), "嵌套"),
        (Some(" 正文 "), Some("nested"), Some("context"), "正文"),
    ] {
        let response = propose_for_task(
            state.clone(),
            task_path(),
            CallContext {
                actor: context_actor.map(str::to_owned),
            },
            suggestion_query(None),
            Some(proposal_body(body_actor, nested_actor)),
        )
        .await
        .unwrap();
        assert_eq!(response.data.proposal.unwrap().created_by, expected);
    }
    let error = propose_for_task(
        state.clone(),
        task_path(),
        CallContext {
            actor: Some("context".to_owned()),
        },
        suggestion_query(None),
        Some(proposal_body(Some(" "), None)),
    )
    .await
    .unwrap_err();
    assert!(matches!(error.0, service::KanbanError::InvalidInput(_)));
    let current = get_semantics(state.clone(), label_path())
        .await
        .unwrap()
        .data;
    let deleted = delete_semantics(
        state.clone(),
        label_path(),
        CallContext {
            actor: Some(" 删除者 ".to_owned()),
        },
        wire::DeleteLabelSemanticsQuery {
            expected_semantics_hash: current.semantics_hash,
            reason: "delete actor evidence".to_owned(),
        },
    )
    .await
    .unwrap();
    assert!(deleted.data.deleted);
    let explanation = state
        .application()
        .explain_label_atom("default", &current.atoms[0].content_hash)
        .await
        .unwrap();
    assert!(
        explanation
            .provenance_actions
            .iter()
            .any(|record| record.action.reason == "delete actor evidence"
                && record.action.created_by == "删除者")
    );
}

#[tokio::test]
async fn typed_records_roundtrip_natural_json_quality_and_index_diagnostics() {
    let (_directory, state) = fixture().await;
    let semantics = upsert_semantics(
        state.clone(),
        label_path(),
        CallContext::default(),
        upsert_body(None, "seed"),
    )
    .await
    .unwrap()
    .data;
    assert_eq!(
        list_semantics(state.clone(), board_path())
            .await
            .unwrap()
            .data,
        vec![semantics.clone()]
    );
    assert_eq!(
        list_atoms(state.clone(), board_path()).await.unwrap().data,
        semantics.atoms
    );
    let index = query_index(
        state.clone(),
        board_path(),
        serde_json::from_value(json!({"q": "typed RPC", "limit": 1})).unwrap(),
    )
    .await
    .unwrap()
    .data;
    assert_eq!(index.data.len(), 1);
    assert_eq!(index.data[0].text, "typed RPC");
    assert!(index.degraded);
    assert_eq!(index.diagnostics, ["vector_provider_unavailable"]);
    assert_eq!(index.data[0].embedding_model, "unavailable");
    index_status(state.clone(), board_path()).await.unwrap();
    rebuild_index(state.clone(), board_path()).await.unwrap();
    let explained = explain_atom(
        state.clone(),
        wire::LabelAtomPath {
            board: "default".to_owned(),
            atom_ref: index.data[0].atom_id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(explained.data.atom.unwrap().text, "typed RPC");

    let response = record_observation(
        state.clone(),
        task_path(),
        wire::rpc::dto::TaskLabelBoardQuery {
            board: Some(" default ".to_owned()),
        },
        observation_body(),
    )
    .await
    .unwrap();
    assert_eq!(response.data.agent_candidates.0[0]["large"], u64::MAX);
    assert_eq!(response.data.suggestion_snapshot.0["signed"], i64::MIN);
    assert_eq!(response.data.created_by, "中文 agent");
    assert_eq!(response.data.agent_type.as_deref(), Some("codex"));
    let signal_id = response.data.signals[0].id.clone();
    let body: wire::LabelOntologyActionRequest = serde_json::from_value(json!({
        "actor": {"name": "reviewer", "type": "user"}, "idempotency_key": "adapter-action",
        "action_type": "confirm", "signal_ids": [signal_id], "reason": "confirm once",
        "target_label_ref": LABEL, "change": {"large": u64::MAX}, "validation": {"checked": true}
    }))
    .unwrap();
    let action = create_action(state.clone(), board_path(), body.clone())
        .await
        .unwrap();
    assert_eq!(
        action,
        create_action(state.clone(), board_path(), body)
            .await
            .unwrap()
    );
    assert_eq!(action.data.change.0["large"], u64::MAX);
    assert!(!action.data.change.0.contains_key("_idempotency_key"));
    assert!(
        !action
            .data
            .change
            .0
            .contains_key("_idempotency_fingerprint")
    );
    let serialized = serde_json::to_value(&action).unwrap();
    assert!(
        serialized["data"]
            .get("parent_action_id")
            .is_some_and(Value::is_null)
    );
    assert!(
        serialized["data"]
            .get("validation_latest_attempt_id")
            .is_some_and(Value::is_null)
    );
    let detail = get_signal(
        state.clone(),
        wire::SignalPath {
            signal_id: signal_id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(detail.data.actions, vec![action.data]);
    assert_eq!(detail.data.signal.related_labels.0, vec![json!(LABEL)]);
    let listed = list_signals(
        state.clone(),
        board_path(),
        serde_json::from_value(json!({"status": ["confirmed,open"], "limit": 1})).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(listed.data[0].id, signal_id);
    assert_eq!(listed.meta.limit, 1);
    assert!(!listed.meta.include_all);
    let review = review_signals(
        state.clone(),
        board_path(),
        serde_json::from_value(json!({"group_by": "label", "include_all": true, "limit": 3}))
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(review.meta.group_by, "label");
    assert!(review.meta.include_all);
    assert_eq!(review.meta.limit, 3);
    assert_eq!(review.data[0].labels[0].name, None);
    let quality = get_label_ontology_quality(state.clone(), board_path(), 1)
        .await
        .unwrap();
    assert_eq!(quality.data.denominator.observation_count, 1);
    assert_eq!(quality.data.denominator.degraded_observation_count, 1);
    assert_eq!(quality.data.disagreement.by_kind["vocabulary_gap"], 1);
    assert_eq!(quality.data.rates.disagreement_task_rate, None);
    assert!(!quality.data.precision_recall.available);

    let applied = apply_atom(state.clone(), board_path(), serde_json::from_value(json!({
        "actor": {"name": "tester", "type": "user"}, "signal_ids": [signal_id],
        "label_ref": LABEL, "kind": "excludes_when", "text": "remote changes", "reason": "negative boundary"
    })).unwrap()).await.unwrap().data;
    assert_eq!(
        applied.action_type,
        wire::LabelOntologyActionTypeWire::AddNegativeAtom
    );
    assert_eq!(applied.change.0["added_atom"]["polarity"], "negative");
    let validated = validate(
        state.clone(),
        board_path(),
        serde_json::from_value(json!({
            "actor": {"name": "tester", "type": "user"}, "parent_action_id": applied.id,
            "signal_ids": [signal_id], "reason": "verify", "validation_status": "passed",
            "validation": {"manual": {"large": u64::MAX}, "cases": ["typed case"]}
        }))
        .unwrap(),
    )
    .await
    .unwrap()
    .data;
    assert_eq!(
        validated.validation_status,
        wire::LabelOntologyValidationStatusWire::Passed
    );
    assert_eq!(
        validated.validation_effective_outcome,
        wire::LabelOntologyValidationEffectiveOutcomeWire::Passed
    );
    assert_eq!(validated.validation.0["manual"]["large"], u64::MAX);
    let reverted = revert(
        state.clone(),
        board_path(),
        serde_json::from_value(json!({
            "actor": {"name": "tester", "type": "user"}, "target_action_id": applied.id,
            "expected_current_hash": applied.canonical_after_hash, "reason": "restore boundary"
        }))
        .unwrap(),
    )
    .await
    .unwrap()
    .data;
    assert_eq!(
        reverted.canonical_after_hash.as_deref(),
        Some(semantics.semantics_hash.as_str())
    );

    let detail = state
        .application()
        .get_label_ontology_signal(&signal_id)
        .await
        .unwrap();
    let supporting = convert::label_atom_explain_signal(
        &state,
        service::LabelAtomExplainSignalRecord {
            signal: detail.signal,
            observation: detail.observation,
            task_id: TASK.to_owned(),
            task_ref_snapshot: "default#1".to_owned(),
            suggest_input_stale: true,
            suggest_degraded: true,
            warnings: vec!["historical evidence".to_owned()],
        },
    )
    .await
    .unwrap();
    assert_eq!(supporting.source_task.id, TASK);
    assert!(supporting.suggest_input_stale);
    assert_eq!(
        supporting.observation.agent_candidates.0[0]["large"],
        u64::MAX
    );

    let mut invalid = observation_body();
    invalid.capture_fingerprint = Some("null-diagnostics".to_owned());
    invalid.diagnostics = wire::JsonBodyFieldWire::Present(Value::Null);
    assert!(
        record_observation(
            state.clone(),
            task_path(),
            wire::rpc::dto::TaskLabelBoardQuery::default(),
            invalid
        )
        .await
        .is_err()
    );
    assert_eq!(
        get_label_ontology_quality(state, board_path(), 1)
            .await
            .unwrap()
            .data
            .denominator
            .observation_count,
        1
    );
}

#[tokio::test]
async fn task_queries_preserve_board_guard_status_filter_and_explicit_limits() {
    let (_directory, state) = fixture().await;
    let first = propose_for_task(
        state.clone(),
        task_path(),
        CallContext::default(),
        suggestion_query(Some("default")),
        Some(proposal_body(None, None)),
    )
    .await
    .unwrap()
    .data
    .proposal
    .unwrap();
    let second = propose_for_task(
        state.clone(),
        task_path(),
        CallContext::default(),
        suggestion_query(Some("default")),
        Some(proposal_body(None, None)),
    )
    .await
    .unwrap()
    .data
    .proposal
    .unwrap();
    let decision: wire::LabelProposalDecisionRequest = serde_json::from_value(json!({"reason": "accept", "actor": " body ", "ontology_actor": {"name": "nested", "type": "user"}})).unwrap();
    accept_proposal(
        state.clone(),
        wire::ProposalPath {
            proposal_id: first.id.clone(),
        },
        CallContext {
            actor: Some("context".to_owned()),
        },
        decision,
    )
    .await
    .unwrap();
    let events = state
        .application()
        .list_events(
            "default",
            service::EventListOptions {
                task_id: None,
                after: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();
    assert_eq!(events.events.last().unwrap().actor.as_deref(), Some("body"));
    let accepted = list_proposals_for_task(
        state.clone(),
        task_path(),
        wire::rpc::dto::TaskLabelProposalQuery {
            board: Some(first.board_id.clone()),
            status: Some("accepted".to_owned()),
        },
    )
    .await
    .unwrap();
    assert_eq!(accepted.data.len(), 1);
    assert_eq!(accepted.data[0].id, first.id);
    let proposed = list_proposals_for_task(
        state.clone(),
        task_path(),
        wire::rpc::dto::TaskLabelProposalQuery {
            board: Some("default".to_owned()),
            status: Some("proposed".to_owned()),
        },
    )
    .await
    .unwrap();
    assert_eq!(proposed.data.len(), 1);
    assert_eq!(proposed.data[0].id, second.id);
    let listed = list_proposals_for_board(
        state.clone(),
        wire::ListBoardLabelProposalsPath {
            board: "default".to_owned(),
        },
        wire::ListBoardLabelProposalsQuery {
            status: Some(wire::LabelProposalStatusWire::Proposed),
        },
    )
    .await
    .unwrap();
    assert_eq!(listed, proposed);
    let shown = get_proposal(
        state.clone(),
        wire::ProposalPath {
            proposal_id: second.id.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(shown.data, second);
    reject_proposal(
        state.clone(),
        wire::ProposalPath {
            proposal_id: second.id,
        },
        CallContext {
            actor: Some("context".to_owned()),
        },
        serde_json::from_value(json!({"reason": "reject"})).unwrap(),
    )
    .await
    .unwrap();
    let events = state
        .application()
        .list_events(
            "default",
            service::EventListOptions {
                task_id: None,
                after: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        events.events.last().unwrap().actor.as_deref(),
        Some("context")
    );

    assert!(
        list_proposals_for_task(
            state.clone(),
            task_path(),
            wire::rpc::dto::TaskLabelProposalQuery {
                board: Some("other".to_owned()),
                status: None
            }
        )
        .await
        .is_err()
    );
    assert!(
        suggestions(state.clone(), task_path(), suggestion_query(Some("other")))
            .await
            .is_err()
    );
    assert!(
        propose_for_task(
            state.clone(),
            task_path(),
            CallContext::default(),
            suggestion_query(Some("other")),
            Some(proposal_body(None, None))
        )
        .await
        .is_err()
    );
    assert!(
        record_observation(
            state.clone(),
            task_path(),
            wire::rpc::dto::TaskLabelBoardQuery {
                board: Some("other".to_owned())
            },
            observation_body()
        )
        .await
        .is_err()
    );
    let mut query = suggestion_query(Some("default"));
    query.limit = 0;
    assert!(matches!(
        suggestions(state.clone(), task_path(), query)
            .await
            .unwrap_err()
            .0,
        service::KanbanError::InvalidInput(_)
    ));
    assert!(
        suggestions(state.clone(), task_path(), suggestion_query(Some(" ")))
            .await
            .unwrap()
            .data
            .degraded
    );
    let absent = propose_for_task(
        state,
        task_path(),
        CallContext::default(),
        suggestion_query(None),
        None,
    )
    .await
    .unwrap();
    assert!(absent.data.proposal.is_none());
    assert!(absent.data.degraded);
}

#[test]
fn missing_json_uses_the_owned_default_and_explicit_null_stays_present() {
    assert_eq!(
        body_value(wire::JsonBodyFieldWire::Missing, json!([])),
        json!([])
    );
    assert_eq!(
        body_value(wire::JsonBodyFieldWire::Missing, json!({})),
        json!({})
    );
    assert_eq!(
        body_value(wire::JsonBodyFieldWire::Present(Value::Null), json!({})),
        Value::Null
    );
}
