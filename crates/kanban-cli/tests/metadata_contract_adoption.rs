use std::collections::BTreeMap;

use kanban_contract::structured_metadata::{
    JsonArray, JsonObject, LabelProposalCandidateMetadataInput, OntologyActorMetadataInput,
    OntologyActorType, OntologyCandidateAtomMetadataInput, OntologyCandidateKind,
    OntologyCandidatePolarity, OntologyProposedAction, OntologyRecordMetadataInput,
    OntologySignalKind, OntologySignalMetadataInput, OntologySuggestState, PositiveRank,
    SignalCommentMetadataInput, SignalLinkMetadataOutput, SignalLinkStatus, SignalLinkType,
    SignalRecordMetadataInput, UnitInterval,
};
use kanban_contract::{DecisionMetadata, DecisionOption};
use serde::Serialize;
use serde_json::{Value, json};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/fixtures/metadata"
);

fn assert_produces_fixture<T: Serialize>(actual: T, fixture: &str) -> anyhow::Result<()> {
    let actual = serde_json::to_value(actual)?;
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{FIXTURES}/{fixture}"))?)?;
    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn metadata_decision_input_fixture_is_produced_by_cli_contract_dto() -> anyhow::Result<()> {
    let actual = DecisionMetadata {
        options: vec![DecisionOption {
            slug: "comment-metadata".into(),
            title: "Use comment metadata".into(),
            detail: "Store structured decision data in task comment metadata.".into(),
            extensions: BTreeMap::from([("owner".into(), json!("adapter"))]),
        }],
        selected: "comment-metadata".into(),
        reason: "Keep structured decisions near the task discussion.".into(),
        risk: Some("Schema and service semantics can drift.".into()),
        verification: Some("Schema fixtures and service tests cover separate layers.".into()),
        extensions: BTreeMap::from([("ticket".into(), json!("kanban-tool#438"))]),
    };
    assert_produces_fixture(actual, "decision.v1.valid.json")
}

#[test]
fn metadata_signal_record_input_fixture_is_produced_by_cli_contract_dto() -> anyhow::Result<()> {
    let actual = SignalRecordMetadataInput {
        kind: "agent_cli_failure".into(),
        title: "Bad flag".into(),
        summary: "comment add rejected body-file".into(),
        severity: Some("medium".into()),
        task_ref: Some("default#1".into()),
        task_id: None,
        run_id: None,
        comment_id: None,
        actor: Some("codex".into()),
        agent_type: Some("executor".into()),
        dedupe_key: Some("cli-body-file".into()),
        source: Some("fixture".into()),
        evidence: Some(JsonObject(BTreeMap::from([(
            "stderr".into(),
            json!("unexpected argument"),
        )]))),
        comment: Some(SignalCommentMetadataInput {
            body: Some("Signal backlink body".into()),
        }),
    };
    assert_produces_fixture(actual, "signal-record-input.v1.valid.json")
}

#[test]
fn metadata_label_proposal_candidate_input_fixture_is_produced_by_cli_contract_dto()
-> anyhow::Result<()> {
    let actual = LabelProposalCandidateMetadataInput {
        name: "workflow".into(),
        description: Some("Workflow classification".into()),
        applies_when: vec!["classifies execution flow".into()],
        excludes_when: vec!["UI-only polish".into()],
        positive_examples: vec!["triage work queue".into()],
        negative_examples: vec!["CSS tweak".into()],
    };
    assert_produces_fixture(actual, "label-proposal-candidate-input.v1.valid.json")
}

#[test]
fn metadata_ontology_record_input_fixture_is_produced_by_cli_contract_dto() -> anyhow::Result<()> {
    let actual = OntologyRecordMetadataInput {
        actor: OntologyActorMetadataInput {
            name: "label-agent".into(),
            actor_type: OntologyActorType::Agent,
            agent_type: Some("local".into()),
        },
        agent_candidates: Some(JsonArray(vec![json!({"label": "cli", "confidence": 0.92})])),
        suggestion_snapshot: Some(JsonObject(BTreeMap::from([(
            "selected_labels".into(),
            json!([]),
        )]))),
        final_decision: Some(JsonObject(BTreeMap::from([(
            "accepted_labels".into(),
            json!(["cli"]),
        )]))),
        suggest_coverage: Some(UnitInterval::new(0.61).expect("valid fixture coverage")),
        suggest_coverage_cosine: Some(
            UnitInterval::new(0.74).expect("valid fixture cosine coverage"),
        ),
        suggest_residual_norm: Some(UnitInterval::new(0.39).expect("valid fixture residual")),
        suggest_needs_new_label: Some(false),
        suggest_degraded: Some(false),
        diagnostics: Some(JsonArray(vec![])),
        capture_fingerprint: Some("metadata-fixture".into()),
        signals: vec![OntologySignalMetadataInput {
            kind: OntologySignalKind::FalseNegative,
            target_label_ref: Some("cli".into()),
            related_labels: Some(JsonArray(vec![])),
            proposed_action: OntologyProposedAction::AddPositiveAtom,
            candidate_atom: Some(OntologyCandidateAtomMetadataInput {
                polarity: OntologyCandidatePolarity::Positive,
                kind: OntologyCandidateKind::AppliesWhen,
                text: "extends CLI subcommands, arguments, help output, or JSON behavior".into(),
            }),
            proposed_label_name: None,
            proposal: Some(JsonObject(BTreeMap::new())),
            agent_selected: Some(true),
            suggest_state: Some(OntologySuggestState::Candidate),
            suggest_score: Some(UnitInterval::new(0.08).expect("valid fixture score")),
            suggest_rank: Some(PositiveRank::new(4).expect("valid fixture rank")),
            final_selected: Some(true),
            rationale: "The task expands the CLI surface although suggest scored cli weakly."
                .into(),
            confidence: Some(UnitInterval::new(0.91).expect("valid fixture confidence")),
            signal_key: Some("cli-false-negative".into()),
        }],
    };
    assert_produces_fixture(actual, "ontology-record-input.v1.valid.json")
}

#[test]
fn metadata_ontology_validation_evidence_input_fixture_is_produced_by_cli_contract_dto()
-> anyhow::Result<()> {
    let actual = JsonObject(BTreeMap::from([
        ("evidence_type".into(), json!("external_attestation")),
        ("source".into(), json!("fixture")),
        (
            "checks".into(),
            json!([{"name": "candidate remains selected", "passed": true}]),
        ),
    ]));
    assert_produces_fixture(actual, "ontology-validation-evidence-input.v1.valid.json")
}

#[test]
fn metadata_signal_link_output_fixture_is_consumed_by_cli_contract_dto() -> anyhow::Result<()> {
    let valid = std::fs::read_to_string(format!("{FIXTURES}/signal-link-output.v1.valid.json"))?;
    let value: SignalLinkMetadataOutput = serde_json::from_str(&valid)?;
    assert_eq!(value.link_type, SignalLinkType::SignalLink);
    assert_eq!(value.signal_status, SignalLinkStatus::Open);

    let invalid =
        std::fs::read_to_string(format!("{FIXTURES}/signal-link-output.v1.invalid.json"))?;
    assert!(serde_json::from_str::<SignalLinkMetadataOutput>(&invalid).is_err());
    Ok(())
}
