use serde::Serialize;

use crate::{ContractSurface, MigrationState, endpoint_catalog};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SurfaceOperation {
    pub key: String,
    pub surface: ContractSurface,
    pub contracts: Vec<&'static str>,
    pub migration: MigrationState,
    pub exclusion: Option<&'static str>,
}

fn non_transport_operations() -> Vec<SurfaceOperation> {
    let mut operations =
        crate::CatalogProjection::new(crate::operation_catalog::operation_catalog())
            .surfaces()
            .into_iter()
            .filter(|operation| {
                !matches!(
                    operation.surface,
                    ContractSurface::Api | ContractSurface::Sse
                )
            })
            .collect::<Vec<_>>();
    operations.push(SurfaceOperation {
        key: "POST /api/v1/maintenance/{operation}".to_owned(),
        surface: ContractSurface::Api,
        contracts: vec!["api.maintenance-path.request"],
        migration: MigrationState::Adopted,
        exclusion: None,
    });
    operations
}

pub fn surface_operation_catalog() -> Vec<SurfaceOperation> {
    let mut operations = endpoint_catalog()
        .iter()
        .map(|endpoint| SurfaceOperation {
            key: String::new(),
            // 下方会对 debug 形式做规范化，以保留历史 catalog key。
            surface: endpoint.surface,
            contracts: endpoint_contract_references(endpoint),
            migration: endpoint.migration,
            exclusion: endpoint.exclusion,
        })
        .collect::<Vec<_>>();
    for (operation, endpoint) in operations.iter_mut().zip(endpoint_catalog()) {
        operation.key = format!(
            "{} {}",
            endpoint_method_name(endpoint.method),
            endpoint.path
        );
    }
    operations.extend(non_transport_operations());
    operations
}

fn endpoint_contract_references(endpoint: &crate::EndpointDescriptor) -> Vec<&'static str> {
    let mut contracts = [
        endpoint.obligations.path,
        endpoint.obligations.query,
        endpoint.obligations.headers,
        endpoint.obligations.body,
        endpoint.obligations.success,
        endpoint.obligations.sse,
    ]
    .into_iter()
    .filter_map(|obligation| match obligation {
        crate::EndpointObligation::Contract(id) => Some(id),
        _ => None,
    })
    .collect::<Vec<_>>();
    contracts.extend_from_slice(endpoint.shared_components);
    contracts
}

fn endpoint_method_name(method: crate::HttpMethod) -> &'static str {
    match method {
        crate::HttpMethod::Get => "GET",
        crate::HttpMethod::Post => "POST",
        crate::HttpMethod::Put => "PUT",
        crate::HttpMethod::Patch => "PATCH",
        crate::HttpMethod::Delete => "DELETE",
    }
}

pub fn surface_operation_keys(surface: ContractSurface) -> impl Iterator<Item = String> {
    surface_operation_catalog()
        .into_iter()
        .filter(move |operation| operation.surface == surface)
        .map(|operation| operation.key)
}
