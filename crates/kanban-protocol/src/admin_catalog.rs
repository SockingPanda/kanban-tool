//! Stats、health 与 host-admin maintenance API 的唯一 declaration source。
//!
//! 这里同时冻结 portable export/import/replace 与 legacy SQLite v30 import 所使用的
//! HTTP request/response contract。真实 host、client 和 CLI adapter 仍由各自 crate 持有；
//! 本模块只描述 wire operation、header profile、schema、fixture、host-admin 的 MCP 边界。

use crate::{
    ApiHeaderProfile, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, ContractTransport, EndpointDescriptor, HttpMethod,
    HttpTransportLocation, McpExposure, McpPolicy, OperationContract, OperationDeclaration,
    SurfaceOperation, WireParameter,
};

const BOARD_QUERY_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(crate::WireParameterCardinality::OptionalOne),
}];

const fn contract_direction(location: HttpTransportLocation) -> ContractDirection {
    match location {
        HttpTransportLocation::Path
        | HttpTransportLocation::Query
        | HttpTransportLocation::Headers
        | HttpTransportLocation::Body => ContractDirection::Deserialize,
        HttpTransportLocation::Success
        | HttpTransportLocation::Error
        | HttpTransportLocation::Sse => ContractDirection::Serialize,
    }
}

const HOST_ADMIN_POLICY: McpPolicy = McpPolicy {
    exposure: McpExposure::HostAdmin,
    tool_bindings: &[],
    invariants: &[],
};

const DOMAIN_INVARIANTS: &[crate::McpOperationInvariant] = &[
    crate::McpOperationInvariant::CanonicalHostOnly,
    crate::McpOperationInvariant::SharedApplicationService,
    crate::McpOperationInvariant::NoHostAdminSurface,
];
const STATS_BINDING: &[crate::McpToolBinding] = &[crate::McpToolBinding {
    tool_name: "stats",
    http_operations: &["api.get-stats"],
}];
const STATS_POLICY: McpPolicy = McpPolicy {
    exposure: McpExposure::Domain,
    tool_bindings: STATS_BINDING,
    invariants: DOMAIN_INVARIANTS,
};

macro_rules! api_contract {
    (
        $id:literal,
        $path:literal,
        $operation:literal,
        $operation_key:literal,
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
            contract_direction($location),
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
        $id:literal,
        $slug:literal,
        $path:literal,
        $operation:literal,
        $operation_key:literal,
        $profile:expr,
        $schema_type:ty,
        $profile_slug:literal $(,)?
    ) => {{
        let contract = ContractDeclaration::new(
            $id,
            $path,
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_operation($operation)
        .with_transport(Some($operation_key), $profile.parameters())
        .with_schema(
            concat!("urn:kanban-tool:schema:api:", $slug, "-headers:v1"),
            concat!("api/", $slug, "-headers.v1.schema.json"),
            concat!("Kanban api.", $slug, " request headers v1"),
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

const API_HEALTH_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.health.headers",
        "health",
        "GET /health headers",
        "GET /health",
        "GET /health",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.health.response",
        "GET /health response",
        "localhost health report",
        "GET /health",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:health-response:v1",
        "api/health-response.v1.schema.json",
        "Kanban API health response v1",
        "schemas/fixtures/api/health-response.v1.valid.json",
        "schemas/fixtures/api/health-response.v1.invalid.json",
        crate::HealthResponse,
    ),
];

const API_STATS_CONTRACTS: &[ContractDeclaration] = &[
    api_contract!(
        "api.get-stats.query",
        "GET /api/v1/stats query",
        "GET /api/v1/stats",
        "GET /api/v1/stats",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS,
        "urn:kanban-tool:schema:api:get-stats-query:v1",
        "api/get-stats-query.v1.schema.json",
        "Kanban get stats query v1",
        "schemas/fixtures/api/get-stats-query.v1.valid.json",
        "schemas/fixtures/api/get-stats-query.v1.invalid.json",
        crate::BoardQuery,
    ),
    header_contract!(
        "api.get-stats.headers",
        "get-stats",
        "GET /api/v1/stats headers",
        "GET /api/v1/stats",
        "GET /api/v1/stats",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.get-stats.response",
        "GET /api/v1/stats response",
        "GET /api/v1/stats",
        "GET /api/v1/stats",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:get-stats-response:v1",
        "api/get-stats-response.v1.schema.json",
        "Kanban get stats response v1",
        "schemas/fixtures/api/get-stats-response.v1.valid.json",
        "schemas/fixtures/api/get-stats-response.v1.invalid.json",
        crate::StatsResponse,
    ),
];

const API_DOCTOR_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.doctor.headers",
        "doctor",
        "GET /api/v1/maintenance/doctor headers",
        "GET /api/v1/maintenance/doctor",
        "GET /api/v1/maintenance/doctor",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.doctor.response",
        "GET /api/v1/maintenance/doctor response",
        "GET /api/v1/maintenance/doctor",
        "GET /api/v1/maintenance/doctor",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:doctor-response:v1",
        "api/doctor-response.v1.schema.json",
        "Kanban doctor response v1",
        "schemas/fixtures/api/doctor-response.v1.valid.json",
        "schemas/fixtures/api/doctor-response.v1.invalid.json",
        crate::DoctorResponse,
    ),
];

const API_CHECKPOINT_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.checkpoint.headers",
        "checkpoint",
        "POST /api/v1/maintenance/checkpoint headers",
        "POST /api/v1/maintenance/checkpoint",
        "POST /api/v1/maintenance/checkpoint",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.checkpoint.response",
        "POST /api/v1/maintenance/checkpoint response",
        "POST /api/v1/maintenance/checkpoint",
        "POST /api/v1/maintenance/checkpoint",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:checkpoint-response:v1",
        "api/checkpoint-response.v1.schema.json",
        "Kanban checkpoint response v1",
        "schemas/fixtures/api/checkpoint-response.v1.valid.json",
        "schemas/fixtures/api/checkpoint-response.v1.invalid.json",
        crate::CheckpointResponse,
    ),
];

const API_BACKUP_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-backup.headers",
        "maintenance-backup",
        "POST /api/v1/maintenance/backup headers",
        "POST /api/v1/maintenance/backup",
        "POST /api/v1/maintenance/backup",
        ApiHeaderProfile::LocaleJson,
        crate::headers::LocaleJsonHeaders,
        "locale-json-headers",
    ),
    api_contract!(
        "api.maintenance-backup.request",
        "POST /api/v1/maintenance/backup body",
        "POST /api/v1/maintenance/backup",
        "POST /api/v1/maintenance/backup",
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:maintenance-backup-request:v1",
        "api/maintenance-backup-request.v1.schema.json",
        "Kanban maintenance backup request v1",
        "schemas/fixtures/api/maintenance-backup-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-backup-request.v1.invalid.json",
        crate::MaintenancePathRequest,
    ),
    api_contract!(
        "api.maintenance-backup.response",
        "POST /api/v1/maintenance/backup response",
        "POST /api/v1/maintenance/backup",
        "POST /api/v1/maintenance/backup",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-backup-response:v1",
        "api/maintenance-backup-response.v1.schema.json",
        "Kanban maintenance backup response v1",
        "schemas/fixtures/api/maintenance-backup-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-backup-response.v1.invalid.json",
        crate::BackupResponse,
    ),
];

const API_EXPORT_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-export.headers",
        "maintenance-export",
        "POST /api/v1/maintenance/export headers",
        "POST /api/v1/maintenance/export",
        "POST /api/v1/maintenance/export",
        ApiHeaderProfile::LocaleJson,
        crate::headers::LocaleJsonHeaders,
        "locale-json-headers",
    ),
    api_contract!(
        "api.maintenance-export.request",
        "POST /api/v1/maintenance/export body",
        "POST /api/v1/maintenance/export",
        "POST /api/v1/maintenance/export",
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:maintenance-export-request:v1",
        "api/maintenance-export-request.v1.schema.json",
        "Kanban maintenance export request v1",
        "schemas/fixtures/api/maintenance-export-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-export-request.v1.invalid.json",
        crate::MaintenancePathRequest,
    ),
    api_contract!(
        "api.maintenance-export.response",
        "POST /api/v1/maintenance/export response",
        "POST /api/v1/maintenance/export",
        "POST /api/v1/maintenance/export",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-export-response:v1",
        "api/maintenance-export-response.v1.schema.json",
        "Kanban maintenance export response v1",
        "schemas/fixtures/api/maintenance-export-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-export-response.v1.invalid.json",
        crate::ExportResponse,
    ),
];

const API_IMPORT_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-import.headers",
        "maintenance-import",
        "POST /api/v1/maintenance/import headers",
        "POST /api/v1/maintenance/import",
        "POST /api/v1/maintenance/import",
        ApiHeaderProfile::LocaleJson,
        crate::headers::LocaleJsonHeaders,
        "locale-json-headers",
    ),
    api_contract!(
        "api.maintenance-import.request",
        "POST /api/v1/maintenance/import body",
        "POST /api/v1/maintenance/import",
        "POST /api/v1/maintenance/import",
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:maintenance-import-request:v1",
        "api/maintenance-import-request.v1.schema.json",
        "Kanban maintenance import request v1",
        "schemas/fixtures/api/maintenance-import-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-import-request.v1.invalid.json",
        crate::MaintenanceImportRequest,
    ),
    api_contract!(
        "api.maintenance-import.response",
        "POST /api/v1/maintenance/import response",
        "POST /api/v1/maintenance/import",
        "POST /api/v1/maintenance/import",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-import-response:v1",
        "api/maintenance-import-response.v1.schema.json",
        "Kanban maintenance import response v1",
        "schemas/fixtures/api/maintenance-import-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-import-response.v1.invalid.json",
        crate::ImportResponse,
    ),
];

const API_VACUUM_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-vacuum.headers",
        "maintenance-vacuum",
        "POST /api/v1/maintenance/vacuum headers",
        "POST /api/v1/maintenance/vacuum",
        "POST /api/v1/maintenance/vacuum",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.maintenance-vacuum.response",
        "POST /api/v1/maintenance/vacuum response",
        "POST /api/v1/maintenance/vacuum",
        "POST /api/v1/maintenance/vacuum",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-vacuum-response:v1",
        "api/maintenance-vacuum-response.v1.schema.json",
        "Kanban maintenance vacuum response v1",
        "schemas/fixtures/api/maintenance-vacuum-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-vacuum-response.v1.invalid.json",
        crate::VacuumResponse,
    ),
];

const API_STATUS_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-status.headers",
        "maintenance-status",
        "GET /api/v1/maintenance/status headers",
        "GET /api/v1/maintenance/status",
        "GET /api/v1/maintenance/status",
        ApiHeaderProfile::Locale,
        crate::headers::LocaleHeaders,
        "locale-headers",
    ),
    api_contract!(
        "api.maintenance-status.response",
        "GET /api/v1/maintenance/status response",
        "GET /api/v1/maintenance/status",
        "GET /api/v1/maintenance/status",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-status-response:v1",
        "api/maintenance-status-response.v1.schema.json",
        "Kanban maintenance status response v1",
        "schemas/fixtures/api/maintenance-status-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-status-response.v1.invalid.json",
        crate::MaintenanceStatusResponse,
    ),
];

const API_RUN_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-run.headers",
        "maintenance-run",
        "POST /api/v1/maintenance/run headers",
        "POST /api/v1/maintenance/run",
        "POST /api/v1/maintenance/run",
        ApiHeaderProfile::LocaleJson,
        crate::headers::LocaleJsonHeaders,
        "locale-json-headers",
    ),
    api_contract!(
        "api.maintenance-run.request",
        "POST /api/v1/maintenance/run body",
        "POST /api/v1/maintenance/run",
        "POST /api/v1/maintenance/run",
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:maintenance-run-request:v1",
        "api/maintenance-run-request.v1.schema.json",
        "Kanban maintenance run request v1",
        "schemas/fixtures/api/maintenance-run-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-run-request.v1.invalid.json",
        crate::MaintenanceRunRequest,
    ),
    api_contract!(
        "api.maintenance-run.response",
        "POST /api/v1/maintenance/run response",
        "POST /api/v1/maintenance/run",
        "POST /api/v1/maintenance/run",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-run-response:v1",
        "api/maintenance-run-response.v1.schema.json",
        "Kanban maintenance run response v1",
        "schemas/fixtures/api/maintenance-run-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-run-response.v1.invalid.json",
        crate::MaintenanceRunResponse,
    ),
];

const API_REBUILD_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-rebuild.headers",
        "maintenance-rebuild",
        "POST /api/v1/maintenance/rebuild headers",
        "POST /api/v1/maintenance/rebuild",
        "POST /api/v1/maintenance/rebuild",
        ApiHeaderProfile::LocaleJson,
        crate::headers::LocaleJsonHeaders,
        "locale-json-headers",
    ),
    api_contract!(
        "api.maintenance-rebuild.request",
        "POST /api/v1/maintenance/rebuild body",
        "POST /api/v1/maintenance/rebuild",
        "POST /api/v1/maintenance/rebuild",
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:maintenance-rebuild-request:v1",
        "api/maintenance-rebuild-request.v1.schema.json",
        "Kanban maintenance rebuild request v1",
        "schemas/fixtures/api/maintenance-rebuild-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-rebuild-request.v1.invalid.json",
        crate::MaintenanceRunRequest,
    ),
    api_contract!(
        "api.maintenance-rebuild.response",
        "POST /api/v1/maintenance/rebuild response",
        "POST /api/v1/maintenance/rebuild",
        "POST /api/v1/maintenance/rebuild",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-rebuild-response:v1",
        "api/maintenance-rebuild-response.v1.schema.json",
        "Kanban maintenance rebuild response v1",
        "schemas/fixtures/api/maintenance-rebuild-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-rebuild-response.v1.invalid.json",
        crate::MaintenanceRunResponse,
    ),
];

const API_CLEANUP_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-cleanup.headers",
        "maintenance-cleanup",
        "POST /api/v1/maintenance/cleanup headers",
        "POST /api/v1/maintenance/cleanup",
        "POST /api/v1/maintenance/cleanup",
        ApiHeaderProfile::LocaleJson,
        crate::headers::LocaleJsonHeaders,
        "locale-json-headers",
    ),
    api_contract!(
        "api.maintenance-cleanup.request",
        "POST /api/v1/maintenance/cleanup body",
        "POST /api/v1/maintenance/cleanup",
        "POST /api/v1/maintenance/cleanup",
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:maintenance-cleanup-request:v1",
        "api/maintenance-cleanup-request.v1.schema.json",
        "Kanban maintenance cleanup request v1",
        "schemas/fixtures/api/maintenance-cleanup-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-cleanup-request.v1.invalid.json",
        crate::MaintenanceRunRequest,
    ),
    api_contract!(
        "api.maintenance-cleanup.response",
        "POST /api/v1/maintenance/cleanup response",
        "POST /api/v1/maintenance/cleanup",
        "POST /api/v1/maintenance/cleanup",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-cleanup-response:v1",
        "api/maintenance-cleanup-response.v1.schema.json",
        "Kanban maintenance cleanup response v1",
        "schemas/fixtures/api/maintenance-cleanup-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-cleanup-response.v1.invalid.json",
        crate::MaintenanceRunResponse,
    ),
];

const API_LEGACY_IMPORT_CONTRACTS: &[ContractDeclaration] = &[
    header_contract!(
        "api.maintenance-import-v30.headers",
        "maintenance-import-v30",
        "POST /api/v1/maintenance/import-v30 headers",
        "POST /api/v1/maintenance/import-v30",
        "POST /api/v1/maintenance/import-v30",
        ApiHeaderProfile::LocaleJson,
        crate::headers::LocaleJsonHeaders,
        "locale-json-headers",
    ),
    api_contract!(
        "api.maintenance-import-v30.request",
        "POST /api/v1/maintenance/import-v30 body",
        "POST /api/v1/maintenance/import-v30",
        "POST /api/v1/maintenance/import-v30",
        HttpTransportLocation::Body,
        &[],
        "urn:kanban-tool:schema:api:maintenance-import-v30-request:v1",
        "api/maintenance-import-v30-request.v1.schema.json",
        "Kanban legacy SQLite v30 import request v1",
        "schemas/fixtures/api/maintenance-import-v30-request.v1.valid.json",
        "schemas/fixtures/api/maintenance-import-v30-request.v1.invalid.json",
        crate::LegacyImportRequest,
    ),
    api_contract!(
        "api.maintenance-import-v30.response",
        "POST /api/v1/maintenance/import-v30 response",
        "POST /api/v1/maintenance/import-v30",
        "POST /api/v1/maintenance/import-v30",
        HttpTransportLocation::Success,
        &[],
        "urn:kanban-tool:schema:api:maintenance-import-v30-response:v1",
        "api/maintenance-import-v30-response.v1.schema.json",
        "Kanban legacy SQLite v30 import response v1",
        "schemas/fixtures/api/maintenance-import-v30-response.v1.valid.json",
        "schemas/fixtures/api/maintenance-import-v30-response.v1.invalid.json",
        crate::LegacyImportResponse,
    ),
];

const ADMIN_OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.health",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/health"),
        "GET /health",
        "GET /health",
        API_HEALTH_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.get-stats",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/stats"),
        "GET /api/v1/stats",
        "GET /api/v1/stats",
        API_STATS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(STATS_POLICY),
    OperationDeclaration::new(
        "api.doctor",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/maintenance/doctor"),
        "GET /api/v1/maintenance/doctor",
        "GET /api/v1/maintenance/doctor",
        API_DOCTOR_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.checkpoint",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/checkpoint"),
        "POST /api/v1/maintenance/checkpoint",
        "POST /api/v1/maintenance/checkpoint",
        API_CHECKPOINT_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-backup",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/backup"),
        "POST /api/v1/maintenance/backup",
        "POST /api/v1/maintenance/backup",
        API_BACKUP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-export",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/export"),
        "POST /api/v1/maintenance/export",
        "POST /api/v1/maintenance/export",
        API_EXPORT_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-import",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/import"),
        "POST /api/v1/maintenance/import",
        "POST /api/v1/maintenance/import",
        API_IMPORT_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-vacuum",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/vacuum"),
        "POST /api/v1/maintenance/vacuum",
        "POST /api/v1/maintenance/vacuum",
        API_VACUUM_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-status",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/maintenance/status"),
        "GET /api/v1/maintenance/status",
        "GET /api/v1/maintenance/status",
        API_STATUS_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-run",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/run"),
        "POST /api/v1/maintenance/run",
        "POST /api/v1/maintenance/run",
        API_RUN_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-rebuild",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/rebuild"),
        "POST /api/v1/maintenance/rebuild",
        "POST /api/v1/maintenance/rebuild",
        API_REBUILD_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-cleanup",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/cleanup"),
        "POST /api/v1/maintenance/cleanup",
        "POST /api/v1/maintenance/cleanup",
        API_CLEANUP_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(HOST_ADMIN_POLICY),
    OperationDeclaration::new(
        "api.maintenance-import-v30",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/maintenance/import-v30"),
        "POST /api/v1/maintenance/import-v30",
        "POST /api/v1/maintenance/import-v30",
        API_LEGACY_IMPORT_CONTRACTS,
    )
    .with_header_profile(ApiHeaderProfile::LocaleJson)
    .with_mcp_policy(HOST_ADMIN_POLICY),
];

/// Stats/health/maintenance 的 parent declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    ADMIN_OPERATIONS
}

/// 返回该 family 的全部 parent/child projection contracts。
pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(ADMIN_OPERATIONS).contracts()
}

/// 返回 source 中的 API endpoint projection。
pub fn endpoint_catalog() -> Vec<EndpointDescriptor> {
    crate::CatalogProjection::new(ADMIN_OPERATIONS).endpoints()
}

/// 按 operation id 查找 API endpoint projection。
pub fn endpoint_descriptor(id: &str) -> Option<EndpointDescriptor> {
    endpoint_catalog()
        .into_iter()
        .find(|endpoint| endpoint.operation_id == id)
}

/// 返回 source 中的 surface projection（仅 parent endpoint）。
pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(ADMIN_OPERATIONS).surfaces()
}

/// 返回 parent 上显式声明的 header profile。
pub fn header_profile(id: &str) -> Option<ApiHeaderProfile> {
    ADMIN_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == id)
        .and_then(|operation| operation.header_profile)
}

/// 返回 source 中的 API header child projection。
pub fn header_contract(id: &str) -> Option<OperationContract> {
    let parent = ADMIN_OPERATIONS
        .iter()
        .find(|operation| operation.operation_id == id)?;
    parent
        .contracts
        .iter()
        .find(|contract| contract.location == Some(HttpTransportLocation::Headers))
        .map(|contract| contract.operation_contract(parent))
}

/// 返回 source 中显式 schema roots（不含 standalone template）。
#[cfg(feature = "schema")]
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    crate::CatalogProjection::new(ADMIN_OPERATIONS).schemas()
}

/// 判断 contract 是否属于 admin parent source。
pub fn owns_contract(id: &str) -> bool {
    ADMIN_OPERATIONS
        .iter()
        .any(|operation| operation.contracts.iter().any(|contract| contract.id == id))
}

/// 判断 operation 是否属于 admin parent source。
pub fn owns_operation(id: &str) -> bool {
    ADMIN_OPERATIONS
        .iter()
        .any(|operation| operation.operation_id == id)
}

const MAINTENANCE_PATH_OPERATION: OperationContract = OperationContract {
    id: "api.maintenance-path.request",
    path: "POST /api/v1/maintenance/{operation} body",
    surface: ContractSurface::Api,
    operation: "POST /api/v1/maintenance/{operation}",
    direction: ContractDirection::Deserialize,
    granularity: ContractGranularity::Exact,
    strictness: ContractStrictness::DenyUnknownFields,
    schema_id: Some("urn:kanban-tool:schema:api:maintenance-path-request:v1"),
    fixture: Some("schemas/fixtures/api/maintenance-path-request.v1.valid.json"),
    exclusion: None,
    transport: ContractTransport::Http {
        operation_key: Some("POST /api/v1/maintenance/{operation}"),
        location: HttpTransportLocation::Body,
        parameters: &[],
    },
    binding: ContractBinding::ExactSurface,
};

/// Standalone generic maintenance path contract；它不伪造 endpoint 或 surface parent。
pub fn template_contracts() -> Vec<OperationContract> {
    vec![MAINTENANCE_PATH_OPERATION]
}

/// 以旧 operations.json 的顺序返回 standalone template 与 maintenance parent contracts。
pub fn inventory_contracts() -> Vec<OperationContract> {
    const ORDER: &[&str] = &[
        "api.maintenance-path.request",
        "api.maintenance-import.request",
        "api.maintenance-backup.request",
        "api.maintenance-export.request",
        "api.maintenance-run.request",
        "api.maintenance-rebuild.request",
        "api.maintenance-cleanup.request",
        "api.maintenance-import-v30.request",
        "api.maintenance-backup.response",
        "api.maintenance-export.response",
        "api.maintenance-import.response",
        "api.maintenance-vacuum.response",
        "api.maintenance-status.response",
        "api.maintenance-run.response",
        "api.maintenance-rebuild.response",
        "api.maintenance-cleanup.response",
        "api.maintenance-import-v30.response",
    ];
    let contracts = operation_contracts();
    ORDER
        .iter()
        .map(|id| {
            if *id == MAINTENANCE_PATH_OPERATION.id {
                return MAINTENANCE_PATH_OPERATION;
            }
            *contracts
                .iter()
                .find(|contract| contract.id == *id)
                .unwrap_or_else(|| panic!("missing admin contract source: {id}"))
        })
        .collect()
}

/// standalone generic maintenance path schema root。
#[cfg(feature = "schema")]
pub fn template_schema_roots() -> Vec<crate::schema::SchemaRoot> {
    vec![crate::schema::SchemaRoot {
        id: "urn:kanban-tool:schema:api:maintenance-path-request:v1",
        artifact_path: "api/maintenance-path-request.v1.schema.json",
        title: "Kanban maintenance path request v1",
        contract_id: "api.maintenance-path.request",
        direction: ContractDirection::Deserialize,
        strictness: ContractStrictness::DenyUnknownFields,
        valid_fixture: "schemas/fixtures/api/maintenance-path-request.v1.valid.json",
        invalid_fixture: "schemas/fixtures/api/maintenance-path-request.v1.invalid.json",
        generate: crate::generate_schema_for::<crate::MaintenancePathRequest>,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_source_keeps_endpoint_and_child_order() {
        assert_eq!(
            endpoint_catalog()
                .iter()
                .map(|endpoint| endpoint.operation_id)
                .collect::<Vec<_>>(),
            vec![
                "api.health",
                "api.get-stats",
                "api.doctor",
                "api.checkpoint",
                "api.maintenance-backup",
                "api.maintenance-export",
                "api.maintenance-import",
                "api.maintenance-vacuum",
                "api.maintenance-status",
                "api.maintenance-run",
                "api.maintenance-rebuild",
                "api.maintenance-cleanup",
                "api.maintenance-import-v30",
            ]
        );
        assert_eq!(operation_contracts().len(), 34);
        assert_eq!(inventory_contracts().len(), 17);
    }

    #[test]
    fn host_admin_policy_has_no_mcp_binding() {
        for operation in operation_declarations() {
            let policy = operation.mcp_policy.expect("admin policy");
            if operation.operation_id == "api.get-stats" {
                assert_eq!(policy.exposure, McpExposure::Domain);
                assert_eq!(policy.tool_bindings, STATS_BINDING);
            } else {
                assert_eq!(policy.exposure, McpExposure::HostAdmin);
                assert!(policy.tool_bindings.is_empty());
            }
        }
    }

    #[test]
    fn health_response_keeps_human_operation_separate_from_transport_key() {
        let contract = operation_contracts()
            .into_iter()
            .find(|contract| contract.id == "api.health.response")
            .expect("health response contract");
        assert_eq!(contract.operation, "localhost health report");
        assert_eq!(
            contract.transport,
            ContractTransport::Http {
                operation_key: Some("GET /health"),
                location: HttpTransportLocation::Success,
                parameters: &[],
            }
        );
    }
}
