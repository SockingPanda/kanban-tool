//! Dependency API family 的唯一 declaration source。

use crate::{
    AdoptionLocator, ApiHeaderProfile, ContractBinding, ContractDeclaration, ContractDirection,
    ContractGranularity, ContractStrictness, ContractSurface, EndpointDescriptor, HttpMethod,
    HttpTransportLocation, McpExposure, McpPolicy, McpToolBinding, MigrationState,
    OperationContract, OperationDeclaration, SurfaceOperation, WireParameter,
};

const DEPENDENCY_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];
const REMOVE_DEPENDENCY_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "child_task_id",
        cardinality: Some(crate::WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "parent_task_id",
        cardinality: Some(crate::WireParameterCardinality::RequiredOne),
    },
];

const DEPENDENCY_WITNESS: AdoptionLocator = AdoptionLocator {
    package: "kanban-server",
    test_target: "lib",
    exact_test:
        "http::operations::contract_adoption::suite_dependencies_adoption_uses_path_body_and_response_fixtures",
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
        $id:expr, $path:expr, $direction:expr, $location:expr, $parameters:expr,
        $schema_id:expr, $artifact_path:expr, $title:expr, $valid_fixture:expr,
        $invalid_fixture:expr, $schema_type:ty
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
        .with_adoption(DEPENDENCY_WITNESS, DEPENDENCY_WITNESS);
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
            _ => panic!("dependency headers 不支持该 profile"),
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
    DEPENDENCY_CREATE_BINDING,
    "dependency_create",
    ["api.list-tasks", "api.add-dependency"]
);
binding!(
    DEPENDENCY_LIST_BINDING,
    "dependency_list",
    ["api.list-tasks", "api.list-dependencies"]
);
binding!(
    DEPENDENCY_REMOVE_BINDING,
    "dependency_remove",
    ["api.list-tasks", "api.remove-dependency"]
);

const API_LIST_DEPENDENCIES_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.list-dependencies.path",
        "GET /api/v1/tasks/:task_id/dependencies path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        DEPENDENCY_TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:list-dependencies-path:v1",
        "api/list-dependencies-path.v1.schema.json",
        "Kanban API list dependencies path v1",
        "schemas/fixtures/api/list-dependencies-path.v1.valid.json",
        "schemas/fixtures/api/list-dependencies-path.v1.invalid.json",
        crate::ListDependenciesPath
    ),
    header_contract!(
        "list-dependencies",
        "GET /api/v1/tasks/:task_id/dependencies",
        ApiHeaderProfile::Locale,
        "locale-headers"
    ),
    api_contract!(
        "api.list-dependencies.response",
        "GET /api/v1/tasks/:task_id/dependencies response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-dependencies-response:v1",
        "api/list-dependencies-response.v1.schema.json",
        "Kanban API list dependencies response v1",
        "schemas/fixtures/api/list-dependencies-response.v1.valid.json",
        "schemas/fixtures/api/list-dependencies-response.v1.invalid.json",
        crate::ListDependenciesResponse
    ),
];

const API_ADD_DEPENDENCY_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.add-dependency.path",
        "POST /api/v1/tasks/:task_id/dependencies path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        DEPENDENCY_TASK_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:add-dependency-path:v1",
        "api/add-dependency-path.v1.schema.json",
        "Kanban API add dependency path v1",
        "schemas/fixtures/api/add-dependency-path.v1.valid.json",
        "schemas/fixtures/api/add-dependency-path.v1.invalid.json",
        crate::AddDependencyPath
    ),
    header_contract!(
        "add-dependency",
        "POST /api/v1/tasks/:task_id/dependencies",
        ApiHeaderProfile::LocaleActorJson,
        "locale-actor-json-headers"
    ),
    api_contract!(
        "api.add-dependency.request",
        "POST /api/v1/tasks/:task_id/dependencies",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:add-dependency-request:v1",
        "api/add-dependency-request.v1.schema.json",
        "Kanban add dependency request v1",
        "schemas/fixtures/api/add-dependency-request.v1.valid.json",
        "schemas/fixtures/api/add-dependency-request.v1.invalid.json",
        crate::AddDependencyRequest
    ),
    api_contract!(
        "api.add-dependency.response",
        "POST /api/v1/tasks/:task_id/dependencies response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:add-dependency-response:v1",
        "api/add-dependency-response.v1.schema.json",
        "Kanban API add dependency response v1",
        "schemas/fixtures/api/add-dependency-response.v1.valid.json",
        "schemas/fixtures/api/add-dependency-response.v1.invalid.json",
        crate::AddDependencyResponse
    ),
];

const API_REMOVE_DEPENDENCY_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.remove-dependency.path",
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        REMOVE_DEPENDENCY_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:remove-dependency-path:v1",
        "api/remove-dependency-path.v1.schema.json",
        "Kanban API remove dependency path v1",
        "schemas/fixtures/api/remove-dependency-path.v1.valid.json",
        "schemas/fixtures/api/remove-dependency-path.v1.invalid.json",
        crate::RemoveDependencyPath
    ),
    header_contract!(
        "remove-dependency",
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
        ApiHeaderProfile::LocaleActor,
        "locale-actor-headers"
    ),
    api_contract!(
        "api.remove-dependency.response",
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:remove-dependency-response:v1",
        "api/remove-dependency-response.v1.schema.json",
        "Kanban API remove dependency response v1",
        "schemas/fixtures/api/remove-dependency-response.v1.valid.json",
        "schemas/fixtures/api/remove-dependency-response.v1.invalid.json",
        crate::RemoveDependencyResponse
    ),
];

const DEPENDENCY_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.list-dependencies",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/tasks/:task_id/dependencies"),
        "GET /api/v1/tasks/:task_id/dependencies",
        "GET /api/v1/tasks/:task_id/dependencies",
        MigrationState::Adopted,
        API_LIST_DEPENDENCIES_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(policy!(DEPENDENCY_LIST_BINDING)),
    OperationDeclaration::new(
        "api.add-dependency",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/tasks/:task_id/dependencies"),
        "POST /api/v1/tasks/:task_id/dependencies",
        "POST /api/v1/tasks/:task_id/dependencies",
        MigrationState::Adopted,
        API_ADD_DEPENDENCY_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(policy!(DEPENDENCY_CREATE_BINDING)),
    OperationDeclaration::new(
        "api.remove-dependency",
        ContractSurface::Api,
        Some(HttpMethod::Delete),
        Some("/api/v1/tasks/:child_task_id/dependencies/:parent_task_id"),
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
        MigrationState::Adopted,
        API_REMOVE_DEPENDENCY_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActor)
    .with_mcp_policy(policy!(DEPENDENCY_REMOVE_BINDING)),
];

pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    DEPENDENCY_OPERATIONS
}

pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(DEPENDENCY_OPERATIONS).contracts()
}

pub fn endpoint_descriptor(id: &str) -> Option<EndpointDescriptor> {
    crate::CatalogProjection::new(DEPENDENCY_OPERATIONS)
        .endpoints()
        .into_iter()
        .find(|endpoint| endpoint.operation_id == id)
}

pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    crate::CatalogProjection::new(DEPENDENCY_OPERATIONS).endpoints()
}

pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(DEPENDENCY_OPERATIONS).surfaces()
}

pub fn header_profile(id: &str) -> Option<ApiHeaderProfile> {
    DEPENDENCY_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == id)
        .and_then(|operation| operation.header_profile)
}

pub fn header_contract(id: &str) -> Option<OperationContract> {
    let parent = DEPENDENCY_OPERATIONS
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
    crate::CatalogProjection::new(DEPENDENCY_OPERATIONS).schemas()
}

pub fn owns_contract(id: &str) -> bool {
    DEPENDENCY_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

pub fn owns_operation(id: &str) -> bool {
    DEPENDENCY_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}
