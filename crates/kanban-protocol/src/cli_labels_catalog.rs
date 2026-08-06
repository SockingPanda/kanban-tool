//! Labels、runs、search、serve 与 signals CLI surface 的唯一声明源。
//!
//! 该连续区段按 `surface.rs::non_transport_operations` 的历史顺序保存每个 CLI
//! parent 及其 `--json` output child。legacy inventory、schema artifact 与 fixture
//! 仍是兼容基线；本模块只声明协议事实，并由中央 projection 负责接入既有形状。

use crate::{
    AdoptionLocator, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, MigrationState, OperationContract, OperationDeclaration,
    SurfaceOperation,
};

const LABEL_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_label_contract_adoption",
    exact_test: "labels_semantics_atoms_and_proposals_flow_through_real_cli",
};
const LABEL_BOOTSTRAP_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "label_contract_adoption",
    exact_test: "bootstrap_label_flow_through_real_cli",
};

const LABEL_ONTOLOGY_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_label_contract_adoption",
    exact_test: "ontology_observation_signal_review_and_action_flow_through_real_cli",
};

const HISTORY_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_history_adoption",
    exact_test: "history_cli_covers_runs_logs_comments_attachments_events_and_stats",
};

const SEARCH_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_queue_adoption",
    exact_test: "queue_cli_uses_real_host_for_config_board_and_task_commands",
};

const SIGNAL_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_label_contract_adoption",
    exact_test: "generic_signals_record_review_and_confirm_flow_through_real_cli",
};

macro_rules! cli_output_contract {
    (
        $slug:literal,
        $operation:literal,
        $title:literal,
        $schema_type:ty,
        $witness:expr
    ) => {{
        let contract = ContractDeclaration::new(
            concat!("cli.", $slug, ".output"),
            concat!("kanban ", $operation, " --json stdout"),
            ContractDirection::Serialize,
            None,
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_schema(
            concat!("urn:kanban-tool:schema:cli:", $slug, "-output:v1"),
            concat!("cli/", $slug, "-output.v1.schema.json"),
            $title,
            concat!("schemas/fixtures/cli/", $slug, "-output.v1.valid.json"),
            concat!("schemas/fixtures/cli/", $slug, "-output.v1.invalid.json"),
        )
        .with_adoption($witness, $witness);
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! cli_operation {
    (
        $slug:literal,
        $operation:literal,
        $title:literal,
        $schema_type:ty,
        $witness:expr
    ) => {{
        const CONTRACTS: &[ContractDeclaration] = &[cli_output_contract!(
            $slug,
            $operation,
            $title,
            $schema_type,
            $witness
        )];
        OperationDeclaration::new(
            concat!("cli.", $slug),
            ContractSurface::Cli,
            None,
            None,
            $operation,
            $operation,
            MigrationState::Adopted,
            CONTRACTS,
        )
    }};
}

const CLI_LABELS_OPERATIONS: &[OperationDeclaration] = &[
    cli_operation!(
        "label-add",
        "label add",
        "Kanban CLI label add output v1",
        crate::cli_labels::CliLabelAddOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-bootstrap",
        "label bootstrap",
        "Kanban CLI label bootstrap output v1",
        crate::cli_labels::CliLabelBootstrapOutput,
        LABEL_BOOTSTRAP_WITNESS
    ),
    cli_operation!(
        "label-atom-index-query",
        "label atom-index query",
        "Kanban CLI label atom-index query output v1",
        crate::cli_labels::CliLabelAtomIndexQueryOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-atom-index-rebuild",
        "label atom-index rebuild",
        "Kanban CLI label atom-index rebuild output v1",
        crate::cli_labels::CliLabelAtomIndexRebuildOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-atom-index-status",
        "label atom-index status",
        "Kanban CLI label atom-index status output v1",
        crate::cli_labels::CliLabelAtomIndexStatusOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-atoms-explain",
        "label atoms explain",
        "Kanban CLI label atoms explain output v1",
        crate::cli_labels::CliLabelAtomsExplainOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-atoms-list",
        "label atoms list",
        "Kanban CLI label atoms list output v1",
        crate::cli_labels::CliLabelAtomsListOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-create",
        "label create",
        "Kanban CLI label create output v1",
        crate::cli_labels::CliLabelCreateOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-delete",
        "label delete",
        "Kanban CLI label delete output v1",
        crate::cli_labels::CliLabelDeleteOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-list",
        "label list",
        "Kanban CLI label list output v1",
        crate::cli_labels::CliLabelListOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-ontology-apply-atom",
        "label ontology apply atom",
        "Kanban CLI label ontology apply atom output v1",
        crate::cli_labels::CliLabelOntologyApplyAtomOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-confirm",
        "label ontology confirm",
        "Kanban CLI label ontology confirm output v1",
        crate::cli_labels::CliLabelOntologyConfirmOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-list",
        "label ontology list",
        "Kanban CLI label ontology list output v1",
        crate::cli_labels::CliLabelOntologyListOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-quality",
        "label ontology quality",
        "Kanban CLI label ontology quality output v1",
        crate::cli_labels::CliLabelOntologyQualityOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-record",
        "label ontology record",
        "Kanban CLI label ontology record output v1",
        crate::cli_labels::CliLabelOntologyRecordOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-reject",
        "label ontology reject",
        "Kanban CLI label ontology reject output v1",
        crate::cli_labels::CliLabelOntologyRejectOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-resolve",
        "label ontology resolve",
        "Kanban CLI label ontology resolve output v1",
        crate::cli_labels::CliLabelOntologyResolveOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-revert",
        "label ontology revert",
        "Kanban CLI label ontology revert output v1",
        crate::cli_labels::CliLabelOntologyRevertOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-review",
        "label ontology review",
        "Kanban CLI label ontology review output v1",
        crate::cli_labels::CliLabelOntologyReviewOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-show",
        "label ontology show",
        "Kanban CLI label ontology show output v1",
        crate::cli_labels::CliLabelOntologyShowOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-supersede",
        "label ontology supersede",
        "Kanban CLI label ontology supersede output v1",
        crate::cli_labels::CliLabelOntologySupersedeOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-ontology-validate",
        "label ontology validate",
        "Kanban CLI label ontology validate output v1",
        crate::cli_labels::CliLabelOntologyValidateOutput,
        LABEL_ONTOLOGY_WITNESS
    ),
    cli_operation!(
        "label-proposals-accept",
        "label proposals accept",
        "Kanban CLI label proposals accept output v1",
        crate::cli_labels::CliLabelProposalsAcceptOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-proposals-list",
        "label proposals list",
        "Kanban CLI label proposals list output v1",
        crate::cli_labels::CliLabelProposalsListOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-proposals-reject",
        "label proposals reject",
        "Kanban CLI label proposals reject output v1",
        crate::cli_labels::CliLabelProposalsRejectOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-proposals-show",
        "label proposals show",
        "Kanban CLI label proposals show output v1",
        crate::cli_labels::CliLabelProposalsShowOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-propose",
        "label propose",
        "Kanban CLI label propose output v1",
        crate::cli_labels::CliLabelProposeOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-remove",
        "label remove",
        "Kanban CLI label remove output v1",
        crate::cli_labels::CliLabelRemoveOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-semantics-delete",
        "label semantics delete",
        "Kanban CLI label semantics delete output v1",
        crate::cli_labels::CliLabelSemanticsDeleteOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-semantics-list",
        "label semantics list",
        "Kanban CLI label semantics list output v1",
        crate::cli_labels::CliLabelSemanticsListOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-semantics-show",
        "label semantics show",
        "Kanban CLI label semantics show output v1",
        crate::cli_labels::CliLabelSemanticsShowOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-semantics-upsert",
        "label semantics upsert",
        "Kanban CLI label semantics upsert output v1",
        crate::cli_labels::CliLabelSemanticsUpsertOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "label-suggest",
        "label suggest",
        "Kanban CLI label suggest output v1",
        crate::cli_labels::CliLabelSuggestOutput,
        LABEL_WITNESS
    ),
    cli_operation!(
        "run-logs",
        "run logs",
        "Kanban CLI run logs output v1",
        crate::CliRunLogsOutput,
        HISTORY_WITNESS
    ),
    cli_operation!(
        "run-show",
        "run show",
        "Kanban CLI run show output v1",
        crate::CliRunShowOutput,
        HISTORY_WITNESS
    ),
    cli_operation!(
        "runs",
        "runs",
        "Kanban CLI runs output v1",
        crate::CliRunsOutput,
        HISTORY_WITNESS
    ),
    cli_operation!(
        "search",
        "search",
        "Kanban CLI search output v1",
        crate::cli_helpers::CliSearchOutput,
        SEARCH_WITNESS
    ),
    OperationDeclaration::new(
        "cli.serve",
        ContractSurface::Cli,
        None,
        None,
        "serve",
        "serve",
        MigrationState::Excluded,
        &[],
    )
    .with_exclusion("daemon lifecycle 不产生有限 JSON document；运行诊断固定写 stderr"),
    cli_operation!(
        "signal-confirm",
        "signal confirm",
        "Kanban CLI signal confirm output v1",
        crate::cli_operator::CliSignalConfirmOutput,
        SIGNAL_WITNESS
    ),
    cli_operation!(
        "signal-list",
        "signal list",
        "Kanban CLI signal list output v1",
        crate::cli_operator::CliSignalListOutput,
        SIGNAL_WITNESS
    ),
    cli_operation!(
        "signal-record",
        "signal record",
        "Kanban CLI signal record output v1",
        crate::cli_operator::CliSignalRecordOutput,
        SIGNAL_WITNESS
    ),
    cli_operation!(
        "signal-reject",
        "signal reject",
        "Kanban CLI signal reject output v1",
        crate::cli_operator::CliSignalRejectOutput,
        SIGNAL_WITNESS
    ),
    cli_operation!(
        "signal-resolve",
        "signal resolve",
        "Kanban CLI signal resolve output v1",
        crate::cli_operator::CliSignalResolveOutput,
        SIGNAL_WITNESS
    ),
    cli_operation!(
        "signal-review",
        "signal review",
        "Kanban CLI signal review output v1",
        crate::cli_operator::CliSignalReviewOutput,
        SIGNAL_WITNESS
    ),
    cli_operation!(
        "signal-show",
        "signal show",
        "Kanban CLI signal show output v1",
        crate::cli_operator::CliSignalShowOutput,
        SIGNAL_WITNESS
    ),
    cli_operation!(
        "signal-supersede",
        "signal supersede",
        "Kanban CLI signal supersede output v1",
        crate::cli_operator::CliSignalSupersedeOutput,
        SIGNAL_WITNESS
    ),
];

/// 返回 labels CLI 连续区段的 parent declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    CLI_LABELS_OPERATIONS
}

/// 返回该 source 的全部 output contract，保留 parent/child 顺序。
pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(CLI_LABELS_OPERATIONS).contracts()
}

/// 返回该 source 的 surface projection，包含被排除的 `serve` parent。
pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(CLI_LABELS_OPERATIONS).surfaces()
}

#[cfg(feature = "schema")]
/// 返回该 source 的显式 schema roots。
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    crate::CatalogProjection::new(CLI_LABELS_OPERATIONS).schemas()
}

/// 判断 contract 是否由该 source 声明。
pub fn owns_contract(id: &str) -> bool {
    CLI_LABELS_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

/// 判断 operation parent 是否由该 source 声明。
pub fn owns_operation(id: &str) -> bool {
    CLI_LABELS_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_labels_source_preserves_contiguous_surface_order() {
        let operations = operation_declarations();
        assert_eq!(operations.len(), 45);
        assert_eq!(
            operations.first().map(|operation| operation.key),
            Some("label add")
        );
        assert_eq!(
            operations.last().map(|operation| operation.key),
            Some("signal supersede")
        );
        assert_eq!(operations[36].operation_id, "cli.serve");
        assert_eq!(operations[36].migration, MigrationState::Excluded);
        assert_eq!(
            operations[36].exclusion,
            Some("daemon lifecycle 不产生有限 JSON document；运行诊断固定写 stderr")
        );
    }

    #[test]
    fn cli_labels_source_has_unique_contracts_and_projects_legacy_rows_once() {
        let contracts = operation_contracts();
        assert_eq!(contracts.len(), 44);
        let mut ids = contracts
            .iter()
            .map(|contract| contract.id)
            .collect::<Vec<_>>();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);

        let inventory = crate::operation_inventory();
        for source in contracts {
            let matches = inventory
                .iter()
                .filter(|contract| contract.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "CLI contract must be projected once: {}",
                source.id
            );
            assert_eq!(matches[0], &source, "CLI contract changed: {}", source.id);
        }
    }

    #[cfg(feature = "schema")]
    #[test]
    fn cli_labels_schema_projection_matches_committed_artifacts() {
        let generated = crate::schema::generated_artifacts();
        let registry = crate::schema::schema_registry();
        for source in schema_roots() {
            let matches = registry
                .iter()
                .filter(|root| root.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "CLI schema root must be projected once: {}",
                source.id
            );
            let root = matches[0];
            assert_eq!(root.artifact_path, source.artifact_path);
            assert_eq!(root.title, source.title);
            assert_eq!(root.contract_id, source.contract_id);
            assert_eq!(root.direction, source.direction);
            assert_eq!(root.strictness, source.strictness);
            assert_eq!(root.valid_fixture, source.valid_fixture);
            assert_eq!(root.invalid_fixture, source.invalid_fixture);
            let actual = generated
                .get(root.artifact_path)
                .unwrap_or_else(|| panic!("missing generated CLI artifact {}", root.artifact_path));
            let committed_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/json-schema/draft-2020-12")
                .join(root.artifact_path);
            let committed = std::fs::read(&committed_path)
                .unwrap_or_else(|error| panic!("read {}: {error}", committed_path.display()));
            assert_eq!(
                actual, &committed,
                "CLI artifact bytes changed: {}",
                root.artifact_path
            );
        }
    }
}
