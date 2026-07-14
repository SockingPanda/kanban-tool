use crate::common::*;
use kanban_contract as contract;
use serde::Serialize;
use std::os::unix::fs::PermissionsExt;
use syn::visit::Visit;

const REQUEST_CONTRACT_IDS: [&str; 48] = [
    "api.list-board-labels.path",
    "api.create-board-label.path",
    "api.create-board-label.request",
    "api.list-label-semantics.path",
    "api.get-label-semantics.path",
    "api.upsert-label-semantics.path",
    "api.upsert-label-semantics.request",
    "api.delete-label-semantics.path",
    "api.delete-label-semantics.query",
    "api.list-label-atoms.path",
    "api.label-atom.path",
    "api.label-atom-index-status.path",
    "api.rebuild-label-atom-index.path",
    "api.query-label-atom-index.path",
    "api.query-label-atom-index.query",
    "api.list-signals.path",
    "api.list-signals.query",
    "api.review-signals.path",
    "api.review-signals.query",
    "api.get-signal.path",
    "api.bootstrap-task-label.path",
    "api.bootstrap-task-label.request",
    "api.suggest-task-labels.path",
    "api.label-suggestion.query",
    "api.list-task-label-proposals.path",
    "api.propose-task-label.path",
    "api.propose-task-label.query",
    "api.propose-task-label.request",
    "api.record-label-ontology-observation.path",
    "api.record-label-ontology-observation.body",
    "api.list-label-ontology-signals.path",
    "api.label-ontology-signal.query",
    "api.review-label-ontology.path",
    "api.label-ontology-review.query",
    "api.create-label-ontology-action.path",
    "api.create-label-ontology-action.request",
    "api.apply-label-ontology-atom.path",
    "api.apply-label-ontology-atom.request",
    "api.revert-label-ontology-mutation.path",
    "api.revert-label-ontology-mutation.request",
    "api.validate-label-ontology-action.path",
    "api.validate-label-ontology-action.request",
    "api.get-label-ontology-signal.path",
    "api.get-label-proposal.path",
    "api.accept-label-proposal.path",
    "api.accept-label-proposal.body",
    "api.reject-label-proposal.path",
    "api.reject-label-proposal.body",
];

const REQUEST_CONSUMER_CASES: [(&str, &str); 48] = [
    (
        "api.list-board-labels.path",
        "list_board_labels_request_fixture_reaches_handler",
    ),
    (
        "api.create-board-label.path",
        "create_board_label_request_fixtures_reach_handler",
    ),
    (
        "api.create-board-label.request",
        "create_board_label_request_fixtures_reach_handler",
    ),
    (
        "api.list-label-semantics.path",
        "list_label_semantics_request_fixture_reaches_handler",
    ),
    (
        "api.get-label-semantics.path",
        "get_label_semantics_request_fixture_reaches_handler",
    ),
    (
        "api.upsert-label-semantics.path",
        "upsert_label_semantics_request_fixtures_reach_handler",
    ),
    (
        "api.upsert-label-semantics.request",
        "upsert_label_semantics_request_fixtures_reach_handler",
    ),
    (
        "api.delete-label-semantics.path",
        "delete_label_semantics_request_fixtures_reach_handler",
    ),
    (
        "api.delete-label-semantics.query",
        "delete_label_semantics_request_fixtures_reach_handler",
    ),
    (
        "api.list-label-atoms.path",
        "list_label_atoms_request_fixture_reaches_handler",
    ),
    (
        "api.label-atom.path",
        "explain_label_atom_request_fixture_reaches_handler",
    ),
    (
        "api.label-atom-index-status.path",
        "label_atom_index_status_request_fixture_reaches_handler",
    ),
    (
        "api.rebuild-label-atom-index.path",
        "rebuild_label_atom_index_request_fixture_reaches_handler",
    ),
    (
        "api.query-label-atom-index.path",
        "query_label_atom_index_request_fixtures_reach_handler",
    ),
    (
        "api.query-label-atom-index.query",
        "query_label_atom_index_request_fixtures_reach_handler",
    ),
    (
        "api.list-signals.path",
        "list_signals_request_fixtures_reach_handler",
    ),
    (
        "api.list-signals.query",
        "list_signals_request_fixtures_reach_handler",
    ),
    (
        "api.review-signals.path",
        "review_signals_request_fixtures_reach_handler",
    ),
    (
        "api.review-signals.query",
        "review_signals_request_fixtures_reach_handler",
    ),
    (
        "api.get-signal.path",
        "get_signal_request_fixture_reaches_handler",
    ),
    (
        "api.bootstrap-task-label.path",
        "bootstrap_task_label_request_fixtures_reach_handler",
    ),
    (
        "api.bootstrap-task-label.request",
        "bootstrap_task_label_request_fixtures_reach_handler",
    ),
    (
        "api.suggest-task-labels.path",
        "suggest_task_labels_request_fixtures_reach_handler",
    ),
    (
        "api.label-suggestion.query",
        "suggest_task_labels_request_fixtures_reach_handler",
    ),
    (
        "api.list-task-label-proposals.path",
        "list_task_label_proposals_request_fixture_reaches_handler",
    ),
    (
        "api.propose-task-label.path",
        "propose_task_label_request_fixtures_reach_handler",
    ),
    (
        "api.propose-task-label.query",
        "propose_task_label_request_fixtures_reach_handler",
    ),
    (
        "api.propose-task-label.request",
        "propose_task_label_request_fixtures_reach_handler",
    ),
    (
        "api.record-label-ontology-observation.path",
        "record_label_ontology_observation_request_fixtures_reach_handler",
    ),
    (
        "api.record-label-ontology-observation.body",
        "record_label_ontology_observation_request_fixtures_reach_handler",
    ),
    (
        "api.list-label-ontology-signals.path",
        "list_label_ontology_signals_request_fixtures_reach_handler",
    ),
    (
        "api.label-ontology-signal.query",
        "list_label_ontology_signals_request_fixtures_reach_handler",
    ),
    (
        "api.review-label-ontology.path",
        "review_label_ontology_request_fixtures_reach_handler",
    ),
    (
        "api.label-ontology-review.query",
        "review_label_ontology_request_fixtures_reach_handler",
    ),
    (
        "api.create-label-ontology-action.path",
        "create_label_ontology_action_request_fixtures_reach_handler",
    ),
    (
        "api.create-label-ontology-action.request",
        "create_label_ontology_action_request_fixtures_reach_handler",
    ),
    (
        "api.apply-label-ontology-atom.path",
        "apply_label_ontology_atom_request_fixtures_reach_handler",
    ),
    (
        "api.apply-label-ontology-atom.request",
        "apply_label_ontology_atom_request_fixtures_reach_handler",
    ),
    (
        "api.revert-label-ontology-mutation.path",
        "revert_label_ontology_mutation_request_fixtures_reach_handler",
    ),
    (
        "api.revert-label-ontology-mutation.request",
        "revert_label_ontology_mutation_request_fixtures_reach_handler",
    ),
    (
        "api.validate-label-ontology-action.path",
        "validate_label_ontology_action_request_fixtures_reach_handler",
    ),
    (
        "api.validate-label-ontology-action.request",
        "validate_label_ontology_action_request_fixtures_reach_handler",
    ),
    (
        "api.get-label-ontology-signal.path",
        "get_label_ontology_signal_request_fixture_reaches_handler",
    ),
    (
        "api.get-label-proposal.path",
        "get_label_proposal_request_fixture_reaches_handler",
    ),
    (
        "api.accept-label-proposal.path",
        "accept_label_proposal_request_fixtures_reach_handler",
    ),
    (
        "api.accept-label-proposal.body",
        "accept_label_proposal_request_fixtures_reach_handler",
    ),
    (
        "api.reject-label-proposal.path",
        "reject_label_proposal_request_fixtures_reach_handler",
    ),
    (
        "api.reject-label-proposal.body",
        "reject_label_proposal_request_fixtures_reach_handler",
    ),
];

const RESPONSE_PRODUCER_CASES: [(&str, &str); 27] = [
    (
        "api.list-board-labels.response",
        "generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.create-board-label.response",
        "generated_label_responses_are_produced_by_real_router",
    ),
    (
        "api.list-label-semantics.response",
        "generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.get-label-semantics.response",
        "generated_label_responses_are_produced_by_real_router",
    ),
    (
        "api.upsert-label-semantics.response",
        "generated_label_responses_are_produced_by_real_router",
    ),
    (
        "api.list-label-atoms.response",
        "generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.explain-label-atom.response",
        "generated_label_responses_are_produced_by_real_router",
    ),
    (
        "api.label-atom-index-status.response",
        "generated_atom_index_responses_are_produced_by_real_router",
    ),
    (
        "api.rebuild-label-atom-index.response",
        "generated_atom_index_responses_are_produced_by_real_router",
    ),
    (
        "api.query-label-atom-index.response",
        "generated_atom_index_responses_are_produced_by_real_router",
    ),
    (
        "api.list-signals.response",
        "generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.review-signals.response",
        "generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.get-signal.response",
        "generated_generic_signal_response_is_produced_by_real_router",
    ),
    (
        "api.bootstrap-task-label.response",
        "generated_task_label_responses_are_produced_by_real_router",
    ),
    (
        "api.suggest-task-labels.response",
        "generated_task_label_responses_are_produced_by_real_router",
    ),
    (
        "api.list-task-label-proposals.response",
        "generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.propose-task-label.response",
        "generated_task_label_responses_are_produced_by_real_router",
    ),
    (
        "api.record-label-ontology-observation.response",
        "generated_ontology_observation_responses_are_produced_by_real_router",
    ),
    (
        "api.review-label-ontology.response",
        "generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.create-label-ontology-action.response",
        "generated_ontology_action_responses_are_produced_by_real_router",
    ),
    (
        "api.apply-label-ontology-atom.response",
        "generated_ontology_action_responses_are_produced_by_real_router",
    ),
    (
        "api.revert-label-ontology-mutation.response",
        "generated_ontology_action_responses_are_produced_by_real_router",
    ),
    (
        "api.validate-label-ontology-action.response",
        "generated_ontology_action_responses_are_produced_by_real_router",
    ),
    (
        "api.get-label-ontology-signal.response",
        "generated_ontology_observation_responses_are_produced_by_real_router",
    ),
    (
        "api.get-label-proposal.response",
        "generated_proposal_responses_are_produced_by_real_router",
    ),
    (
        "api.accept-label-proposal.response",
        "generated_proposal_responses_are_produced_by_real_router",
    ),
    (
        "api.reject-label-proposal.response",
        "generated_proposal_responses_are_produced_by_real_router",
    ),
];

fn fixture(name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/fixtures/api")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

fn assert_dto_fixture<T: Serialize>(value: T, name: &str) {
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        fixture(name),
        "{name}"
    );
}

fn actor() -> contract::LabelOntologyActorWire {
    contract::LabelOntologyActorWire {
        name: "fixture".into(),
        actor_type: "user".into(),
        agent_type: None,
    }
}

macro_rules! path_case {
    ($($ty:ident)::+, $fixture:literal, {$($field:ident: $value:expr),+ $(,)?}) => {
        assert_dto_fixture($($ty)::+ { $($field: $value.into()),+ }, $fixture);
    };
}

#[test]
fn api_generated_request_dtos_serialize_to_committed_fixtures() {
    path_case!(contract::BoardLabelPath, "list-board-labels-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "create-board-label-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "list-label-semantics-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::LabelSemanticsPath, "get-label-semantics-path.v1.valid.json", { board: "fixture", label_id: "l_fixture" });
    path_case!(contract::LabelSemanticsPath, "upsert-label-semantics-path.v1.valid.json", { board: "fixture", label_id: "l_fixture" });
    path_case!(contract::LabelSemanticsPath, "delete-label-semantics-path.v1.valid.json", { board: "fixture", label_id: "l_fixture" });
    path_case!(contract::BoardLabelPath, "list-label-atoms-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::LabelAtomPath, "label-atom-path.v1.valid.json", { board: "fixture", atom_ref: "la_51d68accaaa974f4" });
    path_case!(contract::BoardLabelPath, "label-atom-index-status-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "rebuild-label-atom-index-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "query-label-atom-index-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "list-signals-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "review-signals-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::SignalPath, "get-signal-path.v1.valid.json", { signal_id: "sig_fixture" });
    path_case!(contract::TaskLabelSurfacePath, "bootstrap-task-label-path.v1.valid.json", { task_id: "t_fixture" });
    path_case!(contract::TaskLabelSurfacePath, "suggest-task-labels-path.v1.valid.json", { task_id: "t_fixture" });
    path_case!(contract::TaskLabelSurfacePath, "list-task-label-proposals-path.v1.valid.json", { task_id: "t_fixture" });
    path_case!(contract::TaskLabelSurfacePath, "propose-task-label-path.v1.valid.json", { task_id: "t_fixture" });
    path_case!(contract::TaskLabelSurfacePath, "record-label-ontology-observation-path.v1.valid.json", { task_id: "t_fixture" });
    path_case!(contract::BoardLabelPath, "list-label-ontology-signals-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "review-label-ontology-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "create-label-ontology-action-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "apply-label-ontology-atom-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "revert-label-ontology-mutation-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::BoardLabelPath, "validate-label-ontology-action-path.v1.valid.json", { board: "fixture" });
    path_case!(contract::SignalPath, "get-label-ontology-signal-path.v1.valid.json", { signal_id: "los_fixture" });
    path_case!(contract::ProposalPath, "get-label-proposal-path.v1.valid.json", { proposal_id: "lp_fixture" });
    path_case!(contract::ProposalPath, "accept-label-proposal-path.v1.valid.json", { proposal_id: "lp_fixture" });
    path_case!(contract::ProposalPath, "reject-label-proposal-path.v1.valid.json", { proposal_id: "lp_fixture" });

    assert_dto_fixture(
        contract::CreateBoardLabelRequest {
            name: "fixture".into(),
            color: None,
        },
        "create-board-label-request.v1.valid.json",
    );
    assert_dto_fixture(
        contract::UpsertLabelSemanticsRequest {
            actor: None,
            expected_semantics_hash: None,
            replace: false,
            reason: None,
            source_signal_ids: vec![],
            description: None,
            applies_when: None,
            excludes_when: None,
            positive_examples: None,
            negative_examples: None,
            remove_applies_when: vec![],
            remove_excludes_when: vec![],
            remove_positive_examples: vec![],
            remove_negative_examples: vec![],
        },
        "upsert-label-semantics-request.v1.valid.json",
    );
    assert_dto_fixture(
        contract::DeleteLabelSemanticsQuery {
            expected_semantics_hash: "fixture".into(),
            reason: "fixture".into(),
        },
        "delete-label-semantics-query.v1.valid.json",
    );
    assert_dto_fixture(
        contract::LabelAtomIndexQuery {
            q: Some("fixture".into()),
            vector_json: None,
            embedding_model: None,
            include_vector: false,
            polarity: None,
            limit: 24,
        },
        "query-label-atom-index-query.v1.valid.json",
    );
    assert_dto_fixture(
        contract::SignalQuery {
            status: vec![],
            kind: vec![],
            task_ref: None,
            include_all: false,
            limit: 100,
        },
        "list-signals-query.v1.valid.json",
    );
    assert_dto_fixture(
        contract::SignalQuery {
            status: vec![],
            kind: vec![],
            task_ref: None,
            include_all: false,
            limit: 100,
        },
        "review-signals-query.v1.valid.json",
    );
    assert_dto_fixture(
        contract::BootstrapTaskLabelRequest {
            name: "fixture".into(),
            description: Some("fixture".into()),
            applies_when: vec![],
            excludes_when: vec![],
            positive_examples: vec![],
            negative_examples: vec![],
            actor: None,
        },
        "bootstrap-task-label-request.v1.valid.json",
    );
    assert_dto_fixture(
        contract::LabelSuggestionQuery {
            limit: 5,
            candidate_limit: 32,
            atom_limit: 80,
            max_selected_labels: 4,
            min_score: 0.15,
        },
        "label-suggestion-query.v1.valid.json",
    );
    assert_dto_fixture(
        contract::LabelSuggestionQuery {
            limit: 5,
            candidate_limit: 32,
            atom_limit: 80,
            max_selected_labels: 4,
            min_score: 0.15,
        },
        "propose-task-label-query.v1.valid.json",
    );
    assert_dto_fixture(
        contract::ProposeTaskLabelRequest {
            proposal: None,
            actor: None,
            source_signal_ids: vec![],
            ontology_actor: None,
            allow_retarget: false,
            retarget_reason: None,
        },
        "propose-task-label-request.v1.valid.json",
    );
    assert_dto_fixture(
        contract::RecordLabelOntologyObservationRequest {
            actor: actor(),
            agent_candidates: contract::JsonBodyFieldWire::Missing,
            suggestion_snapshot: contract::JsonBodyFieldWire::Missing,
            final_decision: contract::JsonBodyFieldWire::Missing,
            suggest_coverage: None,
            suggest_coverage_cosine: None,
            suggest_residual_norm: None,
            suggest_needs_new_label: None,
            suggest_degraded: None,
            diagnostics: contract::JsonBodyFieldWire::Missing,
            capture_fingerprint: None,
            signals: vec![contract::LabelOntologySignalRequest {
                kind: contract::LabelOntologySignalKindWire::FalseNegative,
                target_label_ref: Some("fixture".into()),
                related_labels: contract::JsonBodyFieldWire::Missing,
                proposed_action: contract::LabelOntologyProposedActionWire::AddPositiveAtom,
                candidate_atom: Some(contract::LabelOntologyCandidateAtomRequest {
                    polarity: "positive".into(),
                    kind: "applies_when".into(),
                    text: "fixture".into(),
                }),
                proposed_label_name: None,
                proposal: contract::JsonBodyFieldWire::Missing,
                agent_selected: false,
                suggest_state: None,
                suggest_score: None,
                suggest_rank: None,
                final_selected: false,
                rationale: "fixture".into(),
                confidence: None,
                signal_key: Some("fixture".into()),
            }],
        },
        "record-label-ontology-observation-body.v1.valid.json",
    );
    assert_dto_fixture(
        contract::LabelOntologySignalQuery {
            status: vec![],
            kind: vec![],
            task_ref: None,
            target_label_ref: None,
            proposed_label_name: None,
            include_all: false,
            limit: 100,
        },
        "label-ontology-signal-query.v1.valid.json",
    );
    assert_dto_fixture(
        contract::LabelOntologyReviewQuery {
            group_by: contract::LabelOntologyReviewGroupByWire::Label,
            include_all: false,
            limit: 100,
        },
        "label-ontology-review-query.v1.valid.json",
    );
    assert_dto_fixture(
        contract::LabelOntologyActionRequest {
            actor: actor(),
            action_type: contract::LabelOntologyActionTypeWire::Confirm,
            signal_ids: vec!["los_fixture".into()],
            reason: "fixture".into(),
            superseded_by_signal_id: None,
            parent_action_id: None,
            target_label_ref: None,
            result_label_ref: None,
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: None,
            canonical_before_hash: None,
            canonical_after_hash: None,
            change: contract::JsonBodyFieldWire::Missing,
            validation_status: None,
            validation: contract::JsonBodyFieldWire::Missing,
        },
        "create-label-ontology-action-request.v1.valid.json",
    );
    assert_dto_fixture(
        contract::ApplyLabelOntologyAtomRequest {
            actor: actor(),
            signal_ids: vec!["los_fixture".into()],
            label_ref: "fixture".into(),
            kind: "applies_when".into(),
            text: "fixture".into(),
            reason: "fixture".into(),
            allow_retarget: false,
            retarget_reason: None,
        },
        "apply-label-ontology-atom-request.v1.valid.json",
    );
    assert_dto_fixture(
        contract::RevertLabelOntologyMutationRequest {
            actor: actor(),
            target_action_id: "loa_target_fixture".into(),
            expected_current_hash: None,
            reason: "fixture".into(),
        },
        "revert-label-ontology-mutation-request.v1.valid.json",
    );
    assert_dto_fixture(
        contract::ValidateLabelOntologyActionRequest {
            actor: actor(),
            parent_action_id: "loa_target_fixture".into(),
            signal_ids: vec![],
            reason: "fixture".into(),
            validation_status: contract::LabelOntologyValidationStatusWire::Failed,
            validation: contract::JsonBodyFieldWire::Present(json!({"cases": []})),
        },
        "validate-label-ontology-action-request.v1.valid.json",
    );
    let decision = || contract::LabelProposalDecisionRequest {
        reason: None,
        actor: None,
        source_signal_ids: vec![],
        ontology_actor: None,
        allow_retarget: false,
        retarget_reason: None,
    };
    assert_dto_fixture(decision(), "accept-label-proposal-body.v1.valid.json");
    assert_dto_fixture(decision(), "reject-label-proposal-body.v1.valid.json");
}

#[test]
fn api_generated_request_consumer_locators_are_complete() {
    let expected = REQUEST_CONTRACT_IDS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let registered = REQUEST_CONSUMER_CASES
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    let covered = registered
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(covered, expected);
    assert_eq!(
        registered.len(),
        covered.len(),
        "duplicate consumer locator"
    );

    verify_request_consumer_locator_functions(
        include_str!("api_generated_adoption.rs"),
        &REQUEST_CONSUMER_CASES,
    )
    .unwrap();
    assert_runtime_proof_bypass_methods_are_private(include_str!("api_generated_adoption.rs"));

    let hostile_finish = r#"
        #[tokio::test]
        async fn mapped_consumer() {
            let proof = make_proof();
            let _ = proof.finish();
        }
    "#;
    assert!(
        verify_request_consumer_locator_functions(
            hostile_finish,
            &[("api.hostile", "mapped_consumer")],
        )
        .unwrap_err()
        .contains("must not bypass Drop proof with finish()"),
        "mapped locators must reject an explicit finish bypass"
    );
}

#[test]
fn api_generated_request_inventory_locators_match_runtime_proofs() {
    let expected = REQUEST_CONSUMER_CASES
        .iter()
        .map(|(contract_id, locator)| {
            (
                *contract_id,
                format!("suite::api_generated_adoption::{locator}"),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let actual = contract::operation_inventory()
        .iter()
        .filter(|root| REQUEST_CONTRACT_IDS.contains(&root.id))
        .map(|root| {
            let consumer = root
                .adoption
                .as_ref()
                .unwrap_or_else(|| panic!("{} is missing adoption evidence", root.id))
                .consumer
                .exact_test;
            (root.id, consumer.to_owned())
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(actual, expected);
}

#[tokio::test]
async fn runtime_request_consumer_proof_rejects_dead_and_remapped_execution() -> anyhow::Result<()>
{
    runtime_request_consumer_proof::assert_negative_cases().await
}

#[derive(Clone, Copy)]
struct RouterCase {
    method: &'static str,
    uri: &'static str,
    body_fixture: Option<&'static str>,
    contracts: &'static [&'static str],
}

fn request_from_contract_fixtures(
    case: &RouterCase,
) -> anyhow::Result<(&'static str, String, Option<Value>)> {
    let inventory = contract::operation_inventory();
    let roots = case
        .contracts
        .iter()
        .map(|id| {
            inventory
                .iter()
                .find(|root| root.id == *id)
                .unwrap_or_else(|| panic!("missing registered contract {id}"))
        })
        .collect::<Vec<_>>();
    let operation = roots[0].operation;
    assert!(
        roots.iter().all(|root| root.operation == operation),
        "router case mixes operations: {:?}",
        case.contracts
    );
    let (method, path_template) = operation
        .split_once(' ')
        .unwrap_or_else(|| panic!("invalid operation {operation}"));
    let mut uri = path_template.to_owned();
    let mut body = None;

    for root in roots {
        let contract::ContractTransport::Http { location, .. } = root.transport else {
            panic!("{} is not an HTTP contract", root.id);
        };
        let fixture_name = std::path::Path::new(
            root.fixture
                .unwrap_or_else(|| panic!("{} has no fixture", root.id)),
        )
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap();
        let value = fixture(fixture_name);
        match location {
            contract::HttpTransportLocation::Path => {
                for (name, value) in value.as_object().unwrap() {
                    let value = value
                        .as_str()
                        .unwrap_or_else(|| panic!("{} path field {name} is not a string", root.id));
                    uri = uri.replace(&format!(":{name}"), value);
                }
            }
            contract::HttpTransportLocation::Query => {
                let query = serde_urlencoded::to_string(&value)?;
                if !query.is_empty() {
                    uri.push('?');
                    uri.push_str(&query);
                }
            }
            contract::HttpTransportLocation::Body => body = Some(value),
            other => panic!("{} has request-side location {other:?}", root.id),
        }
    }
    assert!(!uri.contains(':'), "unresolved operation path: {uri}");
    assert_eq!(
        method, case.method,
        "method registration drift for {operation}"
    );
    assert_eq!(
        uri.split('?').next(),
        case.uri.split('?').next(),
        "path registration drift for {operation}"
    );
    assert_eq!(
        body.is_some(),
        case.body_fixture.is_some(),
        "body registration drift for {operation}"
    );
    Ok((method, uri, body))
}

fn request_router_cases() -> [RouterCase; 29] {
    [
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/labels",
            body_fixture: None,
            contracts: &["api.list-board-labels.path"],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/boards/fixture/labels",
            body_fixture: Some("create-board-label-request.v1.valid.json"),
            contracts: &[
                "api.create-board-label.path",
                "api.create-board-label.request",
            ],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/labels/semantics",
            body_fixture: None,
            contracts: &["api.list-label-semantics.path"],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/labels/l_fixture/semantics",
            body_fixture: None,
            contracts: &["api.get-label-semantics.path"],
        },
        RouterCase {
            method: "PUT",
            uri: "/api/v1/boards/fixture/labels/l_fixture/semantics",
            body_fixture: Some("upsert-label-semantics-request.v1.valid.json"),
            contracts: &[
                "api.upsert-label-semantics.path",
                "api.upsert-label-semantics.request",
            ],
        },
        RouterCase {
            method: "DELETE",
            uri: "/api/v1/boards/fixture/labels/l_fixture/semantics?expected_semantics_hash=fixture&reason=fixture",
            body_fixture: None,
            contracts: &[
                "api.delete-label-semantics.path",
                "api.delete-label-semantics.query",
            ],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/labels/atoms",
            body_fixture: None,
            contracts: &["api.list-label-atoms.path"],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/labels/atoms/la_51d68accaaa974f4/explain",
            body_fixture: None,
            contracts: &["api.label-atom.path"],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/labels/atom-index/status",
            body_fixture: None,
            contracts: &["api.label-atom-index-status.path"],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/boards/fixture/labels/atom-index/rebuild",
            body_fixture: None,
            contracts: &["api.rebuild-label-atom-index.path"],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/labels/atom-index/query",
            body_fixture: None,
            contracts: &[
                "api.query-label-atom-index.path",
                "api.query-label-atom-index.query",
            ],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/signals",
            body_fixture: None,
            contracts: &["api.list-signals.path", "api.list-signals.query"],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/signals/review",
            body_fixture: None,
            contracts: &["api.review-signals.path", "api.review-signals.query"],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/signals/sig_fixture",
            body_fixture: None,
            contracts: &["api.get-signal.path"],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/tasks/t_fixture/labels/bootstrap",
            body_fixture: Some("bootstrap-task-label-request.v1.valid.json"),
            contracts: &[
                "api.bootstrap-task-label.path",
                "api.bootstrap-task-label.request",
            ],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/tasks/t_fixture/labels/suggestions",
            body_fixture: None,
            contracts: &["api.suggest-task-labels.path", "api.label-suggestion.query"],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/tasks/t_fixture/label-proposals",
            body_fixture: None,
            contracts: &["api.list-task-label-proposals.path"],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/tasks/t_fixture/label-proposals",
            body_fixture: Some("propose-task-label-request.v1.valid.json"),
            contracts: &[
                "api.propose-task-label.path",
                "api.propose-task-label.query",
                "api.propose-task-label.request",
            ],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/tasks/t_fixture/label-ontology/observations",
            body_fixture: Some("record-label-ontology-observation-body.v1.valid.json"),
            contracts: &[
                "api.record-label-ontology-observation.path",
                "api.record-label-ontology-observation.body",
            ],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/label-ontology/signals",
            body_fixture: None,
            contracts: &[
                "api.list-label-ontology-signals.path",
                "api.label-ontology-signal.query",
            ],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/boards/fixture/label-ontology/review",
            body_fixture: None,
            contracts: &[
                "api.review-label-ontology.path",
                "api.label-ontology-review.query",
            ],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/boards/fixture/label-ontology/actions",
            body_fixture: Some("create-label-ontology-action-request.v1.valid.json"),
            contracts: &[
                "api.create-label-ontology-action.path",
                "api.create-label-ontology-action.request",
            ],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/boards/fixture/label-ontology/apply/atom",
            body_fixture: Some("apply-label-ontology-atom-request.v1.valid.json"),
            contracts: &[
                "api.apply-label-ontology-atom.path",
                "api.apply-label-ontology-atom.request",
            ],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/boards/fixture/label-ontology/revert",
            body_fixture: Some("revert-label-ontology-mutation-request.v1.valid.json"),
            contracts: &[
                "api.revert-label-ontology-mutation.path",
                "api.revert-label-ontology-mutation.request",
            ],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/boards/fixture/label-ontology/validate",
            body_fixture: Some("validate-label-ontology-action-request.v1.valid.json"),
            contracts: &[
                "api.validate-label-ontology-action.path",
                "api.validate-label-ontology-action.request",
            ],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/label-ontology/signals/los_fixture",
            body_fixture: None,
            contracts: &["api.get-label-ontology-signal.path"],
        },
        RouterCase {
            method: "GET",
            uri: "/api/v1/label-proposals/lp_fixture",
            body_fixture: None,
            contracts: &["api.get-label-proposal.path"],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/label-proposals/lp_fixture/accept",
            body_fixture: Some("accept-label-proposal-body.v1.valid.json"),
            contracts: &[
                "api.accept-label-proposal.path",
                "api.accept-label-proposal.body",
            ],
        },
        RouterCase {
            method: "POST",
            uri: "/api/v1/label-proposals/lp_fixture/reject",
            body_fixture: Some("reject-label-proposal-body.v1.valid.json"),
            contracts: &[
                "api.reject-label-proposal.path",
                "api.reject-label-proposal.body",
            ],
        },
    ]
}

fn request_router_case(contract_id: &str) -> RouterCase {
    request_router_cases()
        .into_iter()
        .find(|case| case.contracts.contains(&contract_id))
        .unwrap_or_else(|| panic!("missing router case for {contract_id}"))
}

fn verify_request_consumer_locator_functions(
    source: &str,
    mappings: &[(&str, &str)],
) -> Result<(), String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let mut tests = std::collections::BTreeMap::<String, &syn::ItemFn>::new();
    for item in &file.items {
        let syn::Item::Fn(item_fn) = item else {
            continue;
        };
        let is_tokio_test = item_fn.attrs.iter().any(|attribute| {
            let segments = attribute
                .path()
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            segments == ["tokio", "test"]
        });
        if !is_tokio_test {
            continue;
        }
        if tests
            .insert(item_fn.sig.ident.to_string(), item_fn)
            .is_some()
        {
            return Err(format!("duplicate tokio test {}", item_fn.sig.ident));
        }
    }

    for &(_, locator) in mappings {
        let Some(test) = tests.get(locator) else {
            return Err(format!(
                "missing exact #[tokio::test] consumer locator {locator}"
            ));
        };
        struct FinishCallVisitor(bool);
        impl<'ast> Visit<'ast> for FinishCallVisitor {
            fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
                if call.method == "finish" {
                    self.0 = true;
                }
                syn::visit::visit_expr_method_call(self, call);
            }
        }
        let mut finish = FinishCallVisitor(false);
        finish.visit_block(&test.block);
        if finish.0 {
            return Err(format!(
                "mapped consumer locator {locator} must not bypass Drop proof with finish()"
            ));
        }
    }
    Ok(())
}

fn assert_runtime_proof_bypass_methods_are_private(source: &str) {
    let file = syn::parse_file(source).unwrap();
    let module = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Mod(module) if module.ident == "runtime_request_consumer_proof" => {
                Some(module)
            }
            _ => None,
        })
        .expect("runtime proof module");
    let (_, items) = module
        .content
        .as_ref()
        .expect("inline runtime proof module");
    let methods = items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item_impl) => Some(item_impl),
            _ => None,
        })
        .flat_map(|item_impl| &item_impl.items)
        .filter_map(|item| match item {
            syn::ImplItem::Fn(method) if method.sig.ident == "record_case" => Some(method),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(methods.len(), 1, "record_case must have one implementation");
    assert!(
        matches!(methods[0].vis, syn::Visibility::Inherited),
        "record_case must remain private to the runtime proof module"
    );
    let finish_methods = items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item_impl) => Some(item_impl),
            _ => None,
        })
        .flat_map(|item_impl| &item_impl.items)
        .filter_map(|item| match item {
            syn::ImplItem::Fn(method) if method.sig.ident == "finish" => Some(method),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        finish_methods.len(),
        1,
        "finish must have one implementation"
    );
    assert!(
        matches!(finish_methods[0].vis, syn::Visibility::Inherited),
        "finish must remain private to the runtime proof module"
    );
}

mod runtime_request_consumer_proof {
    use super::*;

    pub(super) struct RuntimeRequestConsumerProof {
        locator: &'static str,
        expected: std::collections::BTreeSet<&'static str>,
        observed: std::cell::RefCell<std::collections::BTreeSet<&'static str>>,
        armed: std::cell::Cell<bool>,
    }

    impl RuntimeRequestConsumerProof {
        pub(super) fn new(locator: &'static str) -> Self {
            let expected = REQUEST_CONSUMER_CASES
                .iter()
                .filter_map(|(contract_id, mapped)| (*mapped == locator).then_some(*contract_id))
                .collect::<std::collections::BTreeSet<_>>();
            assert!(
                !expected.is_empty(),
                "unregistered consumer locator {locator}"
            );
            Self {
                locator,
                expected,
                observed: std::cell::RefCell::default(),
                armed: std::cell::Cell::new(true),
            }
        }

        fn record_case(&self, case: RouterCase) {
            self.observed.borrow_mut().extend(case.contracts);
        }

        pub(super) async fn request_fixture_case(
            &self,
            app: axum::Router,
            contract_id: &str,
        ) -> anyhow::Result<(StatusCode, Value)> {
            let case = request_router_case(contract_id);
            match execute_request_fixture_case(app, contract_id).await {
                Ok(response) => {
                    self.record_case(case);
                    Ok(response)
                }
                Err(error) => {
                    self.armed.set(false);
                    Err(error)
                }
            }
        }

        fn completion_error(&self) -> Option<String> {
            let observed = self.observed.borrow();
            (*observed != self.expected).then(|| {
                format!(
                    "runtime consumer {} observed {:?}, expected {:?}",
                    self.locator, observed, self.expected
                )
            })
        }

        fn finish(self) -> Result<(), String> {
            let result = self.completion_error().map_or(Ok(()), Err);
            self.armed.set(false);
            result
        }
    }

    impl Drop for RuntimeRequestConsumerProof {
        fn drop(&mut self) {
            if self.armed.get() && !std::thread::panicking() {
                assert!(
                    self.completion_error().is_none(),
                    "{}",
                    self.completion_error().unwrap()
                );
            }
        }
    }

    pub(super) async fn assert_negative_cases() -> anyhow::Result<()> {
        let dropped_without_request =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _dead = RuntimeRequestConsumerProof::new(
                    "list_board_labels_request_fixture_reaches_handler",
                );
            }));
        assert!(
            dropped_without_request.is_err(),
            "returning normally without a request must fail the Drop proof"
        );

        let remapped =
            RuntimeRequestConsumerProof::new("list_board_labels_request_fixture_reaches_handler");
        let test = TestApp::new()?;
        let _ = remapped
            .request_fixture_case(test.router(), "api.get-signal.path")
            .await?;
        assert!(
            remapped.finish().is_err(),
            "real execution of the wrong router case must not satisfy proof"
        );
        Ok(())
    }
}

use runtime_request_consumer_proof::RuntimeRequestConsumerProof;

macro_rules! runtime_consumer_proof {
    ($test_name:ident) => {
        RuntimeRequestConsumerProof::new(stringify!($test_name))
    };
}

#[derive(Debug, Default)]
struct ItemFnAstFacts {
    extractor_contracts: std::collections::BTreeSet<String>,
}

fn collect_extractor_contract_types(
    ty: &syn::Type,
    under_extractor: bool,
    contracts: &mut std::collections::BTreeSet<String>,
) {
    match ty {
        syn::Type::Path(type_path) => {
            let path = syn_path(&type_path.path);
            if under_extractor && path.starts_with("kanban_contract::") {
                contracts.insert(path);
            }
            let is_extractor = type_path.path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "Path" | "Json" | "Query"
                )
            });
            for segment in &type_path.path.segments {
                let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                    continue;
                };
                for argument in &arguments.args {
                    if let syn::GenericArgument::Type(argument) = argument {
                        collect_extractor_contract_types(
                            argument,
                            under_extractor || is_extractor,
                            contracts,
                        );
                    }
                }
            }
        }
        syn::Type::Reference(reference) => {
            collect_extractor_contract_types(&reference.elem, under_extractor, contracts);
        }
        syn::Type::Tuple(tuple) => {
            for element in &tuple.elems {
                collect_extractor_contract_types(element, under_extractor, contracts);
            }
        }
        syn::Type::Paren(paren) => {
            collect_extractor_contract_types(&paren.elem, under_extractor, contracts);
        }
        syn::Type::Group(group) => {
            collect_extractor_contract_types(&group.elem, under_extractor, contracts);
        }
        _ => {}
    }
}

fn syn_path(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn item_fn_ast_facts(source: &str, function: &str) -> Result<ItemFnAstFacts, String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let mut matches = file.items.iter().filter_map(|item| match item {
        syn::Item::Fn(item_fn) if item_fn.sig.ident == function => Some(item_fn),
        _ => None,
    });
    let item_fn = matches
        .next()
        .ok_or_else(|| format!("missing production ItemFn {function}"))?;
    if matches.next().is_some() {
        return Err(format!("duplicate production ItemFn {function}"));
    }
    let mut facts = ItemFnAstFacts::default();
    for input in &item_fn.sig.inputs {
        if let syn::FnArg::Typed(input) = input {
            collect_extractor_contract_types(&input.ty, false, &mut facts.extractor_contracts);
        }
    }
    Ok(facts)
}

fn production_item_fn_ast_facts(function: &str) -> ItemFnAstFacts {
    item_fn_ast_facts(include_str!("../../src/handlers/tasks.rs"), function).unwrap()
}

fn assert_handler_owns_contracts(handler: &str, contract_types: &[&str]) {
    let facts = production_item_fn_ast_facts(handler);
    for contract_type in contract_types {
        let expected = format!("kanban_contract::{contract_type}");
        assert!(
            facts.extractor_contracts.contains(&expected),
            "production handler {handler} no longer extracts {expected}; extractor contracts={:?}",
            facts.extractor_contracts
        );
    }
}

fn unique_item_fn<'a>(file: &'a syn::File, function: &str) -> Result<&'a syn::ItemFn, String> {
    let mut matches = file.items.iter().filter_map(|item| match item {
        syn::Item::Fn(item_fn) if item_fn.sig.ident == function => Some(item_fn),
        _ => None,
    });
    let item_fn = matches
        .next()
        .ok_or_else(|| format!("missing production ItemFn {function}"))?;
    if matches.next().is_some() {
        return Err(format!("duplicate production ItemFn {function}"));
    }
    Ok(item_fn)
}

fn local_binding(local: &syn::Local) -> Option<String> {
    match &local.pat {
        syn::Pat::Ident(binding) if binding.subpat.is_none() => Some(binding.ident.to_string()),
        _ => None,
    }
}

fn direct_try_call_path(expr: &syn::Expr) -> Option<&syn::Path> {
    let syn::Expr::Try(try_expr) = expr else {
        return None;
    };
    let syn::Expr::Call(call) = try_expr.expr.as_ref() else {
        return None;
    };
    let syn::Expr::Path(function) = call.func.as_ref() else {
        return None;
    };
    Some(&function.path)
}

fn direct_deserialized_contract(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Try(try_expr) = expr else {
        return None;
    };
    let call = match try_expr.expr.as_ref() {
        syn::Expr::Call(call) => call,
        syn::Expr::MethodCall(method) if method.method == "map_err" => {
            let syn::Expr::Call(call) = method.receiver.as_ref() else {
                return None;
            };
            call
        }
        _ => return None,
    };
    let syn::Expr::Path(function) = call.func.as_ref() else {
        return None;
    };
    if syn_path(&function.path) != "serde_urlencoded::from_str" {
        return None;
    }
    let arguments = match &function.path.segments.last()?.arguments {
        syn::PathArguments::AngleBracketed(arguments) => arguments,
        _ => return None,
    };
    let mut types = arguments.args.iter().filter_map(|argument| match argument {
        syn::GenericArgument::Type(syn::Type::Path(contract)) => Some(syn_path(&contract.path)),
        _ => None,
    });
    let contract = types.next()?;
    types.next().is_none().then_some(contract)
}

fn result_ok_type(output: &syn::ReturnType) -> Option<String> {
    let syn::ReturnType::Type(_, output) = output else {
        return None;
    };
    let syn::Type::Path(result) = output.as_ref() else {
        return None;
    };
    let segment = result.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    match arguments.args.first()? {
        syn::GenericArgument::Type(syn::Type::Path(ok)) => Some(syn_path(&ok.path)),
        _ => None,
    }
}

fn final_ok_binding(item_fn: &syn::ItemFn) -> Option<String> {
    let syn::Stmt::Expr(syn::Expr::Call(call), None) = item_fn.block.stmts.last()? else {
        return None;
    };
    let syn::Expr::Path(function) = call.func.as_ref() else {
        return None;
    };
    if syn_path(&function.path) != "Ok" || call.args.len() != 1 {
        return None;
    }
    let syn::Expr::Path(binding) = call.args.first()? else {
        return None;
    };
    (binding.path.segments.len() == 1).then(|| syn_path(&binding.path))
}

fn verify_raw_query_execution_chain(
    source: &str,
    handler: &str,
    parser: &str,
    contract_type: &str,
) -> Result<(), String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let handler_fn = unique_item_fn(&file, handler)?;
    let parser_fn = unique_item_fn(&file, parser)?;
    let expected = format!("kanban_contract::{contract_type}");

    let handler_bindings = handler_fn
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Local(local)
                if local
                    .init
                    .as_ref()
                    .and_then(|init| direct_try_call_path(&init.expr))
                    .is_some_and(|path| syn_path(path) == parser) =>
            {
                local_binding(local)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if handler_bindings.as_slice() != ["query"] {
        return Err(format!(
            "handler {handler} must bind one top-level `let query = {parser}(...)?`; bindings={handler_bindings:?}"
        ));
    }

    let returned = result_ok_type(&parser_fn.sig.output);
    if returned.as_deref() != Some(expected.as_str()) {
        return Err(format!(
            "parser {parser} must return Result<{expected}, _>; actual={returned:?}"
        ));
    }

    let parser_bindings = parser_fn
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Local(local)
                if local
                    .init
                    .as_ref()
                    .and_then(|init| direct_deserialized_contract(&init.expr))
                    .is_some_and(|contract| contract == expected) =>
            {
                local_binding(local)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if parser_bindings.len() != 1 {
        return Err(format!(
            "parser {parser} must directly deserialize {expected} in one top-level initializer; bindings={parser_bindings:?}"
        ));
    }
    let returned = final_ok_binding(parser_fn).ok_or_else(|| {
        format!("parser {parser} must end with a top-level Ok(binding) expression")
    })?;
    if returned != parser_bindings[0] {
        return Err(format!(
            "parser {parser} returns {returned}, not directly deserialized binding {}",
            parser_bindings[0]
        ));
    }
    Ok(())
}

fn assert_raw_query_execution_chain(handler: &str, parser: &str, contract_type: &str) {
    verify_raw_query_execution_chain(
        include_str!("../../src/handlers/tasks.rs"),
        handler,
        parser,
        contract_type,
    )
    .unwrap();
}

#[test]
fn handler_ast_proofs_ignore_comments_and_string_literals() {
    let hostile = r#"
        fn hostile() {
            // kanban_contract::BoardLabelPath; parse_signal_query();
            let _ = "kanban_sqlite::api::list_signals()";
            let _dead: Option<kanban_contract::BoardLabelPath> = None;
        }
    "#;
    let facts = item_fn_ast_facts(hostile, "hostile").unwrap();
    assert!(
        !facts
            .extractor_contracts
            .contains("kanban_contract::BoardLabelPath")
    );

    let dead_calls = r#"
        struct LocalPath;
        fn hostile_handler(_path: Path<LocalPath>) {
            if false {
                parse_signal_query(None);
                kanban_sqlite::api::list_signals(todo!(), todo!(), todo!());
            }
        }
    "#;
    let facts = item_fn_ast_facts(dead_calls, "hostile_handler").unwrap();
    assert!(facts.extractor_contracts.is_empty());

    let dead_turbofish = r#"
        struct PrivateQuery;
        fn hostile_handler() -> Result<(), ()> {
            let query = hostile_parser("")?;
            Ok(())
        }
        fn hostile_parser(raw: &str) -> Result<kanban_contract::SignalQuery, ()> {
            let query = serde_urlencoded::from_str::<PrivateQuery>(raw)?;
            if false {
                let _ = serde_urlencoded::from_str::<kanban_contract::SignalQuery>(raw)?;
            }
            Ok(query.into())
        }
    "#;
    assert!(
        verify_raw_query_execution_chain(
            dead_turbofish,
            "hostile_handler",
            "hostile_parser",
            "SignalQuery",
        )
        .is_err(),
        "a dead contract turbofish must not establish parser ownership"
    );

    let private_conversion = r#"
        struct PrivateQuery;
        fn hostile_handler() -> Result<(), ()> {
            let query = hostile_parser("")?;
            Ok(())
        }
        fn hostile_parser() -> Result<kanban_contract::SignalQuery, ()> {
            let query = serde_urlencoded::from_str::<PrivateQuery>("")?;
            Ok(query.into())
        }
    "#;
    assert!(
        verify_raw_query_execution_chain(
            private_conversion,
            "hostile_handler",
            "hostile_parser",
            "SignalQuery",
        )
        .is_err(),
        "a contract return signature must not prove direct contract deserialization"
    );

    let private_handler_parser = r#"
        struct PrivateQuery;
        fn hostile_handler() -> Result<(), ()> {
            let query = parse_private_query("")?;
            Ok(())
        }
        fn parse_private_query(_: &str) -> Result<PrivateQuery, ()> { todo!() }
        fn unused_contract_parser(raw: &str) -> Result<kanban_contract::SignalQuery, ()> {
            let query = serde_urlencoded::from_str::<kanban_contract::SignalQuery>(raw)?;
            Ok(query)
        }
    "#;
    assert!(
        verify_raw_query_execution_chain(
            private_handler_parser,
            "hostile_handler",
            "unused_contract_parser",
            "SignalQuery",
        )
        .is_err(),
        "an unused contract helper must not prove the handler executes it"
    );

    let nested_dead_call = r#"
        fn hostile_handler() -> Result<(), ()> {
            if false {
                let query = contract_parser("")?;
            }
            Ok(())
        }
        fn contract_parser(raw: &str) -> Result<kanban_contract::SignalQuery, ()> {
            let query = serde_urlencoded::from_str::<kanban_contract::SignalQuery>(raw)?;
            Ok(query)
        }
    "#;
    assert!(
        verify_raw_query_execution_chain(
            nested_dead_call,
            "hostile_handler",
            "contract_parser",
            "SignalQuery",
        )
        .is_err(),
        "a nested dead parser call must not establish the handler execution chain"
    );
}

#[test]
fn fixture_task_rename_rewrites_projection_identity() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    let conn = kanban_test_support::connect_file(test.db_path())?;
    let task_entity: (String, String, String) = conn.query_row(
        "SELECT uri, source_id, task_id FROM entities WHERE source_table='tasks'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(
        task_entity,
        (
            "kb://task/t_fixture".into(),
            "t_fixture".into(),
            "t_fixture".into()
        )
    );
    let relation: (String, String) = conn.query_row(
        "SELECT subject_uri, source_id FROM entity_relations WHERE source_table='tasks'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(relation, ("kb://task/t_fixture".into(), "t_fixture".into()));
    let stale_outbox: i64 = conn.query_row(
        "SELECT COUNT(*) FROM index_outbox
         WHERE entity_uri LIKE 'kb://task/%' AND entity_uri <> 'kb://task/t_fixture'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(stale_outbox, 0);
    Ok(())
}

async fn execute_request_fixture_case(
    app: axum::Router,
    contract_id: &str,
) -> anyhow::Result<(StatusCode, Value)> {
    let case = request_router_case(contract_id);
    let (method, uri, body) = request_from_contract_fixtures(&case)?;
    request_json(app, method, &uri, body, None).await
}

fn assert_fixture_foreign_keys_valid(test: &TestApp, context: &str) -> anyhow::Result<()> {
    let conn = kanban_test_support::connect_file(test.db_path())?;
    let mut statement = conn.prepare("PRAGMA foreign_key_check")?;
    let violations = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    assert!(
        violations.is_empty(),
        "{context} broke foreign keys: {violations:?}"
    );
    Ok(())
}

fn seed_fixture_board(test: &TestApp) -> anyhow::Result<()> {
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture".into(),
            name: "fixture".into(),
            description: None,
        },
    )?;
    Ok(())
}

fn seed_fixture_task(test: &TestApp) -> anyhow::Result<()> {
    seed_fixture_board(test)?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "fixture",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("fixture"),
    )?;
    let old_task_id = task.id;
    let old_task_uri = format!("kb://task/{old_task_id}");
    let new_task_id = "t_fixture";
    let new_task_uri = "kb://task/t_fixture";
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "UPDATE entity_relations SET subject_uri=?1 WHERE subject_uri=?2",
        (new_task_uri, &old_task_uri),
    )?;
    conn.execute(
        "UPDATE entity_relations SET object_uri=?1 WHERE object_uri=?2",
        (new_task_uri, &old_task_uri),
    )?;
    conn.execute(
        "UPDATE entity_relations SET source_id=?1
         WHERE source_table='tasks' AND source_id=?2",
        (new_task_id, &old_task_id),
    )?;
    conn.execute(
        "UPDATE index_outbox SET entity_uri=?1 WHERE entity_uri=?2",
        (new_task_uri, &old_task_uri),
    )?;
    conn.execute(
        "UPDATE entities SET uri=?1, source_id=?2
         WHERE uri=?3 AND source_table='tasks' AND source_id=?4",
        (new_task_uri, new_task_id, &old_task_uri, &old_task_id),
    )?;
    conn.execute(
        "UPDATE tasks SET id=?1 WHERE id=?2",
        (new_task_id, &old_task_id),
    )?;
    conn.execute(
        "UPDATE task_events SET task_id=?1 WHERE task_id=?2",
        (new_task_id, &old_task_id),
    )?;
    conn.execute(
        "UPDATE entities SET task_id=?1 WHERE task_id=?2",
        (new_task_id, &old_task_id),
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    let stale_projection_identity: i64 = conn.query_row(
        "SELECT
           (SELECT COUNT(*) FROM tasks WHERE id=?1) +
           (SELECT COUNT(*) FROM task_events WHERE task_id=?1) +
           (SELECT COUNT(*) FROM entities WHERE task_id=?1 OR source_id=?1 OR uri=?2) +
           (SELECT COUNT(*) FROM entity_relations
            WHERE subject_uri=?2 OR object_uri=?2 OR source_id=?1) +
           (SELECT COUNT(*) FROM index_outbox WHERE entity_uri=?2)",
        (&old_task_id, &old_task_uri),
        |row| row.get(0),
    )?;
    assert_eq!(
        stale_projection_identity, 0,
        "fixture task rename left stale projection identity for {old_task_id}"
    );
    drop(conn);
    assert_fixture_foreign_keys_valid(test, "fixture task rename")?;
    Ok(())
}

fn seed_fixture_label(test: &TestApp) -> anyhow::Result<()> {
    let label = kanban_sqlite::api::create_label(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateLabel {
            name: "fixture".into(),
            color: None,
        },
    )?;
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute("UPDATE labels SET id='l_fixture' WHERE id=?1", [&label.id])?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    drop(conn);
    assert_fixture_foreign_keys_valid(test, "fixture label rename")?;
    Ok(())
}

fn seed_fixture_semantics(test: &TestApp) -> anyhow::Result<String> {
    let semantics = kanban_sqlite::api::upsert_label_semantics_by_id(
        test.db_path(),
        "fixture",
        "l_fixture",
        kanban_sqlite::api::UpsertLabelSemantics {
            label_ref: "l_fixture".into(),
            description: Some("fixture semantics".into()),
            applies_when: vec!["fixture".into()],
            ..Default::default()
        },
    )?;
    Ok(semantics.semantics_hash)
}

fn rename_fixture_atom(test: &TestApp) -> anyhow::Result<()> {
    let conn = kanban_test_support::connect_file(test.db_path())?;
    let already_present: i64 = conn.query_row(
        "SELECT COUNT(*) FROM label_atoms WHERE id='la_51d68accaaa974f4'",
        [],
        |row| row.get(0),
    )?;
    if already_present == 1 {
        drop(conn);
        assert_fixture_foreign_keys_valid(test, "fixture atom rename")?;
        return Ok(());
    }
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "UPDATE label_atoms SET id='la_51d68accaaa974f4'
         WHERE label_id='l_fixture' AND text='fixture'",
        [],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    drop(conn);
    assert_fixture_foreign_keys_valid(test, "fixture atom rename")?;
    Ok(())
}

fn seed_fixture_generic_signal(test: &TestApp) -> anyhow::Result<()> {
    let conn = kanban_test_support::connect_file(test.db_path())?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='fixture'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO signal_observations(
            id, board_id, task_id, task_ref_snapshot, actor, agent_type, source,
            evidence_json, created_at
         ) VALUES ('obs_fixture', ?1, 't_fixture', 'fixture#1', 'fixture', 'fixture',
                   'fixture', '{}', 1)",
        [&board_id],
    )?;
    conn.execute(
        "INSERT INTO signals(
            id, board_id, observation_id, kind, title, summary, severity, status,
            dedupe_key, created_at, updated_at
         ) VALUES ('sig_fixture', ?1, 'obs_fixture', 'fixture', 'fixture', 'fixture',
                   'info', 'open', 'fixture', 1, 1)",
        [&board_id],
    )?;
    Ok(())
}

fn seed_fixture_signal_query_sentinels(test: &TestApp) -> anyhow::Result<()> {
    let other_task = kanban_sqlite::api::create_task(
        test.db_path(),
        "fixture",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("other signal target"),
    )?;
    let conn = kanban_test_support::connect_file(test.db_path())?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='fixture'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO signal_observations(
            id, board_id, task_id, task_ref_snapshot, actor, agent_type, source,
            evidence_json, created_at
         ) VALUES ('obs_other', ?1, ?2, ?3, 'fixture', 'fixture', 'fixture', '{}', 2)",
        (&board_id, &other_task.id, &other_task.task_ref),
    )?;
    conn.execute(
        "INSERT INTO signals(
            id, board_id, observation_id, kind, title, summary, severity, status,
            dedupe_key, created_at, updated_at
         ) VALUES ('sig_other', ?1, 'obs_other', 'other', 'other', 'other',
                   'info', 'open', 'other', 2, 2)",
        [&board_id],
    )?;
    conn.execute(
        "INSERT INTO signal_observations(
            id, board_id, task_id, task_ref_snapshot, actor, agent_type, source,
            evidence_json, created_at
         ) VALUES ('obs_resolved', ?1, 't_fixture', 'fixture#1', 'fixture', 'fixture',
                   'fixture', '{}', 3)",
        [&board_id],
    )?;
    conn.execute(
        "INSERT INTO signals(
            id, board_id, observation_id, kind, title, summary, severity, status,
            dedupe_key, created_at, updated_at
         ) VALUES ('sig_resolved', ?1, 'obs_resolved', 'fixture', 'resolved', 'resolved',
                   'info', 'resolved', 'resolved', 3, 3)",
        [&board_id],
    )?;
    Ok(())
}

async fn assert_signal_query_sentinels(app: axum::Router, path: &str) -> anyhow::Result<()> {
    let (status, response) = get_json(
        app.clone(),
        &format!("{path}?status=resolved&include_all=true"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response["data"].as_array().context("status signals")?.len(),
        1
    );
    assert_eq!(response["data"][0]["id"], "sig_resolved");

    let (status, response) = get_json(app.clone(), &format!("{path}?kind=other")).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response["data"].as_array().context("kind signals")?.len(),
        1
    );
    assert_eq!(response["data"][0]["id"], "sig_other");

    let (status, response) = get_json(app.clone(), &format!("{path}?task_ref=t_fixture")).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response["data"].as_array().context("task signals")?.len(),
        1
    );
    assert_eq!(response["data"][0]["id"], "sig_fixture");

    let (status, response) = get_json(app.clone(), &format!("{path}?include_all=true")).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"].as_array().context("all signals")?.len(), 3);
    assert_eq!(response["meta"]["include_all"], true);

    let (status, response) = get_json(app, &format!("{path}?limit=1")).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response["data"]
            .as_array()
            .context("limited signals")?
            .len(),
        1
    );
    assert_eq!(response["meta"]["limit"], 1);
    Ok(())
}

async fn seed_fixture_ontology_signal(test: &TestApp) -> anyhow::Result<()> {
    let (status, response) =
        execute_request_fixture_case(test.router(), "api.record-label-ontology-observation.body")
            .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    let signal_id = response["data"]["signals"][0]["id"]
        .as_str()
        .context("fixture ontology signal id")?;
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "UPDATE label_ontology_signals SET id='los_fixture' WHERE id=?1",
        [signal_id],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    drop(conn);
    assert_fixture_foreign_keys_valid(test, "fixture ontology signal rename")?;
    Ok(())
}

async fn seed_fixture_apply_action(test: &TestApp) -> anyhow::Result<()> {
    let (status, response) =
        execute_request_fixture_case(test.router(), "api.create-label-ontology-action.request")
            .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    let (status, response) =
        execute_request_fixture_case(test.router(), "api.apply-label-ontology-atom.request")
            .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    let action_id = response["data"]["id"]
        .as_str()
        .context("fixture apply action id")?;
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "UPDATE label_ontology_action_atom_effects
         SET action_id='loa_target_fixture' WHERE action_id=?1",
        [action_id],
    )?;
    conn.execute(
        "UPDATE label_ontology_action_signals
         SET action_id='loa_target_fixture' WHERE action_id=?1",
        [action_id],
    )?;
    conn.execute(
        "UPDATE label_ontology_actions
         SET parent_action_id='loa_target_fixture' WHERE parent_action_id=?1",
        [action_id],
    )?;
    conn.execute(
        "UPDATE label_ontology_actions SET id='loa_target_fixture' WHERE id=?1",
        [action_id],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    drop(conn);
    assert_fixture_foreign_keys_valid(test, "fixture action rename")?;
    Ok(())
}

fn seed_fixture_proposal_for_exact_task(test: &TestApp) -> anyhow::Result<()> {
    seed_fixture_proposal(test, "t_fixture")
}

async fn assert_label_suggestion_sentinels(
    app: axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> anyhow::Result<()> {
    for (query, expected_message) in [
        ("limit=0", "limit must be >= 1"),
        ("candidate_limit=0", "candidate_limit must be >= 1"),
        ("atom_limit=0", "atom_limit must be >= 1"),
        ("max_selected_labels=0", "max_selected_labels must be >= 1"),
        ("min_score=2", "min_score must be between 0 and 1"),
    ] {
        let uri = format!("{path}?{query}");
        let (status, response) =
            request_json(app.clone(), method, &uri, body.clone(), None).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {response}");
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains(expected_message),
            "{uri}: {response}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn api_generated_request_fixtures_are_consumed_by_real_router() -> anyhow::Result<()> {
    let cases = request_router_cases();

    let expected = REQUEST_CONTRACT_IDS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let registered = cases
        .iter()
        .flat_map(|case| case.contracts.iter().copied())
        .collect::<Vec<_>>();
    let covered = registered
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(covered, expected);
    assert_eq!(
        registered.len(),
        covered.len(),
        "duplicate contract registration"
    );
    for id in &registered {
        let root = contract::operation_inventory()
            .iter()
            .find(|root| root.id == *id)
            .unwrap_or_else(|| panic!("registered request contract is missing: {id}"));
        assert_eq!(root.surface, contract::ContractSurface::Api, "{id}");
        assert_eq!(
            root.direction,
            contract::ContractDirection::Deserialize,
            "{id}"
        );
        assert!(
            matches!(
                root.migration,
                contract::MigrationState::Generated | contract::MigrationState::Adopted
            ),
            "{id}: {:?}",
            root.migration
        );
    }

    let test = TestApp::new()?;
    for case in cases {
        let (method, uri, body) = request_from_contract_fixtures(&case)?;
        let (status, response) = request_json(test.router(), method, &uri, body, None).await?;
        assert_ne!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{} {}: {response}",
            method,
            uri
        );
        let message = response
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !message.contains("Failed to deserialize"),
            "{} {}: {response}",
            method,
            uri
        );
        assert!(
            !message.contains("Failed to parse"),
            "{} {}: {response}",
            method,
            uri
        );
        assert!(
            !message.contains("unknown field"),
            "{} {}: {response}",
            method,
            uri
        );
    }
    Ok(())
}

#[tokio::test]
async fn list_board_labels_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(list_board_labels_request_fixture_reaches_handler);
    assert_handler_owns_contracts("list_board_labels", &["BoardLabelPath"]);
    let test = TestApp::new()?;
    seed_fixture_board(&test)?;
    seed_fixture_label(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.list-board-labels.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"][0]["id"], "l_fixture");
    let (status, default) = get_json(test.router(), "/api/v1/boards/default/labels").await?;
    assert_eq!(status, StatusCode::OK, "{default}");
    assert!(
        default["data"]
            .as_array()
            .context("default labels")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn create_board_label_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(create_board_label_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "create_board_label",
        &["BoardLabelPath", "CreateBoardLabelRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_board(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.create-board-label.request")
        .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["data"]["name"], "fixture");
    assert_eq!(
        kanban_sqlite::api::list_labels(test.db_path(), "fixture")?[0].name,
        "fixture"
    );
    assert!(kanban_sqlite::api::list_labels(test.db_path(), "default")?.is_empty());
    Ok(())
}

#[tokio::test]
async fn list_label_semantics_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(list_label_semantics_request_fixture_reaches_handler);
    assert_handler_owns_contracts("list_label_semantics", &["BoardLabelPath"]);
    let test = TestApp::new()?;
    seed_fixture_board(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_semantics(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.list-label-semantics.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"][0]["label_id"], "l_fixture");
    assert!(
        kanban_sqlite::api::list_label_semantics(test.db_path(), "default")?.is_empty(),
        "fixture board semantics must not leak into default"
    );
    Ok(())
}

#[tokio::test]
async fn get_label_semantics_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(get_label_semantics_request_fixture_reaches_handler);
    assert_handler_owns_contracts("get_label_semantics", &["LabelSemanticsPath"]);
    let test = TestApp::new()?;
    seed_fixture_board(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_semantics(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.get-label-semantics.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["label_id"], "l_fixture");
    assert_eq!(response["data"]["description"], "fixture semantics");
    Ok(())
}

#[tokio::test]
async fn upsert_label_semantics_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(upsert_label_semantics_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "upsert_label_semantics",
        &["LabelSemanticsPath", "UpsertLabelSemanticsRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_board(&test)?;
    seed_fixture_label(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.upsert-label-semantics.request")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["label_id"], "l_fixture");
    assert_eq!(
        kanban_sqlite::api::get_label_semantics_by_id(test.db_path(), "fixture", "l_fixture")?
            .label_id,
        "l_fixture"
    );
    Ok(())
}

#[tokio::test]
async fn delete_label_semantics_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(delete_label_semantics_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "delete_label_semantics",
        &["LabelSemanticsPath", "DeleteLabelSemanticsQuery"],
    );
    let test = TestApp::new()?;
    seed_fixture_board(&test)?;
    seed_fixture_label(&test)?;
    let current_hash = seed_fixture_semantics(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.delete-label-semantics.query")
        .await?;
    assert_eq!(status, StatusCode::CONFLICT, "{response}");
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("expected fixture"),
        "{response}"
    );
    let uri = format!(
        "/api/v1/boards/fixture/labels/l_fixture/semantics?expected_semantics_hash={current_hash}&reason=fixture"
    );
    let (status, response) = request_json(test.router(), "DELETE", &uri, None, None).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["deleted"], true);
    Ok(())
}

#[tokio::test]
async fn list_label_atoms_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(list_label_atoms_request_fixture_reaches_handler);
    assert_handler_owns_contracts("list_label_atoms", &["BoardLabelPath"]);
    let test = TestApp::new()?;
    seed_fixture_board(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_semantics(&test)?;
    rename_fixture_atom(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.list-label-atoms.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(
        response["data"]
            .as_array()
            .context("fixture atoms")?
            .iter()
            .any(|atom| atom["id"] == "la_51d68accaaa974f4"),
        "{response}"
    );
    assert!(
        kanban_sqlite::api::list_label_atoms(test.db_path(), "default")?.is_empty(),
        "fixture board atoms must not leak into default"
    );
    Ok(())
}

#[tokio::test]
async fn explain_label_atom_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(explain_label_atom_request_fixture_reaches_handler);
    assert_handler_owns_contracts("explain_label_atom", &["LabelAtomPath"]);
    let test = TestApp::new()?;
    seed_fixture_board(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_semantics(&test)?;
    rename_fixture_atom(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.label-atom.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response.to_string().contains("la_51d68accaaa974f4"));
    Ok(())
}

fn atom_index_app(test: &TestApp) -> anyhow::Result<(axum::Router, std::path::PathBuf)> {
    seed_fixture_board(test)?;
    let helper = write_atom_index_protocol_helper(test)?;
    let log = std::path::PathBuf::from(format!("{}.args.jsonl", helper.display()));
    Ok((
        build_router(AppState::new(test.db_path(), "fixture").with_vector_helper_path(helper)),
        log,
    ))
}

fn assert_atom_index_helper_args(log: &std::path::Path, expected: &[&str]) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(log).context("atom index helper argv log")?;
    let args: Vec<String> = serde_json::from_str(raw.lines().last().context("helper argv line")?)?;
    for value in expected {
        assert!(
            args.iter().any(|arg| arg == value),
            "missing {value}: {args:?}"
        );
    }
    Ok(())
}

fn assert_atom_index_query_helper_args(
    log: &std::path::Path,
    expected_board: &str,
    expected_query: &str,
    expected_limit: &str,
) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(log).context("atom index helper argv log")?;
    let args: Vec<String> = serde_json::from_str(raw.lines().last().context("helper argv line")?)?;
    let command = args
        .iter()
        .position(|arg| arg == "query-label-atoms")
        .context("query-label-atoms argv")?;
    assert_eq!(
        &args[command..command + 5],
        [
            "query-label-atoms",
            "--text",
            expected_query,
            "--limit",
            expected_limit,
        ],
        "query argv prefix/order drift: {args:?}"
    );
    let board = args
        .iter()
        .position(|arg| arg == "--board")
        .context("--board argv")?;
    assert_eq!(
        args.get(board + 1).map(String::as_str),
        Some(expected_board)
    );
    assert_eq!(
        args.iter().filter(|arg| arg.as_str() == "--text").count(),
        1,
        "ambiguous --text argv: {args:?}"
    );
    Ok(())
}

#[tokio::test]
async fn label_atom_index_status_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(label_atom_index_status_request_fixture_reaches_handler);
    assert_handler_owns_contracts("label_atom_index_status", &["BoardLabelPath"]);
    let test = TestApp::new()?;
    let (app, log) = atom_index_app(&test)?;
    let (status, response) = proof
        .request_fixture_case(app, "api.label-atom-index-status.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_atom_index_helper_args(&log, &["fixture", "label-atoms-status"])
}

#[tokio::test]
async fn rebuild_label_atom_index_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(rebuild_label_atom_index_request_fixture_reaches_handler);
    assert_handler_owns_contracts("rebuild_label_atom_index", &["BoardLabelPath"]);
    let test = TestApp::new()?;
    let (app, log) = atom_index_app(&test)?;
    let (status, response) = proof
        .request_fixture_case(app, "api.rebuild-label-atom-index.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_atom_index_helper_args(&log, &["fixture", "rebuild-label-atoms"])
}

#[tokio::test]
async fn query_label_atom_index_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(query_label_atom_index_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "query_label_atom_index",
        &["BoardLabelPath", "LabelAtomIndexQuery"],
    );
    let test = TestApp::new()?;
    let (app, log) = atom_index_app(&test)?;
    let (status, response) = proof
        .request_fixture_case(app.clone(), "api.query-label-atom-index.query")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_atom_index_helper_args(
        &log,
        &["fixture", "query-label-atoms", "--text", "--limit", "24"],
    )?;
    assert_atom_index_query_helper_args(&log, "fixture", "fixture", "24")?;
    let (status, response) = get_json(
        app,
        "/api/v1/boards/fixture/labels/atom-index/query?q=query-sentinel&limit=7",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_atom_index_query_helper_args(&log, "fixture", "query-sentinel", "7")?;
    Ok(())
}

#[tokio::test]
async fn list_signals_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(list_signals_request_fixtures_reach_handler);
    assert_handler_owns_contracts("list_signals", &["BoardLabelPath"]);
    assert_raw_query_execution_chain("list_signals", "parse_signal_query", "SignalQuery");
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_generic_signal(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.list-signals.query")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"][0]["id"], "sig_fixture");
    assert_eq!(response["meta"]["limit"], 100);
    seed_fixture_signal_query_sentinels(&test)?;
    assert_signal_query_sentinels(test.router(), "/api/v1/boards/fixture/signals").await
}

#[tokio::test]
async fn review_signals_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(review_signals_request_fixtures_reach_handler);
    assert_handler_owns_contracts("review_signals", &["BoardLabelPath"]);
    assert_raw_query_execution_chain("review_signals", "parse_signal_query", "SignalQuery");
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_generic_signal(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.review-signals.query")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"][0]["id"], "sig_fixture");
    assert_eq!(response["meta"]["limit"], 100);
    seed_fixture_signal_query_sentinels(&test)?;
    assert_signal_query_sentinels(test.router(), "/api/v1/boards/fixture/signals/review").await
}

#[tokio::test]
async fn get_signal_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(get_signal_request_fixture_reaches_handler);
    assert_handler_owns_contracts("get_signal", &["SignalPath"]);
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_generic_signal(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.get-signal.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["id"], "sig_fixture");
    Ok(())
}

#[tokio::test]
async fn bootstrap_task_label_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(bootstrap_task_label_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "bootstrap_task_label",
        &["TaskLabelSurfacePath", "BootstrapTaskLabelRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.bootstrap-task-label.request")
        .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["data"]["task"]["id"], "t_fixture");
    assert_eq!(response["data"]["semantics"]["label_name"], "fixture");
    assert_eq!(response["data"]["semantics"]["description"], "fixture");
    Ok(())
}

#[tokio::test]
async fn suggest_task_labels_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(suggest_task_labels_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "suggest_task_labels",
        &["TaskLabelSurfacePath", "LabelSuggestionQuery"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.label-suggestion.query")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["task_id"], "t_fixture");
    assert_label_suggestion_sentinels(
        test.router(),
        "GET",
        "/api/v1/tasks/t_fixture/labels/suggestions",
        None,
    )
    .await
}

#[tokio::test]
async fn list_task_label_proposals_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(list_task_label_proposals_request_fixture_reaches_handler);
    assert_handler_owns_contracts("list_task_label_proposals", &["TaskLabelSurfacePath"]);
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_proposal_for_exact_task(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.list-task-label-proposals.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"][0]["id"], "lp_fixture");
    assert_eq!(response["data"][0]["task_id"], "t_fixture");
    Ok(())
}

#[tokio::test]
async fn propose_task_label_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(propose_task_label_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "propose_task_label",
        &[
            "TaskLabelSurfacePath",
            "LabelSuggestionQuery",
            "ProposeTaskLabelRequest",
        ],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.propose-task-label.request")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["task_id"], "t_fixture");
    assert!(response["data"]["proposal"].is_null());
    assert_label_suggestion_sentinels(
        test.router(),
        "POST",
        "/api/v1/tasks/t_fixture/label-proposals",
        Some(fixture("propose-task-label-request.v1.valid.json")),
    )
    .await
}

#[tokio::test]
async fn record_label_ontology_observation_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof =
        runtime_consumer_proof!(record_label_ontology_observation_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "record_label_ontology_observation",
        &[
            "TaskLabelSurfacePath",
            "RecordLabelOntologyObservationRequest",
        ],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_label(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.record-label-ontology-observation.body")
        .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["data"]["task_id"], "t_fixture");
    assert_eq!(response["data"]["created_by"], "fixture");
    assert_eq!(response["data"]["signals"][0]["rationale"], "fixture");
    assert_eq!(response["data"]["signals"][0]["candidate_text"], "fixture");
    Ok(())
}

#[tokio::test]
async fn list_label_ontology_signals_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(list_label_ontology_signals_request_fixtures_reach_handler);
    assert_handler_owns_contracts("list_label_ontology_signals", &["BoardLabelPath"]);
    assert_raw_query_execution_chain(
        "list_label_ontology_signals",
        "parse_label_ontology_signal_query",
        "LabelOntologySignalQuery",
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_ontology_signal(&test).await?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.label-ontology-signal.query")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"][0]["id"], "los_fixture");
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute(
        "UPDATE label_ontology_signals SET status='resolved' WHERE id='los_fixture'",
        [],
    )?;
    drop(conn);
    let (status, hidden) = get_json(
        test.router(),
        "/api/v1/boards/fixture/label-ontology/signals",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{hidden}");
    assert!(
        hidden["data"]
            .as_array()
            .context("hidden signals")?
            .is_empty()
    );
    let (status, all) = get_json(
        test.router(),
        "/api/v1/boards/fixture/label-ontology/signals?include_all=true",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{all}");
    assert_eq!(all["data"][0]["id"], "los_fixture");
    Ok(())
}

#[tokio::test]
async fn review_label_ontology_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(review_label_ontology_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "review_label_ontology",
        &["BoardLabelPath", "LabelOntologyReviewQuery"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_ontology_signal(&test).await?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.label-ontology-review.query")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["meta"]["group_by"], "label");
    assert_eq!(response["data"][0]["signal_ids"][0], "los_fixture");
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute(
        "UPDATE label_ontology_signals SET status='resolved' WHERE id='los_fixture'",
        [],
    )?;
    drop(conn);
    let (status, hidden) = get_json(
        test.router(),
        "/api/v1/boards/fixture/label-ontology/review",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{hidden}");
    assert!(
        hidden["data"]
            .as_array()
            .context("hidden groups")?
            .is_empty()
    );
    let (status, all) = get_json(
        test.router(),
        "/api/v1/boards/fixture/label-ontology/review?include_all=true",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{all}");
    assert_eq!(all["meta"]["include_all"], true);
    assert_eq!(all["data"][0]["signal_ids"][0], "los_fixture");
    Ok(())
}

#[tokio::test]
async fn create_label_ontology_action_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof =
        runtime_consumer_proof!(create_label_ontology_action_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "create_label_ontology_action",
        &["BoardLabelPath", "LabelOntologyActionRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_ontology_signal(&test).await?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.create-label-ontology-action.request")
        .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["data"]["action_type"], "confirm");
    assert_eq!(response["data"]["signal_ids"][0], "los_fixture");
    assert_eq!(response["data"]["reason"], "fixture");
    Ok(())
}

#[tokio::test]
async fn apply_label_ontology_atom_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(apply_label_ontology_atom_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "apply_label_ontology_atom",
        &["BoardLabelPath", "ApplyLabelOntologyAtomRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_ontology_signal(&test).await?;
    let (status, response) =
        execute_request_fixture_case(test.router(), "api.create-label-ontology-action.request")
            .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.apply-label-ontology-atom.request")
        .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["data"]["signal_ids"][0], "los_fixture");
    assert_eq!(response["data"]["reason"], "fixture");
    assert_eq!(response["data"]["change"]["added_atom"]["text"], "fixture");
    Ok(())
}

#[tokio::test]
async fn revert_label_ontology_mutation_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof =
        runtime_consumer_proof!(revert_label_ontology_mutation_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "revert_label_ontology_mutation",
        &["BoardLabelPath", "RevertLabelOntologyMutationRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_ontology_signal(&test).await?;
    seed_fixture_apply_action(&test).await?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.revert-label-ontology-mutation.request")
        .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["data"]["parent_action_id"], "loa_target_fixture");
    assert_eq!(
        response["data"]["change"]["reverted_action_id"],
        "loa_target_fixture"
    );
    assert_eq!(response["data"]["reason"], "fixture");
    Ok(())
}

#[tokio::test]
async fn validate_label_ontology_action_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof =
        runtime_consumer_proof!(validate_label_ontology_action_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "validate_label_ontology_action",
        &["BoardLabelPath", "ValidateLabelOntologyActionRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_ontology_signal(&test).await?;
    seed_fixture_apply_action(&test).await?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.validate-label-ontology-action.request")
        .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["data"]["parent_action_id"], "loa_target_fixture");
    assert_eq!(response["data"]["validation_status"], "failed");
    assert_eq!(response["data"]["reason"], "fixture");
    Ok(())
}

#[tokio::test]
async fn get_label_ontology_signal_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(get_label_ontology_signal_request_fixture_reaches_handler);
    assert_handler_owns_contracts("get_label_ontology_signal", &["SignalPath"]);
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_label(&test)?;
    seed_fixture_ontology_signal(&test).await?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.get-label-ontology-signal.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["signal"]["id"], "los_fixture");
    assert_eq!(response["data"]["observation"]["task_id"], "t_fixture");
    Ok(())
}

#[tokio::test]
async fn get_label_proposal_request_fixture_reaches_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(get_label_proposal_request_fixture_reaches_handler);
    assert_handler_owns_contracts("get_label_proposal", &["ProposalPath"]);
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_proposal_for_exact_task(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.get-label-proposal.path")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["id"], "lp_fixture");
    assert_eq!(response["data"]["task_id"], "t_fixture");
    Ok(())
}

#[tokio::test]
async fn accept_label_proposal_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(accept_label_proposal_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "accept_label_proposal",
        &["ProposalPath", "LabelProposalDecisionRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_proposal_for_exact_task(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.accept-label-proposal.body")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["id"], "lp_fixture");
    assert_eq!(response["data"]["status"], "accepted");
    Ok(())
}

#[tokio::test]
async fn reject_label_proposal_request_fixtures_reach_handler() -> anyhow::Result<()> {
    let proof = runtime_consumer_proof!(reject_label_proposal_request_fixtures_reach_handler);
    assert_handler_owns_contracts(
        "reject_label_proposal",
        &["ProposalPath", "LabelProposalDecisionRequest"],
    );
    let test = TestApp::new()?;
    seed_fixture_task(&test)?;
    seed_fixture_proposal_for_exact_task(&test)?;
    let (status, response) = proof
        .request_fixture_case(test.router(), "api.reject-label-proposal.body")
        .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["id"], "lp_fixture");
    assert_eq!(response["data"]["status"], "rejected");

    let (test, task_id) = fixture_task_app()?;
    seed_fixture_proposal(&test, &task_id)?;
    let (status, response) = post_json(
        test.router(),
        "/api/v1/label-proposals/lp_fixture/reject",
        json!({"reason":"reject sentinel","actor":"reject-fixture"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["decision_reason"], "reject sentinel");
    let conn = kanban_test_support::connect_file(test.db_path())?;
    let actor: String = conn.query_row(
        "SELECT actor FROM task_events
         WHERE kind='task.label_proposal.rejected'
         ORDER BY created_at DESC, id DESC LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(actor, "reject-fixture");
    Ok(())
}

#[tokio::test]
async fn label_suggestion_query_fields_reach_runtime_validation_for_both_routes()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("query field witness"),
    )?;
    let sentinels = [
        ("limit=0", "limit must be >= 1"),
        ("candidate_limit=0", "candidate_limit must be >= 1"),
        ("atom_limit=0", "atom_limit must be >= 1"),
        ("max_selected_labels=0", "max_selected_labels must be >= 1"),
        ("min_score=2", "min_score must be between 0 and 1"),
    ];

    for suffix in ["labels/suggestions", "label-proposals"] {
        for (query, expected_message) in sentinels {
            let uri = format!("/api/v1/tasks/{}/{suffix}?{query}", task.id);
            let (status, response) = if suffix == "label-proposals" {
                request_json(test.router(), "POST", &uri, None, None).await?
            } else {
                get_json(test.router(), &uri).await?
            };
            assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {response}");
            assert!(
                response["error"]["message"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(expected_message),
                "{uri}: {response}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn generated_empty_collection_responses_are_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture".into(),
            name: "fixture".into(),
            description: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "fixture",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("fixture"),
    )?;
    let cases = [
        (
            "/api/v1/boards/fixture/labels",
            "list-board-labels-response.v1.valid.json",
        ),
        (
            "/api/v1/boards/fixture/labels/semantics",
            "list-label-semantics-response.v1.valid.json",
        ),
        (
            "/api/v1/boards/fixture/labels/atoms",
            "list-label-atoms-response.v1.valid.json",
        ),
        (
            "/api/v1/boards/fixture/signals?limit=1",
            "list-signals-response.v1.valid.json",
        ),
        (
            "/api/v1/boards/fixture/signals/review?limit=1",
            "review-signals-response.v1.valid.json",
        ),
        (
            "/api/v1/boards/fixture/label-ontology/review?group_by=label&limit=1",
            "review-label-ontology-response.v1.valid.json",
        ),
    ];
    for (uri, fixture_name) in cases {
        let (status, response) = get_json(test.router(), uri).await?;
        assert_eq!(status, StatusCode::OK, "{uri}: {response}");
        assert_eq!(response, fixture(fixture_name), "{uri}");
    }

    let uri = format!("/api/v1/tasks/{}/label-proposals", task.id);
    let (status, response) = get_json(test.router(), &uri).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response,
        fixture("list-task-label-proposals-response.v1.valid.json")
    );
    Ok(())
}

fn normalize_label(response: &mut Value) {
    response["id"] = json!("l_fixture");
    response["board_id"] = json!("fixture");
    response["created_at"] = json!(1);
    response["updated_at"] = json!(1);
}

fn normalize_semantics(response: &mut Value) {
    response["label_id"] = json!("l_fixture");
    response["board_id"] = json!("fixture");
    response["semantics_hash"] = json!("fixture");
    response["created_at"] = json!(1);
    response["updated_at"] = json!(1);
    for atom in response["atoms"].as_array_mut().into_iter().flatten() {
        atom["id"] = json!("la_fixture");
        atom["label_id"] = json!("l_fixture");
        atom["board_id"] = json!("fixture");
        atom["content_hash"] = json!("fixture");
        atom["created_at"] = json!(1);
        atom["updated_at"] = json!(1);
    }
}

#[tokio::test]
async fn generated_label_responses_are_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture".into(),
            name: "fixture".into(),
            description: None,
        },
    )?;
    let app = test.router();
    let (status, mut created) = post_json(
        app.clone(),
        "/api/v1/boards/fixture/labels",
        fixture("create-board-label-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let label_id = created["data"]["id"]
        .as_str()
        .context("label id")?
        .to_owned();
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute("UPDATE labels SET id='l_fixture' WHERE id=?1", [&label_id])?;
    normalize_label(&mut created["data"]);
    assert_eq!(
        created,
        fixture("create-board-label-response.v1.valid.json")
    );

    let uri = "/api/v1/boards/fixture/labels/l_fixture/semantics";
    let (status, mut upserted) = request_json(
        app.clone(),
        "PUT",
        uri,
        Some(fixture("upsert-label-semantics-request.v1.valid.json")),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{upserted}");
    normalize_semantics(&mut upserted["data"]);
    assert_eq!(
        upserted,
        fixture("upsert-label-semantics-response.v1.valid.json")
    );

    let (status, mut fetched) = get_json(app.clone(), uri).await?;
    assert_eq!(status, StatusCode::OK, "{fetched}");
    normalize_semantics(&mut fetched["data"]);
    assert_eq!(
        fetched,
        fixture("get-label-semantics-response.v1.valid.json")
    );

    let (status, mut explained) = get_json(
        app,
        "/api/v1/boards/fixture/labels/atoms/la_51d68accaaa974f4/explain",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{explained}");
    normalize_label_atom(&mut explained["data"]["atom"]);
    normalize_semantics(&mut explained["data"]["current_semantics"]);
    assert_eq!(
        explained,
        fixture("explain-label-atom-response.v1.valid.json")
    );
    Ok(())
}

#[tokio::test]
async fn generated_generic_signal_response_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture".into(),
            name: "fixture".into(),
            description: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "fixture",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("fixture"),
    )?;
    let task_id = task.id.clone();
    let task_ref = task.task_ref.clone();
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute(
        "INSERT INTO signal_observations(
            id, board_id, task_id, task_ref_snapshot, actor, agent_type, source,
            evidence_json, created_at
         ) VALUES ('obs_fixture', ?1, ?2, ?3, 'fixture', NULL,
                   'fixture', '{}', 1)",
        (&task.board_id, &task_id, &task_ref),
    )?;
    conn.execute(
        "INSERT INTO signals(
            id, board_id, observation_id, kind, title, summary, severity, status,
            dedupe_key, created_at, updated_at
         ) VALUES ('sig_fixture', ?1, 'obs_fixture', 'fixture', 'fixture', 'fixture',
                   'info', 'open', 'fixture', 1, 1)",
        [&task.board_id],
    )?;
    drop(conn);

    let (status, mut response) = get_json(test.router(), "/api/v1/signals/sig_fixture").await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    response["data"]["board_id"] = json!("fixture");
    response["data"]["observation"]["board_id"] = json!("fixture");
    response["data"]["observation"]["task_id"] = json!("t_fixture");
    assert_eq!(response, fixture("get-signal-response.v1.valid.json"));
    Ok(())
}

fn normalize_ontology_observation(data: &mut Value) {
    data["id"] = json!("lor_fixture");
    data["board_id"] = json!("fixture");
    data["task_id"] = json!("t_fixture");
    data["suggest_input_hash"] = json!("fixture");
    data["capture_fingerprint"] = json!("fixture");
    data["created_at"] = json!(1);
    if data["task_snapshot"].is_object() {
        data["task_snapshot"]["id"] = json!("t_fixture");
        data["task_snapshot"]["board_id"] = json!("fixture");
        data["task_snapshot"]["content_hash"] = json!("fixture");
        data["task_snapshot"]["created_at"] = json!(1);
        data["task_snapshot"]["updated_at"] = json!(1);
    }
    for signal in data["signals"].as_array_mut().into_iter().flatten() {
        normalize_ontology_signal(signal);
    }
}

fn normalize_ontology_signal(signal: &mut Value) {
    signal["id"] = json!("los_fixture");
    signal["observation_id"] = json!("lor_fixture");
    signal["board_id"] = json!("fixture");
    signal["target_label_id"] = json!("l_fixture");
    signal["candidate_content_hash"] = json!("fixture");
    signal["created_at"] = json!(1);
    signal["updated_at"] = json!(1);
}

fn normalize_label_atom(atom: &mut Value) {
    atom["id"] = json!("la_51d68accaaa974f4");
    atom["label_id"] = json!("l_fixture");
    atom["board_id"] = json!("fixture");
    atom["content_hash"] = json!("51d68accaaa974f4");
    atom["created_at"] = json!(1);
    atom["updated_at"] = json!(1);
}

#[tokio::test]
async fn generated_ontology_observation_responses_are_produced_by_real_router() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture".into(),
            name: "fixture".into(),
            description: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "fixture",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("fixture"),
    )?;
    let _label = kanban_sqlite::api::create_label(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateLabel {
            name: "fixture".into(),
            color: None,
        },
    )?;

    let (status, mut recorded) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        fixture("record-label-ontology-observation-body.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{recorded}");
    let signal_id = recorded["data"]["signals"][0]["id"]
        .as_str()
        .context("ontology signal id")?
        .to_owned();
    normalize_ontology_observation(&mut recorded["data"]);
    assert_eq!(
        recorded,
        fixture("record-label-ontology-observation-response.v1.valid.json")
    );

    let (status, mut fetched) = get_json(
        test.router(),
        &format!("/api/v1/label-ontology/signals/{signal_id}"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{fetched}");
    normalize_ontology_observation(&mut fetched["data"]["observation"]);
    normalize_ontology_signal(&mut fetched["data"]["signal"]);
    assert_eq!(
        fetched,
        fixture("get-label-ontology-signal-response.v1.valid.json")
    );
    Ok(())
}

fn normalize_action(response: &mut Value, id: &str) {
    response["id"] = json!(id);
    response["board_id"] = json!("fixture");
    response["created_at"] = json!(1);
}

fn assert_exact_string_array(
    value: &Value,
    expected: &[&str],
    context: &str,
) -> anyhow::Result<()> {
    let actual = value
        .as_array()
        .with_context(|| format!("{context} must be an array"))?
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
        .with_context(|| format!("{context} must contain only strings"))?;
    assert_eq!(actual, expected, "{context} provenance drift");
    Ok(())
}

#[tokio::test]
async fn generated_ontology_action_responses_are_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture".into(),
            name: "fixture".into(),
            description: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "fixture",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("fixture"),
    )?;
    let label = kanban_sqlite::api::create_label(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateLabel {
            name: "fixture".into(),
            color: None,
        },
    )?;
    let (status, observation) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        fixture("record-label-ontology-observation-body.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{observation}");
    let signal_id = observation["data"]["signals"][0]["id"]
        .as_str()
        .context("action source signal id")?
        .to_owned();
    let mut action_request = fixture("create-label-ontology-action-request.v1.valid.json");
    action_request["signal_ids"] = json!([signal_id]);
    let (status, mut response) = post_json(
        test.router(),
        "/api/v1/boards/fixture/label-ontology/actions",
        action_request,
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_exact_string_array(
        &response["data"]["signal_ids"],
        &[&signal_id],
        "create action signal_ids",
    )?;
    normalize_action(&mut response["data"], "loa_fixture");
    response["data"]["signal_ids"] = json!(["los_fixture"]);
    assert_eq!(
        response,
        fixture("create-label-ontology-action-response.v1.valid.json")
    );

    let mut apply_request = fixture("apply-label-ontology-atom-request.v1.valid.json");
    apply_request["signal_ids"] = json!([signal_id]);
    let (status, mut response) = post_json(
        test.router(),
        "/api/v1/boards/fixture/label-ontology/apply/atom",
        apply_request,
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_exact_string_array(
        &response["data"]["signal_ids"],
        &[&signal_id],
        "apply action signal_ids",
    )?;
    assert_eq!(response["data"]["target_label_id"], label.id);
    let apply_action_id = response["data"]["id"]
        .as_str()
        .context("apply action id")?
        .to_owned();
    let apply_result_atom_id = response["data"]["result_atom_id"]
        .as_str()
        .context("apply result atom id")?
        .to_owned();
    let apply_result_atom_content_hash = response["data"]["result_atom_content_hash"]
        .as_str()
        .context("apply result atom content hash")?
        .to_owned();
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    conn.execute(
        "UPDATE label_ontology_action_atom_effects SET action_id='loa_target_fixture' WHERE action_id=?1",
        [&apply_action_id],
    )?;
    conn.execute(
        "UPDATE label_ontology_action_signals SET action_id='loa_target_fixture' WHERE action_id=?1",
        [&apply_action_id],
    )?;
    conn.execute(
        "UPDATE label_ontology_actions SET parent_action_id='loa_target_fixture' WHERE parent_action_id=?1",
        [&apply_action_id],
    )?;
    conn.execute(
        "UPDATE label_ontology_actions SET id='loa_target_fixture' WHERE id=?1",
        [&apply_action_id],
    )?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    drop(conn);
    assert_fixture_foreign_keys_valid(&test, "response fixture action rename")?;
    normalize_action(&mut response["data"], "loa_target_fixture");
    response["data"]["target_label_id"] = json!("l_fixture");
    response["data"]["result_atom_id"] = json!("la_fixture");
    response["data"]["result_atom_content_hash"] = json!("fixture");
    response["data"]["canonical_before_hash"] = json!("fixture");
    response["data"]["canonical_after_hash"] = json!("fixture");
    response["data"]["signal_ids"] = json!(["los_fixture"]);
    response["data"]["change"]["label"]["id"] = json!("l_fixture");
    response["data"]["change"]["added_atom"]["id"] = json!("la_fixture");
    response["data"]["change"]["added_atom"]["content_hash"] = json!("fixture");
    response["data"]["change"]["before"]["label_id"] = json!("l_fixture");
    response["data"]["change"]["after"]["label_id"] = json!("l_fixture");
    assert_eq!(
        response,
        fixture("apply-label-ontology-atom-response.v1.valid.json")
    );

    let (status, mut validated) = post_json(
        test.router(),
        "/api/v1/boards/fixture/label-ontology/validate",
        fixture("validate-label-ontology-action-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{validated}");
    assert_exact_string_array(
        &validated["data"]["signal_ids"],
        &[&signal_id],
        "validation signal_ids",
    )?;
    let raw_validation_cases = validated["data"]["validation"]["cases"]
        .as_array()
        .context("validation cases")?;
    assert_eq!(raw_validation_cases.len(), 1, "validation case cardinality");
    let raw_validation_case = &raw_validation_cases[0];
    assert_eq!(raw_validation_case["signal_id"], signal_id);
    assert_eq!(raw_validation_case["task_id"], task.id);
    assert_eq!(raw_validation_case["task_ref_snapshot"], task.task_ref);
    assert_eq!(raw_validation_case["target_label_id"], label.id);
    assert_eq!(raw_validation_case["result_atom_id"], apply_result_atom_id);
    assert_eq!(
        raw_validation_case["result_atom_content_hash"],
        apply_result_atom_content_hash
    );
    assert!(raw_validation_case["before"].is_object());
    assert!(raw_validation_case["after"].is_object());
    assert_eq!(validated["data"]["validation"]["summary"]["case_count"], 1);
    normalize_action(&mut validated["data"], "loa_fixture");
    validated["data"]["target_label_id"] = json!("l_fixture");
    validated["data"]["result_atom_id"] = json!("la_fixture");
    validated["data"]["result_atom_content_hash"] = json!("fixture");
    validated["data"]["canonical_before_hash"] = json!("fixture");
    validated["data"]["canonical_after_hash"] = json!("fixture");
    validated["data"]["signal_ids"] = json!(["los_fixture"]);
    let validation_case = &mut validated["data"]["validation"]["cases"][0];
    validation_case["current_suggest_input_hash"] = json!("fixture");
    validation_case["current_task_snapshot_hash"] = json!("fixture");
    validation_case["result_atom_content_hash"] = json!("fixture");
    validation_case["result_atom_id"] = json!("la_fixture");
    validation_case["signal_id"] = json!("los_fixture");
    validation_case["suggest_input_hash"] = json!("fixture");
    validation_case["target_label_id"] = json!("l_fixture");
    validation_case["task_id"] = json!("t_fixture");
    validation_case["task_snapshot_hash"] = json!("fixture");
    assert_eq!(
        validated,
        fixture("validate-label-ontology-action-response.v1.valid.json")
    );

    let (status, mut reverted) = post_json(
        test.router(),
        "/api/v1/boards/fixture/label-ontology/revert",
        fixture("revert-label-ontology-mutation-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{reverted}");
    assert_exact_string_array(
        &reverted["data"]["signal_ids"],
        &[&signal_id],
        "revert signal_ids",
    )?;
    normalize_action(&mut reverted["data"], "loa_fixture");
    reverted["data"]["target_label_id"] = json!("l_fixture");
    reverted["data"]["result_atom_id"] = json!("la_fixture");
    reverted["data"]["result_atom_content_hash"] = json!("fixture");
    reverted["data"]["canonical_before_hash"] = json!("fixture");
    reverted["data"]["canonical_after_hash"] = json!("fixture");
    reverted["data"]["signal_ids"] = json!(["los_fixture"]);
    reverted["data"]["change"]["label"]["id"] = json!("l_fixture");
    reverted["data"]["change"]["reverted_canonical_before_hash"] = json!("fixture");
    reverted["data"]["change"]["reverted_canonical_after_hash"] = json!("fixture");
    reverted["data"]["change"]["before_revert"]["label_id"] = json!("l_fixture");
    reverted["data"]["change"]["after_revert"]["label_id"] = json!("l_fixture");
    assert_eq!(
        reverted,
        fixture("revert-label-ontology-mutation-response.v1.valid.json")
    );
    Ok(())
}

fn fixture_task_app() -> anyhow::Result<(TestApp, String)> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture".into(),
            name: "fixture".into(),
            description: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "fixture",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("fixture"),
    )?;
    Ok((test, task.id))
}

fn normalize_bootstrap_response(response: &mut Value) {
    let task = &mut response["data"]["task"];
    task["id"] = json!("t_fixture");
    task["board_id"] = json!("fixture");
    task["created_at"] = json!(1);
    task["updated_at"] = json!(1);
    for label in task["labels"].as_array_mut().into_iter().flatten() {
        normalize_label(label);
    }
    normalize_semantics(&mut response["data"]["semantics"]);
}

#[tokio::test]
async fn generated_task_label_responses_are_produced_by_real_router() -> anyhow::Result<()> {
    let (test, task_id) = fixture_task_app()?;
    let (status, mut response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{task_id}/labels/bootstrap"),
        fixture("bootstrap-task-label-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    normalize_bootstrap_response(&mut response);
    assert_eq!(
        response,
        fixture("bootstrap-task-label-response.v1.valid.json")
    );

    let (test, task_id) = fixture_task_app()?;
    let (status, mut response) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{task_id}/labels/suggestions"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    response["data"]["task_id"] = json!("t_fixture");
    response["data"]["board_id"] = json!("fixture");
    assert_eq!(
        response,
        fixture("suggest-task-labels-response.v1.valid.json")
    );

    let (test, task_id) = fixture_task_app()?;
    let (status, mut response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{task_id}/label-proposals"),
        fixture("propose-task-label-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    response["data"]["task_id"] = json!("t_fixture");
    response["data"]["board_id"] = json!("fixture");
    assert_eq!(
        response,
        fixture("propose-task-label-response.v1.valid.json")
    );
    Ok(())
}

fn write_atom_index_protocol_helper(test: &TestApp) -> anyhow::Result<std::path::PathBuf> {
    let path = test.dir_path().join("atom-index-contract-helper");
    std::fs::write(
        &path,
        r#"#!/usr/bin/env python3
import json
import os
import sys

with open(sys.argv[0] + ".args.jsonl", "a", encoding="utf-8") as log:
    log.write(json.dumps(sys.argv[1:]) + "\n")

command = next(
    (arg for arg in sys.argv[1:] if arg in {"label-atoms-status", "rebuild-label-atoms", "query-label-atoms"}),
    None,
)
if command == "query-label-atoms":
    payload = []
else:
    payload = {
        "backend": "fixture",
        "enabled": False,
        "message": "fixture",
        "diagnostics": [],
        "dirty": None,
        "board_dirty": None,
        "generation": 1,
    }
print(json.dumps({"protocol": "kanban-derived-helper.v1", "payload_json": json.dumps(payload)}))
"#,
    )?;
    let mut permissions = std::fs::metadata(&path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions)?;
    Ok(path)
}

#[tokio::test]
async fn generated_atom_index_responses_are_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "fixture".into(),
            name: "fixture".into(),
            description: None,
        },
    )?;
    let helper = write_atom_index_protocol_helper(&test)?;
    let app =
        build_router(AppState::new(test.db_path(), "fixture").with_vector_helper_path(helper));

    let (status, response) = get_json(
        app.clone(),
        "/api/v1/boards/fixture/labels/atom-index/status",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response,
        fixture("label-atom-index-status-response.v1.valid.json")
    );

    let (status, response) = post_json(
        app.clone(),
        "/api/v1/boards/fixture/labels/atom-index/rebuild",
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response,
        fixture("rebuild-label-atom-index-response.v1.valid.json")
    );

    let (status, response) = get_json(
        app,
        "/api/v1/boards/fixture/labels/atom-index/query?q=fixture",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response,
        fixture("query-label-atom-index-response.v1.valid.json")
    );
    Ok(())
}

fn seed_fixture_proposal(test: &TestApp, task_id: &str) -> anyhow::Result<()> {
    let conn = kanban_test_support::connect_file(test.db_path())?;
    let board_id: String =
        conn.query_row("SELECT board_id FROM tasks WHERE id=?1", [task_id], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO label_semantic_proposals(
            id, board_id, task_id, status, name, description, applies_when, excludes_when,
            positive_examples, negative_examples, heuristic_coverage,
            heuristic_coverage_cosine, heuristic_residual_norm, diagnostics_json,
            created_by, created_at, updated_at
         ) VALUES (
            'lp_fixture', ?1, ?2, 'proposed', 'fixture', NULL, '[]', '[]', '[]', '[]',
            0.0, 0.0, 1.0, '[]', 'fixture', 1, 1
         )",
        (&board_id, task_id),
    )?;
    Ok(())
}

fn normalize_proposal(response: &mut Value) {
    response["board_id"] = json!("fixture");
    response["task_id"] = json!("t_fixture");
    if response["resolved_label_id"].is_string() {
        response["resolved_label_id"] = json!("l_fixture");
    }
    response["created_at"] = json!(1);
    response["updated_at"] = json!(1);
    if response["decided_at"].is_number() {
        response["decided_at"] = json!(1);
    }
}

#[tokio::test]
async fn generated_proposal_responses_are_produced_by_real_router() -> anyhow::Result<()> {
    let (test, task_id) = fixture_task_app()?;
    seed_fixture_proposal(&test, &task_id)?;
    let (status, mut response) =
        get_json(test.router(), "/api/v1/label-proposals/lp_fixture").await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    normalize_proposal(&mut response["data"]);
    assert_eq!(
        response,
        fixture("get-label-proposal-response.v1.valid.json")
    );

    let (test, task_id) = fixture_task_app()?;
    seed_fixture_proposal(&test, &task_id)?;
    let (status, mut response) = post_json(
        test.router(),
        "/api/v1/label-proposals/lp_fixture/accept",
        fixture("accept-label-proposal-body.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    normalize_proposal(&mut response["data"]);
    assert_eq!(
        response,
        fixture("accept-label-proposal-response.v1.valid.json")
    );

    let (test, task_id) = fixture_task_app()?;
    seed_fixture_proposal(&test, &task_id)?;
    let (status, mut response) = post_json(
        test.router(),
        "/api/v1/label-proposals/lp_fixture/reject",
        fixture("reject-label-proposal-body.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    normalize_proposal(&mut response["data"]);
    assert_eq!(
        response,
        fixture("reject-label-proposal-response.v1.valid.json")
    );
    Ok(())
}

macro_rules! response_root_case {
    ($registered:expr, $id:literal, $ty:ty, $fixture:literal) => {{
        $registered.push($id);
        let value = fixture($fixture);
        let root: $ty = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(root).unwrap(), value, $fixture);
    }};
}

#[test]
fn api_generated_response_fixtures_are_consumed_by_contract_roots() {
    let mut registered = Vec::new();
    response_root_case!(
        registered,
        "api.list-board-labels.response",
        contract::ListBoardLabelsResponse,
        "list-board-labels-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.create-board-label.response",
        contract::CreateBoardLabelResponse,
        "create-board-label-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.list-label-semantics.response",
        contract::ListLabelSemanticsResponse,
        "list-label-semantics-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.get-label-semantics.response",
        contract::GetLabelSemanticsResponse,
        "get-label-semantics-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.upsert-label-semantics.response",
        contract::UpsertLabelSemanticsResponse,
        "upsert-label-semantics-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.list-label-atoms.response",
        contract::ListLabelAtomsResponse,
        "list-label-atoms-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.explain-label-atom.response",
        contract::ExplainLabelAtomResponse,
        "explain-label-atom-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.label-atom-index-status.response",
        contract::LabelAtomIndexStatusResponse,
        "label-atom-index-status-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.rebuild-label-atom-index.response",
        contract::RebuildLabelAtomIndexResponse,
        "rebuild-label-atom-index-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.query-label-atom-index.response",
        contract::QueryLabelAtomIndexResponse,
        "query-label-atom-index-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.list-signals.response",
        contract::ListSignalsResponse,
        "list-signals-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.review-signals.response",
        contract::ReviewSignalsResponse,
        "review-signals-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.get-signal.response",
        contract::GetSignalResponse,
        "get-signal-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.bootstrap-task-label.response",
        contract::BootstrapTaskLabelResponse,
        "bootstrap-task-label-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.suggest-task-labels.response",
        contract::SuggestTaskLabelsResponse,
        "suggest-task-labels-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.list-task-label-proposals.response",
        contract::ListTaskLabelProposalsResponse,
        "list-task-label-proposals-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.propose-task-label.response",
        contract::ProposeTaskLabelResponse,
        "propose-task-label-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.record-label-ontology-observation.response",
        contract::RecordLabelOntologyObservationResponse,
        "record-label-ontology-observation-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.review-label-ontology.response",
        contract::ReviewLabelOntologyResponse,
        "review-label-ontology-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.create-label-ontology-action.response",
        contract::LabelOntologyActionResponse,
        "create-label-ontology-action-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.apply-label-ontology-atom.response",
        contract::LabelOntologyActionResponse,
        "apply-label-ontology-atom-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.revert-label-ontology-mutation.response",
        contract::LabelOntologyActionResponse,
        "revert-label-ontology-mutation-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.validate-label-ontology-action.response",
        contract::LabelOntologyActionResponse,
        "validate-label-ontology-action-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.get-label-ontology-signal.response",
        contract::GetLabelOntologySignalResponse,
        "get-label-ontology-signal-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.get-label-proposal.response",
        contract::GetLabelProposalResponse,
        "get-label-proposal-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.accept-label-proposal.response",
        contract::LabelProposalDecisionResponse,
        "accept-label-proposal-response.v1.valid.json"
    );
    response_root_case!(
        registered,
        "api.reject-label-proposal.response",
        contract::LabelProposalDecisionResponse,
        "reject-label-proposal-response.v1.valid.json"
    );

    let expected = RESPONSE_PRODUCER_CASES
        .iter()
        .map(|(id, _)| *id)
        .collect::<std::collections::BTreeSet<_>>();
    let covered = registered
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(covered, expected);
    assert_eq!(
        registered.len(),
        covered.len(),
        "duplicate contract registration"
    );
    for id in &registered {
        let root = contract::operation_inventory()
            .iter()
            .find(|root| root.id == *id)
            .unwrap_or_else(|| panic!("registered response contract is missing: {id}"));
        assert_eq!(root.surface, contract::ContractSurface::Api, "{id}");
        assert_eq!(
            root.direction,
            contract::ContractDirection::Serialize,
            "{id}"
        );
        assert!(
            matches!(
                root.migration,
                contract::MigrationState::Generated | contract::MigrationState::Adopted
            ),
            "{id}: {:?}",
            root.migration
        );
    }
}

#[test]
fn api_generated_adoption_target_inventory_is_frozen() {
    let inventory = contract::operation_inventory();
    let registered_requests = REQUEST_CONTRACT_IDS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let registered_responses = RESPONSE_PRODUCER_CASES
        .iter()
        .map(|(id, _)| *id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(REQUEST_CONTRACT_IDS.len(), registered_requests.len());
    assert_eq!(
        RESPONSE_PRODUCER_CASES.len(),
        registered_responses.len(),
        "duplicate response producer registration"
    );
    assert!(registered_requests.is_disjoint(&registered_responses));
    let adoption_closure = registered_requests
        .union(&registered_responses)
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(adoption_closure.len(), 75);

    for id in &adoption_closure {
        let root = inventory
            .iter()
            .find(|root| root.id == *id)
            .unwrap_or_else(|| panic!("API adoption closure contract is missing: {id}"));
        assert_eq!(root.surface, contract::ContractSurface::Api, "{id}");
        assert!(
            matches!(
                root.migration,
                contract::MigrationState::Generated | contract::MigrationState::Adopted
            ),
            "{id}: {:?}",
            root.migration
        );
    }

    let actual_generated = inventory
        .iter()
        .filter(|root| {
            root.surface == contract::ContractSurface::Api
                && root.migration == contract::MigrationState::Generated
        })
        .map(|root| root.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(actual_generated.is_empty());
}
