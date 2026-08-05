//! Contract-owned output DTOs for every `kanban label ... --json` leaf.
//!
//! The CLI adapter converts SQLite service records into these types before
//! serialization. API exact roots remain owned by `label_surfaces`; the CLI
//! roots below deliberately have their own names even when they reuse the same
//! closed wire components.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    ApiLabel, ApiTask, DataEnvelope, LabelAtomExplainWire, LabelAtomWire, LabelOntologyActionWire,
    LabelOntologyObservationWire, LabelOntologyReviewGroupWire, LabelOntologySignalDetailWire,
    LabelOntologySignalWire, LabelProposalAttemptWire, LabelSemanticProposalWire,
    LabelSemanticsWire, LabelSuggestionResultWire,
};

macro_rules! cli_wire {
    ($item:item) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        $item
    };
}

fn deserialize_required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[cfg(feature = "schema")]
fn required_nullable_bool_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generator.subschema_for::<Option<bool>>()
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
fn required_nullable_bootstrap_verification_schema(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    generator.subschema_for::<Option<CliLabelBootstrapVerification>>()
}

pub type CliLabelListOutput = DataEnvelope<Vec<ApiLabel>>;
pub type CliLabelCreateOutput = DataEnvelope<ApiLabel>;

cli_wire!(
    pub struct CliLabelDeleteResult {
        pub label: ApiLabel,
        pub forced: bool,
        pub removed_task_bindings: i64,
        pub removed_semantics: bool,
        pub removed_atoms: i64,
    }
);
pub type CliLabelDeleteOutput = DataEnvelope<CliLabelDeleteResult>;

cli_wire!(
    pub struct CliLabelBootstrapVerification {
        pub label_name: String,
        pub score: f32,
        pub source: String,
        pub min_score: f32,
        pub degraded: bool,
        pub diagnostics: Vec<String>,
    }
);
cli_wire!(
    pub struct CliLabelBootstrapResult {
        pub task: ApiTask,
        pub semantics: LabelSemanticsWire,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(
                required,
                schema_with = "required_nullable_bootstrap_verification_schema"
            )
        )]
        pub verification: Option<CliLabelBootstrapVerification>,
    }
);
pub type CliLabelBootstrapOutput = DataEnvelope<CliLabelBootstrapResult>;

cli_wire!(
    pub struct CliLabelAddWithCreated {
        pub task: ApiTask,
        pub created_labels: Vec<ApiLabel>,
    }
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum CliLabelAddResult {
    Task(ApiTask),
    WithCreated(CliLabelAddWithCreated),
}

pub type CliLabelAddOutput = DataEnvelope<CliLabelAddResult>;
pub type CliLabelRemoveOutput = DataEnvelope<ApiTask>;

pub type CliLabelSemanticsListOutput = DataEnvelope<Vec<LabelSemanticsWire>>;
pub type CliLabelSemanticsShowOutput = DataEnvelope<LabelSemanticsWire>;
pub type CliLabelSemanticsUpsertOutput = DataEnvelope<LabelSemanticsWire>;

cli_wire!(
    pub struct CliLabelSemanticsDeleteResult {
        pub deleted: bool,
    }
);
pub type CliLabelSemanticsDeleteOutput = DataEnvelope<CliLabelSemanticsDeleteResult>;

pub type CliLabelAtomsListOutput = DataEnvelope<Vec<LabelAtomWire>>;
pub type CliLabelAtomsExplainOutput = DataEnvelope<LabelAtomExplainWire>;

cli_wire!(
    pub struct CliLabelAtomIndexStatus {
        pub backend: String,
        pub enabled: bool,
        pub message: String,
        pub diagnostics: Vec<String>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_bool_schema")
        )]
        pub dirty: Option<bool>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_bool_schema")
        )]
        pub board_dirty: Option<bool>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_i64_schema")
        )]
        pub generation: Option<i64>,
    }
);
pub type CliLabelAtomIndexStatusOutput = DataEnvelope<CliLabelAtomIndexStatus>;
pub type CliLabelAtomIndexRebuildOutput = DataEnvelope<CliLabelAtomIndexStatus>;

cli_wire!(
    pub struct CliLabelAtomIndexHit {
        pub atom_id: String,
        pub label_id: String,
        pub label_name: String,
        pub board_id: String,
        pub polarity: String,
        pub kind: String,
        pub text: String,
        pub ordinal: i64,
        pub content_hash: String,
        pub embedding_model: String,
        pub distance: f32,
    }
);
pub type CliLabelAtomIndexQueryOutput = DataEnvelope<Vec<CliLabelAtomIndexHit>>;

pub type CliLabelSuggestOutput = DataEnvelope<LabelSuggestionResultWire>;
pub type CliLabelProposeOutput = DataEnvelope<LabelProposalAttemptWire>;
pub type CliLabelProposalsListOutput = DataEnvelope<Vec<LabelSemanticProposalWire>>;
pub type CliLabelProposalsShowOutput = DataEnvelope<LabelSemanticProposalWire>;
pub type CliLabelProposalsAcceptOutput = DataEnvelope<LabelSemanticProposalWire>;
pub type CliLabelProposalsRejectOutput = DataEnvelope<LabelSemanticProposalWire>;

pub type CliLabelOntologyRecordOutput = DataEnvelope<LabelOntologyObservationWire>;
pub type CliLabelOntologyListOutput = DataEnvelope<Vec<LabelOntologySignalWire>>;
pub type CliLabelOntologyShowOutput = DataEnvelope<LabelOntologySignalDetailWire>;
pub type CliLabelOntologyReviewOutput = DataEnvelope<Vec<LabelOntologyReviewGroupWire>>;

cli_wire!(
    pub struct CliLabelOntologyQualityDenominator {
        pub source: String,
        pub description: String,
        pub observation_count: i64,
        pub distinct_task_count: i64,
        pub agreement_observation_count: i64,
        pub agreement_task_count: i64,
        pub degraded_observation_count: i64,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_i64_schema")
        )]
        pub first_observed_at: Option<i64>,
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_i64_schema")
        )]
        pub latest_observed_at: Option<i64>,
        pub sample_task_refs: Vec<String>,
    }
);
cli_wire!(
    pub struct CliLabelOntologyQualityDisagreement {
        pub signal_count: i64,
        pub distinct_task_count: i64,
        pub by_kind: BTreeMap<String, i64>,
        pub by_status: BTreeMap<String, i64>,
    }
);
cli_wire!(
    pub struct CliLabelOntologyQualityRates {
        #[serde(deserialize_with = "deserialize_required_nullable")]
        #[cfg_attr(
            feature = "schema",
            schemars(required, schema_with = "required_nullable_f64_schema")
        )]
        pub disagreement_task_rate: Option<f64>,
        pub disagreement_task_rate_basis: String,
    }
);
cli_wire!(
    pub struct CliLabelOntologyPrecisionRecall {
        pub available: bool,
        pub reason: String,
    }
);
cli_wire!(
    pub struct CliLabelOntologyQuality {
        pub board_id: String,
        pub denominator: CliLabelOntologyQualityDenominator,
        pub disagreement: CliLabelOntologyQualityDisagreement,
        pub rates: CliLabelOntologyQualityRates,
        pub precision_recall: CliLabelOntologyPrecisionRecall,
        pub warnings: Vec<String>,
    }
);
pub type CliLabelOntologyQualityOutput = DataEnvelope<CliLabelOntologyQuality>;

pub type CliLabelOntologyConfirmOutput = DataEnvelope<LabelOntologyActionWire>;
pub type CliLabelOntologyRejectOutput = DataEnvelope<LabelOntologyActionWire>;
pub type CliLabelOntologySupersedeOutput = DataEnvelope<LabelOntologyActionWire>;
pub type CliLabelOntologyResolveOutput = DataEnvelope<LabelOntologyActionWire>;
pub type CliLabelOntologyApplyAtomOutput = DataEnvelope<LabelOntologyActionWire>;
pub type CliLabelOntologyRevertOutput = DataEnvelope<LabelOntologyActionWire>;
pub type CliLabelOntologyValidateOutput = DataEnvelope<LabelOntologyActionWire>;

#[cfg(all(test, feature = "schema"))]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde_json::Value;
    use std::collections::BTreeSet;

    fn assert_every_object_property_is_required<T: JsonSchema>(root: &str) {
        let schema = serde_json::to_value(schemars::schema_for!(T)).expect("serialize schema");
        assert_required_properties(&schema, root);
    }

    fn assert_required_properties(schema: &Value, path: &str) {
        match schema {
            Value::Object(object) => {
                if let Some(properties) = object.get("properties").and_then(Value::as_object) {
                    let required = object
                        .get("required")
                        .and_then(Value::as_array)
                        .map(|required| {
                            required
                                .iter()
                                .filter_map(Value::as_str)
                                .collect::<BTreeSet<_>>()
                        })
                        .unwrap_or_default();
                    for field in properties.keys() {
                        assert!(
                            required.contains(field.as_str()),
                            "{path} schema allows producer field `{field}` to be missing"
                        );
                    }
                }
                for (key, value) in object {
                    assert_required_properties(value, &format!("{path}/{key}"));
                }
            }
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate() {
                    assert_required_properties(value, &format!("{path}/{index}"));
                }
            }
            _ => {}
        }
    }

    #[test]
    fn every_cli_label_output_schema_requires_every_producer_field() {
        macro_rules! check {
            ($($root:ty),+ $(,)?) => {
                $(assert_every_object_property_is_required::<$root>(stringify!($root));)+
            };
        }
        check!(
            CliLabelAddOutput,
            CliLabelAtomIndexQueryOutput,
            CliLabelAtomIndexRebuildOutput,
            CliLabelAtomIndexStatusOutput,
            CliLabelAtomsExplainOutput,
            CliLabelAtomsListOutput,
            CliLabelBootstrapOutput,
            CliLabelCreateOutput,
            CliLabelDeleteOutput,
            CliLabelListOutput,
            CliLabelOntologyApplyAtomOutput,
            CliLabelOntologyConfirmOutput,
            CliLabelOntologyListOutput,
            CliLabelOntologyQualityOutput,
            CliLabelOntologyRecordOutput,
            CliLabelOntologyRejectOutput,
            CliLabelOntologyResolveOutput,
            CliLabelOntologyRevertOutput,
            CliLabelOntologyReviewOutput,
            CliLabelOntologyShowOutput,
            CliLabelOntologySupersedeOutput,
            CliLabelOntologyValidateOutput,
            CliLabelProposalsAcceptOutput,
            CliLabelProposalsListOutput,
            CliLabelProposalsRejectOutput,
            CliLabelProposalsShowOutput,
            CliLabelProposeOutput,
            CliLabelRemoveOutput,
            CliLabelSemanticsDeleteOutput,
            CliLabelSemanticsListOutput,
            CliLabelSemanticsShowOutput,
            CliLabelSemanticsUpsertOutput,
            CliLabelSuggestOutput,
        );
    }
}
