//! 公共 adapter 共享的自然 JSON 结构化元数据契约。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct UnitInterval(#[cfg_attr(feature = "schema", schemars(range(min = 0.0, max = 1.0)))] f64);

impl UnitInterval {
    pub fn new(value: f64) -> Result<Self, String> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err("值必须是有限数，且范围为 0..=1".to_owned())
        }
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for UnitInterval {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct PositiveRank(#[cfg_attr(feature = "schema", schemars(range(min = 1)))] i64);

impl PositiveRank {
    pub fn new(value: i64) -> Result<Self, String> {
        if value >= 1 {
            Ok(Self(value))
        } else {
            Err("rank 必须至少为 1".to_owned())
        }
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for PositiveRank {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(i64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct JsonObject(pub BTreeMap<String, Value>);

impl JsonObject {
    pub fn from_json_str(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct JsonArray(pub Vec<Value>);

impl JsonArray {
    pub fn from_json_str(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }
}

pub type SignalEvidence = JsonObject;
pub type OntologyTaskSnapshot = JsonObject;
pub type OntologySuggestionSnapshot = JsonObject;
pub type OntologyDecisionSnapshot = JsonObject;
pub type OntologyProposal = JsonObject;
pub type OntologyChange = JsonObject;
pub type OntologyValidation = JsonObject;
pub type OntologyCandidates = JsonArray;
pub type OntologyDiagnostics = JsonArray;
pub type RelatedLabels = JsonArray;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SignalLinkType {
    SignalLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SignalLinkStatus {
    Open,
    Confirmed,
    Rejected,
    Superseded,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OntologyActorType {
    User,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OntologySignalKind {
    FalseNegative,
    FalsePositive,
    VocabularyGap,
    NameIssue,
    BoundaryIssue,
    StructureIssue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OntologyProposedAction {
    Observe,
    AddPositiveAtom,
    AddNegativeAtom,
    UpdateSemantics,
    BootstrapLabel,
    RenameLabel,
    SplitLabel,
    MergeLabels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OntologyCandidatePolarity {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OntologyCandidateKind {
    AppliesWhen,
    PositiveExample,
    ExcludesWhen,
    NegativeExample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OntologySuggestState {
    Selected,
    Candidate,
    Absent,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SignalCommentMetadataInput {
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SignalRecordMetadataInput {
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub severity: Option<String>,
    pub task_ref: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub comment_id: Option<String>,
    pub actor: Option<String>,
    pub agent_type: Option<String>,
    pub dedupe_key: Option<String>,
    pub source: Option<String>,
    pub evidence: Option<SignalEvidence>,
    pub comment: Option<SignalCommentMetadataInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SignalLinkMetadataOutput {
    #[serde(rename = "type")]
    pub link_type: SignalLinkType,
    pub signal_id: String,
    pub observation_id: String,
    pub signal_kind: String,
    pub signal_status: SignalLinkStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LabelProposalCandidateMetadataInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub applies_when: Vec<String>,
    #[serde(default)]
    pub excludes_when: Vec<String>,
    #[serde(default)]
    pub positive_examples: Vec<String>,
    #[serde(default)]
    pub negative_examples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OntologyActorMetadataInput {
    pub name: String,
    #[serde(rename = "type")]
    pub actor_type: OntologyActorType,
    pub agent_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OntologyCandidateAtomMetadataInput {
    pub polarity: OntologyCandidatePolarity,
    pub kind: OntologyCandidateKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OntologySignalMetadataInput {
    pub kind: OntologySignalKind,
    #[serde(default)]
    pub target_label_ref: Option<String>,
    #[serde(default)]
    pub related_labels: Option<RelatedLabels>,
    pub proposed_action: OntologyProposedAction,
    #[serde(default)]
    pub candidate_atom: Option<OntologyCandidateAtomMetadataInput>,
    #[serde(default)]
    pub proposed_label_name: Option<String>,
    #[serde(default)]
    pub proposal: Option<OntologyProposal>,
    #[serde(default)]
    pub agent_selected: Option<bool>,
    #[serde(default)]
    pub suggest_state: Option<OntologySuggestState>,
    #[serde(default)]
    pub suggest_score: Option<UnitInterval>,
    #[serde(default)]
    pub suggest_rank: Option<PositiveRank>,
    #[serde(default)]
    pub final_selected: Option<bool>,
    pub rationale: String,
    #[serde(default)]
    pub confidence: Option<UnitInterval>,
    #[serde(default)]
    pub signal_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct OntologyRecordMetadataInput {
    pub actor: OntologyActorMetadataInput,
    #[serde(default)]
    pub agent_candidates: Option<OntologyCandidates>,
    #[serde(default)]
    pub suggestion_snapshot: Option<OntologySuggestionSnapshot>,
    #[serde(default)]
    pub final_decision: Option<OntologyDecisionSnapshot>,
    #[serde(default)]
    pub suggest_coverage: Option<UnitInterval>,
    #[serde(default)]
    pub suggest_coverage_cosine: Option<UnitInterval>,
    #[serde(default)]
    pub suggest_residual_norm: Option<UnitInterval>,
    #[serde(default)]
    pub suggest_needs_new_label: Option<bool>,
    #[serde(default)]
    pub suggest_degraded: Option<bool>,
    #[serde(default)]
    pub diagnostics: Option<OntologyDiagnostics>,
    #[serde(default)]
    pub capture_fingerprint: Option<String>,
    #[serde(default)]
    pub signals: Vec<OntologySignalMetadataInput>,
}

pub type OntologyValidationEvidenceMetadataInput = JsonObject;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn metadata_contract_fixtures_bind_to_exact_dtos() {
        let decision_valid =
            include_str!("../../../schemas/fixtures/metadata/decision.v1.valid.json");
        let decision_invalid =
            include_str!("../../../schemas/fixtures/metadata/decision.v1.invalid.json");
        assert!(serde_json::from_str::<crate::DecisionMetadata>(decision_valid).is_ok());
        assert!(serde_json::from_str::<crate::DecisionMetadata>(decision_invalid).is_err());

        assert_fixture_pair::<SignalRecordMetadataInput>(
            include_str!("../../../schemas/fixtures/metadata/signal-record-input.v1.valid.json"),
            include_str!("../../../schemas/fixtures/metadata/signal-record-input.v1.invalid.json"),
        );
        assert_fixture_pair::<SignalLinkMetadataOutput>(
            include_str!("../../../schemas/fixtures/metadata/signal-link-output.v1.valid.json"),
            include_str!("../../../schemas/fixtures/metadata/signal-link-output.v1.invalid.json"),
        );
        assert_fixture_pair::<LabelProposalCandidateMetadataInput>(
            include_str!(
                "../../../schemas/fixtures/metadata/label-proposal-candidate-input.v1.valid.json"
            ),
            include_str!(
                "../../../schemas/fixtures/metadata/label-proposal-candidate-input.v1.invalid.json"
            ),
        );
        assert_fixture_pair::<OntologyRecordMetadataInput>(
            include_str!("../../../schemas/fixtures/metadata/ontology-record-input.v1.valid.json"),
            include_str!(
                "../../../schemas/fixtures/metadata/ontology-record-input.v1.invalid.json"
            ),
        );
        assert_fixture_pair::<OntologyValidationEvidenceMetadataInput>(
            include_str!(
                "../../../schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json"
            ),
            include_str!(
                "../../../schemas/fixtures/metadata/ontology-validation-evidence-input.v1.invalid.json"
            ),
        );
    }

    #[test]
    fn ontology_numeric_contracts_accept_boundaries_and_reject_out_of_range_values() {
        assert_eq!(UnitInterval::new(0.0).unwrap().get(), 0.0);
        assert_eq!(UnitInterval::new(1.0).unwrap().get(), 1.0);
        assert_eq!(PositiveRank::new(1).unwrap().get(), 1);
        for invalid in [-0.01, 1.01, f64::INFINITY, f64::NAN] {
            assert!(UnitInterval::new(invalid).is_err());
        }
        assert!(PositiveRank::new(0).is_err());

        let valid: Value = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/metadata/ontology-record-input.v1.valid.json"
        ))
        .unwrap();
        for (path, invalid) in [
            (("signals", "suggest_score"), json!(-0.01)),
            (("signals", "confidence"), json!(1.01)),
            (("record", "suggest_coverage"), json!(-0.01)),
            (("record", "suggest_coverage_cosine"), json!(1.01)),
            (("record", "suggest_residual_norm"), json!(1.01)),
            (("signals", "suggest_rank"), json!(0)),
        ] {
            let mut candidate = valid.clone();
            if path.0 == "signals" {
                candidate["signals"][0][path.1] = invalid;
            } else {
                candidate[path.1] = invalid;
            }
            assert!(
                serde_json::from_value::<OntologyRecordMetadataInput>(candidate).is_err(),
                "out-of-range {} must fail",
                path.1
            );
        }
    }

    #[cfg(feature = "schema")]
    #[test]
    fn ontology_numeric_schemas_publish_exact_ranges() {
        let unit = serde_json::to_value(schemars::schema_for!(UnitInterval)).unwrap();
        assert_eq!(unit["minimum"], json!(0.0));
        assert_eq!(unit["maximum"], json!(1.0));
        let rank = serde_json::to_value(schemars::schema_for!(PositiveRank)).unwrap();
        assert_eq!(rank["minimum"], json!(1));
    }

    fn assert_fixture_pair<T>(valid: &str, invalid: &str)
    where
        T: serde::de::DeserializeOwned,
    {
        assert!(serde_json::from_str::<T>(valid).is_ok());
        assert!(serde_json::from_str::<T>(invalid).is_err());
    }

    #[test]
    fn metadata_decision_input_fixture_is_produced_by_contract_dto() {
        let actual = crate::DecisionMetadata {
            options: vec![crate::DecisionOption {
                slug: "comment-metadata".into(),
                title: "Use comment metadata".into(),
                detail: "Store structured decision data in task comment metadata.".into(),
                extensions: BTreeMap::from([("owner".into(), json!("adapter"))]),
            }],
            selected: "comment-metadata".into(),
            reason: "Keep structured decisions near the task discussion.".into(),
            risk: Some("Schema and service semantics can drift.".into()),
            verification: Some("Schema fixtures and service tests cover separate layers.".into()),
            extensions: BTreeMap::from([("ticket".into(), json!("default#123"))]),
        };
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::from_str::<Value>(include_str!(
                "../../../schemas/fixtures/metadata/decision.v1.valid.json"
            ))
            .unwrap()
        );
    }

    #[test]
    fn metadata_signal_record_input_fixture_is_produced_by_contract_dto() {
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
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::from_str::<Value>(include_str!(
                "../../../schemas/fixtures/metadata/signal-record-input.v1.valid.json"
            ))
            .unwrap()
        );
    }

    #[test]
    fn metadata_signal_link_output_fixture_is_consumed_by_contract_dto() {
        let value: SignalLinkMetadataOutput = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/metadata/signal-link-output.v1.valid.json"
        ))
        .unwrap();
        assert_eq!(value.link_type, SignalLinkType::SignalLink);
        assert_eq!(value.signal_status, SignalLinkStatus::Open);
        assert!(
            serde_json::from_str::<SignalLinkMetadataOutput>(include_str!(
                "../../../schemas/fixtures/metadata/signal-link-output.v1.invalid.json"
            ))
            .is_err()
        );
    }

    #[test]
    fn metadata_label_proposal_candidate_input_fixture_is_produced_by_contract_dto() {
        let actual = LabelProposalCandidateMetadataInput {
            name: "workflow".into(),
            description: Some("Workflow classification".into()),
            applies_when: vec!["classifies execution flow".into()],
            excludes_when: vec!["UI-only polish".into()],
            positive_examples: vec!["triage work queue".into()],
            negative_examples: vec!["CSS tweak".into()],
        };
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::from_str::<Value>(include_str!(
                "../../../schemas/fixtures/metadata/label-proposal-candidate-input.v1.valid.json"
            ))
            .unwrap()
        );
    }

    #[test]
    fn metadata_ontology_record_input_fixture_is_produced_by_contract_dto() {
        let actual: OntologyRecordMetadataInput = serde_json::from_value(json!({
            "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
            "agent_candidates": [{"label": "cli", "confidence": 0.92}],
            "suggestion_snapshot": {"selected_labels": []},
            "final_decision": {"accepted_labels": ["cli"]},
            "suggest_coverage": 0.61,
            "suggest_coverage_cosine": 0.74,
            "suggest_residual_norm": 0.39,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics": [],
            "capture_fingerprint": "metadata-fixture",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "cli",
                "related_labels": [],
                "proposed_action": "add_positive_atom",
                "candidate_atom": {"polarity": "positive", "kind": "applies_when", "text": "extends CLI subcommands, arguments, help output, or JSON behavior"},
                "proposed_label_name": null,
                "proposal": {},
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.08,
                "suggest_rank": 4,
                "final_selected": true,
                "rationale": "The task expands the CLI surface although suggest scored cli weakly.",
                "confidence": 0.91,
                "signal_key": "cli-false-negative"
            }]
        }))
        .unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::from_str::<Value>(include_str!(
                "../../../schemas/fixtures/metadata/ontology-record-input.v1.valid.json"
            ))
            .unwrap()
        );
    }

    #[test]
    fn metadata_ontology_validation_evidence_input_fixture_is_produced_by_contract_dto() {
        let actual = JsonObject(BTreeMap::from([
            ("evidence_type".into(), json!("external_attestation")),
            ("source".into(), json!("fixture")),
            (
                "checks".into(),
                json!([{"name": "candidate remains selected", "passed": true}]),
            ),
        ]));
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::from_str::<Value>(include_str!(
                "../../../schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json"
            ))
            .unwrap()
        );
    }
}
