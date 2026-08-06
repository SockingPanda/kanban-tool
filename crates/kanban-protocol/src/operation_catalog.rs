//! operation declaration source 与 deterministic projection facade。
//!
//! 该模块刻意不复制现有 endpoint/inventory/schema rows。第一阶段只提供一个可供后续
//! domain 文件填充的 declaration source，以及把 source 投影到既有 public shapes 的
//! 薄 facade。声明数组的书写顺序就是 canonical order；projection 不经过无序 map，也
//! 不按 operation id 猜测 schema/type。

use crate::{
    AdoptionWitness, EndpointDescriptor, OperationContract, OperationDeclaration, SurfaceOperation,
};

/// 声明 source 的 projection 视图。
#[derive(Debug, Clone, Copy)]
pub struct CatalogProjection<'a> {
    declarations: &'a [OperationDeclaration],
}

impl<'a> CatalogProjection<'a> {
    /// 从静态 operation declaration source 创建 projection。
    pub const fn new(declarations: &'a [OperationDeclaration]) -> Self {
        Self { declarations }
    }

    /// 逐项投影 API/SSE parent；结果保留 declaration source 的顺序。
    pub fn endpoints(&self) -> Vec<EndpointDescriptor> {
        self.declarations
            .iter()
            .filter(|declaration| {
                matches!(
                    declaration.surface,
                    crate::ContractSurface::Api | crate::ContractSurface::Sse
                )
            })
            .map(OperationDeclaration::endpoint_descriptor)
            .collect()
    }

    /// 逐 parent、逐 child 投影 operation inventory；结果保留 source 顺序。
    pub fn contracts(&self) -> Vec<OperationContract> {
        self.declarations
            .iter()
            .flat_map(OperationDeclaration::operation_contracts)
            .collect()
    }

    /// 逐 parent 投影 surface operation；结果保留 source 顺序。
    pub fn surfaces(&self) -> Vec<SurfaceOperation> {
        self.declarations
            .iter()
            .map(OperationDeclaration::surface_operation)
            .collect()
    }

    /// 展开所有 producer/consumer witness；结果保留 source 与 child 顺序。
    pub fn adoption_witnesses(&self) -> Vec<AdoptionWitness> {
        self.declarations
            .iter()
            .flat_map(|parent| {
                parent.contracts.iter().flat_map(|contract| {
                    [contract.producer, contract.consumer]
                        .into_iter()
                        .flatten()
                        .map(|locator| contract.adoption_witness(parent, locator))
                })
            })
            .collect()
    }

    /// 在 `schema` feature 下投影所有显式 schema declarations。
    #[cfg(feature = "schema")]
    pub fn schemas(&self) -> Vec<crate::schema::SchemaRoot> {
        self.declarations
            .iter()
            .flat_map(|parent| parent.contracts.iter())
            .filter_map(crate::ContractDeclaration::schema_root)
            .collect()
    }

    /// 返回 declaration source，便于 compatibility test 进行唯一性检查。
    pub const fn declarations(&self) -> &'a [OperationDeclaration] {
        self.declarations
    }
}

/// 已迁移的 Board source；`operation_catalog()` 会按 family 顺序汇总所有已迁移声明。
///
/// 该常量保留给旧的编译期 consumer；新增 consumer 应使用 `operation_catalog()`，以免
/// 忽略后续迁移的 family。
pub const OPERATION_DECLARATIONS: &[OperationDeclaration] =
    crate::board_catalog::operation_declarations();

/// 返回当前 declaration source。
pub fn operation_catalog() -> &'static [OperationDeclaration] {
    static ALL: std::sync::OnceLock<Vec<OperationDeclaration>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let mut declarations = Vec::with_capacity(
            crate::board_catalog::operation_declarations().len()
                + crate::task_catalog::operation_declarations().len(),
        );
        declarations.extend_from_slice(crate::board_catalog::operation_declarations());
        declarations.extend_from_slice(crate::task_catalog::operation_declarations());
        declarations
    })
    .as_slice()
}

/// 使用任意 declaration source 创建 projection。
pub const fn project<'a>(declarations: &'a [OperationDeclaration]) -> CatalogProjection<'a> {
    CatalogProjection::new(declarations)
}

/// 声明一个静态 operation source。
///
/// 支持 `operation_catalog!(pub static NAME: &[OperationDeclaration] = &[...]);` 与不带
/// `pub` 的同形写法。数组顺序是 canonical order，macro 不会生成 handler 或 schema
/// type inference。
#[macro_export]
macro_rules! operation_catalog {
    ($vis:vis static $name:ident : &[OperationDeclaration] = $value:expr $(;)?) => {
        $vis static $name: &'static [$crate::OperationDeclaration] = $value;
    };
    ($vis:vis const $name:ident : &[OperationDeclaration] = $value:expr $(;)?) => {
        $vis const $name: &'static [$crate::OperationDeclaration] = $value;
    };
}

/// 声明一个静态 child contract source。
///
/// 该 macro 与 `operation_catalog!` 分开，使按领域拆分的 source 可以先定义 child
/// rows，再在 parent declaration 中引用；不会创建第二个 projection 或 registry。
#[macro_export]
macro_rules! contract_catalog {
    ($vis:vis static $name:ident : &[ContractDeclaration] = $value:expr $(;)?) => {
        $vis static $name: &'static [$crate::ContractDeclaration] = $value;
    };
    ($vis:vis const $name:ident : &[ContractDeclaration] = $value:expr $(;)?) => {
        $vis const $name: &'static [$crate::ContractDeclaration] = $value;
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
        ContractStrictness, ContractSurface, EndpointObligation, EndpointObligationKind,
        HttpMethod, HttpTransportLocation, MigrationState,
    };

    const QUERY: ContractDeclaration = ContractDeclaration {
        id: "api.example.query",
        path: "GET /api/v1/example query",
        operation: None,
        location: Some(HttpTransportLocation::Query),
        operation_key: None,
        parameters: &[],
        direction: ContractDirection::Deserialize,
        strictness: ContractStrictness::DenyUnknownFields,
        granularity: ContractGranularity::Exact,
        binding: ContractBinding::ExactSurface,
        schema_id: None,
        artifact_path: None,
        schema_title: None,
        valid_fixture: None,
        invalid_fixture: None,
        #[cfg(feature = "schema")]
        schema_generator: None,
        producer: None,
        consumer: None,
        migration: None,
        exclusion: None,
    };

    const RESPONSE: ContractDeclaration = ContractDeclaration {
        id: "api.example.response",
        path: "GET /api/v1/example response",
        operation: None,
        location: Some(HttpTransportLocation::Success),
        operation_key: None,
        parameters: &[],
        direction: ContractDirection::Serialize,
        strictness: ContractStrictness::DenyUnknownFields,
        granularity: ContractGranularity::Exact,
        binding: ContractBinding::ExactSurface,
        schema_id: None,
        artifact_path: None,
        schema_title: None,
        valid_fixture: None,
        invalid_fixture: None,
        #[cfg(feature = "schema")]
        schema_generator: None,
        producer: None,
        consumer: None,
        migration: None,
        exclusion: None,
    };

    const CONTRACTS: &[ContractDeclaration] = &[QUERY, RESPONSE];
    const OVERRIDES: &[(EndpointObligationKind, EndpointObligation)] = &[(
        EndpointObligationKind::Headers,
        EndpointObligation::Excluded {
            reason: "example override",
        },
    )];
    const OPERATION: OperationDeclaration = OperationDeclaration {
        operation_id: "api.example",
        surface: ContractSurface::Api,
        method: Some(HttpMethod::Get),
        path: Some("/api/v1/example"),
        operation: "GET /api/v1/example",
        key: "GET /api/v1/example",
        migration: MigrationState::Adopted,
        exclusion: None,
        shared_components: &[],
        header_profile: None,
        mcp_policy: None,
        obligation_overrides: OVERRIDES,
        contracts: CONTRACTS,
    };

    #[test]
    fn projection_is_explicit_and_ordered() {
        let projection = CatalogProjection::new(&[OPERATION]);
        let endpoints = projection.endpoints();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].operation_id, "api.example");
        assert_eq!(
            endpoints[0].obligations.query,
            EndpointObligation::Contract("api.example.query")
        );
        assert_eq!(
            endpoints[0].obligations.headers,
            EndpointObligation::Excluded {
                reason: "example override"
            }
        );

        let contracts = projection.contracts();
        assert_eq!(
            contracts
                .iter()
                .map(|contract| contract.id)
                .collect::<Vec<_>>(),
            vec!["api.example.query", "api.example.response"]
        );
        assert_eq!(
            contracts[0].transport,
            crate::ContractTransport::Http {
                operation_key: Some("GET /api/v1/example"),
                location: HttpTransportLocation::Query,
                parameters: &[],
            }
        );
        assert_eq!(projection.surfaces()[0].key, "GET /api/v1/example");
    }

    #[test]
    fn projection_repeats_without_reordering() {
        let first = CatalogProjection::new(&[OPERATION]).contracts();
        let second = CatalogProjection::new(&[OPERATION]).contracts();
        assert_eq!(first, second);
    }

    #[test]
    fn migrated_domain_source_is_exposed_without_legacy_duplication() {
        assert_eq!(operation_catalog().len(), 27);
        assert_eq!(
            operation_catalog()
                .iter()
                .map(|operation| operation.operation_id)
                .collect::<Vec<_>>(),
            crate::board_catalog::operation_declarations()
                .iter()
                .map(|operation| operation.operation_id)
                .chain(
                    crate::task_catalog::operation_declarations()
                        .iter()
                        .map(|operation| operation.operation_id),
                )
                .collect::<Vec<_>>()
        );
    }

    #[cfg(feature = "schema")]
    #[test]
    fn schema_projection_requires_explicit_generator() {
        #[derive(schemars::JsonSchema)]
        #[expect(dead_code, reason = "该测试只需要验证类型驱动的 schema generator")]
        struct ExampleSchema {
            value: String,
        }

        let contract = ContractDeclaration {
            schema_id: Some("urn:example"),
            artifact_path: Some("api/example.schema.json"),
            schema_title: Some("Example"),
            valid_fixture: Some("schemas/fixtures/example.valid.json"),
            invalid_fixture: Some("schemas/fixtures/example.invalid.json"),
            schema_generator: Some(crate::generate_schema_for::<ExampleSchema>),
            ..QUERY
        };
        let root = contract.schema_root().expect("explicit schema root");
        assert_eq!(root.contract_id, contract.id);
        assert_eq!(root.artifact_path, "api/example.schema.json");
    }
}
