use serde::{Deserialize, Serialize, de};
use serde_json::{Map, Value};

use crate::{
    AdoptionLocator, ContractBinding, ContractDeclaration, ContractDirection, ContractGranularity,
    ContractStrictness, ContractSurface, MigrationState, OperationContract, OperationDeclaration,
};

/// 第 4 阶段的并行实现 lane。这里只冻结 wire contract 的所有权，不表达 SQLite 表语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortableContractLane {
    Core,
    Ledger,
}

/// 单个 JSONL direction 的稳定 authority locator。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PortableContractSide {
    pub contract_id: &'static str,
    pub schema_id: &'static str,
    pub fixture: &'static str,
    pub invalid_fixture: &'static str,
    pub test_target: &'static str,
    pub producer_test: &'static str,
    pub consumer_test: &'static str,
}

/// portable JSONL discriminator 的稳定 contract 描述符。
///
/// 这是 `OperationDeclaration`/`ContractDeclaration` source 的 compatibility projection。
/// SQLite 表、scope 和 import guard 仍由 canonical service 拥有；这里仅提供旧 public
/// API 所需的 surface operation、contract ID、URN、fixture 与 adoption witness locator。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PortableContractDescriptor {
    pub discriminator: &'static str,
    pub operation_key: &'static str,
    pub lane: PortableContractLane,
    pub input: PortableContractSide,
    pub output: PortableContractSide,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableJsonlInputEnvelope {
    #[serde(rename = "type")]
    discriminator: String,
    data: Map<String, Value>,
}

impl PortableContractDescriptor {
    /// 解码完整的导入根对象，并将其中唯一的 `type` 绑定到此描述符。
    /// 随后的 lane adapter 会校验封闭的 record-specific `data` DTO。
    pub fn decode_input_envelope(
        &self,
        value: Value,
    ) -> Result<Map<String, Value>, serde_json::Error> {
        let envelope = serde_json::from_value::<PortableJsonlInputEnvelope>(value)?;
        if envelope.discriminator != self.discriminator {
            return Err(de::Error::custom(format!(
                "期望单一 type={}，实际得到 {}",
                self.discriminator, envelope.discriminator
            )));
        }
        Ok(envelope.data)
    }
}

macro_rules! portable_contract {
    (
        $discriminator:literal,
        $direction:expr,
        $suffix:literal,
        $schema_type:ty
    ) => {{
        let contract = ContractDeclaration::new(
            concat!("jsonl.", $discriminator, ".", $suffix),
            concat!("type=", $discriminator),
            $direction,
            None,
            ContractStrictness::DenyUnknownFields,
            ContractGranularity::Exact,
            ContractBinding::ExactSurface,
        )
        .with_schema(
            concat!(
                "urn:kanban-tool:schema:jsonl:",
                $discriminator,
                "-",
                $suffix,
                ":v1"
            ),
            concat!("jsonl/", $discriminator, "-", $suffix, ".v1.schema.json"),
            concat!("Kanban JSONL ", $discriminator, " ", $suffix, " v1"),
            concat!(
                "schemas/fixtures/jsonl/",
                $discriminator,
                "-",
                $suffix,
                ".v1.valid.json"
            ),
            concat!(
                "schemas/fixtures/jsonl/",
                $discriminator,
                "-",
                $suffix,
                ".v1.invalid.json"
            ),
        )
        .with_adoption(
            AdoptionLocator {
                package: "kanban-server",
                test_target: "lib",
                exact_test: concat!(
                    "suite::portable_adoption::",
                    $discriminator,
                    "_output_fixture_is_produced_by_real_export"
                ),
            },
            AdoptionLocator {
                package: "kanban-server",
                test_target: "lib",
                exact_test: concat!(
                    "suite::portable_adoption::",
                    $discriminator,
                    "_input_fixture_is_consumed_by_real_import"
                ),
            },
        );
        #[cfg(feature = "schema")]
        let contract = contract.with_schema_type::<$schema_type>();
        contract
    }};
}

macro_rules! portable_operations {
    (
        $(
            $name:ident => {
                contracts: $contracts:ident,
                operation_id: $operation_id:literal,
                discriminator: $discriminator:literal,
                lane: $lane:ident,
                input: $input:ty,
                output: $output:ty
            }
        ),+ $(,)?
    ) => {
        $(
            const $contracts: &[ContractDeclaration] = &[
                portable_contract!(
                    $discriminator,
                    ContractDirection::Deserialize,
                    "input",
                    $input
                ),
                portable_contract!(
                    $discriminator,
                    ContractDirection::Serialize,
                    "output",
                    $output
                ),
            ];

            const $name: OperationDeclaration = OperationDeclaration::new(
                $operation_id,
                ContractSurface::Jsonl,
                None,
                None,
                concat!("type=", $discriminator),
                concat!("type=", $discriminator),
                MigrationState::Adopted,
                $contracts,
            );
        )+

        /// portable JSONL parent declaration 的依赖安全导出/导入顺序。
        const PORTABLE_OPERATIONS: &[OperationDeclaration] = &[$($name),+];

        fn portable_lane(operation_id: &str) -> PortableContractLane {
            match operation_id {
                $($operation_id => PortableContractLane::$lane,)+
                _ => panic!("unknown portable operation declaration: {operation_id}"),
            }
        }
    };
}

portable_operations! {
    BOARD => {
        contracts: BOARD_CONTRACTS,
        operation_id: "jsonl.board",
        discriminator: "board",
        lane: Core,
        input: crate::jsonl_core::BoardJsonlInput,
        output: crate::jsonl_core::BoardJsonlOutput
    },
    COLUMN => {
        contracts: COLUMN_CONTRACTS,
        operation_id: "jsonl.column",
        discriminator: "column",
        lane: Core,
        input: crate::jsonl_core::ColumnJsonlInput,
        output: crate::jsonl_core::ColumnJsonlOutput
    },
    TASK => {
        contracts: TASK_CONTRACTS,
        operation_id: "jsonl.task",
        discriminator: "task",
        lane: Core,
        input: crate::jsonl_core::TaskJsonlInput,
        output: crate::jsonl_core::TaskJsonlOutput
    },
    DEPENDENCY => {
        contracts: DEPENDENCY_CONTRACTS,
        operation_id: "jsonl.dependency",
        discriminator: "dependency",
        lane: Core,
        input: crate::jsonl_core::DependencyJsonlInput,
        output: crate::jsonl_core::DependencyJsonlOutput
    },
    RUN => {
        contracts: RUN_CONTRACTS,
        operation_id: "jsonl.run",
        discriminator: "run",
        lane: Core,
        input: crate::jsonl_core::RunJsonlInput,
        output: crate::jsonl_core::RunJsonlOutput
    },
    COMMENT => {
        contracts: COMMENT_CONTRACTS,
        operation_id: "jsonl.comment",
        discriminator: "comment",
        lane: Core,
        input: crate::jsonl_core::CommentJsonlInput,
        output: crate::jsonl_core::CommentJsonlOutput
    },
    SIGNAL_OBSERVATION => {
        contracts: SIGNAL_OBSERVATION_CONTRACTS,
        operation_id: "jsonl.signal_observation",
        discriminator: "signal_observation",
        lane: Ledger,
        input: crate::jsonl_ledger::SignalObservationInput,
        output: crate::jsonl_ledger::SignalObservationOutput
    },
    SIGNAL => {
        contracts: SIGNAL_CONTRACTS,
        operation_id: "jsonl.signal",
        discriminator: "signal",
        lane: Ledger,
        input: crate::jsonl_ledger::SignalInput,
        output: crate::jsonl_ledger::SignalOutput
    },
    EVENT => {
        contracts: EVENT_CONTRACTS,
        operation_id: "jsonl.event",
        discriminator: "event",
        lane: Core,
        input: crate::jsonl_core::EventJsonlInput,
        output: crate::jsonl_core::EventJsonlOutput
    },
    ATTACHMENT => {
        contracts: ATTACHMENT_CONTRACTS,
        operation_id: "jsonl.attachment",
        discriminator: "attachment",
        lane: Core,
        input: crate::jsonl_core::AttachmentJsonlInput,
        output: crate::jsonl_core::AttachmentJsonlOutput
    },
    LABEL => {
        contracts: LABEL_CONTRACTS,
        operation_id: "jsonl.label",
        discriminator: "label",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelInput,
        output: crate::jsonl_ledger::LabelOutput
    },
    LABEL_SEMANTICS => {
        contracts: LABEL_SEMANTICS_CONTRACTS,
        operation_id: "jsonl.label_semantics",
        discriminator: "label_semantics",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelSemanticsInput,
        output: crate::jsonl_ledger::LabelSemanticsOutput
    },
    LABEL_ATOM => {
        contracts: LABEL_ATOM_CONTRACTS,
        operation_id: "jsonl.label_atom",
        discriminator: "label_atom",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelAtomInput,
        output: crate::jsonl_ledger::LabelAtomOutput
    },
    LABEL_SEMANTIC_PROPOSAL => {
        contracts: LABEL_SEMANTIC_PROPOSAL_CONTRACTS,
        operation_id: "jsonl.label_semantic_proposal",
        discriminator: "label_semantic_proposal",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelSemanticProposalInput,
        output: crate::jsonl_ledger::LabelSemanticProposalOutput
    },
    LABEL_ONTOLOGY_OBSERVATION => {
        contracts: LABEL_ONTOLOGY_OBSERVATION_CONTRACTS,
        operation_id: "jsonl.label_ontology_observation",
        discriminator: "label_ontology_observation",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelOntologyObservationInput,
        output: crate::jsonl_ledger::LabelOntologyObservationOutput
    },
    LABEL_ONTOLOGY_SIGNAL => {
        contracts: LABEL_ONTOLOGY_SIGNAL_CONTRACTS,
        operation_id: "jsonl.label_ontology_signal",
        discriminator: "label_ontology_signal",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelOntologySignalInput,
        output: crate::jsonl_ledger::LabelOntologySignalOutput
    },
    LABEL_ONTOLOGY_ACTION => {
        contracts: LABEL_ONTOLOGY_ACTION_CONTRACTS,
        operation_id: "jsonl.label_ontology_action",
        discriminator: "label_ontology_action",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelOntologyActionInput,
        output: crate::jsonl_ledger::LabelOntologyActionOutput
    },
    LABEL_ONTOLOGY_ACTION_ATOM_EFFECT => {
        contracts: LABEL_ONTOLOGY_ACTION_ATOM_EFFECT_CONTRACTS,
        operation_id: "jsonl.label_ontology_action_atom_effect",
        discriminator: "label_ontology_action_atom_effect",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelOntologyActionAtomEffectInput,
        output: crate::jsonl_ledger::LabelOntologyActionAtomEffectOutput
    },
    LABEL_ONTOLOGY_ACTION_SIGNAL => {
        contracts: LABEL_ONTOLOGY_ACTION_SIGNAL_CONTRACTS,
        operation_id: "jsonl.label_ontology_action_signal",
        discriminator: "label_ontology_action_signal",
        lane: Ledger,
        input: crate::jsonl_ledger::LabelOntologyActionSignalInput,
        output: crate::jsonl_ledger::LabelOntologyActionSignalOutput
    },
    TASK_LABEL => {
        contracts: TASK_LABEL_CONTRACTS,
        operation_id: "jsonl.task_label",
        discriminator: "task_label",
        lane: Core,
        input: crate::jsonl_core::TaskLabelJsonlInput,
        output: crate::jsonl_core::TaskLabelJsonlOutput
    },
    SETTING => {
        contracts: SETTING_CONTRACTS,
        operation_id: "jsonl.setting",
        discriminator: "setting",
        lane: Ledger,
        input: crate::jsonl_ledger::SettingInput,
        output: crate::jsonl_ledger::SettingOutput
    },
}

/// Portable JSONL 的唯一 parent declaration source。
pub const fn operation_declarations() -> &'static [OperationDeclaration] {
    PORTABLE_OPERATIONS
}

/// 返回 portable JSONL child contracts 的兼容 projection。
pub fn portable_operation_contracts() -> Vec<OperationContract> {
    crate::CatalogProjection::new(PORTABLE_OPERATIONS).contracts()
}

fn descriptor_side(contract: &ContractDeclaration) -> PortableContractSide {
    let producer = contract
        .producer
        .expect("portable contract declaration must provide producer witness");
    let consumer = contract
        .consumer
        .expect("portable contract declaration must provide consumer witness");
    PortableContractSide {
        contract_id: contract.id,
        schema_id: contract
            .schema_id
            .expect("portable contract declaration must provide schema id"),
        fixture: contract
            .valid_fixture
            .expect("portable contract declaration must provide valid fixture"),
        invalid_fixture: contract
            .invalid_fixture
            .expect("portable contract declaration must provide invalid fixture"),
        test_target: producer.test_target,
        producer_test: producer.exact_test,
        consumer_test: consumer.exact_test,
    }
}

fn portable_descriptor(operation: &OperationDeclaration) -> PortableContractDescriptor {
    let discriminator = operation
        .key
        .strip_prefix("type=")
        .expect("portable operation key must use type= discriminator");
    let input = operation
        .contracts
        .iter()
        .find(|contract| contract.direction == ContractDirection::Deserialize)
        .expect("portable operation must declare an input contract");
    let output = operation
        .contracts
        .iter()
        .find(|contract| contract.direction == ContractDirection::Serialize)
        .expect("portable operation must declare an output contract");
    PortableContractDescriptor {
        discriminator,
        operation_key: operation.key,
        lane: portable_lane(operation.operation_id),
        input: descriptor_side(input),
        output: descriptor_side(output),
    }
}

/// 旧的 portable descriptor catalog；底层数据来自 `operation_declarations()`。
pub fn portable_contract_catalog() -> &'static [PortableContractDescriptor] {
    static CATALOG: std::sync::OnceLock<Vec<PortableContractDescriptor>> =
        std::sync::OnceLock::new();
    CATALOG
        .get_or_init(|| {
            PORTABLE_OPERATIONS
                .iter()
                .map(portable_descriptor)
                .collect()
        })
        .as_slice()
}

/// 返回该 family 的 surface operation projection。
pub fn surface_catalog() -> Vec<crate::SurfaceOperation> {
    crate::CatalogProjection::new(PORTABLE_OPERATIONS).surfaces()
}

/// 返回该 family 的 schema root projection。
#[cfg(feature = "schema")]
pub fn schema_roots() -> Vec<crate::schema::SchemaRoot> {
    crate::CatalogProjection::new(PORTABLE_OPERATIONS).schemas()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_source_preserves_portable_operation_and_contract_counts() {
        assert_eq!(operation_declarations().len(), 21);
        assert_eq!(portable_operation_contracts().len(), 42);
        assert_eq!(portable_contract_catalog().len(), 21);
        assert_eq!(
            portable_operation_contracts(),
            crate::CatalogProjection::new(operation_declarations()).contracts()
        );
    }
}
