#[cfg(feature = "schema")]
use serde::{Deserialize, Serialize};

use crate::{EndpointDescriptor, WireParameter, WireParameterCardinality, endpoint_catalog};

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
    crate::operation_catalog::operation_catalog()
        .iter()
        .filter(|operation| operation.surface == crate::ContractSurface::Api)
        .filter_map(|operation| {
            let contract_id = operation
                .contracts
                .iter()
                .find(|contract| contract.location == Some(crate::HttpTransportLocation::Headers))
                .map(|contract| contract.id)?;
            let endpoint = endpoint_catalog()
                .iter()
                .find(|endpoint| endpoint.operation_id == operation.operation_id)?;
            Some(ApiHeaderContractSpec {
                contract_id,
                endpoint,
                profile: operation.header_profile?,
            })
        })
        .collect()
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
