//! Metadata、Config 与 shared API component 的唯一 declaration source。
//!
//! Metadata/Config parent 不是 HTTP endpoint；其 `OperationDeclaration` 只冻结 surface、
//! key、operation、DTO schema、fixture、witness 与迁移状态。`api.error.response` 是
//! 可被多个 HTTP endpoint 复用的 shared component，因此单独保留一个 declaration parent
//! 仅用于生成正确的 `HttpTransport::Error` projection，不把 synthetic parent 暴露为
//! endpoint 或 host-admin operation。
//!
//! host、CLI、MCP 与 maintenance adapter 仍由各自 crate 持有；本模块只描述 wire
//! contract 事实。admin catalog 已经持有同一 standalone `api.maintenance-path.request`
//! contract，本模块不重复声明它，避免两个 source 在中央 wiring 时产生冲突。

use crate::{
    AdoptionLocator, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, EndpointDescriptor, HttpMethod, HttpTransportLocation,
    MigrationState, OperationContract, OperationDeclaration, SurfaceOperation,
};

#[cfg(test)]
use crate::ContractTransport;

const METADATA_DECISION_PRODUCER: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_history_adoption",
    exact_test: "history_cli_covers_runs_logs_comments_attachments_events_and_stats",
};
const METADATA_DECISION_CONSUMER: AdoptionLocator = METADATA_DECISION_PRODUCER;

const METADATA_SIGNAL_PRODUCER: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_label_contract_adoption",
    exact_test: "generic_signals_record_review_and_confirm_flow_through_real_cli",
};
const METADATA_SIGNAL_CONSUMER: AdoptionLocator = METADATA_SIGNAL_PRODUCER;

const METADATA_LABEL_PROPOSAL_PRODUCER: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_label_contract_adoption",
    exact_test: "labels_semantics_atoms_and_proposals_flow_through_real_cli",
};
const METADATA_LABEL_PROPOSAL_CONSUMER: AdoptionLocator = METADATA_LABEL_PROPOSAL_PRODUCER;

const METADATA_ONTOLOGY_PRODUCER: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_label_contract_adoption",
    exact_test: "ontology_observation_signal_review_and_action_flow_through_real_cli",
};
const METADATA_ONTOLOGY_CONSUMER: AdoptionLocator = METADATA_ONTOLOGY_PRODUCER;

const CONFIG_PROJECT_PRODUCER: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_config_contract_adoption",
    exact_test: "config_adoption::project_config_input_fixture_is_produced_by_runtime_config_dto",
};
const CONFIG_PROJECT_CONSUMER: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_queue_adoption",
    exact_test: "queue_cli_uses_real_host_for_config_board_and_task_commands",
};

const CONFIG_WORKER_PRODUCER: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_config_contract_adoption",
    exact_test: "config_adoption::selected_worker_profile_input_fixture_is_produced_by_runtime_config_dto",
};
const CONFIG_WORKER_CONSUMER: AdoptionLocator = AdoptionLocator {
    package: "kanban-cli",
    test_target: "cli_admin_adoption",
    exact_test: "dispatcher_profile_is_consumed_by_real_serve_and_only_claims_ready",
};

const API_ERROR_PRODUCER: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "http::operations::contract_adoption::suite_health_and_errors_use_real_router_fixtures",
};
const API_ERROR_CONSUMER: AdoptionLocator = API_ERROR_PRODUCER;

macro_rules! structured_contract {
    (
        $id:literal,
        $path:literal,
        $operation:literal,
        $direction:expr,
        $strictness:expr,
        $schema_id:literal,
        $artifact_path:literal,
        $title:literal,
        $valid_fixture:literal,
        $invalid_fixture:literal,
        $schema_type:ty,
        $producer:expr,
        $consumer:expr
    ) => {{
        let contract = ContractDeclaration::new(
            $id,
            $path,
            $direction,
            None,
            $strictness,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_schema(
            $schema_id,
            $artifact_path,
            $title,
            $valid_fixture,
            $invalid_fixture,
        )
        .with_adoption($producer, $consumer);
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! config_contract {
    (
        $id:literal,
        $path:literal,
        $operation:literal,
        $schema_id:literal,
        $artifact_path:literal,
        $title:literal,
        $valid_fixture:literal,
        $invalid_fixture:literal,
        $schema_type:ty,
        $producer:expr,
        $consumer:expr
    ) => {{
        let contract = ContractDeclaration::new(
            $id,
            $path,
            ContractDirection::Deserialize,
            None,
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_schema(
            $schema_id,
            $artifact_path,
            $title,
            $valid_fixture,
            $invalid_fixture,
        )
        .with_adoption($producer, $consumer);
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

const METADATA_DECISION_CONTRACTS: &[ContractDeclaration] = &[structured_contract!(
    "metadata.decision.input",
    "task_comments.metadata_json(kind=decision)",
    "structured decision comment metadata input",
    ContractDirection::Deserialize,
    ContractStrictness::Typed,
    "urn:kanban-tool:schema:metadata:decision:v1",
    "metadata/decision.v1.schema.json",
    "Kanban decision metadata v1",
    "schemas/fixtures/metadata/decision.v1.valid.json",
    "schemas/fixtures/metadata/decision.v1.invalid.json",
    crate::DecisionMetadata,
    METADATA_DECISION_PRODUCER,
    METADATA_DECISION_CONSUMER
)];

const METADATA_SIGNAL_RECORD_CONTRACTS: &[ContractDeclaration] = &[structured_contract!(
    "metadata.signal-record.input",
    "kanban signal record input",
    "generic signal record input",
    ContractDirection::Deserialize,
    ContractStrictness::Typed,
    "urn:kanban-tool:schema:metadata:signal-record-input:v1",
    "metadata/signal-record-input.v1.schema.json",
    "Kanban signal record metadata input v1",
    "schemas/fixtures/metadata/signal-record-input.v1.valid.json",
    "schemas/fixtures/metadata/signal-record-input.v1.invalid.json",
    crate::structured_metadata::SignalRecordMetadataInput,
    METADATA_SIGNAL_PRODUCER,
    METADATA_SIGNAL_CONSUMER
)];

const METADATA_SIGNAL_LINK_CONTRACTS: &[ContractDeclaration] = &[structured_contract!(
    "metadata.signal-link.output",
    "task comment signal backlink metadata",
    "signal backlink comment metadata output",
    ContractDirection::Serialize,
    ContractStrictness::Typed,
    "urn:kanban-tool:schema:metadata:signal-link-output:v1",
    "metadata/signal-link-output.v1.schema.json",
    "Kanban signal backlink metadata output v1",
    "schemas/fixtures/metadata/signal-link-output.v1.valid.json",
    "schemas/fixtures/metadata/signal-link-output.v1.invalid.json",
    crate::structured_metadata::SignalLinkMetadataOutput,
    METADATA_SIGNAL_PRODUCER,
    METADATA_SIGNAL_CONSUMER
)];

const METADATA_LABEL_PROPOSAL_CONTRACTS: &[ContractDeclaration] = &[structured_contract!(
    "metadata.label-proposal-candidate.input",
    "kanban label propose --proposal-json",
    "label proposal candidate input",
    ContractDirection::Deserialize,
    ContractStrictness::Typed,
    "urn:kanban-tool:schema:metadata:label-proposal-candidate-input:v1",
    "metadata/label-proposal-candidate-input.v1.schema.json",
    "Kanban label proposal candidate metadata input v1",
    "schemas/fixtures/metadata/label-proposal-candidate-input.v1.valid.json",
    "schemas/fixtures/metadata/label-proposal-candidate-input.v1.invalid.json",
    crate::structured_metadata::LabelProposalCandidateMetadataInput,
    METADATA_LABEL_PROPOSAL_PRODUCER,
    METADATA_LABEL_PROPOSAL_CONSUMER
)];

const METADATA_ONTOLOGY_RECORD_CONTRACTS: &[ContractDeclaration] = &[structured_contract!(
    "metadata.ontology-record.input",
    "kanban label ontology record input",
    "label ontology observation input",
    ContractDirection::Deserialize,
    ContractStrictness::Typed,
    "urn:kanban-tool:schema:metadata:ontology-record-input:v1",
    "metadata/ontology-record-input.v1.schema.json",
    "Kanban label ontology record metadata input v1",
    "schemas/fixtures/metadata/ontology-record-input.v1.valid.json",
    "schemas/fixtures/metadata/ontology-record-input.v1.invalid.json",
    crate::structured_metadata::OntologyRecordMetadataInput,
    METADATA_ONTOLOGY_PRODUCER,
    METADATA_ONTOLOGY_CONSUMER
)];

const METADATA_ONTOLOGY_EVIDENCE_CONTRACTS: &[ContractDeclaration] = &[structured_contract!(
    "metadata.ontology-validation-evidence.input",
    "kanban label ontology validate external evidence",
    "label ontology external validation evidence",
    ContractDirection::Deserialize,
    ContractStrictness::OpaqueExtension,
    "urn:kanban-tool:schema:metadata:ontology-validation-evidence-input:v1",
    "metadata/ontology-validation-evidence-input.v1.schema.json",
    "Kanban label ontology validation evidence metadata input v1",
    "schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json",
    "schemas/fixtures/metadata/ontology-validation-evidence-input.v1.invalid.json",
    crate::structured_metadata::OntologyValidationEvidenceMetadataInput,
    METADATA_ONTOLOGY_PRODUCER,
    METADATA_ONTOLOGY_CONSUMER
)];

const CONFIG_PROJECT_CONTRACTS: &[ContractDeclaration] = &[config_contract!(
    "config.project.input",
    ".kb/config.toml",
    "project-local config after TOML decoding",
    "urn:kanban-tool:schema:config:project-input:v1",
    "config/project-input.v1.schema.json",
    "Project Config Input v1",
    "schemas/fixtures/config/project-input.v1.valid.json",
    "schemas/fixtures/config/project-input.v1.invalid.json",
    crate::ProjectConfigInput,
    CONFIG_PROJECT_PRODUCER,
    CONFIG_PROJECT_CONSUMER
)];

const CONFIG_WORKER_CONTRACTS: &[ContractDeclaration] = &[config_contract!(
    "config.selected-worker-profile.input",
    "selected [workers.<profile>] section",
    "selected dispatcher worker profile after TOML decoding",
    "urn:kanban-tool:schema:config:selected-worker-profile-input:v1",
    "config/selected-worker-profile-input.v1.schema.json",
    "Selected Worker Profile Input v1",
    "schemas/fixtures/config/selected-worker-profile-input.v1.valid.json",
    "schemas/fixtures/config/selected-worker-profile-input.v1.invalid.json",
    crate::WorkerProfileInput,
    CONFIG_WORKER_PRODUCER,
    CONFIG_WORKER_CONSUMER
)];

const METADATA_CONFIG_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "metadata.decision",
        ContractSurface::Metadata,
        None,
        None,
        "structured decision comment metadata input",
        "structured decision comment metadata input",
        MigrationState::Adopted,
        METADATA_DECISION_CONTRACTS,
    ),
    OperationDeclaration::new(
        "metadata.signal-record",
        ContractSurface::Metadata,
        None,
        None,
        "generic signal record input",
        "generic signal record input",
        MigrationState::Adopted,
        METADATA_SIGNAL_RECORD_CONTRACTS,
    ),
    OperationDeclaration::new(
        "metadata.signal-link",
        ContractSurface::Metadata,
        None,
        None,
        "signal backlink comment metadata output",
        "signal backlink comment metadata output",
        MigrationState::Adopted,
        METADATA_SIGNAL_LINK_CONTRACTS,
    ),
    OperationDeclaration::new(
        "metadata.label-proposal-candidate",
        ContractSurface::Metadata,
        None,
        None,
        "label proposal candidate input",
        "label proposal candidate input",
        MigrationState::Adopted,
        METADATA_LABEL_PROPOSAL_CONTRACTS,
    ),
    OperationDeclaration::new(
        "metadata.ontology-record",
        ContractSurface::Metadata,
        None,
        None,
        "label ontology observation input",
        "label ontology observation input",
        MigrationState::Adopted,
        METADATA_ONTOLOGY_RECORD_CONTRACTS,
    ),
    OperationDeclaration::new(
        "metadata.ontology-validation-evidence",
        ContractSurface::Metadata,
        None,
        None,
        "label ontology external validation evidence",
        "label ontology external validation evidence",
        MigrationState::Adopted,
        METADATA_ONTOLOGY_EVIDENCE_CONTRACTS,
    ),
    OperationDeclaration::new(
        "config.project",
        ContractSurface::Config,
        None,
        None,
        "project-local config after TOML decoding",
        "project-local config after TOML decoding",
        MigrationState::Adopted,
        CONFIG_PROJECT_CONTRACTS,
    ),
    OperationDeclaration::new(
        "config.selected-worker-profile",
        ContractSurface::Config,
        None,
        None,
        "selected dispatcher worker profile after TOML decoding",
        "selected dispatcher worker profile after TOML decoding",
        MigrationState::Adopted,
        CONFIG_WORKER_CONTRACTS,
    ),
];

const SHARED_API_ERROR_CONTRACTS: &[ContractDeclaration] = &[{
    let contract = ContractDeclaration::new(
        "api.error.response",
        "Shared API error response component",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Error),
        ContractStrictness::DenyUnknownFields,
        ContractGranularity::Exact,
        ContractBinding::SharedComponent,
    )
    .with_operation("stable API error envelope")
    .with_transport(None, &[])
    .with_schema(
        "urn:kanban-tool:schema:api:error-response:v1",
        "api/error-response.v1.schema.json",
        "Kanban API error response v1",
        "schemas/fixtures/api/error-response.v1.valid.json",
        "schemas/fixtures/api/error-response.v1.invalid.json",
    )
    .with_adoption(API_ERROR_PRODUCER, API_ERROR_CONSUMER);
    #[cfg(feature = "schema")]
    let contract = contract.with_schema_type::<crate::ErrorEnvelope>();
    contract
}];

// 该 parent 只为 `ContractDeclaration::operation_contract` 提供 Error transport 所需的
// HTTP method/path；它不是 endpoint source，也不会进入 `operation_declarations()`。
const SHARED_API_ERROR_PARENT: OperationDeclaration = OperationDeclaration::new(
    "api.error.shared",
    ContractSurface::Api,
    Some(HttpMethod::Get),
    Some("/api/v1/boards/:board/tasks"),
    "GET /api/v1/boards/:board/tasks",
    "GET /api/v1/boards/:board/tasks",
    MigrationState::Adopted,
    SHARED_API_ERROR_CONTRACTS,
);

/// 返回 Metadata 与 Config 的 parent declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    METADATA_CONFIG_OPERATIONS
}

/// 返回 Metadata 与 Config 的 child projection，顺序与 source 一致。
pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(METADATA_CONFIG_OPERATIONS).contracts()
}

/// 返回 shared API error component 的 child projection。
pub fn shared_component_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(&[SHARED_API_ERROR_PARENT]).contracts()
}

/// 返回 source 中 non-HTTP surface projection。
pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(METADATA_CONFIG_OPERATIONS).surfaces()
}

/// Metadata/Config source 没有 endpoint；此函数用于让中央 wiring 明确保持空投影。
pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    Vec::new()
}

/// 查找 source 中任意 contract 的 operation projection。
pub fn operation_contract(id: &str) -> Option<OperationContract> {
    operation_contracts()
        .into_iter()
        .find(|contract| contract.id == id)
        .or_else(|| {
            shared_component_contracts()
                .into_iter()
                .find(|contract| contract.id == id)
        })
}

/// 判断 contract 是否由本 family 声明（含 shared error）。
pub fn owns_contract(id: &str) -> bool {
    operation_contract(id).is_some()
}

/// 保留旧 helper 名称，供只关心 shared component 的调用方使用。
pub fn contract(id: &str) -> Option<OperationContract> {
    operation_contract(id)
}

/// 返回 source 中显式 schema roots。
#[cfg(feature = "schema")]
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    let mut roots = crate::CatalogProjection::new(METADATA_CONFIG_OPERATIONS).schemas();
    roots.extend(crate::CatalogProjection::new(&[SHARED_API_ERROR_PARENT]).schemas());
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_and_config_source_has_no_endpoint_parent() {
        assert_eq!(operation_declarations().len(), 8);
        assert!(
            operation_declarations()
                .iter()
                .all(|operation| operation.method.is_none() && operation.path.is_none())
        );
        assert!(endpoint_catalog().is_empty());
        assert_eq!(operation_contracts().len(), 8);
    }

    #[test]
    fn source_preserves_non_http_contract_facts() {
        for contract in operation_contracts() {
            assert!(matches!(
                contract.surface,
                ContractSurface::Metadata | ContractSurface::Config
            ));
            assert_eq!(contract.transport, ContractTransport::NoTransport);
            assert_eq!(contract.migration, MigrationState::Adopted);
            assert_eq!(contract.binding, ContractBinding::ExactSurface);
            assert!(contract.schema_id.is_some());
            assert!(contract.fixture.is_some());
            assert!(contract.adoption.is_some());
        }
    }

    #[test]
    fn shared_error_keeps_error_transport_without_endpoint_parent() {
        let contract = contract("api.error.response").expect("shared API error");
        assert_eq!(contract.surface, ContractSurface::Api);
        assert_eq!(contract.path, "Shared API error response component");
        assert_eq!(contract.operation, "stable API error envelope");
        assert_eq!(contract.direction, ContractDirection::Serialize);
        assert_eq!(contract.binding, ContractBinding::SharedComponent);
        assert_eq!(
            contract.transport,
            ContractTransport::Http {
                operation_key: None,
                location: HttpTransportLocation::Error,
                parameters: &[],
            }
        );
    }

    #[cfg(feature = "schema")]
    #[test]
    fn schema_roots_have_unique_contract_ids() {
        let roots = schema_roots();
        assert_eq!(roots.len(), 9);
        let mut ids = roots
            .iter()
            .map(|root| root.contract_id)
            .collect::<Vec<_>>();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }
}
