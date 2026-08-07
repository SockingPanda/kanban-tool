use serde::Serialize;

use crate::ContractSurface;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "contract_id")]
pub enum EndpointObligation {
    Contract(&'static str),
    NotApplicable,
    Excluded { reason: &'static str },
    Todo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EndpointObligations {
    pub path: EndpointObligation,
    pub query: EndpointObligation,
    pub headers: EndpointObligation,
    pub body: EndpointObligation,
    pub success: EndpointObligation,
    pub sse: EndpointObligation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EndpointDescriptor {
    pub operation_id: &'static str,
    pub surface: ContractSurface,
    pub method: HttpMethod,
    pub path: &'static str,
    pub exclusion: Option<&'static str>,
    pub shared_components: &'static [&'static str],
    pub obligations: EndpointObligations,
}

// Endpoint descriptors are projected from the canonical operation declaration source.

// This key-only order preserves the historical endpoint artifact order without duplicating
// method, path, schema, witness, or obligation facts.
const CANONICAL_OPERATION_ORDER: &[&str] = &[
    "api.health",
    "api.list-boards",
    "api.create-board",
    "api.get-board",
    "api.archive-board",
    "api.list-board-columns",
    "api.list-board-labels",
    "api.list-board-label-proposals",
    "api.create-board-label",
    "api.delete-board-label",
    "api.list-label-semantics",
    "api.get-label-semantics",
    "api.upsert-label-semantics",
    "api.delete-label-semantics",
    "api.list-label-atoms",
    "api.explain-label-atom",
    "api.label-atom-index-status",
    "api.rebuild-label-atom-index",
    "api.query-label-atom-index",
    "api.list-tasks",
    "api.list-tasks-by-status",
    "api.create-task",
    "api.list-signals",
    "api.review-signals",
    "api.get-signal",
    "api.record-signal",
    "api.confirm-signals",
    "api.reject-signals",
    "api.resolve-signals",
    "api.supersede-signals",
    "api.board-task-map",
    "api.get-task",
    "api.update-task",
    "api.task-neighborhood",
    "api.list-task-labels",
    "api.add-task-label",
    "api.bootstrap-task-label",
    "api.suggest-task-labels",
    "api.list-task-label-proposals",
    "api.propose-task-label",
    "api.record-label-ontology-observation",
    "api.list-label-ontology-signals",
    "api.review-label-ontology",
    "api.create-label-ontology-action",
    "api.apply-label-ontology-atom",
    "api.revert-label-ontology-mutation",
    "api.validate-label-ontology-action",
    "api.get-label-ontology-signal",
    "api.get-label-proposal",
    "api.accept-label-proposal",
    "api.reject-label-proposal",
    "api.remove-task-label",
    "api.specify-task",
    "api.promote-task",
    "api.claim-task",
    "api.reopen-task",
    "api.reclaim-task",
    "api.heartbeat-task",
    "api.release-task",
    "api.complete-task",
    "api.submit-review-task",
    "api.block-task",
    "api.unblock-task",
    "api.archive-task",
    "api.list-dependencies",
    "api.add-dependency",
    "api.remove-dependency",
    "api.list-steps",
    "api.create-step",
    "api.update-step",
    "api.remove-step",
    "api.complete-step",
    "api.skip-step",
    "api.reopen-step",
    "api.mark-execution-plan-not-required",
    "api.list-runs",
    "api.get-run",
    "api.get-run-log",
    "api.list-comments",
    "api.create-comment",
    "api.list-attachments",
    "api.create-attachment",
    "api.download-attachment",
    "api.delete-attachment",
    "api.get-stats",
    "api.search-tasks",
    "api.search-tasks-by-status",
    "api.search-status",
    "api.rebuild-search-index",
    "api.sync-search-index",
    "api.build-context",
    "api.graph-status",
    "api.graph-neighbors",
    "api.graph-query",
    "api.graph-rebuild",
    "api.graph-sync",
    "api.list-entities",
    "api.upsert-entity",
    "api.get-entity",
    "api.vector-status",
    "api.vector-configure",
    "api.vector-rebuild",
    "api.vector-sync",
    "api.vector-query-chunks",
    "api.vector-query-label-atoms",
    "api.list-events",
    "sse.stream-events",
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
];

pub fn endpoint_catalog() -> &'static [EndpointDescriptor] {
    static CATALOG: std::sync::OnceLock<Vec<EndpointDescriptor>> = std::sync::OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let mut catalog =
                crate::CatalogProjection::new(crate::operation_catalog::operation_catalog())
                    .endpoints();
            catalog.sort_by_key(|endpoint| {
                CANONICAL_OPERATION_ORDER
                    .iter()
                    .position(|operation_id| *operation_id == endpoint.operation_id)
                    .unwrap_or(CANONICAL_OPERATION_ORDER.len())
            });
            catalog
        })
        .as_slice()
}

pub fn endpoint_descriptor(operation_id: &str) -> Option<&'static EndpointDescriptor> {
    endpoint_catalog()
        .iter()
        .find(|endpoint| endpoint.operation_id == operation_id)
}

pub fn endpoint_obligation_todo_count(catalog: &[EndpointDescriptor]) -> usize {
    catalog
        .iter()
        .flat_map(|endpoint| {
            endpoint
                .obligations
                .entries()
                .into_iter()
                .map(|(_, obligation)| obligation)
        })
        .filter(|obligation| matches!(obligation, EndpointObligation::Todo))
        .count()
}

pub fn validate_endpoint_catalog(catalog: &[EndpointDescriptor]) -> Result<(), String> {
    validate_contract_topology(catalog, crate::operation_inventory())
}

pub fn validate_operation_contracts(
    contract_inventory: &[crate::OperationContract],
) -> Result<(), String> {
    validate_contract_inventory_transport(contract_inventory)
}

fn validate_contract_inventory_transport(
    contract_inventory: &[crate::OperationContract],
) -> Result<(), String> {
    let mut contract_ids = std::collections::BTreeMap::new();
    for (index, contract) in contract_inventory.iter().enumerate() {
        if let Some(first_index) = contract_ids.insert(contract.id, index) {
            return Err(format!(
                "duplicate operation contract id: contract={} first={} second={}",
                contract.id, first_index, index
            ));
        }
        validate_contract_transport(contract)?;
    }
    Ok(())
}

pub fn validate_contract_topology(
    catalog: &[EndpointDescriptor],
    contract_inventory: &[crate::OperationContract],
) -> Result<(), String> {
    validate_contract_inventory_transport(contract_inventory)?;

    let mut operation_ids = std::collections::BTreeMap::new();
    let mut method_paths = std::collections::BTreeMap::new();
    let contracts = contract_inventory
        .iter()
        .map(|contract| (contract.id, contract))
        .collect::<std::collections::BTreeMap<_, _>>();

    for endpoint in catalog {
        if endpoint.operation_id.is_empty() || endpoint.path.is_empty() {
            return Err("endpoint descriptor contains empty operation_id/path".to_owned());
        }
        if !matches!(
            endpoint.surface,
            ContractSurface::Api | ContractSurface::Sse
        ) {
            return Err(format!(
                "endpoint has non-transport surface: endpoint={} expected=api_or_sse actual={}",
                endpoint.operation_id,
                contract_surface_name(endpoint.surface)
            ));
        }
        if let Some((first_method, first_path)) =
            operation_ids.insert(endpoint.operation_id, (endpoint.method, endpoint.path))
        {
            return Err(format!(
                "duplicate endpoint operation_id: operation_id={} first={} {} second={} {}",
                endpoint.operation_id,
                http_method_name(first_method),
                first_path,
                http_method_name(endpoint.method),
                endpoint.path
            ));
        }
        if let Some(first_operation) =
            method_paths.insert((endpoint.method, endpoint.path), endpoint.operation_id)
        {
            return Err(format!(
                "duplicate endpoint method/path: expected=unique actual={} {} first={} second={}",
                http_method_name(endpoint.method),
                endpoint.path,
                first_operation,
                endpoint.operation_id
            ));
        }
        if endpoint
            .exclusion
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(format!(
                "endpoint exclusion reason must be non-empty: {}",
                endpoint.operation_id
            ));
        }

        for (kind, obligation) in endpoint.obligations.entries() {
            validate_obligation(endpoint, kind, obligation, &contracts)?;
        }
        validate_shared_component_links(endpoint, &contracts)?;
    }

    Ok(())
}

fn validate_contract_transport(contract: &crate::OperationContract) -> Result<(), String> {
    use crate::{
        ContractBinding, ContractDirection, ContractSurface, ContractTransport,
        HttpTransportLocation, WireParameterCardinality,
    };

    if contract.binding == ContractBinding::ExactSurface
        && contract.granularity != crate::ContractGranularity::Exact
    {
        return Err(format!(
            "ExactSurface contract requires exact granularity: contract={} binding=exact_surface expected=exact actual={}",
            contract.id,
            contract_granularity_name(contract.granularity)
        ));
    }

    let (operation_key, location, parameters) = match contract.transport {
        ContractTransport::NoTransport => {
            if matches!(
                contract.surface,
                ContractSurface::Api | ContractSurface::Sse
            ) {
                return Err(format!(
                    "HTTP contract must declare transport metadata: {}",
                    contract.id
                ));
            }
            return Ok(());
        }
        ContractTransport::Http {
            operation_key,
            location,
            parameters,
        } => {
            if !matches!(
                contract.surface,
                ContractSurface::Api | ContractSurface::Sse
            ) {
                return Err(format!(
                    "non-HTTP contract must declare no_transport: {}",
                    contract.id
                ));
            }
            (operation_key, location, parameters)
        }
    };

    if contract.surface == ContractSurface::Api && location == HttpTransportLocation::Sse {
        return Err(format!(
            "transport location sse is incompatible with api surface: {}",
            contract.id
        ));
    }
    if location == HttpTransportLocation::Error
        && contract.binding != ContractBinding::SharedComponent
    {
        return Err(format!(
            "error transport requires SharedComponent binding: contract={} location=error expected=shared_component actual={}",
            contract.id,
            contract_binding_name(contract.binding)
        ));
    }
    match contract.binding {
        ContractBinding::ExactSurface if operation_key.is_none_or(|key| key.trim().is_empty()) => {
            return Err(format!(
                "ExactSurface HTTP contract must name an operation_key: {}",
                contract.id
            ));
        }
        ContractBinding::SharedComponent if operation_key.is_some() => {
            return Err(format!(
                "SharedComponent HTTP contract must not claim an exact operation_key: {}",
                contract.id
            ));
        }
        _ => {}
    }

    let expected_direction = match location {
        HttpTransportLocation::Path
        | HttpTransportLocation::Query
        | HttpTransportLocation::Headers
        | HttpTransportLocation::Body => ContractDirection::Deserialize,
        HttpTransportLocation::Success
        | HttpTransportLocation::Error
        | HttpTransportLocation::Sse => ContractDirection::Serialize,
    };
    if contract.direction != expected_direction {
        return Err(format!(
            "contract transport direction does not match location {}: contract={} location={} expected={} actual={}",
            transport_location_name(location),
            contract.id,
            transport_location_name(location),
            contract_direction_name(expected_direction),
            contract_direction_name(contract.direction)
        ));
    }

    if !matches!(
        location,
        HttpTransportLocation::Path | HttpTransportLocation::Query | HttpTransportLocation::Headers
    ) && !parameters.is_empty()
    {
        return Err(format!(
            "transport parameters forbidden: contract={} location={} expected=none actual_count={}",
            contract.id,
            transport_location_name(location),
            parameters.len()
        ));
    }

    let mut names = std::collections::BTreeMap::new();
    for (index, parameter) in parameters.iter().enumerate() {
        let name = parameter.name.trim();
        if name.is_empty() {
            return Err(format!(
                "wire parameter name must be non-empty: contract={} location={} parameter_index={} expected=non-empty actual={:?}",
                contract.id,
                transport_location_name(location),
                index,
                parameter.name
            ));
        }
        if name != parameter.name {
            return Err(format!(
                "wire parameter name must not contain surrounding whitespace: contract={} location={} parameter_index={} expected=without_surrounding_whitespace actual={:?}",
                contract.id,
                transport_location_name(location),
                index,
                parameter.name
            ));
        }
        let identity = if location == HttpTransportLocation::Headers {
            name.to_ascii_lowercase()
        } else {
            name.to_owned()
        };
        if let Some((first_name, first_index)) = names.get(&identity) {
            return Err(format!(
                "wire parameter name conflict: contract={} location={} first={} first_index={} second={} second_index={}",
                contract.id,
                transport_location_name(location),
                first_name,
                first_index,
                name,
                index
            ));
        }
        names.insert(identity, (name, index));
        let cardinality = parameter.cardinality.ok_or_else(|| {
            format!(
                "wire parameter missing cardinality: contract={} location={} parameter={} parameter_index={} expected=some actual=none",
                contract.id,
                transport_location_name(location),
                name,
                index
            )
        })?;
        if location == HttpTransportLocation::Path
            && cardinality != WireParameterCardinality::RequiredOne
        {
            return Err(format!(
                "path parameter cardinality must be required_one: contract={} location=path parameter={} expected=required_one actual={}",
                contract.id,
                name,
                wire_parameter_cardinality_name(cardinality)
            ));
        }
    }
    Ok(())
}

fn validate_obligation(
    endpoint: &EndpointDescriptor,
    kind: EndpointObligationKind,
    obligation: EndpointObligation,
    contracts: &std::collections::BTreeMap<&str, &crate::OperationContract>,
) -> Result<(), String> {
    if matches!(kind, EndpointObligationKind::Path)
        && endpoint.path.contains(':')
        && matches!(obligation, EndpointObligation::NotApplicable)
    {
        return Err(format!(
            "parameterized endpoint path must remain Todo or have a path contract: {}",
            endpoint.operation_id
        ));
    }
    match obligation {
        EndpointObligation::Todo => {}
        EndpointObligation::Excluded { reason } if reason.trim().is_empty() => {
            return Err(format!(
                "endpoint obligation exclusion reason must be non-empty: {} {}",
                endpoint.operation_id,
                kind.name()
            ));
        }
        EndpointObligation::Contract(contract_id) => {
            let contract = contracts.get(contract_id).ok_or_else(|| {
                format!(
                    "endpoint obligation references unknown contract: {} {} -> {}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id
                )
            })?;
            if contract.binding != crate::ContractBinding::ExactSurface {
                return Err(format!(
                    "endpoint obligation requires ExactSurface contract: endpoint={} obligation={} contract={} expected=exact_surface actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    contract_binding_name(contract.binding)
                ));
            }
            if contract.granularity != crate::ContractGranularity::Exact {
                return Err(format!(
                    "endpoint obligation requires exact granularity: endpoint={} obligation={} contract={} binding={} expected=exact actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    contract_binding_name(contract.binding),
                    contract_granularity_name(contract.granularity)
                ));
            }
            let expected_direction = if kind.is_input() {
                crate::ContractDirection::Deserialize
            } else {
                crate::ContractDirection::Serialize
            };
            if contract.direction != expected_direction {
                return Err(format!(
                    "endpoint obligation contract has wrong direction: endpoint={} obligation={} contract={} expected={} actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    contract_direction_name(expected_direction),
                    contract_direction_name(contract.direction)
                ));
            }
            if contract.surface != endpoint.surface {
                return Err(format!(
                    "endpoint obligation contract has wrong surface: endpoint={} obligation={} contract={} expected={} actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    contract_surface_name(endpoint.surface),
                    contract_surface_name(contract.surface)
                ));
            }
            let (operation_key, location, parameters) = match contract.transport {
                crate::ContractTransport::Http {
                    operation_key,
                    location,
                    parameters,
                } => (operation_key, location, parameters),
                crate::ContractTransport::NoTransport => {
                    return Err(format!(
                        "endpoint obligation contract lacks HTTP transport: endpoint={} obligation={} contract={} expected=http actual=no_transport",
                        endpoint.operation_id,
                        kind.name(),
                        contract_id
                    ));
                }
            };
            let expected_location = kind.location();
            if location != expected_location {
                return Err(format!(
                    "contract location {} does not match obligation {}: endpoint={} obligation={} contract={} expected={} actual={}",
                    transport_location_name(location),
                    kind.name(),
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    transport_location_name(expected_location),
                    transport_location_name(location)
                ));
            }
            let endpoint_key = endpoint_operation_key(endpoint);
            if operation_key != Some(endpoint_key.as_str()) {
                return Err(format!(
                    "contract operation does not match endpoint: endpoint={} obligation={} contract={} expected={} actual={}",
                    endpoint.operation_id,
                    kind.name(),
                    contract_id,
                    endpoint_key,
                    operation_key.unwrap_or("<none>")
                ));
            }
            if kind == EndpointObligationKind::Path {
                validate_path_parameter_mapping(endpoint, contract_id, parameters)?;
            }
            if kind == EndpointObligationKind::Sse && endpoint.surface != ContractSurface::Sse {
                return Err(format!(
                    "SSE contract obligation is only valid on SSE endpoint: {}",
                    endpoint.operation_id
                ));
            }
        }
        _ => {}
    }
    if kind == EndpointObligationKind::Sse
        && endpoint.surface != ContractSurface::Sse
        && !matches!(
            obligation,
            EndpointObligation::NotApplicable | EndpointObligation::Excluded { .. }
        )
    {
        return Err(format!(
            "non-SSE endpoint must mark SSE obligation NotApplicable or Excluded: {}",
            endpoint.operation_id
        ));
    }
    if kind == EndpointObligationKind::Sse
        && endpoint.surface == ContractSurface::Sse
        && matches!(obligation, EndpointObligation::NotApplicable)
    {
        return Err(format!(
            "SSE endpoint must describe SSE obligation: {}",
            endpoint.operation_id
        ));
    }
    Ok(())
}

fn validate_shared_component_links(
    endpoint: &EndpointDescriptor,
    contracts: &std::collections::BTreeMap<&str, &crate::OperationContract>,
) -> Result<(), String> {
    let mut linked = std::collections::BTreeMap::new();
    for (index, contract_id) in endpoint.shared_components.iter().enumerate() {
        if let Some(first_index) = linked.insert(*contract_id, index) {
            return Err(format!(
                "duplicate shared component link: endpoint={} contract={} first={} second={}",
                endpoint.operation_id, contract_id, first_index, index
            ));
        }
        let contract = contracts.get(contract_id).ok_or_else(|| {
            format!(
                "endpoint shared component references unknown contract: {} -> {}",
                endpoint.operation_id, contract_id
            )
        })?;
        if contract.binding != crate::ContractBinding::SharedComponent {
            return Err(format!(
                "shared component link requires SharedComponent contract: endpoint={} contract={} expected=shared_component actual={}",
                endpoint.operation_id,
                contract_id,
                contract_binding_name(contract.binding)
            ));
        }
        if contract.surface != endpoint.surface {
            return Err(format!(
                "shared component link has wrong surface: endpoint={} contract={} expected={} actual={}",
                endpoint.operation_id,
                contract_id,
                contract_surface_name(endpoint.surface),
                contract_surface_name(contract.surface)
            ));
        }
        if !matches!(contract.transport, crate::ContractTransport::Http { .. }) {
            return Err(format!(
                "shared component link lacks HTTP transport: endpoint={} contract={} expected=http actual=no_transport",
                endpoint.operation_id, contract_id
            ));
        }
    }
    Ok(())
}

fn validate_path_parameter_mapping(
    endpoint: &EndpointDescriptor,
    contract_id: &str,
    parameters: &[crate::WireParameter],
) -> Result<(), String> {
    let placeholders = endpoint
        .path
        .split('/')
        .filter_map(|segment| segment.strip_prefix(':'))
        .collect::<Vec<_>>();
    let declared = parameters
        .iter()
        .map(|parameter| parameter.name)
        .collect::<Vec<_>>();
    if declared != placeholders {
        return Err(format!(
            "path parameter set does not match endpoint placeholders: endpoint={} obligation=path contract={} declared={declared:?} expected={placeholders:?}",
            endpoint.operation_id, contract_id
        ));
    }
    Ok(())
}

fn endpoint_operation_key(endpoint: &EndpointDescriptor) -> String {
    format!("{} {}", http_method_name(endpoint.method), endpoint.path)
}

fn http_method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn transport_location_name(location: crate::HttpTransportLocation) -> &'static str {
    match location {
        crate::HttpTransportLocation::Path => "path",
        crate::HttpTransportLocation::Query => "query",
        crate::HttpTransportLocation::Headers => "headers",
        crate::HttpTransportLocation::Body => "body",
        crate::HttpTransportLocation::Success => "success",
        crate::HttpTransportLocation::Error => "error",
        crate::HttpTransportLocation::Sse => "sse",
    }
}

fn contract_direction_name(direction: crate::ContractDirection) -> &'static str {
    match direction {
        crate::ContractDirection::Serialize => "serialize",
        crate::ContractDirection::Deserialize => "deserialize",
        crate::ContractDirection::Bidirectional => "bidirectional",
    }
}

fn contract_surface_name(surface: crate::ContractSurface) -> &'static str {
    match surface {
        crate::ContractSurface::Api => "api",
        crate::ContractSurface::Cli => "cli",
        crate::ContractSurface::Jsonl => "jsonl",
        crate::ContractSurface::Sse => "sse",
        crate::ContractSurface::Metadata => "metadata",
        crate::ContractSurface::Config => "config",
    }
}

fn contract_binding_name(binding: crate::ContractBinding) -> &'static str {
    match binding {
        crate::ContractBinding::ExactSurface => "exact_surface",
        crate::ContractBinding::SharedComponent => "shared_component",
    }
}

fn contract_granularity_name(granularity: crate::ContractGranularity) -> &'static str {
    match granularity {
        crate::ContractGranularity::Exact => "exact",
        crate::ContractGranularity::Family => "family",
    }
}

fn wire_parameter_cardinality_name(cardinality: crate::WireParameterCardinality) -> &'static str {
    match cardinality {
        crate::WireParameterCardinality::RequiredOne => "required_one",
        crate::WireParameterCardinality::OptionalOne => "optional_one",
        crate::WireParameterCardinality::RepeatedOrdered => "repeated_ordered",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointObligationKind {
    Path,
    Query,
    Headers,
    Body,
    Success,
    Sse,
}

impl EndpointObligationKind {
    const fn is_input(self) -> bool {
        matches!(self, Self::Path | Self::Query | Self::Headers | Self::Body)
    }
    const fn name(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Query => "query",
            Self::Headers => "headers",
            Self::Body => "body",
            Self::Success => "success",
            Self::Sse => "sse",
        }
    }
    pub(crate) const fn location(self) -> crate::HttpTransportLocation {
        match self {
            Self::Path => crate::HttpTransportLocation::Path,
            Self::Query => crate::HttpTransportLocation::Query,
            Self::Headers => crate::HttpTransportLocation::Headers,
            Self::Body => crate::HttpTransportLocation::Body,
            Self::Success => crate::HttpTransportLocation::Success,
            Self::Sse => crate::HttpTransportLocation::Sse,
        }
    }
}

impl EndpointObligations {
    pub const fn entries(self) -> [(EndpointObligationKind, EndpointObligation); 6] {
        [
            (EndpointObligationKind::Path, self.path),
            (EndpointObligationKind::Query, self.query),
            (EndpointObligationKind::Headers, self.headers),
            (EndpointObligationKind::Body, self.body),
            (EndpointObligationKind::Success, self.success),
            (EndpointObligationKind::Sse, self.sse),
        ]
    }
}
