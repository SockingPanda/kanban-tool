//! Browser-first Web host metadata 的唯一 declaration source。
//!
//! `/app/runtime.json` 是同源 host metadata，不是 `/api/v1` operation。使用 `Config`
//! surface 让它进入现有 contract/schema registry，同时保持 endpoint catalog 只投影
//! API/SSE transport；因此 host metadata 不会伪装成 API endpoint。

use crate::{
    ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, OperationContract, OperationDeclaration, SurfaceOperation,
};

const RUNTIME_WEB_CONFIG_CONTRACTS: &[ContractDeclaration] = &[{
    let contract = ContractDeclaration::new(
        "runtime.web-config.output",
        "/app/runtime.json",
        ContractDirection::Serialize,
        None,
        ContractStrictness::DenyUnknownFields,
        ContractGranularity::Exact,
        ContractBinding::ExactSurface,
    )
    .with_operation("GET /app/runtime.json")
    .with_schema(
        "urn:kanban-tool:schema:runtime:web-config:v1",
        "runtime/web-config.v1.schema.json",
        "Kanban Web runtime config v1",
        "schemas/fixtures/runtime/web-config.v1.valid.json",
        "schemas/fixtures/runtime/web-config.v1.invalid.json",
    );
    #[cfg(feature = "schema")]
    let contract = contract.with_schema_type::<crate::WebRuntimeConfig>();
    contract
}];

const RUNTIME_OPERATIONS: &[OperationDeclaration] = &[OperationDeclaration::new(
    "runtime.web-config",
    ContractSurface::Config,
    None,
    None,
    "GET /app/runtime.json",
    "GET /app/runtime.json",
    RUNTIME_WEB_CONFIG_CONTRACTS,
)];

/// 返回 Web host metadata 的声明 source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    RUNTIME_OPERATIONS
}

/// 返回 Web host metadata 的 contract projection。
pub fn operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(RUNTIME_OPERATIONS).contracts()
}

/// 返回非 HTTP host metadata 的 surface projection。
pub fn surface_catalog() -> Vec<SurfaceOperation> {
    crate::CatalogProjection::new(RUNTIME_OPERATIONS).surfaces()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_metadata_is_config_surface_without_endpoint_projection() {
        let operation = operation_declarations()
            .first()
            .expect("runtime operation declaration");
        assert_eq!(operation.operation_id, "runtime.web-config");
        assert_eq!(operation.surface, ContractSurface::Config);
        assert!(operation.method.is_none());
        assert!(operation.path.is_none());

        let contract = operation_contracts()
            .into_iter()
            .next()
            .expect("runtime contract");
        assert_eq!(contract.id, "runtime.web-config.output");
        assert_eq!(contract.direction, ContractDirection::Serialize);
        assert_eq!(contract.surface, ContractSurface::Config);
        assert_eq!(contract.transport, crate::ContractTransport::NoTransport);
        assert_eq!(surface_catalog()[0].key, "GET /app/runtime.json");
        assert!(
            crate::runtime_catalog::operation_declarations()
                .iter()
                .all(|declaration| declaration.method.is_none())
        );
    }
}
