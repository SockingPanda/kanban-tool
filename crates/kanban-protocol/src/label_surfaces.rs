//! label、semantics、atom、proposal、signal 和 ontology API 家族的轻量 wire DTO。
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(feature = "schema")]
fn required_nullable_string_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<String>>()
}

#[cfg(feature = "schema")]
fn required_nullable_i64_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<i64>>()
}

#[cfg(feature = "schema")]
fn required_nullable_f64_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<f64>>()
}

#[cfg(feature = "schema")]
fn required_nullable_label_semantic_proposal_schema(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    generator.subschema_for::<Option<LabelSemanticProposalWire>>()
}

#[cfg(feature = "schema")]
fn required_nullable_label_atom_schema(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    generator.subschema_for::<Option<LabelAtomWire>>()
}

#[cfg(feature = "schema")]
fn required_nullable_label_semantics_schema(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    generator.subschema_for::<Option<LabelSemanticsWire>>()
}

macro_rules! wire {
    ($item:item) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields, rename_all = "snake_case")]
        $item
    };
}

fn default_label_suggestion_limit() -> usize {
    5
}
fn default_label_suggestion_candidate_limit() -> usize {
    32
}
fn default_label_suggestion_atom_limit() -> usize {
    80
}
fn default_label_suggestion_max_selected_labels() -> usize {
    4
}
fn default_label_suggestion_min_score() -> f32 {
    0.15
}
fn default_label_atom_index_limit() -> usize {
    24
}
fn default_label_surface_limit() -> usize {
    100
}
fn is_false(value: &bool) -> bool {
    !*value
}
fn is_default_label_suggestion_limit(value: &usize) -> bool {
    *value == default_label_suggestion_limit()
}
fn is_default_label_suggestion_candidate_limit(value: &usize) -> bool {
    *value == default_label_suggestion_candidate_limit()
}
fn is_default_label_suggestion_atom_limit(value: &usize) -> bool {
    *value == default_label_suggestion_atom_limit()
}
fn is_default_label_suggestion_max_selected_labels(value: &usize) -> bool {
    *value == default_label_suggestion_max_selected_labels()
}
fn is_default_label_suggestion_min_score(value: &f32) -> bool {
    *value == default_label_suggestion_min_score()
}
fn is_default_label_atom_index_limit(value: &usize) -> bool {
    *value == default_label_atom_index_limit()
}
fn is_default_label_surface_limit(value: &usize) -> bool {
    *value == default_label_surface_limit()
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum JsonBodyFieldWire {
    #[default]
    Missing,
    Present(Value),
}

impl JsonBodyFieldWire {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl Serialize for JsonBodyFieldWire {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Missing => serializer.serialize_none(),
            Self::Present(value) => value.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for JsonBodyFieldWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self::Present)
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for JsonBodyFieldWire {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "JsonBodyField".into()
    }
    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <Value as schemars::JsonSchema>::json_schema(generator)
    }
}

wire!(
    pub struct BoardLabelPath {
        pub board: String,
    }
);
wire!(
    pub struct LabelSemanticsPath {
        pub board: String,
        pub label_id: String,
    }
);
wire!(
    pub struct LabelAtomPath {
        pub board: String,
        pub atom_ref: String,
    }
);
wire!(
    pub struct TaskLabelSurfacePath {
        pub task_id: String,
    }
);
wire!(
    pub struct ListBoardLabelProposalsPath {
        pub board: String,
    }
);
wire!(
    pub struct SignalPath {
        pub signal_id: String,
    }
);
wire!(
    pub struct ProposalPath {
        pub proposal_id: String,
    }
);
wire!(
    pub struct CreateBoardLabelRequest {
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub color: Option<String>,
    }
);

wire!(
    pub struct LabelAtomWire {
        pub id: String,
        pub label_id: String,
        pub board_id: String,
        pub label_name: String,
        pub polarity: String,
        pub kind: String,
        pub text: String,
        pub ordinal: i64,
        pub content_hash: String,
        pub created_at: i64,
        pub updated_at: i64,
    }
);
wire!(
    pub struct LabelSemanticsWire {
        pub label_id: String,
        pub board_id: String,
        pub label_name: String,
        pub semantics_hash: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub description: Option<String>,
        pub applies_when: Vec<String>,
        pub excludes_when: Vec<String>,
        pub positive_examples: Vec<String>,
        pub negative_examples: Vec<String>,
        pub created_at: i64,
        pub updated_at: i64,
        pub atoms: Vec<LabelAtomWire>,
    }
);
wire!(
    pub struct UpsertLabelSemanticsRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub actor: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub expected_semantics_hash: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = false)))]
        pub replace: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub reason: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(
            feature = "schema",
            schemars(extend("default" = serde_json::json!([])))
        )]
        pub source_signal_ids: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub applies_when: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub excludes_when: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub positive_examples: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub negative_examples: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub remove_applies_when: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub remove_excludes_when: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub remove_positive_examples: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub remove_negative_examples: Vec<String>,
    }
);
wire!(
    pub struct DeleteLabelSemanticsQuery {
        pub expected_semantics_hash: String,
        pub reason: String,
    }
);
wire!(
    pub struct LabelAtomIndexQuery {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub q: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vector_json: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub embedding_model: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = false)))]
        pub include_vector: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub polarity: Option<String>,
        #[serde(
            default = "default_label_atom_index_limit",
            skip_serializing_if = "is_default_label_atom_index_limit"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 24)))]
        pub limit: usize,
    }
);
wire!(
    pub struct VectorStoreStatusWire {
        pub backend: String,
        pub enabled: bool,
        pub message: String,
        #[serde(default)]
        pub diagnostics: Vec<String>,
        #[serde(default)]
        pub dirty: Option<bool>,
        #[serde(default)]
        pub board_dirty: Option<bool>,
        pub generation: Option<i64>,
    }
);

wire!(
    pub struct LabelSuggestionQuery {
        #[serde(
            default = "default_label_suggestion_limit",
            skip_serializing_if = "is_default_label_suggestion_limit"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 5)))]
        pub limit: usize,
        #[serde(
            default = "default_label_suggestion_candidate_limit",
            skip_serializing_if = "is_default_label_suggestion_candidate_limit"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 32)))]
        pub candidate_limit: usize,
        #[serde(
            default = "default_label_suggestion_atom_limit",
            skip_serializing_if = "is_default_label_suggestion_atom_limit"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 80)))]
        pub atom_limit: usize,
        #[serde(
            default = "default_label_suggestion_max_selected_labels",
            skip_serializing_if = "is_default_label_suggestion_max_selected_labels"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 4)))]
        pub max_selected_labels: usize,
        #[serde(
            default = "default_label_suggestion_min_score",
            skip_serializing_if = "is_default_label_suggestion_min_score"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 0.15_f32)))]
        pub min_score: f32,
    }
);
wire!(
    pub struct ListBoardLabelProposalsQuery {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub status: Option<LabelProposalStatusWire>,
    }
);
wire!(
    pub struct LabelSuggestionEvidenceAtomWire {
        pub atom_id: String,
        pub label_id: String,
        pub label_name: String,
        pub polarity: String,
        pub kind: String,
        pub text: String,
        pub score: f32,
    }
);
wire!(
    pub struct LabelSuggestionCandidateWire {
        pub label_id: String,
        pub label_name: String,
        pub score: f32,
        pub weight: f32,
        pub already_applied: bool,
        pub evidence_atoms: Vec<LabelSuggestionEvidenceAtomWire>,
        pub negative_evidence_atoms: Vec<LabelSuggestionEvidenceAtomWire>,
    }
);
wire!(
    pub struct LabelSuggestionResultWire {
        pub task_id: String,
        pub board_id: String,
        pub selected_labels: Vec<LabelSuggestionCandidateWire>,
        pub candidates: Vec<LabelSuggestionCandidateWire>,
        pub coverage: f32,
        pub coverage_cosine: f32,
        pub residual_norm: f32,
        pub needs_new_label: bool,
        pub reason_codes: Vec<String>,
        pub degraded: bool,
        pub diagnostics: Vec<String>,
    }
);
wire!(
    pub struct BootstrapTaskLabelRequest {
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub applies_when: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub excludes_when: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub positive_examples: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub negative_examples: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub actor: Option<String>,
    }
);
wire!(
    pub struct BootstrapTaskLabelData {
        pub task: crate::ApiTask,
        pub semantics: LabelSemanticsWire,
    }
);

wire!(
    pub struct LabelProposalCandidateWire {
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
);
wire!(
    pub struct LabelOntologyActorWire {
        pub name: String,
        #[serde(rename = "type")]
        pub actor_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub agent_type: Option<String>,
    }
);
wire!(
    pub struct ProposeTaskLabelRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub proposal: Option<LabelProposalCandidateWire>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub actor: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub source_signal_ids: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub ontology_actor: Option<LabelOntologyActorWire>,
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = false)))]
        pub allow_retarget: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub retarget_reason: Option<String>,
    }
);
wire!(
    pub struct LabelProposalDecisionRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub actor: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub source_signal_ids: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub ontology_actor: Option<LabelOntologyActorWire>,
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = false)))]
        pub allow_retarget: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub retarget_reason: Option<String>,
    }
);
wire!(
    pub enum LabelProposalStatusWire {
        Proposed,
        Accepted,
        Rejected,
    }
);
wire!(
    pub struct LabelSemanticProposalWire {
        pub id: String,
        pub board_id: String,
        pub task_id: String,
        pub status: LabelProposalStatusWire,
        pub name: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub description: Option<String>,
        pub applies_when: Vec<String>,
        pub excludes_when: Vec<String>,
        pub positive_examples: Vec<String>,
        pub negative_examples: Vec<String>,
        pub heuristic_coverage: f32,
        pub heuristic_coverage_cosine: f32,
        pub heuristic_residual_norm: f32,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub top1_existing_label_id: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub top1_existing_label_name: Option<String>,
        pub diagnostics: Vec<String>,
        pub created_by: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub decision_reason: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub resolved_label_id: Option<String>,
        pub created_at: i64,
        pub updated_at: i64,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_i64_schema")
        )]
        pub decided_at: Option<i64>,
    }
);
wire!(
    pub struct LabelProposalAttemptWire {
        pub task_id: String,
        pub board_id: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(
                required,
                schema_with = "required_nullable_label_semantic_proposal_schema"
            )
        )]
        pub proposal: Option<LabelSemanticProposalWire>,
        pub degraded: bool,
        pub diagnostics: Vec<String>,
        pub heuristic_coverage: f32,
        pub heuristic_coverage_cosine: f32,
        pub heuristic_residual_norm: f32,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub top1_existing_label_id: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub top1_existing_label_name: Option<String>,
    }
);

wire!(
    pub struct SignalQuery {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub status: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub kind: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub task_ref: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = false)))]
        pub include_all: bool,
        #[serde(
            default = "default_label_surface_limit",
            skip_serializing_if = "is_default_label_surface_limit"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 100)))]
        pub limit: usize,
    }
);
wire!(
    pub struct SignalObservationWire {
        pub id: String,
        pub board_id: String,
        pub task_id: Option<String>,
        pub task_ref_snapshot: Option<String>,
        pub run_id: Option<String>,
        pub comment_id: Option<String>,
        pub actor: String,
        pub agent_type: Option<String>,
        pub source: Option<String>,
        pub evidence: crate::structured_metadata::SignalEvidence,
        pub created_at: i64,
    }
);
wire!(
    pub struct SignalWire {
        pub id: String,
        pub board_id: String,
        pub observation_id: String,
        pub kind: String,
        pub title: String,
        pub summary: String,
        pub severity: String,
        pub status: String,
        pub dedupe_key: Option<String>,
        pub superseded_by_signal_id: Option<String>,
        pub reviewed_by: Option<String>,
        pub reviewed_at: Option<i64>,
        pub review_reason: Option<String>,
        pub created_at: i64,
        pub updated_at: i64,
        pub observation: SignalObservationWire,
    }
);

wire!(
    pub enum LabelOntologySignalKindWire {
        FalseNegative,
        FalsePositive,
        VocabularyGap,
        NameIssue,
        BoundaryIssue,
        StructureIssue,
    }
);
wire!(
    pub enum LabelOntologyProposedActionWire {
        Observe,
        AddPositiveAtom,
        AddNegativeAtom,
        UpdateSemantics,
        BootstrapLabel,
        RenameLabel,
        SplitLabel,
        MergeLabels,
    }
);
wire!(
    pub enum LabelOntologySuggestStateWire {
        Selected,
        Candidate,
        Absent,
        Unavailable,
    }
);
wire!(
    pub enum LabelOntologyActionTypeWire {
        Confirm,
        Reject,
        Supersede,
        ResolveNoChange,
        AddPositiveAtom,
        AddNegativeAtom,
        AdoptExistingAtom,
        UpdateSemantics,
        CreateLabelProposal,
        BootstrapLabel,
        RenameLabel,
        SplitLabel,
        MergeLabels,
        RevertOntologyMutation,
        Validate,
    }
);
wire!(
    pub enum LabelOntologyValidationStatusWire {
        NotRequired,
        Pending,
        Passed,
        Failed,
        Partial,
    }
);
wire!(
    pub enum LabelOntologyValidationRequirementWire {
        None,
        Required,
        Unsupported,
    }
);
wire!(
    pub enum LabelOntologyValidationEffectiveOutcomeWire {
        NotRequired,
        Unsupported,
        Pending,
        Passed,
        Failed,
        Partial,
    }
);
wire!(
    pub struct LabelOntologyCandidateAtomRequest {
        pub polarity: String,
        pub kind: String,
        pub text: String,
    }
);
wire!(
    pub struct LabelOntologySignalRequest {
        pub kind: LabelOntologySignalKindWire,
        pub target_label_ref: Option<String>,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub related_labels: JsonBodyFieldWire,
        pub proposed_action: LabelOntologyProposedActionWire,
        pub candidate_atom: Option<LabelOntologyCandidateAtomRequest>,
        pub proposed_label_name: Option<String>,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub proposal: JsonBodyFieldWire,
        pub agent_selected: bool,
        pub suggest_state: Option<LabelOntologySuggestStateWire>,
        pub suggest_score: Option<f64>,
        pub suggest_rank: Option<i64>,
        pub final_selected: bool,
        pub rationale: String,
        pub confidence: Option<f64>,
        pub signal_key: Option<String>,
    }
);
wire!(
    pub struct RecordLabelOntologyObservationRequest {
        pub actor: LabelOntologyActorWire,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub agent_candidates: JsonBodyFieldWire,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub suggestion_snapshot: JsonBodyFieldWire,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub final_decision: JsonBodyFieldWire,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub suggest_coverage: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub suggest_coverage_cosine: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub suggest_residual_norm: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub suggest_needs_new_label: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub suggest_degraded: Option<bool>,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub diagnostics: JsonBodyFieldWire,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub capture_fingerprint: Option<String>,
        pub signals: Vec<LabelOntologySignalRequest>,
    }
);
wire!(
    pub struct LabelOntologyActionRequest {
        pub actor: LabelOntologyActorWire,
        pub action_type: LabelOntologyActionTypeWire,
        pub signal_ids: Vec<String>,
        pub reason: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub superseded_by_signal_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parent_action_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub target_label_ref: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub result_label_ref: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub result_atom_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub result_atom_content_hash: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub result_proposal_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub canonical_before_hash: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub canonical_after_hash: Option<String>,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub change: JsonBodyFieldWire,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub validation_status: Option<LabelOntologyValidationStatusWire>,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub validation: JsonBodyFieldWire,
    }
);
wire!(
    pub struct ApplyLabelOntologyAtomRequest {
        pub actor: LabelOntologyActorWire,
        pub signal_ids: Vec<String>,
        pub label_ref: String,
        pub kind: String,
        pub text: String,
        pub reason: String,
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = false)))]
        pub allow_retarget: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub retarget_reason: Option<String>,
    }
);
wire!(
    pub struct RevertLabelOntologyMutationRequest {
        pub actor: LabelOntologyActorWire,
        pub target_action_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub expected_current_hash: Option<String>,
        pub reason: String,
    }
);
wire!(
    pub struct ValidateLabelOntologyActionRequest {
        pub actor: LabelOntologyActorWire,
        pub parent_action_id: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub signal_ids: Vec<String>,
        pub reason: String,
        pub validation_status: LabelOntologyValidationStatusWire,
        #[serde(default, skip_serializing_if = "JsonBodyFieldWire::is_missing")]
        pub validation: JsonBodyFieldWire,
    }
);

wire!(
    pub struct LabelOntologyActionWire {
        pub id: String,
        pub board_id: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub parent_action_id: Option<String>,
        pub action_type: LabelOntologyActionTypeWire,
        pub reason: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub target_label_id: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub result_label_id: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub result_atom_id: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub result_atom_content_hash: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub result_proposal_id: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub canonical_before_hash: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub canonical_after_hash: Option<String>,
        pub change: crate::structured_metadata::OntologyChange,
        pub validation_requirement: LabelOntologyValidationRequirementWire,
        pub validation_status: LabelOntologyValidationStatusWire,
        pub validation_effective_outcome: LabelOntologyValidationEffectiveOutcomeWire,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub validation_latest_attempt_id: Option<String>,
        pub validation: crate::structured_metadata::OntologyValidation,
        pub created_by: String,
        pub created_by_type: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub agent_type: Option<String>,
        pub created_at: i64,
        pub signal_ids: Vec<String>,
    }
);
wire!(
    pub struct LabelOntologyObservationWire {
        pub id: String,
        pub board_id: String,
        pub task_id: String,
        pub task_ref_snapshot: String,
        pub task_snapshot: crate::structured_metadata::OntologyTaskSnapshot,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub suggest_input_hash: Option<String>,
        pub agent_candidates: crate::structured_metadata::OntologyCandidates,
        pub suggestion_snapshot: crate::structured_metadata::OntologySuggestionSnapshot,
        pub final_decision: crate::structured_metadata::OntologyDecisionSnapshot,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_f64_schema")
        )]
        pub suggest_coverage: Option<f64>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_f64_schema")
        )]
        pub suggest_coverage_cosine: Option<f64>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_f64_schema")
        )]
        pub suggest_residual_norm: Option<f64>,
        pub suggest_needs_new_label: bool,
        pub suggest_degraded: bool,
        pub diagnostics: crate::structured_metadata::OntologyDiagnostics,
        pub capture_fingerprint: String,
        pub created_by: String,
        pub created_by_type: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub agent_type: Option<String>,
        pub created_at: i64,
        pub signals: Vec<crate::LabelOntologySignalWire>,
    }
);
wire!(
    pub struct LabelOntologySignalDetailWire {
        pub signal: crate::LabelOntologySignalWire,
        pub observation: LabelOntologyObservationWire,
        pub actions: Vec<LabelOntologyActionWire>,
    }
);
wire!(
    pub struct LabelAtomExplainActionWire {
        pub action: LabelOntologyActionWire,
        pub matched_by: String,
    }
);
wire!(
    pub struct LabelAtomExplainSignalWire {
        pub signal: crate::LabelOntologySignalWire,
        pub observation: LabelOntologyObservationWire,
        pub source_task: crate::ApiTask,
        pub task_ref_snapshot: String,
        pub suggest_input_stale: bool,
        pub suggest_degraded: bool,
        pub warnings: Vec<String>,
    }
);
wire!(
    pub struct LabelAtomExplainValidationWire {
        pub action: LabelOntologyActionWire,
        pub parent_action_id: String,
        pub validation_status: LabelOntologyValidationStatusWire,
        pub manual: Value,
        pub summary: Value,
        pub cases: Value,
        pub warnings: Vec<String>,
    }
);
wire!(
    pub struct LabelAtomExplainWire {
        pub query: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_label_atom_schema")
        )]
        pub atom: Option<LabelAtomWire>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_label_semantics_schema")
        )]
        pub current_semantics: Option<LabelSemanticsWire>,
        pub provenance_actions: Vec<LabelAtomExplainActionWire>,
        pub supporting_signals: Vec<LabelAtomExplainSignalWire>,
        pub validation_history: Vec<LabelAtomExplainValidationWire>,
        pub legacy_untracked: bool,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub legacy_reason: Option<String>,
    }
);
wire!(
    pub struct LabelOntologySignalQuery {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub status: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = serde_json::json!([]))))]
        pub kind: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub task_ref: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub target_label_ref: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub proposed_label_name: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = false)))]
        pub include_all: bool,
        #[serde(
            default = "default_label_surface_limit",
            skip_serializing_if = "is_default_label_surface_limit"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 100)))]
        pub limit: usize,
    }
);
wire!(
    pub enum LabelOntologyReviewGroupByWire {
        Label,
        CandidateAtom,
        ProposedLabel,
        Cluster,
    }
);
fn default_label_ontology_review_group_by() -> LabelOntologyReviewGroupByWire {
    LabelOntologyReviewGroupByWire::Label
}
fn is_default_label_ontology_review_group_by(value: &LabelOntologyReviewGroupByWire) -> bool {
    *value == default_label_ontology_review_group_by()
}
wire!(
    pub struct LabelOntologyReviewQuery {
        #[serde(
            default = "default_label_ontology_review_group_by",
            skip_serializing_if = "is_default_label_ontology_review_group_by"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = "label")))]
        pub group_by: LabelOntologyReviewGroupByWire,
        #[serde(default, skip_serializing_if = "is_false")]
        #[cfg_attr(feature = "schema", schemars(extend("default" = false)))]
        pub include_all: bool,
        #[serde(
            default = "default_label_surface_limit",
            skip_serializing_if = "is_default_label_surface_limit"
        )]
        #[cfg_attr(feature = "schema", schemars(extend("default" = 100)))]
        pub limit: usize,
    }
);
wire!(
    pub struct LabelOntologyReviewLabelRefWire {
        pub id: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub name: Option<String>,
    }
);
wire!(
    pub struct LabelOntologyReviewAtomVariantWire {
        pub content_hash: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub polarity: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub kind: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub text: Option<String>,
        pub signal_count: i64,
    }
);
wire!(
    pub struct LabelOntologyReviewGroupWire {
        pub group_by: LabelOntologyReviewGroupByWire,
        pub key: String,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub label_id: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub label_name: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub candidate_atom_polarity: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub candidate_atom_kind: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub candidate_text: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub candidate_content_hash: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub proposed_label_name: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub proposed_label_name_normalized: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub cluster_key: Option<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_string_schema")
        )]
        pub cluster_reason: Option<String>,
        pub task_count: i64,
        pub signal_count: i64,
        pub open_count: i64,
        pub confirmed_count: i64,
        pub resolved_count: i64,
        pub rejected_count: i64,
        pub superseded_count: i64,
        pub degraded_count: i64,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_f64_schema")
        )]
        pub average_score: Option<f64>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_f64_schema")
        )]
        pub median_score: Option<f64>,
        pub oldest_signal_at: i64,
        pub latest_signal_at: i64,
        pub sample_task_refs: Vec<String>,
        pub signal_ids: Vec<String>,
        pub action_count: i64,
        pub action_ids: Vec<String>,
        pub proposal_ids: Vec<String>,
        pub labels: Vec<LabelOntologyReviewLabelRefWire>,
        pub candidate_atom_variants: Vec<LabelOntologyReviewAtomVariantWire>,
    }
);

pub type ListBoardLabelsResponse = crate::DataEnvelope<Vec<crate::ApiLabel>>;
pub type CreateBoardLabelResponse = crate::DataEnvelope<crate::ApiLabel>;
pub type ListLabelSemanticsResponse = crate::DataEnvelope<Vec<LabelSemanticsWire>>;
pub type GetLabelSemanticsResponse = crate::DataEnvelope<LabelSemanticsWire>;
pub type UpsertLabelSemanticsResponse = crate::DataEnvelope<LabelSemanticsWire>;
pub type ListLabelAtomsResponse = crate::DataEnvelope<Vec<LabelAtomWire>>;
pub type ExplainLabelAtomResponse = crate::DataEnvelope<LabelAtomExplainWire>;
pub type LabelAtomIndexStatusResponse = crate::DataEnvelope<VectorStoreStatusWire>;
pub type RebuildLabelAtomIndexResponse = crate::DataEnvelope<VectorStoreStatusWire>;
pub type QueryLabelAtomIndexResponse = crate::DataEnvelope<Value>;
pub type SuggestTaskLabelsResponse = crate::DataEnvelope<LabelSuggestionResultWire>;
pub type BootstrapTaskLabelResponse = crate::DataEnvelope<BootstrapTaskLabelData>;
pub type ProposeTaskLabelResponse = crate::DataEnvelope<LabelProposalAttemptWire>;
pub type ListTaskLabelProposalsResponse = crate::DataEnvelope<Vec<LabelSemanticProposalWire>>;
pub type ListBoardLabelProposalsResponse = crate::DataEnvelope<Vec<LabelSemanticProposalWire>>;
pub type GetLabelProposalResponse = crate::DataEnvelope<LabelSemanticProposalWire>;
pub type LabelProposalDecisionResponse = crate::DataEnvelope<LabelSemanticProposalWire>;
pub type ListSignalsResponse = crate::MetadataEnvelope<Vec<SignalWire>, crate::SignalFilterMeta>;
pub type ReviewSignalsResponse = crate::MetadataEnvelope<Vec<SignalWire>, crate::SignalFilterMeta>;
pub type GetSignalResponse = crate::DataEnvelope<SignalWire>;
pub type RecordLabelOntologyObservationResponse = crate::DataEnvelope<LabelOntologyObservationWire>;
pub type ReviewLabelOntologyResponse =
    crate::MetadataEnvelope<Vec<LabelOntologyReviewGroupWire>, crate::LabelOntologyReviewMeta>;
pub type LabelOntologyActionResponse = crate::DataEnvelope<LabelOntologyActionWire>;
pub type GetLabelOntologySignalResponse = crate::DataEnvelope<LabelOntologySignalDetailWire>;
