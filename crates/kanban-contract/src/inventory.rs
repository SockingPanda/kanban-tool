use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractSurface {
    Api,
    Cli,
    Jsonl,
    Sse,
    Metadata,
    Config,
    Helper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractDirection {
    Serialize,
    Deserialize,
    Bidirectional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractStrictness {
    DenyUnknownFields,
    Typed,
    OpaqueExtension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractGranularity {
    Exact,
    Family,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractBinding {
    ExactSurface,
    SharedComponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpTransportLocation {
    Path,
    Query,
    Headers,
    Body,
    Success,
    Error,
    Sse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireParameterCardinality {
    RequiredOne,
    OptionalOne,
    RepeatedOrdered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WireParameter {
    pub name: &'static str,
    pub cardinality: Option<WireParameterCardinality>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContractTransport {
    NoTransport,
    Http {
        operation_key: Option<&'static str>,
        location: HttpTransportLocation,
        parameters: &'static [WireParameter],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationState {
    Planned,
    Generated,
    Adopted,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AdoptionWitness {
    pub operation: &'static str,
    pub contract_id: &'static str,
    pub surface: ContractSurface,
    pub direction: ContractDirection,
    pub package: &'static str,
    pub test_target: &'static str,
    pub exact_test: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AdoptionEvidence {
    pub producer_fixture: &'static str,
    pub producer: AdoptionWitness,
    pub consumer: AdoptionWitness,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OperationContract {
    pub id: &'static str,
    pub path: &'static str,
    pub surface: ContractSurface,
    pub operation: &'static str,
    pub direction: ContractDirection,
    pub granularity: ContractGranularity,
    pub strictness: ContractStrictness,
    pub schema_id: Option<&'static str>,
    pub fixture: Option<&'static str>,
    pub adoption: Option<AdoptionEvidence>,
    pub exclusion: Option<&'static str>,
    pub migration: MigrationState,
    pub transport: ContractTransport,
    pub binding: ContractBinding,
}

macro_rules! adopted_api_request {
    (
        $id:literal,
        $path:literal,
        $operation:literal,
        $schema_id:literal,
        $fixture:literal,
        $producer_test:literal,
        $consumer_test:literal
    ) => {
        OperationContract {
            id: $id,
            path: $path,
            surface: ContractSurface::Api,
            operation: $operation,
            direction: ContractDirection::Deserialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some($schema_id),
            fixture: Some($fixture),
            adoption: Some(AdoptionEvidence {
                producer_fixture: $fixture,
                producer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: $producer_test,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: $consumer_test,
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::Http {
                operation_key: Some($operation),
                location: HttpTransportLocation::Body,
                parameters: &[],
            },
            binding: ContractBinding::ExactSurface,
        }
    };
}

macro_rules! adopted_comment_contract {
    ($id:literal, $path:literal, $operation:literal, $direction:expr, $schema_id:literal, $fixture:literal, $location:expr, $parameters:expr, $producer:literal, $consumer:literal) => {
        OperationContract {
            id: $id,
            path: $path,
            surface: ContractSurface::Api,
            operation: $operation,
            direction: $direction,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some($schema_id),
            fixture: Some($fixture),
            adoption: Some(AdoptionEvidence {
                producer_fixture: $fixture,
                producer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: $direction,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: $producer,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: $direction,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: $consumer,
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::Http {
                operation_key: Some($operation),
                location: $location,
                parameters: $parameters,
            },
            binding: ContractBinding::ExactSurface,
        }
    };
}

macro_rules! adopted_cli_output_contract {
    ($id:literal, $operation:literal, $schema_id:literal, $fixture:literal, $test_target:literal, $producer:literal, $consumer:literal) => {
        OperationContract {
            id: $id,
            path: concat!("kanban ", $operation, " --json stdout"),
            surface: ContractSurface::Cli,
            operation: $operation,
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some($schema_id),
            fixture: Some($fixture),
            adoption: Some(AdoptionEvidence {
                producer_fixture: $fixture,
                producer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Cli,
                    direction: ContractDirection::Serialize,
                    package: "kanban-cli",
                    test_target: $test_target,
                    exact_test: $producer,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Cli,
                    direction: ContractDirection::Serialize,
                    package: "kanban-cli",
                    test_target: $test_target,
                    exact_test: $consumer,
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::NoTransport,
            binding: ContractBinding::ExactSurface,
        }
    };
}

macro_rules! adopted_cli_slug_output_contract {
    ($slug:literal, $test_slug:literal, $operation:literal, $test_target:literal) => {
        OperationContract {
            id: concat!("cli.", $slug, ".output"),
            path: concat!("kanban ", $operation, " --json stdout"),
            surface: ContractSurface::Cli,
            operation: $operation,
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some(concat!("urn:kanban-tool:schema:cli:", $slug, "-output:v1")),
            fixture: Some(concat!(
                "schemas/fixtures/cli/",
                $slug,
                "-output.v1.valid.json"
            )),
            adoption: Some(AdoptionEvidence {
                producer_fixture: concat!("schemas/fixtures/cli/", $slug, "-output.v1.valid.json"),
                producer: AdoptionWitness {
                    operation: $operation,
                    contract_id: concat!("cli.", $slug, ".output"),
                    surface: ContractSurface::Cli,
                    direction: ContractDirection::Serialize,
                    package: "kanban-cli",
                    test_target: $test_target,
                    exact_test: concat!("producer_", $test_slug, "_matches_exact_fixture"),
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: concat!("cli.", $slug, ".output"),
                    surface: ContractSurface::Cli,
                    direction: ContractDirection::Serialize,
                    package: "kanban-cli",
                    test_target: $test_target,
                    exact_test: concat!(
                        $test_slug,
                        "_output_fixture_is_consumed_by_public_contract"
                    ),
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::NoTransport,
            binding: ContractBinding::ExactSurface,
        }
    };
}

const TASK_TRANSITION_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

const COMMENT_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

const STEP_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

const STEP_ITEM_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "task_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "step_id",
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
const RUN_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const RUN_ID_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "run_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const DEPENDENCY_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const REMOVE_DEPENDENCY_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "child_task_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "parent_task_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
];
const BOARD_COLUMNS_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const CREATE_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const TASK_READ_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const TASK_CORE_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const GET_TASK_QUERY_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "include",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];

const GRAPH_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const GRAPH_BOARD_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
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

const TASK_READ_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "status",
        cardinality: Some(WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "priority",
        cardinality: Some(WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "label",
        cardinality: Some(WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "plan_filter",
        cardinality: Some(WireParameterCardinality::RepeatedOrdered),
    },
    WireParameter {
        name: "assignee",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "q",
        cardinality: Some(WireParameterCardinality::OptionalOne),
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
        name: "sort",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

macro_rules! adopted_api_parameter_contract {
    (
        $id:literal,
        $path:literal,
        $operation:literal,
        $schema_id:literal,
        $fixture:literal,
        $producer_test:literal,
        $consumer_test:literal,
        $location:expr,
        $parameters:expr
    ) => {
        OperationContract {
            id: $id,
            path: $path,
            surface: ContractSurface::Api,
            operation: $operation,
            direction: ContractDirection::Deserialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some($schema_id),
            fixture: Some($fixture),
            adoption: Some(AdoptionEvidence {
                producer_fixture: $fixture,
                producer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: $producer_test,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: $consumer_test,
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::Http {
                operation_key: Some($operation),
                location: $location,
                parameters: $parameters,
            },
            binding: ContractBinding::ExactSurface,
        }
    };
}

macro_rules! adopted_api_response_contract {
    ($id:literal, $path:literal, $operation:literal, $schema_id:literal, $fixture:literal, $producer:literal, $consumer:literal) => {
        OperationContract {
            id: $id,
            path: $path,
            surface: ContractSurface::Api,
            operation: $operation,
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some($schema_id),
            fixture: Some($fixture),
            adoption: Some(AdoptionEvidence {
                producer_fixture: $fixture,
                producer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Serialize,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: $producer,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Serialize,
                    package: "kanban-server",
                    test_target: "all",
                    exact_test: $consumer,
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::Http {
                operation_key: Some($operation),
                location: HttpTransportLocation::Success,
                parameters: &[],
            },
            binding: ContractBinding::ExactSurface,
        }
    };
}

const LIST_BOARDS_QUERY_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "include_archived",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];

const BOARD_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

macro_rules! generated_api_contract {
    ($id:literal, $path:literal, $operation:literal, $direction:expr, $schema:literal, $fixture:literal, $location:expr, $parameters:expr) => {
        OperationContract {
            id: $id,
            path: $path,
            surface: ContractSurface::Api,
            operation: $operation,
            direction: $direction,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some($schema),
            fixture: Some($fixture),
            adoption: None,
            exclusion: None,
            migration: MigrationState::Generated,
            transport: ContractTransport::Http {
                operation_key: Some($operation),
                location: $location,
                parameters: $parameters,
            },
            binding: ContractBinding::ExactSurface,
        }
    };
}

const BOARD_QUERY_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "board",
    cardinality: Some(WireParameterCardinality::OptionalOne),
}];
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
const BUILD_CONTEXT_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];
const BUILD_CONTEXT_QUERY_PARAMETERS: &[WireParameter] = &[
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
];
const LIST_EVENTS_QUERY_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "board",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "task_id",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "after",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
    WireParameter {
        name: "limit",
        cardinality: Some(WireParameterCardinality::OptionalOne),
    },
];

const OPERATION_INVENTORY: &[OperationContract] = &[
    adopted_cli_output_contract!(
        "cli.init.output",
        "init",
        "urn:kanban-tool:schema:cli:init-output:v1",
        "schemas/fixtures/cli/init-output.v1.valid.json",
        "cli_bootstrap_config_contract_adoption",
        "init_output_fixture_is_produced_by_real_cli",
        "init_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.config-show.output",
        "config show",
        "urn:kanban-tool:schema:cli:config-show-output:v1",
        "schemas/fixtures/cli/config-show-output.v1.valid.json",
        "cli_bootstrap_config_contract_adoption",
        "config_show_output_fixture_is_produced_without_creating_database",
        "config_show_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.index-status.output",
        "index status",
        "urn:kanban-tool:schema:cli:index-status-output:v1",
        "schemas/fixtures/cli/index-status-output.v1.valid.json",
        "cli_index_health_contract_adoption",
        "index_status_output_fixture_is_produced_by_real_cli",
        "index_status_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.index-doctor.output",
        "index doctor",
        "urn:kanban-tool:schema:cli:index-doctor-output:v1",
        "schemas/fixtures/cli/index-doctor-output.v1.valid.json",
        "cli_index_health_contract_adoption",
        "index_doctor_output_fixture_is_produced_by_real_cli",
        "index_doctor_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.maintenance-status.output",
        "maintenance status",
        "urn:kanban-tool:schema:cli:maintenance-status-output:v2",
        "schemas/fixtures/cli/maintenance-status-output.v2.valid.json",
        "cli_projection_maintenance_contract_adoption",
        "maintenance_status_output_fixture_is_produced_by_real_cli",
        "maintenance_status_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.maintenance-run.output",
        "maintenance run",
        "urn:kanban-tool:schema:cli:maintenance-run-output:v2",
        "schemas/fixtures/cli/maintenance-run-output.v2.valid.json",
        "cli_projection_maintenance_contract_adoption",
        "maintenance_run_output_fixture_is_produced_by_real_cli",
        "maintenance_run_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.maintenance-rebuild.output",
        "maintenance rebuild",
        "urn:kanban-tool:schema:cli:maintenance-rebuild-output:v2",
        "schemas/fixtures/cli/maintenance-rebuild-output.v2.valid.json",
        "cli_projection_maintenance_contract_adoption",
        "maintenance_rebuild_output_fixture_is_produced_by_real_cli",
        "maintenance_rebuild_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.maintenance-cleanup-legacy-inventory.output",
        "maintenance cleanup-legacy inventory",
        "urn:kanban-tool:schema:cli:maintenance-cleanup-legacy-inventory-output:v1",
        "schemas/fixtures/cli/maintenance-cleanup-legacy-inventory-output.v1.valid.json",
        "cli_projection_maintenance_contract_adoption",
        "maintenance_cleanup_legacy_inventory_output_fixture_is_produced_by_real_cli",
        "maintenance_cleanup_legacy_inventory_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.maintenance-cleanup-legacy-apply.output",
        "maintenance cleanup-legacy apply",
        "urn:kanban-tool:schema:cli:maintenance-cleanup-legacy-apply-output:v1",
        "schemas/fixtures/cli/maintenance-cleanup-legacy-apply-output.v1.valid.json",
        "cli_projection_maintenance_contract_adoption",
        "maintenance_cleanup_legacy_apply_output_fixture_is_produced_by_real_cli",
        "maintenance_cleanup_legacy_apply_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.maintenance-cleanup-legacy-verify.output",
        "maintenance cleanup-legacy verify",
        "urn:kanban-tool:schema:cli:maintenance-cleanup-legacy-verify-output:v1",
        "schemas/fixtures/cli/maintenance-cleanup-legacy-verify-output.v1.valid.json",
        "cli_projection_maintenance_contract_adoption",
        "maintenance_cleanup_legacy_verify_output_fixture_is_produced_by_real_cli",
        "maintenance_cleanup_legacy_verify_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.maintenance-cleanup-legacy-restore.output",
        "maintenance cleanup-legacy restore",
        "urn:kanban-tool:schema:cli:maintenance-cleanup-legacy-restore-output:v1",
        "schemas/fixtures/cli/maintenance-cleanup-legacy-restore-output.v1.valid.json",
        "cli_projection_maintenance_contract_adoption",
        "maintenance_cleanup_legacy_restore_output_fixture_is_produced_by_real_cli",
        "maintenance_cleanup_legacy_restore_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.derived-status.output",
        "derived status",
        "urn:kanban-tool:schema:cli:derived-status-output:v1",
        "schemas/fixtures/cli/derived-status-output.v1.valid.json",
        "cli_substrate_contract_adoption",
        "derived_status_output_fixture_proves_global_dirty_watermark",
        "derived_status_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.outbox-list.output",
        "outbox list",
        "urn:kanban-tool:schema:cli:outbox-list-output:v1",
        "schemas/fixtures/cli/outbox-list-output.v1.valid.json",
        "cli_substrate_contract_adoption",
        "outbox_list_output_fixture_is_produced_by_real_cli",
        "outbox_list_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.entity-list.output",
        "entity list",
        "urn:kanban-tool:schema:cli:entity-list-output:v1",
        "schemas/fixtures/cli/entity-list-output.v1.valid.json",
        "cli_entity_contract_adoption",
        "producer_entity_list_matches_exact_fixture_and_honors_kind_and_limit",
        "entity_list_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.entity-show.output",
        "entity show",
        "urn:kanban-tool:schema:cli:entity-show-output:v1",
        "schemas/fixtures/cli/entity-show-output.v1.valid.json",
        "cli_entity_contract_adoption",
        "producer_entity_show_matches_exact_fixture",
        "entity_show_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.doctor.output",
        "doctor",
        "urn:kanban-tool:schema:cli:doctor-output:v1",
        "schemas/fixtures/cli/doctor-output.v1.valid.json",
        "cli_diagnostics_contract_adoption",
        "doctor_output_fixture_is_produced_by_real_cli",
        "doctor_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.stats.output",
        "stats",
        "urn:kanban-tool:schema:cli:stats-output:v1",
        "schemas/fixtures/cli/stats-output.v1.valid.json",
        "cli_diagnostics_contract_adoption",
        "stats_output_fixture_is_produced_by_real_cli",
        "stats_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.backup.output",
        "backup",
        "urn:kanban-tool:schema:cli:backup-output:v1",
        "schemas/fixtures/cli/backup-output.v1.valid.json",
        "cli_maintenance_contract_adoption",
        "backup_output_fixture_is_produced_by_real_cli",
        "backup_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.checkpoint.output",
        "checkpoint",
        "urn:kanban-tool:schema:cli:checkpoint-output:v1",
        "schemas/fixtures/cli/checkpoint-output.v1.valid.json",
        "cli_maintenance_contract_adoption",
        "checkpoint_output_fixture_is_produced_by_real_cli",
        "checkpoint_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.vacuum.output",
        "vacuum",
        "urn:kanban-tool:schema:cli:vacuum-output:v1",
        "schemas/fixtures/cli/vacuum-output.v1.valid.json",
        "cli_maintenance_contract_adoption",
        "vacuum_output_fixture_is_produced_by_real_cli",
        "vacuum_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.board-list.output",
        "board list",
        "urn:kanban-tool:schema:cli:board-list-output:v1",
        "schemas/fixtures/cli/board-list-output.v1.valid.json",
        "cli_board_contract_adoption",
        "board_list_output_fixture_is_produced_by_real_cli",
        "board_list_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.board-create.output",
        "board create",
        "urn:kanban-tool:schema:cli:board-create-output:v1",
        "schemas/fixtures/cli/board-create-output.v1.valid.json",
        "cli_board_contract_adoption",
        "board_create_output_fixture_is_produced_by_real_cli",
        "board_create_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.board-show.output",
        "board show",
        "urn:kanban-tool:schema:cli:board-show-output:v1",
        "schemas/fixtures/cli/board-show-output.v1.valid.json",
        "cli_board_contract_adoption",
        "board_show_output_fixture_is_produced_by_real_cli",
        "board_show_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.board-use.output",
        "board use",
        "urn:kanban-tool:schema:cli:board-use-output:v1",
        "schemas/fixtures/cli/board-use-output.v1.valid.json",
        "cli_board_contract_adoption",
        "board_use_output_fixture_is_produced_by_real_cli",
        "board_use_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.board-current.output",
        "board current",
        "urn:kanban-tool:schema:cli:board-current-output:v1",
        "schemas/fixtures/cli/board-current-output.v1.valid.json",
        "cli_board_contract_adoption",
        "board_current_output_fixture_is_produced_by_real_cli",
        "board_current_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.board-archive.output",
        "board archive",
        "urn:kanban-tool:schema:cli:board-archive-output:v1",
        "schemas/fixtures/cli/board-archive-output.v1.valid.json",
        "cli_board_contract_adoption",
        "board_archive_output_fixture_is_produced_by_real_cli",
        "board_archive_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-list.output",
        "task list",
        "urn:kanban-tool:schema:cli:task-list-output:v1",
        "schemas/fixtures/cli/task-list-output.v1.valid.json",
        "cli_task_read_contract_adoption",
        "task_list_output_fixture_is_produced_by_real_cli",
        "task_list_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-show.output",
        "task show",
        "urn:kanban-tool:schema:cli:task-show-output:v1",
        "schemas/fixtures/cli/task-show-output.v1.valid.json",
        "cli_task_read_contract_adoption",
        "task_show_output_fixture_is_produced_by_real_cli",
        "task_show_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.comment-add.output",
        "comment add",
        "urn:kanban-tool:schema:cli:comment-add-output:v1",
        "schemas/fixtures/cli/comment-add-output.v1.valid.json",
        "cli_relation_mutation_contract_adoption",
        "comment_add_output_fixture_is_produced_by_real_cli",
        "comment_add_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.comment-list.output",
        "comment list",
        "urn:kanban-tool:schema:cli:comment-list-output:v1",
        "schemas/fixtures/cli/comment-list-output.v1.valid.json",
        "cli_core_read_contract_adoption",
        "comment_list_output_fixture_is_produced_by_real_cli",
        "comment_list_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.dep-add.output",
        "dep add",
        "urn:kanban-tool:schema:cli:dep-add-output:v1",
        "schemas/fixtures/cli/dep-add-output.v1.valid.json",
        "cli_relation_mutation_contract_adoption",
        "dependency_add_output_fixture_is_produced_by_real_cli",
        "dependency_add_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.dep-list.output",
        "dep list",
        "urn:kanban-tool:schema:cli:dep-list-output:v1",
        "schemas/fixtures/cli/dep-list-output.v1.valid.json",
        "cli_core_read_contract_adoption",
        "dependency_list_output_fixture_is_produced_by_real_cli",
        "dependency_list_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.dep-remove.output",
        "dep remove",
        "urn:kanban-tool:schema:cli:dep-remove-output:v1",
        "schemas/fixtures/cli/dep-remove-output.v1.valid.json",
        "cli_relation_mutation_contract_adoption",
        "dependency_remove_output_fixture_is_produced_by_real_cli",
        "dependency_remove_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_cli_output_contract!(
        "cli.events.output",
        "events",
        "urn:kanban-tool:schema:cli:events-output:v1",
        "schemas/fixtures/cli/events-output.v1.valid.json",
        "cli_core_read_contract_adoption",
        "events_output_fixture_is_produced_by_real_cli",
        "events_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-step-list.output",
        "task step list",
        "urn:kanban-tool:schema:cli:task-step-list-output:v1",
        "schemas/fixtures/cli/task-step-list-output.v1.valid.json",
        "cli_step_read_contract_adoption",
        "task_step_list_output_fixture_is_produced_by_real_cli",
        "task_step_list_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-step-add.output",
        "task step add",
        "urn:kanban-tool:schema:cli:task-step-add-output:v1",
        "schemas/fixtures/cli/task-step-add-output.v1.valid.json",
        "cli_step_mutation_contract_adoption",
        "task_step_add_output_fixture_is_produced_by_real_cli",
        "task_step_add_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-step-update.output",
        "task step update",
        "urn:kanban-tool:schema:cli:task-step-update-output:v1",
        "schemas/fixtures/cli/task-step-update-output.v1.valid.json",
        "cli_step_mutation_contract_adoption",
        "task_step_update_output_fixture_is_produced_by_real_cli",
        "task_step_update_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-step-done.output",
        "task step done",
        "urn:kanban-tool:schema:cli:task-step-done-output:v1",
        "schemas/fixtures/cli/task-step-done-output.v1.valid.json",
        "cli_step_mutation_contract_adoption",
        "task_step_done_output_fixture_is_produced_by_real_cli",
        "task_step_done_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-step-skip.output",
        "task step skip",
        "urn:kanban-tool:schema:cli:task-step-skip-output:v1",
        "schemas/fixtures/cli/task-step-skip-output.v1.valid.json",
        "cli_step_mutation_contract_adoption",
        "task_step_skip_output_fixture_is_produced_by_real_cli",
        "task_step_skip_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-step-reopen.output",
        "task step reopen",
        "urn:kanban-tool:schema:cli:task-step-reopen-output:v1",
        "schemas/fixtures/cli/task-step-reopen-output.v1.valid.json",
        "cli_step_mutation_contract_adoption",
        "task_step_reopen_output_fixture_is_produced_by_real_cli",
        "task_step_reopen_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-step-remove.output",
        "task step remove",
        "urn:kanban-tool:schema:cli:task-step-remove-output:v1",
        "schemas/fixtures/cli/task-step-remove-output.v1.valid.json",
        "cli_step_mutation_contract_adoption",
        "task_step_remove_output_fixture_is_produced_by_real_cli",
        "task_step_remove_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-step-not-required.output",
        "task step not-required",
        "urn:kanban-tool:schema:cli:task-step-not-required-output:v1",
        "schemas/fixtures/cli/task-step-not-required-output.v1.valid.json",
        "cli_step_mutation_contract_adoption",
        "task_step_not_required_output_fixture_is_produced_by_real_cli",
        "task_step_not_required_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.runs.output",
        "runs",
        "urn:kanban-tool:schema:cli:runs-output:v1",
        "schemas/fixtures/cli/runs-output.v1.valid.json",
        "cli_run_read_contract_adoption",
        "runs_output_fixture_is_produced_by_real_cli",
        "runs_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.run-show.output",
        "run show",
        "urn:kanban-tool:schema:cli:run-show-output:v1",
        "schemas/fixtures/cli/run-show-output.v1.valid.json",
        "cli_run_read_contract_adoption",
        "run_show_output_fixture_is_produced_by_real_cli",
        "run_show_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.run-logs.output",
        "run logs",
        "urn:kanban-tool:schema:cli:run-logs-output:v1",
        "schemas/fixtures/cli/run-logs-output.v1.valid.json",
        "cli_run_read_contract_adoption",
        "run_logs_output_fixture_is_produced_by_real_cli",
        "run_logs_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-create.output",
        "task create",
        "urn:kanban-tool:schema:cli:task-create-output:v1",
        "schemas/fixtures/cli/task-create-output.v1.valid.json",
        "cli_task_mutation_contract_adoption",
        "task_create_output_fixture_is_produced_by_real_cli",
        "task_create_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-update.output",
        "task update",
        "urn:kanban-tool:schema:cli:task-update-output:v1",
        "schemas/fixtures/cli/task-update-output.v1.valid.json",
        "cli_task_mutation_contract_adoption",
        "task_update_output_fixture_is_produced_by_real_cli",
        "task_update_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-claim.output",
        "task claim",
        "urn:kanban-tool:schema:cli:task-claim-output:v1",
        "schemas/fixtures/cli/task-claim-output.v1.valid.json",
        "cli_task_claim_contract_adoption",
        "task_claim_output_fixture_is_produced_by_real_cli",
        "task_claim_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-start.output",
        "task start",
        "urn:kanban-tool:schema:cli:task-start-output:v1",
        "schemas/fixtures/cli/task-start-output.v1.valid.json",
        "cli_task_claim_contract_adoption",
        "task_start_output_fixture_is_produced_by_real_cli",
        "task_start_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-reclaim.output",
        "task reclaim",
        "urn:kanban-tool:schema:cli:task-reclaim-output:v1",
        "schemas/fixtures/cli/task-reclaim-output.v1.valid.json",
        "cli_task_claim_contract_adoption",
        "task_reclaim_output_fixture_is_produced_by_real_cli",
        "task_reclaim_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-promote.output",
        "task promote",
        "urn:kanban-tool:schema:cli:task-promote-output:v1",
        "schemas/fixtures/cli/task-promote-output.v1.valid.json",
        "cli_task_lifecycle_contract_adoption",
        "task_promote_output_fixture_is_produced_by_real_cli",
        "task_promote_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-reopen.output",
        "task reopen",
        "urn:kanban-tool:schema:cli:task-reopen-output:v1",
        "schemas/fixtures/cli/task-reopen-output.v1.valid.json",
        "cli_task_lifecycle_contract_adoption",
        "task_reopen_output_fixture_is_produced_by_real_cli",
        "task_reopen_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-heartbeat.output",
        "task heartbeat",
        "urn:kanban-tool:schema:cli:task-heartbeat-output:v1",
        "schemas/fixtures/cli/task-heartbeat-output.v1.valid.json",
        "cli_task_lifecycle_contract_adoption",
        "task_heartbeat_output_fixture_is_produced_by_real_cli",
        "task_heartbeat_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-release.output",
        "task release",
        "urn:kanban-tool:schema:cli:task-release-output:v1",
        "schemas/fixtures/cli/task-release-output.v1.valid.json",
        "cli_task_release_contract_adoption",
        "task_release_output_fixture_is_produced_by_real_cli",
        "task_release_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-done.output",
        "task done",
        "urn:kanban-tool:schema:cli:task-done-output:v1",
        "schemas/fixtures/cli/task-done-output.v1.valid.json",
        "all",
        "task_done_output_contract",
        "task_done_output_contract"
    ),
    adopted_cli_output_contract!(
        "cli.task-complete.output",
        "task complete",
        "urn:kanban-tool:schema:cli:task-complete-output:v1",
        "schemas/fixtures/cli/task-complete-output.v1.valid.json",
        "all",
        "task_complete_output_contract",
        "task_complete_output_contract"
    ),
    adopted_cli_output_contract!(
        "cli.task-review.output",
        "task review",
        "urn:kanban-tool:schema:cli:task-review-output:v1",
        "schemas/fixtures/cli/task-review-output.v1.valid.json",
        "cli_task_lifecycle_contract_adoption",
        "task_review_output_fixture_is_produced_by_real_cli",
        "task_review_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-block.output",
        "task block",
        "urn:kanban-tool:schema:cli:task-block-output:v1",
        "schemas/fixtures/cli/task-block-output.v1.valid.json",
        "all",
        "task_block_output_contract",
        "task_block_output_contract"
    ),
    adopted_cli_output_contract!(
        "cli.task-unblock.output",
        "task unblock",
        "urn:kanban-tool:schema:cli:task-unblock-output:v1",
        "schemas/fixtures/cli/task-unblock-output.v1.valid.json",
        "cli_task_lifecycle_contract_adoption",
        "task_unblock_output_fixture_is_produced_by_real_cli",
        "task_unblock_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_output_contract!(
        "cli.task-archive.output",
        "task archive",
        "urn:kanban-tool:schema:cli:task-archive-output:v1",
        "schemas/fixtures/cli/task-archive-output.v1.valid.json",
        "cli_task_lifecycle_contract_adoption",
        "task_archive_output_fixture_is_produced_by_real_cli",
        "task_archive_output_fixture_is_consumed_by_contract_root"
    ),
    adopted_cli_slug_output_contract!(
        "label-add",
        "label_add",
        "label add",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-atom-index-query",
        "label_atom_index_query",
        "label atom-index query",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-atom-index-rebuild",
        "label_atom_index_rebuild",
        "label atom-index rebuild",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-atom-index-status",
        "label_atom_index_status",
        "label atom-index status",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-atoms-explain",
        "label_atoms_explain",
        "label atoms explain",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-atoms-list",
        "label_atoms_list",
        "label atoms list",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-bootstrap",
        "label_bootstrap",
        "label bootstrap",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-create",
        "label_create",
        "label create",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-delete",
        "label_delete",
        "label delete",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-list",
        "label_list",
        "label list",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-apply-atom",
        "label_ontology_apply_atom",
        "label ontology apply atom",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-confirm",
        "label_ontology_confirm",
        "label ontology confirm",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-list",
        "label_ontology_list",
        "label ontology list",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-quality",
        "label_ontology_quality",
        "label ontology quality",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-record",
        "label_ontology_record",
        "label ontology record",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-reject",
        "label_ontology_reject",
        "label ontology reject",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-resolve",
        "label_ontology_resolve",
        "label ontology resolve",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-revert",
        "label_ontology_revert",
        "label ontology revert",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-review",
        "label_ontology_review",
        "label ontology review",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-show",
        "label_ontology_show",
        "label ontology show",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-supersede",
        "label_ontology_supersede",
        "label ontology supersede",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-ontology-validate",
        "label_ontology_validate",
        "label ontology validate",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-proposals-accept",
        "label_proposals_accept",
        "label proposals accept",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-proposals-list",
        "label_proposals_list",
        "label proposals list",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-proposals-reject",
        "label_proposals_reject",
        "label proposals reject",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-proposals-show",
        "label_proposals_show",
        "label proposals show",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-propose",
        "label_propose",
        "label propose",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-remove",
        "label_remove",
        "label remove",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-semantics-delete",
        "label_semantics_delete",
        "label semantics delete",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-semantics-list",
        "label_semantics_list",
        "label semantics list",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-semantics-show",
        "label_semantics_show",
        "label semantics show",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-semantics-upsert",
        "label_semantics_upsert",
        "label semantics upsert",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "label-suggest",
        "label_suggest",
        "label suggest",
        "cli_label_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "graph-neighbors",
        "graph_neighbors",
        "graph neighbors",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "graph-query",
        "graph_query",
        "graph query",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "graph-rebuild",
        "graph_rebuild",
        "graph rebuild",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "graph-status",
        "graph_status",
        "graph status",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "graph-sync",
        "graph_sync",
        "graph sync",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "vector-configure",
        "vector_configure",
        "vector configure",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "vector-query-chunks",
        "vector_query_chunks",
        "vector query-chunks",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "vector-query-label-atoms",
        "vector_query_label_atoms",
        "vector query-label-atoms",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "vector-rebuild",
        "vector_rebuild",
        "vector rebuild",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "vector-status",
        "vector_status",
        "vector status",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "vector-sync",
        "vector_sync",
        "vector sync",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "context-build",
        "context_build",
        "context build",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!("search", "search", "search", "cli_helper_contract_adoption"),
    adopted_cli_slug_output_contract!(
        "index-rebuild",
        "index_rebuild",
        "index rebuild",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "index-sync",
        "index_sync",
        "index sync",
        "cli_helper_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "signal-confirm",
        "signal_confirm",
        "signal confirm",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "signal-list",
        "signal_list",
        "signal list",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "signal-record",
        "signal_record",
        "signal record",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "signal-reject",
        "signal_reject",
        "signal reject",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "signal-resolve",
        "signal_resolve",
        "signal resolve",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "signal-review",
        "signal_review",
        "signal review",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "signal-show",
        "signal_show",
        "signal show",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "signal-supersede",
        "signal_supersede",
        "signal supersede",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "hook-codex-install",
        "hook_codex_install",
        "hook codex install",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "hook-codex-status",
        "hook_codex_status",
        "hook codex status",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "hook-codex-uninstall",
        "hook_codex_uninstall",
        "hook codex uninstall",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "dispatch",
        "dispatch",
        "dispatch",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_slug_output_contract!(
        "export",
        "export",
        "export",
        "cli_operator_contract_adoption"
    ),
    adopted_cli_output_contract!(
        "cli.import.output",
        "import",
        "urn:kanban-tool:schema:cli:import-output:v2",
        "schemas/fixtures/cli/import-output.v2.valid.json",
        "cli_operator_contract_adoption",
        "import_output_fixture_is_produced_by_real_cli",
        "import_output_fixture_is_consumed_by_public_contract"
    ),
    adopted_api_parameter_contract!(
        "api.get-stats.query",
        "GET /api/v1/stats query",
        "GET /api/v1/stats",
        "urn:kanban-tool:schema:api:get-stats-query:v1",
        "schemas/fixtures/api/get-stats-query.v1.valid.json",
        "suite::derived_adoption::get_stats_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::get_stats_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.get-stats.response",
        "GET /api/v1/stats response",
        "GET /api/v1/stats",
        "urn:kanban-tool:schema:api:get-stats-response:v1",
        "schemas/fixtures/api/get-stats-response.v1.valid.json",
        "suite::derived_adoption::get_stats_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::get_stats_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.search-tasks.query",
        "GET /api/v1/search/tasks query",
        "GET /api/v1/search/tasks",
        "urn:kanban-tool:schema:api:search-tasks-query:v1",
        "schemas/fixtures/api/search-tasks-query.v1.valid.json",
        "suite::derived_adoption::search_tasks_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::search_tasks_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        SEARCH_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.search-tasks.response",
        "GET /api/v1/search/tasks response",
        "GET /api/v1/search/tasks",
        "urn:kanban-tool:schema:api:search-tasks-response:v1",
        "schemas/fixtures/api/search-tasks-response.v1.valid.json",
        "suite::derived_adoption::search_tasks_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::search_tasks_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.search-tasks-by-status.query",
        "GET /api/v1/search/tasks/by-status query",
        "GET /api/v1/search/tasks/by-status",
        "urn:kanban-tool:schema:api:search-tasks-by-status-query:v1",
        "schemas/fixtures/api/search-tasks-by-status-query.v1.valid.json",
        "suite::derived_adoption::search_tasks_by_status_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::search_tasks_by_status_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        SEARCH_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.search-tasks-by-status.response",
        "GET /api/v1/search/tasks/by-status response",
        "GET /api/v1/search/tasks/by-status",
        "urn:kanban-tool:schema:api:search-tasks-by-status-response:v1",
        "schemas/fixtures/api/search-tasks-by-status-response.v1.valid.json",
        "suite::derived_adoption::search_tasks_by_status_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::search_tasks_by_status_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.search-status.query",
        "GET /api/v1/search/status query",
        "GET /api/v1/search/status",
        "urn:kanban-tool:schema:api:search-status-query:v1",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "suite::derived_adoption::search_status_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::search_status_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.search-status.response",
        "GET /api/v1/search/status response",
        "GET /api/v1/search/status",
        "urn:kanban-tool:schema:api:search-status-response:v1",
        "schemas/fixtures/api/search-status-response.v1.valid.json",
        "suite::derived_adoption::search_status_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::search_status_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.build-context.path",
        "GET /api/v1/tasks/:task_id/context path",
        "GET /api/v1/tasks/:task_id/context",
        "urn:kanban-tool:schema:api:build-context-path:v1",
        "schemas/fixtures/api/build-context-path.v1.valid.json",
        "suite::derived_adoption::build_context_path_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::build_context_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        BUILD_CONTEXT_PATH_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.build-context.query",
        "GET /api/v1/tasks/:task_id/context query",
        "GET /api/v1/tasks/:task_id/context",
        "urn:kanban-tool:schema:api:build-context-query:v1",
        "schemas/fixtures/api/build-context-query.v1.valid.json",
        "suite::derived_adoption::build_context_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::build_context_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BUILD_CONTEXT_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.build-context.response",
        "GET /api/v1/tasks/:task_id/context response",
        "GET /api/v1/tasks/:task_id/context",
        "urn:kanban-tool:schema:api:build-context-response:v1",
        "schemas/fixtures/api/build-context-response.v1.valid.json",
        "suite::derived_adoption::build_context_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::build_context_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.graph-status.query",
        "GET /api/v1/graph/status query",
        "GET /api/v1/graph/status",
        "urn:kanban-tool:schema:api:graph-status-query:v1",
        "schemas/fixtures/api/graph-status-query.v1.valid.json",
        "suite::derived_adoption::graph_status_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::graph_status_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.graph-status.response",
        "GET /api/v1/graph/status response",
        "GET /api/v1/graph/status",
        "urn:kanban-tool:schema:api:graph-status-response:v1",
        "schemas/fixtures/api/graph-status-response.v1.valid.json",
        "suite::derived_adoption::graph_status_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::graph_status_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.graph-neighbors.query",
        "GET /api/v1/graph/neighbors query",
        "GET /api/v1/graph/neighbors",
        "urn:kanban-tool:schema:api:graph-neighbors-query:v1",
        "schemas/fixtures/api/graph-neighbors-query.v1.valid.json",
        "suite::derived_adoption::graph_neighbors_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::graph_neighbors_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        GRAPH_NEIGHBORS_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.graph-neighbors.response",
        "GET /api/v1/graph/neighbors response",
        "GET /api/v1/graph/neighbors",
        "urn:kanban-tool:schema:api:graph-neighbors-response:v1",
        "schemas/fixtures/api/graph-neighbors-response.v1.valid.json",
        "suite::derived_adoption::graph_neighbors_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::graph_neighbors_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.vector-status.query",
        "GET /api/v1/vector/status query",
        "GET /api/v1/vector/status",
        "urn:kanban-tool:schema:api:vector-status-query:v1",
        "schemas/fixtures/api/vector-status-query.v1.valid.json",
        "suite::derived_adoption::vector_status_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::vector_status_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.vector-status.response",
        "GET /api/v1/vector/status response",
        "GET /api/v1/vector/status",
        "urn:kanban-tool:schema:api:vector-status-response:v1",
        "schemas/fixtures/api/vector-status-response.v1.valid.json",
        "suite::derived_adoption::vector_status_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::vector_status_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.list-events.query",
        "GET /api/v1/events query",
        "GET /api/v1/events",
        "urn:kanban-tool:schema:api:list-events-query:v1",
        "schemas/fixtures/api/list-events-query.v1.valid.json",
        "suite::derived_adoption::list_events_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::list_events_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        LIST_EVENTS_QUERY_PARAMETERS
    ),
    adopted_comment_contract!(
        "api.list-dependencies.path",
        "GET /api/v1/tasks/:task_id/dependencies path",
        "GET /api/v1/tasks/:task_id/dependencies",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-dependencies-path:v1",
        "schemas/fixtures/api/list-dependencies-path.v1.valid.json",
        HttpTransportLocation::Path,
        DEPENDENCY_TASK_PATH_PARAMETERS,
        "suite::execution_adoption::list_dependencies_path_dto_serializes_to_committed_fixture",
        "suite::execution_adoption::list_dependencies_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.list-dependencies.response",
        "GET /api/v1/tasks/:task_id/dependencies response",
        "GET /api/v1/tasks/:task_id/dependencies",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-dependencies-response:v1",
        "schemas/fixtures/api/list-dependencies-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::execution_adoption::list_dependencies_response_fixture_is_produced_by_real_router",
        "suite::execution_adoption::list_dependencies_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.add-dependency.path",
        "POST /api/v1/tasks/:task_id/dependencies path",
        "POST /api/v1/tasks/:task_id/dependencies",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:add-dependency-path:v1",
        "schemas/fixtures/api/add-dependency-path.v1.valid.json",
        HttpTransportLocation::Path,
        DEPENDENCY_TASK_PATH_PARAMETERS,
        "suite::execution_adoption::add_dependency_path_dto_serializes_to_committed_fixture",
        "suite::execution_adoption::add_dependency_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.add-dependency.response",
        "POST /api/v1/tasks/:task_id/dependencies response",
        "POST /api/v1/tasks/:task_id/dependencies",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:add-dependency-response:v1",
        "schemas/fixtures/api/add-dependency-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::execution_adoption::add_dependency_response_fixture_is_produced_by_real_router",
        "suite::execution_adoption::add_dependency_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.remove-dependency.path",
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id path",
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:remove-dependency-path:v1",
        "schemas/fixtures/api/remove-dependency-path.v1.valid.json",
        HttpTransportLocation::Path,
        REMOVE_DEPENDENCY_PATH_PARAMETERS,
        "suite::execution_adoption::remove_dependency_path_dto_serializes_to_committed_fixture",
        "suite::execution_adoption::remove_dependency_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.remove-dependency.response",
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id response",
        "DELETE /api/v1/tasks/:child_task_id/dependencies/:parent_task_id",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:remove-dependency-response:v1",
        "schemas/fixtures/api/remove-dependency-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::execution_adoption::remove_dependency_response_fixture_is_produced_by_real_router",
        "suite::execution_adoption::remove_dependency_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.mark-execution-plan-not-required.path",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required path",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-path:v1",
        "schemas/fixtures/api/mark-execution-plan-not-required-path.v1.valid.json",
        HttpTransportLocation::Path,
        DEPENDENCY_TASK_PATH_PARAMETERS,
        "suite::execution_adoption::mark_plan_path_dto_serializes_to_committed_fixture",
        "suite::execution_adoption::mark_plan_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.mark-execution-plan-not-required.request",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required body",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-request:v1",
        "schemas/fixtures/api/mark-execution-plan-not-required-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::execution_adoption::mark_plan_request_dto_serializes_to_committed_fixture",
        "suite::execution_adoption::mark_plan_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.mark-execution-plan-not-required.response",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required response",
        "POST /api/v1/tasks/:task_id/execution-plan/not-required",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:mark-execution-plan-not-required-response:v1",
        "schemas/fixtures/api/mark-execution-plan-not-required-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::execution_adoption::mark_plan_response_fixture_is_produced_by_real_router",
        "suite::execution_adoption::mark_plan_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.get-run-log.path",
        "GET /api/v1/runs/:run_id/log path",
        "GET /api/v1/runs/:run_id/log",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:get-run-log-path:v1",
        "schemas/fixtures/api/get-run-log-path.v1.valid.json",
        HttpTransportLocation::Path,
        RUN_ID_PATH_PARAMETERS,
        "suite::execution_adoption::get_run_log_path_dto_serializes_to_committed_fixture",
        "suite::execution_adoption::get_run_log_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.get-run-log.response",
        "GET /api/v1/runs/:run_id/log response",
        "GET /api/v1/runs/:run_id/log",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:get-run-log-response:v1",
        "schemas/fixtures/api/get-run-log-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::execution_adoption::get_run_log_response_fixture_is_produced_by_real_router",
        "suite::execution_adoption::get_run_log_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.list-board-columns.path",
        "GET /api/v1/boards/:board/columns path",
        "GET /api/v1/boards/:board/columns",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-board-columns-path:v1",
        "schemas/fixtures/api/list-board-columns-path.v1.valid.json",
        HttpTransportLocation::Path,
        BOARD_COLUMNS_PATH_PARAMETERS,
        "suite::execution_adoption::list_board_columns_path_dto_serializes_to_committed_fixture",
        "suite::execution_adoption::list_board_columns_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.list-board-columns.response",
        "GET /api/v1/boards/:board/columns response",
        "GET /api/v1/boards/:board/columns",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-board-columns-response:v1",
        "schemas/fixtures/api/list-board-columns-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::execution_adoption::list_board_columns_response_fixture_is_produced_by_real_router",
        "suite::execution_adoption::list_board_columns_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_response_contract!(
        "api.doctor.response",
        "POST /api/v1/maintenance/doctor response",
        "POST /api/v1/maintenance/doctor",
        "urn:kanban-tool:schema:api:doctor-response:v1",
        "schemas/fixtures/api/doctor-response.v1.valid.json",
        "suite::maintenance_adoption::doctor_response_maps_real_non_default_report_before_fixture_normalization",
        "suite::maintenance_adoption::doctor_response_contract_consumes_producer_fixture"
    ),
    adopted_api_response_contract!(
        "api.checkpoint.response",
        "POST /api/v1/maintenance/checkpoint response",
        "POST /api/v1/maintenance/checkpoint",
        "urn:kanban-tool:schema:api:checkpoint-response:v1",
        "schemas/fixtures/api/checkpoint-response.v1.valid.json",
        "suite::maintenance_adoption::checkpoint_response_reports_real_wal_field_relationships",
        "suite::maintenance_adoption::checkpoint_response_contract_consumes_producer_fixture"
    ),
    OperationContract {
        id: "api.health.response",
        path: "GET /health response",
        surface: ContractSurface::Api,
        operation: "localhost health report",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: Some("urn:kanban-tool:schema:api:health-response:v1"),
        fixture: Some("schemas/fixtures/api/health-response.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/api/health-response.v1.valid.json",
            producer: AdoptionWitness {
                operation: "GET /health",
                contract_id: "api.health.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::health::health_response_fixture_is_produced_by_real_router",
            },
            consumer: AdoptionWitness {
                operation: "GET /health",
                contract_id: "api.health.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::health::health_response_contract_consumes_producer_fixture",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::Http {
            operation_key: Some("GET /health"),
            location: HttpTransportLocation::Success,
            parameters: &[],
        },
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "api.error.response",
        path: "Shared API error response component",
        surface: ContractSurface::Api,
        operation: "stable API error envelope",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: Some("urn:kanban-tool:schema:api:error-response:v1"),
        fixture: Some("schemas/fixtures/api/error-response.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/api/error-response.v1.valid.json",
            producer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks",
                contract_id: "api.error.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::errors::api_error_response_contract_produces_fixture",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks",
                contract_id: "api.error.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::errors::api_error_response_contract_consumes_fixture",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::Http {
            operation_key: None,
            location: HttpTransportLocation::Error,
            parameters: &[],
        },
        binding: ContractBinding::SharedComponent,
    },
    adopted_api_parameter_contract!(
        "api.list-boards.query",
        "GET /api/v1/boards query",
        "GET /api/v1/boards",
        "urn:kanban-tool:schema:api:list-boards-query:v1",
        "schemas/fixtures/api/list-boards-query.v1.valid.json",
        "suite::boards_adoption::list_boards_query_dto_serializes_to_committed_fixture",
        "suite::boards_adoption::list_boards_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        LIST_BOARDS_QUERY_PARAMETERS
    ),
    adopted_api_request!(
        "api.create-board.request",
        "POST /api/v1/boards request",
        "POST /api/v1/boards",
        "urn:kanban-tool:schema:api:create-board-request:v1",
        "schemas/fixtures/api/create-board-request.v1.valid.json",
        "suite::boards_adoption::create_board_request_dto_serializes_to_committed_fixture",
        "suite::boards_adoption::create_board_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_parameter_contract!(
        "api.get-board.path",
        "GET /api/v1/boards/:board path",
        "GET /api/v1/boards/:board",
        "urn:kanban-tool:schema:api:get-board-path:v1",
        "schemas/fixtures/api/get-board-path.v1.valid.json",
        "suite::boards_adoption::get_board_path_dto_serializes_to_committed_fixture",
        "suite::boards_adoption::get_board_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.archive-board.path",
        "POST /api/v1/boards/:board/archive path",
        "POST /api/v1/boards/:board/archive",
        "urn:kanban-tool:schema:api:archive-board-path:v1",
        "schemas/fixtures/api/archive-board-path.v1.valid.json",
        "suite::boards_adoption::archive_board_path_dto_serializes_to_committed_fixture",
        "suite::boards_adoption::archive_board_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.list-boards.response",
        "GET /api/v1/boards response",
        "GET /api/v1/boards",
        "urn:kanban-tool:schema:api:list-boards-response:v1",
        "schemas/fixtures/api/list-boards-response.v1.valid.json",
        "suite::boards_adoption::list_boards_response_fixture_is_produced_by_real_router",
        "suite::boards_adoption::list_boards_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_response_contract!(
        "api.create-board.response",
        "POST /api/v1/boards response",
        "POST /api/v1/boards",
        "urn:kanban-tool:schema:api:create-board-response:v1",
        "schemas/fixtures/api/create-board-response.v1.valid.json",
        "suite::boards_adoption::create_board_response_fixture_is_produced_by_real_router",
        "suite::boards_adoption::create_board_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_response_contract!(
        "api.get-board.response",
        "GET /api/v1/boards/:board response",
        "GET /api/v1/boards/:board",
        "urn:kanban-tool:schema:api:get-board-response:v1",
        "schemas/fixtures/api/get-board-response.v1.valid.json",
        "suite::boards_adoption::get_board_response_fixture_is_produced_by_real_router",
        "suite::boards_adoption::get_board_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_response_contract!(
        "api.archive-board.response",
        "POST /api/v1/boards/:board/archive response",
        "POST /api/v1/boards/:board/archive",
        "urn:kanban-tool:schema:api:archive-board-response:v1",
        "schemas/fixtures/api/archive-board-response.v1.valid.json",
        "suite::boards_adoption::archive_board_response_fixture_is_produced_by_real_router",
        "suite::boards_adoption::archive_board_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.list-tasks.path",
        "GET /api/v1/boards/:board/tasks path",
        "GET /api/v1/boards/:board/tasks",
        "urn:kanban-tool:schema:api:list-tasks-path:v1",
        "schemas/fixtures/api/list-tasks-path.v1.valid.json",
        "suite::task_read_request_adoption::list_tasks_path_dto_serializes_to_committed_fixture",
        "suite::task_read_request_adoption::list_tasks_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        TASK_READ_PATH_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.list-tasks.query",
        "GET /api/v1/boards/:board/tasks query",
        "GET /api/v1/boards/:board/tasks",
        "urn:kanban-tool:schema:api:list-tasks-query:v1",
        "schemas/fixtures/api/list-tasks-query.v1.valid.json",
        "suite::task_read_request_adoption::list_tasks_query_dto_serializes_to_committed_fixture",
        "suite::task_read_request_adoption::list_tasks_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        TASK_READ_QUERY_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.list-tasks-by-status.path",
        "GET /api/v1/boards/:board/tasks/by-status path",
        "GET /api/v1/boards/:board/tasks/by-status",
        "urn:kanban-tool:schema:api:list-tasks-by-status-path:v1",
        "schemas/fixtures/api/list-tasks-by-status-path.v1.valid.json",
        "suite::task_read_request_adoption::list_tasks_by_status_path_dto_serializes_to_committed_fixture",
        "suite::task_read_request_adoption::list_tasks_by_status_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        TASK_READ_PATH_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.list-tasks-by-status.query",
        "GET /api/v1/boards/:board/tasks/by-status query",
        "GET /api/v1/boards/:board/tasks/by-status",
        "urn:kanban-tool:schema:api:list-tasks-by-status-query:v1",
        "schemas/fixtures/api/list-tasks-by-status-query.v1.valid.json",
        "suite::task_read_request_adoption::list_tasks_by_status_query_dto_serializes_to_committed_fixture",
        "suite::task_read_request_adoption::list_tasks_by_status_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        TASK_READ_QUERY_PARAMETERS
    ),
    OperationContract {
        id: "api.list-tasks.response",
        path: "GET /api/v1/boards/:board/tasks response",
        surface: ContractSurface::Api,
        operation: "GET /api/v1/boards/:board/tasks",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: Some("urn:kanban-tool:schema:api:list-tasks-response:v1"),
        fixture: Some("schemas/fixtures/api/list-tasks-response.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/api/list-tasks-response.v1.valid.json",
            producer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks",
                contract_id: "api.list-tasks.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::api_task_component::list_tasks_response_producer_fixture",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks",
                contract_id: "api.list-tasks.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::api_task_component::list_tasks_response_consumer_fixture",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::Http {
            operation_key: Some("GET /api/v1/boards/:board/tasks"),
            location: HttpTransportLocation::Success,
            parameters: &[],
        },
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "api.list-tasks-by-status.response",
        path: "GET /api/v1/boards/:board/tasks/by-status response",
        surface: ContractSurface::Api,
        operation: "GET /api/v1/boards/:board/tasks/by-status",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: Some("urn:kanban-tool:schema:api:list-tasks-by-status-response:v1"),
        fixture: Some("schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json",
            producer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks/by-status",
                contract_id: "api.list-tasks-by-status.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::api_task_component::list_tasks_by_status_response_producer_fixture",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks/by-status",
                contract_id: "api.list-tasks-by-status.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::api_task_component::list_tasks_by_status_response_consumer_fixture",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::Http {
            operation_key: Some("GET /api/v1/boards/:board/tasks/by-status"),
            location: HttpTransportLocation::Success,
            parameters: &[],
        },
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "api.label-semantics-delete.response",
        path: "DELETE /api/v1/boards/:board/labels/:label_id/semantics response",
        surface: ContractSurface::Api,
        operation: "label semantics deletion acknowledgement",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: Some("urn:kanban-tool:schema:api:delete-response:v1"),
        fixture: Some("schemas/fixtures/api/delete-response.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/api/delete-response.v1.valid.json",
            producer: AdoptionWitness {
                operation: "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
                contract_id: "api.label-semantics-delete.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::delete_adoption::delete_label_semantics_response_fixture_is_produced_by_real_router",
            },
            consumer: AdoptionWitness {
                operation: "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
                contract_id: "api.label-semantics-delete.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::delete_adoption::delete_label_semantics_response_fixture_is_consumed_by_contract_root",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::Http {
            operation_key: Some("DELETE /api/v1/boards/:board/labels/:label_id/semantics"),
            location: HttpTransportLocation::Success,
            parameters: &[],
        },
        binding: ContractBinding::ExactSurface,
    },
    adopted_comment_contract!(
        "api.specify-task.path",
        "POST /api/v1/tasks/:task_id/transitions/specify path",
        "POST /api/v1/tasks/:task_id/transitions/specify",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:specify-task-path:v1",
        "schemas/fixtures/api/specify-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "suite::transitions_adoption::specify_task_path_dto_serializes_to_committed_fixture",
        "suite::transitions_adoption::specify_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.specify-task.response",
        "POST /api/v1/tasks/:task_id/transitions/specify response",
        "POST /api/v1/tasks/:task_id/transitions/specify",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:specify-task-response:v1",
        "schemas/fixtures/api/specify-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::transitions_adoption::specify_task_response_fixture_is_produced_by_real_router",
        "suite::transitions_adoption::specify_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.promote-task.path",
        "POST /api/v1/tasks/:task_id/transitions/promote path",
        "POST /api/v1/tasks/:task_id/transitions/promote",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:promote-task-path:v1",
        "schemas/fixtures/api/promote-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "suite::transitions_adoption::promote_task_path_dto_serializes_to_committed_fixture",
        "suite::transitions_adoption::promote_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.promote-task.response",
        "POST /api/v1/tasks/:task_id/transitions/promote response",
        "POST /api/v1/tasks/:task_id/transitions/promote",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:promote-task-response:v1",
        "schemas/fixtures/api/promote-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::transitions_adoption::promote_task_response_fixture_is_produced_by_real_router",
        "suite::transitions_adoption::promote_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.reopen-task.path",
        "POST /api/v1/tasks/:task_id/transitions/reopen path",
        "POST /api/v1/tasks/:task_id/transitions/reopen",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:reopen-task-path:v1",
        "schemas/fixtures/api/reopen-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "suite::transitions_adoption::reopen_task_path_dto_serializes_to_committed_fixture",
        "suite::transitions_adoption::reopen_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.reopen-task.response",
        "POST /api/v1/tasks/:task_id/transitions/reopen response",
        "POST /api/v1/tasks/:task_id/transitions/reopen",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:reopen-task-response:v1",
        "schemas/fixtures/api/reopen-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::transitions_adoption::reopen_task_response_fixture_is_produced_by_real_router",
        "suite::transitions_adoption::reopen_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.unblock-task.path",
        "POST /api/v1/tasks/:task_id/transitions/unblock path",
        "POST /api/v1/tasks/:task_id/transitions/unblock",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:unblock-task-path:v1",
        "schemas/fixtures/api/unblock-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "suite::transitions_adoption::unblock_task_path_dto_serializes_to_committed_fixture",
        "suite::transitions_adoption::unblock_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.unblock-task.response",
        "POST /api/v1/tasks/:task_id/transitions/unblock response",
        "POST /api/v1/tasks/:task_id/transitions/unblock",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:unblock-task-response:v1",
        "schemas/fixtures/api/unblock-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::transitions_adoption::unblock_task_response_fixture_is_produced_by_real_router",
        "suite::transitions_adoption::unblock_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.archive-task.path",
        "POST /api/v1/tasks/:task_id/transitions/archive path",
        "POST /api/v1/tasks/:task_id/transitions/archive",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:archive-task-path:v1",
        "schemas/fixtures/api/archive-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "suite::transitions_adoption::archive_task_path_dto_serializes_to_committed_fixture",
        "suite::transitions_adoption::archive_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.archive-task.response",
        "POST /api/v1/tasks/:task_id/transitions/archive response",
        "POST /api/v1/tasks/:task_id/transitions/archive",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:archive-task-response:v1",
        "schemas/fixtures/api/archive-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::transitions_adoption::archive_task_response_fixture_is_produced_by_real_router",
        "suite::transitions_adoption::archive_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_request!(
        "api.specify-task.request",
        "POST /api/v1/tasks/:task_id/transitions/specify",
        "POST /api/v1/tasks/:task_id/transitions/specify",
        "urn:kanban-tool:schema:api:specify-task-request:v1",
        "schemas/fixtures/api/specify-task-request.v1.valid.json",
        "suite::lifecycle_request_adoption::specify_task_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::specify_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_request!(
        "api.promote-task.request",
        "POST /api/v1/tasks/:task_id/transitions/promote",
        "POST /api/v1/tasks/:task_id/transitions/promote",
        "urn:kanban-tool:schema:api:promote-task-request:v1",
        "schemas/fixtures/api/promote-task-request.v1.valid.json",
        "suite::lifecycle_request_adoption::promote_task_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::promote_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_request!(
        "api.claim-task.request",
        "POST /api/v1/tasks/:task_id/transitions/claim",
        "POST /api/v1/tasks/:task_id/transitions/claim",
        "urn:kanban-tool:schema:api:claim-task-request:v1",
        "schemas/fixtures/api/claim-task-request.v1.valid.json",
        "suite::lifecycle_request_adoption::claim_task_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::claim_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.claim-task.path",
        "POST /api/v1/tasks/:task_id/transitions/claim path",
        "POST /api/v1/tasks/:task_id/transitions/claim",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:claim-task-path:v1",
        "schemas/fixtures/api/claim-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "suite::transitions_adoption::claim_task_path_dto_serializes_to_committed_fixture",
        "suite::transitions_adoption::claim_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.claim-task.response",
        "POST /api/v1/tasks/:task_id/transitions/claim response",
        "POST /api/v1/tasks/:task_id/transitions/claim",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:claim-task-response:v1",
        "schemas/fixtures/api/claim-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::transitions_adoption::claim_task_response_fixture_is_produced_by_real_router",
        "suite::transitions_adoption::claim_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_request!(
        "api.reclaim-task.request",
        "POST /api/v1/tasks/:task_id/transitions/reclaim",
        "POST /api/v1/tasks/:task_id/transitions/reclaim",
        "urn:kanban-tool:schema:api:reclaim-task-request:v1",
        "schemas/fixtures/api/reclaim-task-request.v1.valid.json",
        "suite::lifecycle_request_adoption::reclaim_task_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::reclaim_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.reclaim-task.path",
        "POST /api/v1/tasks/:task_id/transitions/reclaim path",
        "POST /api/v1/tasks/:task_id/transitions/reclaim",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:reclaim-task-path:v1",
        "schemas/fixtures/api/reclaim-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "suite::transitions_adoption::reclaim_task_path_dto_serializes_to_committed_fixture",
        "suite::transitions_adoption::reclaim_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.reclaim-task.response",
        "POST /api/v1/tasks/:task_id/transitions/reclaim response",
        "POST /api/v1/tasks/:task_id/transitions/reclaim",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:reclaim-task-response:v1",
        "schemas/fixtures/api/reclaim-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::transitions_adoption::reclaim_task_response_fixture_is_produced_by_real_router",
        "suite::transitions_adoption::reclaim_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_request!(
        "api.heartbeat-task.request",
        "POST /api/v1/tasks/:task_id/transitions/heartbeat",
        "POST /api/v1/tasks/:task_id/transitions/heartbeat",
        "urn:kanban-tool:schema:api:heartbeat-task-request:v1",
        "schemas/fixtures/api/heartbeat-task-request.v1.valid.json",
        "suite::lifecycle_request_adoption::heartbeat_task_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::heartbeat_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.heartbeat-task.path",
        "POST /api/v1/tasks/:task_id/transitions/heartbeat path",
        "POST /api/v1/tasks/:task_id/transitions/heartbeat",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:heartbeat-task-path:v1",
        "schemas/fixtures/api/heartbeat-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "suite::transitions_adoption::heartbeat_task_path_dto_serializes_to_committed_fixture",
        "suite::transitions_adoption::heartbeat_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.heartbeat-task.response",
        "POST /api/v1/tasks/:task_id/transitions/heartbeat response",
        "POST /api/v1/tasks/:task_id/transitions/heartbeat",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:heartbeat-task-response:v1",
        "schemas/fixtures/api/heartbeat-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::transitions_adoption::heartbeat_task_response_fixture_is_produced_by_real_router",
        "suite::transitions_adoption::heartbeat_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_request!(
        "api.release-task.request",
        "POST /api/v1/tasks/:task_id/transitions/release",
        "POST /api/v1/tasks/:task_id/transitions/release",
        "urn:kanban-tool:schema:api:release-task-request:v1",
        "schemas/fixtures/api/release-task-request.v1.valid.json",
        "lifecycle_release_request_contract",
        "router::tests::task_release_closes_the_application_path"
    ),
    adopted_comment_contract!(
        "api.release-task.path",
        "POST /api/v1/tasks/:task_id/transitions/release path",
        "POST /api/v1/tasks/:task_id/transitions/release",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:release-task-path:v1",
        "schemas/fixtures/api/release-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "release_task_path_contract",
        "router::tests::task_release_closes_the_application_path"
    ),
    adopted_comment_contract!(
        "api.release-task.response",
        "POST /api/v1/tasks/:task_id/transitions/release response",
        "POST /api/v1/tasks/:task_id/transitions/release",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:release-task-response:v1",
        "schemas/fixtures/api/release-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "router::tests::task_release_closes_the_application_path",
        "release_task_response_contract"
    ),
    adopted_api_request!(
        "api.complete-task.request",
        "POST /api/v1/tasks/:task_id/transitions/complete",
        "POST /api/v1/tasks/:task_id/transitions/complete",
        "urn:kanban-tool:schema:api:complete-task-request:v1",
        "schemas/fixtures/api/complete-task-request.v1.valid.json",
        "complete_task_request_contract",
        "router::tests::task_done_closes_the_running_application_path"
    ),
    adopted_comment_contract!(
        "api.complete-task.path",
        "POST /api/v1/tasks/:task_id/transitions/complete path",
        "POST /api/v1/tasks/:task_id/transitions/complete",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:complete-task-path:v1",
        "schemas/fixtures/api/complete-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "complete_task_path_contract",
        "router::tests::task_done_closes_the_running_application_path"
    ),
    adopted_comment_contract!(
        "api.complete-task.response",
        "POST /api/v1/tasks/:task_id/transitions/complete response",
        "POST /api/v1/tasks/:task_id/transitions/complete",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:complete-task-response:v1",
        "schemas/fixtures/api/complete-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "router::tests::task_done_closes_the_running_application_path",
        "complete_task_response_contract"
    ),
    adopted_api_request!(
        "api.submit-review-task.request",
        "POST /api/v1/tasks/:task_id/transitions/submit-review",
        "POST /api/v1/tasks/:task_id/transitions/submit-review",
        "urn:kanban-tool:schema:api:submit-review-task-request:v1",
        "schemas/fixtures/api/submit-review-task-request.v1.valid.json",
        "submit_review_task_request_contract",
        "router::tests::task_review_closes_the_application_path"
    ),
    adopted_comment_contract!(
        "api.submit-review-task.path",
        "POST /api/v1/tasks/:task_id/transitions/submit-review path",
        "POST /api/v1/tasks/:task_id/transitions/submit-review",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:submit-review-task-path:v1",
        "schemas/fixtures/api/submit-review-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "submit_review_task_path_contract",
        "router::tests::task_review_closes_the_application_path"
    ),
    adopted_comment_contract!(
        "api.submit-review-task.response",
        "POST /api/v1/tasks/:task_id/transitions/submit-review response",
        "POST /api/v1/tasks/:task_id/transitions/submit-review",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:submit-review-task-response:v1",
        "schemas/fixtures/api/submit-review-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "router::tests::task_review_closes_the_application_path",
        "submit_review_task_response_contract"
    ),
    adopted_api_request!(
        "api.block-task.request",
        "POST /api/v1/tasks/:task_id/transitions/block",
        "POST /api/v1/tasks/:task_id/transitions/block",
        "urn:kanban-tool:schema:api:block-task-request:v1",
        "schemas/fixtures/api/block-task-request.v1.valid.json",
        "block_task_request_contract",
        "router::tests::task_block_closes_non_running_and_running_application_paths"
    ),
    adopted_comment_contract!(
        "api.block-task.path",
        "POST /api/v1/tasks/:task_id/transitions/block path",
        "POST /api/v1/tasks/:task_id/transitions/block",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:block-task-path:v1",
        "schemas/fixtures/api/block-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_TRANSITION_PATH_PARAMETERS,
        "block_task_path_contract",
        "router::tests::task_block_closes_non_running_and_running_application_paths"
    ),
    adopted_comment_contract!(
        "api.block-task.response",
        "POST /api/v1/tasks/:task_id/transitions/block response",
        "POST /api/v1/tasks/:task_id/transitions/block",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:block-task-response:v1",
        "schemas/fixtures/api/block-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "router::tests::task_block_closes_non_running_and_running_application_paths",
        "block_task_response_contract"
    ),
    adopted_api_request!(
        "api.unblock-task.request",
        "POST /api/v1/tasks/:task_id/transitions/unblock",
        "POST /api/v1/tasks/:task_id/transitions/unblock",
        "urn:kanban-tool:schema:api:unblock-task-request:v1",
        "schemas/fixtures/api/unblock-task-request.v1.valid.json",
        "suite::lifecycle_request_adoption::unblock_task_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::unblock_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_request!(
        "api.reopen-task.request",
        "POST /api/v1/tasks/:task_id/transitions/reopen",
        "POST /api/v1/tasks/:task_id/transitions/reopen",
        "urn:kanban-tool:schema:api:reopen-task-request:v1",
        "schemas/fixtures/api/reopen-task-request.v1.valid.json",
        "suite::lifecycle_request_adoption::reopen_task_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::reopen_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_request!(
        "api.archive-task.request",
        "POST /api/v1/tasks/:task_id/transitions/archive",
        "POST /api/v1/tasks/:task_id/transitions/archive",
        "urn:kanban-tool:schema:api:archive-task-request:v1",
        "schemas/fixtures/api/archive-task-request.v1.valid.json",
        "suite::lifecycle_request_adoption::archive_task_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::archive_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_request!(
        "api.archive-board.request",
        "POST /api/v1/boards/:board/archive",
        "POST /api/v1/boards/:board/archive",
        "urn:kanban-tool:schema:api:archive-board-request:v1",
        "schemas/fixtures/api/archive-board-request.v1.valid.json",
        "suite::lifecycle_request_adoption::archive_board_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::archive_board_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_request!(
        "api.add-dependency.request",
        "POST /api/v1/tasks/:task_id/dependencies",
        "POST /api/v1/tasks/:task_id/dependencies",
        "urn:kanban-tool:schema:api:add-dependency-request:v1",
        "schemas/fixtures/api/add-dependency-request.v1.valid.json",
        "suite::lifecycle_request_adoption::add_dependency_request_dto_serializes_to_committed_fixture",
        "suite::lifecycle_request_adoption::add_dependency_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.list-task-labels.path",
        "GET /api/v1/tasks/:task_id/labels path",
        "GET /api/v1/tasks/:task_id/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-task-labels-path:v1",
        "schemas/fixtures/api/list-task-labels-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_LABEL_PATH_PARAMETERS,
        "suite::labels_adoption::list_task_labels_path_dto_serializes_to_committed_fixture",
        "suite::labels_adoption::list_task_labels_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.list-task-labels.response",
        "GET /api/v1/tasks/:task_id/labels response",
        "GET /api/v1/tasks/:task_id/labels",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-task-labels-response:v1",
        "schemas/fixtures/api/list-task-labels-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::labels_adoption::list_task_labels_response_fixture_is_produced_by_real_router",
        "suite::labels_adoption::list_task_labels_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.add-task-label.path",
        "POST /api/v1/tasks/:task_id/labels path",
        "POST /api/v1/tasks/:task_id/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:add-task-label-path:v1",
        "schemas/fixtures/api/add-task-label-path.v1.valid.json",
        HttpTransportLocation::Path,
        TASK_LABEL_PATH_PARAMETERS,
        "suite::labels_adoption::add_task_label_path_dto_serializes_to_committed_fixture",
        "suite::labels_adoption::add_task_label_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.add-task-label.request",
        "POST /api/v1/tasks/:task_id/labels request",
        "POST /api/v1/tasks/:task_id/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:add-task-label-request:v1",
        "schemas/fixtures/api/add-task-label-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::labels_adoption::add_task_label_request_dto_serializes_to_committed_fixture",
        "suite::labels_adoption::add_task_label_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.add-task-label.response",
        "POST /api/v1/tasks/:task_id/labels response",
        "POST /api/v1/tasks/:task_id/labels",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:add-task-label-response:v1",
        "schemas/fixtures/api/add-task-label-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::labels_adoption::add_task_label_response_fixture_is_produced_by_real_router",
        "suite::labels_adoption::add_task_label_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.remove-task-label.path",
        "DELETE /api/v1/tasks/:task_id/labels/:label_id path",
        "DELETE /api/v1/tasks/:task_id/labels/:label_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:remove-task-label-path:v1",
        "schemas/fixtures/api/remove-task-label-path.v1.valid.json",
        HttpTransportLocation::Path,
        REMOVE_TASK_LABEL_PATH_PARAMETERS,
        "suite::labels_adoption::remove_task_label_path_dto_serializes_to_committed_fixture",
        "suite::labels_adoption::remove_task_label_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.remove-task-label.response",
        "DELETE /api/v1/tasks/:task_id/labels/:label_id response",
        "DELETE /api/v1/tasks/:task_id/labels/:label_id",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:remove-task-label-response:v1",
        "schemas/fixtures/api/remove-task-label-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::labels_adoption::remove_task_label_response_fixture_is_produced_by_real_router",
        "suite::labels_adoption::remove_task_label_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.list-runs.path",
        "GET /api/v1/tasks/:task_id/runs path",
        "GET /api/v1/tasks/:task_id/runs",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-runs-path:v1",
        "schemas/fixtures/api/list-runs-path.v1.valid.json",
        HttpTransportLocation::Path,
        RUN_TASK_PATH_PARAMETERS,
        "suite::runs_adoption::list_runs_path_dto_serializes_to_committed_fixture",
        "suite::runs_adoption::list_runs_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.list-runs.response",
        "GET /api/v1/tasks/:task_id/runs response",
        "GET /api/v1/tasks/:task_id/runs",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-runs-response:v1",
        "schemas/fixtures/api/list-runs-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::runs_adoption::list_runs_response_fixture_is_produced_by_real_router",
        "suite::runs_adoption::list_runs_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.get-run.path",
        "GET /api/v1/runs/:run_id path",
        "GET /api/v1/runs/:run_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:get-run-path:v1",
        "schemas/fixtures/api/get-run-path.v1.valid.json",
        HttpTransportLocation::Path,
        RUN_ID_PATH_PARAMETERS,
        "suite::runs_adoption::get_run_path_dto_serializes_to_committed_fixture",
        "suite::runs_adoption::get_run_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.get-run.response",
        "GET /api/v1/runs/:run_id response",
        "GET /api/v1/runs/:run_id",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:get-run-response:v1",
        "schemas/fixtures/api/get-run-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::runs_adoption::get_run_response_fixture_is_produced_by_real_router",
        "suite::runs_adoption::get_run_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.create-task.path",
        "POST /api/v1/boards/:board/tasks path",
        "POST /api/v1/boards/:board/tasks",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-task-path:v1",
        "schemas/fixtures/api/create-task-path.v1.valid.json",
        HttpTransportLocation::Path,
        CREATE_TASK_PATH_PARAMETERS,
        "suite::create_task_adoption::create_task_path_dto_serializes_to_committed_fixture",
        "suite::create_task_adoption::create_task_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.create-task.request",
        "POST /api/v1/boards/:board/tasks request",
        "POST /api/v1/boards/:board/tasks",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-task-request:v1",
        "schemas/fixtures/api/create-task-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::create_task_adoption::create_task_request_dto_serializes_to_committed_fixture",
        "suite::create_task_adoption::create_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.create-task.response",
        "POST /api/v1/boards/:board/tasks response",
        "POST /api/v1/boards/:board/tasks",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:create-task-response:v1",
        "schemas/fixtures/api/create-task-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::create_task_adoption::create_task_response_fixture_is_produced_by_real_router",
        "suite::create_task_adoption::create_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.list-comments.path",
        "GET /api/v1/tasks/:task_id/comments path",
        "GET /api/v1/tasks/:task_id/comments",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-comments-path:v1",
        "schemas/fixtures/api/list-comments-path.v1.valid.json",
        HttpTransportLocation::Path,
        COMMENT_PATH_PARAMETERS,
        "suite::comments_adoption::list_comments_path_dto_serializes_to_committed_fixture",
        "suite::comments_adoption::list_comments_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.list-comments.response",
        "GET /api/v1/tasks/:task_id/comments response",
        "GET /api/v1/tasks/:task_id/comments",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-comments-response:v1",
        "schemas/fixtures/api/list-comments-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::comments_adoption::list_comments_response_fixture_is_produced_by_real_router",
        "suite::comments_adoption::list_comments_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.create-comment.path",
        "POST /api/v1/tasks/:task_id/comments path",
        "POST /api/v1/tasks/:task_id/comments",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-comment-path:v1",
        "schemas/fixtures/api/create-comment-path.v1.valid.json",
        HttpTransportLocation::Path,
        COMMENT_PATH_PARAMETERS,
        "suite::comments_adoption::create_comment_path_dto_serializes_to_committed_fixture",
        "suite::comments_adoption::create_comment_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.create-comment.request",
        "POST /api/v1/tasks/:task_id/comments request",
        "POST /api/v1/tasks/:task_id/comments",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-comment-request:v1",
        "schemas/fixtures/api/create-comment-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::comments_adoption::create_comment_request_dto_serializes_to_committed_fixture",
        "suite::comments_adoption::create_comment_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.create-comment.response",
        "POST /api/v1/tasks/:task_id/comments response",
        "POST /api/v1/tasks/:task_id/comments",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:create-comment-response:v1",
        "schemas/fixtures/api/create-comment-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::comments_adoption::create_comment_response_fixture_is_produced_by_real_router",
        "suite::comments_adoption::create_comment_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.list-steps.path",
        "GET /api/v1/tasks/:task_id/steps path",
        "GET /api/v1/tasks/:task_id/steps",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-steps-path:v1",
        "schemas/fixtures/api/list-steps-path.v1.valid.json",
        HttpTransportLocation::Path,
        STEP_TASK_PATH_PARAMETERS,
        "suite::steps_adoption::list_steps_path_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::list_steps_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.list-steps.response",
        "GET /api/v1/tasks/:task_id/steps response",
        "GET /api/v1/tasks/:task_id/steps",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-steps-response:v1",
        "schemas/fixtures/api/list-steps-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::steps_adoption::list_steps_response_fixture_is_produced_by_real_router",
        "suite::steps_adoption::list_steps_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.create-step.path",
        "POST /api/v1/tasks/:task_id/steps path",
        "POST /api/v1/tasks/:task_id/steps",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-step-path:v1",
        "schemas/fixtures/api/create-step-path.v1.valid.json",
        HttpTransportLocation::Path,
        STEP_TASK_PATH_PARAMETERS,
        "suite::steps_adoption::create_step_path_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::create_step_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.create-step.request",
        "POST /api/v1/tasks/:task_id/steps request",
        "POST /api/v1/tasks/:task_id/steps",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-step-request:v1",
        "schemas/fixtures/api/create-step-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::steps_adoption::create_step_request_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::create_step_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.create-step.response",
        "POST /api/v1/tasks/:task_id/steps response",
        "POST /api/v1/tasks/:task_id/steps",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:create-step-response:v1",
        "schemas/fixtures/api/create-step-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::steps_adoption::create_step_response_fixture_is_produced_by_real_router",
        "suite::steps_adoption::create_step_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.update-step.path",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id path",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:update-step-path:v1",
        "schemas/fixtures/api/update-step-path.v1.valid.json",
        HttpTransportLocation::Path,
        STEP_ITEM_PATH_PARAMETERS,
        "suite::steps_adoption::update_step_path_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::update_step_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.update-step.request",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id request",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:update-step-request:v1",
        "schemas/fixtures/api/update-step-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::steps_adoption::update_step_request_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::update_step_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.update-step.response",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id response",
        "PATCH /api/v1/tasks/:task_id/steps/:step_id",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:update-step-response:v1",
        "schemas/fixtures/api/update-step-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::steps_adoption::update_step_response_fixture_is_produced_by_real_router",
        "suite::steps_adoption::update_step_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.remove-step.path",
        "DELETE /api/v1/tasks/:task_id/steps/:step_id path",
        "DELETE /api/v1/tasks/:task_id/steps/:step_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:remove-step-path:v1",
        "schemas/fixtures/api/remove-step-path.v1.valid.json",
        HttpTransportLocation::Path,
        STEP_ITEM_PATH_PARAMETERS,
        "suite::steps_adoption::remove_step_path_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::remove_step_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.remove-step.response",
        "DELETE /api/v1/tasks/:task_id/steps/:step_id response",
        "DELETE /api/v1/tasks/:task_id/steps/:step_id",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:remove-step-response:v1",
        "schemas/fixtures/api/remove-step-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::steps_adoption::remove_step_response_fixture_is_produced_by_real_router",
        "suite::steps_adoption::remove_step_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.complete-step.path",
        "POST /api/v1/tasks/:task_id/steps/:step_id/done path",
        "POST /api/v1/tasks/:task_id/steps/:step_id/done",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:complete-step-path:v1",
        "schemas/fixtures/api/complete-step-path.v1.valid.json",
        HttpTransportLocation::Path,
        STEP_ITEM_PATH_PARAMETERS,
        "suite::steps_adoption::complete_step_path_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::complete_step_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.complete-step.request",
        "POST /api/v1/tasks/:task_id/steps/:step_id/done request",
        "POST /api/v1/tasks/:task_id/steps/:step_id/done",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:complete-step-request:v1",
        "schemas/fixtures/api/complete-step-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::steps_adoption::complete_step_request_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::complete_step_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.complete-step.response",
        "POST /api/v1/tasks/:task_id/steps/:step_id/done response",
        "POST /api/v1/tasks/:task_id/steps/:step_id/done",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:complete-step-response:v1",
        "schemas/fixtures/api/complete-step-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::steps_adoption::complete_step_response_fixture_is_produced_by_real_router",
        "suite::steps_adoption::complete_step_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.skip-step.path",
        "POST /api/v1/tasks/:task_id/steps/:step_id/skip path",
        "POST /api/v1/tasks/:task_id/steps/:step_id/skip",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:skip-step-path:v1",
        "schemas/fixtures/api/skip-step-path.v1.valid.json",
        HttpTransportLocation::Path,
        STEP_ITEM_PATH_PARAMETERS,
        "suite::steps_adoption::skip_step_path_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::skip_step_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.skip-step.request",
        "POST /api/v1/tasks/:task_id/steps/:step_id/skip request",
        "POST /api/v1/tasks/:task_id/steps/:step_id/skip",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:skip-step-request:v1",
        "schemas/fixtures/api/skip-step-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::steps_adoption::skip_step_request_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::skip_step_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.skip-step.response",
        "POST /api/v1/tasks/:task_id/steps/:step_id/skip response",
        "POST /api/v1/tasks/:task_id/steps/:step_id/skip",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:skip-step-response:v1",
        "schemas/fixtures/api/skip-step-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::steps_adoption::skip_step_response_fixture_is_produced_by_real_router",
        "suite::steps_adoption::skip_step_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.reopen-step.path",
        "POST /api/v1/tasks/:task_id/steps/:step_id/reopen path",
        "POST /api/v1/tasks/:task_id/steps/:step_id/reopen",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:reopen-step-path:v1",
        "schemas/fixtures/api/reopen-step-path.v1.valid.json",
        HttpTransportLocation::Path,
        STEP_ITEM_PATH_PARAMETERS,
        "suite::steps_adoption::reopen_step_path_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::reopen_step_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.reopen-step.request",
        "POST /api/v1/tasks/:task_id/steps/:step_id/reopen request",
        "POST /api/v1/tasks/:task_id/steps/:step_id/reopen",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:reopen-step-request:v1",
        "schemas/fixtures/api/reopen-step-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::steps_adoption::reopen_step_request_dto_serializes_to_committed_fixture",
        "suite::steps_adoption::reopen_step_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.reopen-step.response",
        "POST /api/v1/tasks/:task_id/steps/:step_id/reopen response",
        "POST /api/v1/tasks/:task_id/steps/:step_id/reopen",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:reopen-step-response:v1",
        "schemas/fixtures/api/reopen-step-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::steps_adoption::reopen_step_response_fixture_is_produced_by_real_router",
        "suite::steps_adoption::reopen_step_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_response_contract!(
        "api.list-events.response",
        "GET /api/v1/events response",
        "GET /api/v1/events",
        "urn:kanban-tool:schema:api:list-events-response:v1",
        "schemas/fixtures/api/list-events-response.v1.valid.json",
        "suite::events::list_events_response_fixture_is_produced_by_real_router",
        "suite::events::list_events_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_response_contract!(
        "api.list-label-ontology-signals.response",
        "GET /api/v1/boards/:board/label-ontology/signals response",
        "GET /api/v1/boards/:board/label-ontology/signals",
        "urn:kanban-tool:schema:api:list-label-ontology-signals-response:v1",
        "schemas/fixtures/api/list-label-ontology-signals-response.v1.valid.json",
        "suite::ontology_signals::list_label_ontology_signals_response_fixture_is_produced_by_real_router",
        "suite::ontology_signals::list_label_ontology_signals_response_fixture_is_consumed_by_contract_root"
    ),
    generated_api_contract!(
        "api.list-board-labels.path",
        "GET /api/v1/boards/:board/labels path",
        "GET /api/v1/boards/:board/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-board-labels-path:v1",
        "schemas/fixtures/api/list-board-labels-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.list-board-labels.response",
        "GET /api/v1/boards/:board/labels success",
        "GET /api/v1/boards/:board/labels",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-board-labels-response:v1",
        "schemas/fixtures/api/list-board-labels-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.create-board-label.path",
        "POST /api/v1/boards/:board/labels path",
        "POST /api/v1/boards/:board/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-board-label-path:v1",
        "schemas/fixtures/api/create-board-label-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.create-board-label.request",
        "POST /api/v1/boards/:board/labels body",
        "POST /api/v1/boards/:board/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-board-label-request:v1",
        "schemas/fixtures/api/create-board-label-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.create-board-label.response",
        "POST /api/v1/boards/:board/labels success",
        "POST /api/v1/boards/:board/labels",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:create-board-label-response:v1",
        "schemas/fixtures/api/create-board-label-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.list-label-semantics.path",
        "GET /api/v1/boards/:board/labels/semantics path",
        "GET /api/v1/boards/:board/labels/semantics",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-label-semantics-path:v1",
        "schemas/fixtures/api/list-label-semantics-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.list-label-semantics.response",
        "GET /api/v1/boards/:board/labels/semantics success",
        "GET /api/v1/boards/:board/labels/semantics",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-label-semantics-response:v1",
        "schemas/fixtures/api/list-label-semantics-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.get-label-semantics.path",
        "GET /api/v1/boards/:board/labels/:label_id/semantics path",
        "GET /api/v1/boards/:board/labels/:label_id/semantics",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:get-label-semantics-path:v1",
        "schemas/fixtures/api/get-label-semantics-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "label_id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ]
    ),
    generated_api_contract!(
        "api.get-label-semantics.response",
        "GET /api/v1/boards/:board/labels/:label_id/semantics success",
        "GET /api/v1/boards/:board/labels/:label_id/semantics",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:get-label-semantics-response:v1",
        "schemas/fixtures/api/get-label-semantics-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.upsert-label-semantics.path",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics path",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:upsert-label-semantics-path:v1",
        "schemas/fixtures/api/upsert-label-semantics-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "label_id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ]
    ),
    generated_api_contract!(
        "api.upsert-label-semantics.request",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics body",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:upsert-label-semantics-request:v1",
        "schemas/fixtures/api/upsert-label-semantics-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.upsert-label-semantics.response",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics success",
        "PUT /api/v1/boards/:board/labels/:label_id/semantics",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:upsert-label-semantics-response:v1",
        "schemas/fixtures/api/upsert-label-semantics-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.delete-label-semantics.path",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics path",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:delete-label-semantics-path:v1",
        "schemas/fixtures/api/delete-label-semantics-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "label_id",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ]
    ),
    generated_api_contract!(
        "api.delete-label-semantics.query",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics query",
        "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:delete-label-semantics-query:v1",
        "schemas/fixtures/api/delete-label-semantics-query.v1.valid.json",
        HttpTransportLocation::Query,
        &[]
    ),
    generated_api_contract!(
        "api.list-label-atoms.path",
        "GET /api/v1/boards/:board/labels/atoms path",
        "GET /api/v1/boards/:board/labels/atoms",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-label-atoms-path:v1",
        "schemas/fixtures/api/list-label-atoms-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.list-label-atoms.response",
        "GET /api/v1/boards/:board/labels/atoms success",
        "GET /api/v1/boards/:board/labels/atoms",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-label-atoms-response:v1",
        "schemas/fixtures/api/list-label-atoms-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.label-atom.path",
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain path",
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:label-atom-path:v1",
        "schemas/fixtures/api/label-atom-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[
            WireParameter {
                name: "board",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            },
            WireParameter {
                name: "atom_ref",
                cardinality: Some(WireParameterCardinality::RequiredOne)
            }
        ]
    ),
    generated_api_contract!(
        "api.explain-label-atom.response",
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain success",
        "GET /api/v1/boards/:board/labels/atoms/:atom_ref/explain",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:explain-label-atom-response:v1",
        "schemas/fixtures/api/explain-label-atom-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.label-atom-index-status.path",
        "GET /api/v1/boards/:board/labels/atom-index/status path",
        "GET /api/v1/boards/:board/labels/atom-index/status",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:label-atom-index-status-path:v1",
        "schemas/fixtures/api/label-atom-index-status-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.label-atom-index-status.response",
        "GET /api/v1/boards/:board/labels/atom-index/status success",
        "GET /api/v1/boards/:board/labels/atom-index/status",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:label-atom-index-status-response:v1",
        "schemas/fixtures/api/label-atom-index-status-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.rebuild-label-atom-index.path",
        "POST /api/v1/boards/:board/labels/atom-index/rebuild path",
        "POST /api/v1/boards/:board/labels/atom-index/rebuild",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:rebuild-label-atom-index-path:v1",
        "schemas/fixtures/api/rebuild-label-atom-index-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.rebuild-label-atom-index.response",
        "POST /api/v1/boards/:board/labels/atom-index/rebuild success",
        "POST /api/v1/boards/:board/labels/atom-index/rebuild",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:rebuild-label-atom-index-response:v1",
        "schemas/fixtures/api/rebuild-label-atom-index-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.query-label-atom-index.path",
        "GET /api/v1/boards/:board/labels/atom-index/query path",
        "GET /api/v1/boards/:board/labels/atom-index/query",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:query-label-atom-index-path:v1",
        "schemas/fixtures/api/query-label-atom-index-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.query-label-atom-index.query",
        "GET /api/v1/boards/:board/labels/atom-index/query query",
        "GET /api/v1/boards/:board/labels/atom-index/query",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:query-label-atom-index-query:v1",
        "schemas/fixtures/api/query-label-atom-index-query.v1.valid.json",
        HttpTransportLocation::Query,
        &[]
    ),
    generated_api_contract!(
        "api.query-label-atom-index.response",
        "GET /api/v1/boards/:board/labels/atom-index/query success",
        "GET /api/v1/boards/:board/labels/atom-index/query",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:query-label-atom-index-response:v1",
        "schemas/fixtures/api/query-label-atom-index-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.list-signals.path",
        "GET /api/v1/boards/:board/signals path",
        "GET /api/v1/boards/:board/signals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-signals-path:v1",
        "schemas/fixtures/api/list-signals-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.list-signals.query",
        "GET /api/v1/boards/:board/signals query",
        "GET /api/v1/boards/:board/signals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-signals-query:v1",
        "schemas/fixtures/api/list-signals-query.v1.valid.json",
        HttpTransportLocation::Query,
        &[]
    ),
    generated_api_contract!(
        "api.list-signals.response",
        "GET /api/v1/boards/:board/signals success",
        "GET /api/v1/boards/:board/signals",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-signals-response:v1",
        "schemas/fixtures/api/list-signals-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.review-signals.path",
        "GET /api/v1/boards/:board/signals/review path",
        "GET /api/v1/boards/:board/signals/review",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:review-signals-path:v1",
        "schemas/fixtures/api/review-signals-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.review-signals.query",
        "GET /api/v1/boards/:board/signals/review query",
        "GET /api/v1/boards/:board/signals/review",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:review-signals-query:v1",
        "schemas/fixtures/api/review-signals-query.v1.valid.json",
        HttpTransportLocation::Query,
        &[]
    ),
    generated_api_contract!(
        "api.review-signals.response",
        "GET /api/v1/boards/:board/signals/review success",
        "GET /api/v1/boards/:board/signals/review",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:review-signals-response:v1",
        "schemas/fixtures/api/review-signals-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.get-signal.path",
        "GET /api/v1/signals/:signal_id path",
        "GET /api/v1/signals/:signal_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:get-signal-path:v1",
        "schemas/fixtures/api/get-signal-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "signal_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.get-signal.response",
        "GET /api/v1/signals/:signal_id success",
        "GET /api/v1/signals/:signal_id",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:get-signal-response:v1",
        "schemas/fixtures/api/get-signal-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.bootstrap-task-label.path",
        "POST /api/v1/tasks/:task_id/labels/bootstrap path",
        "POST /api/v1/tasks/:task_id/labels/bootstrap",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:bootstrap-task-label-path:v1",
        "schemas/fixtures/api/bootstrap-task-label-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "task_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.bootstrap-task-label.request",
        "POST /api/v1/tasks/:task_id/labels/bootstrap body",
        "POST /api/v1/tasks/:task_id/labels/bootstrap",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:bootstrap-task-label-request:v1",
        "schemas/fixtures/api/bootstrap-task-label-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.bootstrap-task-label.response",
        "POST /api/v1/tasks/:task_id/labels/bootstrap success",
        "POST /api/v1/tasks/:task_id/labels/bootstrap",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:bootstrap-task-label-response:v1",
        "schemas/fixtures/api/bootstrap-task-label-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.suggest-task-labels.path",
        "GET /api/v1/tasks/:task_id/labels/suggestions path",
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:suggest-task-labels-path:v1",
        "schemas/fixtures/api/suggest-task-labels-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "task_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.label-suggestion.query",
        "GET /api/v1/tasks/:task_id/labels/suggestions query",
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:label-suggestion-query:v1",
        "schemas/fixtures/api/label-suggestion-query.v1.valid.json",
        HttpTransportLocation::Query,
        &[]
    ),
    generated_api_contract!(
        "api.suggest-task-labels.response",
        "GET /api/v1/tasks/:task_id/labels/suggestions success",
        "GET /api/v1/tasks/:task_id/labels/suggestions",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:suggest-task-labels-response:v1",
        "schemas/fixtures/api/suggest-task-labels-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.list-task-label-proposals.path",
        "GET /api/v1/tasks/:task_id/label-proposals path",
        "GET /api/v1/tasks/:task_id/label-proposals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-task-label-proposals-path:v1",
        "schemas/fixtures/api/list-task-label-proposals-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "task_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.list-task-label-proposals.response",
        "GET /api/v1/tasks/:task_id/label-proposals success",
        "GET /api/v1/tasks/:task_id/label-proposals",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-task-label-proposals-response:v1",
        "schemas/fixtures/api/list-task-label-proposals-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.propose-task-label.path",
        "POST /api/v1/tasks/:task_id/label-proposals path",
        "POST /api/v1/tasks/:task_id/label-proposals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:propose-task-label-path:v1",
        "schemas/fixtures/api/propose-task-label-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "task_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.propose-task-label.query",
        "POST /api/v1/tasks/:task_id/label-proposals query",
        "POST /api/v1/tasks/:task_id/label-proposals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:propose-task-label-query:v1",
        "schemas/fixtures/api/propose-task-label-query.v1.valid.json",
        HttpTransportLocation::Query,
        &[]
    ),
    generated_api_contract!(
        "api.propose-task-label.request",
        "POST /api/v1/tasks/:task_id/label-proposals body",
        "POST /api/v1/tasks/:task_id/label-proposals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:propose-task-label-request:v1",
        "schemas/fixtures/api/propose-task-label-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.propose-task-label.response",
        "POST /api/v1/tasks/:task_id/label-proposals success",
        "POST /api/v1/tasks/:task_id/label-proposals",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:propose-task-label-response:v1",
        "schemas/fixtures/api/propose-task-label-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.record-label-ontology-observation.path",
        "POST /api/v1/tasks/:task_id/label-ontology/observations path",
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:record-label-ontology-observation-path:v1",
        "schemas/fixtures/api/record-label-ontology-observation-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "task_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.record-label-ontology-observation.body",
        "POST /api/v1/tasks/:task_id/label-ontology/observations body",
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:record-label-ontology-observation-body:v1",
        "schemas/fixtures/api/record-label-ontology-observation-body.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.record-label-ontology-observation.response",
        "POST /api/v1/tasks/:task_id/label-ontology/observations success",
        "POST /api/v1/tasks/:task_id/label-ontology/observations",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:record-label-ontology-observation-response:v1",
        "schemas/fixtures/api/record-label-ontology-observation-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.list-label-ontology-signals.path",
        "GET /api/v1/boards/:board/label-ontology/signals path",
        "GET /api/v1/boards/:board/label-ontology/signals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-label-ontology-signals-path:v1",
        "schemas/fixtures/api/list-label-ontology-signals-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.label-ontology-signal.query",
        "GET /api/v1/boards/:board/label-ontology/signals query",
        "GET /api/v1/boards/:board/label-ontology/signals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:label-ontology-signal-query:v1",
        "schemas/fixtures/api/label-ontology-signal-query.v1.valid.json",
        HttpTransportLocation::Query,
        &[]
    ),
    generated_api_contract!(
        "api.review-label-ontology.path",
        "GET /api/v1/boards/:board/label-ontology/review path",
        "GET /api/v1/boards/:board/label-ontology/review",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:review-label-ontology-path:v1",
        "schemas/fixtures/api/review-label-ontology-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.label-ontology-review.query",
        "GET /api/v1/boards/:board/label-ontology/review query",
        "GET /api/v1/boards/:board/label-ontology/review",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:label-ontology-review-query:v1",
        "schemas/fixtures/api/label-ontology-review-query.v1.valid.json",
        HttpTransportLocation::Query,
        &[]
    ),
    generated_api_contract!(
        "api.review-label-ontology.response",
        "GET /api/v1/boards/:board/label-ontology/review success",
        "GET /api/v1/boards/:board/label-ontology/review",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:review-label-ontology-response:v1",
        "schemas/fixtures/api/review-label-ontology-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.create-label-ontology-action.path",
        "POST /api/v1/boards/:board/label-ontology/actions path",
        "POST /api/v1/boards/:board/label-ontology/actions",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-label-ontology-action-path:v1",
        "schemas/fixtures/api/create-label-ontology-action-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.create-label-ontology-action.request",
        "POST /api/v1/boards/:board/label-ontology/actions body",
        "POST /api/v1/boards/:board/label-ontology/actions",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-label-ontology-action-request:v1",
        "schemas/fixtures/api/create-label-ontology-action-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.create-label-ontology-action.response",
        "POST /api/v1/boards/:board/label-ontology/actions success",
        "POST /api/v1/boards/:board/label-ontology/actions",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:create-label-ontology-action-response:v1",
        "schemas/fixtures/api/create-label-ontology-action-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.apply-label-ontology-atom.path",
        "POST /api/v1/boards/:board/label-ontology/apply/atom path",
        "POST /api/v1/boards/:board/label-ontology/apply/atom",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-path:v1",
        "schemas/fixtures/api/apply-label-ontology-atom-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.apply-label-ontology-atom.request",
        "POST /api/v1/boards/:board/label-ontology/apply/atom body",
        "POST /api/v1/boards/:board/label-ontology/apply/atom",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-request:v1",
        "schemas/fixtures/api/apply-label-ontology-atom-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.apply-label-ontology-atom.response",
        "POST /api/v1/boards/:board/label-ontology/apply/atom success",
        "POST /api/v1/boards/:board/label-ontology/apply/atom",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:apply-label-ontology-atom-response:v1",
        "schemas/fixtures/api/apply-label-ontology-atom-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.revert-label-ontology-mutation.path",
        "POST /api/v1/boards/:board/label-ontology/revert path",
        "POST /api/v1/boards/:board/label-ontology/revert",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-path:v1",
        "schemas/fixtures/api/revert-label-ontology-mutation-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.revert-label-ontology-mutation.request",
        "POST /api/v1/boards/:board/label-ontology/revert body",
        "POST /api/v1/boards/:board/label-ontology/revert",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-request:v1",
        "schemas/fixtures/api/revert-label-ontology-mutation-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.revert-label-ontology-mutation.response",
        "POST /api/v1/boards/:board/label-ontology/revert success",
        "POST /api/v1/boards/:board/label-ontology/revert",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:revert-label-ontology-mutation-response:v1",
        "schemas/fixtures/api/revert-label-ontology-mutation-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.validate-label-ontology-action.path",
        "POST /api/v1/boards/:board/label-ontology/validate path",
        "POST /api/v1/boards/:board/label-ontology/validate",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:validate-label-ontology-action-path:v1",
        "schemas/fixtures/api/validate-label-ontology-action-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "board",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.validate-label-ontology-action.request",
        "POST /api/v1/boards/:board/label-ontology/validate body",
        "POST /api/v1/boards/:board/label-ontology/validate",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:validate-label-ontology-action-request:v1",
        "schemas/fixtures/api/validate-label-ontology-action-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.validate-label-ontology-action.response",
        "POST /api/v1/boards/:board/label-ontology/validate success",
        "POST /api/v1/boards/:board/label-ontology/validate",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:validate-label-ontology-action-response:v1",
        "schemas/fixtures/api/validate-label-ontology-action-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.get-label-ontology-signal.path",
        "GET /api/v1/label-ontology/signals/:signal_id path",
        "GET /api/v1/label-ontology/signals/:signal_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:get-label-ontology-signal-path:v1",
        "schemas/fixtures/api/get-label-ontology-signal-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "signal_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.get-label-ontology-signal.response",
        "GET /api/v1/label-ontology/signals/:signal_id success",
        "GET /api/v1/label-ontology/signals/:signal_id",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:get-label-ontology-signal-response:v1",
        "schemas/fixtures/api/get-label-ontology-signal-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.get-label-proposal.path",
        "GET /api/v1/label-proposals/:proposal_id path",
        "GET /api/v1/label-proposals/:proposal_id",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:get-label-proposal-path:v1",
        "schemas/fixtures/api/get-label-proposal-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "proposal_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.get-label-proposal.response",
        "GET /api/v1/label-proposals/:proposal_id success",
        "GET /api/v1/label-proposals/:proposal_id",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:get-label-proposal-response:v1",
        "schemas/fixtures/api/get-label-proposal-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.accept-label-proposal.path",
        "POST /api/v1/label-proposals/:proposal_id/accept path",
        "POST /api/v1/label-proposals/:proposal_id/accept",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:accept-label-proposal-path:v1",
        "schemas/fixtures/api/accept-label-proposal-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "proposal_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.accept-label-proposal.body",
        "POST /api/v1/label-proposals/:proposal_id/accept body",
        "POST /api/v1/label-proposals/:proposal_id/accept",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:accept-label-proposal-body:v1",
        "schemas/fixtures/api/accept-label-proposal-body.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.accept-label-proposal.response",
        "POST /api/v1/label-proposals/:proposal_id/accept success",
        "POST /api/v1/label-proposals/:proposal_id/accept",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:accept-label-proposal-response:v1",
        "schemas/fixtures/api/accept-label-proposal-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.reject-label-proposal.path",
        "POST /api/v1/label-proposals/:proposal_id/reject path",
        "POST /api/v1/label-proposals/:proposal_id/reject",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:reject-label-proposal-path:v1",
        "schemas/fixtures/api/reject-label-proposal-path.v1.valid.json",
        HttpTransportLocation::Path,
        &[WireParameter {
            name: "proposal_id",
            cardinality: Some(WireParameterCardinality::RequiredOne)
        }]
    ),
    generated_api_contract!(
        "api.reject-label-proposal.body",
        "POST /api/v1/label-proposals/:proposal_id/reject body",
        "POST /api/v1/label-proposals/:proposal_id/reject",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:reject-label-proposal-body:v1",
        "schemas/fixtures/api/reject-label-proposal-body.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.reject-label-proposal.response",
        "POST /api/v1/label-proposals/:proposal_id/reject success",
        "POST /api/v1/label-proposals/:proposal_id/reject",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:reject-label-proposal-response:v1",
        "schemas/fixtures/api/reject-label-proposal-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    OperationContract {
        id: "sse.stream-events.query",
        path: "GET /api/v1/stream/events query",
        surface: ContractSurface::Sse,
        operation: "GET /api/v1/stream/events",
        direction: ContractDirection::Deserialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: Some("urn:kanban-tool:schema:sse:stream-events-query:v1"),
        fixture: Some("schemas/fixtures/sse/stream-events-query.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/sse/stream-events-query.v1.valid.json",
            producer: AdoptionWitness {
                operation: "GET /api/v1/stream/events",
                contract_id: "sse.stream-events.query",
                surface: ContractSurface::Sse,
                direction: ContractDirection::Deserialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::sse_adoption::stream_events_query_dto_serializes_to_committed_fixture",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/stream/events",
                contract_id: "sse.stream-events.query",
                surface: ContractSurface::Sse,
                direction: ContractDirection::Deserialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::sse_adoption::stream_events_query_fixture_is_consumed_by_real_router",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::Http {
            operation_key: Some("GET /api/v1/stream/events"),
            location: HttpTransportLocation::Query,
            parameters: &[
                WireParameter {
                    name: "board",
                    cardinality: Some(WireParameterCardinality::OptionalOne),
                },
                WireParameter {
                    name: "task_id",
                    cardinality: Some(WireParameterCardinality::OptionalOne),
                },
                WireParameter {
                    name: "after",
                    cardinality: Some(WireParameterCardinality::OptionalOne),
                },
                WireParameter {
                    name: "limit",
                    cardinality: Some(WireParameterCardinality::OptionalOne),
                },
            ],
        },
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "sse.event.data",
        path: "GET /api/v1/stream/events data",
        surface: ContractSurface::Sse,
        operation: "GET /api/v1/stream/events",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: Some("urn:kanban-tool:schema:sse:stream-event-data:v1"),
        fixture: Some("schemas/fixtures/sse/stream-event-data.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/sse/stream-event-data.v1.valid.json",
            producer: AdoptionWitness {
                operation: "GET /api/v1/stream/events",
                contract_id: "sse.event.data",
                surface: ContractSurface::Sse,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::sse_adoption::stream_event_data_fixture_is_produced_by_real_router",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/stream/events",
                contract_id: "sse.event.data",
                surface: ContractSurface::Sse,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "all",
                exact_test: "suite::sse_adoption::stream_event_data_fixture_is_consumed_by_contract_root",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::Http {
            operation_key: Some("GET /api/v1/stream/events"),
            location: HttpTransportLocation::Sse,
            parameters: &[],
        },
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "metadata.decision.input",
        path: "task_comments.metadata_json(kind=decision)",
        surface: ContractSurface::Metadata,
        operation: "structured decision comment metadata input",
        direction: ContractDirection::Deserialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::Typed,
        schema_id: Some("urn:kanban-tool:schema:metadata:decision:v1"),
        fixture: Some("schemas/fixtures/metadata/decision.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/metadata/decision.v1.valid.json",
            producer: AdoptionWitness {
                operation: "structured decision comment metadata input",
                contract_id: "metadata.decision.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "metadata_contract_adoption",
                exact_test: "metadata_decision_input_fixture_is_produced_by_cli_contract_dto",
            },
            consumer: AdoptionWitness {
                operation: "structured decision comment metadata input",
                contract_id: "metadata.decision.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "comments",
                exact_test: "metadata_decision_input_fixture_is_consumed_by_real_cli",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "metadata.signal-record.input",
        path: "kanban signal record input",
        surface: ContractSurface::Metadata,
        operation: "generic signal record input",
        direction: ContractDirection::Deserialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::Typed,
        schema_id: Some("urn:kanban-tool:schema:metadata:signal-record-input:v1"),
        fixture: Some("schemas/fixtures/metadata/signal-record-input.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/metadata/signal-record-input.v1.valid.json",
            producer: AdoptionWitness {
                operation: "generic signal record input",
                contract_id: "metadata.signal-record.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "metadata_contract_adoption",
                exact_test: "metadata_signal_record_input_fixture_is_produced_by_cli_contract_dto",
            },
            consumer: AdoptionWitness {
                operation: "generic signal record input",
                contract_id: "metadata.signal-record.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "signal",
                exact_test: "metadata_signal_record_input_fixture_is_consumed_by_real_cli",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "metadata.signal-link.output",
        path: "task comment signal backlink metadata",
        surface: ContractSurface::Metadata,
        operation: "signal backlink comment metadata output",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::Typed,
        schema_id: Some("urn:kanban-tool:schema:metadata:signal-link-output:v1"),
        fixture: Some("schemas/fixtures/metadata/signal-link-output.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/metadata/signal-link-output.v1.valid.json",
            producer: AdoptionWitness {
                operation: "signal backlink comment metadata output",
                contract_id: "metadata.signal-link.output",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Serialize,
                package: "kanban-cli",
                test_target: "signal",
                exact_test: "metadata_signal_link_output_fixture_is_produced_by_real_service_adapter",
            },
            consumer: AdoptionWitness {
                operation: "signal backlink comment metadata output",
                contract_id: "metadata.signal-link.output",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Serialize,
                package: "kanban-cli",
                test_target: "metadata_contract_adoption",
                exact_test: "metadata_signal_link_output_fixture_is_consumed_by_cli_contract_dto",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "metadata.label-proposal-candidate.input",
        path: "kanban label propose --proposal-json",
        surface: ContractSurface::Metadata,
        operation: "label proposal candidate input",
        direction: ContractDirection::Deserialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::Typed,
        schema_id: Some("urn:kanban-tool:schema:metadata:label-proposal-candidate-input:v1"),
        fixture: Some("schemas/fixtures/metadata/label-proposal-candidate-input.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/metadata/label-proposal-candidate-input.v1.valid.json",
            producer: AdoptionWitness {
                operation: "label proposal candidate input",
                contract_id: "metadata.label-proposal-candidate.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "metadata_contract_adoption",
                exact_test: "metadata_label_proposal_candidate_input_fixture_is_produced_by_cli_contract_dto",
            },
            consumer: AdoptionWitness {
                operation: "label proposal candidate input",
                contract_id: "metadata.label-proposal-candidate.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "task",
                exact_test: "metadata_label_proposal_candidate_input_fixture_is_consumed_by_real_cli",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "metadata.ontology-record.input",
        path: "kanban label ontology record input",
        surface: ContractSurface::Metadata,
        operation: "label ontology observation input",
        direction: ContractDirection::Deserialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::Typed,
        schema_id: Some("urn:kanban-tool:schema:metadata:ontology-record-input:v1"),
        fixture: Some("schemas/fixtures/metadata/ontology-record-input.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/metadata/ontology-record-input.v1.valid.json",
            producer: AdoptionWitness {
                operation: "label ontology observation input",
                contract_id: "metadata.ontology-record.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "metadata_contract_adoption",
                exact_test: "metadata_ontology_record_input_fixture_is_produced_by_cli_contract_dto",
            },
            consumer: AdoptionWitness {
                operation: "label ontology observation input",
                contract_id: "metadata.ontology-record.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "task",
                exact_test: "metadata_ontology_record_input_fixture_is_consumed_by_real_cli",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "metadata.ontology-validation-evidence.input",
        path: "kanban label ontology validate external evidence",
        surface: ContractSurface::Metadata,
        operation: "label ontology external validation evidence",
        direction: ContractDirection::Deserialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::OpaqueExtension,
        schema_id: Some("urn:kanban-tool:schema:metadata:ontology-validation-evidence-input:v1"),
        fixture: Some("schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json"),
        adoption: Some(AdoptionEvidence {
            producer_fixture: "schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json",
            producer: AdoptionWitness {
                operation: "label ontology external validation evidence",
                contract_id: "metadata.ontology-validation-evidence.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "metadata_contract_adoption",
                exact_test: "metadata_ontology_validation_evidence_input_fixture_is_produced_by_cli_contract_dto",
            },
            consumer: AdoptionWitness {
                operation: "label ontology external validation evidence",
                contract_id: "metadata.ontology-validation-evidence.input",
                surface: ContractSurface::Metadata,
                direction: ContractDirection::Deserialize,
                package: "kanban-cli",
                test_target: "cli_label_contract_adoption",
                exact_test: "metadata_ontology_validation_evidence_input_fixture_is_consumed_by_real_cli",
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "config.project.input",
        path: ".kb/config.toml",
        surface: ContractSurface::Config,
        operation: "project-local config after TOML decoding",
        direction: ContractDirection::Deserialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "config.selected-worker-profile.input",
        path: "selected [workers.<profile>] section",
        surface: ContractSurface::Config,
        operation: "selected dispatcher worker profile after TOML decoding",
        direction: ContractDirection::Deserialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.graph.handshake.response",
        path: "kanban-graph-oxigraph handshake stdout",
        surface: ContractSurface::Helper,
        operation: "graph helper handshake response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.graph.error.response",
        path: "kanban-graph-oxigraph error stdout",
        surface: ContractSurface::Helper,
        operation: "graph helper error response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.graph.status.response",
        path: "kanban-graph-oxigraph status stdout",
        surface: ContractSurface::Helper,
        operation: "graph helper status response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.graph.rebuild.response",
        path: "kanban-graph-oxigraph rebuild stdout",
        surface: ContractSurface::Helper,
        operation: "graph helper rebuild response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.graph.sync.response",
        path: "kanban-graph-oxigraph sync stdout",
        surface: ContractSurface::Helper,
        operation: "graph helper sync response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.graph.neighbors.response",
        path: "kanban-graph-oxigraph neighbors stdout",
        surface: ContractSurface::Helper,
        operation: "graph helper neighbors response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.graph.query.response",
        path: "kanban-graph-oxigraph query stdout",
        surface: ContractSurface::Helper,
        operation: "graph helper query response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.handshake.response",
        path: "kanban-vector-lancedb handshake stdout",
        surface: ContractSurface::Helper,
        operation: "vector helper handshake response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.error.response",
        path: "kanban-vector-lancedb error stdout",
        surface: ContractSurface::Helper,
        operation: "vector helper error response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.check-provider.response",
        path: "kanban-vector-lancedb check-provider stdout",
        surface: ContractSurface::Helper,
        operation: "vector provider availability response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.status.response",
        path: "kanban-vector-lancedb status stdout",
        surface: ContractSurface::Helper,
        operation: "vector helper status response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.rebuild.response",
        path: "kanban-vector-lancedb rebuild stdout",
        surface: ContractSurface::Helper,
        operation: "vector helper rebuild response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.sync.response",
        path: "kanban-vector-lancedb sync stdout",
        surface: ContractSurface::Helper,
        operation: "vector helper sync response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.label-atoms-status.response",
        path: "kanban-vector-lancedb label-atoms-status stdout",
        surface: ContractSurface::Helper,
        operation: "label atom vector status response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.rebuild-label-atoms.response",
        path: "kanban-vector-lancedb rebuild-label-atoms stdout",
        surface: ContractSurface::Helper,
        operation: "label atom vector rebuild response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.sync-label-atoms.response",
        path: "kanban-vector-lancedb sync-label-atoms stdout",
        surface: ContractSurface::Helper,
        operation: "label atom vector sync response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.query-chunks.response",
        path: "kanban-vector-lancedb query-chunks stdout",
        surface: ContractSurface::Helper,
        operation: "vector chunk query response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.query-label-atoms.response",
        path: "kanban-vector-lancedb query-label-atoms stdout",
        surface: ContractSurface::Helper,
        operation: "label atom vector query response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    OperationContract {
        id: "helper.vector.embed-query.response",
        path: "kanban-vector-lancedb embed-query stdout",
        surface: ContractSurface::Helper,
        operation: "vector embedding response",
        direction: ContractDirection::Serialize,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: None,
        fixture: None,
        adoption: None,
        exclusion: None,
        migration: MigrationState::Planned,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    },
    adopted_api_parameter_contract!(
        "api.board-task-map.path",
        "GET /api/v1/boards/:board/task-map path",
        "GET /api/v1/boards/:board/task-map",
        "urn:kanban-tool:schema:api:board-task-map-path:v1",
        "schemas/fixtures/api/board-task-map-path.v1.valid.json",
        "suite::task_graph_adoption::board_task_map_path_dto_serializes_to_committed_fixture",
        "suite::task_graph_adoption::board_task_map_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        GRAPH_BOARD_PATH_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.board-task-map.query",
        "GET /api/v1/boards/:board/task-map query",
        "GET /api/v1/boards/:board/task-map",
        "urn:kanban-tool:schema:api:board-task-map-query:v1",
        "schemas/fixtures/api/board-task-map-query.v1.valid.json",
        "suite::task_graph_adoption::board_task_map_query_dto_serializes_to_committed_fixture",
        "suite::task_graph_adoption::board_task_map_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        GRAPH_BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.board-task-map.response",
        "GET /api/v1/boards/:board/task-map response",
        "GET /api/v1/boards/:board/task-map",
        "urn:kanban-tool:schema:api:board-task-map-response:v1",
        "schemas/fixtures/api/board-task-map-response.v1.valid.json",
        "suite::task_graph_adoption::board_task_map_response_fixture_is_produced_by_real_router",
        "suite::task_graph_adoption::board_task_map_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.get-task.path",
        "GET /api/v1/tasks/:task_id path",
        "GET /api/v1/tasks/:task_id",
        "urn:kanban-tool:schema:api:get-task-path:v1",
        "schemas/fixtures/api/get-task-path.v1.valid.json",
        "suite::task_core_adoption::get_task_path_dto_serializes_to_committed_fixture",
        "suite::task_core_adoption::get_task_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        TASK_CORE_PATH_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.get-task.query",
        "GET /api/v1/tasks/:task_id query",
        "GET /api/v1/tasks/:task_id",
        "urn:kanban-tool:schema:api:get-task-query:v1",
        "schemas/fixtures/api/get-task-query.v1.valid.json",
        "suite::task_core_adoption::get_task_query_dto_serializes_to_committed_fixture",
        "suite::task_core_adoption::get_task_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        GET_TASK_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.get-task.response",
        "GET /api/v1/tasks/:task_id response",
        "GET /api/v1/tasks/:task_id",
        "urn:kanban-tool:schema:api:get-task-response:v1",
        "schemas/fixtures/api/get-task-response.v1.valid.json",
        "suite::task_core_adoption::get_task_response_fixture_is_produced_by_real_router",
        "suite::task_core_adoption::get_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.update-task.path",
        "PATCH /api/v1/tasks/:task_id path",
        "PATCH /api/v1/tasks/:task_id",
        "urn:kanban-tool:schema:api:update-task-path:v1",
        "schemas/fixtures/api/update-task-path.v1.valid.json",
        "suite::task_core_adoption::update_task_path_dto_serializes_to_committed_fixture",
        "suite::task_core_adoption::update_task_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        TASK_CORE_PATH_PARAMETERS
    ),
    adopted_api_request!(
        "api.update-task.request",
        "PATCH /api/v1/tasks/:task_id body",
        "PATCH /api/v1/tasks/:task_id",
        "urn:kanban-tool:schema:api:update-task-request:v1",
        "schemas/fixtures/api/update-task-request.v1.valid.json",
        "suite::task_core_adoption::update_task_request_dto_serializes_to_committed_fixture",
        "suite::task_core_adoption::update_task_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_response_contract!(
        "api.update-task.response",
        "PATCH /api/v1/tasks/:task_id response",
        "PATCH /api/v1/tasks/:task_id",
        "urn:kanban-tool:schema:api:update-task-response:v1",
        "schemas/fixtures/api/update-task-response.v1.valid.json",
        "suite::task_core_adoption::update_task_response_fixture_is_produced_by_real_router",
        "suite::task_core_adoption::update_task_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.task-neighborhood.path",
        "GET /api/v1/tasks/:task_id/neighborhood path",
        "GET /api/v1/tasks/:task_id/neighborhood",
        "urn:kanban-tool:schema:api:task-neighborhood-path:v1",
        "schemas/fixtures/api/task-neighborhood-path.v1.valid.json",
        "suite::task_graph_adoption::task_neighborhood_path_dto_serializes_to_committed_fixture",
        "suite::task_graph_adoption::task_neighborhood_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        GRAPH_TASK_PATH_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.task-neighborhood.query",
        "GET /api/v1/tasks/:task_id/neighborhood query",
        "GET /api/v1/tasks/:task_id/neighborhood",
        "urn:kanban-tool:schema:api:task-neighborhood-query:v1",
        "schemas/fixtures/api/task-neighborhood-query.v1.valid.json",
        "suite::task_graph_adoption::task_neighborhood_query_dto_serializes_to_committed_fixture",
        "suite::task_graph_adoption::task_neighborhood_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        GRAPH_TASK_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.task-neighborhood.response",
        "GET /api/v1/tasks/:task_id/neighborhood response",
        "GET /api/v1/tasks/:task_id/neighborhood",
        "urn:kanban-tool:schema:api:task-neighborhood-response:v1",
        "schemas/fixtures/api/task-neighborhood-response.v1.valid.json",
        "suite::task_graph_adoption::task_neighborhood_response_fixture_is_produced_by_real_router",
        "suite::task_graph_adoption::task_neighborhood_response_fixture_is_consumed_by_contract_root"
    ),
];

pub fn operation_inventory() -> &'static [OperationContract] {
    static INVENTORY: std::sync::OnceLock<Vec<OperationContract>> = std::sync::OnceLock::new();
    INVENTORY
        .get_or_init(|| {
            let mut inventory = OPERATION_INVENTORY.to_vec();
            adopt_phase5_api_contracts(&mut inventory);
            adopt_protocol_contracts(&mut inventory);
            inventory.extend(vector_projection_protocol_contracts());
            inventory.extend(portable_operation_contracts());
            inventory.extend(crate::headers::header_operation_contracts());
            inventory
        })
        .as_slice()
}

fn vector_projection_protocol_contracts() -> [OperationContract; 2] {
    [
        OperationContract {
            id: "helper.vector-projection.request",
            path: "kanban-vector-lancedb projection stdin",
            surface: ContractSurface::Helper,
            operation: "vector projection helper protocol",
            direction: ContractDirection::Deserialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some("urn:kanban-tool:schema:helper:vector-projection-request:v2"),
            fixture: Some("schemas/fixtures/helper/vector-projection-request.v2.valid.json"),
            adoption: Some(AdoptionEvidence {
                producer_fixture: "schemas/fixtures/helper/vector-projection-request.v2.valid.json",
                producer: AdoptionWitness {
                    operation: "vector projection helper protocol",
                    contract_id: "helper.vector-projection.request",
                    surface: ContractSurface::Helper,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-vector-lancedb",
                    test_target: "vector_projection_contract_adoption",
                    exact_test: "vector_projection_request_fixture_is_produced_by_contract_dto",
                },
                consumer: AdoptionWitness {
                    operation: "vector projection helper protocol",
                    contract_id: "helper.vector-projection.request",
                    surface: ContractSurface::Helper,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-vector-lancedb",
                    test_target: "vector_projection_contract_adoption",
                    exact_test: "vector_projection_request_fixture_is_consumed_by_real_projection_handler",
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::NoTransport,
            binding: ContractBinding::ExactSurface,
        },
        OperationContract {
            id: "helper.vector-projection.response",
            path: "kanban-vector-lancedb projection stdout",
            surface: ContractSurface::Helper,
            operation: "vector projection helper protocol",
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some("urn:kanban-tool:schema:helper:vector-projection-response:v1"),
            fixture: Some("schemas/fixtures/helper/vector-projection-response.v1.valid.json"),
            adoption: Some(AdoptionEvidence {
                producer_fixture: "schemas/fixtures/helper/vector-projection-response.v1.valid.json",
                producer: AdoptionWitness {
                    operation: "vector projection helper protocol",
                    contract_id: "helper.vector-projection.response",
                    surface: ContractSurface::Helper,
                    direction: ContractDirection::Serialize,
                    package: "kanban-vector-lancedb",
                    test_target: "vector_projection_contract_adoption",
                    exact_test: "vector_projection_response_fixture_is_produced_by_real_projection_handler",
                },
                consumer: AdoptionWitness {
                    operation: "vector projection helper protocol",
                    contract_id: "helper.vector-projection.response",
                    surface: ContractSurface::Helper,
                    direction: ContractDirection::Serialize,
                    package: "kanban-vector-lancedb",
                    test_target: "vector_projection_contract_adoption",
                    exact_test: "vector_projection_response_fixture_is_consumed_by_runtime_protocol_decoder",
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::NoTransport,
            binding: ContractBinding::ExactSurface,
        },
    ]
}

macro_rules! phase5_api_request_contract {
    ($id:literal, $consumer_test:ident) => {
        (
            $id,
            concat!(
                "suite::api_generated_adoption::",
                stringify!($consumer_test)
            ),
        )
    };
}

const PHASE5_API_REQUEST_CONTRACTS: [(&str, &str); 48] = [
    phase5_api_request_contract!(
        "api.list-board-labels.path",
        list_board_labels_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.create-board-label.path",
        create_board_label_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.create-board-label.request",
        create_board_label_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.list-label-semantics.path",
        list_label_semantics_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.get-label-semantics.path",
        get_label_semantics_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.upsert-label-semantics.path",
        upsert_label_semantics_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.upsert-label-semantics.request",
        upsert_label_semantics_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.delete-label-semantics.path",
        delete_label_semantics_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.delete-label-semantics.query",
        delete_label_semantics_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.list-label-atoms.path",
        list_label_atoms_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.label-atom.path",
        explain_label_atom_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.label-atom-index-status.path",
        label_atom_index_status_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.rebuild-label-atom-index.path",
        rebuild_label_atom_index_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.query-label-atom-index.path",
        query_label_atom_index_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.query-label-atom-index.query",
        query_label_atom_index_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.list-signals.path",
        list_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.list-signals.query",
        list_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.review-signals.path",
        review_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.review-signals.query",
        review_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.get-signal.path",
        get_signal_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.bootstrap-task-label.path",
        bootstrap_task_label_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.bootstrap-task-label.request",
        bootstrap_task_label_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.suggest-task-labels.path",
        suggest_task_labels_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.label-suggestion.query",
        suggest_task_labels_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.list-task-label-proposals.path",
        list_task_label_proposals_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.propose-task-label.path",
        propose_task_label_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.propose-task-label.query",
        propose_task_label_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.propose-task-label.request",
        propose_task_label_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.record-label-ontology-observation.path",
        record_label_ontology_observation_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.record-label-ontology-observation.body",
        record_label_ontology_observation_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.list-label-ontology-signals.path",
        list_label_ontology_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.label-ontology-signal.query",
        list_label_ontology_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.review-label-ontology.path",
        review_label_ontology_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.label-ontology-review.query",
        review_label_ontology_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.create-label-ontology-action.path",
        create_label_ontology_action_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.create-label-ontology-action.request",
        create_label_ontology_action_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.apply-label-ontology-atom.path",
        apply_label_ontology_atom_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.apply-label-ontology-atom.request",
        apply_label_ontology_atom_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.revert-label-ontology-mutation.path",
        revert_label_ontology_mutation_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.revert-label-ontology-mutation.request",
        revert_label_ontology_mutation_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.validate-label-ontology-action.path",
        validate_label_ontology_action_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.validate-label-ontology-action.request",
        validate_label_ontology_action_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.get-label-ontology-signal.path",
        get_label_ontology_signal_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.get-label-proposal.path",
        get_label_proposal_request_fixture_reaches_handler
    ),
    phase5_api_request_contract!(
        "api.accept-label-proposal.path",
        accept_label_proposal_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.accept-label-proposal.body",
        accept_label_proposal_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.reject-label-proposal.path",
        reject_label_proposal_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.reject-label-proposal.body",
        reject_label_proposal_request_fixtures_reach_handler
    ),
];

const PHASE5_API_RESPONSE_CONTRACTS: [(&str, &str); 27] = [
    (
        "api.list-board-labels.response",
        "suite::api_generated_adoption::generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.create-board-label.response",
        "suite::api_generated_adoption::generated_label_responses_are_produced_by_real_router",
    ),
    (
        "api.list-label-semantics.response",
        "suite::api_generated_adoption::generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.get-label-semantics.response",
        "suite::api_generated_adoption::generated_label_responses_are_produced_by_real_router",
    ),
    (
        "api.upsert-label-semantics.response",
        "suite::api_generated_adoption::generated_label_responses_are_produced_by_real_router",
    ),
    (
        "api.list-label-atoms.response",
        "suite::api_generated_adoption::generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.explain-label-atom.response",
        "suite::api_generated_adoption::generated_label_responses_are_produced_by_real_router",
    ),
    (
        "api.label-atom-index-status.response",
        "suite::api_generated_adoption::generated_atom_index_responses_are_produced_by_real_router",
    ),
    (
        "api.rebuild-label-atom-index.response",
        "suite::api_generated_adoption::generated_atom_index_responses_are_produced_by_real_router",
    ),
    (
        "api.query-label-atom-index.response",
        "suite::api_generated_adoption::generated_atom_index_responses_are_produced_by_real_router",
    ),
    (
        "api.list-signals.response",
        "suite::api_generated_adoption::generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.review-signals.response",
        "suite::api_generated_adoption::generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.get-signal.response",
        "suite::api_generated_adoption::generated_generic_signal_response_is_produced_by_real_router",
    ),
    (
        "api.bootstrap-task-label.response",
        "suite::api_generated_adoption::generated_task_label_responses_are_produced_by_real_router",
    ),
    (
        "api.suggest-task-labels.response",
        "suite::api_generated_adoption::generated_task_label_responses_are_produced_by_real_router",
    ),
    (
        "api.list-task-label-proposals.response",
        "suite::api_generated_adoption::generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.propose-task-label.response",
        "suite::api_generated_adoption::generated_task_label_responses_are_produced_by_real_router",
    ),
    (
        "api.record-label-ontology-observation.response",
        "suite::api_generated_adoption::generated_ontology_observation_responses_are_produced_by_real_router",
    ),
    (
        "api.review-label-ontology.response",
        "suite::api_generated_adoption::generated_empty_collection_responses_are_produced_by_real_router",
    ),
    (
        "api.create-label-ontology-action.response",
        "suite::api_generated_adoption::generated_ontology_action_responses_are_produced_by_real_router",
    ),
    (
        "api.apply-label-ontology-atom.response",
        "suite::api_generated_adoption::generated_ontology_action_responses_are_produced_by_real_router",
    ),
    (
        "api.revert-label-ontology-mutation.response",
        "suite::api_generated_adoption::generated_ontology_action_responses_are_produced_by_real_router",
    ),
    (
        "api.validate-label-ontology-action.response",
        "suite::api_generated_adoption::generated_ontology_action_responses_are_produced_by_real_router",
    ),
    (
        "api.get-label-ontology-signal.response",
        "suite::api_generated_adoption::generated_ontology_observation_responses_are_produced_by_real_router",
    ),
    (
        "api.get-label-proposal.response",
        "suite::api_generated_adoption::generated_proposal_responses_are_produced_by_real_router",
    ),
    (
        "api.accept-label-proposal.response",
        "suite::api_generated_adoption::generated_proposal_responses_are_produced_by_real_router",
    ),
    (
        "api.reject-label-proposal.response",
        "suite::api_generated_adoption::generated_proposal_responses_are_produced_by_real_router",
    ),
];

fn adopt_phase5_api_contracts(inventory: &mut [OperationContract]) {
    for contract in inventory {
        let Some((producer_test, consumer_test)) = phase5_api_adoption_tests(contract.id) else {
            continue;
        };
        let fixture = contract
            .fixture
            .expect("phase 5 API adoption target must own a fixture");
        contract.adoption = Some(AdoptionEvidence {
            producer_fixture: fixture,
            producer: AdoptionWitness {
                operation: contract.operation,
                contract_id: contract.id,
                surface: contract.surface,
                direction: contract.direction,
                package: "kanban-server",
                test_target: "all",
                exact_test: producer_test,
            },
            consumer: AdoptionWitness {
                operation: contract.operation,
                contract_id: contract.id,
                surface: contract.surface,
                direction: contract.direction,
                package: "kanban-server",
                test_target: "all",
                exact_test: consumer_test,
            },
        });
        contract.migration = MigrationState::Adopted;
    }
}

fn phase5_api_adoption_tests(id: &str) -> Option<(&'static str, &'static str)> {
    if let Some((_, consumer_test)) = PHASE5_API_REQUEST_CONTRACTS
        .iter()
        .find(|(contract_id, _)| *contract_id == id)
    {
        return Some((
            "suite::api_generated_adoption::api_generated_request_dtos_serialize_to_committed_fixtures",
            *consumer_test,
        ));
    }
    PHASE5_API_RESPONSE_CONTRACTS
        .iter()
        .find(|(contract_id, _)| *contract_id == id)
        .map(|(_, producer_test)| {
            (
                *producer_test,
                "suite::api_generated_adoption::api_generated_response_fixtures_are_consumed_by_contract_roots",
            )
        })
}

fn adopt_protocol_contracts(inventory: &mut [OperationContract]) {
    for contract in inventory {
        let Some((schema_id, fixture, package, test_target, producer_test, consumer_test)) =
            protocol_adoption_spec(contract.id)
        else {
            continue;
        };
        contract.schema_id = Some(schema_id);
        contract.fixture = Some(fixture);
        contract.adoption = Some(AdoptionEvidence {
            producer_fixture: fixture,
            producer: AdoptionWitness {
                operation: contract.operation,
                contract_id: contract.id,
                surface: contract.surface,
                direction: contract.direction,
                package,
                test_target,
                exact_test: producer_test,
            },
            consumer: AdoptionWitness {
                operation: contract.operation,
                contract_id: contract.id,
                surface: contract.surface,
                direction: contract.direction,
                package,
                test_target,
                exact_test: consumer_test,
            },
        });
        contract.migration = MigrationState::Adopted;
    }
}

type ProtocolAdoptionSpec = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

fn protocol_adoption_spec(id: &str) -> Option<ProtocolAdoptionSpec> {
    let spec = match id {
        "config.project.input" => (
            "urn:kanban-tool:schema:config:project-input:v1",
            "schemas/fixtures/config/project-input.v1.valid.json",
            "kanban-local",
            "lib",
            "tests::project_config_input_fixture_is_produced_by_runtime_config_dto",
            "tests::project_config_input_fixture_is_consumed_by_real_toml_decoder",
        ),
        "config.selected-worker-profile.input" => (
            "urn:kanban-tool:schema:config:selected-worker-profile-input:v1",
            "schemas/fixtures/config/selected-worker-profile-input.v1.valid.json",
            "kanban-local",
            "lib",
            "tests::selected_worker_profile_input_fixture_is_produced_by_runtime_config_dto",
            "tests::selected_worker_profile_input_fixture_is_consumed_by_real_toml_decoder",
        ),
        "helper.graph.handshake.response" => graph_protocol_spec(
            "graph-handshake-response",
            "graph_helper_handshake_response_fixture_is_produced_by_real_helper_adapter",
            "graph_helper_handshake_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.graph.error.response" => graph_protocol_spec(
            "graph-error-response",
            "graph_helper_error_response_fixture_is_produced_by_real_helper_adapter",
            "graph_helper_error_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.graph.status.response" => graph_protocol_spec(
            "graph-status-response",
            "graph_helper_status_response_fixture_is_produced_by_real_helper_adapter",
            "graph_helper_status_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.graph.rebuild.response" => graph_protocol_spec(
            "graph-rebuild-response",
            "graph_helper_rebuild_response_fixture_is_produced_by_real_helper_adapter",
            "graph_helper_rebuild_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.graph.sync.response" => graph_protocol_spec(
            "graph-sync-response",
            "graph_helper_sync_response_fixture_is_produced_by_real_helper_adapter",
            "graph_helper_sync_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.graph.neighbors.response" => graph_protocol_spec(
            "graph-neighbors-response",
            "graph_helper_neighbors_response_fixture_is_produced_by_real_helper_adapter",
            "graph_helper_neighbors_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.graph.query.response" => graph_protocol_spec(
            "graph-query-response",
            "graph_helper_query_response_fixture_is_produced_by_real_helper_adapter",
            "graph_helper_query_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.handshake.response" => vector_protocol_spec(
            "vector-handshake-response",
            "vector_helper_handshake_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_handshake_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.error.response" => vector_protocol_spec(
            "vector-error-response",
            "vector_helper_error_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_error_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.check-provider.response" => vector_protocol_spec(
            "vector-check-provider-response",
            "vector_helper_check_provider_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_check_provider_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.status.response" => vector_protocol_spec(
            "vector-status-response",
            "vector_helper_status_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_status_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.rebuild.response" => vector_protocol_spec(
            "vector-rebuild-response",
            "vector_helper_rebuild_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_rebuild_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.sync.response" => vector_protocol_spec(
            "vector-sync-response",
            "vector_helper_sync_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_sync_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.label-atoms-status.response" => vector_protocol_spec(
            "vector-label-atoms-status-response",
            "vector_helper_label_atoms_status_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_label_atoms_status_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.rebuild-label-atoms.response" => vector_protocol_spec(
            "vector-rebuild-label-atoms-response",
            "vector_helper_rebuild_label_atoms_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_rebuild_label_atoms_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.sync-label-atoms.response" => vector_protocol_spec(
            "vector-sync-label-atoms-response",
            "vector_helper_sync_label_atoms_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_sync_label_atoms_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.query-chunks.response" => vector_protocol_spec(
            "vector-query-chunks-response",
            "vector_helper_query_chunks_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_query_chunks_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.query-label-atoms.response" => vector_protocol_spec(
            "vector-query-label-atoms-response",
            "vector_helper_query_label_atoms_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_query_label_atoms_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        "helper.vector.embed-query.response" => vector_protocol_spec(
            "vector-embed-query-response",
            "vector_helper_embed_query_response_fixture_is_produced_by_real_helper_adapter",
            "vector_helper_embed_query_response_fixture_is_consumed_by_runtime_protocol_decoder",
        ),
        _ => return None,
    };
    Some(spec)
}

fn graph_protocol_spec(
    slug: &'static str,
    producer: &'static str,
    consumer: &'static str,
) -> ProtocolAdoptionSpec {
    helper_protocol_spec(slug, "kanban-graph-oxigraph", producer, consumer)
}

fn vector_protocol_spec(
    slug: &'static str,
    producer: &'static str,
    consumer: &'static str,
) -> ProtocolAdoptionSpec {
    helper_protocol_spec(slug, "kanban-vector-lancedb", producer, consumer)
}

fn helper_protocol_spec(
    slug: &'static str,
    package: &'static str,
    producer: &'static str,
    consumer: &'static str,
) -> ProtocolAdoptionSpec {
    let (schema_id, fixture) = match slug {
        "graph-handshake-response" => (
            "urn:kanban-tool:schema:helper:graph-handshake-response:v1",
            "schemas/fixtures/helper/graph-handshake-response.v1.valid.json",
        ),
        "graph-error-response" => (
            "urn:kanban-tool:schema:helper:graph-error-response:v1",
            "schemas/fixtures/helper/graph-error-response.v1.valid.json",
        ),
        "graph-status-response" => (
            "urn:kanban-tool:schema:helper:graph-status-response:v1",
            "schemas/fixtures/helper/graph-status-response.v1.valid.json",
        ),
        "graph-rebuild-response" => (
            "urn:kanban-tool:schema:helper:graph-rebuild-response:v1",
            "schemas/fixtures/helper/graph-rebuild-response.v1.valid.json",
        ),
        "graph-sync-response" => (
            "urn:kanban-tool:schema:helper:graph-sync-response:v1",
            "schemas/fixtures/helper/graph-sync-response.v1.valid.json",
        ),
        "graph-neighbors-response" => (
            "urn:kanban-tool:schema:helper:graph-neighbors-response:v1",
            "schemas/fixtures/helper/graph-neighbors-response.v1.valid.json",
        ),
        "graph-query-response" => (
            "urn:kanban-tool:schema:helper:graph-query-response:v1",
            "schemas/fixtures/helper/graph-query-response.v1.valid.json",
        ),
        "vector-handshake-response" => (
            "urn:kanban-tool:schema:helper:vector-handshake-response:v1",
            "schemas/fixtures/helper/vector-handshake-response.v1.valid.json",
        ),
        "vector-error-response" => (
            "urn:kanban-tool:schema:helper:vector-error-response:v1",
            "schemas/fixtures/helper/vector-error-response.v1.valid.json",
        ),
        "vector-check-provider-response" => (
            "urn:kanban-tool:schema:helper:vector-check-provider-response:v1",
            "schemas/fixtures/helper/vector-check-provider-response.v1.valid.json",
        ),
        "vector-status-response" => (
            "urn:kanban-tool:schema:helper:vector-status-response:v1",
            "schemas/fixtures/helper/vector-status-response.v1.valid.json",
        ),
        "vector-rebuild-response" => (
            "urn:kanban-tool:schema:helper:vector-rebuild-response:v1",
            "schemas/fixtures/helper/vector-rebuild-response.v1.valid.json",
        ),
        "vector-sync-response" => (
            "urn:kanban-tool:schema:helper:vector-sync-response:v1",
            "schemas/fixtures/helper/vector-sync-response.v1.valid.json",
        ),
        "vector-label-atoms-status-response" => (
            "urn:kanban-tool:schema:helper:vector-label-atoms-status-response:v1",
            "schemas/fixtures/helper/vector-label-atoms-status-response.v1.valid.json",
        ),
        "vector-rebuild-label-atoms-response" => (
            "urn:kanban-tool:schema:helper:vector-rebuild-label-atoms-response:v1",
            "schemas/fixtures/helper/vector-rebuild-label-atoms-response.v1.valid.json",
        ),
        "vector-sync-label-atoms-response" => (
            "urn:kanban-tool:schema:helper:vector-sync-label-atoms-response:v1",
            "schemas/fixtures/helper/vector-sync-label-atoms-response.v1.valid.json",
        ),
        "vector-query-chunks-response" => (
            "urn:kanban-tool:schema:helper:vector-query-chunks-response:v1",
            "schemas/fixtures/helper/vector-query-chunks-response.v1.valid.json",
        ),
        "vector-query-label-atoms-response" => (
            "urn:kanban-tool:schema:helper:vector-query-label-atoms-response:v1",
            "schemas/fixtures/helper/vector-query-label-atoms-response.v1.valid.json",
        ),
        "vector-embed-query-response" => (
            "urn:kanban-tool:schema:helper:vector-embed-query-response:v1",
            "schemas/fixtures/helper/vector-embed-query-response.v1.valid.json",
        ),
        _ => unreachable!("unknown helper protocol slug"),
    };
    (
        schema_id,
        fixture,
        package,
        "helper_protocol_contract_adoption",
        producer,
        consumer,
    )
}

fn portable_operation_contracts() -> Vec<OperationContract> {
    crate::portable_contract_catalog()
        .iter()
        .flat_map(|descriptor| {
            [
                portable_operation_contract(
                    descriptor.operation_key,
                    &descriptor.input,
                    ContractDirection::Deserialize,
                ),
                portable_operation_contract(
                    descriptor.operation_key,
                    &descriptor.output,
                    ContractDirection::Serialize,
                ),
            ]
        })
        .collect()
}

fn portable_operation_contract(
    operation: &'static str,
    side: &'static crate::PortableContractSide,
    direction: ContractDirection,
) -> OperationContract {
    OperationContract {
        id: side.contract_id,
        path: operation,
        surface: ContractSurface::Jsonl,
        operation,
        direction,
        granularity: ContractGranularity::Exact,
        strictness: ContractStrictness::DenyUnknownFields,
        schema_id: Some(side.schema_id),
        fixture: Some(side.fixture),
        adoption: Some(AdoptionEvidence {
            producer_fixture: side.fixture,
            producer: AdoptionWitness {
                operation,
                contract_id: side.contract_id,
                surface: ContractSurface::Jsonl,
                direction,
                package: "kanban-sqlite",
                test_target: side.test_target,
                exact_test: side.producer_test,
            },
            consumer: AdoptionWitness {
                operation,
                contract_id: side.contract_id,
                surface: ContractSurface::Jsonl,
                direction,
                package: "kanban-sqlite",
                test_target: side.test_target,
                exact_test: side.consumer_test,
            },
        }),
        exclusion: None,
        migration: MigrationState::Adopted,
        transport: ContractTransport::NoTransport,
        binding: ContractBinding::ExactSurface,
    }
}
