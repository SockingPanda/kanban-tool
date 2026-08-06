#[cfg(feature = "schema")]
use serde::{Deserialize, Serialize};

use crate::{
    AdoptionEvidence, AdoptionWitness, ContractBinding, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, ContractTransport, EndpointDescriptor, EndpointObligation,
    HttpMethod, HttpTransportLocation, MigrationState, OperationContract, WireParameter,
    WireParameterCardinality, endpoint_catalog,
};

const ACCEPT_LANGUAGE: WireParameter = WireParameter {
    name: "Accept-Language",
    cardinality: Some(WireParameterCardinality::OptionalOne),
};
const ACTOR: WireParameter = WireParameter {
    name: "X-KB-Actor",
    cardinality: Some(WireParameterCardinality::OptionalOne),
};
const REQUIRED_CONTENT_TYPE: WireParameter = WireParameter {
    name: "Content-Type",
    cardinality: Some(WireParameterCardinality::RequiredOne),
};
const OPTIONAL_CONTENT_TYPE: WireParameter = WireParameter {
    name: "Content-Type",
    cardinality: Some(WireParameterCardinality::OptionalOne),
};

const LOCALE_PARAMETERS: &[WireParameter] = &[ACCEPT_LANGUAGE];
const LOCALE_ACTOR_PARAMETERS: &[WireParameter] = &[ACCEPT_LANGUAGE, ACTOR];
const LOCALE_JSON_PARAMETERS: &[WireParameter] = &[ACCEPT_LANGUAGE, REQUIRED_CONTENT_TYPE];
const LOCALE_ACTOR_JSON_PARAMETERS: &[WireParameter] =
    &[ACCEPT_LANGUAGE, ACTOR, REQUIRED_CONTENT_TYPE];
const LOCALE_ACTOR_OPTIONAL_JSON_PARAMETERS: &[WireParameter] =
    &[ACCEPT_LANGUAGE, ACTOR, OPTIONAL_CONTENT_TYPE];

const ACTOR_OPERATIONS: &[&str] = &[];

const OPTIONAL_JSON_BODY_OPERATIONS: &[&str] = &[
    "api.accept-label-proposal",
    "api.propose-task-label",
    "api.reject-label-proposal",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApiHeaderProfile {
    Locale,
    LocaleActor,
    LocaleJson,
    LocaleActorJson,
    LocaleActorOptionalJson,
}

impl ApiHeaderProfile {
    pub const fn parameters(self) -> &'static [WireParameter] {
        match self {
            Self::Locale => LOCALE_PARAMETERS,
            Self::LocaleActor => LOCALE_ACTOR_PARAMETERS,
            Self::LocaleJson => LOCALE_JSON_PARAMETERS,
            Self::LocaleActorJson => LOCALE_ACTOR_JSON_PARAMETERS,
            Self::LocaleActorOptionalJson => LOCALE_ACTOR_OPTIONAL_JSON_PARAMETERS,
        }
    }

    pub const fn fixture_stem(self) -> &'static str {
        match self {
            Self::Locale => "locale-headers",
            Self::LocaleActor => "locale-actor-headers",
            Self::LocaleJson => "locale-json-headers",
            Self::LocaleActorJson => "locale-actor-json-headers",
            Self::LocaleActorOptionalJson => "locale-actor-optional-json-headers",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ApiHeaderContractSpec {
    pub contract_id: &'static str,
    pub endpoint: &'static EndpointDescriptor,
    pub profile: ApiHeaderProfile,
}

pub fn api_header_contract_specs() -> Vec<ApiHeaderContractSpec> {
    endpoint_catalog()
        .iter()
        .filter_map(|endpoint| match endpoint.obligations.headers {
            EndpointObligation::Contract(contract_id) if contract_id.ends_with(".headers") => {
                Some(ApiHeaderContractSpec {
                    contract_id,
                    endpoint,
                    profile: crate::board_catalog::header_profile(endpoint.operation_id)
                        .or_else(|| crate::history_catalog::header_profile(endpoint.operation_id))
                        .or_else(|| {
                            crate::dependency_catalog::header_profile(endpoint.operation_id)
                        })
                        .or_else(|| crate::step_catalog::header_profile(endpoint.operation_id))
                        .or_else(|| crate::task_catalog::header_profile(endpoint.operation_id))
                        .or_else(|| crate::labels_catalog::header_profile(endpoint.operation_id))
                        .unwrap_or_else(|| header_profile(endpoint)),
                })
            }
            _ => None,
        })
        .collect()
}

pub(crate) fn header_operation_contracts() -> Vec<OperationContract> {
    api_header_contract_specs()
        .into_iter()
        .map(|spec| {
            if let Some(contract) = crate::board_catalog::header_contract(spec.endpoint.operation_id)
            {
                return contract;
            }
            if let Some(contract) = crate::history_catalog::header_contract(spec.endpoint.operation_id) {
                return contract;
            }
            if let Some(contract) =
                crate::dependency_catalog::header_contract(spec.endpoint.operation_id)
            {
                return contract;
            }
            if let Some(contract) = crate::step_catalog::header_contract(spec.endpoint.operation_id) {
                return contract;
            }
            if let Some(contract) = crate::task_catalog::header_contract(spec.endpoint.operation_id) {
                return contract;
            }
            if let Some(contract) = crate::labels_catalog::header_contract(spec.endpoint.operation_id) {
                return contract;
            }
            let operation = leak(format!(
                "{} {}",
                method_name(spec.endpoint.method),
                spec.endpoint.path
            ));
            let fixture = fixture_path(spec.profile, true);
            OperationContract {
                id: spec.contract_id,
                path: leak(format!("{operation} headers")),
                surface: ContractSurface::Api,
                operation,
                direction: ContractDirection::Deserialize,
                granularity: ContractGranularity::Exact,
                strictness: ContractStrictness::DenyUnknownFields,
                schema_id: Some(schema_id(spec.endpoint.operation_id)),
                fixture: Some(fixture),
                adoption: Some(AdoptionEvidence {
                    producer_fixture: fixture,
                    producer: AdoptionWitness {
                        operation,
                        contract_id: spec.contract_id,
                        surface: ContractSurface::Api,
                        direction: ContractDirection::Deserialize,
                        package: "kanban-server",
                        test_target: "all",
                        exact_test: "suite::header_adoption::exact_header_profile_fixtures_are_produced",
                    },
                    consumer: AdoptionWitness {
                        operation,
                        contract_id: spec.contract_id,
                        surface: ContractSurface::Api,
                        direction: ContractDirection::Deserialize,
                        package: "kanban-server",
                        test_target: "all",
                        exact_test: "suite::header_adoption::exact_header_profiles_are_consumed_by_real_router",
                    },
                }),
                exclusion: None,
                migration: MigrationState::Adopted,
                transport: ContractTransport::Http {
                    operation_key: Some(operation),
                    location: HttpTransportLocation::Headers,
                    parameters: spec.profile.parameters(),
                },
                binding: ContractBinding::ExactSurface,
            }
        })
        .collect()
}

pub(crate) fn schema_id(operation_id: &str) -> &'static str {
    leak(format!(
        "urn:kanban-tool:schema:api:{}-headers:v1",
        operation_id.trim_start_matches("api.")
    ))
}

#[cfg(feature = "schema")]
pub(crate) fn artifact_path(operation_id: &str) -> &'static str {
    leak(format!(
        "api/{}-headers.v1.schema.json",
        operation_id.trim_start_matches("api.")
    ))
}

pub(crate) fn fixture_path(profile: ApiHeaderProfile, valid: bool) -> &'static str {
    leak(format!(
        "schemas/fixtures/api/headers/{}.v1.{}.json",
        profile.fixture_stem(),
        if valid { "valid" } else { "invalid" }
    ))
}

fn method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn leak(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

fn header_profile(endpoint: &EndpointDescriptor) -> ApiHeaderProfile {
    let actor = ACTOR_OPERATIONS.contains(&endpoint.operation_id);
    let body = matches!(endpoint.obligations.body, EndpointObligation::Contract(_));
    let optional_body = OPTIONAL_JSON_BODY_OPERATIONS.contains(&endpoint.operation_id);
    assert!(
        !optional_body || actor,
        "optional JSON endpoint must declare actor header semantics: {}",
        endpoint.operation_id
    );
    match (actor, body, optional_body) {
        (false, false, _) => ApiHeaderProfile::Locale,
        (true, false, _) => ApiHeaderProfile::LocaleActor,
        (false, true, false) => ApiHeaderProfile::LocaleJson,
        (true, true, false) => ApiHeaderProfile::LocaleActorJson,
        (false, true, true) => unreachable!("optional body actor invariant checked above"),
        (true, true, true) => ApiHeaderProfile::LocaleActorOptionalJson,
    }
}

#[cfg(feature = "schema")]
macro_rules! header_wire {
    ($name:ident { $($field:ident => $wire:literal : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $(#[serde(rename = $wire)] pub $field: $ty,)*
        }
    };
}

#[cfg(feature = "schema")]
header_wire!(LocaleHeaders {
    accept_language => "Accept-Language": Option<String>,
});
#[cfg(feature = "schema")]
header_wire!(LocaleActorHeaders {
    accept_language => "Accept-Language": Option<String>,
    actor => "X-KB-Actor": Option<String>,
});
#[cfg(feature = "schema")]
header_wire!(LocaleJsonHeaders {
    accept_language => "Accept-Language": Option<String>,
    content_type => "Content-Type": String,
});
#[cfg(feature = "schema")]
header_wire!(LocaleActorJsonHeaders {
    accept_language => "Accept-Language": Option<String>,
    actor => "X-KB-Actor": Option<String>,
    content_type => "Content-Type": String,
});
#[cfg(feature = "schema")]
header_wire!(LocaleActorOptionalJsonHeaders {
    accept_language => "Accept-Language": Option<String>,
    actor => "X-KB-Actor": Option<String>,
    content_type => "Content-Type": Option<String>,
});
