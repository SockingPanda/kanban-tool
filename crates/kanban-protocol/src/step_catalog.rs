//! Step 与 execution-plan API family 的唯一 declaration source。

use crate::{
    ApiHeaderProfile, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, EndpointDescriptor, HttpMethod, HttpTransportLocation,
    McpExposure, McpPolicy, McpToolBinding, OperationContract, OperationDeclaration,
    SurfaceOperation, WireParameter,
};

const STEP_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];
const STEP_ITEM_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "task_id",
        cardinality: Some(crate::WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "step_id",
        cardinality: Some(crate::WireParameterCardinality::RequiredOne),
    },
];

const DOMAIN_INVARIANTS: &[crate::McpOperationInvariant] = &[
    crate::McpOperationInvariant::CanonicalHostOnly,
    crate::McpOperationInvariant::SharedApplicationService,
    crate::McpOperationInvariant::NoHostAdminSurface,
];

macro_rules! api_contract {
    (
        $id:expr, $path:expr, $direction:expr, $location:expr, $parameters:expr,
        $schema_id:expr, $artifact_path:expr, $title:expr, $valid_fixture:expr,
        $invalid_fixture:expr, $schema_type:ty $(,)?
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
        let contract = match $profile {
            ApiHeaderProfile::Locale => {
                contract.with_schema_type::<crate::headers::LocaleHeaders>()
            }
            ApiHeaderProfile::LocaleActor => {
                contract.with_schema_type::<crate::headers::LocaleActorHeaders>()
            }
            ApiHeaderProfile::LocaleJson => {
                contract.with_schema_type::<crate::headers::LocaleJsonHeaders>()
            }
            ApiHeaderProfile::LocaleActorJson => {
                contract.with_schema_type::<crate::headers::LocaleActorJsonHeaders>()
            }
            ApiHeaderProfile::LocaleActorOptionalJson => unreachable!(),
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
    STEP_CREATE_BINDING,
    "step_create",
    ["api.list-tasks", "api.create-step"]
);
binding!(
    STEP_DONE_BINDING,
    "step_done",
    ["api.list-tasks", "api.list-steps", "api.complete-step"]
);
binding!(
    STEP_LIST_BINDING,
    "step_list",
    ["api.list-tasks", "api.list-steps"]
);
binding!(
    STEP_REMOVE_BINDING,
    "step_remove",
    ["api.list-tasks", "api.list-steps", "api.remove-step"]
);
binding!(
    STEP_REOPEN_BINDING,
    "step_reopen",
    ["api.list-tasks", "api.list-steps", "api.reopen-step"]
);
binding!(
    STEP_SKIP_BINDING,
    "step_skip",
    ["api.list-tasks", "api.list-steps", "api.skip-step"]
);
binding!(
    STEP_UPDATE_BINDING,
    "step_update",
    ["api.list-tasks", "api.list-steps", "api.update-step"]
);
binding!(
    STEP_PLAN_BINDING,
    "task_plan_not_required",
    ["api.list-tasks", "api.mark-execution-plan-not-required"]
);

const API_LIST_STEPS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.list-steps.path",
        "GET /api/v1/tasks/:task_id/steps path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        STEP_TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:list-steps-path:v1",
        "api/list-steps-path.v1.schema.json",
        "Kanban list steps path v1",
        "schemas/fixtures/api/list-steps-path.v1.valid.json",
        "schemas/fixtures/api/list-steps-path.v1.invalid.json",
        crate::ListStepsPath
    ),
    header_contract!(
        "list-steps",
        "GET /api/v1/tasks/:task_id/steps",
        ApiHeaderProfile::Locale,
        "locale-headers",
        crate::headers::LocaleHeaders
    ),
    api_contract!(
        "api.list-steps.response",
        "GET /api/v1/tasks/:task_id/steps response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-steps-response:v1",
        "api/list-steps-response.v1.schema.json",
        "Kanban list steps response v1",
        "schemas/fixtures/api/list-steps-response.v1.valid.json",
        "schemas/fixtures/api/list-steps-response.v1.invalid.json",
        crate::ListStepsResponse
    ),
];

const API_CREATE_STEP_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.create-step.path",
        "POST /api/v1/tasks/:task_id/steps path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        STEP_TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:create-step-path:v1",
        "api/create-step-path.v1.schema.json",
        "Kanban create step path v1",
        "schemas/fixtures/api/create-step-path.v1.valid.json",
        "schemas/fixtures/api/create-step-path.v1.invalid.json",
        crate::CreateStepPath
    ),
    header_contract!(
        "create-step",
        "POST /api/v1/tasks/:task_id/steps",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers",
        crate::headers::LocaleActorJsonHeaders
    ),
    api_contract!(
        "api.create-step.request",
        "POST /api/v1/tasks/:task_id/steps request",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:create-step-request:v1",
        "api/create-step-request.v1.schema.json",
        "Kanban create step request v1",
        "schemas/fixtures/api/create-step-request.v1.valid.json",
        "schemas/fixtures/api/create-step-request.v1.invalid.json",
        crate::CreateStepRequest
    ),
    api_contract!(
        "api.create-step.response",
        "POST /api/v1/tasks/:task_id/steps response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:create-step-response:v1",
        "api/create-step-response.v1.schema.json",
        "Kanban create step response v1",
        "schemas/fixtures/api/create-step-response.v1.valid.json",
        "schemas/fixtures/api/create-step-response.v1.invalid.json",
        crate::CreateStepResponse
    ),
];

const API_UPDATE_STEP_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.update-step.path",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        STEP_ITEM_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:update-step-path:v1",
        "api/update-step-path.v1.schema.json",
        "Kanban update step path v1",
        "schemas/fixtures/api/update-step-path.v1.valid.json",
        "schemas/fixtures/api/update-step-path.v1.invalid.json",
        crate::UpdateStepPath
    ),
    header_contract!(
        "update-step",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers",
        crate::headers::LocaleActorJsonHeaders
    ),
    api_contract!(
        "api.update-step.request",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id request",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:update-step-request:v1",
        "api/update-step-request.v1.schema.json",
        "Kanban update step request v1",
        "schemas/fixtures/api/update-step-request.v1.valid.json",
        "schemas/fixtures/api/update-step-request.v1.invalid.json",
        crate::UpdateStepRequest
    ),
    api_contract!(
        "api.update-step.response",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:update-step-response:v1",
        "api/update-step-response.v1.schema.json",
        "Kanban update step response v1",
        "schemas/fixtures/api/update-step-response.v1.valid.json",
        "schemas/fixtures/api/update-step-response.v1.invalid.json",
        crate::UpdateStepResponse
    ),
];

const API_REMOVE_STEP_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.remove-step.path",
        "DELETE /api/v1/tasks/:task_id/steps/:step_id path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        STEP_ITEM_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:remove-step-path:v1",
        "api/remove-step-path.v1.schema.json",
        "Kanban remove step path v1",
        "schemas/fixtures/api/remove-step-path.v1.valid.json",
        "schemas/fixtures/api/remove-step-path.v1.invalid.json",
        crate::RemoveStepPath
    ),
    header_contract!(
        "remove-step",
        "DELETE /api/v1/tasks/:task_id/steps/:step_id",
        ApiHeaderProfile::LocaleActor,
        "locale-actor-headers",
        crate::headers::LocaleActorHeaders
    ),
    api_contract!(
        "api.remove-step.response",
        "DELETE /api/v1/tasks/:task_id/steps/:step_id response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:remove-step-response:v1",
        "api/remove-step-response.v1.schema.json",
        "Kanban remove step response v1",
        "schemas/fixtures/api/remove-step-response.v1.valid.json",
        "schemas/fixtures/api/remove-step-response.v1.invalid.json",
        crate::RemoveStepResponse
    ),
];

macro_rules! step_transition_contracts {
    ($slug:literal, $title:literal, $path:literal, $path_ty:ty, $request_ty:ty, $response_ty:ty) => {
        &[
            api_contract!(
                concat!("api.", $slug, ".path"),
                concat!($path, " path"),
                ContractDirection::Deserialize,
                HttpTransportLocation::Path,
                STEP_ITEM_PATH_PARAMETERS,
                concat!("urn:kanban-tool:schema:api:", $slug, "-path:v1"),
                concat!("api/", $slug, "-path.v1.schema.json"),
                concat!("Kanban ", $title, " path v1"),
                concat!("schemas/fixtures/api/", $slug, "-path.v1.valid.json"),
                concat!("schemas/fixtures/api/", $slug, "-path.v1.invalid.json"),
                $path_ty
            ),
            header_contract!(
                $slug,
                $path,
                ApiHeaderProfile::LocaleActorJson,
                "locale-actor-json-headers",
                crate::headers::LocaleActorJsonHeaders
            ),
            api_contract!(
                concat!("api.", $slug, ".request"),
                concat!($path, " request"),
                ContractDirection::Deserialize,
                HttpTransportLocation::Body,
                &[],
                concat!("urn:kanban-tool:schema:api:", $slug, "-request:v1"),
                concat!("api/", $slug, "-request.v1.schema.json"),
                concat!("Kanban ", $title, " request v1"),
                concat!("schemas/fixtures/api/", $slug, "-request.v1.valid.json"),
                concat!("schemas/fixtures/api/", $slug, "-request.v1.invalid.json"),
                $request_ty
            ),
            api_contract!(
                concat!("api.", $slug, ".response"),
                concat!($path, " response"),
                ContractDirection::Serialize,
                HttpTransportLocation::Success,
                &[],
                concat!("urn:kanban-tool:schema:api:", $slug, "-response:v1"),
                concat!("api/", $slug, "-response.v1.schema.json"),
                concat!("Kanban ", $title, " response v1"),
                concat!("schemas/fixtures/api/", $slug, "-response.v1.valid.json"),
                concat!("schemas/fixtures/api/", $slug, "-response.v1.invalid.json"),
                $response_ty
            ),
        ]
    };
}

const API_COMPLETE_STEP_CONTRACTS: &[ContractDeclaration] = step_transition_contracts!(
    "complete-step",
    "complete step",
    "POST /api/v1/tasks/:task_id/steps/:step_id/done",
    crate::CompleteStepPath,
    crate::CompleteStepRequest,
    crate::CompleteStepResponse
);
const API_SKIP_STEP_CONTRACTS: &[ContractDeclaration] = step_transition_contracts!(
    "skip-step",
    "skip step",
    "POST /api/v1/tasks/:task_id/steps/:step_id/skip",
    crate::SkipStepPath,
    crate::SkipStepRequest,
    crate::SkipStepResponse
);
const API_REOPEN_STEP_CONTRACTS: &[ContractDeclaration] = step_transition_contracts!(
    "reopen-step",
    "reopen step",
    "POST /api/v1/tasks/:task_id/steps/:step_id/reopen",
    crate::ReopenStepPath,
    crate::ReopenStepRequest,
    crate::ReopenStepResponse
);

const API_PLAN_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.mark-execution-plan-not-required.path",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        STEP_TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-path:v1",
        "api/mark-execution-plan-not-required-path.v1.schema.json",
        "Kanban API mark execution plan not required path v1",
        "schemas/fixtures/api/mark-execution-plan-not-required-path.v1.valid.json",
        "schemas/fixtures/api/mark-execution-plan-not-required-path.v1.invalid.json",
        crate::MarkExecutionPlanNotRequiredPath
    ),
    header_contract!(
        "mark-execution-plan-not-required",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers",
        crate::headers::LocaleActorJsonHeaders
    ),
    api_contract!(
        "api.mark-execution-plan-not-required.request",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required body",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-request:v1",
        "api/mark-execution-plan-not-required-request.v1.schema.json",
        "Kanban API mark execution plan not required request v1",
        "schemas/fixtures/api/mark-execution-plan-not-required-request.v1.valid.json",
        "schemas/fixtures/api/mark-execution-plan-not-required-request.v1.invalid.json",
        crate::MarkExecutionPlanNotRequiredRequest
    ),
    api_contract!(
        "api.mark-execution-plan-not-required.response",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-response:v1",
        "api/mark-execution-plan-not-required-response.v1.schema.json",
        "Kanban API mark execution plan not required response v1",
        "schemas/fixtures/api/mark-execution-plan-not-required-response.v1.valid.json",
        "schemas/fixtures/api/mark-execution-plan-not-required-response.v1.invalid.json",
        crate::MarkExecutionPlanNotRequiredResponse
    ),
];

const STEP_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.list-steps",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/steps"),
        "GET /api/v1/tasks/:task_id/steps",
        "GET /api/v1/tasks/:task_id/steps",
        API_LIST_STEPS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(STEP_LIST_BINDING)),
    OperationDeclaration::new(
        "api.create-step",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/steps"),
        "POST /api/v1/tasks/:task_id/steps",
        "POST /api/v1/tasks/:task_id/steps",
        API_CREATE_STEP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(STEP_CREATE_BINDING)),
    OperationDeclaration::new(
        "api.update-step",
        ContractSurface::Api,
        Some(HttpMethod::Patch),
        Some("/api/v1/tasks/:task_id/steps/:step_id"),
        "PATCH /api/v1/tasks/:task_id/steps/:step_id",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id",
        API_UPDATE_STEP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(STEP_UPDATE_BINDING)),
    OperationDeclaration::new(
        "api.remove-step",
        ContractSurface::Api,
        Some(HttpMethod::Delete),
        Some("/api/v1/tasks/:task_id/steps/:step_id"),
        "DELETE /api/v1/tasks/:task_id/steps/:step_id",
        "DELETE /api/v1/tasks/:task_id/steps/:step_id",
        API_REMOVE_STEP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActor)
    .with_mcp_policy(policy!(STEP_REMOVE_BINDING)),
    OperationDeclaration::new(
        "api.complete-step",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/steps/:step_id/done"),
        "POST /api/v1/tasks/:task_id/steps/:step_id/done",
        "POST /api/v1/tasks/:task_id/steps/:step_id/done",
        API_COMPLETE_STEP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(STEP_DONE_BINDING)),
    OperationDeclaration::new(
        "api.skip-step",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/steps/:step_id/skip"),
        "POST /api/v1/tasks/:task_id/steps/:step_id/skip",
        "POST /api/v1/tasks/:task_id/steps/:step_id/skip",
        API_SKIP_STEP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(STEP_SKIP_BINDING)),
    OperationDeclaration::new(
        "api.reopen-step",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/steps/:step_id/reopen"),
        "POST /api/v1/tasks/:task_id/steps/:step_id/reopen",
        "POST /api/v1/tasks/:task_id/steps/:step_id/reopen",
        API_REOPEN_STEP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(STEP_REOPEN_BINDING)),
    OperationDeclaration::new(
        "api.mark-execution-plan-not-required",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/execution-plan/not-required"),
        "POST /api/v1/tasks/:task_id/execution-plan/not-required",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required",
        API_PLAN_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(STEP_PLAN_BINDING)),
];

pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    STEP_OPERATIONS
}
pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(STEP_OPERATIONS).contracts()
}
pub fn endpoint_descriptor(id: &str) -> Option<EndpointDescriptor> {
    crate::CatalogProjection::new(STEP_OPERATIONS)
        .endpoints()
        .into_iter()
        .find(|endpoint| endpoint.operation_id == id)
}
pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    crate::CatalogProjection::new(STEP_OPERATIONS).endpoints()
}
pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(STEP_OPERATIONS).surfaces()
}
pub fn header_profile(id: &str) -> Option<ApiHeaderProfile> {
    STEP_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == id)
        .and_then(|operation| operation.header_profile)
}
pub fn header_contract(id: &str) -> Option<OperationContract> {
    let parent = STEP_OPERATIONS
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
    crate::CatalogProjection::new(STEP_OPERATIONS).schemas()
}
pub fn owns_contract(id: &str) -> bool {
    STEP_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}
pub fn owns_operation(id: &str) -> bool {
    STEP_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}
