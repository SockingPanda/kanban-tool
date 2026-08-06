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
                    test_target: "lib",
                    exact_test: $producer_test,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-server",
                    test_target: "lib",
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
                    test_target: "lib",
                    exact_test: $producer,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: $direction,
                    package: "kanban-server",
                    test_target: "lib",
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

const ATTACHMENT_TASK_PATH_PARAMETERS: &[WireParameter] = &[WireParameter {
    name: "task_id",
    cardinality: Some(WireParameterCardinality::RequiredOne),
}];

const ATTACHMENT_ITEM_PATH_PARAMETERS: &[WireParameter] = &[
    WireParameter {
        name: "task_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
    WireParameter {
        name: "attachment_id",
        cardinality: Some(WireParameterCardinality::RequiredOne),
    },
];

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
                    test_target: "lib",
                    exact_test: $producer_test,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Deserialize,
                    package: "kanban-server",
                    test_target: "lib",
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
                    test_target: "lib",
                    exact_test: $producer,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: ContractDirection::Serialize,
                    package: "kanban-server",
                    test_target: "lib",
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

macro_rules! adopted_api_runtime_contract {
    (
        $id:literal,
        $path:literal,
        $operation:literal,
        $direction:expr,
        $schema_id:literal,
        $fixture:literal,
        $location:expr,
        $parameters:expr,
        $producer_package:literal,
        $producer_target:literal,
        $producer_test:literal,
        $consumer_package:literal,
        $consumer_target:literal,
        $consumer_test:literal
    ) => {
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
                    package: $producer_package,
                    test_target: $producer_target,
                    exact_test: $producer_test,
                },
                consumer: AdoptionWitness {
                    operation: $operation,
                    contract_id: $id,
                    surface: ContractSurface::Api,
                    direction: $direction,
                    package: $consumer_package,
                    test_target: $consumer_target,
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
const GRAPH_QUERY_QUERY_PARAMETERS: &[WireParameter] = &[
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
        "cli.entity-upsert.output",
        "entity upsert",
        "urn:kanban-tool:schema:cli:entity-upsert-output:v1",
        "schemas/fixtures/cli/entity-upsert-output.v1.valid.json",
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
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
        "cli.checkpoint.output",
        "checkpoint",
        "urn:kanban-tool:schema:cli:checkpoint-output:v1",
        "schemas/fixtures/cli/checkpoint-output.v1.valid.json",
        "cli_maintenance_contract_adoption",
        "checkpoint_output_fixture_is_produced_by_real_cli",
        "checkpoint_output_fixture_is_consumed_by_public_contract"
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
        "cli.task-specify.output",
        "task specify",
        "urn:kanban-tool:schema:cli:task-specify-output:v1",
        "schemas/fixtures/cli/task-specify-output.v1.valid.json",
        "cli_lifecycle_adoption",
        "lifecycle_cli_runs_each_transition_through_localhost_host",
        "lifecycle_cli_runs_each_transition_through_localhost_host"
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
        "cli_task_contract_adoption",
        "task_done_output_contract",
        "task_done_output_contract"
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
        "cli_task_contract_adoption",
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
        "label-create",
        "label_create",
        "label create",
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
    adopted_cli_output_contract!(
        "cli.graph-neighborhood.output",
        "graph neighborhood",
        "urn:kanban-tool:schema:cli:graph-neighborhood-output:v1",
        "schemas/fixtures/cli/graph-neighborhood-output.v1.valid.json",
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
    ),
    adopted_cli_output_contract!(
        "cli.graph-map.output",
        "graph map",
        "urn:kanban-tool:schema:cli:graph-map-output:v1",
        "schemas/fixtures/cli/graph-map-output.v1.valid.json",
        "cli_knowledge_adoption",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers"
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
        "api.rebuild-search-index.query",
        "POST /api/v1/search/index/rebuild query",
        "POST /api/v1/search/index/rebuild",
        "urn:kanban-tool:schema:api:rebuild-search-index-query:v1",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "suite::derived_adoption::search_status_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::search_status_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.rebuild-search-index.response",
        "POST /api/v1/search/index/rebuild response",
        "POST /api/v1/search/index/rebuild",
        "urn:kanban-tool:schema:api:rebuild-search-index-response:v1",
        "schemas/fixtures/api/search-status-response.v1.valid.json",
        "suite::derived_adoption::search_status_response_fixture_is_produced_by_real_router",
        "suite::derived_adoption::search_status_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.sync-search-index.query",
        "POST /api/v1/search/index/sync query",
        "POST /api/v1/search/index/sync",
        "urn:kanban-tool:schema:api:sync-search-index-query:v1",
        "schemas/fixtures/api/search-status-query.v1.valid.json",
        "suite::derived_adoption::search_status_query_dto_serializes_to_committed_fixture",
        "suite::derived_adoption::search_status_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.sync-search-index.response",
        "POST /api/v1/search/index/sync response",
        "POST /api/v1/search/index/sync",
        "urn:kanban-tool:schema:api:sync-search-index-response:v1",
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
        "api.graph-query.query",
        "GET /api/v1/graph/query query",
        "GET /api/v1/graph/query",
        "urn:kanban-tool:schema:api:graph-query-query:v1",
        "schemas/fixtures/api/graph-query-query.v1.valid.json",
        "suite::graph_adoption::graph_query_query_dto_serializes_to_committed_fixture",
        "suite::graph_adoption::graph_query_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        GRAPH_QUERY_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.graph-query.response",
        "GET /api/v1/graph/query response",
        "GET /api/v1/graph/query",
        "urn:kanban-tool:schema:api:graph-query-response:v1",
        "schemas/fixtures/api/graph-query-response.v1.valid.json",
        "suite::graph_adoption::graph_query_response_fixture_is_produced_by_real_router",
        "suite::graph_adoption::graph_query_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.graph-rebuild.query",
        "POST /api/v1/graph/rebuild query",
        "POST /api/v1/graph/rebuild",
        "urn:kanban-tool:schema:api:graph-rebuild-query:v1",
        "schemas/fixtures/api/graph-rebuild-query.v1.valid.json",
        "suite::graph_adoption::graph_rebuild_query_dto_serializes_to_committed_fixture",
        "suite::graph_adoption::graph_rebuild_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.graph-rebuild.response",
        "POST /api/v1/graph/rebuild response",
        "POST /api/v1/graph/rebuild",
        "urn:kanban-tool:schema:api:graph-rebuild-response:v1",
        "schemas/fixtures/api/graph-rebuild-response.v1.valid.json",
        "suite::graph_adoption::graph_rebuild_response_fixture_is_produced_by_real_router",
        "suite::graph_adoption::graph_rebuild_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.graph-sync.query",
        "POST /api/v1/graph/sync query",
        "POST /api/v1/graph/sync",
        "urn:kanban-tool:schema:api:graph-sync-query:v1",
        "schemas/fixtures/api/graph-sync-query.v1.valid.json",
        "suite::graph_adoption::graph_sync_query_dto_serializes_to_committed_fixture",
        "suite::graph_adoption::graph_sync_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        BOARD_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.graph-sync.response",
        "POST /api/v1/graph/sync response",
        "POST /api/v1/graph/sync",
        "urn:kanban-tool:schema:api:graph-sync-response:v1",
        "schemas/fixtures/api/graph-sync-response.v1.valid.json",
        "suite::graph_adoption::graph_sync_response_fixture_is_produced_by_real_router",
        "suite::graph_adoption::graph_sync_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.entity-list.query",
        "GET /api/v1/entities query",
        "GET /api/v1/entities",
        "urn:kanban-tool:schema:api:entity-list-query:v1",
        "schemas/fixtures/api/entity-list-query.v1.valid.json",
        "suite::entity_adoption::entity_list_query_dto_serializes_to_committed_fixture",
        "suite::entity_adoption::entity_list_query_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Query,
        ENTITY_LIST_QUERY_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.entity-list.response",
        "GET /api/v1/entities response",
        "GET /api/v1/entities",
        "urn:kanban-tool:schema:api:entity-list-response:v1",
        "schemas/fixtures/api/entity-list-response.v1.valid.json",
        "suite::entity_adoption::entity_list_response_fixture_is_produced_by_real_router",
        "suite::entity_adoption::entity_list_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_parameter_contract!(
        "api.entity.path",
        "GET /api/v1/entities/:uri path",
        "GET /api/v1/entities/:uri",
        "urn:kanban-tool:schema:api:entity-path:v1",
        "schemas/fixtures/api/entity-path.v1.valid.json",
        "suite::entity_adoption::entity_path_dto_serializes_to_committed_fixture",
        "suite::entity_adoption::entity_path_fixture_is_consumed_by_real_router",
        HttpTransportLocation::Path,
        ENTITY_PATH_PARAMETERS
    ),
    adopted_api_response_contract!(
        "api.entity.response",
        "GET /api/v1/entities/:uri response",
        "GET /api/v1/entities/:uri",
        "urn:kanban-tool:schema:api:entity-response:v1",
        "schemas/fixtures/api/entity-response.v1.valid.json",
        "suite::entity_adoption::entity_response_fixture_is_produced_by_real_router",
        "suite::entity_adoption::entity_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_api_request!(
        "api.entity-upsert.request",
        "PUT /api/v1/entities body",
        "PUT /api/v1/entities",
        "urn:kanban-tool:schema:api:entity-upsert-request:v1",
        "schemas/fixtures/api/entity-upsert-request.v1.valid.json",
        "suite::entity_adoption::entity_upsert_request_dto_serializes_to_committed_fixture",
        "suite::entity_adoption::entity_upsert_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_response_contract!(
        "api.entity-upsert.response",
        "PUT /api/v1/entities response",
        "PUT /api/v1/entities",
        "urn:kanban-tool:schema:api:entity-upsert-response:v1",
        "schemas/fixtures/api/entity-upsert-response.v1.valid.json",
        "suite::entity_adoption::entity_upsert_response_fixture_is_produced_by_real_router",
        "suite::entity_adoption::entity_upsert_response_fixture_is_consumed_by_contract_root"
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
    adopted_api_runtime_contract!(
        "api.vector-configure.request",
        "POST /api/v1/vector/configure request",
        "POST /api/v1/vector/configure",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:vector-configure-request:v1",
        "schemas/fixtures/api/vector-configure-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_configure_request_fixture_is_produced",
        "kanban-server",
        "lib",
        "vector::tests::vector_configure_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_runtime_contract!(
        "api.vector-configure.response",
        "POST /api/v1/vector/configure response",
        "POST /api/v1/vector/configure",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:vector-configure-response:v1",
        "schemas/fixtures/api/vector-configure-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "kanban-server",
        "lib",
        "vector::tests::vector_configure_response_fixture_is_produced_by_real_router",
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_configure_response_fixture_is_consumed_by_client"
    ),
    adopted_api_runtime_contract!(
        "api.vector-rebuild.request",
        "POST /api/v1/vector/rebuild request",
        "POST /api/v1/vector/rebuild",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:vector-rebuild-request:v1",
        "schemas/fixtures/api/vector-rebuild-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_rebuild_request_fixture_is_produced",
        "kanban-server",
        "lib",
        "vector::tests::vector_rebuild_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_runtime_contract!(
        "api.vector-rebuild.response",
        "POST /api/v1/vector/rebuild response",
        "POST /api/v1/vector/rebuild",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:vector-rebuild-response:v1",
        "schemas/fixtures/api/vector-rebuild-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "kanban-server",
        "lib",
        "vector::tests::vector_rebuild_response_fixture_is_produced_by_real_router",
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_rebuild_response_fixture_is_consumed_by_client"
    ),
    adopted_api_runtime_contract!(
        "api.vector-sync.request",
        "POST /api/v1/vector/sync request",
        "POST /api/v1/vector/sync",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:vector-sync-request:v1",
        "schemas/fixtures/api/vector-sync-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_sync_request_fixture_is_produced",
        "kanban-server",
        "lib",
        "vector::tests::vector_sync_request_fixture_is_consumed_by_real_router"
    ),
    adopted_api_runtime_contract!(
        "api.vector-sync.response",
        "POST /api/v1/vector/sync response",
        "POST /api/v1/vector/sync",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:vector-sync-response:v1",
        "schemas/fixtures/api/vector-sync-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "kanban-server",
        "lib",
        "vector::tests::vector_sync_response_fixture_is_produced_by_real_router",
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_sync_response_fixture_is_consumed_by_client"
    ),
    adopted_api_runtime_contract!(
        "api.vector-query-chunks.query",
        "GET /api/v1/vector/query-chunks query",
        "GET /api/v1/vector/query-chunks",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:vector-query-chunks-query:v1",
        "schemas/fixtures/api/vector-query-chunks-query.v1.valid.json",
        HttpTransportLocation::Query,
        VECTOR_QUERY_PARAMETERS,
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_query_chunks_query_fixture_is_produced",
        "kanban-server",
        "lib",
        "vector::tests::vector_query_chunks_query_fixture_is_consumed_by_real_router"
    ),
    adopted_api_runtime_contract!(
        "api.vector-query-chunks.response",
        "GET /api/v1/vector/query-chunks response",
        "GET /api/v1/vector/query-chunks",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:vector-query-chunks-response:v1",
        "schemas/fixtures/api/vector-query-chunks-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "kanban-server",
        "lib",
        "vector::tests::vector_query_chunks_response_fixture_is_produced_by_real_router",
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_query_chunks_response_fixture_is_consumed_by_client"
    ),
    adopted_api_runtime_contract!(
        "api.vector-query-label-atoms.query",
        "GET /api/v1/vector/query-label-atoms query",
        "GET /api/v1/vector/query-label-atoms",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:vector-query-label-atoms-query:v1",
        "schemas/fixtures/api/vector-query-label-atoms-query.v1.valid.json",
        HttpTransportLocation::Query,
        VECTOR_QUERY_PARAMETERS,
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_query_label_atoms_query_fixture_is_produced",
        "kanban-server",
        "lib",
        "vector::tests::vector_query_label_atoms_query_fixture_is_consumed_by_real_router"
    ),
    adopted_api_runtime_contract!(
        "api.vector-query-label-atoms.response",
        "GET /api/v1/vector/query-label-atoms response",
        "GET /api/v1/vector/query-label-atoms",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:vector-query-label-atoms-response:v1",
        "schemas/fixtures/api/vector-query-label-atoms-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "kanban-server",
        "lib",
        "vector::tests::vector_query_label_atoms_response_fixture_is_produced_by_real_router",
        "kanban-client",
        "lib",
        "operations::vector::tests::vector_query_label_atoms_response_fixture_is_consumed_by_client"
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
    adopted_api_response_contract!(
        "api.doctor.response",
        "GET /api/v1/maintenance/doctor response",
        "GET /api/v1/maintenance/doctor",
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
                test_target: "lib",
                exact_test: "suite::health::health_response_fixture_is_produced_by_real_router",
            },
            consumer: AdoptionWitness {
                operation: "GET /health",
                contract_id: "api.health.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "lib",
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
                test_target: "lib",
                exact_test: "suite::errors::api_error_response_contract_produces_fixture",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks",
                contract_id: "api.error.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "lib",
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
        "http::operations::contract_adoption::suite_tasks_crud_and_reads_use_committed_fixtures_through_router",
        "http::operations::contract_adoption::suite_tasks_crud_and_reads_use_committed_fixtures_through_router",
        HttpTransportLocation::Path,
        TASK_READ_PATH_PARAMETERS
    ),
    adopted_api_parameter_contract!(
        "api.list-tasks-by-status.query",
        "GET /api/v1/boards/:board/tasks/by-status query",
        "GET /api/v1/boards/:board/tasks/by-status",
        "urn:kanban-tool:schema:api:list-tasks-by-status-query:v1",
        "schemas/fixtures/api/list-tasks-by-status-query.v1.valid.json",
        "http::operations::contract_adoption::suite_tasks_crud_and_reads_use_committed_fixtures_through_router",
        "http::operations::contract_adoption::suite_tasks_crud_and_reads_use_committed_fixtures_through_router",
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
                test_target: "lib",
                exact_test: "suite::api_task_component::list_tasks_response_producer_fixture",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks",
                contract_id: "api.list-tasks.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "lib",
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
                test_target: "lib",
                exact_test: "http::operations::contract_adoption::suite_tasks_crud_and_reads_use_committed_fixtures_through_router",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/boards/:board/tasks/by-status",
                contract_id: "api.list-tasks-by-status.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "lib",
                exact_test: "http::operations::contract_adoption::suite_tasks_crud_and_reads_use_committed_fixtures_through_router",
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
                test_target: "lib",
                exact_test: "suite::delete_adoption::delete_label_semantics_response_fixture_is_produced_by_real_router",
            },
            consumer: AdoptionWitness {
                operation: "DELETE /api/v1/boards/:board/labels/:label_id/semantics",
                contract_id: "api.label-semantics-delete.response",
                surface: ContractSurface::Api,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "lib",
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
    adopted_comment_contract!(
        "api.list-board-labels.path",
        "GET /api/v1/boards/:board/labels path",
        "GET /api/v1/boards/:board/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:list-board-labels-path:v1",
        "schemas/fixtures/api/list-board-labels-path.v1.valid.json",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS,
        "suite::labels_adoption::list_board_labels_path_dto_serializes_to_committed_fixture",
        "suite::labels_adoption::list_board_labels_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.list-board-labels.response",
        "GET /api/v1/boards/:board/labels success",
        "GET /api/v1/boards/:board/labels",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:list-board-labels-response:v1",
        "schemas/fixtures/api/list-board-labels-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::labels_adoption::list_board_labels_response_fixture_is_produced_by_real_router",
        "suite::labels_adoption::list_board_labels_response_fixture_is_consumed_by_contract_root"
    ),
    adopted_comment_contract!(
        "api.create-board-label.path",
        "POST /api/v1/boards/:board/labels path",
        "POST /api/v1/boards/:board/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-board-label-path:v1",
        "schemas/fixtures/api/create-board-label-path.v1.valid.json",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS,
        "suite::labels_adoption::create_board_label_path_dto_serializes_to_committed_fixture",
        "suite::labels_adoption::create_board_label_path_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.create-board-label.request",
        "POST /api/v1/boards/:board/labels body",
        "POST /api/v1/boards/:board/labels",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:create-board-label-request:v1",
        "schemas/fixtures/api/create-board-label-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[],
        "suite::labels_adoption::create_board_label_request_dto_serializes_to_committed_fixture",
        "suite::labels_adoption::create_board_label_request_fixture_is_consumed_by_real_router"
    ),
    adopted_comment_contract!(
        "api.create-board-label.response",
        "POST /api/v1/boards/:board/labels success",
        "POST /api/v1/boards/:board/labels",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:create-board-label-response:v1",
        "schemas/fixtures/api/create-board-label-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[],
        "suite::labels_adoption::create_board_label_response_fixture_is_produced_by_real_router",
        "suite::labels_adoption::create_board_label_response_fixture_is_consumed_by_contract_root"
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
        "api.record-signal.path",
        "POST /api/v1/boards/:board/signals path",
        "POST /api/v1/boards/:board/signals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:record-signal-path:v1",
        "schemas/fixtures/api/record-signal-path.v1.valid.json",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS
    ),
    generated_api_contract!(
        "api.record-signal.request",
        "POST /api/v1/boards/:board/signals request",
        "POST /api/v1/boards/:board/signals",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:record-signal-request:v1",
        "schemas/fixtures/api/record-signal-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.record-signal.response",
        "POST /api/v1/boards/:board/signals success",
        "POST /api/v1/boards/:board/signals",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:record-signal-response:v1",
        "schemas/fixtures/api/record-signal-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.confirm-signals.path",
        "POST /api/v1/boards/:board/signals/confirm path",
        "POST /api/v1/boards/:board/signals/confirm",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:confirm-signals-path:v1",
        "schemas/fixtures/api/confirm-signals-path.v1.valid.json",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS
    ),
    generated_api_contract!(
        "api.confirm-signals.request",
        "POST /api/v1/boards/:board/signals/confirm request",
        "POST /api/v1/boards/:board/signals/confirm",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:review-signals-request:v1",
        "schemas/fixtures/api/review-signals-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.confirm-signals.response",
        "POST /api/v1/boards/:board/signals/confirm success",
        "POST /api/v1/boards/:board/signals/confirm",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:confirm-signals-response:v1",
        "schemas/fixtures/api/confirm-signals-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.reject-signals.path",
        "POST /api/v1/boards/:board/signals/reject path",
        "POST /api/v1/boards/:board/signals/reject",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:reject-signals-path:v1",
        "schemas/fixtures/api/reject-signals-path.v1.valid.json",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS
    ),
    generated_api_contract!(
        "api.reject-signals.request",
        "POST /api/v1/boards/:board/signals/reject request",
        "POST /api/v1/boards/:board/signals/reject",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:reject-signals-request:v1",
        "schemas/fixtures/api/reject-signals-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.reject-signals.response",
        "POST /api/v1/boards/:board/signals/reject success",
        "POST /api/v1/boards/:board/signals/reject",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:reject-signals-response:v1",
        "schemas/fixtures/api/reject-signals-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.resolve-signals.path",
        "POST /api/v1/boards/:board/signals/resolve path",
        "POST /api/v1/boards/:board/signals/resolve",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:resolve-signals-path:v1",
        "schemas/fixtures/api/resolve-signals-path.v1.valid.json",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS
    ),
    generated_api_contract!(
        "api.resolve-signals.request",
        "POST /api/v1/boards/:board/signals/resolve request",
        "POST /api/v1/boards/:board/signals/resolve",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:resolve-signals-request:v1",
        "schemas/fixtures/api/resolve-signals-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.resolve-signals.response",
        "POST /api/v1/boards/:board/signals/resolve success",
        "POST /api/v1/boards/:board/signals/resolve",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:resolve-signals-response:v1",
        "schemas/fixtures/api/resolve-signals-response.v1.valid.json",
        HttpTransportLocation::Success,
        &[]
    ),
    generated_api_contract!(
        "api.supersede-signals.path",
        "POST /api/v1/boards/:board/signals/supersede path",
        "POST /api/v1/boards/:board/signals/supersede",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:supersede-signals-path:v1",
        "schemas/fixtures/api/supersede-signals-path.v1.valid.json",
        HttpTransportLocation::Path,
        BOARD_PATH_PARAMETERS
    ),
    generated_api_contract!(
        "api.supersede-signals.request",
        "POST /api/v1/boards/:board/signals/supersede request",
        "POST /api/v1/boards/:board/signals/supersede",
        ContractDirection::Deserialize,
        "urn:kanban-tool:schema:api:supersede-signals-request:v1",
        "schemas/fixtures/api/supersede-signals-request.v1.valid.json",
        HttpTransportLocation::Body,
        &[]
    ),
    generated_api_contract!(
        "api.supersede-signals.response",
        "POST /api/v1/boards/:board/signals/supersede success",
        "POST /api/v1/boards/:board/signals/supersede",
        ContractDirection::Serialize,
        "urn:kanban-tool:schema:api:supersede-signals-response:v1",
        "schemas/fixtures/api/supersede-signals-response.v1.valid.json",
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
                test_target: "lib",
                exact_test: "suite::sse_adoption::stream_events_query_dto_serializes_to_committed_fixture",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/stream/events",
                contract_id: "sse.stream-events.query",
                surface: ContractSurface::Sse,
                direction: ContractDirection::Deserialize,
                package: "kanban-server",
                test_target: "lib",
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
                test_target: "lib",
                exact_test: "suite::sse_adoption::stream_event_data_fixture_is_produced_by_real_router",
            },
            consumer: AdoptionWitness {
                operation: "GET /api/v1/stream/events",
                contract_id: "sse.event.data",
                surface: ContractSurface::Sse,
                direction: ContractDirection::Serialize,
                package: "kanban-server",
                test_target: "lib",
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
            let mut inventory = hybrid_static_inventory();
            adopt_phase5_api_contracts(&mut inventory);
            adopt_protocol_contracts(&mut inventory);
            inventory.extend(portable_operation_contracts());
            inventory.extend(crate::headers::header_operation_contracts());
            inventory.extend(attachment_api_contracts());
            converge_history_catalog_contracts(&mut inventory);
            converge_labels_catalog_contracts(&mut inventory);
            converge_knowledge_catalog_contracts(&mut inventory);
            inventory.extend(attachment_cli_contracts());
            let mut legacy_maintenance = maintenance_operation_contracts();
            legacy_maintenance.retain(|contract| contract.surface == ContractSurface::Cli);
            inventory.extend(crate::admin_catalog::inventory_contracts());
            inventory.extend(legacy_maintenance);
            converge_adoption_witnesses(&mut inventory);
            inventory
        })
        .as_slice()
}

fn converge_history_catalog_contracts(inventory: &mut [OperationContract]) {
    let history = crate::history_catalog::operation_contracts();
    for contract in inventory {
        if let Some(source) = history.iter().find(|candidate| candidate.id == contract.id) {
            *contract = *source;
        }
    }
}

fn converge_labels_catalog_contracts(inventory: &mut [OperationContract]) {
    let labels = crate::labels_catalog::operation_contracts();
    for contract in inventory {
        if let Some(source) = labels.iter().find(|candidate| candidate.id == contract.id) {
            *contract = *source;
        }
    }
}

fn converge_knowledge_catalog_contracts(inventory: &mut [OperationContract]) {
    let knowledge = crate::knowledge_catalog::operation_contracts();
    for contract in inventory {
        if let Some(source) = knowledge
            .iter()
            .find(|candidate| candidate.id == contract.id)
        {
            *contract = *source;
        }
    }
}

fn hybrid_static_inventory() -> Vec<OperationContract> {
    let admin = crate::admin_catalog::operation_contracts();
    let board = crate::board_catalog::operation_contracts();
    let dependency = crate::dependency_catalog::operation_contracts();
    let step = crate::step_catalog::operation_contracts();
    let task = crate::task_catalog::operation_contracts();
    let labels = crate::labels_catalog::operation_contracts();
    let knowledge = crate::knowledge_catalog::operation_contracts();
    let mut inventory = Vec::with_capacity(OPERATION_INVENTORY.len() + 55);

    for contract in OPERATION_INVENTORY {
        // archive-board request 历史上以 add-dependency 作为插入锚点。
        // Dependency 已迁移为声明投影后，仍需先保留这条 Board row。
        if contract.id == "api.add-dependency.request" {
            append_board_contract(&mut inventory, &board, "api.archive-board.request");
        }
        if let Some(dependency_contract) = dependency
            .iter()
            .find(|candidate| candidate.id == contract.id)
        {
            inventory.push(*dependency_contract);
            continue;
        }
        if let Some(step_contract) = step.iter().find(|candidate| candidate.id == contract.id) {
            inventory.push(*step_contract);
            continue;
        }
        if let Some(task_contract) = task.iter().find(|candidate| candidate.id == contract.id) {
            inventory.push(*task_contract);
            continue;
        }
        if let Some(labels_contract) = labels.iter().find(|candidate| candidate.id == contract.id) {
            inventory.push(*labels_contract);
            continue;
        }
        if let Some(knowledge_contract) = knowledge
            .iter()
            .find(|candidate| candidate.id == contract.id)
        {
            inventory.push(*knowledge_contract);
            continue;
        }
        if contract.id == "api.doctor.response" {
            append_board_contract(&mut inventory, &board, "api.list-board-columns.path");
            append_board_contract(&mut inventory, &board, "api.list-board-columns.response");
        }
        if let Some(admin_contract) = admin.iter().find(|candidate| candidate.id == contract.id) {
            inventory.push(*admin_contract);
            continue;
        }
        match contract.id {
            // Keep the historical CLI order around the retained board use/current rows.
            "cli.board-use.output" => {
                append_board_contract(&mut inventory, &board, "cli.board-list.output");
                append_board_contract(&mut inventory, &board, "cli.board-create.output");
                append_board_contract(&mut inventory, &board, "cli.board-show.output");
                inventory.push(*contract);
            }
            "cli.task-list.output" => {
                append_board_contract(&mut inventory, &board, "cli.board-archive.output");
                append_board_contract(&mut inventory, &board, "cli.board-columns.output");
                inventory.push(*contract);
            }
            // The CRUD board rows used to follow the shared error component.
            "api.error.response" => {
                inventory.push(*contract);
                append_board_contract(&mut inventory, &board, "api.list-boards.query");
                append_board_contract(&mut inventory, &board, "api.create-board.request");
                append_board_contract(&mut inventory, &board, "api.get-board.path");
                append_board_contract(&mut inventory, &board, "api.archive-board.path");
                append_board_contract(&mut inventory, &board, "api.list-boards.response");
                append_board_contract(&mut inventory, &board, "api.create-board.response");
                append_board_contract(&mut inventory, &board, "api.get-board.response");
                append_board_contract(&mut inventory, &board, "api.archive-board.response");
            }
            _ => inventory.push(*contract),
        }
    }
    inventory
}

fn append_board_contract(
    inventory: &mut Vec<OperationContract>,
    board: &[OperationContract],
    id: &str,
) {
    inventory.push(
        board
            .iter()
            .find(|contract| contract.id == id)
            .copied()
            .unwrap_or_else(|| panic!("missing board catalog contract: {id}")),
    );
}

#[derive(Debug, Clone, Copy)]
struct WitnessLocator {
    package: &'static str,
    test_target: &'static str,
    exact_test: &'static str,
}

const fn server_route_test(exact_test: &'static str) -> WitnessLocator {
    WitnessLocator {
        package: "kanban-server",
        test_target: "lib",
        exact_test,
    }
}

const fn cli_test(test_target: &'static str, exact_test: &'static str) -> WitnessLocator {
    WitnessLocator {
        package: "kanban-cli",
        test_target,
        exact_test,
    }
}

fn converge_adoption_witnesses(inventory: &mut [OperationContract]) {
    for contract in inventory {
        let Some(mut adoption) = contract.adoption else {
            continue;
        };

        if let Some(locator) = canonical_witness(contract, false) {
            adoption.producer.package = locator.package;
            adoption.producer.test_target = locator.test_target;
            adoption.producer.exact_test = locator.exact_test;
        }
        if let Some(locator) = canonical_witness(contract, true) {
            adoption.consumer.package = locator.package;
            adoption.consumer.test_target = locator.test_target;
            adoption.consumer.exact_test = locator.exact_test;
        }
        contract.adoption = Some(adoption);
    }
}

fn canonical_witness(contract: &OperationContract, consumer: bool) -> Option<WitnessLocator> {
    match contract.surface {
        ContractSurface::Jsonl => None,
        ContractSurface::Config => Some(canonical_config_witness(contract.id, consumer)),
        ContractSurface::Metadata => Some(match contract.id {
            "metadata.decision.input" => cli_test(
                "cli_history_adoption",
                "history_cli_covers_runs_logs_comments_attachments_events_and_stats",
            ),
            "metadata.signal-record.input" | "metadata.signal-link.output" => cli_test(
                "cli_label_contract_adoption",
                "generic_signals_record_review_and_confirm_flow_through_real_cli",
            ),
            "metadata.label-proposal-candidate.input" => cli_test(
                "cli_label_contract_adoption",
                "labels_semantics_atoms_and_proposals_flow_through_real_cli",
            ),
            _ => cli_test(
                "cli_label_contract_adoption",
                "ontology_observation_signal_review_and_action_flow_through_real_cli",
            ),
        }),
        ContractSurface::Cli => Some(canonical_cli_witness(contract.id)),
        ContractSurface::Sse => Some(server_route_test(
            "http::operations::contract_adoption::suite_events_sse_and_stats_adoption_use_query_fixtures",
        )),
        ContractSurface::Api => canonical_api_witness(contract, consumer),
    }
}

fn canonical_config_witness(id: &str, consumer: bool) -> WitnessLocator {
    match (id, consumer) {
        ("config.selected-worker-profile.input", false) => cli_test(
            "cli_config_contract_adoption",
            "config_adoption::selected_worker_profile_input_fixture_is_produced_by_runtime_config_dto",
        ),
        ("config.selected-worker-profile.input", true) => cli_test(
            "cli_admin_adoption",
            "dispatcher_profile_is_consumed_by_real_serve_and_only_claims_ready",
        ),
        (_, false) => cli_test(
            "cli_config_contract_adoption",
            "config_adoption::project_config_input_fixture_is_produced_by_runtime_config_dto",
        ),
        (_, true) => cli_test(
            "cli_queue_adoption",
            "queue_cli_uses_real_host_for_config_board_and_task_commands",
        ),
    }
}

fn canonical_cli_witness(id: &str) -> WitnessLocator {
    if id.starts_with("cli.task-step-") || id.starts_with("cli.dep-") {
        return cli_test(
            "cli_steps_dependencies_adoption",
            "steps_and_dependencies_cli_use_real_host_and_committed_contract_shapes",
        );
    }
    if id.starts_with("cli.task-")
        && !matches!(
            id,
            "cli.task-create.output"
                | "cli.task-list.output"
                | "cli.task-show.output"
                | "cli.task-update.output"
        )
    {
        return cli_test(
            "cli_lifecycle_adoption",
            "lifecycle_cli_runs_each_transition_through_localhost_host",
        );
    }
    if id.starts_with("cli.run")
        || id.starts_with("cli.runs")
        || id.starts_with("cli.comment-")
        || id.starts_with("cli.attachment-")
        || id == "cli.events.output"
    {
        return cli_test(
            "cli_history_adoption",
            "history_cli_covers_runs_logs_comments_attachments_events_and_stats",
        );
    }
    if id.starts_with("cli.label-") {
        if id.contains("ontology") {
            return cli_test(
                "cli_label_contract_adoption",
                "ontology_observation_signal_review_and_action_flow_through_real_cli",
            );
        }
        return cli_test(
            "cli_label_contract_adoption",
            "labels_semantics_atoms_and_proposals_flow_through_real_cli",
        );
    }
    if id.starts_with("cli.signal-") {
        return cli_test(
            "cli_label_contract_adoption",
            "generic_signals_record_review_and_confirm_flow_through_real_cli",
        );
    }
    if id.starts_with("cli.hook-") {
        return cli_test(
            "cli_admin_adoption",
            "codex_hooks_install_handle_status_and_uninstall_use_real_binary",
        );
    }
    if id.starts_with("cli.completion") || id.starts_with("cli.__complete") {
        return cli_test(
            "cli_admin_adoption",
            "completion_and_hidden_complete_are_local_and_do_not_open_database",
        );
    }
    if id.starts_with("cli.graph-")
        || id.starts_with("cli.entity-")
        || id.starts_with("cli.search-")
        || id.starts_with("cli.vector-")
        || id.starts_with("cli.index-")
        || id.starts_with("cli.context-")
    {
        return cli_test(
            "cli_knowledge_adoption",
            "knowledge_commands_use_real_canonical_host_and_preserve_degraded_providers",
        );
    }
    if id.starts_with("cli.maintenance-")
        || id.starts_with("cli.import-v30")
        || id.starts_with("cli.doctor")
        || id.starts_with("cli.checkpoint")
        || id.starts_with("cli.stats")
    {
        return cli_test(
            "cli_admin_adoption",
            "maintenance_admin_commands_use_real_host_and_typed_json",
        );
    }
    cli_test(
        "cli_queue_adoption",
        "queue_cli_uses_real_host_for_config_board_and_task_commands",
    )
}

fn canonical_api_witness(contract: &OperationContract, consumer: bool) -> Option<WitnessLocator> {
    let id = contract.id;

    if id.ends_with(".headers") {
        return Some(header_witness(contract));
    }
    if id == "api.vector-configure.request" {
        return Some(if consumer {
            server_route_test(
                "vector::tests::vector_configure_request_fixture_is_consumed_by_real_router",
            )
        } else {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_configure_request_fixture_is_produced",
            }
        });
    }
    if id == "api.vector-rebuild.request" {
        return Some(if consumer {
            server_route_test(
                "vector::tests::vector_rebuild_request_fixture_is_consumed_by_real_router",
            )
        } else {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_rebuild_request_fixture_is_produced",
            }
        });
    }
    if id == "api.vector-sync.request" {
        return Some(if consumer {
            server_route_test(
                "vector::tests::vector_sync_request_fixture_is_consumed_by_real_router",
            )
        } else {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_sync_request_fixture_is_produced",
            }
        });
    }
    if id == "api.vector-query-chunks.query" {
        return Some(if consumer {
            server_route_test(
                "vector::tests::vector_query_chunks_query_fixture_is_consumed_by_real_router",
            )
        } else {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_query_chunks_query_fixture_is_produced",
            }
        });
    }
    if id == "api.vector-query-label-atoms.query" {
        return Some(if consumer {
            server_route_test(
                "vector::tests::vector_query_label_atoms_query_fixture_is_consumed_by_real_router",
            )
        } else {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_query_label_atoms_query_fixture_is_produced",
            }
        });
    }
    if id == "api.vector-configure.response" {
        return Some(if consumer {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_configure_response_fixture_is_consumed_by_client",
            }
        } else {
            server_route_test(
                "vector::tests::vector_configure_response_fixture_is_produced_by_real_router",
            )
        });
    }
    if id == "api.vector-rebuild.response" {
        return Some(if consumer {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_rebuild_response_fixture_is_consumed_by_client",
            }
        } else {
            server_route_test(
                "vector::tests::vector_rebuild_response_fixture_is_produced_by_real_router",
            )
        });
    }
    if id == "api.vector-sync.response" {
        return Some(if consumer {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_sync_response_fixture_is_consumed_by_client",
            }
        } else {
            server_route_test(
                "vector::tests::vector_sync_response_fixture_is_produced_by_real_router",
            )
        });
    }
    if id == "api.vector-query-chunks.response" {
        return Some(if consumer {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_query_chunks_response_fixture_is_consumed_by_client",
            }
        } else {
            server_route_test(
                "vector::tests::vector_query_chunks_response_fixture_is_produced_by_real_router",
            )
        });
    }
    if id == "api.vector-query-label-atoms.response" {
        return Some(if consumer {
            WitnessLocator {
                package: "kanban-client",
                test_target: "lib",
                exact_test: "operations::vector::tests::vector_query_label_atoms_response_fixture_is_consumed_by_client",
            }
        } else {
            server_route_test(
                "vector::tests::vector_query_label_atoms_response_fixture_is_produced_by_real_router",
            )
        });
    }

    if id.starts_with("api.maintenance-") {
        return Some(server_route_test(maintenance_witness(id, consumer)));
    }
    if id == "api.doctor.response" {
        return Some(server_route_test(if consumer {
            "suite::maintenance_adoption::doctor_response_contract_consumes_producer_fixture"
        } else {
            "suite::maintenance_adoption::doctor_response_maps_real_non_default_report_before_fixture_normalization"
        }));
    }
    if id == "api.checkpoint.response" {
        return Some(server_route_test(if consumer {
            "suite::maintenance_adoption::checkpoint_response_contract_consumes_producer_fixture"
        } else {
            "suite::maintenance_adoption::checkpoint_response_reports_real_wal_field_relationships"
        }));
    }
    if id == "api.health.response" {
        return Some(server_route_test(
            "http::operations::contract_adoption::suite_health_and_errors_use_real_router_fixtures",
        ));
    }
    if id == "api.error.response" {
        return Some(server_route_test(
            "http::operations::contract_adoption::suite_health_and_errors_use_real_router_fixtures",
        ));
    }

    if id == "api.list-board-labels.path"
        || id == "api.list-board-labels.response"
        || id == "api.create-board-label.path"
        || id == "api.create-board-label.request"
        || id == "api.create-board-label.response"
        || id == "api.list-task-labels.path"
        || id == "api.list-task-labels.response"
        || id == "api.add-task-label.path"
        || id == "api.add-task-label.request"
        || id == "api.add-task-label.response"
        || id == "api.remove-task-label.path"
        || id == "api.remove-task-label.response"
    {
        return None;
    }

    if matches!(
        id,
        "api.board-task-map.path"
            | "api.board-task-map.query"
            | "api.board-task-map.response"
            | "api.task-neighborhood.path"
            | "api.task-neighborhood.query"
            | "api.task-neighborhood.response"
            | "api.search-tasks.query"
            | "api.search-tasks.response"
            | "api.search-tasks-by-status.query"
            | "api.search-tasks-by-status.response"
            | "api.search-status.query"
            | "api.search-status.response"
            | "api.rebuild-search-index.query"
            | "api.rebuild-search-index.response"
            | "api.sync-search-index.query"
            | "api.sync-search-index.response"
            | "api.build-context.path"
            | "api.build-context.query"
            | "api.build-context.response"
            | "api.graph-status.query"
            | "api.graph-status.response"
            | "api.graph-neighbors.query"
            | "api.graph-neighbors.response"
            | "api.graph-query.query"
            | "api.graph-query.response"
            | "api.graph-rebuild.query"
            | "api.graph-rebuild.response"
            | "api.graph-sync.query"
            | "api.graph-sync.response"
            | "api.entity-list.query"
            | "api.entity-list.response"
            | "api.entity.path"
            | "api.entity.response"
            | "api.entity-upsert.request"
            | "api.entity-upsert.response"
            | "api.vector-status.query"
            | "api.vector-status.response"
            | "api.vector-configure.request"
            | "api.vector-configure.response"
            | "api.vector-rebuild.request"
            | "api.vector-rebuild.response"
            | "api.vector-sync.request"
            | "api.vector-sync.response"
            | "api.vector-query-chunks.query"
            | "api.vector-query-chunks.response"
            | "api.vector-query-label-atoms.query"
            | "api.vector-query-label-atoms.response"
    ) {
        return None;
    }

    if matches!(
        id,
        "api.list-tasks.path"
            | "api.list-tasks.query"
            | "api.list-tasks.response"
            | "api.list-tasks-by-status.path"
            | "api.list-tasks-by-status.query"
            | "api.list-tasks-by-status.response"
            | "api.create-task.path"
            | "api.create-task.request"
            | "api.create-task.response"
            | "api.get-task.path"
            | "api.get-task.query"
            | "api.get-task.response"
            | "api.update-task.path"
            | "api.update-task.request"
            | "api.update-task.response"
    ) {
        return Some(server_route_test(
            "http::operations::contract_adoption::suite_tasks_crud_and_reads_use_committed_fixtures_through_router",
        ));
    }

    let route_test = if id.contains("label-ontology") || id.contains("ontology-") {
        "knowledge_adoption::ontology_ledger_routes_consume_observation_and_action_fixtures"
    } else if id.contains("signal") {
        "knowledge_adoption::signal_routes_consume_record_list_show_and_review_fixtures"
    } else if id.contains("proposal") || id.contains("propose-task-label") {
        "knowledge_adoption::label_proposal_routes_consume_typed_fixtures_and_persist_real_proposal"
    } else if id.contains("label") {
        "knowledge_adoption::labels_semantics_and_atoms_use_committed_fixtures_through_host"
    } else if id.contains("entity") {
        "knowledge_adoption::entity_routes_consume_upsert_list_and_path_fixtures"
    } else if id.contains("search") {
        "knowledge_adoption::search_routes_consume_query_and_status_fixtures_against_real_index"
    } else if id.contains("context") {
        "knowledge_adoption::context_neighborhood_and_task_map_routes_consume_typed_fixtures"
    } else if id.contains("graph") || id.contains("task-neighborhood") || id.contains("task-map") {
        "knowledge_adoption::graph_routes_consume_query_and_projection_fixtures"
    } else if id.contains("vector-status") {
        "knowledge_adoption::vector_routes_consume_typed_projection_fixtures_and_real_degraded_queries"
    } else if id.contains("stats") || id.contains("events") || id.starts_with("sse.") {
        "http::operations::contract_adoption::suite_events_sse_and_stats_adoption_use_query_fixtures"
    } else if id.contains("board") {
        "http::operations::contract_adoption::suite_boards_adoption_uses_request_path_query_and_response_fixtures"
    } else if id.contains("step") || id.contains("execution-plan") {
        "http::operations::contract_adoption::suite_steps_and_plans_adoption_uses_real_router_fixtures"
    } else if id.contains("depend") {
        "http::operations::contract_adoption::suite_dependencies_adoption_uses_path_body_and_response_fixtures"
    } else if id.contains("comment") || id.contains("attachment") {
        "http::operations::contract_adoption::suite_comments_and_attachments_adoption_uses_real_router_fixtures"
    } else if id.contains("run") {
        "http::operations::contract_adoption::suite_runs_and_logs_adoption_uses_real_router_paths_and_fixtures"
    } else if id.contains("task") || id.contains("transition") {
        "http::operations::contract_adoption::suite_task_lifecycle_adoption_uses_committed_requests_and_typed_responses"
    } else {
        "http::operations::contract_adoption::suite_health_and_errors_use_real_router_fixtures"
    };
    Some(server_route_test(route_test))
}

fn header_witness(contract: &OperationContract) -> WitnessLocator {
    let fixture = contract.fixture.unwrap_or_default();
    let exact_test = if fixture.contains("locale-actor-optional-json-headers") {
        "knowledge_adoption::locale_actor_optional_json_header_fixture_is_consumed_by_real_router"
    } else if fixture.contains("locale-actor-json-headers") {
        "knowledge_adoption::locale_actor_json_header_fixture_is_consumed_by_real_router"
    } else if fixture.contains("locale-actor-headers") {
        "knowledge_adoption::locale_actor_header_fixture_is_consumed_by_real_router"
    } else if fixture.contains("locale-json-headers") {
        "knowledge_adoption::locale_json_header_fixture_is_consumed_by_real_router"
    } else if fixture.contains("locale-headers") {
        "knowledge_adoption::locale_header_fixture_is_consumed_by_real_router"
    } else {
        panic!("unmapped API header fixture: {fixture}");
    };
    server_route_test(exact_test)
}

fn maintenance_witness(id: &str, consumer: bool) -> &'static str {
    let role = if consumer { "consumer" } else { "producer" };
    match id {
        "api.maintenance-path.request" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_path_request_consumer",
            _ => "suite::maintenance_adoption::maintenance_path_request_producer",
        },
        "api.maintenance-import.request" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_import_request_consumer",
            _ => "suite::maintenance_adoption::maintenance_import_request_producer",
        },
        "api.maintenance-backup.request" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_backup_request_consumer",
            _ => "suite::maintenance_adoption::maintenance_backup_request_producer",
        },
        "api.maintenance-export.request" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_export_request_consumer",
            _ => "suite::maintenance_adoption::maintenance_export_request_producer",
        },
        "api.maintenance-run.request" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_run_request_consumer",
            _ => "suite::maintenance_adoption::maintenance_run_request_producer",
        },
        "api.maintenance-rebuild.request" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_rebuild_request_consumer",
            _ => "suite::maintenance_adoption::maintenance_rebuild_request_producer",
        },
        "api.maintenance-cleanup.request" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_cleanup_request_consumer",
            _ => "suite::maintenance_adoption::maintenance_cleanup_request_producer",
        },
        "api.maintenance-import-v30.request" => match role {
            "consumer" => "suite::maintenance_adoption::legacy_import_v30_request_consumer",
            _ => "suite::maintenance_adoption::legacy_import_v30_request_producer",
        },
        "api.maintenance-backup.response" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_backup_response_consumer",
            _ => "suite::maintenance_adoption::maintenance_backup_response_producer",
        },
        "api.maintenance-export.response" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_export_response_consumer",
            _ => "suite::maintenance_adoption::maintenance_export_response_producer",
        },
        "api.maintenance-import.response" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_import_response_consumer",
            _ => "suite::maintenance_adoption::maintenance_import_response_producer",
        },
        "api.maintenance-vacuum.response" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_vacuum_response_consumer",
            _ => "suite::maintenance_adoption::maintenance_vacuum_response_producer",
        },
        "api.maintenance-status.response" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_status_response_consumer",
            _ => "suite::maintenance_adoption::maintenance_status_response_producer",
        },
        "api.maintenance-run.response" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_run_response_consumer",
            _ => "suite::maintenance_adoption::maintenance_run_response_producer",
        },
        "api.maintenance-rebuild.response" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_rebuild_response_consumer",
            _ => "suite::maintenance_adoption::maintenance_rebuild_response_producer",
        },
        "api.maintenance-cleanup.response" => match role {
            "consumer" => "suite::maintenance_adoption::maintenance_cleanup_response_consumer",
            _ => "suite::maintenance_adoption::maintenance_cleanup_response_producer",
        },
        "api.maintenance-import-v30.response" => match role {
            "consumer" => "suite::maintenance_adoption::legacy_import_v30_response_consumer",
            _ => "suite::maintenance_adoption::legacy_import_v30_response_producer",
        },
        _ => "suite::maintenance_adoption::maintenance_run_response_consumer",
    }
}

fn attachment_api_contracts() -> Vec<OperationContract> {
    const LIST: &str = "GET /api/v1/tasks/:task_id/attachments";
    const CREATE: &str = "POST /api/v1/tasks/:task_id/attachments";
    const ITEM: &str = "GET /api/v1/tasks/:task_id/attachments/:attachment_id";
    const DELETE: &str = "DELETE /api/v1/tasks/:task_id/attachments/:attachment_id";
    fn contract(
        id: &'static str,
        path: &'static str,
        operation: &'static str,
        direction_and_strictness: (ContractDirection, ContractStrictness),
        schema_id: &'static str,
        fixture: &'static str,
        transport: (HttpTransportLocation, &'static [WireParameter]),
    ) -> OperationContract {
        let (direction, strictness) = direction_and_strictness;
        let (location, parameters) = transport;
        OperationContract {
            id,
            path,
            surface: ContractSurface::Api,
            operation,
            direction,
            granularity: ContractGranularity::Exact,
            strictness,
            schema_id: Some(schema_id),
            fixture: Some(fixture),
            adoption: Some(AdoptionEvidence {
                producer_fixture: fixture,
                producer: AdoptionWitness {
                    operation,
                    contract_id: id,
                    surface: ContractSurface::Api,
                    direction,
                    package: "kanban-server",
                    test_target: "lib",
                    exact_test: "http::operations::attachments::tests::attachment_round_trip_is_metadata_typed_and_file_backed",
                },
                consumer: AdoptionWitness {
                    operation,
                    contract_id: id,
                    surface: ContractSurface::Api,
                    direction,
                    package: "kanban-server",
                    test_target: "lib",
                    exact_test: "http::operations::attachments::tests::attachment_round_trip_is_metadata_typed_and_file_backed",
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::Http {
                operation_key: Some(operation),
                location,
                parameters,
            },
            binding: ContractBinding::ExactSurface,
        }
    }
    vec![
        contract(
            "api.list-attachments.path",
            "GET /api/v1/tasks/:task_id/attachments path",
            LIST,
            (
                ContractDirection::Deserialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:list-attachments-path:v1",
            "schemas/fixtures/api/list-attachments-path.v1.valid.json",
            (HttpTransportLocation::Path, ATTACHMENT_TASK_PATH_PARAMETERS),
        ),
        contract(
            "api.list-attachments.response",
            "GET /api/v1/tasks/:task_id/attachments success",
            LIST,
            (
                ContractDirection::Serialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:list-attachments-response:v1",
            "schemas/fixtures/api/list-attachments-response.v1.valid.json",
            (HttpTransportLocation::Success, &[]),
        ),
        contract(
            "api.create-attachment.path",
            "POST /api/v1/tasks/:task_id/attachments path",
            CREATE,
            (
                ContractDirection::Deserialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:create-attachment-path:v1",
            "schemas/fixtures/api/create-attachment-path.v1.valid.json",
            (HttpTransportLocation::Path, ATTACHMENT_TASK_PATH_PARAMETERS),
        ),
        contract(
            "api.create-attachment.request",
            "POST /api/v1/tasks/:task_id/attachments body",
            CREATE,
            (
                ContractDirection::Deserialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:create-attachment-request:v1",
            "schemas/fixtures/api/create-attachment-request.v1.valid.json",
            (HttpTransportLocation::Body, &[]),
        ),
        contract(
            "api.create-attachment.response",
            "POST /api/v1/tasks/:task_id/attachments success",
            CREATE,
            (
                ContractDirection::Serialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:create-attachment-response:v1",
            "schemas/fixtures/api/create-attachment-response.v1.valid.json",
            (HttpTransportLocation::Success, &[]),
        ),
        contract(
            "api.download-attachment.path",
            "GET /api/v1/tasks/:task_id/attachments/:attachment_id path",
            ITEM,
            (
                ContractDirection::Deserialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:download-attachment-path:v1",
            "schemas/fixtures/api/download-attachment-path.v1.valid.json",
            (HttpTransportLocation::Path, ATTACHMENT_ITEM_PATH_PARAMETERS),
        ),
        contract(
            "api.download-attachment.response",
            "GET /api/v1/tasks/:task_id/attachments/:attachment_id success",
            ITEM,
            (
                ContractDirection::Serialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:download-attachment-response:v1",
            "schemas/fixtures/api/download-attachment-response.v1.valid.json",
            (HttpTransportLocation::Success, &[]),
        ),
        contract(
            "api.delete-attachment.path",
            "DELETE /api/v1/tasks/:task_id/attachments/:attachment_id path",
            DELETE,
            (
                ContractDirection::Deserialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:delete-attachment-path:v1",
            "schemas/fixtures/api/delete-attachment-path.v1.valid.json",
            (HttpTransportLocation::Path, ATTACHMENT_ITEM_PATH_PARAMETERS),
        ),
        contract(
            "api.delete-attachment.response",
            "DELETE /api/v1/tasks/:task_id/attachments/:attachment_id success",
            DELETE,
            (
                ContractDirection::Serialize,
                ContractStrictness::DenyUnknownFields,
            ),
            "urn:kanban-tool:schema:api:delete-attachment-response:v1",
            "schemas/fixtures/api/delete-attachment-response.v1.valid.json",
            (HttpTransportLocation::Success, &[]),
        ),
    ]
}

fn attachment_cli_contracts() -> Vec<OperationContract> {
    fn contract(
        id: &'static str,
        operation: &'static str,
        schema_id: &'static str,
        fixture: &'static str,
    ) -> OperationContract {
        OperationContract {
            id,
            path: operation,
            surface: ContractSurface::Cli,
            operation,
            direction: ContractDirection::Serialize,
            granularity: ContractGranularity::Exact,
            strictness: ContractStrictness::DenyUnknownFields,
            schema_id: Some(schema_id),
            fixture: Some(fixture),
            adoption: Some(AdoptionEvidence {
                producer_fixture: fixture,
                producer: AdoptionWitness {
                    operation,
                    contract_id: id,
                    surface: ContractSurface::Cli,
                    direction: ContractDirection::Serialize,
                    package: "kanban-server",
                    test_target: "lib",
                    exact_test: "http::operations::attachments::tests::attachment_round_trip_is_metadata_typed_and_file_backed",
                },
                consumer: AdoptionWitness {
                    operation,
                    contract_id: id,
                    surface: ContractSurface::Cli,
                    direction: ContractDirection::Serialize,
                    package: "kanban-server",
                    test_target: "lib",
                    exact_test: "http::operations::attachments::tests::attachment_round_trip_is_metadata_typed_and_file_backed",
                },
            }),
            exclusion: None,
            migration: MigrationState::Adopted,
            transport: ContractTransport::NoTransport,
            binding: ContractBinding::ExactSurface,
        }
    }
    vec![
        contract(
            "cli.attachment-add.output",
            "attachment add",
            "urn:kanban-tool:schema:cli:attachment-add-output:v1",
            "schemas/fixtures/cli/attachment-add-output.v1.valid.json",
        ),
        contract(
            "cli.attachment-list.output",
            "attachment list",
            "urn:kanban-tool:schema:cli:attachment-list-output:v1",
            "schemas/fixtures/cli/attachment-list-output.v1.valid.json",
        ),
        contract(
            "cli.attachment-remove.output",
            "attachment remove",
            "urn:kanban-tool:schema:cli:attachment-remove-output:v1",
            "schemas/fixtures/cli/attachment-remove-output.v1.valid.json",
        ),
    ]
}

fn maintenance_operation_contracts() -> Vec<OperationContract> {
    vec![
        adopted_api_request!(
            "api.maintenance-path.request",
            "POST /api/v1/maintenance/{operation} body",
            "POST /api/v1/maintenance/{operation}",
            "urn:kanban-tool:schema:api:maintenance-path-request:v1",
            "schemas/fixtures/api/maintenance-path-request.v1.valid.json",
            "maintenance_adoption::maintenance_path_request_producer",
            "maintenance_adoption::maintenance_path_request_consumer"
        ),
        adopted_api_request!(
            "api.maintenance-import.request",
            "POST /api/v1/maintenance/import body",
            "POST /api/v1/maintenance/import",
            "urn:kanban-tool:schema:api:maintenance-import-request:v1",
            "schemas/fixtures/api/maintenance-import-request.v1.valid.json",
            "maintenance_adoption::maintenance_import_request_producer",
            "maintenance_adoption::maintenance_import_request_consumer"
        ),
        adopted_api_request!(
            "api.maintenance-backup.request",
            "POST /api/v1/maintenance/backup body",
            "POST /api/v1/maintenance/backup",
            "urn:kanban-tool:schema:api:maintenance-backup-request:v1",
            "schemas/fixtures/api/maintenance-backup-request.v1.valid.json",
            "maintenance_adoption::maintenance_backup_request_producer",
            "maintenance_adoption::maintenance_backup_request_consumer"
        ),
        adopted_api_request!(
            "api.maintenance-export.request",
            "POST /api/v1/maintenance/export body",
            "POST /api/v1/maintenance/export",
            "urn:kanban-tool:schema:api:maintenance-export-request:v1",
            "schemas/fixtures/api/maintenance-export-request.v1.valid.json",
            "maintenance_adoption::maintenance_export_request_producer",
            "maintenance_adoption::maintenance_export_request_consumer"
        ),
        adopted_api_request!(
            "api.maintenance-run.request",
            "POST /api/v1/maintenance/run body",
            "POST /api/v1/maintenance/run",
            "urn:kanban-tool:schema:api:maintenance-run-request:v1",
            "schemas/fixtures/api/maintenance-run-request.v1.valid.json",
            "maintenance_adoption::maintenance_run_request_producer",
            "maintenance_adoption::maintenance_run_request_consumer"
        ),
        adopted_api_request!(
            "api.maintenance-rebuild.request",
            "POST /api/v1/maintenance/rebuild body",
            "POST /api/v1/maintenance/rebuild",
            "urn:kanban-tool:schema:api:maintenance-rebuild-request:v1",
            "schemas/fixtures/api/maintenance-rebuild-request.v1.valid.json",
            "maintenance_adoption::maintenance_rebuild_request_producer",
            "maintenance_adoption::maintenance_rebuild_request_consumer"
        ),
        adopted_api_request!(
            "api.maintenance-cleanup.request",
            "POST /api/v1/maintenance/cleanup body",
            "POST /api/v1/maintenance/cleanup",
            "urn:kanban-tool:schema:api:maintenance-cleanup-request:v1",
            "schemas/fixtures/api/maintenance-cleanup-request.v1.valid.json",
            "maintenance_adoption::maintenance_cleanup_request_producer",
            "maintenance_adoption::maintenance_cleanup_request_consumer"
        ),
        adopted_api_request!(
            "api.maintenance-import-v30.request",
            "POST /api/v1/maintenance/import-v30 body",
            "POST /api/v1/maintenance/import-v30",
            "urn:kanban-tool:schema:api:maintenance-import-v30-request:v1",
            "schemas/fixtures/api/maintenance-import-v30-request.v1.valid.json",
            "maintenance_adoption::legacy_import_v30_request_producer",
            "maintenance_adoption::legacy_import_v30_request_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-backup.response",
            "POST /api/v1/maintenance/backup response",
            "POST /api/v1/maintenance/backup",
            "urn:kanban-tool:schema:api:maintenance-backup-response:v1",
            "schemas/fixtures/api/maintenance-backup-response.v1.valid.json",
            "maintenance_adoption::maintenance_backup_response_producer",
            "maintenance_adoption::maintenance_backup_response_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-export.response",
            "POST /api/v1/maintenance/export response",
            "POST /api/v1/maintenance/export",
            "urn:kanban-tool:schema:api:maintenance-export-response:v1",
            "schemas/fixtures/api/maintenance-export-response.v1.valid.json",
            "maintenance_adoption::maintenance_export_response_producer",
            "maintenance_adoption::maintenance_export_response_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-import.response",
            "POST /api/v1/maintenance/import response",
            "POST /api/v1/maintenance/import",
            "urn:kanban-tool:schema:api:maintenance-import-response:v1",
            "schemas/fixtures/api/maintenance-import-response.v1.valid.json",
            "maintenance_adoption::maintenance_import_response_producer",
            "maintenance_adoption::maintenance_import_response_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-vacuum.response",
            "POST /api/v1/maintenance/vacuum response",
            "POST /api/v1/maintenance/vacuum",
            "urn:kanban-tool:schema:api:maintenance-vacuum-response:v1",
            "schemas/fixtures/api/maintenance-vacuum-response.v1.valid.json",
            "maintenance_adoption::maintenance_vacuum_response_producer",
            "maintenance_adoption::maintenance_vacuum_response_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-status.response",
            "GET /api/v1/maintenance/status response",
            "GET /api/v1/maintenance/status",
            "urn:kanban-tool:schema:api:maintenance-status-response:v1",
            "schemas/fixtures/api/maintenance-status-response.v1.valid.json",
            "maintenance_adoption::maintenance_status_response_producer",
            "maintenance_adoption::maintenance_status_response_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-run.response",
            "POST /api/v1/maintenance/run response",
            "POST /api/v1/maintenance/run",
            "urn:kanban-tool:schema:api:maintenance-run-response:v1",
            "schemas/fixtures/api/maintenance-run-response.v1.valid.json",
            "maintenance_adoption::maintenance_run_response_producer",
            "maintenance_adoption::maintenance_run_response_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-rebuild.response",
            "POST /api/v1/maintenance/rebuild response",
            "POST /api/v1/maintenance/rebuild",
            "urn:kanban-tool:schema:api:maintenance-rebuild-response:v1",
            "schemas/fixtures/api/maintenance-rebuild-response.v1.valid.json",
            "maintenance_adoption::maintenance_rebuild_response_producer",
            "maintenance_adoption::maintenance_rebuild_response_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-cleanup.response",
            "POST /api/v1/maintenance/cleanup response",
            "POST /api/v1/maintenance/cleanup",
            "urn:kanban-tool:schema:api:maintenance-cleanup-response:v1",
            "schemas/fixtures/api/maintenance-cleanup-response.v1.valid.json",
            "maintenance_adoption::maintenance_cleanup_response_producer",
            "maintenance_adoption::maintenance_cleanup_response_consumer"
        ),
        adopted_api_response_contract!(
            "api.maintenance-import-v30.response",
            "POST /api/v1/maintenance/import-v30 response",
            "POST /api/v1/maintenance/import-v30",
            "urn:kanban-tool:schema:api:maintenance-import-v30-response:v1",
            "schemas/fixtures/api/maintenance-import-v30-response.v1.valid.json",
            "maintenance_adoption::legacy_import_v30_response_producer",
            "maintenance_adoption::legacy_import_v30_response_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.maintenance-backup.output",
            "backup",
            "urn:kanban-tool:schema:cli:maintenance-backup-output:v1",
            "schemas/fixtures/cli/maintenance-backup-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::maintenance_backup_cli_producer",
            "maintenance_adoption::maintenance_backup_cli_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.maintenance-export.output",
            "export",
            "urn:kanban-tool:schema:cli:maintenance-export-output:v1",
            "schemas/fixtures/cli/maintenance-export-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::maintenance_export_cli_producer",
            "maintenance_adoption::maintenance_export_cli_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.maintenance-import.output",
            "import",
            "urn:kanban-tool:schema:cli:maintenance-import-output:v1",
            "schemas/fixtures/cli/maintenance-import-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::maintenance_import_cli_producer",
            "maintenance_adoption::maintenance_import_cli_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.maintenance-vacuum.output",
            "vacuum",
            "urn:kanban-tool:schema:cli:maintenance-vacuum-output:v1",
            "schemas/fixtures/cli/maintenance-vacuum-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::maintenance_vacuum_cli_producer",
            "maintenance_adoption::maintenance_vacuum_cli_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.maintenance-status-v1.output",
            "maintenance status",
            "urn:kanban-tool:schema:cli:maintenance-status-output:v1",
            "schemas/fixtures/cli/maintenance-status-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::maintenance_status_cli_producer",
            "maintenance_adoption::maintenance_status_cli_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.maintenance-run-v1.output",
            "maintenance run",
            "urn:kanban-tool:schema:cli:maintenance-run-output:v1",
            "schemas/fixtures/cli/maintenance-run-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::maintenance_run_cli_producer",
            "maintenance_adoption::maintenance_run_cli_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.maintenance-rebuild-v1.output",
            "maintenance rebuild",
            "urn:kanban-tool:schema:cli:maintenance-rebuild-output.v1",
            "schemas/fixtures/cli/maintenance-rebuild-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::maintenance_rebuild_cli_producer",
            "maintenance_adoption::maintenance_rebuild_cli_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.maintenance-cleanup.output",
            "maintenance cleanup",
            "urn:kanban-tool:schema:cli:maintenance-cleanup-output.v1",
            "schemas/fixtures/cli/maintenance-cleanup-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::maintenance_cleanup_cli_producer",
            "maintenance_adoption::maintenance_cleanup_cli_consumer"
        ),
        adopted_cli_output_contract!(
            "cli.import-v30.output",
            "import-v30",
            "urn:kanban-tool:schema:cli:import-v30-output:v1",
            "schemas/fixtures/cli/import-v30-output.v1.valid.json",
            "cli_maintenance_contract_adoption",
            "maintenance_adoption::legacy_import_v30_cli_producer",
            "maintenance_adoption::legacy_import_v30_cli_consumer"
        ),
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

const PHASE5_API_REQUEST_CONTRACTS: [(&str, &str); 53] = [
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
        "api.record-signal.path",
        record_signal_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.record-signal.request",
        record_signal_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.confirm-signals.path",
        confirm_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.confirm-signals.request",
        confirm_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.reject-signals.path",
        reject_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.reject-signals.request",
        reject_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.resolve-signals.path",
        resolve_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.resolve-signals.request",
        resolve_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.supersede-signals.path",
        supersede_signals_request_fixtures_reach_handler
    ),
    phase5_api_request_contract!(
        "api.supersede-signals.request",
        supersede_signals_request_fixtures_reach_handler
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

const PHASE5_API_RESPONSE_CONTRACTS: [(&str, &str); 29] = [
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
        "api.record-signal.response",
        "suite::api_generated_adoption::generated_signal_record_response_is_produced_by_real_router",
    ),
    (
        "api.confirm-signals.response",
        "suite::api_generated_adoption::generated_signal_action_responses_are_produced_by_real_router",
    ),
    (
        "api.reject-signals.response",
        "suite::api_generated_adoption::generated_signal_action_responses_are_produced_by_real_router",
    ),
    (
        "api.resolve-signals.response",
        "suite::api_generated_adoption::generated_signal_action_responses_are_produced_by_real_router",
    ),
    (
        "api.supersede-signals.response",
        "suite::api_generated_adoption::generated_signal_action_responses_are_produced_by_real_router",
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
                test_target: "lib",
                exact_test: producer_test,
            },
            consumer: AdoptionWitness {
                operation: contract.operation,
                contract_id: contract.id,
                surface: contract.surface,
                direction: contract.direction,
                package: "kanban-server",
                test_target: "lib",
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
            "kanban-cli",
            "cli_config_contract_adoption",
            "config_adoption::project_config_input_fixture_is_produced_by_runtime_config_dto",
            "config_adoption::project_config_input_fixture_is_consumed_by_real_toml_decoder",
        ),
        "config.selected-worker-profile.input" => (
            "urn:kanban-tool:schema:config:selected-worker-profile-input:v1",
            "schemas/fixtures/config/selected-worker-profile-input.v1.valid.json",
            "kanban-cli",
            "cli_config_contract_adoption",
            "config_adoption::selected_worker_profile_input_fixture_is_produced_by_runtime_config_dto",
            "config_adoption::selected_worker_profile_input_fixture_is_consumed_by_real_toml_decoder",
        ),
        _ => return None,
    };
    Some(spec)
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
                package: "kanban-server",
                test_target: side.test_target,
                exact_test: side.producer_test,
            },
            consumer: AdoptionWitness {
                operation,
                contract_id: side.contract_id,
                surface: ContractSurface::Jsonl,
                direction,
                package: "kanban-server",
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
