//! Labels、ontology 与 signals API family 的唯一 declaration source。
//!
//! 该 family 的 parent/child declaration 同时保存 endpoint、header、schema、fixture、
//! adoption locator 和 MCP binding。真实 handler/client/MCP adapter 仍由各自 crate
//! 持有；本模块只提供协议事实及其 deterministic projection。

use crate::{
    AdoptionLocator, ApiHeaderProfile, ContractBinding, ContractDeclaration, ContractDirection,
    ContractGranularity, ContractStrictness, ContractSurface, EndpointDescriptor, HttpMethod,
    HttpTransportLocation, McpExposure, McpPolicy, McpToolBinding, MigrationState,
    OperationContract, OperationDeclaration, SurfaceOperation, WireParameter,
    WireParameterCardinality,
};

const BOARD_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const DELETE_BOARD_LABEL_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "label_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
];
const BOARD_LABEL_PROPOSALS_QUERY_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "status",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];

const LABEL_SEMANTICS_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "label_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
];

const LABEL_ATOM_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "atom_ref",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
];

const TASK_LABEL_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

const REMOVE_TASK_LABEL_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "task_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "label_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
];

const SIGNAL_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "signal_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

const PROPOSAL_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "proposal_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

const DOMAIN_INVARIANTS: &[crate::McpOperationInvariant] = &[
    crate::McpOperationInvariant::CanonicalHostOnly,
    crate::McpOperationInvariant::SharedApplicationService,
    crate::McpOperationInvariant::NoHostAdminSurface,
];

const LABEL_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::labels_semantics_and_atoms_use_committed_fixtures_through_host",
};
const LABEL_DELETE_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "suite::labels_adoption::delete_board_label_response_fixture_is_produced_by_real_router",
};
const SIGNAL_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::signal_routes_consume_record_list_show_and_review_fixtures",
};
const ONTOLOGY_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::ontology_ledger_routes_consume_observation_and_action_fixtures",
};
const PROPOSAL_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::label_proposal_routes_consume_typed_fixtures_and_persist_real_proposal",
};
const HEADER_LOCALE_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::locale_header_fixture_is_consumed_by_real_router",
};
const HEADER_ACTOR_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::locale_actor_header_fixture_is_consumed_by_real_router",
};
const HEADER_JSON_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::locale_json_header_fixture_is_consumed_by_real_router",
};
const HEADER_ACTOR_JSON_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::locale_actor_json_header_fixture_is_consumed_by_real_router",
};
const HEADER_OPTIONAL_JSON_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::locale_actor_optional_json_header_fixture_is_consumed_by_real_router",
};

macro_rules! api_contract {
    (
        $id:expr,
        $path:expr,
        $operation:expr,
        $direction:expr,
        $location:expr,
        $parameters:expr,
        $schema_slug:literal,
        $title:literal,
        $schema_type:ty,
        $witness:expr
    ) => {{
        let contract = ContractDeclaration::new(
            $id,
            $path,
            $direction,
            Some($location),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_operation($operation)
        .with_transport(None, $parameters)
        .with_schema(
            concat!("urn:kanban-tool:schema:api:", $schema_slug, ":v1"),
            concat!("api/", $schema_slug, ".v1.schema.json"),
            $title,
            concat!("schemas/fixtures/api/", $schema_slug, ".v1.valid.json"),
            concat!("schemas/fixtures/api/", $schema_slug, ".v1.invalid.json"),
        )
        .with_adoption($witness, $witness);
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! path_contract {
    ($id:expr, $operation:literal, $schema:literal, $title:literal, $parameters:expr, $ty:ty, $witness:expr) => {
        api_contract!(
            $id,
            concat!($operation, " path"),
            $operation,
            ContractDirection::Deserialize,
            HttpTransportLocation::Path,
            $parameters,
            $schema,
            $title,
            $ty,
            $witness
        )
    };
}

macro_rules! query_contract {
    ($id:expr, $operation:literal, $schema:literal, $title:literal, $ty:ty, $witness:expr) => {
        api_contract!(
            $id,
            concat!($operation, " query"),
            $operation,
            ContractDirection::Deserialize,
            HttpTransportLocation::Query,
            &[],
            $schema,
            $title,
            $ty,
            $witness
        )
    };
}

macro_rules! body_contract {
    ($id:expr, $operation:literal, $suffix:literal, $schema:literal, $title:literal, $ty:ty, $witness:expr) => {
        api_contract!(
            $id,
            concat!($operation, " ", $suffix),
            $operation,
            ContractDirection::Deserialize,
            HttpTransportLocation::Body,
            &[],
            $schema,
            $title,
            $ty,
            $witness
        )
    };
}

macro_rules! response_contract {
    ($id:expr, $operation:literal, $suffix:literal, $schema:literal, $title:literal, $ty:ty, $witness:expr) => {
        api_contract!(
            $id,
            concat!($operation, " ", $suffix),
            $operation,
            ContractDirection::Serialize,
            HttpTransportLocation::Success,
            &[],
            $schema,
            $title,
            $ty,
            $witness
        )
    };
}

macro_rules! header_contract {
    ($operation_id:literal, $operation:literal, $profile:expr, $profile_slug:literal, $ty:ty) => {{
        let contract = ContractDeclaration::new(
            concat!("api.", $operation_id, ".headers"),
            concat!($operation, " headers"),
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, $profile.parameters())
        .with_schema(
            concat!("urn:kanban-tool:schema:api:", $operation_id, "-headers:v1"),
            concat!("api/", $operation_id, "-headers.v1.schema.json"),
            concat!("Kanban api.", $operation_id, " request headers v1"),
            concat!(
                "schemas/fixtures/api/headers/",
                $profile_slug,
                ".v1.valid.json"
            ),
            concat!(
                "schemas/fixtures/api/headers/",
                $profile_slug,
                ".v1.invalid.json"
            ),
        )
        .with_adoption(
            match $profile {
                ApiHeaderProfile::Locale => HEADER_LOCALE_WITNESS,
                ApiHeaderProfile::LocaleActor => HEADER_ACTOR_WITNESS,
                ApiHeaderProfile::LocaleJson => HEADER_JSON_WITNESS,
                ApiHeaderProfile::LocaleActorJson => HEADER_ACTOR_JSON_WITNESS,
                ApiHeaderProfile::LocaleActorOptionalJson => HEADER_OPTIONAL_JSON_WITNESS,
            },
            match $profile {
                ApiHeaderProfile::Locale => HEADER_LOCALE_WITNESS,
                ApiHeaderProfile::LocaleActor => HEADER_ACTOR_WITNESS,
                ApiHeaderProfile::LocaleJson => HEADER_JSON_WITNESS,
                ApiHeaderProfile::LocaleActorJson => HEADER_ACTOR_JSON_WITNESS,
                ApiHeaderProfile::LocaleActorOptionalJson => HEADER_OPTIONAL_JSON_WITNESS,
            },
        );
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$ty>();
        contract
    }};
}

macro_rules! policy {
    ($tool:literal, [$($operation:literal),+ $(,)?]) => {
        McpPolicy {
            exposure: McpExposure::Domain,
            tool_bindings: &[McpToolBinding {
                tool_name: $tool,
                http_operations: &[$($operation),+],
            }],
            invariants: DOMAIN_INVARIANTS,
        }
    };
    ($($binding:expr),+ $(,)?) => {
        McpPolicy {
            exposure: McpExposure::Domain,
            tool_bindings: &[$($binding),+],
            invariants: DOMAIN_INVARIANTS,
        }
    };
}

macro_rules! label_adoption {
    ($test:literal) => {
        AdoptionLocator {
            package: "kanban-server",
            test_target: "lib",
            exact_test: $test,
        }
    };
}

const API_LIST_TASK_LABELS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.list-task-labels.path",
        "GET /api/v1/tasks/:task_id/labels",
        "list-task-labels-path",
        "Kanban task labels path v1",
        TASK_LABEL_PATH_PARAMETERS,
        crate::ListTaskLabelsPath,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::list_task_labels_path_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::list_task_labels_path_fixture_is_consumed_by_real_router"
        ),
    ),
    header_contract!(
        "list-task-labels",
        "GET /api/v1/tasks/:task_id/labels",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.list-task-labels.response",
        "GET /api/v1/tasks/:task_id/labels",
        "response",
        "list-task-labels-response",
        "Kanban list task labels response v1",
        crate::ListTaskLabelsResponse,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::list_task_labels_response_fixture_is_produced_by_real_router"
        ),
        label_adoption!(
            "suite::labels_adoption::list_task_labels_response_fixture_is_consumed_by_contract_root"
        ),
    ),
];

const API_ADD_TASK_LABEL_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.add-task-label.path",
        "POST /api/v1/tasks/:task_id/labels",
        "add-task-label-path",
        "Kanban add task label path v1",
        TASK_LABEL_PATH_PARAMETERS,
        crate::AddTaskLabelPath,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::add_task_label_path_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::add_task_label_path_fixture_is_consumed_by_real_router"
        ),
    ),
    header_contract!(
        "add-task-label",
        "POST /api/v1/tasks/:task_id/labels",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers",
        crate::headers::LocaleActorJsonHeaders
    ),
    body_contract!(
        "api.add-task-label.request",
        "POST /api/v1/tasks/:task_id/labels",
        "request",
        "add-task-label-request",
        "Kanban add task label request v1",
        crate::AddTaskLabelRequest,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::add_task_label_request_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::add_task_label_request_fixture_is_consumed_by_real_router"
        ),
    ),
    response_contract!(
        "api.add-task-label.response",
        "POST /api/v1/tasks/:task_id/labels",
        "response",
        "add-task-label-response",
        "Kanban add task label response v1",
        crate::AddTaskLabelResponse,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::add_task_label_response_fixture_is_produced_by_real_router"
        ),
        label_adoption!(
            "suite::labels_adoption::add_task_label_response_fixture_is_consumed_by_contract_root"
        ),
    ),
];

const API_REMOVE_TASK_LABEL_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.remove-task-label.path",
        "DELETE /api/v1/tasks/:task_id/labels/:label_id",
        "remove-task-label-path",
        "Kanban remove task label path v1",
        REMOVE_TASK_LABEL_PATH_PARAMETERS,
        crate::RemoveTaskLabelPath,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::remove_task_label_path_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::remove_task_label_path_fixture_is_consumed_by_real_router"
        ),
    ),
    header_contract!(
        "remove-task-label",
        "DELETE /api/v1/tasks/:task_id/labels/:label_id",
        ApiHeaderProfile::LocaleActor,
        "locale-actor-headers",
        crate::headers::LocaleActorHeaders
    ),
    response_contract!(
        "api.remove-task-label.response",
        "DELETE /api/v1/tasks/:task_id/labels/:label_id",
        "response",
        "remove-task-label-response",
        "Kanban remove task label response v1",
        crate::RemoveTaskLabelResponse,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::remove_task_label_response_fixture_is_produced_by_real_router"
        ),
        label_adoption!(
            "suite::labels_adoption::remove_task_label_response_fixture_is_consumed_by_contract_root"
        ),
    ),
];

const API_LIST_BOARD_LABELS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.list-board-labels.path",
        "GET /api/v1/boards/:board/labels",
        "list-board-labels-path",
        "List Board Labels Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::list_board_labels_path_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::list_board_labels_path_fixture_is_consumed_by_real_router"
        ),
    ),
    header_contract!(
        "list-board-labels",
        "GET /api/v1/boards/:board/labels",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.list-board-labels.response",
        "GET /api/v1/boards/:board/labels",
        "success",
        "list-board-labels-response",
        "List board labels response v1",
        crate::ListBoardLabelsResponse,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::list_board_labels_response_fixture_is_produced_by_real_router"
        ),
        label_adoption!(
            "suite::labels_adoption::list_board_labels_response_fixture_is_consumed_by_contract_root"
        ),
    ),
];

const API_CREATE_BOARD_LABEL_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.create-board-label.path",
        "POST /api/v1/boards/:board/labels",
        "create-board-label-path",
        "Create Board Label Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::create_board_label_path_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::create_board_label_path_fixture_is_consumed_by_real_router"
        ),
    ),
    header_contract!(
        "create-board-label",
        "POST /api/v1/boards/:board/labels",
        ApiHeaderProfile::LocaleJson,
        "locale-json-headers",
        crate::headers::LocaleJsonHeaders
    ),
    body_contract!(
        "api.create-board-label.request",
        "POST /api/v1/boards/:board/labels",
        "body",
        "create-board-label-request",
        "Create board label request v1",
        crate::CreateBoardLabelRequest,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::create_board_label_request_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::create_board_label_request_fixture_is_consumed_by_real_router"
        ),
    ),
    response_contract!(
        "api.create-board-label.response",
        "POST /api/v1/boards/:board/labels",
        "success",
        "create-board-label-response",
        "Create board label response v1",
        crate::CreateBoardLabelResponse,
        LABEL_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::create_board_label_response_fixture_is_produced_by_real_router"
        ),
        label_adoption!(
            "suite::labels_adoption::create_board_label_response_fixture_is_consumed_by_contract_root"
        ),
    ),
];

const API_DELETE_BOARD_LABEL_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.delete-board-label.path",
        "DELETE /api/v1/boards/:board/labels/:label_id",
        "delete-board-label-path",
        "Delete Board Label Path v1",
        DELETE_BOARD_LABEL_PATH_PARAMETERS,
        crate::DeleteBoardLabelPath,
        LABEL_DELETE_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::delete_board_label_path_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::delete_board_label_path_fixture_is_consumed_by_real_router"
        ),
    ),
    api_contract!(
        "api.delete-board-label.query",
        "DELETE /api/v1/boards/:board/labels/:label_id query",
        "DELETE /api/v1/boards/:board/labels/:label_id",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        &[WireParameter {
            name: "force",
            cardinality: Some(WireParameterCardinality::OptionalOne),
        }],
        "delete-board-label-query",
        "Delete Board Label Query v1",
        crate::DeleteBoardLabelQuery,
        LABEL_DELETE_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::delete_board_label_query_dto_serializes_to_committed_fixture"
        ),
        label_adoption!(
            "suite::labels_adoption::delete_board_label_query_fixture_is_consumed_by_real_router"
        ),
    ),
    header_contract!(
        "delete-board-label",
        "DELETE /api/v1/boards/:board/labels/:label_id",
        ApiHeaderProfile::LocaleActor,
        "locale-actor-headers",
        crate::headers::LocaleActorHeaders
    ),
    response_contract!(
        "api.delete-board-label.response",
        "DELETE /api/v1/boards/:board/labels/:label_id",
        "success",
        "delete-board-label-response",
        "Delete Board Label Response v1",
        crate::DeleteBoardLabelResponse,
        LABEL_DELETE_WITNESS
    )
    .with_adoption(
        label_adoption!(
            "suite::labels_adoption::delete_board_label_response_fixture_is_produced_by_real_router"
        ),
        label_adoption!(
            "suite::labels_adoption::delete_board_label_response_fixture_is_consumed_by_contract_root"
        ),
    ),
];

const API_LIST_LABEL_SEMANTICS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.list-label-semantics.path",
        "GET /api/v1/boards/:board/labels/semantics",
        "list-label-semantics-path",
        "List Label Semantics Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        LABEL_WITNESS
    ),
    header_contract!(
        "list-label-semantics",
        "GET /api/v1/boards/:board/labels/semantics",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.list-label-semantics.response",
        "GET /api/v1/boards/:board/labels/semantics",
        "success",
        "list-label-semantics-response",
        "List label semantics response v1",
        crate::ListLabelSemanticsResponse,
        LABEL_WITNESS
    ),
];

const API_GET_LABEL_SEMANTICS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.get-label-semantics.path",
        "GET /api/v1/boards/:board/labels/:label_id/semantics",
        "get-label-semantics-path",
        "Get Label Semantics Path v1",
        LABEL_SEMANTICS_PATH_PARAMETERS,
        crate::LabelSemanticsPath,
        LABEL_WITNESS
    ),
    header_contract!(
        "get-label-semantics",
        "GET /api/v1/boards/:board/labels/:label_id/semantics",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.get-label-semantics.response",
        "GET /api/v1/boards/:board/labels/:label_id/semantics",
        "success",
        "get-label-semantics-response",
        "Get label semantics response v1",
        crate::GetLabelSemanticsResponse,
        LABEL_WITNESS
    ),
];

const API_UPSERT_LABEL_SEMANTICS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.upsert-label-semantics.path",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        "upsert-label-semantics-path",
        "Upsert Label Semantics Path v1",
        LABEL_SEMANTICS_PATH_PARAMETERS,
        crate::LabelSemanticsPath,
        LABEL_WITNESS
    ),
    header_contract!(
        "upsert-label-semantics",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers",
        crate::headers::LocaleActorJsonHeaders
    ),
    body_contract!(
        "api.upsert-label-semantics.request",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        "body",
        "upsert-label-semantics-request",
        "Upsert label semantics request v1",
        crate::UpsertLabelSemanticsRequest,
        LABEL_WITNESS
    ),
    response_contract!(
        "api.upsert-label-semantics.response",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        "success",
        "upsert-label-semantics-response",
        "Upsert label semantics response v1",
        crate::UpsertLabelSemanticsResponse,
        LABEL_WITNESS
    ),
];

const API_DELETE_LABEL_SEMANTICS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.delete-label-semantics.path",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
        "delete-label-semantics-path",
        "Delete Label Semantics Path v1",
        LABEL_SEMANTICS_PATH_PARAMETERS,
        crate::LabelSemanticsPath,
        LABEL_WITNESS
    ),
    query_contract!(
        "api.delete-label-semantics.query",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
        "delete-label-semantics-query",
        "Delete label semantics query v1",
        crate::DeleteLabelSemanticsQuery,
        LABEL_WITNESS
    ),
    header_contract!(
        "delete-label-semantics",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
        ApiHeaderProfile::LocaleActor,
        "locale-actor-headers",
        crate::headers::LocaleActorHeaders
    ),
    response_contract!(
        "api.label-semantics-delete.response",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
        "response",
        "delete-response",
        "Kanban API delete response v1",
        crate::DeleteResponse,
        LABEL_WITNESS
    )
    .with_operation("label semantics deletion acknowledgement")
    .with_transport(
        Some("DELETE /api/v1/boards/:board/labels/:label_id/semantics"),
        &[],
    ),
];

const API_LIST_LABEL_ATOMS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.list-label-atoms.path",
        "GET /api/v1/boards/:board/labels/atoms",
        "list-label-atoms-path",
        "List Label Atoms Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        LABEL_WITNESS
    ),
    header_contract!(
        "list-label-atoms",
        "GET /api/v1/boards/:board/labels/atoms",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.list-label-atoms.response",
        "GET /api/v1/boards/:board/labels/atoms",
        "success",
        "list-label-atoms-response",
        "List label atoms response v1",
        crate::ListLabelAtomsResponse,
        LABEL_WITNESS
    ),
];

const API_EXPLAIN_LABEL_ATOM_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.label-atom.path",
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain",
        "label-atom-path",
        "Label atom path v1",
        LABEL_ATOM_PATH_PARAMETERS,
        crate::LabelAtomPath,
        LABEL_WITNESS
    ),
    header_contract!(
        "explain-label-atom",
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.explain-label-atom.response",
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain",
        "success",
        "explain-label-atom-response",
        "Explain label atom response v1",
        crate::ExplainLabelAtomResponse,
        LABEL_WITNESS
    ),
];

const API_LABEL_ATOM_INDEX_STATUS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.label-atom-index-status.path",
        "GET /api/v1/boards/:board/labels/atom-index/status",
        "label-atom-index-status-path",
        "Label Atom Index Status Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        LABEL_WITNESS
    ),
    header_contract!(
        "label-atom-index-status",
        "GET /api/v1/boards/:board/labels/atom-index/status",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.label-atom-index-status.response",
        "GET /api/v1/boards/:board/labels/atom-index/status",
        "success",
        "label-atom-index-status-response",
        "Label atom index status response v1",
        crate::LabelAtomIndexStatusResponse,
        LABEL_WITNESS
    ),
];

const API_REBUILD_LABEL_ATOM_INDEX_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.rebuild-label-atom-index.path",
        "POST /api/v1/boards/:board/labels/atom-index/rebuild",
        "rebuild-label-atom-index-path",
        "Rebuild Label Atom Index Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        LABEL_WITNESS
    ),
    header_contract!(
        "rebuild-label-atom-index",
        "POST /api/v1/boards/:board/labels/atom-index/rebuild",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.rebuild-label-atom-index.response",
        "POST /api/v1/boards/:board/labels/atom-index/rebuild",
        "success",
        "rebuild-label-atom-index-response",
        "Rebuild label atom index response v1",
        crate::RebuildLabelAtomIndexResponse,
        LABEL_WITNESS
    ),
];

const API_QUERY_LABEL_ATOM_INDEX_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.query-label-atom-index.path",
        "GET /api/v1/boards/:board/labels/atom-index/query",
        "query-label-atom-index-path",
        "Query Label Atom Index Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        LABEL_WITNESS
    ),
    query_contract!(
        "api.query-label-atom-index.query",
        "GET /api/v1/boards/:board/labels/atom-index/query",
        "query-label-atom-index-query",
        "Query Label Atom Index Query v1",
        crate::LabelAtomIndexQuery,
        LABEL_WITNESS
    ),
    header_contract!(
        "query-label-atom-index",
        "GET /api/v1/boards/:board/labels/atom-index/query",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.query-label-atom-index.response",
        "GET /api/v1/boards/:board/labels/atom-index/query",
        "success",
        "query-label-atom-index-response",
        "Query label atom index response v1",
        crate::QueryLabelAtomIndexResponse,
        LABEL_WITNESS
    ),
];

const API_LIST_SIGNALS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.list-signals.path",
        "GET /api/v1/boards/:board/signals",
        "list-signals-path",
        "List Signals Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        SIGNAL_WITNESS
    ),
    query_contract!(
        "api.list-signals.query",
        "GET /api/v1/boards/:board/signals",
        "list-signals-query",
        "List Signals Query v1",
        crate::SignalQuery,
        SIGNAL_WITNESS
    ),
    header_contract!(
        "list-signals",
        "GET /api/v1/boards/:board/signals",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.list-signals.response",
        "GET /api/v1/boards/:board/signals",
        "success",
        "list-signals-response",
        "List signals response v1",
        crate::ListSignalsResponse,
        SIGNAL_WITNESS
    ),
];

const API_REVIEW_SIGNALS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.review-signals.path",
        "GET /api/v1/boards/:board/signals/review",
        "review-signals-path",
        "Review Signals Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        SIGNAL_WITNESS
    ),
    query_contract!(
        "api.review-signals.query",
        "GET /api/v1/boards/:board/signals/review",
        "review-signals-query",
        "Review Signals Query v1",
        crate::SignalQuery,
        SIGNAL_WITNESS
    ),
    header_contract!(
        "review-signals",
        "GET /api/v1/boards/:board/signals/review",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.review-signals.response",
        "GET /api/v1/boards/:board/signals/review",
        "success",
        "review-signals-response",
        "Review signals response v1",
        crate::ReviewSignalsResponse,
        SIGNAL_WITNESS
    ),
];

const API_GET_SIGNAL_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.get-signal.path",
        "GET /api/v1/signals/:signal_id",
        "get-signal-path",
        "Get Signal Path v1",
        SIGNAL_PATH_PARAMETERS,
        crate::SignalPath,
        SIGNAL_WITNESS
    ),
    header_contract!(
        "get-signal",
        "GET /api/v1/signals/:signal_id",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.get-signal.response",
        "GET /api/v1/signals/:signal_id",
        "success",
        "get-signal-response",
        "Get signal response v1",
        crate::GetSignalResponse,
        SIGNAL_WITNESS
    ),
];

macro_rules! signal_mutation_contracts {
    ($slug:literal, $operation:literal, $path_slug:literal, $path_title:literal, $request_slug:literal, $request_title:literal, $request_type:ty, $response_slug:literal, $response_title:literal, $response_type:ty) => {
        &[
            path_contract!(
                concat!("api.", $slug, ".path"),
                $operation,
                $path_slug,
                $path_title,
                BOARD_PATH_PARAMETERS,
                crate::BoardLabelPath,
                SIGNAL_WITNESS
            ),
            header_contract!(
                $slug,
                $operation,
                ApiHeaderProfile::LocaleJson,
                "locale-json-headers",
                crate::headers::LocaleJsonHeaders
            ),
            body_contract!(
                concat!("api.", $slug, ".request"),
                $operation,
                "request",
                $request_slug,
                $request_title,
                $request_type,
                SIGNAL_WITNESS
            ),
            response_contract!(
                concat!("api.", $slug, ".response"),
                $operation,
                "success",
                $response_slug,
                $response_title,
                $response_type,
                SIGNAL_WITNESS
            ),
        ]
    };
}

const API_RECORD_SIGNAL_CONTRACTS: &[ContractDeclaration] = signal_mutation_contracts!(
    "record-signal",
    "POST /api/v1/boards/:board/signals",
    "record-signal-path",
    "Record Signal Path v1",
    "record-signal-request",
    "Record signal request v1",
    crate::RecordSignalRequest,
    "record-signal-response",
    "Record signal response v1",
    crate::RecordSignalResponse
);
const API_CONFIRM_SIGNALS_CONTRACTS: &[ContractDeclaration] = signal_mutation_contracts!(
    "confirm-signals",
    "POST /api/v1/boards/:board/signals/confirm",
    "confirm-signals-path",
    "Confirm Signals Path v1",
    "review-signals-request",
    "Review signals request v1",
    crate::ReviewSignalsRequest,
    "confirm-signals-response",
    "Confirm signals response v1",
    crate::ConfirmSignalsResponse
);
const API_REJECT_SIGNALS_CONTRACTS: &[ContractDeclaration] = signal_mutation_contracts!(
    "reject-signals",
    "POST /api/v1/boards/:board/signals/reject",
    "reject-signals-path",
    "Reject Signals Path v1",
    "reject-signals-request",
    "Reject signals request v1",
    crate::ReviewSignalsRequest,
    "reject-signals-response",
    "Reject signals response v1",
    crate::RejectSignalsResponse
);
const API_RESOLVE_SIGNALS_CONTRACTS: &[ContractDeclaration] = signal_mutation_contracts!(
    "resolve-signals",
    "POST /api/v1/boards/:board/signals/resolve",
    "resolve-signals-path",
    "Resolve Signals Path v1",
    "resolve-signals-request",
    "Resolve signals request v1",
    crate::ReviewSignalsRequest,
    "resolve-signals-response",
    "Resolve signals response v1",
    crate::ResolveSignalsResponse
);
const API_SUPERSEDE_SIGNALS_CONTRACTS: &[ContractDeclaration] = signal_mutation_contracts!(
    "supersede-signals",
    "POST /api/v1/boards/:board/signals/supersede",
    "supersede-signals-path",
    "Supersede Signals Path v1",
    "supersede-signals-request",
    "Supersede signals request v1",
    crate::ReviewSignalsRequest,
    "supersede-signals-response",
    "Supersede signals response v1",
    crate::SupersedeSignalsResponse
);

const API_SUGGEST_TASK_LABELS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.suggest-task-labels.path",
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        "suggest-task-labels-path",
        "Suggest Task Labels Path v1",
        TASK_LABEL_PATH_PARAMETERS,
        crate::TaskLabelSurfacePath,
        LABEL_WITNESS
    ),
    query_contract!(
        "api.label-suggestion.query",
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        "label-suggestion-query",
        "Label suggestion query v1",
        crate::LabelSuggestionQuery,
        LABEL_WITNESS
    ),
    header_contract!(
        "suggest-task-labels",
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.suggest-task-labels.response",
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        "success",
        "suggest-task-labels-response",
        "Suggest task labels response v1",
        crate::SuggestTaskLabelsResponse,
        LABEL_WITNESS
    ),
];

const API_LIST_TASK_LABEL_PROPOSALS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.list-task-label-proposals.path",
        "GET /api/v1/tasks/:task_id/label-proposals",
        "list-task-label-proposals-path",
        "List Task Label Proposals Path v1",
        TASK_LABEL_PATH_PARAMETERS,
        crate::TaskLabelSurfacePath,
        PROPOSAL_WITNESS
    ),
    header_contract!(
        "list-task-label-proposals",
        "GET /api/v1/tasks/:task_id/label-proposals",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.list-task-label-proposals.response",
        "GET /api/v1/tasks/:task_id/label-proposals",
        "success",
        "list-task-label-proposals-response",
        "List task label proposals response v1",
        crate::ListTaskLabelProposalsResponse,
        PROPOSAL_WITNESS
    ),
];

const API_LIST_BOARD_LABEL_PROPOSALS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.list-board-label-proposals.path",
        "GET /api/v1/boards/:board/label-proposals",
        "list-board-label-proposals-path",
        "List Board Label Proposals Path v1",
        BOARD_PATH_PARAMETERS,
        crate::ListBoardLabelProposalsPath,
        PROPOSAL_WITNESS
    ),
    api_contract!(
        "api.list-board-label-proposals.query",
        "GET /api/v1/boards/:board/label-proposals query",
        "GET /api/v1/boards/:board/label-proposals",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        BOARD_LABEL_PROPOSALS_QUERY_PARAMETERS,
        "list-board-label-proposals-query",
        "List Board Label Proposals Query v1",
        crate::ListBoardLabelProposalsQuery,
        PROPOSAL_WITNESS
    ),
    {
        let contract = ContractDeclaration::new(
            "api.list-board-label-proposals.headers",
            "GET /api/v1/boards/:board/label-proposals headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:list-board-label-proposals-headers:v1",
            "api/list-board-label-proposals-headers.v1.schema.json",
            "List Board Label Proposals Headers v1",
            "schemas/fixtures/api/headers/list-board-label-proposals-locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/list-board-label-proposals-locale-headers.v1.invalid.json",
        )
        .with_adoption(HEADER_LOCALE_WITNESS, HEADER_LOCALE_WITNESS);
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<crate::headers::LocaleHeaders>();
        contract
    },
    response_contract!(
        "api.list-board-label-proposals.response",
        "GET /api/v1/boards/:board/label-proposals",
        "success",
        "list-board-label-proposals-response",
        "List board label proposals response v1",
        crate::ListBoardLabelProposalsResponse,
        PROPOSAL_WITNESS
    ),
];

const API_PROPOSE_TASK_LABEL_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.propose-task-label.path",
        "POST /api/v1/tasks/:task_id/label-proposals",
        "propose-task-label-path",
        "Propose Task Label Path v1",
        TASK_LABEL_PATH_PARAMETERS,
        crate::TaskLabelSurfacePath,
        PROPOSAL_WITNESS
    ),
    query_contract!(
        "api.propose-task-label.query",
        "POST /api/v1/tasks/:task_id/label-proposals",
        "propose-task-label-query",
        "Propose task label query v1",
        crate::LabelSuggestionQuery,
        PROPOSAL_WITNESS
    ),
    header_contract!(
        "propose-task-label",
        "POST /api/v1/tasks/:task_id/label-proposals",
        ApiHeaderProfile::LocaleActorOptionalJson,
        "locale-actor-optional-json-headers",
        crate::headers::LocaleActorOptionalJsonHeaders
    ),
    body_contract!(
        "api.propose-task-label.request",
        "POST /api/v1/tasks/:task_id/label-proposals",
        "body",
        "propose-task-label-request",
        "Propose task label request v1",
        crate::ProposeTaskLabelRequest,
        PROPOSAL_WITNESS
    ),
    response_contract!(
        "api.propose-task-label.response",
        "POST /api/v1/tasks/:task_id/label-proposals",
        "success",
        "propose-task-label-response",
        "Propose task label response v1",
        crate::ProposeTaskLabelResponse,
        PROPOSAL_WITNESS
    ),
];

const API_RECORD_LABEL_ONTOLOGY_OBSERVATION_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.record-label-ontology-observation.path",
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        "record-label-ontology-observation-path",
        "Record Label Ontology Observation Path v1",
        TASK_LABEL_PATH_PARAMETERS,
        crate::TaskLabelSurfacePath,
        ONTOLOGY_WITNESS
    ),
    header_contract!(
        "record-label-ontology-observation",
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        ApiHeaderProfile::LocaleJson,
        "locale-json-headers",
        crate::headers::LocaleJsonHeaders
    ),
    body_contract!(
        "api.record-label-ontology-observation.body",
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        "body",
        "record-label-ontology-observation-body",
        "Record label ontology observation request v1",
        crate::RecordLabelOntologyObservationRequest,
        ONTOLOGY_WITNESS
    ),
    response_contract!(
        "api.record-label-ontology-observation.response",
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        "success",
        "record-label-ontology-observation-response",
        "Record label ontology observation response v1",
        crate::RecordLabelOntologyObservationResponse,
        ONTOLOGY_WITNESS
    ),
];

const API_LIST_LABEL_ONTOLOGY_SIGNALS_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.list-label-ontology-signals.path",
        "GET /api/v1/boards/:board/label-ontology/signals",
        "list-label-ontology-signals-path",
        "List Label Ontology Signals Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        ONTOLOGY_WITNESS
    ),
    query_contract!(
        "api.label-ontology-signal.query",
        "GET /api/v1/boards/:board/label-ontology/signals",
        "label-ontology-signal-query",
        "Label ontology signal query v1",
        crate::LabelOntologySignalQuery,
        ONTOLOGY_WITNESS
    ),
    header_contract!(
        "list-label-ontology-signals",
        "GET /api/v1/boards/:board/label-ontology/signals",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.list-label-ontology-signals.response",
        "GET /api/v1/boards/:board/label-ontology/signals",
        "response",
        "list-label-ontology-signals-response",
        "Kanban API list label ontology signals response v1",
        crate::LabelOntologySignalsResponse,
        ONTOLOGY_WITNESS
    ),
];

const API_REVIEW_LABEL_ONTOLOGY_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.review-label-ontology.path",
        "GET /api/v1/boards/:board/label-ontology/review",
        "review-label-ontology-path",
        "Review Label Ontology Path v1",
        BOARD_PATH_PARAMETERS,
        crate::BoardLabelPath,
        ONTOLOGY_WITNESS
    ),
    query_contract!(
        "api.label-ontology-review.query",
        "GET /api/v1/boards/:board/label-ontology/review",
        "label-ontology-review-query",
        "Label ontology review query v1",
        crate::LabelOntologyReviewQuery,
        ONTOLOGY_WITNESS
    ),
    header_contract!(
        "review-label-ontology",
        "GET /api/v1/boards/:board/label-ontology/review",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.review-label-ontology.response",
        "GET /api/v1/boards/:board/label-ontology/review",
        "success",
        "review-label-ontology-response",
        "Review label ontology response v1",
        crate::ReviewLabelOntologyResponse,
        ONTOLOGY_WITNESS
    ),
];

macro_rules! ontology_mutation_contracts {
    ($slug:literal, $operation:literal, $path_slug:literal, $path_title:literal, $request_slug:literal, $request_title:literal, $request_type:ty, $response_slug:literal, $response_title:literal, $response_type:ty) => {
        &[
            path_contract!(
                concat!("api.", $slug, ".path"),
                $operation,
                $path_slug,
                $path_title,
                BOARD_PATH_PARAMETERS,
                crate::BoardLabelPath,
                ONTOLOGY_WITNESS
            ),
            header_contract!(
                $slug,
                $operation,
                ApiHeaderProfile::LocaleJson,
                "locale-json-headers",
                crate::headers::LocaleJsonHeaders
            ),
            body_contract!(
                concat!("api.", $slug, ".request"),
                $operation,
                "body",
                $request_slug,
                $request_title,
                $request_type,
                ONTOLOGY_WITNESS
            ),
            response_contract!(
                concat!("api.", $slug, ".response"),
                $operation,
                "success",
                $response_slug,
                $response_title,
                $response_type,
                ONTOLOGY_WITNESS
            ),
        ]
    };
}

const API_CREATE_LABEL_ONTOLOGY_ACTION_CONTRACTS: &[ContractDeclaration] = ontology_mutation_contracts!(
    "create-label-ontology-action",
    "POST /api/v1/boards/:board/label-ontology/actions",
    "create-label-ontology-action-path",
    "Create Label Ontology Action Path v1",
    "create-label-ontology-action-request",
    "Create label ontology action request v1",
    crate::LabelOntologyActionRequest,
    "create-label-ontology-action-response",
    "Create label ontology action response v1",
    crate::LabelOntologyActionResponse
);
const API_APPLY_LABEL_ONTOLOGY_ATOM_CONTRACTS: &[ContractDeclaration] = ontology_mutation_contracts!(
    "apply-label-ontology-atom",
    "POST /api/v1/boards/:board/label-ontology/apply/atom",
    "apply-label-ontology-atom-path",
    "Apply Label Ontology Atom Path v1",
    "apply-label-ontology-atom-request",
    "Apply label ontology atom request v1",
    crate::ApplyLabelOntologyAtomRequest,
    "apply-label-ontology-atom-response",
    "Apply label ontology atom response v1",
    crate::LabelOntologyActionResponse
);
const API_REVERT_LABEL_ONTOLOGY_MUTATION_CONTRACTS: &[ContractDeclaration] = ontology_mutation_contracts!(
    "revert-label-ontology-mutation",
    "POST /api/v1/boards/:board/label-ontology/revert",
    "revert-label-ontology-mutation-path",
    "Revert Label Ontology Mutation Path v1",
    "revert-label-ontology-mutation-request",
    "Revert label ontology mutation request v1",
    crate::RevertLabelOntologyMutationRequest,
    "revert-label-ontology-mutation-response",
    "Revert label ontology mutation response v1",
    crate::LabelOntologyActionResponse
);
const API_VALIDATE_LABEL_ONTOLOGY_ACTION_CONTRACTS: &[ContractDeclaration] = ontology_mutation_contracts!(
    "validate-label-ontology-action",
    "POST /api/v1/boards/:board/label-ontology/validate",
    "validate-label-ontology-action-path",
    "Validate Label Ontology Action Path v1",
    "validate-label-ontology-action-request",
    "Validate label ontology action request v1",
    crate::ValidateLabelOntologyActionRequest,
    "validate-label-ontology-action-response",
    "Validate label ontology action response v1",
    crate::LabelOntologyActionResponse
);

const API_GET_LABEL_ONTOLOGY_SIGNAL_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.get-label-ontology-signal.path",
        "GET /api/v1/label-ontology/signals/:signal_id",
        "get-label-ontology-signal-path",
        "Get Label Ontology Signal Path v1",
        SIGNAL_PATH_PARAMETERS,
        crate::SignalPath,
        ONTOLOGY_WITNESS
    ),
    header_contract!(
        "get-label-ontology-signal",
        "GET /api/v1/label-ontology/signals/:signal_id",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.get-label-ontology-signal.response",
        "GET /api/v1/label-ontology/signals/:signal_id",
        "success",
        "get-label-ontology-signal-response",
        "Get label ontology signal response v1",
        crate::GetLabelOntologySignalResponse,
        ONTOLOGY_WITNESS
    ),
];

const API_GET_LABEL_PROPOSAL_CONTRACTS: &[ContractDeclaration] = &[
    path_contract!(
        "api.get-label-proposal.path",
        "GET /api/v1/label-proposals/:proposal_id",
        "get-label-proposal-path",
        "Get Label Proposal Path v1",
        PROPOSAL_PATH_PARAMETERS,
        crate::ProposalPath,
        PROPOSAL_WITNESS
    ),
    header_contract!(
        "get-label-proposal",
        "GET /api/v1/label-proposals/:proposal_id",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    response_contract!(
        "api.get-label-proposal.response",
        "GET /api/v1/label-proposals/:proposal_id",
        "success",
        "get-label-proposal-response",
        "Get label proposal response v1",
        crate::GetLabelProposalResponse,
        PROPOSAL_WITNESS
    ),
];

macro_rules! label_proposal_decision_contracts {
    ($slug:literal, $operation:literal, $path_slug:literal, $path_title:literal, $body_slug:literal, $body_title:literal, $response_slug:literal, $response_title:literal) => {
        &[
            path_contract!(
                concat!("api.", $slug, ".path"),
                $operation,
                $path_slug,
                $path_title,
                PROPOSAL_PATH_PARAMETERS,
                crate::ProposalPath,
                PROPOSAL_WITNESS
            ),
            header_contract!(
                $slug,
                $operation,
                ApiHeaderProfile::LocaleActorOptionalJson,
                "locale-actor-optional-json-headers",
                crate::headers::LocaleActorOptionalJsonHeaders
            ),
            body_contract!(
                concat!("api.", $slug, ".body"),
                $operation,
                "body",
                $body_slug,
                $body_title,
                crate::LabelProposalDecisionRequest,
                PROPOSAL_WITNESS
            ),
            response_contract!(
                concat!("api.", $slug, ".response"),
                $operation,
                "success",
                $response_slug,
                $response_title,
                crate::LabelProposalDecisionResponse,
                PROPOSAL_WITNESS
            ),
        ]
    };
}

const API_ACCEPT_LABEL_PROPOSAL_CONTRACTS: &[ContractDeclaration] = label_proposal_decision_contracts!(
    "accept-label-proposal",
    "POST /api/v1/label-proposals/:proposal_id/accept",
    "accept-label-proposal-path",
    "Accept Label Proposal Path v1",
    "accept-label-proposal-body",
    "Accept Label Proposal Body v1",
    "accept-label-proposal-response",
    "Accept label proposal response v1"
);
const API_REJECT_LABEL_PROPOSAL_CONTRACTS: &[ContractDeclaration] = label_proposal_decision_contracts!(
    "reject-label-proposal",
    "POST /api/v1/label-proposals/:proposal_id/reject",
    "reject-label-proposal-path",
    "Reject Label Proposal Path v1",
    "reject-label-proposal-body",
    "Reject Label Proposal Body v1",
    "reject-label-proposal-response",
    "Reject label proposal response v1"
);

const TASK_LABEL_LIST_BINDING: McpToolBinding = McpToolBinding {
    tool_name: "task_label_list",
    http_operations: &["api.list-tasks", "api.list-task-labels"],
};
const TASK_LABEL_ADD_BINDING: McpToolBinding = McpToolBinding {
    tool_name: "task_label_add",
    http_operations: &["api.list-tasks", "api.add-task-label"],
};
const TASK_LABEL_REMOVE_BINDING: McpToolBinding = McpToolBinding {
    tool_name: "task_label_remove",
    http_operations: &["api.list-tasks", "api.remove-task-label"],
};
const ONTOLOGY_REVIEW_BINDING: McpToolBinding = McpToolBinding {
    tool_name: "label_ontology_review",
    http_operations: &["api.review-label-ontology"],
};
const ONTOLOGY_QUALITY_BINDING: McpToolBinding = McpToolBinding {
    tool_name: "label_ontology_quality",
    http_operations: &["api.review-label-ontology"],
};

const LABEL_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.list-task-labels",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/labels"),
        "GET /api/v1/tasks/:task_id/labels",
        "GET /api/v1/tasks/:task_id/labels",
        MigrationState::Adopted,
        API_LIST_TASK_LABELS_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(TASK_LABEL_LIST_BINDING)),
    OperationDeclaration::new(
        "api.add-task-label",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/labels"),
        "POST /api/v1/tasks/:task_id/labels",
        "POST /api/v1/tasks/:task_id/labels",
        MigrationState::Adopted,
        API_ADD_TASK_LABEL_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_LABEL_ADD_BINDING)),
    OperationDeclaration::new(
        "api.remove-task-label",
        ContractSurface::Api,
        Some(HttpMethod::Delete),
        Some("/api/v1/tasks/:task_id/labels/:label_id"),
        "DELETE /api/v1/tasks/:task_id/labels/:label_id",
        "DELETE /api/v1/tasks/:task_id/labels/:label_id",
        MigrationState::Adopted,
        API_REMOVE_TASK_LABEL_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::LocaleActor)
    .with_mcp_policy(policy!(TASK_LABEL_REMOVE_BINDING)),
    OperationDeclaration::new(
        "api.list-board-labels",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/labels"),
        "GET /api/v1/boards/:board/labels",
        "GET /api/v1/boards/:board/labels",
        MigrationState::Adopted,
        API_LIST_BOARD_LABELS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("label_list", ["api.list-board-labels"])),
    OperationDeclaration::new(
        "api.list-board-label-proposals",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/label-proposals"),
        "GET /api/v1/boards/:board/label-proposals",
        "GET /api/v1/boards/:board/label-proposals",
        MigrationState::Adopted,
        API_LIST_BOARD_LABEL_PROPOSALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "label_proposals_list",
        [
            "api.list-task-label-proposals",
            "api.list-board-label-proposals"
        ]
    )),
    OperationDeclaration::new(
        "api.create-board-label",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/labels"),
        "POST /api/v1/boards/:board/labels",
        "POST /api/v1/boards/:board/labels",
        MigrationState::Adopted,
        API_CREATE_BOARD_LABEL_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("label_create", ["api.create-board-label"])),
    OperationDeclaration::new(
        "api.delete-board-label",
        ContractSurface::Api,
        Some(HttpMethod::Delete),
        Some("/api/v1/boards/:board/labels/:label_id"),
        "DELETE /api/v1/boards/:board/labels/:label_id",
        "DELETE /api/v1/boards/:board/labels/:label_id",
        MigrationState::Adopted,
        API_DELETE_BOARD_LABEL_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::LocaleActor)
    .with_mcp_policy(policy!("label_delete", ["api.delete-board-label"])),
    OperationDeclaration::new(
        "api.list-label-semantics",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/labels/semantics"),
        "GET /api/v1/boards/:board/labels/semantics",
        "GET /api/v1/boards/:board/labels/semantics",
        MigrationState::Adopted,
        API_LIST_LABEL_SEMANTICS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "label_semantics_list",
        ["api.list-label-semantics"]
    )),
    OperationDeclaration::new(
        "api.get-label-semantics",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/labels/:label_id/semantics"),
        "GET /api/v1/boards/:board/labels/:label_id/semantics",
        "GET /api/v1/boards/:board/labels/:label_id/semantics",
        MigrationState::Adopted,
        API_GET_LABEL_SEMANTICS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("label_semantics_show", ["api.get-label-semantics"])),
    OperationDeclaration::new(
        "api.upsert-label-semantics",
        ContractSurface::Api,
        Some(HttpMethod::Put),
        Some("/api/v1/boards/:board/labels/:label_id/semantics"),
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        MigrationState::Adopted,
        API_UPSERT_LABEL_SEMANTICS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(
        "label_semantics_upsert",
        ["api.upsert-label-semantics"]
    )),
    OperationDeclaration::new(
        "api.delete-label-semantics",
        ContractSurface::Api,
        Some(HttpMethod::Delete),
        Some("/api/v1/boards/:board/labels/:label_id/semantics"),
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
        MigrationState::Adopted,
        API_DELETE_LABEL_SEMANTICS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActor)
    .with_mcp_policy(policy!(
        "label_semantics_delete",
        ["api.delete-label-semantics"]
    )),
    OperationDeclaration::new(
        "api.list-label-atoms",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/labels/atoms"),
        "GET /api/v1/boards/:board/labels/atoms",
        "GET /api/v1/boards/:board/labels/atoms",
        MigrationState::Adopted,
        API_LIST_LABEL_ATOMS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("label_atoms_list", ["api.list-label-atoms"])),
    OperationDeclaration::new(
        "api.explain-label-atom",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/labels/atoms/:atom_ref/explain"),
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain",
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain",
        MigrationState::Adopted,
        API_EXPLAIN_LABEL_ATOM_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("label_atom_explain", ["api.explain-label-atom"])),
    OperationDeclaration::new(
        "api.label-atom-index-status",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/labels/atom-index/status"),
        "GET /api/v1/boards/:board/labels/atom-index/status",
        "GET /api/v1/boards/:board/labels/atom-index/status",
        MigrationState::Adopted,
        API_LABEL_ATOM_INDEX_STATUS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "label_atom_index_status",
        ["api.label-atom-index-status"]
    )),
    OperationDeclaration::new(
        "api.rebuild-label-atom-index",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/labels/atom-index/rebuild"),
        "POST /api/v1/boards/:board/labels/atom-index/rebuild",
        "POST /api/v1/boards/:board/labels/atom-index/rebuild",
        MigrationState::Adopted,
        API_REBUILD_LABEL_ATOM_INDEX_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "label_atom_index_rebuild",
        ["api.rebuild-label-atom-index"]
    )),
    OperationDeclaration::new(
        "api.query-label-atom-index",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/labels/atom-index/query"),
        "GET /api/v1/boards/:board/labels/atom-index/query",
        "GET /api/v1/boards/:board/labels/atom-index/query",
        MigrationState::Adopted,
        API_QUERY_LABEL_ATOM_INDEX_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "label_atom_index_query",
        ["api.query-label-atom-index"]
    )),
    OperationDeclaration::new(
        "api.list-signals",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/signals"),
        "GET /api/v1/boards/:board/signals",
        "GET /api/v1/boards/:board/signals",
        MigrationState::Adopted,
        API_LIST_SIGNALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("signal_list", ["api.list-signals"])),
    OperationDeclaration::new(
        "api.review-signals",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/signals/review"),
        "GET /api/v1/boards/:board/signals/review",
        "GET /api/v1/boards/:board/signals/review",
        MigrationState::Adopted,
        API_REVIEW_SIGNALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("signal_review", ["api.review-signals"])),
    OperationDeclaration::new(
        "api.get-signal",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/signals/:signal_id"),
        "GET /api/v1/signals/:signal_id",
        "GET /api/v1/signals/:signal_id",
        MigrationState::Adopted,
        API_GET_SIGNAL_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("signal_show", ["api.get-signal"])),
    OperationDeclaration::new(
        "api.record-signal",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/signals"),
        "POST /api/v1/boards/:board/signals",
        "POST /api/v1/boards/:board/signals",
        MigrationState::Adopted,
        API_RECORD_SIGNAL_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("signal_record", ["api.record-signal"])),
    OperationDeclaration::new(
        "api.confirm-signals",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/signals/confirm"),
        "POST /api/v1/boards/:board/signals/confirm",
        "POST /api/v1/boards/:board/signals/confirm",
        MigrationState::Adopted,
        API_CONFIRM_SIGNALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("signal_confirm", ["api.confirm-signals"])),
    OperationDeclaration::new(
        "api.reject-signals",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/signals/reject"),
        "POST /api/v1/boards/:board/signals/reject",
        "POST /api/v1/boards/:board/signals/reject",
        MigrationState::Adopted,
        API_REJECT_SIGNALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("signal_reject", ["api.reject-signals"])),
    OperationDeclaration::new(
        "api.resolve-signals",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/signals/resolve"),
        "POST /api/v1/boards/:board/signals/resolve",
        "POST /api/v1/boards/:board/signals/resolve",
        MigrationState::Adopted,
        API_RESOLVE_SIGNALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("signal_resolve", ["api.resolve-signals"])),
    OperationDeclaration::new(
        "api.supersede-signals",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/signals/supersede"),
        "POST /api/v1/boards/:board/signals/supersede",
        "POST /api/v1/boards/:board/signals/supersede",
        MigrationState::Adopted,
        API_SUPERSEDE_SIGNALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("signal_supersede", ["api.supersede-signals"])),
    OperationDeclaration::new(
        "api.suggest-task-labels",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/labels/suggestions"),
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        MigrationState::Adopted,
        API_SUGGEST_TASK_LABELS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("label_suggest", ["api.suggest-task-labels"])),
    OperationDeclaration::new(
        "api.list-task-label-proposals",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/label-proposals"),
        "GET /api/v1/tasks/:task_id/label-proposals",
        "GET /api/v1/tasks/:task_id/label-proposals",
        MigrationState::Adopted,
        API_LIST_TASK_LABEL_PROPOSALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "label_proposals_list",
        [
            "api.list-task-label-proposals",
            "api.list-board-label-proposals"
        ]
    )),
    OperationDeclaration::new(
        "api.propose-task-label",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/label-proposals"),
        "POST /api/v1/tasks/:task_id/label-proposals",
        "POST /api/v1/tasks/:task_id/label-proposals",
        MigrationState::Adopted,
        API_PROPOSE_TASK_LABEL_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorOptionalJson)
    .with_mcp_policy(policy!("label_propose", ["api.propose-task-label"])),
    OperationDeclaration::new(
        "api.record-label-ontology-observation",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/label-ontology/observations"),
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        MigrationState::Adopted,
        API_RECORD_LABEL_ONTOLOGY_OBSERVATION_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!(
        "label_ontology_observe",
        ["api.record-label-ontology-observation"]
    )),
    OperationDeclaration::new(
        "api.list-label-ontology-signals",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/label-ontology/signals"),
        "GET /api/v1/boards/:board/label-ontology/signals",
        "GET /api/v1/boards/:board/label-ontology/signals",
        MigrationState::Adopted,
        API_LIST_LABEL_ONTOLOGY_SIGNALS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "label_ontology_signals",
        ["api.list-label-ontology-signals"]
    )),
    OperationDeclaration::new(
        "api.review-label-ontology",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/label-ontology/review"),
        "GET /api/v1/boards/:board/label-ontology/review",
        "GET /api/v1/boards/:board/label-ontology/review",
        MigrationState::Adopted,
        API_REVIEW_LABEL_ONTOLOGY_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(ONTOLOGY_REVIEW_BINDING, ONTOLOGY_QUALITY_BINDING)),
    OperationDeclaration::new(
        "api.create-label-ontology-action",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/label-ontology/actions"),
        "POST /api/v1/boards/:board/label-ontology/actions",
        "POST /api/v1/boards/:board/label-ontology/actions",
        MigrationState::Adopted,
        API_CREATE_LABEL_ONTOLOGY_ACTION_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!(
        "label_ontology_action",
        ["api.create-label-ontology-action"]
    )),
    OperationDeclaration::new(
        "api.apply-label-ontology-atom",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/label-ontology/apply/atom"),
        "POST /api/v1/boards/:board/label-ontology/apply/atom",
        "POST /api/v1/boards/:board/label-ontology/apply/atom",
        MigrationState::Adopted,
        API_APPLY_LABEL_ONTOLOGY_ATOM_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!(
        "label_ontology_apply_atom",
        ["api.apply-label-ontology-atom"]
    )),
    OperationDeclaration::new(
        "api.revert-label-ontology-mutation",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/label-ontology/revert"),
        "POST /api/v1/boards/:board/label-ontology/revert",
        "POST /api/v1/boards/:board/label-ontology/revert",
        MigrationState::Adopted,
        API_REVERT_LABEL_ONTOLOGY_MUTATION_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!(
        "label_ontology_revert",
        ["api.revert-label-ontology-mutation"]
    )),
    OperationDeclaration::new(
        "api.validate-label-ontology-action",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/label-ontology/validate"),
        "POST /api/v1/boards/:board/label-ontology/validate",
        "POST /api/v1/boards/:board/label-ontology/validate",
        MigrationState::Adopted,
        API_VALIDATE_LABEL_ONTOLOGY_ACTION_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!(
        "label_ontology_validate",
        ["api.validate-label-ontology-action"]
    )),
    OperationDeclaration::new(
        "api.get-label-ontology-signal",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/label-ontology/signals/:signal_id"),
        "GET /api/v1/label-ontology/signals/:signal_id",
        "GET /api/v1/label-ontology/signals/:signal_id",
        MigrationState::Adopted,
        API_GET_LABEL_ONTOLOGY_SIGNAL_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "label_ontology_signal_show",
        ["api.get-label-ontology-signal"]
    )),
    OperationDeclaration::new(
        "api.get-label-proposal",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/label-proposals/:proposal_id"),
        "GET /api/v1/label-proposals/:proposal_id",
        "GET /api/v1/label-proposals/:proposal_id",
        MigrationState::Adopted,
        API_GET_LABEL_PROPOSAL_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("label_proposal_show", ["api.get-label-proposal"])),
    OperationDeclaration::new(
        "api.accept-label-proposal",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/label-proposals/:proposal_id/accept"),
        "POST /api/v1/label-proposals/:proposal_id/accept",
        "POST /api/v1/label-proposals/:proposal_id/accept",
        MigrationState::Adopted,
        API_ACCEPT_LABEL_PROPOSAL_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorOptionalJson)
    .with_mcp_policy(policy!(
        "label_proposal_accept",
        ["api.accept-label-proposal"]
    )),
    OperationDeclaration::new(
        "api.reject-label-proposal",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/label-proposals/:proposal_id/reject"),
        "POST /api/v1/label-proposals/:proposal_id/reject",
        "POST /api/v1/label-proposals/:proposal_id/reject",
        MigrationState::Adopted,
        API_REJECT_LABEL_PROPOSAL_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorOptionalJson)
    .with_mcp_policy(policy!(
        "label_proposal_reject",
        ["api.reject-label-proposal"]
    )),
];

/// Labels、ontology 与 signals API parent declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    LABEL_OPERATIONS
}

pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(LABEL_OPERATIONS).contracts()
}

pub fn endpoint_descriptor(operation_id: &str) -> Option<EndpointDescriptor> {
    crate::CatalogProjection::new(LABEL_OPERATIONS)
        .endpoints()
        .into_iter()
        .find(|endpoint| endpoint.operation_id == operation_id)
}

pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    crate::CatalogProjection::new(LABEL_OPERATIONS).endpoints()
}

pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(LABEL_OPERATIONS).surfaces()
}

pub fn header_profile(operation_id: &str) -> Option<ApiHeaderProfile> {
    LABEL_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == operation_id)
        .and_then(|operation| operation.header_profile)
}

pub fn header_contract(operation_id: &str) -> Option<OperationContract> {
    let parent = LABEL_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == operation_id)?;
    parent
        .contracts
        .iter()
        .find(|contract| contract.location == Some(HttpTransportLocation::Headers))
        .map(|contract| contract.operation_contract(parent))
}

#[cfg(feature = "schema")]
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    crate::CatalogProjection::new(LABEL_OPERATIONS).schemas()
}

pub fn owns_contract(id: &str) -> bool {
    LABEL_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

pub fn owns_operation(id: &str) -> bool {
    LABEL_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_declarations_have_unique_contracts_and_explicit_mcp_bindings() {
        let contracts = operation_contracts();
        let mut ids = contracts
            .iter()
            .map(|contract| contract.id)
            .collect::<Vec<_>>();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert_eq!(LABEL_OPERATIONS.len(), 38);
        assert_eq!(contracts.len(), 140);
        assert!(
            LABEL_OPERATIONS
                .iter()
                .all(|operation| operation.mcp_policy.is_some())
        );
    }

    #[test]
    fn labels_hybrid_projection_replaces_each_legacy_row_once() {
        let inventory = crate::operation_inventory();
        for source in operation_contracts() {
            let matches = inventory
                .iter()
                .filter(|contract| contract.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "Labels contract must be projected once: {}",
                source.id
            );
            assert_eq!(
                matches[0], &source,
                "Labels contract changed: {}",
                source.id
            );
        }

        let endpoints = crate::endpoint_catalog();
        for source in endpoint_catalog() {
            let matches = endpoints
                .iter()
                .filter(|endpoint| endpoint.operation_id == source.operation_id)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "Labels endpoint must be projected once");
            assert_eq!(matches[0], &source);
        }
    }
}
