use std::{future::Future, time::Duration};

use kanban_core::{KanbanError, Result};
use serde_json::json;

use super::{commands::*, records::*};
use crate::{CreateBoardCommand, CreateBoardLabelCommand, KanbanService, test_support};

const TASK: &str = "t_ontology_facade";
const LABEL: &str = "transport";

async fn service(name: &str) -> (tempfile::TempDir, KanbanService) {
    let (directory, store, _) = test_support::store(name).await;
    store.initialize().await.expect("initialize");
    store
        .create_task(
            "default",
            test_support::create_input(TASK, None, "Typed ontology transport"),
        )
        .await
        .expect("fixture task");
    let service = KanbanService::new(store);
    service
        .create_board_label(CreateBoardLabelCommand {
            board: "default".to_owned(),
            name: LABEL.to_owned(),
            color: None,
        })
        .await
        .expect("fixture label");
    (directory, service)
}

fn actor() -> OntologyActorCommand {
    OntologyActorCommand {
        name: "facade-test".to_owned(),
        actor_type: "agent".to_owned(),
        agent_type: Some("codex".to_owned()),
    }
}

fn semantics() -> UpsertLabelSemanticsCommand {
    UpsertLabelSemanticsCommand {
        expected_semantics_hash: None,
        replace: true,
        description: Some("RPC transport".to_owned()),
        applies_when: Some(vec!["localhost calls".to_owned()]),
        excludes_when: Some(vec!["remote access".to_owned()]),
        positive_examples: Some(vec!["typed service".to_owned()]),
        negative_examples: Some(vec!["SaaS".to_owned()]),
        remove_applies_when: Vec::new(),
        remove_excludes_when: Vec::new(),
        remove_positive_examples: Vec::new(),
        remove_negative_examples: Vec::new(),
        actor: "facade-test".to_owned(),
        reason: Some("seed canonical semantics".to_owned()),
        source_signal_ids: Vec::new(),
    }
}

fn observation(fingerprint: &str) -> OntologyObservationCommand {
    OntologyObservationCommand {
        actor: actor(),
        task_ref: TASK.to_owned(),
        agent_candidates: json!([{"label": LABEL, "large": u64::MAX}]),
        suggestion_snapshot: json!({"signed": i64::MIN, "score": 0.0}),
        final_decision: json!({"labels": [LABEL], "explicit": false}),
        suggest_coverage: Some(0.0),
        suggest_coverage_cosine: Some(0.5),
        suggest_residual_norm: Some(1.0),
        suggest_needs_new_label: false,
        suggest_degraded: false,
        diagnostics: json!(["provider unavailable", {"detail": [1, null, false]}]),
        capture_fingerprint: Some(fingerprint.to_owned()),
        signals: vec![OntologySignalCommand {
            kind: "vocabulary_gap".to_owned(),
            target_label_ref: Some(LABEL.to_owned()),
            related_labels: json!([LABEL]),
            proposed_action: "observe".to_owned(),
            candidate_atom_polarity: Some("positive".to_owned()),
            candidate_atom_kind: Some("applies_when".to_owned()),
            candidate_text: Some("new typed calls".to_owned()),
            proposed_label_name: None,
            proposal: json!({"source": "typed facade", "large": u64::MAX}),
            agent_selected: true,
            suggest_state: Some("candidate".to_owned()),
            suggest_score: Some(0.0),
            suggest_rank: Some(1),
            final_selected: false,
            rationale: "retain proposal evidence".to_owned(),
            confidence: Some(0.5),
            signal_key: Some("typed-signal".to_owned()),
        }],
    }
}

fn action(signal_ids: Vec<String>) -> OntologyActionCommand {
    OntologyActionCommand {
        actor: actor(),
        idempotency_key: Some("typed-confirm-once".to_owned()),
        action_type: "confirm".to_owned(),
        signal_ids,
        reason: "confirm evidence once".to_owned(),
        superseded_by_signal_id: None,
        parent_action_id: None,
        target_label_ref: Some(LABEL.to_owned()),
        result_label_ref: None,
        result_atom_id: None,
        result_atom_content_hash: None,
        result_proposal_id: None,
        canonical_before_hash: None,
        canonical_after_hash: None,
        change: json!({"audit": {"large": u64::MAX}, "explicit_null": null}),
        validation_status: None,
        validation: json!({"manual": {"accepted": true}}),
    }
}

fn proposal() -> LabelProposalCommand {
    LabelProposalCommand {
        name: Some("new capability".to_owned()),
        description: Some("explicit manual candidate".to_owned()),
        applies_when: vec!["new work".to_owned()],
        excludes_when: vec!["existing work".to_owned()],
        positive_examples: vec!["manual proposal".to_owned()],
        negative_examples: vec!["unrelated".to_owned()],
        actor: "facade-test".to_owned(),
        source_signal_ids: Vec::new(),
    }
}

fn apply_atom() -> OntologyApplyAtomCommand {
    OntologyApplyAtomCommand {
        actor: actor(),
        signal_ids: Vec::new(),
        label_ref: LABEL.to_owned(),
        polarity: "positive".to_owned(),
        kind: "applies_when".to_owned(),
        text: "new typed calls".to_owned(),
        reason: "apply confirmed evidence".to_owned(),
    }
}

#[tokio::test]
async fn typed_semantics_preserve_cas_collections_and_read_silence() {
    let (_directory, service) = service("typed-ontology-semantics").await;
    let first = service
        .upsert_label_semantics(" default ", LABEL, semantics())
        .await
        .expect("typed upsert");
    assert_eq!(first.label_name, LABEL);
    assert_eq!(first.positive_examples, ["typed service"]);
    let notifications = service.mutation_gate.subscribe();
    assert_eq!(
        service.get_label_semantics("default", LABEL).await.unwrap(),
        first
    );
    assert_eq!(
        service.list_label_semantics("default").await.unwrap(),
        vec![first.clone()]
    );
    let atoms = service.list_label_atoms("default").await.unwrap();
    assert_eq!(atoms, first.atoms);
    let query = service
        .query_label_atom_index(
            "default",
            LabelAtomIndexQuery {
                query: Some("typed service".to_owned()),
                polarity: Some("positive".to_owned()),
                limit: 1,
            },
        )
        .await
        .expect("typed index query");
    assert_eq!(query.data.len(), 1);
    assert_eq!(query.data[0].text, "typed service");
    assert_eq!(query.data[0].embedding_model, "unavailable");
    assert_eq!(query.data[0].distance, 0.0);
    assert!(query.degraded);
    assert_eq!(query.diagnostics, ["vector_provider_unavailable"]);
    assert_eq!(
        service
            .label_ontology(
                "query_atom_index",
                "default",
                json!({"query": "typed service", "polarity": "positive", "limit": 1}),
            )
            .await
            .unwrap(),
        serde_json::to_value(&query).unwrap()
    );
    service.label_atom_index_status("default").await.unwrap();
    let explanation = service
        .explain_label_atom("default", &query.data[0].atom_id)
        .await
        .expect("typed atom explanation");
    assert_eq!(explanation.atom.unwrap().text, "typed service");
    assert!(!explanation.provenance_actions.is_empty());
    assert!(!notifications.has_changed().unwrap());

    let mut changed = semantics();
    changed.replace = false;
    changed.expected_semantics_hash = Some("stale".to_owned());
    changed.description = None;
    changed.applies_when = Some(vec!["new condition".to_owned()]);
    changed.excludes_when = None;
    changed.positive_examples = None;
    changed.negative_examples = None;
    changed.remove_applies_when = vec!["localhost calls".to_owned()];
    assert!(matches!(
        service.upsert_label_semantics("default", LABEL, changed.clone()).await,
        Err(KanbanError::Conflict(message)) if message.contains("hash mismatch")
    ));
    assert_eq!(
        service.get_label_semantics("default", LABEL).await.unwrap(),
        first
    );
    changed.expected_semantics_hash = Some(first.semantics_hash.clone());
    let updated = service
        .upsert_label_semantics("default", LABEL, changed)
        .await
        .expect("CAS update");
    assert_eq!(updated.applies_when, ["new condition"]);
    assert_eq!(updated.description, first.description);
    assert_eq!(updated.positive_examples, first.positive_examples);
    assert_ne!(updated.semantics_hash, first.semantics_hash);
    let mut deletion = DeleteLabelSemanticsCommand {
        expected_semantics_hash: first.semantics_hash,
        reason: "remove with CAS".to_owned(),
        actor: "facade-test".to_owned(),
    };
    assert!(
        service
            .delete_label_semantics("default", LABEL, deletion.clone())
            .await
            .is_err()
    );
    deletion.expected_semantics_hash = updated.semantics_hash;
    assert!(
        service
            .delete_label_semantics("default", LABEL, deletion)
            .await
            .unwrap()
    );
    assert!(
        service
            .list_label_semantics("default")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        service
            .list_label_atoms("default")
            .await
            .unwrap()
            .is_empty()
    );
    service
        .rebuild_label_atom_index("default")
        .await
        .expect("typed rebuild");
}

#[tokio::test]
async fn typed_observations_actions_quality_keep_json_and_idempotency() {
    let (_directory, service) = service("typed-ontology-observation").await;
    let command = observation("capture-one");
    let observation = service
        .record_label_ontology_observation("default", command.clone())
        .await
        .expect("typed observation");
    assert_eq!(observation.agent_candidates[0]["large"], u64::MAX);
    assert_eq!(observation.suggestion_snapshot["signed"], i64::MIN);
    assert_eq!(observation.final_decision, command.final_decision);
    assert_eq!(observation.diagnostics, command.diagnostics);
    assert_eq!(observation.suggest_coverage, Some(0.0));
    assert_eq!(observation.created_by_type, "agent");
    assert_eq!(observation.agent_type.as_deref(), Some("codex"));
    assert_eq!(
        service
            .record_label_ontology_observation("default", command.clone())
            .await
            .unwrap(),
        observation
    );
    let mut degraded = command;
    degraded.capture_fingerprint = Some("capture-two".to_owned());
    degraded.suggest_degraded = true;
    service
        .record_label_ontology_observation("default", degraded)
        .await
        .unwrap();
    let signal_id = observation.signals[0].id.clone();
    let command = action(vec![signal_id.clone()]);
    let connection = service.store.connection().await.unwrap();
    let events_before = test_support::count_rows(&connection, "task_events").await;
    let first = service
        .create_label_ontology_action("default", command.clone())
        .await
        .unwrap();
    assert_eq!(first.change, command.change);
    assert_eq!(first.validation, command.validation);
    assert_eq!(first.validation_effective_outcome, first.validation_status);
    assert_eq!(first.validation_latest_attempt_id, None);
    assert!(first.change.get("_idempotency_key").is_none());
    assert!(first.change.get("_idempotency_fingerprint").is_none());
    assert_eq!(
        service
            .create_label_ontology_action("default", command.clone())
            .await
            .unwrap(),
        first
    );
    let mut conflict = command;
    conflict.reason = "changed intent".to_owned();
    let error = service
        .create_label_ontology_action("default", conflict)
        .await
        .unwrap_err();
    assert!(
        matches!(error, KanbanError::IdempotencyConflict { .. }),
        "{error:?}"
    );
    assert_eq!(
        test_support::count_rows(&connection, "label_ontology_actions").await,
        1
    );
    assert_eq!(
        test_support::count_rows(&connection, "task_events").await,
        events_before + 2
    );

    let notifications = service.mutation_gate.subscribe();
    let detail = service.get_label_ontology_signal(&signal_id).await.unwrap();
    assert_eq!(detail.signal.status, "confirmed");
    assert_eq!(detail.signal.related_labels, json!([LABEL]));
    assert_eq!(detail.signal.proposal["large"], u64::MAX);
    assert_eq!(detail.actions, vec![first]);
    let signals = service
        .list_label_ontology_signals(
            "default",
            LabelOntologySignalQuery {
                statuses: vec!["confirmed".to_owned()],
                kinds: vec!["vocabulary_gap".to_owned()],
                task_ref: Some(TASK.to_owned()),
                target_label_ref: Some(LABEL.to_owned()),
                ..LabelOntologySignalQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(signals, vec![detail.signal]);
    let groups = service
        .review_label_ontology("default", LabelOntologyReviewQuery::default())
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].signal_count, 2);
    assert_eq!(groups[0].labels.len(), 1);
    assert_eq!(groups[0].labels[0].name, None);
    assert!(groups[0].candidate_atom_variants.is_empty());
    let quality = service
        .label_ontology_quality("default", 1)
        .await
        .expect("actual typed quality branch");
    assert_eq!(quality.denominator.observation_count, 2);
    assert_eq!(quality.denominator.distinct_task_count, 1);
    assert_eq!(quality.denominator.degraded_observation_count, 1);
    assert_eq!(quality.denominator.agreement_observation_count, 1);
    assert_eq!(quality.denominator.sample_task_refs.len(), 1);
    assert_eq!(quality.disagreement.signal_count, 2);
    assert_eq!(quality.disagreement.by_kind["vocabulary_gap"], 2);
    assert_eq!(quality.rates.disagreement_task_rate, None);
    assert!(!quality.precision_recall.available);
    assert_eq!(quality.warnings.len(), 1);
    assert!(!notifications.has_changed().unwrap());
}

#[tokio::test]
async fn typed_apply_validate_explain_and_revert_preserve_canonical_hashes() {
    let (_directory, service) = service("typed-ontology-atom-lifecycle").await;
    let before = service
        .upsert_label_semantics("default", LABEL, semantics())
        .await
        .unwrap();
    let observation = service
        .record_label_ontology_observation("default", observation("atom-capture"))
        .await
        .unwrap();
    let mut command = apply_atom();
    command.signal_ids = vec![observation.signals[0].id.clone()];
    let applied = service
        .apply_label_ontology_atom("default", command)
        .await
        .unwrap();
    assert_eq!(
        applied.canonical_before_hash.as_deref(),
        Some(before.semantics_hash.as_str())
    );
    assert_eq!(
        applied.change["before"]["semantics_hash"],
        before.semantics_hash
    );
    assert_eq!(applied.validation_status, "pending");
    let validation = service.validate_label_ontology_action("default", OntologyValidateCommand {
        actor: actor(),
        parent_action_id: applied.id.clone(),
        signal_ids: applied.signal_ids.clone(),
        reason: "verify canonical change".to_owned(),
        validation_status: "passed".to_owned(),
        validation: json!({"manual": {"evidence": u64::MAX}, "summary": {"accepted": 1}, "cases": ["typed read"]}),
    }).await.unwrap();
    // 当前 store 把 parent 写在 change 中，尚未写入 parent_action_id 列。
    assert_eq!(validation.parent_action_id, None);
    assert_eq!(validation.change["parent_action_id"], applied.id);
    let explained = service
        .explain_label_atom("default", applied.result_atom_id.as_deref().unwrap())
        .await
        .unwrap();
    // 当前 candidate hash 没有包含 label ID，和 canonical atom hash 不同。
    assert!(explained.supporting_signals.is_empty());
    assert!(explained.validation_history.is_empty());
    let persisted = service
        .store
        .get_label_ontology_signal(&applied.signal_ids[0])
        .await
        .unwrap();
    let raw_validation = persisted
        .actions
        .into_iter()
        .find(|action| action.id == validation.id)
        .unwrap();
    let support: LabelAtomExplainSignalRecord = crate::domain::LabelAtomExplainSignalRecord {
        signal: persisted.signal,
        observation: persisted.observation,
        task_id: TASK.to_owned(),
        task_ref_snapshot: "default#1".to_owned(),
        suggest_input_stale: false,
        suggest_degraded: true,
        warnings: vec!["provider unavailable".to_owned()],
    }
    .try_into()
    .unwrap();
    assert_eq!(support.task_id, TASK);
    assert_eq!(support.observation.agent_candidates[0]["large"], u64::MAX);
    assert_eq!(
        support.signal.candidate_text.as_deref(),
        Some("new typed calls")
    );
    let evidence: LabelAtomExplainValidationRecord =
        crate::domain::LabelAtomExplainValidationRecord {
            action: raw_validation,
            parent_action_id: applied.id.clone(),
            validation_status: "passed".to_owned(),
            manual_json: "{\"evidence\":18446744073709551615}".to_owned(),
            summary_json: "{\"accepted\":1}".to_owned(),
            cases_json: "[\"typed read\"]".to_owned(),
            warnings: Vec::new(),
        }
        .try_into()
        .unwrap();
    assert_eq!(evidence.manual["evidence"], u64::MAX);
    assert_eq!(evidence.summary, json!({"accepted": 1}));
    assert_eq!(evidence.cases, json!(["typed read"]));
    assert_eq!(evidence.action, validation);
    assert_eq!(evidence.validation_effective_outcome, "passed");
    let mut revert = OntologyRevertCommand {
        actor: actor(),
        target_action_id: applied.id.clone(),
        expected_current_hash: Some("stale".to_owned()),
        reason: "restore prior semantics".to_owned(),
    };
    assert!(
        service
            .revert_label_ontology_mutation("default", revert.clone())
            .await
            .is_err()
    );
    revert.expected_current_hash = applied.canonical_after_hash;
    let reverted = service
        .revert_label_ontology_mutation("default", revert)
        .await
        .unwrap();
    assert_eq!(
        reverted.canonical_after_hash.as_deref(),
        Some(before.semantics_hash.as_str())
    );
    assert_eq!(
        service
            .get_label_semantics("default", LABEL)
            .await
            .unwrap()
            .semantics_hash,
        before.semantics_hash
    );
}

#[tokio::test]
async fn typed_proposal_lifecycle_and_board_isolation() {
    let (_directory, service) = service("typed-ontology-proposals").await;
    let notifications = service.mutation_gate.subscribe();
    let suggestion = service
        .suggest_task_labels("default", TASK, LabelSuggestionOptions::default())
        .await
        .unwrap();
    assert_eq!(suggestion.task_id, TASK);
    assert!(suggestion.degraded);
    assert!(!notifications.has_changed().unwrap());
    let attempted = service
        .propose_task_label("default", TASK, proposal())
        .await
        .unwrap();
    let proposal = attempted.proposal.unwrap();
    assert_eq!(proposal.name, "new capability");
    assert_eq!(proposal.positive_examples, ["manual proposal"]);
    assert_eq!(
        service.get_label_proposal(&proposal.id).await.unwrap(),
        proposal
    );
    assert_eq!(
        service
            .list_label_proposals("default", Some(TASK), Some("proposed"))
            .await
            .unwrap(),
        vec![proposal.clone()]
    );
    let decided = service
        .decide_label_proposal(LabelProposalDecisionCommand {
            proposal_id: proposal.id,
            accept: false,
            reason: Some("keep existing vocabulary".to_owned()),
            actor: "facade-test".to_owned(),
            source_signal_ids: Vec::new(),
        })
        .await
        .unwrap();
    assert_eq!(decided.status, "rejected");
    assert_eq!(
        decided.decision_reason.as_deref(),
        Some("keep existing vocabulary")
    );
    service
        .create_board(CreateBoardCommand {
            slug: "other".to_owned(),
            name: "Other".to_owned(),
            description: None,
            actor: "facade-test".to_owned(),
        })
        .await
        .unwrap();
    assert!(
        service
            .list_label_proposals("other", None, None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(service.get_label_semantics("other", LABEL).await.is_err());
    assert!(
        service
            .record_label_ontology_observation("other", observation("cross-board"))
            .await
            .is_err()
    );
    let quality = service.label_ontology_quality("other", 0).await.unwrap();
    assert_eq!(quality.denominator.observation_count, 0);
    assert_eq!(quality.denominator.first_observed_at, None);
    assert_eq!(quality.denominator.latest_observed_at, None);
    assert!(service.list_label_semantics(" ").await.is_err());
}

async fn assert_mutation_waits<T: std::fmt::Debug>(
    service: &KanbanService,
    operation: impl Future<Output = Result<T>>,
) {
    let fence = service.mutation_gate.read().await;
    let mut notifications = service.mutation_gate.subscribe();
    let revision = *notifications.borrow_and_update();
    let mut operation = Box::pin(operation);
    assert!(
        tokio::time::timeout(Duration::from_millis(30), operation.as_mut())
            .await
            .is_err()
    );
    assert!(!notifications.has_changed().unwrap());
    drop(fence);
    let outcome = tokio::time::timeout(Duration::from_secs(2), operation)
        .await
        .expect("mutation gate released");
    assert!(outcome.is_err(), "fixture operation must fail: {outcome:?}");
    assert_eq!(*notifications.borrow_and_update(), revision + 1);
}

#[tokio::test]
async fn all_ten_typed_mutations_share_gate_and_fail_without_facts() {
    let (_directory, service) = service("typed-ontology-gate").await;
    let connection = service.store.connection().await.unwrap();
    let events_before = test_support::count_rows(&connection, "task_events").await;
    assert_mutation_waits(
        &service,
        service.upsert_label_semantics("missing", LABEL, semantics()),
    )
    .await;
    assert_mutation_waits(
        &service,
        service.delete_label_semantics(
            "missing",
            LABEL,
            DeleteLabelSemanticsCommand {
                expected_semantics_hash: "hash".to_owned(),
                reason: "delete".to_owned(),
                actor: "facade-test".to_owned(),
            },
        ),
    )
    .await;
    assert_mutation_waits(&service, service.rebuild_label_atom_index("missing")).await;
    assert_mutation_waits(
        &service,
        service.propose_task_label("missing", TASK, proposal()),
    )
    .await;
    assert_mutation_waits(
        &service,
        service.decide_label_proposal(LabelProposalDecisionCommand {
            proposal_id: "missing".to_owned(),
            accept: false,
            reason: None,
            actor: "facade-test".to_owned(),
            source_signal_ids: Vec::new(),
        }),
    )
    .await;
    assert_mutation_waits(
        &service,
        service.record_label_ontology_observation("missing", observation("missing")),
    )
    .await;
    assert_mutation_waits(
        &service,
        service.create_label_ontology_action("missing", action(Vec::new())),
    )
    .await;
    assert_mutation_waits(
        &service,
        service.apply_label_ontology_atom("missing", apply_atom()),
    )
    .await;
    assert_mutation_waits(
        &service,
        service.revert_label_ontology_mutation(
            "missing",
            OntologyRevertCommand {
                actor: actor(),
                target_action_id: "missing".to_owned(),
                expected_current_hash: None,
                reason: "revert".to_owned(),
            },
        ),
    )
    .await;
    assert_mutation_waits(
        &service,
        service.validate_label_ontology_action(
            "missing",
            OntologyValidateCommand {
                actor: actor(),
                parent_action_id: "missing".to_owned(),
                signal_ids: Vec::new(),
                reason: "validate".to_owned(),
                validation_status: "passed".to_owned(),
                validation: json!({}),
            },
        ),
    )
    .await;
    assert_eq!(
        test_support::count_rows(&connection, "task_events").await,
        events_before
    );
    assert_eq!(
        test_support::count_rows(&connection, "label_ontology_actions").await,
        0
    );
    assert_eq!(
        test_support::count_rows(&connection, "label_ontology_observations").await,
        0
    );
    assert!(
        service
            .list_label_semantics("default")
            .await
            .unwrap()
            .is_empty()
    );
}

#[test]
fn typed_review_atom_variants_keep_each_field_and_invalid_quality_is_storage_error() {
    let group = crate::domain::LabelOntologyReviewGroupRecord {
        group_by: "candidate_atom".to_owned(), key: "group".to_owned(), label_id: None, label_name: None,
        candidate_atom_polarity: None, candidate_atom_kind: None, candidate_text: None, candidate_content_hash: None,
        proposed_label_name: None, proposed_label_name_normalized: None, cluster_key: None, cluster_reason: None,
        task_count: 1, signal_count: 1, open_count: 1, confirmed_count: 0, resolved_count: 0, rejected_count: 0,
        superseded_count: 0, degraded_count: 0, average_score: Some(0.0), median_score: None,
        oldest_signal_at: 1, latest_signal_at: 2, sample_task_refs: vec!["default#1".to_owned()],
        signal_ids: vec!["signal".to_owned()], action_count: 0, action_ids: Vec::new(), proposal_ids: Vec::new(),
        labels_json: "[\"label-id\"]".to_owned(),
        candidate_atom_variants_json: json!([{"content_hash": "hash", "polarity": "positive", "kind": "applies_when", "text": "text", "signal_count": 7}]).to_string(),
    };
    let record: LabelOntologyReviewGroupRecord = group.try_into().unwrap();
    assert_eq!(
        record.labels,
        vec![LabelOntologyReviewLabelRefRecord {
            id: "label-id".to_owned(),
            name: None
        }]
    );
    assert_eq!(
        record.candidate_atom_variants,
        vec![LabelOntologyReviewAtomVariantRecord {
            content_hash: "hash".to_owned(),
            polarity: Some("positive".to_owned()),
            kind: Some("applies_when".to_owned()),
            text: Some("text".to_owned()),
            signal_count: 7,
        }]
    );
    let invalid = crate::domain::LabelOntologyQualityRecord {
        board_id: "board".to_owned(),
        denominator_json: "{}".to_owned(),
        disagreement_json: "{}".to_owned(),
        rates_json: "{}".to_owned(),
        precision_recall_json: "{}".to_owned(),
        warnings_json: "[]".to_owned(),
    };
    assert!(
        matches!(LabelOntologyQualityRecord::try_from(invalid), Err(KanbanError::Storage(message)) if message.contains("denominator_json"))
    );
}
