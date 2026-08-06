//! Comments、attachments、runs、events API/SSE family 的唯一 declaration source。

use crate::{
    AdoptionLocator, ApiHeaderProfile, ContractBinding, ContractDeclaration, ContractDirection,
    ContractGranularity, ContractStrictness, ContractSurface, EndpointDescriptor,
    EndpointObligation, EndpointObligationKind, HttpMethod, HttpTransportLocation, McpExposure,
    McpPolicy, McpToolBinding, MigrationState, OperationContract, OperationDeclaration,
    SurfaceOperation, WireParameter,
};

const COMMENT_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];
const ATTACHMENT_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];
const ATTACHMENT_ITEM_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "task_id",
        cardinality: Some(crate::WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "attachment_id",
        cardinality: Some(crate::WireParameterCardinality::RequiredOne),
    },
];
const RUN_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];
const RUN_ID_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "run_id",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];
const LIST_EVENTS_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "task_id",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "after",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit",
        cardinality: Some(crate::WireParameterCardinality::OptionalOne),
    },
];

const RUN_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test:
        "http::operations::contract_adoption::suite_runs_and_logs_adoption_uses_real_router_paths_and_fixtures",
};
const COMMENT_ATTACHMENT_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test:
        "http::operations::contract_adoption::suite_comments_and_attachments_adoption_uses_real_router_fixtures",
};
const EVENT_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test:
        "http::operations::contract_adoption::suite_events_sse_and_stats_adoption_use_query_fixtures",
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
const HEADER_ACTOR_JSON_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test: "knowledge_adoption::locale_actor_json_header_fixture_is_consumed_by_real_router",
};

const DOMAIN_INVARIANTS: &[crate::McpOperationInvariant] = &[
    crate::McpOperationInvariant::CanonicalHostOnly,
    crate::McpOperationInvariant::SharedApplicationService,
    crate::McpOperationInvariant::NoHostAdminSurface,
];

macro_rules! api_contract {
    (
        $witness:expr, $id:expr, $path:expr, $direction:expr, $location:expr,
        $parameters:expr, $schema_id:expr, $artifact_path:expr, $title:expr,
        $valid_fixture:expr, $invalid_fixture:expr, $schema_type:ty
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
        )
        .with_adoption($witness, $witness);
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! header_contract {
    ($operation:literal, $path:literal, $profile:expr, $profile_slug:literal) => {{
        let witness = match $profile {
            ApiHeaderProfile::Locale => HEADER_LOCALE_WITNESS,
            ApiHeaderProfile::LocaleActor => HEADER_ACTOR_WITNESS,
            ApiHeaderProfile::LocaleActorJson => HEADER_ACTOR_JSON_WITNESS,
            _ => panic!("history headers 不支持该 profile"),
        };
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
        )
        .with_adoption(witness, witness);
        #[cfg(feature = "schema")]
        let contract = match $profile {
            ApiHeaderProfile::Locale => {
                contract.with_schema_type::<crate::headers::LocaleHeaders>()
            }
            ApiHeaderProfile::LocaleActor => {
                contract.with_schema_type::<crate::headers::LocaleActorHeaders>()
            }
            ApiHeaderProfile::LocaleActorJson => {
                contract.with_schema_type::<crate::headers::LocaleActorJsonHeaders>()
            }
            ApiHeaderProfile::LocaleJson | ApiHeaderProfile::LocaleActorOptionalJson => {
                unreachable!()
            }
        };
        contract
    }};
}

macro_rules! binding {
    ($name:ident, $tool:literal, [$($operation:literal),+ $(,)?]) => {
        const $name: &[McpToolBinding] = &[McpToolBinding {
            tool_name: $tool,
            http_operations: &[$($operation),+],
        }];
    };
}
macro_rules! policy {
    ($binding:ident) => {
        McpPolicy {
            exposure: McpExposure::Domain,
            tool_bindings: $binding,
            invariants: DOMAIN_INVARIANTS,
        }
    };
}

binding!(
    ATTACHMENT_CREATE_BINDING,
    "attachment_create",
    ["api.list-tasks", "api.create-attachment"]
);
binding!(
    ATTACHMENT_DOWNLOAD_BINDING,
    "attachment_download",
    ["api.list-tasks", "api.download-attachment"]
);
binding!(
    ATTACHMENT_LIST_BINDING,
    "attachment_list",
    ["api.list-tasks", "api.list-attachments"]
);
binding!(
    ATTACHMENT_REMOVE_BINDING,
    "attachment_remove",
    ["api.list-tasks", "api.delete-attachment"]
);
binding!(
    COMMENT_CREATE_BINDING,
    "comment_create",
    ["api.list-tasks", "api.create-comment"]
);
binding!(
    COMMENT_LIST_BINDING,
    "comment_list",
    ["api.list-tasks", "api.list-comments"]
);
binding!(
    EVENT_LIST_BINDING,
    "event_list",
    ["api.list-tasks", "api.list-events"]
);
binding!(
    RUN_LIST_BINDING,
    "run_list",
    ["api.list-tasks", "api.list-runs"]
);
binding!(RUN_LOG_BINDING, "run_log", ["api.get-run-log"]);
binding!(RUN_SHOW_BINDING, "run_show", ["api.get-run"]);

const API_LIST_RUNS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        RUN_WITNESS,
        "api.list-runs.path",
        "GET /api/v1/tasks/:task_id/runs path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        RUN_TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:list-runs-path:v1",
        "api/list-runs-path.v1.schema.json",
        "Kanban list runs path v1",
        "schemas/fixtures/api/list-runs-path.v1.valid.json",
        "schemas/fixtures/api/list-runs-path.v1.invalid.json",
        crate::ListRunsPath
    ),
    header_contract!(
        "list-runs",
        "GET /api/v1/tasks/:task_id/runs",
        ApiHeaderProfile::Locale,
        "locale-headers"
    ),
    api_contract!(
        RUN_WITNESS,
        "api.list-runs.response",
        "GET /api/v1/tasks/:task_id/runs response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-runs-response:v1",
        "api/list-runs-response.v1.schema.json",
        "Kanban list runs response v1",
        "schemas/fixtures/api/list-runs-response.v1.valid.json",
        "schemas/fixtures/api/list-runs-response.v1.invalid.json",
        crate::ListRunsResponse
    ),
];

const API_GET_RUN_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        RUN_WITNESS,
        "api.get-run.path",
        "GET /api/v1/runs/:run_id path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        RUN_ID_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:get-run-path:v1",
        "api/get-run-path.v1.schema.json",
        "Kanban get run path v1",
        "schemas/fixtures/api/get-run-path.v1.valid.json",
        "schemas/fixtures/api/get-run-path.v1.invalid.json",
        crate::GetRunPath
    ),
    header_contract!(
        "get-run",
        "GET /api/v1/runs/:run_id",
        ApiHeaderProfile::Locale,
        "locale-headers"
    ),
    api_contract!(
        RUN_WITNESS,
        "api.get-run.response",
        "GET /api/v1/runs/:run_id response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:get-run-response:v1",
        "api/get-run-response.v1.schema.json",
        "Kanban get run response v1",
        "schemas/fixtures/api/get-run-response.v1.valid.json",
        "schemas/fixtures/api/get-run-response.v1.invalid.json",
        crate::GetRunResponse
    ),
];

const API_GET_RUN_LOG_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        RUN_WITNESS,
        "api.get-run-log.path",
        "GET /api/v1/runs/:run_id/log path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        RUN_ID_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:get-run-log-path:v1",
        "api/get-run-log-path.v1.schema.json",
        "Kanban API get run log path v1",
        "schemas/fixtures/api/get-run-log-path.v1.valid.json",
        "schemas/fixtures/api/get-run-log-path.v1.invalid.json",
        crate::GetRunLogPath
    ),
    header_contract!(
        "get-run-log",
        "GET /api/v1/runs/:run_id/log",
        ApiHeaderProfile::Locale,
        "locale-headers"
    ),
    api_contract!(
        RUN_WITNESS,
        "api.get-run-log.response",
        "GET /api/v1/runs/:run_id/log response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:get-run-log-response:v1",
        "api/get-run-log-response.v1.schema.json",
        "Kanban API get run log response v1",
        "schemas/fixtures/api/get-run-log-response.v1.valid.json",
        "schemas/fixtures/api/get-run-log-response.v1.invalid.json",
        crate::GetRunLogResponse
    ),
];

const API_LIST_COMMENTS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.list-comments.path",
        "GET /api/v1/tasks/:task_id/comments path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        COMMENT_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:list-comments-path:v1",
        "api/list-comments-path.v1.schema.json",
        "Kanban list comments path v1",
        "schemas/fixtures/api/list-comments-path.v1.valid.json",
        "schemas/fixtures/api/list-comments-path.v1.invalid.json",
        crate::ListCommentsPath
    ),
    header_contract!(
        "list-comments",
        "GET /api/v1/tasks/:task_id/comments",
        ApiHeaderProfile::Locale,
        "locale-headers"
    ),
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.list-comments.response",
        "GET /api/v1/tasks/:task_id/comments response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-comments-response:v1",
        "api/list-comments-response.v1.schema.json",
        "Kanban list comments response v1",
        "schemas/fixtures/api/list-comments-response.v1.valid.json",
        "schemas/fixtures/api/list-comments-response.v1.invalid.json",
        crate::ListCommentsResponse
    ),
];

const API_CREATE_COMMENT_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.create-comment.path",
        "POST /api/v1/tasks/:task_id/comments path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        COMMENT_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:create-comment-path:v1",
        "api/create-comment-path.v1.schema.json",
        "Kanban create comment path v1",
        "schemas/fixtures/api/create-comment-path.v1.valid.json",
        "schemas/fixtures/api/create-comment-path.v1.invalid.json",
        crate::CreateCommentPath
    ),
    header_contract!(
        "create-comment",
        "POST /api/v1/tasks/:task_id/comments",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers"
    ),
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.create-comment.request",
        "POST /api/v1/tasks/:task_id/comments request",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:create-comment-request:v1",
        "api/create-comment-request.v1.schema.json",
        "Kanban create comment request v1",
        "schemas/fixtures/api/create-comment-request.v1.valid.json",
        "schemas/fixtures/api/create-comment-request.v1.invalid.json",
        crate::CreateCommentRequest
    ),
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.create-comment.response",
        "POST /api/v1/tasks/:task_id/comments response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:create-comment-response:v1",
        "api/create-comment-response.v1.schema.json",
        "Kanban create comment response v1",
        "schemas/fixtures/api/create-comment-response.v1.valid.json",
        "schemas/fixtures/api/create-comment-response.v1.invalid.json",
        crate::CreateCommentResponse
    ),
];

const API_LIST_ATTACHMENTS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.list-attachments.path",
        "GET /api/v1/tasks/:task_id/attachments path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        ATTACHMENT_TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:list-attachments-path:v1",
        "api/list-attachments-path.v1.schema.json",
        "Kanban API list attachments path v1",
        "schemas/fixtures/api/list-attachments-path.v1.valid.json",
        "schemas/fixtures/api/list-attachments-path.v1.invalid.json",
        crate::ListAttachmentsPath
    ),
    header_contract!(
        "list-attachments",
        "GET /api/v1/tasks/:task_id/attachments",
        ApiHeaderProfile::Locale,
        "locale-headers"
    ),
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.list-attachments.response",
        "GET /api/v1/tasks/:task_id/attachments success",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-attachments-response:v1",
        "api/list-attachments-response.v1.schema.json",
        "Kanban API list attachments response v1",
        "schemas/fixtures/api/list-attachments-response.v1.valid.json",
        "schemas/fixtures/api/list-attachments-response.v1.invalid.json",
        crate::ListAttachmentsResponse
    ),
];

const API_CREATE_ATTACHMENT_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.create-attachment.path",
        "POST /api/v1/tasks/:task_id/attachments path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        ATTACHMENT_TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:create-attachment-path:v1",
        "api/create-attachment-path.v1.schema.json",
        "Kanban API create attachment path v1",
        "schemas/fixtures/api/create-attachment-path.v1.valid.json",
        "schemas/fixtures/api/create-attachment-path.v1.invalid.json",
        crate::CreateAttachmentPath
    ),
    header_contract!(
        "create-attachment",
        "POST /api/v1/tasks/:task_id/attachments",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers"
    ),
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.create-attachment.request",
        "POST /api/v1/tasks/:task_id/attachments body",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:create-attachment-request:v1",
        "api/create-attachment-request.v1.schema.json",
        "Kanban API create attachment request v1",
        "schemas/fixtures/api/create-attachment-request.v1.valid.json",
        "schemas/fixtures/api/create-attachment-request.v1.invalid.json",
        crate::CreateAttachmentRequest
    ),
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.create-attachment.response",
        "POST /api/v1/tasks/:task_id/attachments success",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:create-attachment-response:v1",
        "api/create-attachment-response.v1.schema.json",
        "Kanban API create attachment response v1",
        "schemas/fixtures/api/create-attachment-response.v1.valid.json",
        "schemas/fixtures/api/create-attachment-response.v1.invalid.json",
        crate::CreateAttachmentResponse
    ),
];

const API_DOWNLOAD_ATTACHMENT_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.download-attachment.path",
        "GET /api/v1/tasks/:task_id/attachments/:attachment_id path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        ATTACHMENT_ITEM_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:download-attachment-path:v1",
        "api/download-attachment-path.v1.schema.json",
        "Kanban API download attachment path v1",
        "schemas/fixtures/api/download-attachment-path.v1.valid.json",
        "schemas/fixtures/api/download-attachment-path.v1.invalid.json",
        crate::GetAttachmentPath
    ),
    header_contract!(
        "download-attachment",
        "GET /api/v1/tasks/:task_id/attachments/:attachment_id",
        ApiHeaderProfile::Locale,
        "locale-headers"
    ),
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.download-attachment.response",
        "GET /api/v1/tasks/:task_id/attachments/:attachment_id success",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:download-attachment-response:v1",
        "api/download-attachment-response.v1.schema.json",
        "Kanban API download attachment bytes v1",
        "schemas/fixtures/api/download-attachment-response.v1.valid.json",
        "schemas/fixtures/api/download-attachment-response.v1.invalid.json",
        crate::AttachmentDownloadResponse
    ),
];

const API_DELETE_ATTACHMENT_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.delete-attachment.path",
        "DELETE /api/v1/tasks/:task_id/attachments/:attachment_id path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        ATTACHMENT_ITEM_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:delete-attachment-path:v1",
        "api/delete-attachment-path.v1.schema.json",
        "Kanban API delete attachment path v1",
        "schemas/fixtures/api/delete-attachment-path.v1.valid.json",
        "schemas/fixtures/api/delete-attachment-path.v1.invalid.json",
        crate::DeleteAttachmentPath
    ),
    header_contract!(
        "delete-attachment",
        "DELETE /api/v1/tasks/:task_id/attachments/:attachment_id",
        ApiHeaderProfile::LocaleActor,
        "locale-actor-headers"
    ),
    api_contract!(
        COMMENT_ATTACHMENT_WITNESS,
        "api.delete-attachment.response",
        "DELETE /api/v1/tasks/:task_id/attachments/:attachment_id success",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:delete-attachment-response:v1",
        "api/delete-attachment-response.v1.schema.json",
        "Kanban API delete attachment response v1",
        "schemas/fixtures/api/delete-attachment-response.v1.valid.json",
        "schemas/fixtures/api/delete-attachment-response.v1.invalid.json",
        crate::DeleteAttachmentResponse
    ),
];

const API_LIST_EVENTS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        EVENT_WITNESS,
        "api.list-events.query",
        "GET /api/v1/events query",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        LIST_EVENTS_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:list-events-query:v1",
        "api/list-events-query.v1.schema.json",
        "Kanban list events query v1",
        "schemas/fixtures/api/list-events-query.v1.valid.json",
        "schemas/fixtures/api/list-events-query.v1.invalid.json",
        crate::ListEventsQuery
    ),
    header_contract!(
        "list-events",
        "GET /api/v1/events",
        ApiHeaderProfile::Locale,
        "locale-headers"
    ),
    api_contract!(
        EVENT_WITNESS,
        "api.list-events.response",
        "GET /api/v1/events response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-events-response:v1",
        "api/list-events-response.v1.schema.json",
        "Kanban API list events response v1",
        "schemas/fixtures/api/list-events-response.v1.valid.json",
        "schemas/fixtures/api/list-events-response.v1.invalid.json",
        crate::ListEventsResponse
    ),
];

const SSE_STREAM_EVENTS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        EVENT_WITNESS,
        "sse.stream-events.query",
        "GET /api/v1/stream/events query",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        LIST_EVENTS_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:sse:stream-events-query:v1",
        "sse/stream-events-query.v1.schema.json",
        "Kanban SSE stream events query v1",
        "schemas/fixtures/sse/stream-events-query.v1.valid.json",
        "schemas/fixtures/sse/stream-events-query.v1.invalid.json",
        crate::StreamEventsQuery
    ),
    api_contract!(
        EVENT_WITNESS,
        "sse.event.data",
        "GET /api/v1/stream/events data",
        ContractDirection::Serialize,
        HttpTransportLocation::Sse,
        &[],
        "urn:kanban-tool:schema:sse:stream-event-data:v1",
        "sse/stream-event-data.v1.schema.json",
        "Kanban SSE stream event data v1",
        "schemas/fixtures/sse/stream-event-data.v1.valid.json",
        "schemas/fixtures/sse/stream-event-data.v1.invalid.json",
        crate::StreamEventData
    ),
];

const HISTORY_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.list-runs",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/runs"),
        "GET /api/v1/tasks/:task_id/runs",
        "GET /api/v1/tasks/:task_id/runs",
        MigrationState::Adopted,
        API_LIST_RUNS_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(RUN_LIST_BINDING)),
    OperationDeclaration::new(
        "api.get-run",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/runs/:run_id"),
        "GET /api/v1/runs/:run_id",
        "GET /api/v1/runs/:run_id",
        MigrationState::Adopted,
        API_GET_RUN_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(RUN_SHOW_BINDING)),
    OperationDeclaration::new(
        "api.get-run-log",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/runs/:run_id/log"),
        "GET /api/v1/runs/:run_id/log",
        "GET /api/v1/runs/:run_id/log",
        MigrationState::Adopted,
        API_GET_RUN_LOG_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(RUN_LOG_BINDING)),
    OperationDeclaration::new(
        "api.list-comments",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/comments"),
        "GET /api/v1/tasks/:task_id/comments",
        "GET /api/v1/tasks/:task_id/comments",
        MigrationState::Adopted,
        API_LIST_COMMENTS_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(COMMENT_LIST_BINDING)),
    OperationDeclaration::new(
        "api.create-comment",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/comments"),
        "POST /api/v1/tasks/:task_id/comments",
        "POST /api/v1/tasks/:task_id/comments",
        MigrationState::Adopted,
        API_CREATE_COMMENT_CONTRACTS,
    )
    .with_shared_components(&["api.error.response"])
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(COMMENT_CREATE_BINDING)),
    OperationDeclaration::new(
        "api.list-attachments",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/attachments"),
        "GET /api/v1/tasks/:task_id/attachments",
        "GET /api/v1/tasks/:task_id/attachments",
        MigrationState::Adopted,
        API_LIST_ATTACHMENTS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(ATTACHMENT_LIST_BINDING)),
    OperationDeclaration::new(
        "api.create-attachment",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/attachments"),
        "POST /api/v1/tasks/:task_id/attachments",
        "POST /api/v1/tasks/:task_id/attachments",
        MigrationState::Adopted,
        API_CREATE_ATTACHMENT_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(ATTACHMENT_CREATE_BINDING)),
    OperationDeclaration::new(
        "api.download-attachment",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/attachments/:attachment_id"),
        "GET /api/v1/tasks/:task_id/attachments/:attachment_id",
        "GET /api/v1/tasks/:task_id/attachments/:attachment_id",
        MigrationState::Adopted,
        API_DOWNLOAD_ATTACHMENT_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(ATTACHMENT_DOWNLOAD_BINDING)),
    OperationDeclaration::new(
        "api.delete-attachment",
        ContractSurface::Api,
        Some(HttpMethod::Delete),
        Some("/api/v1/tasks/:task_id/attachments/:attachment_id"),
        "DELETE /api/v1/tasks/:task_id/attachments/:attachment_id",
        "DELETE /api/v1/tasks/:task_id/attachments/:attachment_id",
        MigrationState::Adopted,
        API_DELETE_ATTACHMENT_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActor)
    .with_mcp_policy(policy!(ATTACHMENT_REMOVE_BINDING)),
    OperationDeclaration::new(
        "api.list-events",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/events"),
        "GET /api/v1/events",
        "GET /api/v1/events",
        MigrationState::Adopted,
        API_LIST_EVENTS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(EVENT_LIST_BINDING)),
    OperationDeclaration::new(
        "sse.stream-events",
        ContractSurface::Sse,
        Some(HttpMethod::Get),
        Some("/api/v1/stream/events"),
        "GET /api/v1/stream/events",
        "GET /api/v1/stream/events",
        MigrationState::Adopted,
        SSE_STREAM_EVENTS_CONTRACTS,
    )
    .with_obligation_overrides(&[(
        EndpointObligationKind::Headers,
        EndpointObligation::Excluded {
            reason:
                "V1 finite snapshot intentionally ignores Last-Event-ID; after query owns the cursor",
        },
    )]),
];

pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    HISTORY_OPERATIONS
}

pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(HISTORY_OPERATIONS).contracts()
}

pub fn endpoint_descriptor(id: &str) -> Option<EndpointDescriptor> {
    crate::CatalogProjection::new(HISTORY_OPERATIONS)
        .endpoints()
        .into_iter()
        .find(|endpoint| endpoint.operation_id == id)
}

pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    crate::CatalogProjection::new(HISTORY_OPERATIONS).endpoints()
}

pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(HISTORY_OPERATIONS).surfaces()
}

pub fn header_profile(id: &str) -> Option<ApiHeaderProfile> {
    HISTORY_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == id)
        .and_then(|operation| operation.header_profile)
}

pub fn header_contract(id: &str) -> Option<OperationContract> {
    let parent = HISTORY_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == id)?;
    parent
        .contracts
        .iter()
        .find(|contract| contract.location == Some(HttpTransportLocation::Headers))
        .map(|contract| contract.operation_contract(parent))
}

#[cfg(feature = "schema")]
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    crate::CatalogProjection::new(HISTORY_OPERATIONS).schemas()
}

pub fn owns_contract(id: &str) -> bool {
    HISTORY_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

pub fn owns_operation(id: &str) -> bool {
    HISTORY_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}
