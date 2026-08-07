//! Task read、CRUD 与 lifecycle API family 的唯一 declaration source。
//!
//! parent/child declaration 同时描述 endpoint、headers、schema 和 fixture。真实
//! router/client/MCP adapter 仍由各自 crate 持有；本模块只提供协议事实
//! 及其 deterministic projection。

use crate::{
    ApiHeaderProfile, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, EndpointDescriptor, HttpMethod, HttpTransportLocation,
    McpExposure, McpPolicy, McpToolBinding, OperationContract, OperationDeclaration,
    SurfaceOperation, WireParameter,
};

const TASK_READ_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];

const TASK_CORE_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];

const TASK_TRANSITION_PATH_PARAMETERS: &[WireParameter] = TASK_CORE_PATH_PARAMETERS;

const GET_TASK_QUERY_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "include",
    cardinality: Some(crate::WireParameterCardinality::OptionalOne),
}];

const TASK_READ_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "status",
        cardinality: Some(crate::WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "priority",
        cardinality: Some(crate::WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "label",
        cardinality: Some(crate::WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "plan_filter",
        cardinality: Some(crate::WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "assignee",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "q",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "include_archived",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "offset",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "sort",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
];

const DOMAIN_INVARIANTS: &[crate::McpOperationInvariant] = &[
    crate::McpOperationInvariant::CanonicalHostOnly,
    crate::McpOperationInvariant::SharedApplicationService,
    crate::McpOperationInvariant::NoHostAdminSurface,
];

macro_rules! api_contract {
    (
        $id:expr,
        $path:expr,
        $direction:expr,
        $location:expr,
        $parameters:expr,
        $schema_id:expr,
        $artifact_path:expr,
        $title:expr,
        $valid_fixture:expr,
        $invalid_fixture:expr,
        $schema_type:ty $(,)?
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
        .with_transport(None, $parameters)
        .with_schema(
            $schema_id,
            $artifact_path,
            $title,
            $valid_fixture,
            $invalid_fixture,
        );
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! header_contract {
    ($operation:literal, $path:literal, $profile:expr, $profile_slug:literal, $schema_type:ty $(,)?) => {{
        let contract = ContractDeclaration::new(
            concat!("api.", $operation, ".headers"),
            concat!($path, " headers"),
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, $profile.parameters())
        .with_schema(
            concat!("urn:kanban-tool:schema:api:", $operation, "-headers:v1"),
            concat!("api/", $operation, "-headers.v1.schema.json"),
            concat!("Kanban api.", $operation, " request headers v1"),
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
        );
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! task_policy {
    ($binding:ident, $tool:literal, [$($operation:literal),+ $(,)?]) => {
        const $binding: &[McpToolBinding] = &[McpToolBinding {
            tool_name: $tool,
            http_operations: &[$($operation),+],
        }];
    };
}

task_policy!(TASK_LIST_BINDING, "task_list", ["api.list-tasks"]);
task_policy!(
    TASK_LIST_BY_STATUS_BINDING,
    "task_list_by_status",
    ["api.list-tasks-by-status"]
);
task_policy!(TASK_CREATE_BINDING, "task_create", ["api.create-task"]);
task_policy!(
    TASK_SHOW_BINDING,
    "task_show",
    ["api.list-tasks", "api.get-task"]
);
task_policy!(
    TASK_UPDATE_BINDING,
    "task_update",
    ["api.list-tasks", "api.update-task"]
);
task_policy!(
    TASK_SPECIFY_BINDING,
    "task_specify",
    ["api.list-tasks", "api.specify-task"]
);
task_policy!(
    TASK_PROMOTE_BINDING,
    "task_promote",
    ["api.list-tasks", "api.promote-task"]
);
task_policy!(
    TASK_CLAIM_BINDING,
    "task_claim",
    ["api.list-tasks", "api.claim-task"]
);
task_policy!(
    TASK_REOPEN_BINDING,
    "task_reopen",
    ["api.list-tasks", "api.reopen-task"]
);
task_policy!(
    TASK_RECLAIM_BINDING,
    "task_reclaim",
    ["api.list-tasks", "api.reclaim-task"]
);
task_policy!(
    TASK_HEARTBEAT_BINDING,
    "task_heartbeat",
    ["api.list-tasks", "api.heartbeat-task"]
);
task_policy!(
    TASK_RELEASE_BINDING,
    "task_release",
    ["api.list-tasks", "api.release-task"]
);
task_policy!(
    TASK_DONE_BINDING,
    "task_done",
    ["api.list-tasks", "api.complete-task"]
);
task_policy!(
    TASK_REVIEW_BINDING,
    "task_review",
    ["api.list-tasks", "api.submit-review-task"]
);
task_policy!(
    TASK_BLOCK_BINDING,
    "task_block",
    ["api.list-tasks", "api.block-task"]
);
task_policy!(
    TASK_UNBLOCK_BINDING,
    "task_unblock",
    ["api.list-tasks", "api.unblock-task"]
);
task_policy!(
    TASK_ARCHIVE_BINDING,
    "task_archive",
    ["api.list-tasks", "api.archive-task"]
);

macro_rules! policy {
    ($binding:ident) => {
        McpPolicy {
            exposure: McpExposure::Domain,
            tool_bindings: $binding,
            invariants: DOMAIN_INVARIANTS,
        }
    };
}

const API_LIST_TASKS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.list-tasks.path",
        "GET /api/v1/boards/:board/tasks path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        TASK_READ_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:list-tasks-path:v1",
        "api/list-tasks-path.v1.schema.json",
        "Kanban list tasks path v1",
        "schemas/fixtures/api/list-tasks-path.v1.valid.json",
        "schemas/fixtures/api/list-tasks-path.v1.invalid.json",
        crate::ListTasksPath,
    ),
    api_contract!(
        "api.list-tasks.query",
        "GET /api/v1/boards/:board/tasks query",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        TASK_READ_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:list-tasks-query:v1",
        "api/list-tasks-query.v1.schema.json",
        "Kanban list tasks query v1",
        "schemas/fixtures/api/list-tasks-query.v1.valid.json",
        "schemas/fixtures/api/list-tasks-query.v1.invalid.json",
        crate::ListTasksQuery,
    ),
    header_contract!(
        "list-tasks",
        "GET /api/v1/boards/:board/tasks",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.list-tasks.response",
        "GET /api/v1/boards/:board/tasks response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-tasks-response:v1",
        "api/list-tasks-response.v1.schema.json",
        "Kanban list tasks response v1",
        "schemas/fixtures/api/list-tasks-response.v1.valid.json",
        "schemas/fixtures/api/list-tasks-response.v1.invalid.json",
        crate::ListTasksResponse,
    ),
];

const API_LIST_TASKS_BY_STATUS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.list-tasks-by-status.path",
        "GET /api/v1/boards/:board/tasks/by-status path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        TASK_READ_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:list-tasks-by-status-path:v1",
        "api/list-tasks-by-status-path.v1.schema.json",
        "Kanban list tasks by status path v1",
        "schemas/fixtures/api/list-tasks-by-status-path.v1.valid.json",
        "schemas/fixtures/api/list-tasks-by-status-path.v1.invalid.json",
        crate::ListTasksByStatusPath,
    ),
    api_contract!(
        "api.list-tasks-by-status.query",
        "GET /api/v1/boards/:board/tasks/by-status query",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        TASK_READ_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:list-tasks-by-status-query:v1",
        "api/list-tasks-by-status-query.v1.schema.json",
        "Kanban list tasks by status query v1",
        "schemas/fixtures/api/list-tasks-by-status-query.v1.valid.json",
        "schemas/fixtures/api/list-tasks-by-status-query.v1.invalid.json",
        crate::ListTasksByStatusQuery,
    ),
    header_contract!(
        "list-tasks-by-status",
        "GET /api/v1/boards/:board/tasks/by-status",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.list-tasks-by-status.response",
        "GET /api/v1/boards/:board/tasks/by-status response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-tasks-by-status-response:v1",
        "api/list-tasks-by-status-response.v1.schema.json",
        "Kanban list tasks by status response v1",
        "schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json",
        "schemas/fixtures/api/list-tasks-by-status-response.v1.invalid.json",
        crate::ListTasksByStatusResponse,
    ),
];

const API_CREATE_TASK_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.create-task.path",
        "POST /api/v1/boards/:board/tasks path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        TASK_READ_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:create-task-path:v1",
        "api/create-task-path.v1.schema.json",
        "Kanban create task path v1",
        "schemas/fixtures/api/create-task-path.v1.valid.json",
        "schemas/fixtures/api/create-task-path.v1.invalid.json",
        crate::CreateTaskPath,
    ),
    header_contract!(
        "create-task",
        "POST /api/v1/boards/:board/tasks",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers",
        crate::headers::LocaleActorJsonHeaders
    ),
    api_contract!(
        "api.create-task.request",
        "POST /api/v1/boards/:board/tasks request",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:create-task-request:v1",
        "api/create-task-request.v1.schema.json",
        "Kanban create task request v1",
        "schemas/fixtures/api/create-task-request.v1.valid.json",
        "schemas/fixtures/api/create-task-request.v1.invalid.json",
        crate::CreateTaskRequest,
    ),
    api_contract!(
        "api.create-task.response",
        "POST /api/v1/boards/:board/tasks response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:create-task-response:v1",
        "api/create-task-response.v1.schema.json",
        "Kanban create task response v1",
        "schemas/fixtures/api/create-task-response.v1.valid.json",
        "schemas/fixtures/api/create-task-response.v1.invalid.json",
        crate::CreateTaskResponse,
    ),
];

const API_GET_TASK_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.get-task.path",
        "GET /api/v1/tasks/:task_id path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        TASK_CORE_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:get-task-path:v1",
        "api/get-task-path.v1.schema.json",
        "Kanban get task path v1",
        "schemas/fixtures/api/get-task-path.v1.valid.json",
        "schemas/fixtures/api/get-task-path.v1.invalid.json",
        crate::GetTaskPath,
    ),
    api_contract!(
        "api.get-task.query",
        "GET /api/v1/tasks/:task_id query",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        GET_TASK_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:get-task-query:v1",
        "api/get-task-query.v1.schema.json",
        "Kanban get task query v1",
        "schemas/fixtures/api/get-task-query.v1.valid.json",
        "schemas/fixtures/api/get-task-query.v1.invalid.json",
        crate::GetTaskQuery,
    ),
    header_contract!(
        "get-task",
        "GET /api/v1/tasks/:task_id",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.get-task.response",
        "GET /api/v1/tasks/:task_id response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:get-task-response:v1",
        "api/get-task-response.v1.schema.json",
        "Kanban get task response v1",
        "schemas/fixtures/api/get-task-response.v1.valid.json",
        "schemas/fixtures/api/get-task-response.v1.invalid.json",
        crate::GetTaskResponse,
    ),
];

const API_UPDATE_TASK_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.update-task.path",
        "PATCH /api/v1/tasks/:task_id path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        TASK_CORE_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:update-task-path:v1",
        "api/update-task-path.v1.schema.json",
        "Kanban update task path v1",
        "schemas/fixtures/api/update-task-path.v1.valid.json",
        "schemas/fixtures/api/update-task-path.v1.invalid.json",
        crate::UpdateTaskPath,
    ),
    header_contract!(
        "update-task",
        "PATCH /api/v1/tasks/:task_id",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers",
        crate::headers::LocaleActorJsonHeaders
    ),
    api_contract!(
        "api.update-task.request",
        "PATCH /api/v1/tasks/:task_id body",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:update-task-request:v1",
        "api/update-task-request.v1.schema.json",
        "Kanban update task request v1",
        "schemas/fixtures/api/update-task-request.v1.valid.json",
        "schemas/fixtures/api/update-task-request.v1.invalid.json",
        crate::UpdateTaskRequest,
    ),
    api_contract!(
        "api.update-task.response",
        "PATCH /api/v1/tasks/:task_id response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:update-task-response:v1",
        "api/update-task-response.v1.schema.json",
        "Kanban update task response v1",
        "schemas/fixtures/api/update-task-response.v1.valid.json",
        "schemas/fixtures/api/update-task-response.v1.invalid.json",
        crate::UpdateTaskResponse,
    ),
];

macro_rules! lifecycle_contracts {
    (
        $operation_slug:literal,
        $operation_path:literal,
        $path_type:ty,
        $request_type:ty,
        $response_type:ty,
        $title_prefix:literal,
        $header_profile:expr,
        $header_slug:literal,
        $header_type:ty
    ) => {
        &[
            api_contract!(
                concat!("api.", $operation_slug, ".path"),
                concat!($operation_path, " path"),
                ContractDirection::Deserialize,
                HttpTransportLocation::Path,
                TASK_TRANSITION_PATH_PARAMETERS,
                concat!("urn:kanban-tool:schema:api:", $operation_slug, "-path:v1"),
                concat!("api/", $operation_slug, "-path.v1.schema.json"),
                concat!("Kanban ", $title_prefix, " path v1"),
                concat!(
                    "schemas/fixtures/api/",
                    $operation_slug,
                    "-path.v1.valid.json"
                ),
                concat!(
                    "schemas/fixtures/api/",
                    $operation_slug,
                    "-path.v1.invalid.json"
                ),
                $path_type,
            ),
            header_contract!(
                $operation_slug,
                $operation_path,
                $header_profile,
                $header_slug,
                $header_type
            ),
            api_contract!(
                concat!("api.", $operation_slug, ".request"),
                $operation_path,
                ContractDirection::Deserialize,
                HttpTransportLocation::Body,
                &[],
                concat!(
                    "urn:kanban-tool:schema:api:",
                    $operation_slug,
                    "-request:v1"
                ),
                concat!("api/", $operation_slug, "-request.v1.schema.json"),
                concat!("Kanban ", $title_prefix, " request v1"),
                concat!(
                    "schemas/fixtures/api/",
                    $operation_slug,
                    "-request.v1.valid.json"
                ),
                concat!(
                    "schemas/fixtures/api/",
                    $operation_slug,
                    "-request.v1.invalid.json"
                ),
                $request_type,
            ),
            api_contract!(
                concat!("api.", $operation_slug, ".response"),
                concat!($operation_path, " response"),
                ContractDirection::Serialize,
                HttpTransportLocation::Success,
                &[],
                concat!(
                    "urn:kanban-tool:schema:api:",
                    $operation_slug,
                    "-response:v1"
                ),
                concat!("api/", $operation_slug, "-response.v1.schema.json"),
                concat!("Kanban ", $title_prefix, " response v1"),
                concat!(
                    "schemas/fixtures/api/",
                    $operation_slug,
                    "-response.v1.valid.json"
                ),
                concat!(
                    "schemas/fixtures/api/",
                    $operation_slug,
                    "-response.v1.invalid.json"
                ),
                $response_type,
            ),
        ]
    };
}

const API_SPECIFY_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "specify-task",
    "POST /api/v1/tasks/:task_id/transitions/specify",
    crate::SpecifyTaskPath,
    crate::SpecifyTaskRequest,
    crate::SpecifyTaskResponse,
    "specify task",
    ApiHeaderProfile::LocaleActorJson,
    "locale-actor-json-headers",
    crate::headers::LocaleActorJsonHeaders
);
const API_PROMOTE_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "promote-task",
    "POST /api/v1/tasks/:task_id/transitions/promote",
    crate::PromoteTaskPath,
    crate::PromoteTaskRequest,
    crate::PromoteTaskResponse,
    "promote task",
    ApiHeaderProfile::LocaleActorOptionalJson,
    "locale-actor-optional-json-headers",
    crate::headers::LocaleActorOptionalJsonHeaders
);
const API_CLAIM_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "claim-task",
    "POST /api/v1/tasks/:task_id/transitions/claim",
    crate::ClaimTaskPath,
    crate::ClaimTaskRequest,
    crate::ClaimTaskResponse,
    "claim task",
    ApiHeaderProfile::LocaleActorJson,
    "locale-actor-json-headers",
    crate::headers::LocaleActorJsonHeaders
);
const API_REOPEN_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "reopen-task",
    "POST /api/v1/tasks/:task_id/transitions/reopen",
    crate::ReopenTaskPath,
    crate::ReopenTaskRequest,
    crate::ReopenTaskResponse,
    "reopen task",
    ApiHeaderProfile::LocaleActorJson,
    "locale-actor-json-headers",
    crate::headers::LocaleActorJsonHeaders
);
const API_RECLAIM_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "reclaim-task",
    "POST /api/v1/tasks/:task_id/transitions/reclaim",
    crate::ReclaimTaskPath,
    crate::ReclaimTaskRequest,
    crate::ReclaimTaskResponse,
    "reclaim task",
    ApiHeaderProfile::LocaleActorOptionalJson,
    "locale-actor-optional-json-headers",
    crate::headers::LocaleActorOptionalJsonHeaders
);
const API_HEARTBEAT_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "heartbeat-task",
    "POST /api/v1/tasks/:task_id/transitions/heartbeat",
    crate::HeartbeatTaskPath,
    crate::HeartbeatTaskRequest,
    crate::HeartbeatTaskResponse,
    "heartbeat task",
    ApiHeaderProfile::LocaleActorJson,
    "locale-actor-json-headers",
    crate::headers::LocaleActorJsonHeaders
);
const API_RELEASE_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "release-task",
    "POST /api/v1/tasks/:task_id/transitions/release",
    crate::ReleaseTaskPath,
    crate::ReleaseTaskRequest,
    crate::ReleaseTaskResponse,
    "release task",
    ApiHeaderProfile::LocaleActorJson,
    "locale-actor-json-headers",
    crate::headers::LocaleActorJsonHeaders
);
const API_COMPLETE_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "complete-task",
    "POST /api/v1/tasks/:task_id/transitions/complete",
    crate::CompleteTaskPath,
    crate::CompleteTaskRequest,
    crate::CompleteTaskResponse,
    "complete task",
    ApiHeaderProfile::LocaleActorJson,
    "locale-actor-json-headers",
    crate::headers::LocaleActorJsonHeaders
);
const API_SUBMIT_REVIEW_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "submit-review-task",
    "POST /api/v1/tasks/:task_id/transitions/submit-review",
    crate::SubmitReviewTaskPath,
    crate::SubmitReviewTaskRequest,
    crate::SubmitReviewTaskResponse,
    "submit review task",
    ApiHeaderProfile::LocaleActorJson,
    "locale-actor-json-headers",
    crate::headers::LocaleActorJsonHeaders
);
const API_BLOCK_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "block-task",
    "POST /api/v1/tasks/:task_id/transitions/block",
    crate::BlockTaskPath,
    crate::BlockTaskRequest,
    crate::BlockTaskResponse,
    "block task",
    ApiHeaderProfile::LocaleActorJson,
    "locale-actor-json-headers",
    crate::headers::LocaleActorJsonHeaders
);
const API_UNBLOCK_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "unblock-task",
    "POST /api/v1/tasks/:task_id/transitions/unblock",
    crate::UnblockTaskPath,
    crate::UnblockTaskRequest,
    crate::UnblockTaskResponse,
    "unblock task",
    ApiHeaderProfile::LocaleActorOptionalJson,
    "locale-actor-optional-json-headers",
    crate::headers::LocaleActorOptionalJsonHeaders
);
const API_ARCHIVE_TASK_CONTRACTS: &[ContractDeclaration] = lifecycle_contracts!(
    "archive-task",
    "POST /api/v1/tasks/:task_id/transitions/archive",
    crate::ArchiveTaskPath,
    crate::ArchiveTaskRequest,
    crate::ArchiveTaskResponse,
    "archive task",
    ApiHeaderProfile::LocaleActorOptionalJson,
    "locale-actor-optional-json-headers",
    crate::headers::LocaleActorOptionalJsonHeaders
);

const TASK_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.list-tasks",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/tasks"),
        "GET /api/v1/boards/:board/tasks",
        "GET /api/v1/boards/:board/tasks",
        API_LIST_TASKS_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(TASK_LIST_BINDING)),
    OperationDeclaration::new(
        "api.list-tasks-by-status",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/tasks/by-status"),
        "GET /api/v1/boards/:board/tasks/by-status",
        "GET /api/v1/boards/:board/tasks/by-status",
        API_LIST_TASKS_BY_STATUS_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(TASK_LIST_BY_STATUS_BINDING)),
    OperationDeclaration::new(
        "api.create-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/tasks"),
        "POST /api/v1/boards/:board/tasks",
        "POST /api/v1/boards/:board/tasks",
        API_CREATE_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_CREATE_BINDING)),
    OperationDeclaration::new(
        "api.get-task",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id"),
        "GET /api/v1/tasks/:task_id",
        "GET /api/v1/tasks/:task_id",
        API_GET_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(TASK_SHOW_BINDING)),
    OperationDeclaration::new(
        "api.update-task",
        ContractSurface::Api,
        Some(HttpMethod::Patch),
        Some("/api/v1/tasks/:task_id"),
        "PATCH /api/v1/tasks/:task_id",
        "PATCH /api/v1/tasks/:task_id",
        API_UPDATE_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_UPDATE_BINDING)),
    OperationDeclaration::new(
        "api.specify-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/specify"),
        "POST /api/v1/tasks/:task_id/transitions/specify",
        "POST /api/v1/tasks/:task_id/transitions/specify",
        API_SPECIFY_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_SPECIFY_BINDING)),
    OperationDeclaration::new(
        "api.promote-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/promote"),
        "POST /api/v1/tasks/:task_id/transitions/promote",
        "POST /api/v1/tasks/:task_id/transitions/promote",
        API_PROMOTE_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorOptionalJson)
    .with_mcp_policy(policy!(TASK_PROMOTE_BINDING)),
    OperationDeclaration::new(
        "api.claim-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/claim"),
        "POST /api/v1/tasks/:task_id/transitions/claim",
        "POST /api/v1/tasks/:task_id/transitions/claim",
        API_CLAIM_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_CLAIM_BINDING)),
    OperationDeclaration::new(
        "api.reopen-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/reopen"),
        "POST /api/v1/tasks/:task_id/transitions/reopen",
        "POST /api/v1/tasks/:task_id/transitions/reopen",
        API_REOPEN_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_REOPEN_BINDING)),
    OperationDeclaration::new(
        "api.reclaim-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/reclaim"),
        "POST /api/v1/tasks/:task_id/transitions/reclaim",
        "POST /api/v1/tasks/:task_id/transitions/reclaim",
        API_RECLAIM_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorOptionalJson)
    .with_mcp_policy(policy!(TASK_RECLAIM_BINDING)),
    OperationDeclaration::new(
        "api.heartbeat-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/heartbeat"),
        "POST /api/v1/tasks/:task_id/transitions/heartbeat",
        "POST /api/v1/tasks/:task_id/transitions/heartbeat",
        API_HEARTBEAT_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_HEARTBEAT_BINDING)),
    OperationDeclaration::new(
        "api.release-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/release"),
        "POST /api/v1/tasks/:task_id/transitions/release",
        "POST /api/v1/tasks/:task_id/transitions/release",
        API_RELEASE_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_RELEASE_BINDING)),
    OperationDeclaration::new(
        "api.complete-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/complete"),
        "POST /api/v1/tasks/:task_id/transitions/complete",
        "POST /api/v1/tasks/:task_id/transitions/complete",
        API_COMPLETE_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_DONE_BINDING)),
    OperationDeclaration::new(
        "api.submit-review-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/submit-review"),
        "POST /api/v1/tasks/:task_id/transitions/submit-review",
        "POST /api/v1/tasks/:task_id/transitions/submit-review",
        API_SUBMIT_REVIEW_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_REVIEW_BINDING)),
    OperationDeclaration::new(
        "api.block-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/block"),
        "POST /api/v1/tasks/:task_id/transitions/block",
        "POST /api/v1/tasks/:task_id/transitions/block",
        API_BLOCK_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(TASK_BLOCK_BINDING)),
    OperationDeclaration::new(
        "api.unblock-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/unblock"),
        "POST /api/v1/tasks/:task_id/transitions/unblock",
        "POST /api/v1/tasks/:task_id/transitions/unblock",
        API_UNBLOCK_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorOptionalJson)
    .with_mcp_policy(policy!(TASK_UNBLOCK_BINDING)),
    OperationDeclaration::new(
        "api.archive-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/transitions/archive"),
        "POST /api/v1/tasks/:task_id/transitions/archive",
        "POST /api/v1/tasks/:task_id/transitions/archive",
        API_ARCHIVE_TASK_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorOptionalJson)
    .with_mcp_policy(policy!(TASK_ARCHIVE_BINDING)),
];

/// Task read、CRUD 与 lifecycle API parent declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    TASK_OPERATIONS
}

pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(TASK_OPERATIONS).contracts()
}

pub fn operation_contract(id: &str) -> Option<OperationContract> {
    operation_contracts()
        .into_iter()
        .find(|contract| contract.id == id)
}

pub fn endpoint_descriptor(operation_id: &str) -> Option<EndpointDescriptor> {
    crate::CatalogProjection::new(TASK_OPERATIONS)
        .endpoints()
        .into_iter()
        .find(|endpoint| endpoint.operation_id == operation_id)
}

pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    crate::CatalogProjection::new(TASK_OPERATIONS).endpoints()
}

pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(TASK_OPERATIONS).surfaces()
}

pub fn header_profile(operation_id: &str) -> Option<ApiHeaderProfile> {
    TASK_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == operation_id)
        .and_then(|operation| operation.header_profile)
}

pub fn header_contract(operation_id: &str) -> Option<OperationContract> {
    let parent = TASK_OPERATIONS
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
    crate::CatalogProjection::new(TASK_OPERATIONS).schemas()
}

pub fn owns_contract(id: &str) -> bool {
    TASK_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

pub fn owns_operation(id: &str) -> bool {
    TASK_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_declarations_have_unique_contracts_and_explicit_mcp_bindings() {
        let contracts = operation_contracts();
        let mut ids = contracts
            .iter()
            .map(|contract| contract.id)
            .collect::<Vec<_>>();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert_eq!(TASK_OPERATIONS.len(), 17);
        assert_eq!(contracts.len(), 68);
        assert!(
            TASK_OPERATIONS
                .iter()
                .all(|operation| operation.mcp_policy.is_some())
        );
    }

    #[test]
    fn task_hybrid_projection_replaces_each_legacy_row_once() {
        let inventory = crate::operation_inventory();
        for source in operation_contracts() {
            let matches = inventory
                .iter()
                .filter(|contract| contract.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "Task contract must be projected once: {}",
                source.id
            );
            assert_eq!(matches[0], &source, "Task contract changed: {}", source.id);
        }

        let endpoints = crate::endpoint_catalog();
        for source in endpoint_catalog() {
            let matches = endpoints
                .iter()
                .filter(|endpoint| endpoint.operation_id == source.operation_id)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "Task endpoint must be projected once");
            assert_eq!(matches[0], &source);
        }
    }
}
