//! label、signal、ontology 和 setting portable JSONL DTO 的 owner。
//!
//! 这些 DTO 描述 portable JSON 文档，而不是 SQLite row。SQLite 中按文本存储的 JSON
//! 会以自然数组/对象暴露；数据库中每个可空列在 wire 上都必须存在（值为 `null`）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

/// 有意保持 opaque 的 JSON 对象：外层 kind 稳定，provider-specific 成员仍可扩展，
/// 但包含它的 row 和字段名仍是封闭的。
pub type JsonObject = BTreeMap<String, Value>;

/// wire 上必须存在的 key，但其值可以明确为 JSON `null`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct RequiredNullable<T>(pub Option<T>);

fn deserialize_required_nullable<'de, D, T>(
    deserializer: D,
) -> Result<RequiredNullable<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(RequiredNullable)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortableRecord<K, D> {
    #[serde(rename = "type")]
    pub record_type: K,
    pub data: D,
}

macro_rules! record_kind {
    ($kind:ident, $variant:ident, $wire:literal, $row:ident, $input:ident, $output:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        pub enum $kind {
            #[serde(rename = $wire)]
            $variant,
        }

        pub type $input = PortableRecord<$kind, $row>;
        pub type $output = PortableRecord<$kind, $row>;
    };
}

macro_rules! closed_row {
    ($name:ident { $($(#[$field_meta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($(#[$field_meta])* pub $field: $ty,)*
        }
    };
}

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        pub enum $name {
            $(#[serde(rename = $wire)] $variant,)+
        }
    };
}

string_enum!(ActorType { User => "user", Agent => "agent" });
string_enum!(AtomPolarity { Positive => "positive", Negative => "negative" });
string_enum!(AtomKind {
    Name => "name",
    Description => "description",
    AppliesWhen => "applies_when",
    PositiveExample => "positive_example",
    ExcludesWhen => "excludes_when",
    NegativeExample => "negative_example",
});
string_enum!(OntologyCandidateAtomKind {
    AppliesWhen => "applies_when",
    PositiveExample => "positive_example",
    ExcludesWhen => "excludes_when",
    NegativeExample => "negative_example",
});
string_enum!(ProposalStatus { Proposed => "proposed", Accepted => "accepted", Rejected => "rejected" });
string_enum!(SignalStatus { Open => "open", Confirmed => "confirmed", Rejected => "rejected", Superseded => "superseded", Resolved => "resolved" });
string_enum!(OntologySignalKind {
    FalseNegative => "false_negative",
    FalsePositive => "false_positive",
    VocabularyGap => "vocabulary_gap",
    NameIssue => "name_issue",
    BoundaryIssue => "boundary_issue",
    StructureIssue => "structure_issue",
});
string_enum!(OntologyProposedAction {
    Observe => "observe",
    AddPositiveAtom => "add_positive_atom",
    AddNegativeAtom => "add_negative_atom",
    UpdateSemantics => "update_semantics",
    BootstrapLabel => "bootstrap_label",
    RenameLabel => "rename_label",
    SplitLabel => "split_label",
    MergeLabels => "merge_labels",
});
string_enum!(SuggestState { Selected => "selected", Candidate => "candidate", Absent => "absent", Unavailable => "unavailable" });
string_enum!(OntologyActionType {
    Confirm => "confirm",
    Reject => "reject",
    Supersede => "supersede",
    ResolveNoChange => "resolve_no_change",
    AddPositiveAtom => "add_positive_atom",
    AddNegativeAtom => "add_negative_atom",
    AdoptExistingAtom => "adopt_existing_atom",
    UpdateSemantics => "update_semantics",
    CreateLabelProposal => "create_label_proposal",
    BootstrapLabel => "bootstrap_label",
    RenameLabel => "rename_label",
    SplitLabel => "split_label",
    MergeLabels => "merge_labels",
    Validate => "validate",
    RevertOntologyMutation => "revert_ontology_mutation",
});
string_enum!(ValidationStatus { NotRequired => "not_required", Pending => "pending", Passed => "passed", Failed => "failed", Partial => "partial" });
string_enum!(ValidationRequirement { None => "none", Required => "required", Unsupported => "unsupported" });
string_enum!(AtomEffect { Added => "added", Removed => "removed" });

closed_row!(LabelData {
    id: String,
    board_id: String,
    name: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    color: RequiredNullable<String>,
    created_at: i64,
    updated_at: i64,
});
record_kind!(
    LabelRecordType,
    Label,
    "label",
    LabelData,
    LabelInput,
    LabelOutput
);

closed_row!(LabelSemanticsData {
    label_id: String,
    board_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    description: RequiredNullable<String>,
    applies_when: Vec<String>,
    excludes_when: Vec<String>,
    positive_examples: Vec<String>,
    negative_examples: Vec<String>,
    created_at: i64,
    updated_at: i64,
});
record_kind!(
    LabelSemanticsRecordType,
    LabelSemantics,
    "label_semantics",
    LabelSemanticsData,
    LabelSemanticsInput,
    LabelSemanticsOutput
);

closed_row!(LabelAtomData {
    id: String,
    label_id: String,
    board_id: String,
    polarity: AtomPolarity,
    kind: AtomKind,
    text: String,
    ordinal: i64,
    content_hash: String,
    created_at: i64,
    updated_at: i64,
});
record_kind!(
    LabelAtomRecordType,
    LabelAtom,
    "label_atom",
    LabelAtomData,
    LabelAtomInput,
    LabelAtomOutput
);

closed_row!(LabelSemanticProposalData {
    id: String,
    board_id: String,
    task_id: String,
    status: ProposalStatus,
    name: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    description: RequiredNullable<String>,
    applies_when: Vec<String>,
    excludes_when: Vec<String>,
    positive_examples: Vec<String>,
    negative_examples: Vec<String>,
    heuristic_coverage: f64,
    heuristic_residual_norm: f64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    top1_existing_label_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    top1_existing_label_name: RequiredNullable<String>,
    diagnostics: Vec<Value>,
    created_by: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    decision_reason: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    resolved_label_id: RequiredNullable<String>,
    created_at: i64,
    updated_at: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    decided_at: RequiredNullable<i64>,
    heuristic_coverage_cosine: f64,
});
record_kind!(
    LabelSemanticProposalRecordType,
    LabelSemanticProposal,
    "label_semantic_proposal",
    LabelSemanticProposalData,
    LabelSemanticProposalInput,
    LabelSemanticProposalOutput
);

closed_row!(LabelOntologyObservationData {
    id: String,
    board_id: String,
    task_id: String,
    task_ref_snapshot: String,
    task_snapshot: JsonObject,
    agent_candidates: Vec<Value>,
    suggestion_snapshot: JsonObject,
    final_decision: JsonObject,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    suggest_coverage: RequiredNullable<f64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    suggest_coverage_cosine: RequiredNullable<f64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    suggest_residual_norm: RequiredNullable<f64>,
    suggest_needs_new_label: bool,
    suggest_degraded: bool,
    diagnostics: Vec<Value>,
    capture_fingerprint: String,
    created_by: String,
    created_by_type: ActorType,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    agent_type: RequiredNullable<String>,
    created_at: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    suggest_input_hash: RequiredNullable<String>,
});
record_kind!(
    LabelOntologyObservationRecordType,
    LabelOntologyObservation,
    "label_ontology_observation",
    LabelOntologyObservationData,
    LabelOntologyObservationInput,
    LabelOntologyObservationOutput
);

closed_row!(LabelOntologySignalData {
    id: String,
    observation_id: String,
    board_id: String,
    kind: OntologySignalKind,
    status: SignalStatus,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    target_label_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    target_label_name_snapshot: RequiredNullable<String>,
    related_labels: Vec<Value>,
    proposed_action: OntologyProposedAction,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    candidate_atom_polarity: RequiredNullable<AtomPolarity>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    candidate_atom_kind: RequiredNullable<OntologyCandidateAtomKind>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    candidate_text: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    candidate_content_hash: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    proposed_label_name: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    proposed_label_name_normalized: RequiredNullable<String>,
    proposal: JsonObject,
    agent_selected: bool,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    suggest_state: RequiredNullable<SuggestState>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    suggest_score: RequiredNullable<f64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    suggest_rank: RequiredNullable<i64>,
    final_selected: bool,
    rationale: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    confidence: RequiredNullable<f64>,
    signal_key: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    superseded_by_signal_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    status_reason: RequiredNullable<String>,
    created_at: i64,
    updated_at: i64,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    reviewed_at: RequiredNullable<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    closed_at: RequiredNullable<i64>,
});
record_kind!(
    LabelOntologySignalRecordType,
    LabelOntologySignal,
    "label_ontology_signal",
    LabelOntologySignalData,
    LabelOntologySignalInput,
    LabelOntologySignalOutput
);

closed_row!(LabelOntologyActionData {
    id: String,
    board_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    parent_action_id: RequiredNullable<String>,
    action_type: OntologyActionType,
    reason: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    target_label_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    result_label_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    result_atom_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    result_atom_content_hash: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    result_proposal_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    canonical_before_hash: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    canonical_after_hash: RequiredNullable<String>,
    change: JsonObject,
    validation_status: ValidationStatus,
    validation: JsonObject,
    created_by: String,
    created_by_type: ActorType,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    agent_type: RequiredNullable<String>,
    created_at: i64,
    validation_requirement: ValidationRequirement,
});
record_kind!(
    LabelOntologyActionRecordType,
    LabelOntologyAction,
    "label_ontology_action",
    LabelOntologyActionData,
    LabelOntologyActionInput,
    LabelOntologyActionOutput
);

closed_row!(LabelOntologyActionAtomEffectData {
    board_id: String,
    action_id: String,
    label_id_snapshot: String,
    atom_id_snapshot: String,
    atom_content_hash: String,
    polarity: AtomPolarity,
    kind: AtomKind,
    text: String,
    effect: AtomEffect,
    created_at: i64,
});
record_kind!(
    LabelOntologyActionAtomEffectRecordType,
    LabelOntologyActionAtomEffect,
    "label_ontology_action_atom_effect",
    LabelOntologyActionAtomEffectData,
    LabelOntologyActionAtomEffectInput,
    LabelOntologyActionAtomEffectOutput
);

closed_row!(LabelOntologyActionSignalData {
    board_id: String,
    action_id: String,
    signal_id: String,
    created_at: i64,
});
record_kind!(
    LabelOntologyActionSignalRecordType,
    LabelOntologyActionSignal,
    "label_ontology_action_signal",
    LabelOntologyActionSignalData,
    LabelOntologyActionSignalInput,
    LabelOntologyActionSignalOutput
);

closed_row!(SignalObservationData {
    id: String,
    board_id: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    task_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    task_ref_snapshot: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    run_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    comment_id: RequiredNullable<String>,
    actor: String,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    agent_type: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    source: RequiredNullable<String>,
    evidence: JsonObject,
    created_at: i64,
});
record_kind!(
    SignalObservationRecordType,
    SignalObservation,
    "signal_observation",
    SignalObservationData,
    SignalObservationInput,
    SignalObservationOutput
);

closed_row!(SignalData {
    id: String,
    board_id: String,
    observation_id: String,
    kind: String,
    title: String,
    summary: String,
    severity: String,
    status: SignalStatus,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    dedupe_key: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    superseded_by_signal_id: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    reviewed_by: RequiredNullable<String>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    reviewed_at: RequiredNullable<i64>,
    #[serde(deserialize_with = "deserialize_required_nullable")]
    #[cfg_attr(feature = "schema", schemars(required))]
    review_reason: RequiredNullable<String>,
    created_at: i64,
    updated_at: i64,
});
record_kind!(
    SignalRecordType,
    Signal,
    "signal",
    SignalData,
    SignalInput,
    SignalOutput
);

closed_row!(SettingData {
    key: String,
    value: Value,
    updated_at: i64,
});
record_kind!(
    SettingRecordType,
    Setting,
    "setting",
    SettingData,
    SettingInput,
    SettingOutput
);

fn normalize<T>(data: Map<String, Value>) -> serde_json::Result<Map<String, Value>>
where
    T: DeserializeOwned + Serialize,
{
    let row: T = serde_json::from_value(Value::Object(data))?;
    serde_json::to_value(row).map(|value| {
        value
            .as_object()
            .cloned()
            .expect("row serializes as object")
    })
}

macro_rules! normalize_match {
    ($discriminator:expr, $data:expr, $($wire:literal => $row:ty),+ $(,)?) => {
        match $discriminator {
            $($wire => normalize::<$row>($data),)+
            other => Err(<serde_json::Error as serde::de::Error>::custom(format!(
                "ledger contract 不拥有 discriminator：{other}"
            ))),
        }
    };
}

pub fn validate_input_data(
    discriminator: &str,
    data: Map<String, Value>,
) -> serde_json::Result<Map<String, Value>> {
    normalize_match!(discriminator, data,
        "label" => LabelData,
        "label_semantics" => LabelSemanticsData,
        "label_atom" => LabelAtomData,
        "label_semantic_proposal" => LabelSemanticProposalData,
        "label_ontology_observation" => LabelOntologyObservationData,
        "label_ontology_signal" => LabelOntologySignalData,
        "label_ontology_action" => LabelOntologyActionData,
        "label_ontology_action_atom_effect" => LabelOntologyActionAtomEffectData,
        "label_ontology_action_signal" => LabelOntologyActionSignalData,
        "signal_observation" => SignalObservationData,
        "signal" => SignalData,
        "setting" => SettingData,
    )
}

pub fn validate_output_data(
    discriminator: &str,
    data: Map<String, Value>,
) -> serde_json::Result<Map<String, Value>> {
    normalize_match!(discriminator, data,
        "label" => LabelData,
        "label_semantics" => LabelSemanticsData,
        "label_atom" => LabelAtomData,
        "label_semantic_proposal" => LabelSemanticProposalData,
        "label_ontology_observation" => LabelOntologyObservationData,
        "label_ontology_signal" => LabelOntologySignalData,
        "label_ontology_action" => LabelOntologyActionData,
        "label_ontology_action_atom_effect" => LabelOntologyActionAtomEffectData,
        "label_ontology_action_signal" => LabelOntologyActionSignalData,
        "signal_observation" => SignalObservationData,
        "signal" => SignalData,
        "setting" => SettingData,
    )
}

#[cfg(all(test, feature = "schema"))]
mod schema_tests {
    use std::collections::BTreeSet;

    use schemars::JsonSchema;

    use super::*;

    fn assert_every_property_is_required<T: JsonSchema>() {
        let schema = serde_json::to_value(schemars::schema_for!(T)).expect("schema JSON");
        let properties = schema["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{} must be an object schema", std::any::type_name::<T>()));
        let required = schema["required"]
            .as_array()
            .expect("closed portable row has required array")
            .iter()
            .map(|value| value.as_str().expect("required name"))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            properties
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            required,
            "all SQLite columns are present on the portable wire, including explicit nulls"
        );
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn every_ledger_row_schema_is_closed_and_requires_nullable_keys() {
        assert_every_property_is_required::<LabelData>();
        assert_every_property_is_required::<LabelSemanticsData>();
        assert_every_property_is_required::<LabelAtomData>();
        assert_every_property_is_required::<LabelSemanticProposalData>();
        assert_every_property_is_required::<LabelOntologyObservationData>();
        assert_every_property_is_required::<LabelOntologySignalData>();
        assert_every_property_is_required::<LabelOntologyActionData>();
        assert_every_property_is_required::<LabelOntologyActionAtomEffectData>();
        assert_every_property_is_required::<LabelOntologyActionSignalData>();
        assert_every_property_is_required::<SignalObservationData>();
        assert_every_property_is_required::<SignalData>();
        assert_every_property_is_required::<SettingData>();
    }

    #[test]
    fn ontology_candidate_atom_kind_schema_is_migration_exact() {
        let schema = serde_json::to_value(schemars::schema_for!(OntologyCandidateAtomKind))
            .expect("candidate atom kind schema");
        assert_eq!(
            schema["enum"],
            serde_json::json!([
                "applies_when",
                "positive_example",
                "excludes_when",
                "negative_example"
            ])
        );
        for invalid in ["name", "description"] {
            assert!(
                serde_json::from_value::<OntologyCandidateAtomKind>(serde_json::json!(invalid))
                    .is_err()
            );
        }
    }
}
