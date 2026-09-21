//! 模块与迭代的 API、CLI 和 MCP 共同契约。
use crate::*;

macro_rules! contract {
    ($id:expr, $path:expr, $direction:expr, $location:expr, $params:expr, $file:expr, $schema_id:expr, $type:ty) => {{
        let c = ContractDeclaration::new(
            $id,
            $path,
            $direction,
            $location,
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, $params)
        .with_schema(
            $schema_id,
            concat!($file, ".v1.schema.json"),
            $id,
            concat!("schemas/fixtures/", $file, ".v1.valid.json"),
            concat!("schemas/fixtures/", $file, ".v1.invalid.json"),
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<$type>();
        c
    }};
}

const INVARIANTS: &[McpOperationInvariant] = &[
    McpOperationInvariant::CanonicalHostOnly,
    McpOperationInvariant::SharedApplicationService,
    McpOperationInvariant::NoHostAdminSurface,
];
const GET_PLANNING_CAPABILITIES: &[ContractDeclaration] = &[
    contract!(
        "api.get-planning-capabilities.response",
        "GET /api/v1/planning/capabilities response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/get-planning-capabilities-response",
        "urn:kanban-tool:schema:api:get-planning-capabilities-response:v1",
        crate::PlanningCapabilitiesResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.get-planning-capabilities.headers",
            "GET /api/v1/planning/capabilities headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:get-planning-capabilities-headers:v1",
            "api/get-planning-capabilities-headers.v1.schema.json",
            "get-planning-capabilities headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const LIST_MODULES: &[ContractDeclaration] = &[
    contract!(
        "api.list-modules.path",
        "GET /api/v1/boards/:board/modules/list-modules path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }],
        "api/list-modules-path",
        "urn:kanban-tool:schema:api:list-modules-path:v1",
        crate::PlanningBoardPath
    ),
    contract!(
        "api.list-modules.query",
        "GET /api/v1/boards/:board/modules/list-modules query",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Query),
        &[
            WireParameter {
                name: "q",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "include_archived",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "limit",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "offset",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "parent_id",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            }
        ],
        "api/list-modules-query",
        "urn:kanban-tool:schema:api:list-modules-query:v1",
        crate::ListModulesQuery
    ),
    contract!(
        "api.list-modules.response",
        "GET /api/v1/boards/:board/modules/list-modules response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/list-modules-response",
        "urn:kanban-tool:schema:api:list-modules-response:v1",
        crate::ListModulesResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.list-modules.headers",
            "GET /api/v1/boards/:board/modules/list-modules headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:list-modules-headers:v1",
            "api/list-modules-headers.v1.schema.json",
            "list-modules headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const CLI_LIST_MODULES: &[ContractDeclaration] = &[contract!(
    "cli.module-list.output",
    "kanban module list --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-list-output",
    "urn:kanban-tool:schema:cli:module-list-output:v1",
    crate::ListModulesResponse
)];
const GET_MODULE: &[ContractDeclaration] = &[
    contract!(
        "api.get-module.path",
        "GET /api/v1/boards/:board/modules/:id/get-module path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/get-module-path",
        "urn:kanban-tool:schema:api:get-module-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.get-module.response",
        "GET /api/v1/boards/:board/modules/:id/get-module response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/get-module-response",
        "urn:kanban-tool:schema:api:get-module-response:v1",
        crate::GetModuleResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.get-module.headers",
            "GET /api/v1/boards/:board/modules/:id/get-module headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:get-module-headers:v1",
            "api/get-module-headers.v1.schema.json",
            "get-module headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const CLI_GET_MODULE: &[ContractDeclaration] = &[contract!(
    "cli.module-show.output",
    "kanban module show --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-show-output",
    "urn:kanban-tool:schema:cli:module-show-output:v1",
    crate::GetModuleResponse
)];
const CREATE_MODULE: &[ContractDeclaration] = &[
    contract!(
        "api.create-module.path",
        "POST /api/v1/boards/:board/modules/create-module path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }],
        "api/create-module-path",
        "urn:kanban-tool:schema:api:create-module-path:v1",
        crate::PlanningBoardPath
    ),
    contract!(
        "api.create-module.request",
        "POST /api/v1/boards/:board/modules/create-module request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/create-module-request",
        "urn:kanban-tool:schema:api:create-module-request:v1",
        crate::CreateModuleRequest
    ),
    contract!(
        "api.create-module.response",
        "POST /api/v1/boards/:board/modules/create-module response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/create-module-response",
        "urn:kanban-tool:schema:api:create-module-response:v1",
        crate::ModuleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.create-module.headers",
            "POST /api/v1/boards/:board/modules/create-module headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:create-module-headers:v1",
            "api/create-module-headers.v1.schema.json",
            "create-module headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_CREATE_MODULE: &[ContractDeclaration] = &[contract!(
    "cli.module-create.output",
    "kanban module create --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-create-output",
    "urn:kanban-tool:schema:cli:module-create-output:v1",
    crate::ModuleMutationResponse
)];
const UPDATE_MODULE: &[ContractDeclaration] = &[
    contract!(
        "api.update-module.path",
        "POST /api/v1/boards/:board/modules/:id/update-module path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/update-module-path",
        "urn:kanban-tool:schema:api:update-module-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.update-module.request",
        "POST /api/v1/boards/:board/modules/:id/update-module request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/update-module-request",
        "urn:kanban-tool:schema:api:update-module-request:v1",
        crate::UpdateModuleRequest
    ),
    contract!(
        "api.update-module.response",
        "POST /api/v1/boards/:board/modules/:id/update-module response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/update-module-response",
        "urn:kanban-tool:schema:api:update-module-response:v1",
        crate::ModuleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.update-module.headers",
            "POST /api/v1/boards/:board/modules/:id/update-module headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:update-module-headers:v1",
            "api/update-module-headers.v1.schema.json",
            "update-module headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_UPDATE_MODULE: &[ContractDeclaration] = &[contract!(
    "cli.module-update.output",
    "kanban module update --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-update-output",
    "urn:kanban-tool:schema:cli:module-update-output:v1",
    crate::ModuleMutationResponse
)];
const ARCHIVE_MODULE: &[ContractDeclaration] = &[
    contract!(
        "api.archive-module.path",
        "POST /api/v1/boards/:board/modules/:id/archive-module path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/archive-module-path",
        "urn:kanban-tool:schema:api:archive-module-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.archive-module.request",
        "POST /api/v1/boards/:board/modules/:id/archive-module request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/archive-module-request",
        "urn:kanban-tool:schema:api:archive-module-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.archive-module.response",
        "POST /api/v1/boards/:board/modules/:id/archive-module response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/archive-module-response",
        "urn:kanban-tool:schema:api:archive-module-response:v1",
        crate::ModuleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.archive-module.headers",
            "POST /api/v1/boards/:board/modules/:id/archive-module headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:archive-module-headers:v1",
            "api/archive-module-headers.v1.schema.json",
            "archive-module headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_ARCHIVE_MODULE: &[ContractDeclaration] = &[contract!(
    "cli.module-archive.output",
    "kanban module archive --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-archive-output",
    "urn:kanban-tool:schema:cli:module-archive-output:v1",
    crate::ModuleMutationResponse
)];
const RESTORE_MODULE: &[ContractDeclaration] = &[
    contract!(
        "api.restore-module.path",
        "POST /api/v1/boards/:board/modules/:id/restore-module path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/restore-module-path",
        "urn:kanban-tool:schema:api:restore-module-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.restore-module.request",
        "POST /api/v1/boards/:board/modules/:id/restore-module request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/restore-module-request",
        "urn:kanban-tool:schema:api:restore-module-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.restore-module.response",
        "POST /api/v1/boards/:board/modules/:id/restore-module response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/restore-module-response",
        "urn:kanban-tool:schema:api:restore-module-response:v1",
        crate::ModuleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.restore-module.headers",
            "POST /api/v1/boards/:board/modules/:id/restore-module headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:restore-module-headers:v1",
            "api/restore-module-headers.v1.schema.json",
            "restore-module headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_RESTORE_MODULE: &[ContractDeclaration] = &[contract!(
    "cli.module-restore.output",
    "kanban module restore --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-restore-output",
    "urn:kanban-tool:schema:cli:module-restore-output:v1",
    crate::ModuleMutationResponse
)];
const GET_MODULE_OVERVIEW: &[ContractDeclaration] = &[
    contract!(
        "api.get-module-overview.path",
        "GET /api/v1/boards/:board/modules/:id/get-module-overview path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/get-module-overview-path",
        "urn:kanban-tool:schema:api:get-module-overview-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.get-module-overview.response",
        "GET /api/v1/boards/:board/modules/:id/get-module-overview response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/get-module-overview-response",
        "urn:kanban-tool:schema:api:get-module-overview-response:v1",
        crate::ModuleOverviewResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.get-module-overview.headers",
            "GET /api/v1/boards/:board/modules/:id/get-module-overview headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:get-module-overview-headers:v1",
            "api/get-module-overview-headers.v1.schema.json",
            "get-module-overview headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const CLI_GET_MODULE_OVERVIEW: &[ContractDeclaration] = &[contract!(
    "cli.module-overview.output",
    "kanban module overview --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-overview-output",
    "urn:kanban-tool:schema:cli:module-overview-output:v1",
    crate::ModuleOverviewResponse
)];
const LIST_MODULE_TASKS: &[ContractDeclaration] = &[
    contract!(
        "api.list-module-tasks.path",
        "GET /api/v1/boards/:board/modules/:id/list-module-tasks path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/list-module-tasks-path",
        "urn:kanban-tool:schema:api:list-module-tasks-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.list-module-tasks.query",
        "GET /api/v1/boards/:board/modules/:id/list-module-tasks query",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Query),
        &[
            WireParameter {
                name: "limit",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "offset",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            }
        ],
        "api/list-module-tasks-query",
        "urn:kanban-tool:schema:api:list-module-tasks-query:v1",
        crate::PlanningMembersQuery
    ),
    contract!(
        "api.list-module-tasks.response",
        "GET /api/v1/boards/:board/modules/:id/list-module-tasks response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/list-module-tasks-response",
        "urn:kanban-tool:schema:api:list-module-tasks-response:v1",
        crate::PlanningMembersResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.list-module-tasks.headers",
            "GET /api/v1/boards/:board/modules/:id/list-module-tasks headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:list-module-tasks-headers:v1",
            "api/list-module-tasks-headers.v1.schema.json",
            "list-module-tasks headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const CLI_LIST_MODULE_TASKS: &[ContractDeclaration] = &[contract!(
    "cli.module-task-list.output",
    "kanban module task list --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-task-list-output",
    "urn:kanban-tool:schema:cli:module-task-list-output:v1",
    crate::PlanningMembersResponse
)];
const ADD_MODULE_TASK: &[ContractDeclaration] = &[
    contract!(
        "api.add-module-task.path",
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/add-module-task path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "task_id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/add-module-task-path",
        "urn:kanban-tool:schema:api:add-module-task-path:v1",
        crate::PlanningMemberPath
    ),
    contract!(
        "api.add-module-task.request",
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/add-module-task request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/add-module-task-request",
        "urn:kanban-tool:schema:api:add-module-task-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.add-module-task.response",
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/add-module-task response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/add-module-task-response",
        "urn:kanban-tool:schema:api:add-module-task-response:v1",
        crate::ModuleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.add-module-task.headers",
            "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/add-module-task headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:add-module-task-headers:v1",
            "api/add-module-task-headers.v1.schema.json",
            "add-module-task headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_ADD_MODULE_TASK: &[ContractDeclaration] = &[contract!(
    "cli.module-task-add.output",
    "kanban module task add --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-task-add-output",
    "urn:kanban-tool:schema:cli:module-task-add-output:v1",
    crate::ModuleMutationResponse
)];
const REMOVE_MODULE_TASK: &[ContractDeclaration] = &[
    contract!(
        "api.remove-module-task.path",
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/remove-module-task path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "task_id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/remove-module-task-path",
        "urn:kanban-tool:schema:api:remove-module-task-path:v1",
        crate::PlanningMemberPath
    ),
    contract!(
        "api.remove-module-task.request",
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/remove-module-task request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/remove-module-task-request",
        "urn:kanban-tool:schema:api:remove-module-task-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.remove-module-task.response",
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/remove-module-task response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/remove-module-task-response",
        "urn:kanban-tool:schema:api:remove-module-task-response:v1",
        crate::ModuleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.remove-module-task.headers",
            "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/remove-module-task headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:remove-module-task-headers:v1",
            "api/remove-module-task-headers.v1.schema.json",
            "remove-module-task headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_REMOVE_MODULE_TASK: &[ContractDeclaration] = &[contract!(
    "cli.module-task-remove.output",
    "kanban module task remove --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/module-task-remove-output",
    "urn:kanban-tool:schema:cli:module-task-remove-output:v1",
    crate::ModuleMutationResponse
)];
const LIST_CYCLES: &[ContractDeclaration] = &[
    contract!(
        "api.list-cycles.path",
        "GET /api/v1/boards/:board/cycles/list-cycles path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }],
        "api/list-cycles-path",
        "urn:kanban-tool:schema:api:list-cycles-path:v1",
        crate::PlanningBoardPath
    ),
    contract!(
        "api.list-cycles.query",
        "GET /api/v1/boards/:board/cycles/list-cycles query",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Query),
        &[
            WireParameter {
                name: "q",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "include_archived",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "limit",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "offset",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "status",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            }
        ],
        "api/list-cycles-query",
        "urn:kanban-tool:schema:api:list-cycles-query:v1",
        crate::ListCyclesQuery
    ),
    contract!(
        "api.list-cycles.response",
        "GET /api/v1/boards/:board/cycles/list-cycles response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/list-cycles-response",
        "urn:kanban-tool:schema:api:list-cycles-response:v1",
        crate::ListCyclesResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.list-cycles.headers",
            "GET /api/v1/boards/:board/cycles/list-cycles headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:list-cycles-headers:v1",
            "api/list-cycles-headers.v1.schema.json",
            "list-cycles headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const CLI_LIST_CYCLES: &[ContractDeclaration] = &[contract!(
    "cli.cycle-list.output",
    "kanban cycle list --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-list-output",
    "urn:kanban-tool:schema:cli:cycle-list-output:v1",
    crate::ListCyclesResponse
)];
const GET_CYCLE: &[ContractDeclaration] = &[
    contract!(
        "api.get-cycle.path",
        "GET /api/v1/boards/:board/cycles/:id/get-cycle path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/get-cycle-path",
        "urn:kanban-tool:schema:api:get-cycle-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.get-cycle.response",
        "GET /api/v1/boards/:board/cycles/:id/get-cycle response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/get-cycle-response",
        "urn:kanban-tool:schema:api:get-cycle-response:v1",
        crate::GetCycleResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.get-cycle.headers",
            "GET /api/v1/boards/:board/cycles/:id/get-cycle headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:get-cycle-headers:v1",
            "api/get-cycle-headers.v1.schema.json",
            "get-cycle headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const CLI_GET_CYCLE: &[ContractDeclaration] = &[contract!(
    "cli.cycle-show.output",
    "kanban cycle show --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-show-output",
    "urn:kanban-tool:schema:cli:cycle-show-output:v1",
    crate::GetCycleResponse
)];
const CREATE_CYCLE: &[ContractDeclaration] = &[
    contract!(
        "api.create-cycle.path",
        "POST /api/v1/boards/:board/cycles/create-cycle path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }],
        "api/create-cycle-path",
        "urn:kanban-tool:schema:api:create-cycle-path:v1",
        crate::PlanningBoardPath
    ),
    contract!(
        "api.create-cycle.request",
        "POST /api/v1/boards/:board/cycles/create-cycle request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/create-cycle-request",
        "urn:kanban-tool:schema:api:create-cycle-request:v1",
        crate::CreateCycleRequest
    ),
    contract!(
        "api.create-cycle.response",
        "POST /api/v1/boards/:board/cycles/create-cycle response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/create-cycle-response",
        "urn:kanban-tool:schema:api:create-cycle-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.create-cycle.headers",
            "POST /api/v1/boards/:board/cycles/create-cycle headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:create-cycle-headers:v1",
            "api/create-cycle-headers.v1.schema.json",
            "create-cycle headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_CREATE_CYCLE: &[ContractDeclaration] = &[contract!(
    "cli.cycle-create.output",
    "kanban cycle create --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-create-output",
    "urn:kanban-tool:schema:cli:cycle-create-output:v1",
    crate::CycleMutationResponse
)];
const UPDATE_CYCLE: &[ContractDeclaration] = &[
    contract!(
        "api.update-cycle.path",
        "POST /api/v1/boards/:board/cycles/:id/update-cycle path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/update-cycle-path",
        "urn:kanban-tool:schema:api:update-cycle-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.update-cycle.request",
        "POST /api/v1/boards/:board/cycles/:id/update-cycle request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/update-cycle-request",
        "urn:kanban-tool:schema:api:update-cycle-request:v1",
        crate::UpdateCycleRequest
    ),
    contract!(
        "api.update-cycle.response",
        "POST /api/v1/boards/:board/cycles/:id/update-cycle response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/update-cycle-response",
        "urn:kanban-tool:schema:api:update-cycle-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.update-cycle.headers",
            "POST /api/v1/boards/:board/cycles/:id/update-cycle headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:update-cycle-headers:v1",
            "api/update-cycle-headers.v1.schema.json",
            "update-cycle headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_UPDATE_CYCLE: &[ContractDeclaration] = &[contract!(
    "cli.cycle-update.output",
    "kanban cycle update --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-update-output",
    "urn:kanban-tool:schema:cli:cycle-update-output:v1",
    crate::CycleMutationResponse
)];
const ARCHIVE_CYCLE: &[ContractDeclaration] = &[
    contract!(
        "api.archive-cycle.path",
        "POST /api/v1/boards/:board/cycles/:id/archive-cycle path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/archive-cycle-path",
        "urn:kanban-tool:schema:api:archive-cycle-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.archive-cycle.request",
        "POST /api/v1/boards/:board/cycles/:id/archive-cycle request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/archive-cycle-request",
        "urn:kanban-tool:schema:api:archive-cycle-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.archive-cycle.response",
        "POST /api/v1/boards/:board/cycles/:id/archive-cycle response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/archive-cycle-response",
        "urn:kanban-tool:schema:api:archive-cycle-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.archive-cycle.headers",
            "POST /api/v1/boards/:board/cycles/:id/archive-cycle headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:archive-cycle-headers:v1",
            "api/archive-cycle-headers.v1.schema.json",
            "archive-cycle headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_ARCHIVE_CYCLE: &[ContractDeclaration] = &[contract!(
    "cli.cycle-archive.output",
    "kanban cycle archive --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-archive-output",
    "urn:kanban-tool:schema:cli:cycle-archive-output:v1",
    crate::CycleMutationResponse
)];
const RESTORE_CYCLE: &[ContractDeclaration] = &[
    contract!(
        "api.restore-cycle.path",
        "POST /api/v1/boards/:board/cycles/:id/restore-cycle path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/restore-cycle-path",
        "urn:kanban-tool:schema:api:restore-cycle-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.restore-cycle.request",
        "POST /api/v1/boards/:board/cycles/:id/restore-cycle request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/restore-cycle-request",
        "urn:kanban-tool:schema:api:restore-cycle-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.restore-cycle.response",
        "POST /api/v1/boards/:board/cycles/:id/restore-cycle response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/restore-cycle-response",
        "urn:kanban-tool:schema:api:restore-cycle-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.restore-cycle.headers",
            "POST /api/v1/boards/:board/cycles/:id/restore-cycle headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:restore-cycle-headers:v1",
            "api/restore-cycle-headers.v1.schema.json",
            "restore-cycle headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_RESTORE_CYCLE: &[ContractDeclaration] = &[contract!(
    "cli.cycle-restore.output",
    "kanban cycle restore --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-restore-output",
    "urn:kanban-tool:schema:cli:cycle-restore-output:v1",
    crate::CycleMutationResponse
)];
const GET_CYCLE_OVERVIEW: &[ContractDeclaration] = &[
    contract!(
        "api.get-cycle-overview.path",
        "GET /api/v1/boards/:board/cycles/:id/get-cycle-overview path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/get-cycle-overview-path",
        "urn:kanban-tool:schema:api:get-cycle-overview-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.get-cycle-overview.response",
        "GET /api/v1/boards/:board/cycles/:id/get-cycle-overview response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/get-cycle-overview-response",
        "urn:kanban-tool:schema:api:get-cycle-overview-response:v1",
        crate::CycleOverviewResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.get-cycle-overview.headers",
            "GET /api/v1/boards/:board/cycles/:id/get-cycle-overview headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:get-cycle-overview-headers:v1",
            "api/get-cycle-overview-headers.v1.schema.json",
            "get-cycle-overview headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const CLI_GET_CYCLE_OVERVIEW: &[ContractDeclaration] = &[contract!(
    "cli.cycle-overview.output",
    "kanban cycle overview --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-overview-output",
    "urn:kanban-tool:schema:cli:cycle-overview-output:v1",
    crate::CycleOverviewResponse
)];
const LIST_CYCLE_TASKS: &[ContractDeclaration] = &[
    contract!(
        "api.list-cycle-tasks.path",
        "GET /api/v1/boards/:board/cycles/:id/list-cycle-tasks path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/list-cycle-tasks-path",
        "urn:kanban-tool:schema:api:list-cycle-tasks-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.list-cycle-tasks.query",
        "GET /api/v1/boards/:board/cycles/:id/list-cycle-tasks query",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Query),
        &[
            WireParameter {
                name: "limit",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            },
            WireParameter {
                name: "offset",
                cardinality: Some(WireParameterCardinality::OptionalOne)
            }
        ],
        "api/list-cycle-tasks-query",
        "urn:kanban-tool:schema:api:list-cycle-tasks-query:v1",
        crate::PlanningMembersQuery
    ),
    contract!(
        "api.list-cycle-tasks.response",
        "GET /api/v1/boards/:board/cycles/:id/list-cycle-tasks response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/list-cycle-tasks-response",
        "urn:kanban-tool:schema:api:list-cycle-tasks-response:v1",
        crate::PlanningMembersResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.list-cycle-tasks.headers",
            "GET /api/v1/boards/:board/cycles/:id/list-cycle-tasks headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::Locale.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:list-cycle-tasks-headers:v1",
            "api/list-cycle-tasks-headers.v1.schema.json",
            "list-cycle-tasks headers",
            "schemas/fixtures/api/headers/locale-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleHeaders>();
        c
    },
];
const CLI_LIST_CYCLE_TASKS: &[ContractDeclaration] = &[contract!(
    "cli.cycle-task-list.output",
    "kanban cycle task list --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-task-list-output",
    "urn:kanban-tool:schema:cli:cycle-task-list-output:v1",
    crate::PlanningMembersResponse
)];
const ADD_CYCLE_TASK: &[ContractDeclaration] = &[
    contract!(
        "api.add-cycle-task.path",
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/add-cycle-task path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "task_id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/add-cycle-task-path",
        "urn:kanban-tool:schema:api:add-cycle-task-path:v1",
        crate::PlanningMemberPath
    ),
    contract!(
        "api.add-cycle-task.request",
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/add-cycle-task request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/add-cycle-task-request",
        "urn:kanban-tool:schema:api:add-cycle-task-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.add-cycle-task.response",
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/add-cycle-task response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/add-cycle-task-response",
        "urn:kanban-tool:schema:api:add-cycle-task-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.add-cycle-task.headers",
            "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/add-cycle-task headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:add-cycle-task-headers:v1",
            "api/add-cycle-task-headers.v1.schema.json",
            "add-cycle-task headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_ADD_CYCLE_TASK: &[ContractDeclaration] = &[contract!(
    "cli.cycle-task-add.output",
    "kanban cycle task add --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-task-add-output",
    "urn:kanban-tool:schema:cli:cycle-task-add-output:v1",
    crate::CycleMutationResponse
)];
const REMOVE_CYCLE_TASK: &[ContractDeclaration] = &[
    contract!(
        "api.remove-cycle-task.path",
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/remove-cycle-task path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "task_id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/remove-cycle-task-path",
        "urn:kanban-tool:schema:api:remove-cycle-task-path:v1",
        crate::PlanningMemberPath
    ),
    contract!(
        "api.remove-cycle-task.request",
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/remove-cycle-task request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/remove-cycle-task-request",
        "urn:kanban-tool:schema:api:remove-cycle-task-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.remove-cycle-task.response",
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/remove-cycle-task response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/remove-cycle-task-response",
        "urn:kanban-tool:schema:api:remove-cycle-task-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.remove-cycle-task.headers",
            "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/remove-cycle-task headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:remove-cycle-task-headers:v1",
            "api/remove-cycle-task-headers.v1.schema.json",
            "remove-cycle-task headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_REMOVE_CYCLE_TASK: &[ContractDeclaration] = &[contract!(
    "cli.cycle-task-remove.output",
    "kanban cycle task remove --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-task-remove-output",
    "urn:kanban-tool:schema:cli:cycle-task-remove-output:v1",
    crate::CycleMutationResponse
)];
const START_CYCLE: &[ContractDeclaration] = &[
    contract!(
        "api.start-cycle.path",
        "POST /api/v1/boards/:board/cycles/:id/start-cycle path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/start-cycle-path",
        "urn:kanban-tool:schema:api:start-cycle-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.start-cycle.request",
        "POST /api/v1/boards/:board/cycles/:id/start-cycle request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/start-cycle-request",
        "urn:kanban-tool:schema:api:start-cycle-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.start-cycle.response",
        "POST /api/v1/boards/:board/cycles/:id/start-cycle response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/start-cycle-response",
        "urn:kanban-tool:schema:api:start-cycle-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.start-cycle.headers",
            "POST /api/v1/boards/:board/cycles/:id/start-cycle headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:start-cycle-headers:v1",
            "api/start-cycle-headers.v1.schema.json",
            "start-cycle headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_START_CYCLE: &[ContractDeclaration] = &[contract!(
    "cli.cycle-start.output",
    "kanban cycle start --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-start-output",
    "urn:kanban-tool:schema:cli:cycle-start-output:v1",
    crate::CycleMutationResponse
)];
const CLOSE_CYCLE: &[ContractDeclaration] = &[
    contract!(
        "api.close-cycle.path",
        "POST /api/v1/boards/:board/cycles/:id/close-cycle path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/close-cycle-path",
        "urn:kanban-tool:schema:api:close-cycle-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.close-cycle.request",
        "POST /api/v1/boards/:board/cycles/:id/close-cycle request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/close-cycle-request",
        "urn:kanban-tool:schema:api:close-cycle-request:v1",
        crate::CloseCycleRequest
    ),
    contract!(
        "api.close-cycle.response",
        "POST /api/v1/boards/:board/cycles/:id/close-cycle response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/close-cycle-response",
        "urn:kanban-tool:schema:api:close-cycle-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.close-cycle.headers",
            "POST /api/v1/boards/:board/cycles/:id/close-cycle headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:close-cycle-headers:v1",
            "api/close-cycle-headers.v1.schema.json",
            "close-cycle headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_CLOSE_CYCLE: &[ContractDeclaration] = &[contract!(
    "cli.cycle-close.output",
    "kanban cycle close --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-close-output",
    "urn:kanban-tool:schema:cli:cycle-close-output:v1",
    crate::CycleMutationResponse
)];
const CANCEL_CYCLE: &[ContractDeclaration] = &[
    contract!(
        "api.cancel-cycle.path",
        "POST /api/v1/boards/:board/cycles/:id/cancel-cycle path",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Path),
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ],
        "api/cancel-cycle-path",
        "urn:kanban-tool:schema:api:cancel-cycle-path:v1",
        crate::PlanningObjectPath
    ),
    contract!(
        "api.cancel-cycle.request",
        "POST /api/v1/boards/:board/cycles/:id/cancel-cycle request",
        ContractDirection::Deserialize,
        Some(HttpTransportLocation::Body),
        &[],
        "api/cancel-cycle-request",
        "urn:kanban-tool:schema:api:cancel-cycle-request:v1",
        crate::PlanningActionRequest
    ),
    contract!(
        "api.cancel-cycle.response",
        "POST /api/v1/boards/:board/cycles/:id/cancel-cycle response",
        ContractDirection::Serialize,
        Some(HttpTransportLocation::Success),
        &[],
        "api/cancel-cycle-response",
        "urn:kanban-tool:schema:api:cancel-cycle-response:v1",
        crate::CycleMutationResponse
    ),
    {
        let c = ContractDeclaration::new(
            "api.cancel-cycle.headers",
            "POST /api/v1/boards/:board/cycles/:id/cancel-cycle headers",
            ContractDirection::Deserialize,
            Some(HttpTransportLocation::Headers),
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_transport(None, ApiHeaderProfile::LocaleActorJson.parameters())
        .with_schema(
            "urn:kanban-tool:schema:api:cancel-cycle-headers:v1",
            "api/cancel-cycle-headers.v1.schema.json",
            "cancel-cycle headers",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.valid.json",
            "schemas/fixtures/api/headers/locale-actor-json-headers.v1.invalid.json",
        );
        #[cfg(feature = "schema")]
        let c = c.with_schema_type::<crate::headers::LocaleActorJsonHeaders>();
        c
    },
];
const CLI_CANCEL_CYCLE: &[ContractDeclaration] = &[contract!(
    "cli.cycle-cancel.output",
    "kanban cycle cancel --json stdout",
    ContractDirection::Serialize,
    None,
    &[],
    "cli/cycle-cancel-output",
    "urn:kanban-tool:schema:cli:cycle-cancel-output:v1",
    crate::CycleMutationResponse
)];
const OPERATIONS: &[OperationDeclaration] = &[
    OperationDeclaration::new(
        "api.get-planning-capabilities",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/planning/capabilities"),
        "GET /api/v1/planning/capabilities",
        "GET /api/v1/planning/capabilities",
        GET_PLANNING_CAPABILITIES,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "api.list-modules",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/modules/list-modules"),
        "GET /api/v1/boards/:board/modules/list-modules",
        "GET /api/v1/boards/:board/modules/list-modules",
        LIST_MODULES,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_list",
            http_operations: &["api.get-board", "api.list-modules"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-list",
        ContractSurface::Cli,
        None,
        None,
        "module list",
        "module list",
        CLI_LIST_MODULES,
    ),
    OperationDeclaration::new(
        "api.get-module",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/modules/:id/get-module"),
        "GET /api/v1/boards/:board/modules/:id/get-module",
        "GET /api/v1/boards/:board/modules/:id/get-module",
        GET_MODULE,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_show",
            http_operations: &["api.get-board", "api.get-module"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-show",
        ContractSurface::Cli,
        None,
        None,
        "module show",
        "module show",
        CLI_GET_MODULE,
    ),
    OperationDeclaration::new(
        "api.create-module",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/modules/create-module"),
        "POST /api/v1/boards/:board/modules/create-module",
        "POST /api/v1/boards/:board/modules/create-module",
        CREATE_MODULE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_create",
            http_operations: &["api.get-board", "api.create-module"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-create",
        ContractSurface::Cli,
        None,
        None,
        "module create",
        "module create",
        CLI_CREATE_MODULE,
    ),
    OperationDeclaration::new(
        "api.update-module",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/modules/:id/update-module"),
        "POST /api/v1/boards/:board/modules/:id/update-module",
        "POST /api/v1/boards/:board/modules/:id/update-module",
        UPDATE_MODULE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_update",
            http_operations: &["api.get-board", "api.update-module"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-update",
        ContractSurface::Cli,
        None,
        None,
        "module update",
        "module update",
        CLI_UPDATE_MODULE,
    ),
    OperationDeclaration::new(
        "api.archive-module",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/modules/:id/archive-module"),
        "POST /api/v1/boards/:board/modules/:id/archive-module",
        "POST /api/v1/boards/:board/modules/:id/archive-module",
        ARCHIVE_MODULE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_archive",
            http_operations: &["api.get-board", "api.archive-module"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-archive",
        ContractSurface::Cli,
        None,
        None,
        "module archive",
        "module archive",
        CLI_ARCHIVE_MODULE,
    ),
    OperationDeclaration::new(
        "api.restore-module",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/modules/:id/restore-module"),
        "POST /api/v1/boards/:board/modules/:id/restore-module",
        "POST /api/v1/boards/:board/modules/:id/restore-module",
        RESTORE_MODULE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_restore",
            http_operations: &["api.get-board", "api.restore-module"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-restore",
        ContractSurface::Cli,
        None,
        None,
        "module restore",
        "module restore",
        CLI_RESTORE_MODULE,
    ),
    OperationDeclaration::new(
        "api.get-module-overview",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/modules/:id/get-module-overview"),
        "GET /api/v1/boards/:board/modules/:id/get-module-overview",
        "GET /api/v1/boards/:board/modules/:id/get-module-overview",
        GET_MODULE_OVERVIEW,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_overview",
            http_operations: &["api.get-board", "api.get-module-overview"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-overview",
        ContractSurface::Cli,
        None,
        None,
        "module overview",
        "module overview",
        CLI_GET_MODULE_OVERVIEW,
    ),
    OperationDeclaration::new(
        "api.list-module-tasks",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/modules/:id/list-module-tasks"),
        "GET /api/v1/boards/:board/modules/:id/list-module-tasks",
        "GET /api/v1/boards/:board/modules/:id/list-module-tasks",
        LIST_MODULE_TASKS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_task_list",
            http_operations: &["api.get-board", "api.list-module-tasks"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-task-list",
        ContractSurface::Cli,
        None,
        None,
        "module task list",
        "module task list",
        CLI_LIST_MODULE_TASKS,
    ),
    OperationDeclaration::new(
        "api.add-module-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/modules/:id/tasks/:task_id/add-module-task"),
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/add-module-task",
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/add-module-task",
        ADD_MODULE_TASK,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_task_add",
            http_operations: &["api.get-board", "api.list-tasks", "api.add-module-task"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-task-add",
        ContractSurface::Cli,
        None,
        None,
        "module task add",
        "module task add",
        CLI_ADD_MODULE_TASK,
    ),
    OperationDeclaration::new(
        "api.remove-module-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/modules/:id/tasks/:task_id/remove-module-task"),
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/remove-module-task",
        "POST /api/v1/boards/:board/modules/:id/tasks/:task_id/remove-module-task",
        REMOVE_MODULE_TASK,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "module_task_remove",
            http_operations: &["api.get-board", "api.list-tasks", "api.remove-module-task"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.module-task-remove",
        ContractSurface::Cli,
        None,
        None,
        "module task remove",
        "module task remove",
        CLI_REMOVE_MODULE_TASK,
    ),
    OperationDeclaration::new(
        "api.list-cycles",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/cycles/list-cycles"),
        "GET /api/v1/boards/:board/cycles/list-cycles",
        "GET /api/v1/boards/:board/cycles/list-cycles",
        LIST_CYCLES,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_list",
            http_operations: &["api.get-board", "api.list-cycles"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-list",
        ContractSurface::Cli,
        None,
        None,
        "cycle list",
        "cycle list",
        CLI_LIST_CYCLES,
    ),
    OperationDeclaration::new(
        "api.get-cycle",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/cycles/:id/get-cycle"),
        "GET /api/v1/boards/:board/cycles/:id/get-cycle",
        "GET /api/v1/boards/:board/cycles/:id/get-cycle",
        GET_CYCLE,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_show",
            http_operations: &["api.get-board", "api.get-cycle"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-show",
        ContractSurface::Cli,
        None,
        None,
        "cycle show",
        "cycle show",
        CLI_GET_CYCLE,
    ),
    OperationDeclaration::new(
        "api.create-cycle",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/create-cycle"),
        "POST /api/v1/boards/:board/cycles/create-cycle",
        "POST /api/v1/boards/:board/cycles/create-cycle",
        CREATE_CYCLE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_create",
            http_operations: &["api.get-board", "api.create-cycle"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-create",
        ContractSurface::Cli,
        None,
        None,
        "cycle create",
        "cycle create",
        CLI_CREATE_CYCLE,
    ),
    OperationDeclaration::new(
        "api.update-cycle",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/:id/update-cycle"),
        "POST /api/v1/boards/:board/cycles/:id/update-cycle",
        "POST /api/v1/boards/:board/cycles/:id/update-cycle",
        UPDATE_CYCLE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_update",
            http_operations: &["api.get-board", "api.update-cycle"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-update",
        ContractSurface::Cli,
        None,
        None,
        "cycle update",
        "cycle update",
        CLI_UPDATE_CYCLE,
    ),
    OperationDeclaration::new(
        "api.archive-cycle",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/:id/archive-cycle"),
        "POST /api/v1/boards/:board/cycles/:id/archive-cycle",
        "POST /api/v1/boards/:board/cycles/:id/archive-cycle",
        ARCHIVE_CYCLE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_archive",
            http_operations: &["api.get-board", "api.archive-cycle"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-archive",
        ContractSurface::Cli,
        None,
        None,
        "cycle archive",
        "cycle archive",
        CLI_ARCHIVE_CYCLE,
    ),
    OperationDeclaration::new(
        "api.restore-cycle",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/:id/restore-cycle"),
        "POST /api/v1/boards/:board/cycles/:id/restore-cycle",
        "POST /api/v1/boards/:board/cycles/:id/restore-cycle",
        RESTORE_CYCLE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_restore",
            http_operations: &["api.get-board", "api.restore-cycle"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-restore",
        ContractSurface::Cli,
        None,
        None,
        "cycle restore",
        "cycle restore",
        CLI_RESTORE_CYCLE,
    ),
    OperationDeclaration::new(
        "api.get-cycle-overview",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/cycles/:id/get-cycle-overview"),
        "GET /api/v1/boards/:board/cycles/:id/get-cycle-overview",
        "GET /api/v1/boards/:board/cycles/:id/get-cycle-overview",
        GET_CYCLE_OVERVIEW,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_overview",
            http_operations: &["api.get-board", "api.get-cycle-overview"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-overview",
        ContractSurface::Cli,
        None,
        None,
        "cycle overview",
        "cycle overview",
        CLI_GET_CYCLE_OVERVIEW,
    ),
    OperationDeclaration::new(
        "api.list-cycle-tasks",
        ContractSurface::Api,
        Some(HttpMethod::Get),
        Some("/api/v1/boards/:board/cycles/:id/list-cycle-tasks"),
        "GET /api/v1/boards/:board/cycles/:id/list-cycle-tasks",
        "GET /api/v1/boards/:board/cycles/:id/list-cycle-tasks",
        LIST_CYCLE_TASKS,
    )
    .with_header_profile(ApiHeaderProfile::Locale)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_task_list",
            http_operations: &["api.get-board", "api.list-cycle-tasks"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-task-list",
        ContractSurface::Cli,
        None,
        None,
        "cycle task list",
        "cycle task list",
        CLI_LIST_CYCLE_TASKS,
    ),
    OperationDeclaration::new(
        "api.add-cycle-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/:id/tasks/:task_id/add-cycle-task"),
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/add-cycle-task",
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/add-cycle-task",
        ADD_CYCLE_TASK,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_task_add",
            http_operations: &["api.get-board", "api.list-tasks", "api.add-cycle-task"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-task-add",
        ContractSurface::Cli,
        None,
        None,
        "cycle task add",
        "cycle task add",
        CLI_ADD_CYCLE_TASK,
    ),
    OperationDeclaration::new(
        "api.remove-cycle-task",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/:id/tasks/:task_id/remove-cycle-task"),
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/remove-cycle-task",
        "POST /api/v1/boards/:board/cycles/:id/tasks/:task_id/remove-cycle-task",
        REMOVE_CYCLE_TASK,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_task_remove",
            http_operations: &["api.get-board", "api.list-tasks", "api.remove-cycle-task"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-task-remove",
        ContractSurface::Cli,
        None,
        None,
        "cycle task remove",
        "cycle task remove",
        CLI_REMOVE_CYCLE_TASK,
    ),
    OperationDeclaration::new(
        "api.start-cycle",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/:id/start-cycle"),
        "POST /api/v1/boards/:board/cycles/:id/start-cycle",
        "POST /api/v1/boards/:board/cycles/:id/start-cycle",
        START_CYCLE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_start",
            http_operations: &["api.get-board", "api.start-cycle"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-start",
        ContractSurface::Cli,
        None,
        None,
        "cycle start",
        "cycle start",
        CLI_START_CYCLE,
    ),
    OperationDeclaration::new(
        "api.close-cycle",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/:id/close-cycle"),
        "POST /api/v1/boards/:board/cycles/:id/close-cycle",
        "POST /api/v1/boards/:board/cycles/:id/close-cycle",
        CLOSE_CYCLE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_close",
            http_operations: &["api.get-board", "api.close-cycle"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-close",
        ContractSurface::Cli,
        None,
        None,
        "cycle close",
        "cycle close",
        CLI_CLOSE_CYCLE,
    ),
    OperationDeclaration::new(
        "api.cancel-cycle",
        ContractSurface::Api,
        Some(HttpMethod::Post),
        Some("/api/v1/boards/:board/cycles/:id/cancel-cycle"),
        "POST /api/v1/boards/:board/cycles/:id/cancel-cycle",
        "POST /api/v1/boards/:board/cycles/:id/cancel-cycle",
        CANCEL_CYCLE,
    )
    .with_header_profile(ApiHeaderProfile::LocaleActorJson)
    .with_mcp_policy(McpPolicy {
        exposure: McpExposure::Domain,
        tool_bindings: &[McpToolBinding {
            tool_name: "cycle_cancel",
            http_operations: &["api.get-board", "api.cancel-cycle"],
        }],
        invariants: INVARIANTS,
    }),
    OperationDeclaration::new(
        "cli.cycle-cancel",
        ContractSurface::Cli,
        None,
        None,
        "cycle cancel",
        "cycle cancel",
        CLI_CANCEL_CYCLE,
    ),
];
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    OPERATIONS
}
