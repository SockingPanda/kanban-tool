//! Knowledge substrate、search、graph、vector 与 context API family 的唯一 declaration source。
//!
//! parent/child declaration 保存 endpoint、header、schema、fixture、adoption locator 和
//! MCP binding。真实 handler/client 仍由各自 crate 持有；本模块只提供协议事实及其
//! deterministic projection。

use crate::{
    AdoptionLocator, ApiHeaderProfile, ContractBinding, ContractDeclaration, ContractDirection,
    ContractGranularity, ContractStrictness, ContractSurface, EndpointDescriptor, HttpMethod,
    HttpTransportLocation, McpExposure, McpPolicy, McpToolBinding, MigrationState,
    OperationContract, OperationDeclaration, SurfaceOperation, WireParameter,
    WireParameterCardinality,
};

const TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const BOARD_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const ENTITY_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "uri",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

const SEARCH_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "q",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "status",
        cardinality: Some(WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "label",
        cardinality: Some(WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "include_archived",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "offset",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "assignee",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const BOARD_QUERY_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];

const CONTEXT_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "lexical_limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "graph_limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "vector_limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "max_items",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "task",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "reference",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "query",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "depth",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "budget",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const GRAPH_TASK_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "depth",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit_nodes",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "include_archived_context",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const GRAPH_BOARD_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "active_only",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "context_depth",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit_nodes",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "include_done_context",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "include_archived_context",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "hide_isolated",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const GRAPH_NEIGHBORS_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "entity_uri",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "predicate",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const GRAPH_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "query",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const ENTITY_LIST_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "kind",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const VECTOR_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "q",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "embedding_model",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "polarity",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "include_vector",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const DOMAIN_INVARIANTS: &[crate::McpOperationInvariant] = &[
    crate::McpOperationInvariant::CanonicalHostOnly,
    crate::McpOperationInvariant::SharedApplicationService,
    crate::McpOperationInvariant::NoHostAdminSurface,
];

const fn server_witness(exact_test: &'static str) -> AdoptionLocator {
    AdoptionLocator {
        package: "kanban-server",
        test_target: "lib",
        exact_test,
    }
}

const fn client_witness(exact_test: &'static str) -> AdoptionLocator {
    AdoptionLocator {
        package: "kanban-client",
        test_target: "lib",
        exact_test,
    }
}

const fn header_witness(profile: ApiHeaderProfile) -> AdoptionLocator {
    match profile {
        ApiHeaderProfile::Locale => {
            server_witness("knowledge_adoption::locale_header_fixture_is_consumed_by_real_router")
        }
        ApiHeaderProfile::LocaleActor => server_witness(
            "knowledge_adoption::locale_actor_header_fixture_is_consumed_by_real_router",
        ),
        ApiHeaderProfile::LocaleJson => server_witness(
            "knowledge_adoption::locale_json_header_fixture_is_consumed_by_real_router",
        ),
        ApiHeaderProfile::LocaleActorJson => server_witness(
            "knowledge_adoption::locale_actor_json_header_fixture_is_consumed_by_real_router",
        ),
        ApiHeaderProfile::LocaleActorOptionalJson => server_witness(
            "knowledge_adoption::locale_actor_optional_json_header_fixture_is_consumed_by_real_router",
        ),
    }
}

macro_rules! api_contract {
    (
        $id:literal, $path:literal, $operation:literal, $operation_key:literal,
        $direction:expr, $location:expr, $parameters:expr, $schema_id:literal,
        $artifact_path:literal, $schema_title:literal, $valid_fixture:literal,
        $invalid_fixture:literal, $schema_type:ty, $producer:expr, $consumer:expr
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
        .with_transport(Some($operation_key), $parameters)
        .with_schema(
            $schema_id,
            $artifact_path,
            $schema_title,
            $valid_fixture,
            $invalid_fixture,
        )
        .with_adoption($producer, $consumer);
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! header_contract {
    (
        $id:literal, $path:literal, $operation:literal, $profile:expr,
        $schema_id:literal, $artifact_path:literal, $schema_title:literal,
        $valid_fixture:literal, $invalid_fixture:literal, $schema_type:ty
    ) => {
        api_contract!(
            $id,
            $path,
            $operation,
            $operation,
            ContractDirection::Deserialize,
            HttpTransportLocation::Headers,
            $profile.parameters(),
            $schema_id,
            $artifact_path,
            $schema_title,
            $valid_fixture,
            $invalid_fixture,
            $schema_type,
            header_witness($profile),
            header_witness($profile)
        )
    };
}

macro_rules! policy {
    ($tool:literal, $operation:literal) => {
        McpPolicy {
            exposure: McpExposure::Domain,
            tool_bindings: &[McpToolBinding {
                tool_name: $tool,
                http_operations: &[$operation],
            }],
            invariants: DOMAIN_INVARIANTS,
        }
    };
}

const API_BOARD_TASK_MAP_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.board-task-map.path",
        "GET /api/v1/boards/:board/task-map path",
        "GET /api/v1/boards/:board/task-map",
        "GET /api/v1/boards/:board/task-map",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:board-task-map-path:v1",
        "api/board-task-map-path.v1.schema.json",
        "Kanban board task map path v1",
        "schemas/fixtures/api/board-task-map-path.v1.valid.json",
        "schemas/fixtures/api/board-task-map-path.v1.invalid.json",
        crate::BoardTaskMapPath,
        server_witness(
            "suite::task_graph_adoption::board_task_map_path_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::task_graph_adoption::board_task_map_path_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.board-task-map.headers",
        "GET /api/v1/boards/:board/task-map headers",
        "GET /api/v1/boards/:board/task-map",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:board-task-map-headers:v1",
        "api/board-task-map-headers.v1.schema.json",
        "Kanban api.board-task-map request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.board-task-map.query",
        "GET /api/v1/boards/:board/task-map query",
        "GET /api/v1/boards/:board/task-map",
        "GET /api/v1/boards/:board/task-map",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        GRAPH_BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:board-task-map-query:v1",
        "api/board-task-map-query.v1.schema.json",
        "Kanban board task map query v1",
        "schemas/fixtures/api/board-task-map-query.v1.valid.json",
        "schemas/fixtures/api/board-task-map-query.v1.invalid.json",
        crate::BoardTaskMapQuery,
        server_witness(
            "suite::task_graph_adoption::board_task_map_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::task_graph_adoption::board_task_map_query_fixture_is_consumed_by_real_router"
        )
    ),
    api_contract!(
        "api.board-task-map.response",
        "GET /api/v1/boards/:board/task-map response",
        "GET /api/v1/boards/:board/task-map",
        "GET /api/v1/boards/:board/task-map",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:board-task-map-response:v1",
        "api/board-task-map-response.v1.schema.json",
        "Kanban board task map response v1",
        "schemas/fixtures/api/board-task-map-response.v1.valid.json",
        "schemas/fixtures/api/board-task-map-response.v1.invalid.json",
        crate::BoardTaskMapResponse,
        server_witness(
            "suite::task_graph_adoption::board_task_map_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::task_graph_adoption::board_task_map_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_TASK_NEIGHBORHOOD_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.task-neighborhood.path",
        "GET /api/v1/tasks/:task_id/neighborhood path",
        "GET /api/v1/tasks/:task_id/neighborhood",
        "GET /api/v1/tasks/:task_id/neighborhood",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:task-neighborhood-path:v1",
        "api/task-neighborhood-path.v1.schema.json",
        "Kanban task neighborhood path v1",
        "schemas/fixtures/api/task-neighborhood-path.v1.valid.json",
        "schemas/fixtures/api/task-neighborhood-path.v1.invalid.json",
        crate::TaskNeighborhoodPath,
        server_witness(
            "suite::task_graph_adoption::task_neighborhood_path_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::task_graph_adoption::task_neighborhood_path_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.task-neighborhood.headers",
        "GET /api/v1/tasks/:task_id/neighborhood headers",
        "GET /api/v1/tasks/:task_id/neighborhood",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:task-neighborhood-headers:v1",
        "api/task-neighborhood-headers.v1.schema.json",
        "Kanban api.task-neighborhood request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.task-neighborhood.query",
        "GET /api/v1/tasks/:task_id/neighborhood query",
        "GET /api/v1/tasks/:task_id/neighborhood",
        "GET /api/v1/tasks/:task_id/neighborhood",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        GRAPH_TASK_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:task-neighborhood-query:v1",
        "api/task-neighborhood-query.v1.schema.json",
        "Kanban task neighborhood query v1",
        "schemas/fixtures/api/task-neighborhood-query.v1.valid.json",
        "schemas/fixtures/api/task-neighborhood-query.v1.invalid.json",
        crate::TaskNeighborhoodQuery,
        server_witness(
            "suite::task_graph_adoption::task_neighborhood_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::task_graph_adoption::task_neighborhood_query_fixture_is_consumed_by_real_router"
        )
    ),
    api_contract!(
        "api.task-neighborhood.response",
        "GET /api/v1/tasks/:task_id/neighborhood response",
        "GET /api/v1/tasks/:task_id/neighborhood",
        "GET /api/v1/tasks/:task_id/neighborhood",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:task-neighborhood-response:v1",
        "api/task-neighborhood-response.v1.schema.json",
        "Kanban task neighborhood response v1",
        "schemas/fixtures/api/task-neighborhood-response.v1.valid.json",
        "schemas/fixtures/api/task-neighborhood-response.v1.invalid.json",
        crate::TaskNeighborhoodResponse,
        server_witness(
            "suite::task_graph_adoption::task_neighborhood_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::task_graph_adoption::task_neighborhood_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_SEARCH_TASKS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.search-tasks.query",
        "GET /api/v1/search/tasks query",
        "GET /api/v1/search/tasks",
        "GET /api/v1/search/tasks",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        SEARCH_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:search-tasks-query:v1",
        "api/search-tasks-query.v1.schema.json",
        "Kanban search tasks query v1",
        "schemas/fixtures/api/search-tasks-query.v1.valid.json",
        "schemas/fixtures/api/search-tasks-query.v1.invalid.json",
        crate::SearchTasksQuery,
        server_witness(
            "suite::derived_adoption::search_tasks_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::search_tasks_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.search-tasks.headers",
        "GET /api/v1/search/tasks headers",
        "GET /api/v1/search/tasks",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:search-tasks-headers:v1",
        "api/search-tasks-headers.v1.schema.json",
        "Kanban api.search-tasks request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.search-tasks.response",
        "GET /api/v1/search/tasks response",
        "GET /api/v1/search/tasks",
        "GET /api/v1/search/tasks",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:search-tasks-response:v1",
        "api/search-tasks-response.v1.schema.json",
        "Kanban search tasks response v1",
        "schemas/fixtures/api/search-tasks-response.v1.valid.json",
        "schemas/fixtures/api/search-tasks-response.v1.invalid.json",
        crate::SearchTasksResponse,
        server_witness(
            "suite::derived_adoption::search_tasks_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::search_tasks_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_SEARCH_TASKS_BY_STATUS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.search-tasks-by-status.query",
        "GET /api/v1/search/tasks/by-status query",
        "GET /api/v1/search/tasks/by-status",
        "GET /api/v1/search/tasks/by-status",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        SEARCH_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:search-tasks-by-status-query:v1",
        "api/search-tasks-by-status-query.v1.schema.json",
        "Kanban search tasks by status query v1",
        "schemas/fixtures/api/search-tasks-by-status-query.v1.valid.json",
        "schemas/fixtures/api/search-tasks-by-status-query.v1.invalid.json",
        crate::SearchTasksQuery,
        server_witness(
            "suite::derived_adoption::search_tasks_by_status_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::search_tasks_by_status_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.search-tasks-by-status.headers",
        "GET /api/v1/search/tasks/by-status headers",
        "GET /api/v1/search/tasks/by-status",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:search-tasks-by-status-headers:v1",
        "api/search-tasks-by-status-headers.v1.schema.json",
        "Kanban api.search-tasks-by-status request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.search-tasks-by-status.response",
        "GET /api/v1/search/tasks/by-status response",
        "GET /api/v1/search/tasks/by-status",
        "GET /api/v1/search/tasks/by-status",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:search-tasks-by-status-response:v1",
        "api/search-tasks-by-status-response.v1.schema.json",
        "Kanban search tasks by status response v1",
        "schemas/fixtures/api/search-tasks-by-status-response.v1.valid.json",
        "schemas/fixtures/api/search-tasks-by-status-response.v1.invalid.json",
        crate::SearchTasksByStatusResponse,
        server_witness(
            "suite::derived_adoption::search_tasks_by_status_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::search_tasks_by_status_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_SEARCH_STATUS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.search-status.query",
        "GET /api/v1/search/status query",
        "GET /api/v1/search/status",
        "GET /api/v1/search/status",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:search-status-query:v1",
        "api/search-status-query.v1.schema.json",
        "Kanban search status query v1",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "schemas/fixtures/api/search-status-query.v1.invalid.json",
        crate::BoardQuery,
        server_witness(
            "suite::derived_adoption::search_status_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::search_status_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.search-status.headers",
        "GET /api/v1/search/status headers",
        "GET /api/v1/search/status",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:search-status-headers:v1",
        "api/search-status-headers.v1.schema.json",
        "Kanban api.search-status request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.search-status.response",
        "GET /api/v1/search/status response",
        "GET /api/v1/search/status",
        "GET /api/v1/search/status",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:search-status-response:v1",
        "api/search-status-response.v1.schema.json",
        "Kanban search status response v1",
        "schemas/fixtures/api/search-status-response.v1.valid.json",
        "schemas/fixtures/api/search-status-response.v1.invalid.json",
        crate::SearchStatusResponse,
        server_witness(
            "suite::derived_adoption::search_status_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::search_status_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_REBUILD_SEARCH_INDEX_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.rebuild-search-index.query",
        "POST /api/v1/search/index/rebuild query",
        "POST /api/v1/search/index/rebuild",
        "POST /api/v1/search/index/rebuild",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:rebuild-search-index-query:v1",
        "api/rebuild-search-index-query.v1.schema.json",
        "Kanban rebuild search index query v1",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "schemas/fixtures/api/search-status-query.v1.invalid.json",
        crate::BoardQuery,
        server_witness(
            "suite::derived_adoption::search_status_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::search_status_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.rebuild-search-index.headers",
        "POST /api/v1/search/index/rebuild headers",
        "POST /api/v1/search/index/rebuild",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:rebuild-search-index-headers:v1",
        "api/rebuild-search-index-headers.v1.schema.json",
        "Kanban api.rebuild-search-index request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.rebuild-search-index.response",
        "POST /api/v1/search/index/rebuild response",
        "POST /api/v1/search/index/rebuild",
        "POST /api/v1/search/index/rebuild",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:rebuild-search-index-response:v1",
        "api/rebuild-search-index-response.v1.schema.json",
        "Kanban rebuild search index response v1",
        "schemas/fixtures/api/search-status-response.v1.valid.json",
        "schemas/fixtures/api/search-status-response.v1.invalid.json",
        crate::SearchStatusResponse,
        server_witness(
            "suite::derived_adoption::search_status_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::search_status_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_SYNC_SEARCH_INDEX_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.sync-search-index.query",
        "POST /api/v1/search/index/sync query",
        "POST /api/v1/search/index/sync",
        "POST /api/v1/search/index/sync",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:sync-search-index-query:v1",
        "api/sync-search-index-query.v1.schema.json",
        "Kanban sync search index query v1",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "schemas/fixtures/api/search-status-query.v1.invalid.json",
        crate::BoardQuery,
        server_witness(
            "suite::derived_adoption::search_status_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::search_status_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.sync-search-index.headers",
        "POST /api/v1/search/index/sync headers",
        "POST /api/v1/search/index/sync",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:sync-search-index-headers:v1",
        "api/sync-search-index-headers.v1.schema.json",
        "Kanban api.sync-search-index request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.sync-search-index.response",
        "POST /api/v1/search/index/sync response",
        "POST /api/v1/search/index/sync",
        "POST /api/v1/search/index/sync",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:sync-search-index-response:v1",
        "api/sync-search-index-response.v1.schema.json",
        "Kanban sync search index response v1",
        "schemas/fixtures/api/search-status-response.v1.valid.json",
        "schemas/fixtures/api/search-status-response.v1.invalid.json",
        crate::SearchStatusResponse,
        server_witness(
            "suite::derived_adoption::search_status_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::search_status_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_BUILD_CONTEXT_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.build-context.path",
        "GET /api/v1/tasks/:task_id/context path",
        "GET /api/v1/tasks/:task_id/context",
        "GET /api/v1/tasks/:task_id/context",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:build-context-path:v1",
        "api/build-context-path.v1.schema.json",
        "Kanban build context path v1",
        "schemas/fixtures/api/build-context-path.v1.valid.json",
        "schemas/fixtures/api/build-context-path.v1.invalid.json",
        crate::BuildContextPath,
        server_witness(
            "suite::derived_adoption::build_context_path_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::build_context_path_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.build-context.headers",
        "GET /api/v1/tasks/:task_id/context headers",
        "GET /api/v1/tasks/:task_id/context",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:build-context-headers:v1",
        "api/build-context-headers.v1.schema.json",
        "Kanban api.build-context request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.build-context.query",
        "GET /api/v1/tasks/:task_id/context query",
        "GET /api/v1/tasks/:task_id/context",
        "GET /api/v1/tasks/:task_id/context",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        CONTEXT_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:build-context-query:v1",
        "api/build-context-query.v1.schema.json",
        "Kanban build context query v1",
        "schemas/fixtures/api/build-context-query.v1.valid.json",
        "schemas/fixtures/api/build-context-query.v1.invalid.json",
        crate::BuildContextQuery,
        server_witness(
            "suite::derived_adoption::build_context_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::build_context_query_fixture_is_consumed_by_real_router"
        )
    ),
    api_contract!(
        "api.build-context.response",
        "GET /api/v1/tasks/:task_id/context response",
        "GET /api/v1/tasks/:task_id/context",
        "GET /api/v1/tasks/:task_id/context",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:build-context-response:v1",
        "api/build-context-response.v1.schema.json",
        "Kanban build context response v1",
        "schemas/fixtures/api/build-context-response.v1.valid.json",
        "schemas/fixtures/api/build-context-response.v1.invalid.json",
        crate::BuildContextResponse,
        server_witness(
            "suite::derived_adoption::build_context_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::build_context_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_GRAPH_STATUS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.graph-status.query",
        "GET /api/v1/graph/status query",
        "GET /api/v1/graph/status",
        "GET /api/v1/graph/status",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:graph-status-query:v1",
        "api/graph-status-query.v1.schema.json",
        "Kanban graph status query v1",
        "schemas/fixtures/api/graph-status-query.v1.valid.json",
        "schemas/fixtures/api/graph-status-query.v1.invalid.json",
        crate::BoardQuery,
        server_witness(
            "suite::derived_adoption::graph_status_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::graph_status_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.graph-status.headers",
        "GET /api/v1/graph/status headers",
        "GET /api/v1/graph/status",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:graph-status-headers:v1",
        "api/graph-status-headers.v1.schema.json",
        "Kanban api.graph-status request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.graph-status.response",
        "GET /api/v1/graph/status response",
        "GET /api/v1/graph/status",
        "GET /api/v1/graph/status",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:graph-status-response:v1",
        "api/graph-status-response.v1.schema.json",
        "Kanban graph status response v1",
        "schemas/fixtures/api/graph-status-response.v1.valid.json",
        "schemas/fixtures/api/graph-status-response.v1.invalid.json",
        crate::GraphStatusResponse,
        server_witness(
            "suite::derived_adoption::graph_status_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::graph_status_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_GRAPH_NEIGHBORS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.graph-neighbors.query",
        "GET /api/v1/graph/neighbors query",
        "GET /api/v1/graph/neighbors",
        "GET /api/v1/graph/neighbors",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        GRAPH_NEIGHBORS_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:graph-neighbors-query:v1",
        "api/graph-neighbors-query.v1.schema.json",
        "Kanban graph neighbors query v1",
        "schemas/fixtures/api/graph-neighbors-query.v1.valid.json",
        "schemas/fixtures/api/graph-neighbors-query.v1.invalid.json",
        crate::GraphNeighborsQuery,
        server_witness(
            "suite::derived_adoption::graph_neighbors_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::graph_neighbors_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.graph-neighbors.headers",
        "GET /api/v1/graph/neighbors headers",
        "GET /api/v1/graph/neighbors",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:graph-neighbors-headers:v1",
        "api/graph-neighbors-headers.v1.schema.json",
        "Kanban api.graph-neighbors request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.graph-neighbors.response",
        "GET /api/v1/graph/neighbors response",
        "GET /api/v1/graph/neighbors",
        "GET /api/v1/graph/neighbors",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:graph-neighbors-response:v1",
        "api/graph-neighbors-response.v1.schema.json",
        "Kanban graph neighbors response v1",
        "schemas/fixtures/api/graph-neighbors-response.v1.valid.json",
        "schemas/fixtures/api/graph-neighbors-response.v1.invalid.json",
        crate::GraphNeighborsResponse,
        server_witness(
            "suite::derived_adoption::graph_neighbors_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::graph_neighbors_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_GRAPH_QUERY_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.graph-query.query",
        "GET /api/v1/graph/query query",
        "GET /api/v1/graph/query",
        "GET /api/v1/graph/query",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        GRAPH_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:graph-query-query:v1",
        "api/graph-query-query.v1.schema.json",
        "Kanban graph query query v1",
        "schemas/fixtures/api/graph-query-query.v1.valid.json",
        "schemas/fixtures/api/graph-query-query.v1.invalid.json",
        crate::GraphQueryQuery,
        server_witness(
            "suite::graph_adoption::graph_query_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::graph_adoption::graph_query_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.graph-query.headers",
        "GET /api/v1/graph/query headers",
        "GET /api/v1/graph/query",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:graph-query-headers:v1",
        "api/graph-query-headers.v1.schema.json",
        "Kanban api.graph-query request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.graph-query.response",
        "GET /api/v1/graph/query response",
        "GET /api/v1/graph/query",
        "GET /api/v1/graph/query",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:graph-query-response:v1",
        "api/graph-query-response.v1.schema.json",
        "Kanban graph query response v1",
        "schemas/fixtures/api/graph-query-response.v1.valid.json",
        "schemas/fixtures/api/graph-query-response.v1.invalid.json",
        crate::cli_helpers::CliGraphQueryOutput,
        server_witness(
            "suite::graph_adoption::graph_query_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::graph_adoption::graph_query_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_GRAPH_REBUILD_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.graph-rebuild.query",
        "POST /api/v1/graph/rebuild query",
        "POST /api/v1/graph/rebuild",
        "POST /api/v1/graph/rebuild",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:graph-rebuild-query:v1",
        "api/graph-rebuild-query.v1.schema.json",
        "Kanban graph rebuild query v1",
        "schemas/fixtures/api/graph-rebuild-query.v1.valid.json",
        "schemas/fixtures/api/graph-rebuild-query.v1.invalid.json",
        crate::BoardQuery,
        server_witness(
            "suite::graph_adoption::graph_rebuild_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::graph_adoption::graph_rebuild_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.graph-rebuild.headers",
        "POST /api/v1/graph/rebuild headers",
        "POST /api/v1/graph/rebuild",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:graph-rebuild-headers:v1",
        "api/graph-rebuild-headers.v1.schema.json",
        "Kanban api.graph-rebuild request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.graph-rebuild.response",
        "POST /api/v1/graph/rebuild response",
        "POST /api/v1/graph/rebuild",
        "POST /api/v1/graph/rebuild",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:graph-rebuild-response:v1",
        "api/graph-rebuild-response.v1.schema.json",
        "Kanban graph rebuild response v1",
        "schemas/fixtures/api/graph-rebuild-response.v1.valid.json",
        "schemas/fixtures/api/graph-rebuild-response.v1.invalid.json",
        crate::GraphMaintenanceResponse,
        server_witness(
            "suite::graph_adoption::graph_rebuild_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::graph_adoption::graph_rebuild_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_GRAPH_SYNC_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.graph-sync.query",
        "POST /api/v1/graph/sync query",
        "POST /api/v1/graph/sync",
        "POST /api/v1/graph/sync",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:graph-sync-query:v1",
        "api/graph-sync-query.v1.schema.json",
        "Kanban graph sync query v1",
        "schemas/fixtures/api/graph-sync-query.v1.valid.json",
        "schemas/fixtures/api/graph-sync-query.v1.invalid.json",
        crate::BoardQuery,
        server_witness(
            "suite::graph_adoption::graph_sync_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::graph_adoption::graph_sync_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.graph-sync.headers",
        "POST /api/v1/graph/sync headers",
        "POST /api/v1/graph/sync",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:graph-sync-headers:v1",
        "api/graph-sync-headers.v1.schema.json",
        "Kanban api.graph-sync request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.graph-sync.response",
        "POST /api/v1/graph/sync response",
        "POST /api/v1/graph/sync",
        "POST /api/v1/graph/sync",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:graph-sync-response:v1",
        "api/graph-sync-response.v1.schema.json",
        "Kanban graph sync response v1",
        "schemas/fixtures/api/graph-sync-response.v1.valid.json",
        "schemas/fixtures/api/graph-sync-response.v1.invalid.json",
        crate::GraphMaintenanceResponse,
        server_witness(
            "suite::graph_adoption::graph_sync_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::graph_adoption::graph_sync_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_LIST_ENTITIES_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.entity-list.query",
        "GET /api/v1/entities query",
        "GET /api/v1/entities",
        "GET /api/v1/entities",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        ENTITY_LIST_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:entity-list-query:v1",
        "api/entity-list-query.v1.schema.json",
        "Kanban entity list query v1",
        "schemas/fixtures/api/entity-list-query.v1.valid.json",
        "schemas/fixtures/api/entity-list-query.v1.invalid.json",
        crate::EntityListQuery,
        server_witness(
            "suite::entity_adoption::entity_list_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::entity_adoption::entity_list_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.entity-list.headers",
        "GET /api/v1/entities headers",
        "GET /api/v1/entities",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:list-entities-headers:v1",
        "api/list-entities-headers.v1.schema.json",
        "Kanban api.list-entities request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.entity-list.response",
        "GET /api/v1/entities response",
        "GET /api/v1/entities",
        "GET /api/v1/entities",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:entity-list-response:v1",
        "api/entity-list-response.v1.schema.json",
        "Kanban entity list response v1",
        "schemas/fixtures/api/entity-list-response.v1.valid.json",
        "schemas/fixtures/api/entity-list-response.v1.invalid.json",
        crate::EntityListResponse,
        server_witness(
            "suite::entity_adoption::entity_list_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::entity_adoption::entity_list_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_UPSERT_ENTITY_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.upsert-entity.headers",
        "PUT /api/v1/entities headers",
        "PUT /api/v1/entities",
        ApiHeaderProfile::LocaleJson,
        "urn:kanban-tool:schema:api:upsert-entity-headers:v1",
        "api/upsert-entity-headers.v1.schema.json",
        "Kanban api.upsert-entity request headers v1",
        "schemas/fixtures/api/headers/locale-json-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-json-headers.v1.invalid.json",
        crate::headers::LocaleJsonHeaders
    ),
    api_contract!(
        "api.entity-upsert.request",
        "PUT /api/v1/entities body",
        "PUT /api/v1/entities",
        "PUT /api/v1/entities",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:entity-upsert-request:v1",
        "api/entity-upsert-request.v1.schema.json",
        "Kanban entity upsert request v1",
        "schemas/fixtures/api/entity-upsert-request.v1.valid.json",
        "schemas/fixtures/api/entity-upsert-request.v1.invalid.json",
        crate::EntityUpsertRequest,
        server_witness(
            "suite::entity_adoption::entity_upsert_request_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::entity_adoption::entity_upsert_request_fixture_is_consumed_by_real_router"
        )
    ),
    api_contract!(
        "api.entity-upsert.response",
        "PUT /api/v1/entities response",
        "PUT /api/v1/entities",
        "PUT /api/v1/entities",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:entity-upsert-response:v1",
        "api/entity-upsert-response.v1.schema.json",
        "Kanban entity upsert response v1",
        "schemas/fixtures/api/entity-upsert-response.v1.valid.json",
        "schemas/fixtures/api/entity-upsert-response.v1.invalid.json",
        crate::EntityResponse,
        server_witness(
            "suite::entity_adoption::entity_upsert_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::entity_adoption::entity_upsert_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_GET_ENTITY_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.entity.path",
        "GET /api/v1/entities/:uri path",
        "GET /api/v1/entities/:uri",
        "GET /api/v1/entities/:uri",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        ENTITY_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:entity-path:v1",
        "api/entity-path.v1.schema.json",
        "Kanban entity path v1",
        "schemas/fixtures/api/entity-path.v1.valid.json",
        "schemas/fixtures/api/entity-path.v1.invalid.json",
        crate::EntityPath,
        server_witness("suite::entity_adoption::entity_path_dto_serializes_to_committed_fixture"),
        server_witness("suite::entity_adoption::entity_path_fixture_is_consumed_by_real_router")
    ),
    header_contract!(
        "api.entity.headers",
        "GET /api/v1/entities/:uri headers",
        "GET /api/v1/entities/:uri",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:get-entity-headers:v1",
        "api/get-entity-headers.v1.schema.json",
        "Kanban api.get-entity request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.entity.response",
        "GET /api/v1/entities/:uri response",
        "GET /api/v1/entities/:uri",
        "GET /api/v1/entities/:uri",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:entity-response:v1",
        "api/entity-response.v1.schema.json",
        "Kanban entity response v1",
        "schemas/fixtures/api/entity-response.v1.valid.json",
        "schemas/fixtures/api/entity-response.v1.invalid.json",
        crate::EntityResponse,
        server_witness(
            "suite::entity_adoption::entity_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::entity_adoption::entity_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_VECTOR_STATUS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.vector-status.query",
        "GET /api/v1/vector/status query",
        "GET /api/v1/vector/status",
        "GET /api/v1/vector/status",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:vector-status-query:v1",
        "api/vector-status-query.v1.schema.json",
        "Kanban vector status query v1",
        "schemas/fixtures/api/vector-status-query.v1.valid.json",
        "schemas/fixtures/api/vector-status-query.v1.invalid.json",
        crate::VectorStatusQuery,
        server_witness(
            "suite::derived_adoption::vector_status_query_dto_serializes_to_committed_fixture"
        ),
        server_witness(
            "suite::derived_adoption::vector_status_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.vector-status.headers",
        "GET /api/v1/vector/status headers",
        "GET /api/v1/vector/status",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:vector-status-headers:v1",
        "api/vector-status-headers.v1.schema.json",
        "Kanban api.vector-status request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.vector-status.response",
        "GET /api/v1/vector/status response",
        "GET /api/v1/vector/status",
        "GET /api/v1/vector/status",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:vector-status-response:v1",
        "api/vector-status-response.v1.schema.json",
        "Kanban vector status response v1",
        "schemas/fixtures/api/vector-status-response.v1.valid.json",
        "schemas/fixtures/api/vector-status-response.v1.invalid.json",
        crate::VectorStatusResponse,
        server_witness(
            "suite::derived_adoption::vector_status_response_fixture_is_produced_by_real_router"
        ),
        server_witness(
            "suite::derived_adoption::vector_status_response_fixture_is_consumed_by_contract_root"
        )
    ),
];

const API_VECTOR_CONFIGURE_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.vector-configure.headers",
        "POST /api/v1/vector/configure headers",
        "POST /api/v1/vector/configure",
        ApiHeaderProfile::LocaleJson,
        "urn:kanban-tool:schema:api:vector-configure-headers:v1",
        "api/vector-configure-headers.v1.schema.json",
        "Kanban api.vector-configure request headers v1",
        "schemas/fixtures/api/headers/locale-json-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-json-headers.v1.invalid.json",
        crate::headers::LocaleJsonHeaders
    ),
    api_contract!(
        "api.vector-configure.request",
        "POST /api/v1/vector/configure request",
        "POST /api/v1/vector/configure",
        "POST /api/v1/vector/configure",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:vector-configure-request:v1",
        "api/vector-configure-request.v1.schema.json",
        "Kanban vector configure request v1",
        "schemas/fixtures/api/vector-configure-request.v1.valid.json",
        "schemas/fixtures/api/vector-configure-request.v1.invalid.json",
        crate::VectorConfigureRequest,
        client_witness("operations::vector::tests::vector_configure_request_fixture_is_produced"),
        server_witness(
            "vector::tests::vector_configure_request_fixture_is_consumed_by_real_router"
        )
    ),
    api_contract!(
        "api.vector-configure.response",
        "POST /api/v1/vector/configure response",
        "POST /api/v1/vector/configure",
        "POST /api/v1/vector/configure",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:vector-configure-response:v1",
        "api/vector-configure-response.v1.schema.json",
        "Kanban vector configure response v1",
        "schemas/fixtures/api/vector-configure-response.v1.valid.json",
        "schemas/fixtures/api/vector-configure-response.v1.invalid.json",
        crate::VectorConfigureResponse,
        server_witness(
            "vector::tests::vector_configure_response_fixture_is_produced_by_real_router"
        ),
        client_witness(
            "operations::vector::tests::vector_configure_response_fixture_is_consumed_by_client"
        )
    ),
];

const API_VECTOR_REBUILD_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.vector-rebuild.headers",
        "POST /api/v1/vector/rebuild headers",
        "POST /api/v1/vector/rebuild",
        ApiHeaderProfile::LocaleJson,
        "urn:kanban-tool:schema:api:vector-rebuild-headers:v1",
        "api/vector-rebuild-headers.v1.schema.json",
        "Kanban api.vector-rebuild request headers v1",
        "schemas/fixtures/api/headers/locale-json-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-json-headers.v1.invalid.json",
        crate::headers::LocaleJsonHeaders
    ),
    api_contract!(
        "api.vector-rebuild.request",
        "POST /api/v1/vector/rebuild request",
        "POST /api/v1/vector/rebuild",
        "POST /api/v1/vector/rebuild",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:vector-rebuild-request:v1",
        "api/vector-rebuild-request.v1.schema.json",
        "Kanban vector rebuild request v1",
        "schemas/fixtures/api/vector-rebuild-request.v1.valid.json",
        "schemas/fixtures/api/vector-rebuild-request.v1.invalid.json",
        crate::VectorProjectionRequest,
        client_witness("operations::vector::tests::vector_rebuild_request_fixture_is_produced"),
        server_witness("vector::tests::vector_rebuild_request_fixture_is_consumed_by_real_router")
    ),
    api_contract!(
        "api.vector-rebuild.response",
        "POST /api/v1/vector/rebuild response",
        "POST /api/v1/vector/rebuild",
        "POST /api/v1/vector/rebuild",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:vector-rebuild-response:v1",
        "api/vector-rebuild-response.v1.schema.json",
        "Kanban vector rebuild response v1",
        "schemas/fixtures/api/vector-rebuild-response.v1.valid.json",
        "schemas/fixtures/api/vector-rebuild-response.v1.invalid.json",
        crate::VectorProjectionResponse,
        server_witness("vector::tests::vector_rebuild_response_fixture_is_produced_by_real_router"),
        client_witness(
            "operations::vector::tests::vector_rebuild_response_fixture_is_consumed_by_client"
        )
    ),
];

const API_VECTOR_SYNC_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.vector-sync.headers",
        "POST /api/v1/vector/sync headers",
        "POST /api/v1/vector/sync",
        ApiHeaderProfile::LocaleJson,
        "urn:kanban-tool:schema:api:vector-sync-headers:v1",
        "api/vector-sync-headers.v1.schema.json",
        "Kanban api.vector-sync request headers v1",
        "schemas/fixtures/api/headers/locale-json-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-json-headers.v1.invalid.json",
        crate::headers::LocaleJsonHeaders
    ),
    api_contract!(
        "api.vector-sync.request",
        "POST /api/v1/vector/sync request",
        "POST /api/v1/vector/sync",
        "POST /api/v1/vector/sync",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:vector-sync-request:v1",
        "api/vector-sync-request.v1.schema.json",
        "Kanban vector sync request v1",
        "schemas/fixtures/api/vector-sync-request.v1.valid.json",
        "schemas/fixtures/api/vector-sync-request.v1.invalid.json",
        crate::VectorProjectionRequest,
        client_witness("operations::vector::tests::vector_sync_request_fixture_is_produced"),
        server_witness("vector::tests::vector_sync_request_fixture_is_consumed_by_real_router")
    ),
    api_contract!(
        "api.vector-sync.response",
        "POST /api/v1/vector/sync response",
        "POST /api/v1/vector/sync",
        "POST /api/v1/vector/sync",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:vector-sync-response:v1",
        "api/vector-sync-response.v1.schema.json",
        "Kanban vector sync response v1",
        "schemas/fixtures/api/vector-sync-response.v1.valid.json",
        "schemas/fixtures/api/vector-sync-response.v1.invalid.json",
        crate::VectorProjectionResponse,
        server_witness("vector::tests::vector_sync_response_fixture_is_produced_by_real_router"),
        client_witness(
            "operations::vector::tests::vector_sync_response_fixture_is_consumed_by_client"
        )
    ),
];

const API_VECTOR_QUERY_CHUNKS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.vector-query-chunks.query",
        "GET /api/v1/vector/query-chunks query",
        "GET /api/v1/vector/query-chunks",
        "GET /api/v1/vector/query-chunks",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        VECTOR_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:vector-query-chunks-query:v1",
        "api/vector-query-chunks-query.v1.schema.json",
        "Kanban vector query chunks query v1",
        "schemas/fixtures/api/vector-query-chunks-query.v1.valid.json",
        "schemas/fixtures/api/vector-query-chunks-query.v1.invalid.json",
        crate::VectorQuery,
        client_witness("operations::vector::tests::vector_query_chunks_query_fixture_is_produced"),
        server_witness(
            "vector::tests::vector_query_chunks_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.vector-query-chunks.headers",
        "GET /api/v1/vector/query-chunks headers",
        "GET /api/v1/vector/query-chunks",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:vector-query-chunks-headers:v1",
        "api/vector-query-chunks-headers.v1.schema.json",
        "Kanban api.vector-query-chunks request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.vector-query-chunks.response",
        "GET /api/v1/vector/query-chunks response",
        "GET /api/v1/vector/query-chunks",
        "GET /api/v1/vector/query-chunks",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:vector-query-chunks-response:v1",
        "api/vector-query-chunks-response.v1.schema.json",
        "Kanban vector query chunks response v1",
        "schemas/fixtures/api/vector-query-chunks-response.v1.valid.json",
        "schemas/fixtures/api/vector-query-chunks-response.v1.invalid.json",
        crate::VectorQueryChunksResponse,
        server_witness(
            "vector::tests::vector_query_chunks_response_fixture_is_produced_by_real_router"
        ),
        client_witness(
            "operations::vector::tests::vector_query_chunks_response_fixture_is_consumed_by_client"
        )
    ),
];

const API_VECTOR_QUERY_LABEL_ATOMS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.vector-query-label-atoms.query",
        "GET /api/v1/vector/query-label-atoms query",
        "GET /api/v1/vector/query-label-atoms",
        "GET /api/v1/vector/query-label-atoms",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        VECTOR_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:vector-query-label-atoms-query:v1",
        "api/vector-query-label-atoms-query.v1.schema.json",
        "Kanban vector query label atoms query v1",
        "schemas/fixtures/api/vector-query-label-atoms-query.v1.valid.json",
        "schemas/fixtures/api/vector-query-label-atoms-query.v1.invalid.json",
        crate::VectorQuery,
        client_witness(
            "operations::vector::tests::vector_query_label_atoms_query_fixture_is_produced"
        ),
        server_witness(
            "vector::tests::vector_query_label_atoms_query_fixture_is_consumed_by_real_router"
        )
    ),
    header_contract!(
        "api.vector-query-label-atoms.headers",
        "GET /api/v1/vector/query-label-atoms headers",
        "GET /api/v1/vector/query-label-atoms",
        ApiHeaderProfile::Locale,
        "urn:kanban-tool:schema:api:vector-query-label-atoms-headers:v1",
        "api/vector-query-label-atoms-headers.v1.schema.json",
        "Kanban api.vector-query-label-atoms request headers v1",
        "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
        "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.vector-query-label-atoms.response",
        "GET /api/v1/vector/query-label-atoms response",
        "GET /api/v1/vector/query-label-atoms",
        "GET /api/v1/vector/query-label-atoms",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:vector-query-label-atoms-response:v1",
        "api/vector-query-label-atoms-response.v1.schema.json",
        "Kanban vector query label atoms response v1",
        "schemas/fixtures/api/vector-query-label-atoms-response.v1.valid.json",
        "schemas/fixtures/api/vector-query-label-atoms-response.v1.invalid.json",
        crate::VectorQueryLabelAtomsResponse,
        server_witness(
            "vector::tests::vector_query_label_atoms_response_fixture_is_produced_by_real_router"
        ),
        client_witness(
            "operations::vector::tests::vector_query_label_atoms_response_fixture_is_consumed_by_client"
        )
    ),
];

const KNOWLEDGE_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.board-task-map",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/task-map"),
        "GET /api/v1/boards/:board/task-map",
        "GET /api/v1/boards/:board/task-map",
        MigrationState::Adopted,
        API_BOARD_TASK_MAP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("board_task_map", "api.board-task-map")),
    OperationDeclaration::new(
        "api.task-neighborhood",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/neighborhood"),
        "GET /api/v1/tasks/:task_id/neighborhood",
        "GET /api/v1/tasks/:task_id/neighborhood",
        MigrationState::Adopted,
        API_TASK_NEIGHBORHOOD_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("task_neighborhood", "api.task-neighborhood")),
    OperationDeclaration::new(
        "api.search-tasks",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/search/tasks"),
        "GET /api/v1/search/tasks",
        "GET /api/v1/search/tasks",
        MigrationState::Adopted,
        API_SEARCH_TASKS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("search_tasks", "api.search-tasks")),
    OperationDeclaration::new(
        "api.search-tasks-by-status",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/search/tasks/by-status"),
        "GET /api/v1/search/tasks/by-status",
        "GET /api/v1/search/tasks/by-status",
        MigrationState::Adopted,
        API_SEARCH_TASKS_BY_STATUS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "search_tasks_by_status",
        "api.search-tasks-by-status"
    )),
    OperationDeclaration::new(
        "api.search-status",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/search/status"),
        "GET /api/v1/search/status",
        "GET /api/v1/search/status",
        MigrationState::Adopted,
        API_SEARCH_STATUS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("search_status", "api.search-status")),
    OperationDeclaration::new(
        "api.rebuild-search-index",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/search/index/rebuild"),
        "POST /api/v1/search/index/rebuild",
        "POST /api/v1/search/index/rebuild",
        MigrationState::Adopted,
        API_REBUILD_SEARCH_INDEX_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("search_index_rebuild", "api.rebuild-search-index")),
    OperationDeclaration::new(
        "api.sync-search-index",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/search/index/sync"),
        "POST /api/v1/search/index/sync",
        "POST /api/v1/search/index/sync",
        MigrationState::Adopted,
        API_SYNC_SEARCH_INDEX_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("search_index_sync", "api.sync-search-index")),
    OperationDeclaration::new(
        "api.build-context",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/context"),
        "GET /api/v1/tasks/:task_id/context",
        "GET /api/v1/tasks/:task_id/context",
        MigrationState::Adopted,
        API_BUILD_CONTEXT_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("context_build", "api.build-context")),
    OperationDeclaration::new(
        "api.graph-status",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/graph/status"),
        "GET /api/v1/graph/status",
        "GET /api/v1/graph/status",
        MigrationState::Adopted,
        API_GRAPH_STATUS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("graph_status", "api.graph-status")),
    OperationDeclaration::new(
        "api.graph-neighbors",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/graph/neighbors"),
        "GET /api/v1/graph/neighbors",
        "GET /api/v1/graph/neighbors",
        MigrationState::Adopted,
        API_GRAPH_NEIGHBORS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("graph_neighbors", "api.graph-neighbors")),
    OperationDeclaration::new(
        "api.graph-query",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/graph/query"),
        "GET /api/v1/graph/query",
        "GET /api/v1/graph/query",
        MigrationState::Adopted,
        API_GRAPH_QUERY_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("graph_query", "api.graph-query")),
    OperationDeclaration::new(
        "api.graph-rebuild",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/graph/rebuild"),
        "POST /api/v1/graph/rebuild",
        "POST /api/v1/graph/rebuild",
        MigrationState::Adopted,
        API_GRAPH_REBUILD_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("graph_rebuild", "api.graph-rebuild")),
    OperationDeclaration::new(
        "api.graph-sync",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/graph/sync"),
        "POST /api/v1/graph/sync",
        "POST /api/v1/graph/sync",
        MigrationState::Adopted,
        API_GRAPH_SYNC_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("graph_sync", "api.graph-sync")),
    OperationDeclaration::new(
        "api.list-entities",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/entities"),
        "GET /api/v1/entities",
        "GET /api/v1/entities",
        MigrationState::Adopted,
        API_LIST_ENTITIES_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("entity_list", "api.list-entities")),
    OperationDeclaration::new(
        "api.upsert-entity",
        ContractSurface::Api,
        Some(HttpMethod::Put),
        Some("/api/v1/entities"),
        "PUT /api/v1/entities",
        "PUT /api/v1/entities",
        MigrationState::Adopted,
        API_UPSERT_ENTITY_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("entity_upsert", "api.upsert-entity")),
    OperationDeclaration::new(
        "api.get-entity",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/entities/:uri"),
        "GET /api/v1/entities/:uri",
        "GET /api/v1/entities/:uri",
        MigrationState::Adopted,
        API_GET_ENTITY_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("entity_show", "api.get-entity")),
    OperationDeclaration::new(
        "api.vector-status",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/vector/status"),
        "GET /api/v1/vector/status",
        "GET /api/v1/vector/status",
        MigrationState::Adopted,
        API_VECTOR_STATUS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("vector_status", "api.vector-status")),
    OperationDeclaration::new(
        "api.vector-configure",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/vector/configure"),
        "POST /api/v1/vector/configure",
        "POST /api/v1/vector/configure",
        MigrationState::Adopted,
        API_VECTOR_CONFIGURE_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("vector_configure", "api.vector-configure")),
    OperationDeclaration::new(
        "api.vector-rebuild",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/vector/rebuild"),
        "POST /api/v1/vector/rebuild",
        "POST /api/v1/vector/rebuild",
        MigrationState::Adopted,
        API_VECTOR_REBUILD_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("vector_rebuild", "api.vector-rebuild")),
    OperationDeclaration::new(
        "api.vector-sync",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/vector/sync"),
        "POST /api/v1/vector/sync",
        "POST /api/v1/vector/sync",
        MigrationState::Adopted,
        API_VECTOR_SYNC_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(policy!("vector_sync", "api.vector-sync")),
    OperationDeclaration::new(
        "api.vector-query-chunks",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/vector/query-chunks"),
        "GET /api/v1/vector/query-chunks",
        "GET /api/v1/vector/query-chunks",
        MigrationState::Adopted,
        API_VECTOR_QUERY_CHUNKS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!("vector_query_chunks", "api.vector-query-chunks")),
    OperationDeclaration::new(
        "api.vector-query-label-atoms",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/vector/query-label-atoms"),
        "GET /api/v1/vector/query-label-atoms",
        "GET /api/v1/vector/query-label-atoms",
        MigrationState::Adopted,
        API_VECTOR_QUERY_LABEL_ATOMS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(
        "vector_query_label_atoms",
        "api.vector-query-label-atoms"
    )),
];

/// Knowledge substrate、search、graph、vector 与 context API parent declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    KNOWLEDGE_OPERATIONS
}

pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(KNOWLEDGE_OPERATIONS).contracts()
}

pub fn endpoint_descriptor(operation_id: &str) -> Option<EndpointDescriptor> {
    crate::CatalogProjection::new(KNOWLEDGE_OPERATIONS)
        .endpoints()
        .into_iter()
        .find(|endpoint| endpoint.operation_id == operation_id)
}

pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    crate::CatalogProjection::new(KNOWLEDGE_OPERATIONS).endpoints()
}

pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(KNOWLEDGE_OPERATIONS).surfaces()
}

pub fn header_profile(operation_id: &str) -> Option<ApiHeaderProfile> {
    KNOWLEDGE_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == operation_id)
        .and_then(|operation| operation.header_profile)
}

pub fn header_contract(operation_id: &str) -> Option<OperationContract> {
    let parent = KNOWLEDGE_OPERATIONS
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
    crate::CatalogProjection::new(KNOWLEDGE_OPERATIONS).schemas()
}

pub fn owns_contract(id: &str) -> bool {
    KNOWLEDGE_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

pub fn owns_operation(id: &str) -> bool {
    KNOWLEDGE_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knowledge_declarations_have_unique_contracts_and_explicit_mcp_bindings() {
        let contracts = operation_contracts();
        let mut ids = contracts
            .iter()
            .map(|contract| contract.id)
            .collect::<Vec<_>>();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert_eq!(KNOWLEDGE_OPERATIONS.len(), 22);
        assert_eq!(contracts.len(), 69);
        assert!(
            KNOWLEDGE_OPERATIONS
                .iter()
                .all(|operation| operation.mcp_policy.is_some())
        );
    }

    #[test]
    fn knowledge_hybrid_projection_replaces_each_legacy_row_once() {
        let inventory = crate::operation_inventory();
        for source in operation_contracts() {
            let matches = inventory
                .iter()
                .filter(|contract| contract.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "Knowledge contract must be projected once: {}",
                source.id
            );
            assert_eq!(
                matches[0], &source,
                "Knowledge contract changed: {}",
                source.id
            );
        }

        let endpoints = crate::endpoint_catalog();
        for source in endpoint_catalog() {
            let matches = endpoints
                .iter()
                .filter(|endpoint| endpoint.operation_id == source.operation_id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "Knowledge endpoint must be projected once"
            );
            assert_eq!(matches[0], &source);
        }
    }
}
