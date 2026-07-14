use serde::{Deserialize, Serialize, de};
use serde_json::{Map, Value};

/// Phase 4 的并行实现 lane。这里只冻结 wire contract 的所有权，不表达 SQLite 表语义。
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

/// Portable JSONL discriminator 的稳定 contract descriptor。
///
/// SQLite table、scope 和 import guard 仍由 `kanban-sqlite` 拥有；这里仅作为
/// surface operation、contract ID、URN、fixture 与 adoption witness locator 的单一来源。
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
    /// Decode the complete import root and bind its singleton `type` to this descriptor.
    /// Lane adapters subsequently validate the closed record-specific `data` DTO.
    pub fn decode_input_envelope(
        &self,
        value: Value,
    ) -> Result<Map<String, Value>, serde_json::Error> {
        let envelope = serde_json::from_value::<PortableJsonlInputEnvelope>(value)?;
        if envelope.discriminator != self.discriminator {
            return Err(de::Error::custom(format!(
                "expected singleton type={}, got {}",
                self.discriminator, envelope.discriminator
            )));
        }
        Ok(envelope.data)
    }
}

macro_rules! portable_descriptor {
    ($discriminator:literal, $lane:ident, $target:literal) => {
        PortableContractDescriptor {
            discriminator: $discriminator,
            operation_key: concat!("type=", $discriminator),
            lane: PortableContractLane::$lane,
            input: PortableContractSide {
                contract_id: concat!("jsonl.", $discriminator, ".input"),
                schema_id: concat!("urn:kanban-tool:schema:jsonl:", $discriminator, "-input:v1"),
                fixture: concat!(
                    "schemas/fixtures/jsonl/",
                    $discriminator,
                    "-input.v1.valid.json"
                ),
                invalid_fixture: concat!(
                    "schemas/fixtures/jsonl/",
                    $discriminator,
                    "-input.v1.invalid.json"
                ),
                test_target: $target,
                producer_test: concat!($discriminator, "_input_fixture_is_produced_by_contract"),
                consumer_test: concat!($discriminator, "_input_fixture_is_consumed_by_real_import"),
            },
            output: PortableContractSide {
                contract_id: concat!("jsonl.", $discriminator, ".output"),
                schema_id: concat!(
                    "urn:kanban-tool:schema:jsonl:",
                    $discriminator,
                    "-output:v1"
                ),
                fixture: concat!(
                    "schemas/fixtures/jsonl/",
                    $discriminator,
                    "-output.v1.valid.json"
                ),
                invalid_fixture: concat!(
                    "schemas/fixtures/jsonl/",
                    $discriminator,
                    "-output.v1.invalid.json"
                ),
                test_target: $target,
                producer_test: concat!(
                    $discriminator,
                    "_output_fixture_is_produced_by_real_export"
                ),
                consumer_test: concat!($discriminator, "_output_fixture_is_consumed_by_contract"),
            },
        }
    };
}

const PORTABLE_CONTRACTS: &[PortableContractDescriptor] = &[
    portable_descriptor!("board", Core, "portable_core_contract_adoption"),
    portable_descriptor!("column", Core, "portable_core_contract_adoption"),
    portable_descriptor!("task", Core, "portable_core_contract_adoption"),
    portable_descriptor!("dependency", Core, "portable_core_contract_adoption"),
    portable_descriptor!("run", Core, "portable_core_contract_adoption"),
    portable_descriptor!("comment", Core, "portable_core_contract_adoption"),
    portable_descriptor!(
        "signal_observation",
        Ledger,
        "portable_ledger_contract_adoption"
    ),
    portable_descriptor!("signal", Ledger, "portable_ledger_contract_adoption"),
    portable_descriptor!("event", Core, "portable_core_contract_adoption"),
    portable_descriptor!("attachment", Core, "portable_core_contract_adoption"),
    portable_descriptor!("label", Ledger, "portable_ledger_contract_adoption"),
    portable_descriptor!(
        "label_semantics",
        Ledger,
        "portable_ledger_contract_adoption"
    ),
    portable_descriptor!("label_atom", Ledger, "portable_ledger_contract_adoption"),
    portable_descriptor!(
        "label_semantic_proposal",
        Ledger,
        "portable_ledger_contract_adoption"
    ),
    portable_descriptor!(
        "label_ontology_observation",
        Ledger,
        "portable_ledger_contract_adoption"
    ),
    portable_descriptor!(
        "label_ontology_signal",
        Ledger,
        "portable_ledger_contract_adoption"
    ),
    portable_descriptor!(
        "label_ontology_action",
        Ledger,
        "portable_ledger_contract_adoption"
    ),
    portable_descriptor!(
        "label_ontology_action_atom_effect",
        Ledger,
        "portable_ledger_contract_adoption"
    ),
    portable_descriptor!(
        "label_ontology_action_signal",
        Ledger,
        "portable_ledger_contract_adoption"
    ),
    portable_descriptor!("task_label", Core, "portable_core_contract_adoption"),
    portable_descriptor!("setting", Ledger, "portable_ledger_contract_adoption"),
];

pub fn portable_contract_catalog() -> &'static [PortableContractDescriptor] {
    PORTABLE_CONTRACTS
}
