use kanban_contract::cli_labels::{
    CliLabelAddOutput, CliLabelAtomIndexQueryOutput, CliLabelAtomIndexRebuildOutput,
    CliLabelAtomIndexStatusOutput, CliLabelAtomsExplainOutput, CliLabelAtomsListOutput,
    CliLabelBootstrapOutput, CliLabelCreateOutput, CliLabelDeleteOutput, CliLabelListOutput,
    CliLabelOntologyApplyAtomOutput, CliLabelOntologyConfirmOutput, CliLabelOntologyListOutput,
    CliLabelOntologyQualityOutput, CliLabelOntologyRecordOutput, CliLabelOntologyRejectOutput,
    CliLabelOntologyResolveOutput, CliLabelOntologyRevertOutput, CliLabelOntologyReviewOutput,
    CliLabelOntologyShowOutput, CliLabelOntologySupersedeOutput, CliLabelOntologyValidateOutput,
    CliLabelProposalsAcceptOutput, CliLabelProposalsListOutput, CliLabelProposalsRejectOutput,
    CliLabelProposalsShowOutput, CliLabelProposeOutput, CliLabelRemoveOutput,
    CliLabelSemanticsDeleteOutput, CliLabelSemanticsListOutput, CliLabelSemanticsShowOutput,
    CliLabelSemanticsUpsertOutput, CliLabelSuggestOutput,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/fixtures/cli")
        .join(format!("{name}-output.v1.valid.json"));
    serde_json::from_str(
        &fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn assert_json_equivalent(actual: &Value, expected: &Value) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            assert_eq!(
                actual.as_f64(),
                expected.as_f64(),
                "numeric fixture value differs: {actual} != {expected}"
            );
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "array length differs");
            for (actual, expected) in actual.iter().zip(expected) {
                assert_json_equivalent(actual, expected);
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(
                actual.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>()
            );
            for (key, actual) in actual {
                assert_json_equivalent(actual, &expected[key]);
            }
        }
        (actual, expected) => assert_eq!(actual, expected),
    }
}

fn assert_producer<T>(name: &str)
where
    T: DeserializeOwned + Serialize,
{
    let expected = fixture(name);
    let value: T = serde_json::from_value(expected.clone()).expect("fixture output DTO");
    let actual = serde_json::to_value(value).expect("serialize output DTO");
    assert_json_equivalent(&actual, &expected);
}

fn assert_consumer<T>(name: &str)
where
    T: DeserializeOwned,
{
    let _: T = serde_json::from_value(fixture(name)).expect("public CLI contract output");
}

#[test]
fn producer_label_add_matches_exact_fixture() {
    assert_producer::<CliLabelAddOutput>("label-add");
}

#[test]
fn label_add_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelAddOutput>("label-add");
}

#[test]
fn producer_label_atom_index_query_matches_exact_fixture() {
    assert_producer::<CliLabelAtomIndexQueryOutput>("label-atom-index-query");
}

#[test]
fn label_atom_index_query_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelAtomIndexQueryOutput>("label-atom-index-query");
}

#[test]
fn producer_label_atom_index_rebuild_matches_exact_fixture() {
    assert_producer::<CliLabelAtomIndexRebuildOutput>("label-atom-index-rebuild");
}

#[test]
fn label_atom_index_rebuild_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelAtomIndexRebuildOutput>("label-atom-index-rebuild");
}

#[test]
fn producer_label_atom_index_status_matches_exact_fixture() {
    assert_producer::<CliLabelAtomIndexStatusOutput>("label-atom-index-status");
}

#[test]
fn label_atom_index_status_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelAtomIndexStatusOutput>("label-atom-index-status");
}

#[test]
fn producer_label_atoms_explain_matches_exact_fixture() {
    assert_producer::<CliLabelAtomsExplainOutput>("label-atoms-explain");
}

#[test]
fn label_atoms_explain_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelAtomsExplainOutput>("label-atoms-explain");
}

#[test]
fn producer_label_atoms_list_matches_exact_fixture() {
    assert_producer::<CliLabelAtomsListOutput>("label-atoms-list");
}

#[test]
fn label_atoms_list_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelAtomsListOutput>("label-atoms-list");
}

#[test]
fn producer_label_bootstrap_matches_exact_fixture() {
    assert_producer::<CliLabelBootstrapOutput>("label-bootstrap");
}

#[test]
fn label_bootstrap_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelBootstrapOutput>("label-bootstrap");
}

#[test]
fn producer_label_create_matches_exact_fixture() {
    assert_producer::<CliLabelCreateOutput>("label-create");
}

#[test]
fn label_create_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelCreateOutput>("label-create");
}

#[test]
fn producer_label_delete_matches_exact_fixture() {
    assert_producer::<CliLabelDeleteOutput>("label-delete");
}

#[test]
fn label_delete_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelDeleteOutput>("label-delete");
}

#[test]
fn producer_label_list_matches_exact_fixture() {
    assert_producer::<CliLabelListOutput>("label-list");
}

#[test]
fn label_list_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelListOutput>("label-list");
}

#[test]
fn producer_label_ontology_apply_atom_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyApplyAtomOutput>("label-ontology-apply-atom");
}

#[test]
fn label_ontology_apply_atom_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyApplyAtomOutput>("label-ontology-apply-atom");
}

#[test]
fn producer_label_ontology_confirm_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyConfirmOutput>("label-ontology-confirm");
}

#[test]
fn label_ontology_confirm_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyConfirmOutput>("label-ontology-confirm");
}

#[test]
fn producer_label_ontology_list_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyListOutput>("label-ontology-list");
}

#[test]
fn label_ontology_list_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyListOutput>("label-ontology-list");
}

#[test]
fn producer_label_ontology_quality_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyQualityOutput>("label-ontology-quality");
}

#[test]
fn label_ontology_quality_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyQualityOutput>("label-ontology-quality");
}

#[test]
fn producer_label_ontology_record_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyRecordOutput>("label-ontology-record");
}

#[test]
fn label_ontology_record_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyRecordOutput>("label-ontology-record");
}

#[test]
fn producer_label_ontology_reject_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyRejectOutput>("label-ontology-reject");
}

#[test]
fn label_ontology_reject_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyRejectOutput>("label-ontology-reject");
}

#[test]
fn producer_label_ontology_resolve_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyResolveOutput>("label-ontology-resolve");
}

#[test]
fn label_ontology_resolve_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyResolveOutput>("label-ontology-resolve");
}

#[test]
fn producer_label_ontology_revert_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyRevertOutput>("label-ontology-revert");
}

#[test]
fn label_ontology_revert_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyRevertOutput>("label-ontology-revert");
}

#[test]
fn producer_label_ontology_review_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyReviewOutput>("label-ontology-review");
}

#[test]
fn label_ontology_review_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyReviewOutput>("label-ontology-review");
}

#[test]
fn producer_label_ontology_show_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyShowOutput>("label-ontology-show");
}

#[test]
fn label_ontology_show_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyShowOutput>("label-ontology-show");
}

#[test]
fn producer_label_ontology_supersede_matches_exact_fixture() {
    assert_producer::<CliLabelOntologySupersedeOutput>("label-ontology-supersede");
}

#[test]
fn label_ontology_supersede_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologySupersedeOutput>("label-ontology-supersede");
}

#[test]
fn producer_label_ontology_validate_matches_exact_fixture() {
    assert_producer::<CliLabelOntologyValidateOutput>("label-ontology-validate");
}

#[test]
fn label_ontology_validate_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelOntologyValidateOutput>("label-ontology-validate");
}

#[test]
fn producer_label_proposals_accept_matches_exact_fixture() {
    assert_producer::<CliLabelProposalsAcceptOutput>("label-proposals-accept");
}

#[test]
fn label_proposals_accept_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelProposalsAcceptOutput>("label-proposals-accept");
}

#[test]
fn producer_label_proposals_list_matches_exact_fixture() {
    assert_producer::<CliLabelProposalsListOutput>("label-proposals-list");
}

#[test]
fn label_proposals_list_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelProposalsListOutput>("label-proposals-list");
}

#[test]
fn producer_label_proposals_reject_matches_exact_fixture() {
    assert_producer::<CliLabelProposalsRejectOutput>("label-proposals-reject");
}

#[test]
fn label_proposals_reject_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelProposalsRejectOutput>("label-proposals-reject");
}

#[test]
fn producer_label_proposals_show_matches_exact_fixture() {
    assert_producer::<CliLabelProposalsShowOutput>("label-proposals-show");
}

#[test]
fn label_proposals_show_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelProposalsShowOutput>("label-proposals-show");
}

#[test]
fn producer_label_propose_matches_exact_fixture() {
    assert_producer::<CliLabelProposeOutput>("label-propose");
}

#[test]
fn label_propose_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelProposeOutput>("label-propose");
}

#[test]
fn producer_label_remove_matches_exact_fixture() {
    assert_producer::<CliLabelRemoveOutput>("label-remove");
}

#[test]
fn label_remove_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelRemoveOutput>("label-remove");
}

#[test]
fn producer_label_semantics_delete_matches_exact_fixture() {
    assert_producer::<CliLabelSemanticsDeleteOutput>("label-semantics-delete");
}

#[test]
fn label_semantics_delete_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelSemanticsDeleteOutput>("label-semantics-delete");
}

#[test]
fn producer_label_semantics_list_matches_exact_fixture() {
    assert_producer::<CliLabelSemanticsListOutput>("label-semantics-list");
}

#[test]
fn label_semantics_list_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelSemanticsListOutput>("label-semantics-list");
}

#[test]
fn producer_label_semantics_show_matches_exact_fixture() {
    assert_producer::<CliLabelSemanticsShowOutput>("label-semantics-show");
}

#[test]
fn label_semantics_show_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelSemanticsShowOutput>("label-semantics-show");
}

#[test]
fn producer_label_semantics_upsert_matches_exact_fixture() {
    assert_producer::<CliLabelSemanticsUpsertOutput>("label-semantics-upsert");
}

#[test]
fn label_semantics_upsert_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelSemanticsUpsertOutput>("label-semantics-upsert");
}

#[test]
fn producer_label_suggest_matches_exact_fixture() {
    assert_producer::<CliLabelSuggestOutput>("label-suggest");
}

#[test]
fn label_suggest_output_fixture_is_consumed_by_public_contract() {
    assert_consumer::<CliLabelSuggestOutput>("label-suggest");
}
