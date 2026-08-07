//! Boards operation family 的第一条 declaration source。
//!
//! 该文件只持有 board list/create/show/archive/columns 的 parent/child declaration；旧
//! registry 在 hybrid projection 中继续提供其余 operation。schema type、header profile、
//! fixture 和 wire shape 都显式写在 child/parent 上，不从 operation id 猜测。

use crate::{
    ApiHeaderProfile, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, EndpointDescriptor, HttpMethod, HttpTransportLocation,
    McpExposure, McpPolicy, McpToolBinding, OperationContract, OperationDeclaration,
    SurfaceOperation, WireParameter,
};

const LIST_BOARDS_QUERY_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "include_archived",
    cardinality: Some(crate::WireParameterCardinality::OptionalOne),
}];

const BOARD_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(crate::WireParameterCardinality::RequiredOne),
}];

const BOARD_COLUMNS_PATH_PARAMETERS: &[WireParameter] = BOARD_PATH_PARAMETERS;

const DOMAIN_INVARIANTS: &[crate::McpOperationInvariant] = &[
    crate::McpOperationInvariant::CanonicalHostOnly,
    crate::McpOperationInvariant::SharedApplicationService,
    crate::McpOperationInvariant::NoHostAdminSurface,
];

const BOARD_ARCHIVE_BINDING: &[McpToolBinding] = &[McpToolBinding {
    tool_name: "board_archive",
    http_operations: &["api.archive-board"],
}];
const BOARD_COLUMNS_BINDING: &[McpToolBinding] = &[McpToolBinding {
    tool_name: "board_columns",
    http_operations: &["api.list-board-columns"],
}];
const BOARD_CREATE_BINDING: &[McpToolBinding] = &[McpToolBinding {
    tool_name: "board_create",
    http_operations: &["api.create-board"],
}];
const BOARD_LIST_BINDING: &[McpToolBinding] = &[McpToolBinding {
    tool_name: "board_list",
    http_operations: &["api.list-boards"],
}];
const BOARD_SHOW_BINDING: &[McpToolBinding] = &[McpToolBinding {
    tool_name: "board_show",
    http_operations: &["api.get-board"],
}];

const BOARD_LIST_POLICY: McpPolicy = McpPolicy {
    exposure: McpExposure::Domain,
    tool_bindings: BOARD_LIST_BINDING,
    invariants: DOMAIN_INVARIANTS,
};
const BOARD_CREATE_POLICY: McpPolicy = McpPolicy {
    exposure: McpExposure::Domain,
    tool_bindings: BOARD_CREATE_BINDING,
    invariants: DOMAIN_INVARIANTS,
};
const BOARD_SHOW_POLICY: McpPolicy = McpPolicy {
    exposure: McpExposure::Domain,
    tool_bindings: BOARD_SHOW_BINDING,
    invariants: DOMAIN_INVARIANTS,
};
const BOARD_ARCHIVE_POLICY: McpPolicy = McpPolicy {
    exposure: McpExposure::Domain,
    tool_bindings: BOARD_ARCHIVE_BINDING,
    invariants: DOMAIN_INVARIANTS,
};
const BOARD_COLUMNS_POLICY: McpPolicy = McpPolicy {
    exposure: McpExposure::Domain,
    tool_bindings: BOARD_COLUMNS_BINDING,
    invariants: DOMAIN_INVARIANTS,
};

macro_rules! api_contract {
    (
        $id:literal,
        $path:literal,
        $direction:expr,
        $location:expr,
        $parameters:expr,
        $schema_id:literal,
        $artifact_path:literal,
        $title:literal,
        $valid_fixture:literal,
        $invalid_fixture:literal,
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
    (
        $operation:literal,
        $path:literal,
        $profile:expr,
        $schema_type:ty,
        $profile_slug:literal $(,)?
    ) => {{
        let contract = ContractDeclaration::new(
            concat!("api.", $operation, ".headers"),
            $path,
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

macro_rules! cli_contract {
    (
        $slug:literal,
        $command:literal,
        $schema_type:ty $(,)?
    ) => {{
        let contract = ContractDeclaration::new(
            concat!("cli.", $slug, ".output"),
            concat!("kanban ", $command, " --json stdout"),
            ContractDirection::Serialize,
            None,
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_schema(
            concat!("urn:kanban-tool:schema:cli:", $slug, "-output:v1"),
            concat!("cli/", $slug, "-output.v1.schema.json"),
            concat!("Kanban CLI ", $command, " output v1"),
            concat!("schemas/fixtures/cli/", $slug, "-output.v1.valid.json"),
            concat!("schemas/fixtures/cli/", $slug, "-output.v1.invalid.json"),
        );
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

const API_LIST_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.list-boards.query",
        "GET /api/v1/boards query",
        ContractDirection::Deserialize,
        HttpTransportLocation::Query,
        LIST_BOARDS_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:list-boards-query:v1",
        "api/list-boards-query.v1.schema.json",
        "Kanban list boards query v1",
        "schemas/fixtures/api/list-boards-query.v1.valid.json",
        "schemas/fixtures/api/list-boards-query.v1.invalid.json",
        crate::ListBoardsQuery,
    ),
    header_contract!(
        "list-boards",
        "GET /api/v1/boards headers",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.list-boards.response",
        "GET /api/v1/boards response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-boards-response:v1",
        "api/list-boards-response.v1.schema.json",
        "Kanban list boards response v1",
        "schemas/fixtures/api/list-boards-response.v1.valid.json",
        "schemas/fixtures/api/list-boards-response.v1.invalid.json",
        crate::ListBoardsResponse,
    ),
];

const API_CREATE_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "create-board",
        "POST /api/v1/boards headers",
        ApiHeaderProfile::LocaleActorJson,
        crate::headers::LocaleActorJsonHeaders,
        "locale-actor-json-headers",
    ),
    api_contract!(
        "api.create-board.request",
        "POST /api/v1/boards request",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:create-board-request:v1",
        "api/create-board-request.v1.schema.json",
        "Kanban create board request v1",
        "schemas/fixtures/api/create-board-request.v1.valid.json",
        "schemas/fixtures/api/create-board-request.v1.invalid.json",
        crate::CreateBoardRequest,
    ),
    api_contract!(
        "api.create-board.response",
        "POST /api/v1/boards response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:create-board-response:v1",
        "api/create-board-response.v1.schema.json",
        "Kanban create board response v1",
        "schemas/fixtures/api/create-board-response.v1.valid.json",
        "schemas/fixtures/api/create-board-response.v1.invalid.json",
        crate::CreateBoardResponse,
    ),
];

const API_SHOW_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.get-board.path",
        "GET /api/v1/boards/:board path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:get-board-path:v1",
        "api/get-board-path.v1.schema.json",
        "Kanban get board path v1",
        "schemas/fixtures/api/get-board-path.v1.valid.json",
        "schemas/fixtures/api/get-board-path.v1.invalid.json",
        crate::GetBoardPath,
    ),
    header_contract!(
        "get-board",
        "GET /api/v1/boards/:board headers",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.get-board.response",
        "GET /api/v1/boards/:board response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:get-board-response:v1",
        "api/get-board-response.v1.schema.json",
        "Kanban get board response v1",
        "schemas/fixtures/api/get-board-response.v1.valid.json",
        "schemas/fixtures/api/get-board-response.v1.invalid.json",
        crate::GetBoardResponse,
    ),
];

const API_ARCHIVE_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.archive-board.path",
        "POST /api/v1/boards/:board/archive path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:archive-board-path:v1",
        "api/archive-board-path.v1.schema.json",
        "Kanban archive board path v1",
        "schemas/fixtures/api/archive-board-path.v1.valid.json",
        "schemas/fixtures/api/archive-board-path.v1.invalid.json",
        crate::ArchiveBoardPath,
    ),
    header_contract!(
        "archive-board",
        "POST /api/v1/boards/:board/archive headers",
        ApiHeaderProfile::LocaleActorOptionalJson,
        crate::headers::LocaleActorOptionalJsonHeaders,
        "locale-actor-optional-json-headers",
    ),
    api_contract!(
        "api.archive-board.request",
        "POST /api/v1/boards/:board/archive",
        ContractDirection::Deserialize,
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:archive-board-request:v1",
        "api/archive-board-request.v1.schema.json",
        "Kanban archive board request v1",
        "schemas/fixtures/api/archive-board-request.v1.valid.json",
        "schemas/fixtures/api/archive-board-request.v1.invalid.json",
        crate::ArchiveBoardRequest,
    ),
    api_contract!(
        "api.archive-board.response",
        "POST /api/v1/boards/:board/archive response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:archive-board-response:v1",
        "api/archive-board-response.v1.schema.json",
        "Kanban archive board response v1",
        "schemas/fixtures/api/archive-board-response.v1.valid.json",
        "schemas/fixtures/api/archive-board-response.v1.invalid.json",
        crate::ArchiveBoardResponse,
    ),
];

const API_COLUMNS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.list-board-columns.path",
        "GET /api/v1/boards/:board/columns path",
        ContractDirection::Deserialize,
        HttpTransportLocation::Path,
        BOARD_COLUMNS_PATH_PARAMETERS,
        "urn:kanban-tool:schema:api:list-board-columns-path:v1",
        "api/list-board-columns-path.v1.schema.json",
        "Kanban API list board columns path v1",
        "schemas/fixtures/api/list-board-columns-path.v1.valid.json",
        "schemas/fixtures/api/list-board-columns-path.v1.invalid.json",
        crate::ListBoardColumnsPath,
    ),
    header_contract!(
        "list-board-columns",
        "GET /api/v1/boards/:board/columns headers",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.list-board-columns.response",
        "GET /api/v1/boards/:board/columns response",
        ContractDirection::Serialize,
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:list-board-columns-response:v1",
        "api/list-board-columns-response.v1.schema.json",
        "Kanban API list board columns response v1",
        "schemas/fixtures/api/list-board-columns-response.v1.valid.json",
        "schemas/fixtures/api/list-board-columns-response.v1.invalid.json",
        crate::ListBoardColumnsResponse,
    ),
];

const CLI_BOARD_LIST_CONTRACTS: &[ContractDeclaration] = &[cli_contract!(
    "board-list",
    "board list",
    crate::ListBoardsResponse,
)];

const CLI_BOARD_CREATE_CONTRACTS: &[ContractDeclaration] = &[cli_contract!(
    "board-create",
    "board create",
    crate::CreateBoardResponse,
)];

const CLI_BOARD_SHOW_CONTRACTS: &[ContractDeclaration] = &[cli_contract!(
    "board-show",
    "board show",
    crate::GetBoardResponse,
)];

const CLI_BOARD_ARCHIVE_CONTRACTS: &[ContractDeclaration] = &[cli_contract!(
    "board-archive",
    "board archive",
    crate::ArchiveBoardResponse,
)];

const CLI_BOARD_COLUMNS_CONTRACTS: &[ContractDeclaration] = &[cli_contract!(
    "board-columns",
    "board columns",
    crate::CliBoardColumnsOutput,
)];

const BOARD_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.list-boards",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards"),
        "GET /api/v1/boards",
        "GET /api/v1/boards",
        API_LIST_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(BOARD_LIST_POLICY),
    OperationDeclaration::new(
        "api.create-board",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards"),
        "POST /api/v1/boards",
        "POST /api/v1/boards",
        API_CREATE_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(BOARD_CREATE_POLICY),
    OperationDeclaration::new(
        "api.get-board",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board"),
        "GET /api/v1/boards/:board",
        "GET /api/v1/boards/:board",
        API_SHOW_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(BOARD_SHOW_POLICY),
    OperationDeclaration::new(
        "api.archive-board",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/archive"),
        "POST /api/v1/boards/:board/archive",
        "POST /api/v1/boards/:board/archive",
        API_ARCHIVE_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorOptionalJson)
    .with_mcp_policy(BOARD_ARCHIVE_POLICY),
    OperationDeclaration::new(
        "api.list-board-columns",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/columns"),
        "GET /api/v1/boards/:board/columns",
        "GET /api/v1/boards/:board/columns",
        API_COLUMNS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(BOARD_COLUMNS_POLICY),
    OperationDeclaration::new(
        "cli.board-list",
        ContractSurface::Cli,
        None,
        None,
        "board list",
        "board list",
        CLI_BOARD_LIST_CONTRACTS,
    ),
    OperationDeclaration::new(
        "cli.board-create",
        ContractSurface::Cli,
        None,
        None,
        "board create",
        "board create",
        CLI_BOARD_CREATE_CONTRACTS,
    ),
    OperationDeclaration::new(
        "cli.board-show",
        ContractSurface::Cli,
        None,
        None,
        "board show",
        "board show",
        CLI_BOARD_SHOW_CONTRACTS,
    ),
    OperationDeclaration::new(
        "cli.board-archive",
        ContractSurface::Cli,
        None,
        None,
        "board archive",
        "board archive",
        CLI_BOARD_ARCHIVE_CONTRACTS,
    ),
    OperationDeclaration::new(
        "cli.board-columns",
        ContractSurface::Cli,
        None,
        None,
        "board columns",
        "board columns",
        CLI_BOARD_COLUMNS_CONTRACTS,
    ),
];

/// 保留既有生成 artifact 所需的 board contract key 顺序；不承载 contract 事实。
pub const HISTORICAL_CONTRACT_ORDER: &[&str] = &[
    "cli.board-list.output",
    "cli.board-create.output",
    "cli.board-show.output",
    "cli.board-use.output",
    "cli.board-current.output",
    "cli.board-archive.output",
    "cli.board-columns.output",
    "api.list-board-columns.path",
    "api.list-board-columns.response",
    "api.doctor.response",
    "api.error.response",
    "api.list-boards.query",
    "api.create-board.request",
    "api.get-board.path",
    "api.archive-board.path",
    "api.list-boards.response",
    "api.create-board.response",
    "api.get-board.response",
    "api.archive-board.response",
    "api.archive-task.request",
    "api.archive-board.request",
    "api.add-dependency.request",
];

/// Schema artifact 的历史 key 顺序；仅保存顺序，不重复 schema 或 contract 事实。
pub const HISTORICAL_SCHEMA_ORDER: &[&str] = &[
    "cli.board-list.output",
    "cli.board-create.output",
    "cli.board-show.output",
    "cli.board-use.output",
    "cli.board-current.output",
    "cli.board-archive.output",
    "cli.board-columns.output",
    "cli.task-list.output",
    "api.health.response",
    "api.list-boards.query",
    "api.create-board.request",
    "api.get-board.path",
    "api.archive-board.path",
    "api.list-boards.response",
    "api.create-board.response",
    "api.get-board.response",
    "api.archive-board.response",
    "api.archive-task.request",
    "api.archive-board.request",
    "api.add-dependency.request",
    "api.get-run-log.response",
    "api.list-board-columns.path",
    "api.list-board-columns.response",
    "api.list-attachments.path",
];

/// Boards family 的 declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    BOARD_OPERATIONS
}

/// 返回该 family 的全部 projection contracts，保留 parent/child source 顺序。
pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(BOARD_OPERATIONS).contracts()
}

/// 查找该 family 的单个 projected contract。
pub fn operation_contract(id: &str) -> Option<OperationContract> {
    operation_contracts()
        .into_iter()
        .find(|contract| contract.id == id)
}

/// 查找 API parent endpoint projection。
pub fn endpoint_descriptor(operation_id: &str) -> Option<EndpointDescriptor> {
    crate::CatalogProjection::new(BOARD_OPERATIONS)
        .endpoints()
        .into_iter()
        .find(|endpoint| endpoint.operation_id == operation_id)
}

/// 查找 parent 上显式声明的 header profile。
pub fn header_profile(operation_id: &str) -> Option<ApiHeaderProfile> {
    BOARD_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == operation_id)
        .and_then(|operation| operation.header_profile)
}

/// 返回 source 中的 API header child id，供 hybrid header/schema projection 使用。
pub fn header_contract(operation_id: &str) -> Option<OperationContract> {
    let parent = BOARD_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == operation_id)?;
    parent
        .contracts
        .iter()
        .find(|contract| contract.location == Some(HttpTransportLocation::Headers))
        .map(|contract| contract.operation_contract(parent))
}

/// 返回 source 中 API/SSE endpoint projection。
pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    crate::CatalogProjection::new(BOARD_OPERATIONS).endpoints()
}

/// 返回 source 中 non-HTTP surface projection。
pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(BOARD_OPERATIONS).surfaces()
}

/// 返回一个按 canonical surface key 查找的 board projection。
pub fn surface_operation(key: &str) -> SurfaceOperation {
    surface_catalog()
        .into_iter()
        .find(|operation| operation.key == key)
        .unwrap_or_else(|| panic!("missing board surface operation: {key}"))
}

/// 返回 source 中显式 schema roots。
#[cfg(feature = "schema")]
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    crate::CatalogProjection::new(BOARD_OPERATIONS).schemas()
}

/// 判断 contract 是否属于该迁移 family。
pub fn owns_contract(id: &str) -> bool {
    operation_contracts()
        .iter()
        .any(|contract| contract.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_source_projects_unique_family_rows() {
        let endpoints = endpoint_catalog();
        assert_eq!(
            endpoints
                .iter()
                .map(|endpoint| endpoint.operation_id)
                .collect::<Vec<_>>(),
            vec![
                "api.list-boards",
                "api.create-board",
                "api.get-board",
                "api.archive-board",
                "api.list-board-columns",
            ]
        );
        let contracts = operation_contracts();
        let mut ids = contracts
            .iter()
            .map(|contract| contract.id)
            .collect::<Vec<_>>();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert_eq!(contracts.len(), 21);
        assert!(
            contracts
                .iter()
                .all(|contract| { contract.schema_id.is_some() })
        );
    }

    #[test]
    fn board_parent_policy_keeps_explicit_mcp_bindings() {
        let mut tools = Vec::new();
        for operation in BOARD_OPERATIONS.iter().take(5) {
            let policy = operation.mcp_policy.expect("board API MCP policy");
            assert_eq!(policy.exposure, McpExposure::Domain);
            assert_eq!(policy.invariants, DOMAIN_INVARIANTS);
            assert_eq!(policy.tool_bindings.len(), 1);
            assert_eq!(
                policy.tool_bindings[0].http_operations,
                &[operation.operation_id]
            );
            tools.push(policy.tool_bindings[0].tool_name);
        }
        tools.sort_unstable();
        tools.dedup();
        assert_eq!(tools.len(), 5);
        assert_eq!(DOMAIN_INVARIANTS.len(), 3);
    }

    #[test]
    fn board_family_hybrid_projections_are_unique_and_compatible() {
        let source_contracts = operation_contracts();
        let inventory = crate::operation_inventory();
        for source in &source_contracts {
            let matches = inventory
                .iter()
                .filter(|contract| contract.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "board contract must be projected once: {}",
                source.id
            );
            assert_eq!(matches[0], source, "hybrid inventory changed {}", source.id);
        }

        let endpoints = crate::endpoint_catalog();
        for source in endpoint_catalog() {
            let matches = endpoints
                .iter()
                .filter(|endpoint| endpoint.operation_id == source.operation_id)
                .collect::<Vec<_>>();
            assert_eq!(matches.len(), 1, "board endpoint must be projected once");
            assert_eq!(*matches[0], source);
        }

        let surfaces = crate::surface_operation_catalog();
        for source in crate::CatalogProjection::new(BOARD_OPERATIONS).surfaces() {
            let matches = surfaces
                .iter()
                .filter(|surface| surface.key == source.key)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "board surface must be projected once: {}",
                source.key
            );
            assert_eq!(matches[0], &source, "hybrid surface changed {}", source.key);
        }
    }

    #[test]
    fn board_family_preserves_historical_inventory_order() {
        let ids = crate::operation_inventory()
            .iter()
            .map(|contract| contract.id)
            .collect::<Vec<_>>();
        assert_in_order(
            &ids,
            &[
                "cli.board-list.output",
                "cli.board-create.output",
                "cli.board-show.output",
                "cli.board-use.output",
                "cli.board-current.output",
                "cli.board-archive.output",
                "cli.board-columns.output",
                "cli.task-list.output",
            ],
        );
        assert_in_order(
            &ids,
            &[
                "api.list-board-columns.path",
                "api.list-board-columns.response",
                "api.doctor.response",
                "api.error.response",
                "api.list-boards.query",
                "api.create-board.request",
                "api.get-board.path",
                "api.archive-board.path",
                "api.list-boards.response",
                "api.create-board.response",
                "api.get-board.response",
                "api.archive-board.response",
                "api.archive-task.request",
                "api.archive-board.request",
                "api.add-dependency.request",
            ],
        );
    }

    fn assert_in_order(ids: &[&str], expected: &[&str]) {
        let mut previous = None;
        for id in expected {
            let index = ids
                .iter()
                .position(|candidate| candidate == id)
                .unwrap_or_else(|| panic!("missing historical board row: {id}"));
            if let Some(previous) = previous {
                assert!(
                    previous < index,
                    "historical board row order changed around {id}"
                );
            }
            previous = Some(index);
        }
    }

    #[cfg(feature = "schema")]
    #[test]
    fn board_schema_projection_matches_committed_artifacts() {
        let generated = crate::schema::generated_artifacts();
        let registry = crate::schema::schema_registry();
        let ids = registry
            .iter()
            .map(|root| root.contract_id)
            .collect::<Vec<_>>();
        assert_in_order(
            &ids,
            &[
                "cli.board-list.output",
                "cli.board-create.output",
                "cli.board-show.output",
                "cli.board-use.output",
                "cli.board-current.output",
                "cli.board-archive.output",
                "cli.board-columns.output",
                "cli.task-list.output",
                "api.health.response",
                "api.list-boards.query",
                "api.create-board.request",
                "api.get-board.path",
                "api.archive-board.path",
                "api.list-boards.response",
                "api.create-board.response",
                "api.get-board.response",
                "api.archive-board.response",
                "api.archive-task.request",
                "api.archive-board.request",
                "api.add-dependency.request",
                "api.get-run-log.response",
                "api.list-board-columns.path",
                "api.list-board-columns.response",
                "api.list-attachments.path",
            ],
        );
        for source in schema_roots() {
            let matches = registry
                .iter()
                .filter(|root| root.id == source.id)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "board schema root must be projected once: {}",
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

            let actual = generated.get(root.artifact_path).unwrap_or_else(|| {
                panic!("missing generated board artifact {}", root.artifact_path)
            });
            let committed_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/json-schema/draft-2020-12")
                .join(root.artifact_path);
            let committed = std::fs::read(&committed_path)
                .unwrap_or_else(|error| panic!("read {}: {error}", committed_path.display()));
            assert_eq!(
                actual, &committed,
                "board artifact bytes changed: {}",
                root.artifact_path
            );
        }
    }
}
