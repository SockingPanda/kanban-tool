//! operation/contract catalog 的低层声明与投影。
//!
//! 这里没有任何 HTTP handler、CLI command 或 schema registry 的所有权。声明只描述
//! operation 与其 child contract 的事实，具体 adapter 仍需通过已有的 public shape
//! （`EndpointDescriptor`、`OperationContract` 和 `SurfaceOperation`）消费 projection。
//! schema generator 只在 `schema` feature 下携带函数指针，因此默认 wire build 不会
//! 反向依赖 schema registry。

use crate::{
    ContractBinding, ContractDirection, ContractGranularity, ContractStrictness, ContractSurface,
    ContractTransport, EndpointDescriptor, EndpointObligation, EndpointObligationKind,
    EndpointObligations, HttpMethod, HttpTransportLocation, OperationContract, SurfaceOperation,
    WireParameter,
};

/// MCP tool 与 operation 的显式多对多绑定。
///
/// declaration 层只记录绑定和边界政策，不生成 rmcp handler，也不改变 MCP adapter
/// 的真实 tool catalog。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpToolBinding {
    pub tool_name: &'static str,
    pub http_operations: &'static [&'static str],
}

/// parent declaration 的 MCP 暴露政策。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpExposure {
    Domain,
    HostAdmin,
}

/// parent declaration 的 MCP policy。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpPolicy {
    pub exposure: McpExposure,
    pub tool_bindings: &'static [McpToolBinding],
    pub invariants: &'static [crate::McpOperationInvariant],
}

/// `schema` feature 下的显式 schema generator。
///
/// 使用函数指针而非 `TypeId` 或 operation-id 猜测，调用方必须在 declaration 中显式
/// 提供 generator。默认 feature 下不暴露该类型，避免把可选 schemars 依赖带入 wire
/// contract。
#[cfg(feature = "schema")]
pub type SchemaGenerator = fn(crate::ContractDirection) -> serde_json::Value;

/// 一个 child contract 的唯一声明。
///
/// `valid_fixture` 同时是现有 `OperationContract::fixture` 的值；invalid fixture 和
/// artifact path 只用于 schema projection。没有 schema 的 contract 可以保留
/// `schema_id`、artifact 和 fixture 全为空。
#[derive(Debug, Clone, Copy)]
pub struct ContractDeclaration {
    pub id: &'static str,
    pub path: &'static str,
    pub operation: Option<&'static str>,
    pub location: Option<HttpTransportLocation>,
    pub operation_key: Option<&'static str>,
    pub parameters: &'static [WireParameter],
    pub direction: ContractDirection,
    pub strictness: ContractStrictness,
    pub granularity: ContractGranularity,
    pub binding: ContractBinding,
    pub schema_id: Option<&'static str>,
    pub artifact_path: Option<&'static str>,
    pub schema_title: Option<&'static str>,
    pub valid_fixture: Option<&'static str>,
    pub invalid_fixture: Option<&'static str>,
    #[cfg(feature = "schema")]
    pub schema_generator: Option<SchemaGenerator>,
    pub exclusion: Option<&'static str>,
}

impl ContractDeclaration {
    /// 创建一个不带 schema 的最小声明，供无 artifact 的 contract 或 shared component 使用。
    pub const fn new(
        id: &'static str,
        path: &'static str,
        direction: ContractDirection,
        location: Option<HttpTransportLocation>,
        strictness: ContractStrictness,
        granularity: ContractGranularity,
        binding: ContractBinding,
    ) -> Self {
        Self {
            id,
            path,
            operation: None,
            location,
            operation_key: None,
            parameters: &[],
            direction,
            strictness,
            granularity,
            binding,
            schema_id: None,
            artifact_path: None,
            schema_title: None,
            valid_fixture: None,
            invalid_fixture: None,
            #[cfg(feature = "schema")]
            schema_generator: None,
            exclusion: None,
        }
    }

    /// 为声明补充 operation-specific transport 参数。
    pub const fn with_transport(
        mut self,
        operation_key: Option<&'static str>,
        parameters: &'static [WireParameter],
    ) -> Self {
        self.operation_key = operation_key;
        self.parameters = parameters;
        self
    }

    /// 为声明补充 schema artifact/fixture 元数据。
    pub const fn with_schema(
        mut self,
        schema_id: &'static str,
        artifact_path: &'static str,
        schema_title: &'static str,
        valid_fixture: &'static str,
        invalid_fixture: &'static str,
    ) -> Self {
        self.schema_id = Some(schema_id);
        self.artifact_path = Some(artifact_path);
        self.schema_title = Some(schema_title);
        self.valid_fixture = Some(valid_fixture);
        self.invalid_fixture = Some(invalid_fixture);
        self
    }

    /// 为声明补充 operation 文案。
    pub const fn with_operation(mut self, operation: &'static str) -> Self {
        self.operation = Some(operation);
        self
    }

    /// 覆盖该 child 的 exclusion reason。
    pub const fn with_exclusion(mut self, reason: &'static str) -> Self {
        self.exclusion = Some(reason);
        self
    }

    #[cfg(feature = "schema")]
    /// 显式绑定一个 `schemars::JsonSchema` 类型；不从 operation id 推断类型。
    pub const fn with_schema_type<T: schemars::JsonSchema>(mut self) -> Self {
        self.schema_generator = Some(generate_schema_for::<T>);
        self
    }
}

/// 一个 operation parent 的唯一声明。
///
/// API/SSE parent 必须提供 `method` 与 raw URI `path`；CLI、JSONL、metadata 和 config
/// 等 non-HTTP surface 可将两者设为 `None`，并用 `key` 指定其 canonical surface key。
#[derive(Debug, Clone, Copy)]
pub struct OperationDeclaration {
    pub operation_id: &'static str,
    pub surface: ContractSurface,
    pub method: Option<HttpMethod>,
    pub path: Option<&'static str>,
    pub operation: &'static str,
    pub key: &'static str,
    pub exclusion: Option<&'static str>,
    pub shared_components: &'static [&'static str],
    pub header_profile: Option<crate::headers::ApiHeaderProfile>,
    pub mcp_policy: Option<McpPolicy>,
    pub obligation_overrides: &'static [(EndpointObligationKind, EndpointObligation)],
    pub contracts: &'static [ContractDeclaration],
}

impl OperationDeclaration {
    /// 创建一个 parent declaration。
    pub const fn new(
        operation_id: &'static str,
        surface: ContractSurface,
        method: Option<HttpMethod>,
        path: Option<&'static str>,
        operation: &'static str,
        key: &'static str,
        contracts: &'static [ContractDeclaration],
    ) -> Self {
        Self {
            operation_id,
            surface,
            method,
            path,
            operation,
            key,
            exclusion: None,
            shared_components: &[],
            header_profile: None,
            mcp_policy: None,
            obligation_overrides: &[],
            contracts,
        }
    }

    pub const fn with_exclusion(mut self, reason: &'static str) -> Self {
        self.exclusion = Some(reason);
        self
    }

    pub const fn with_shared_components(
        mut self,
        shared_components: &'static [&'static str],
    ) -> Self {
        self.shared_components = shared_components;
        self
    }

    pub const fn with_header_profile(mut self, profile: crate::headers::ApiHeaderProfile) -> Self {
        self.header_profile = Some(profile);
        self
    }

    pub const fn with_mcp_policy(mut self, policy: McpPolicy) -> Self {
        self.mcp_policy = Some(policy);
        self
    }

    pub const fn with_obligation_overrides(
        mut self,
        overrides: &'static [(EndpointObligationKind, EndpointObligation)],
    ) -> Self {
        self.obligation_overrides = overrides;
        self
    }

    /// 返回该 parent 的 child contract source，便于按领域拆分声明文件。
    pub const fn contract_catalog(&self) -> &'static [ContractDeclaration] {
        self.contracts
    }

    /// 将 API/SSE parent 投影为现有 endpoint shape。
    pub fn endpoint_descriptor(&self) -> EndpointDescriptor {
        let method = self
            .method
            .expect("API/SSE operation declaration must provide an HTTP method");
        let path = self
            .path
            .expect("API/SSE operation declaration must provide an HTTP path");
        assert!(
            matches!(self.surface, ContractSurface::Api | ContractSurface::Sse),
            "only API/SSE operation declarations can project to EndpointDescriptor"
        );

        EndpointDescriptor {
            operation_id: self.operation_id,
            surface: self.surface,
            method,
            path,
            exclusion: self.exclusion,
            shared_components: self.shared_components,
            obligations: EndpointObligations {
                path: self.obligation(EndpointObligationKind::Path),
                query: self.obligation(EndpointObligationKind::Query),
                headers: self.obligation(EndpointObligationKind::Headers),
                body: self.obligation(EndpointObligationKind::Body),
                success: self.obligation(EndpointObligationKind::Success),
                sse: self.obligation(EndpointObligationKind::Sse),
            },
        }
    }

    /// 将 child declarations 投影为现有 operation inventory rows。
    pub fn operation_contracts(&self) -> Vec<OperationContract> {
        self.contracts
            .iter()
            .map(|contract| contract.operation_contract(self))
            .collect()
    }

    /// 将 parent/child declarations 投影为现有 surface operation row。
    pub fn surface_operation(&self) -> SurfaceOperation {
        SurfaceOperation {
            key: self.key.to_owned(),
            surface: self.surface,
            contracts: self
                .contracts
                .iter()
                .filter(|contract| contract.binding == ContractBinding::ExactSurface)
                .map(|contract| contract.id)
                .chain(self.shared_components.iter().copied())
                .collect(),
            exclusion: self.exclusion,
        }
    }

    fn obligation(&self, kind: EndpointObligationKind) -> EndpointObligation {
        if let Some((_, obligation)) = self
            .obligation_overrides
            .iter()
            .find(|(override_kind, _)| *override_kind == kind)
        {
            return *obligation;
        }
        self.contracts
            .iter()
            .find(|contract| {
                contract.binding == ContractBinding::ExactSurface
                    && contract.location == Some(kind.location())
            })
            .map_or(EndpointObligation::NotApplicable, |contract| {
                EndpointObligation::Contract(contract.id)
            })
    }
}

impl ContractDeclaration {
    /// 将 child declaration 投影为现有 operation inventory row。
    pub fn operation_contract(&self, parent: &OperationDeclaration) -> OperationContract {
        let operation = self.operation.unwrap_or(parent.operation);
        let transport = match (parent.method, parent.path, self.location) {
            (Some(_), Some(_), Some(location)) => ContractTransport::Http {
                operation_key: if self.binding == ContractBinding::SharedComponent {
                    None
                } else {
                    Some(self.operation_key.unwrap_or(operation))
                },
                location,
                parameters: self.parameters,
            },
            _ => ContractTransport::NoTransport,
        };
        OperationContract {
            id: self.id,
            path: self.path,
            surface: parent.surface,
            operation,
            direction: self.direction,
            granularity: self.granularity,
            strictness: self.strictness,
            schema_id: self.schema_id,
            fixture: self.valid_fixture,
            exclusion: self.exclusion.or(parent.exclusion),
            transport,
            binding: self.binding,
        }
    }

    /// 在 `schema` feature 下将 child declaration 投影为 public `SchemaRoot`。
    #[cfg(feature = "schema")]
    pub fn schema_root(&self) -> Option<crate::schema::SchemaRoot> {
        let schema_id = self.schema_id?;
        let artifact_path = self
            .artifact_path
            .expect("schema declaration must provide artifact_path");
        let title = self
            .schema_title
            .expect("schema declaration must provide schema_title");
        let valid_fixture = self
            .valid_fixture
            .expect("schema declaration must provide valid_fixture");
        let invalid_fixture = self
            .invalid_fixture
            .expect("schema declaration must provide invalid_fixture");
        let generate = self
            .schema_generator
            .expect("schema declaration must provide an explicit schema generator");
        Some(crate::schema::SchemaRoot {
            id: schema_id,
            artifact_path,
            title,
            contract_id: self.id,
            direction: self.direction,
            strictness: self.strictness,
            valid_fixture,
            invalid_fixture,
            generate,
        })
    }
}

#[cfg(feature = "schema")]
/// 使用显式 `schemars::JsonSchema` 类型生成 schema document。
pub fn generate_schema_for<T: schemars::JsonSchema>(
    direction: crate::ContractDirection,
) -> serde_json::Value {
    use schemars::generate::SchemaSettings;

    let settings = match direction {
        crate::ContractDirection::Serialize => SchemaSettings::draft2020_12().for_serialize(),
        crate::ContractDirection::Deserialize => SchemaSettings::draft2020_12().for_deserialize(),
        crate::ContractDirection::Bidirectional => {
            panic!("bidirectional operation must register separate input and output roots")
        }
    };
    let schema = settings.into_generator().into_root_schema_for::<T>();
    serde_json::to_value(schema).expect("schemars root schema must serialize")
}
