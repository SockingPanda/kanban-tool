use kanban_contract::{PortableContractDescriptor, portable_contract_catalog};
use kanban_core::{KanbanError, Result};
use serde_json::{Map, Value};

use super::{portable_core, portable_ledger};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportScope {
    SelectedBoard,
    BoardScoped,
    Global,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PortableRecordDescriptor {
    pub contract: &'static PortableContractDescriptor,
    pub table: &'static str,
    pub scope: ExportScope,
}

pub(crate) fn portable_record_catalog() -> Result<Vec<PortableRecordDescriptor>> {
    portable_contract_catalog()
        .iter()
        .map(portable_record_descriptor_for_contract)
        .collect()
}

pub(crate) fn portable_record_descriptor(discriminator: &str) -> Result<PortableRecordDescriptor> {
    let contract = portable_contract_catalog()
        .iter()
        .find(|descriptor| descriptor.discriminator == discriminator)
        .ok_or_else(|| {
            KanbanError::InvalidInput(format!("unsupported export record type: {discriminator}"))
        })?;
    portable_record_descriptor_for_contract(contract)
}

fn portable_record_descriptor_for_contract(
    contract: &'static PortableContractDescriptor,
) -> Result<PortableRecordDescriptor> {
    let (table, scope) = match contract.discriminator {
        "board" => ("boards", ExportScope::SelectedBoard),
        "column" => ("board_columns", ExportScope::BoardScoped),
        "task" => ("tasks", ExportScope::BoardScoped),
        "dependency" => ("task_dependencies", ExportScope::BoardScoped),
        "run" => ("task_runs", ExportScope::BoardScoped),
        "comment" => ("task_comments", ExportScope::BoardScoped),
        "signal_observation" => ("signal_observations", ExportScope::BoardScoped),
        "signal" => ("signals", ExportScope::BoardScoped),
        "event" => ("task_events", ExportScope::BoardScoped),
        "attachment" => ("task_attachments", ExportScope::BoardScoped),
        "label" => ("labels", ExportScope::BoardScoped),
        "label_semantics" => ("label_semantics", ExportScope::BoardScoped),
        "label_atom" => ("label_atoms", ExportScope::BoardScoped),
        "label_semantic_proposal" => ("label_semantic_proposals", ExportScope::BoardScoped),
        "label_ontology_observation" => ("label_ontology_observations", ExportScope::BoardScoped),
        "label_ontology_signal" => ("label_ontology_signals", ExportScope::BoardScoped),
        "label_ontology_action" => ("label_ontology_actions", ExportScope::BoardScoped),
        "label_ontology_action_atom_effect" => (
            "label_ontology_action_atom_effects",
            ExportScope::BoardScoped,
        ),
        "label_ontology_action_signal" => {
            ("label_ontology_action_signals", ExportScope::BoardScoped)
        }
        "task_label" => ("task_labels", ExportScope::BoardScoped),
        "setting" => ("app_settings", ExportScope::Global),
        discriminator => {
            return Err(KanbanError::Storage(format!(
                "portable contract catalog contains unmapped discriminator: {discriminator}"
            )));
        }
    };
    Ok(PortableRecordDescriptor {
        contract,
        table,
        scope,
    })
}

pub(crate) fn encode_portable_record(
    discriminator: &str,
    data: Map<String, Value>,
) -> Result<Map<String, Value>> {
    let descriptor = portable_record_descriptor(discriminator)?;
    match descriptor.contract.lane {
        kanban_contract::PortableContractLane::Core => {
            portable_core::encode_record(discriminator, data)
        }
        kanban_contract::PortableContractLane::Ledger => {
            portable_ledger::encode_record(discriminator, data)
        }
    }
}

pub(crate) fn encode_portable_output_envelope(
    discriminator: &str,
    data: Map<String, Value>,
) -> Result<Value> {
    let data = encode_portable_record(discriminator, data)?;
    serde_json::to_value(kanban_contract::jsonl_ledger::PortableRecord {
        record_type: discriminator,
        data,
    })
    .map_err(|error| KanbanError::Storage(error.to_string()))
}

pub(crate) fn decode_portable_record(
    discriminator: &str,
    data: Map<String, Value>,
) -> Result<Map<String, Value>> {
    let descriptor = portable_record_descriptor(discriminator)?;
    match descriptor.contract.lane {
        kanban_contract::PortableContractLane::Core => {
            portable_core::decode_record(discriminator, data)
        }
        kanban_contract::PortableContractLane::Ledger => {
            portable_ledger::decode_record(discriminator, data)
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, json};

    use super::{
        decode_portable_record, encode_portable_output_envelope, encode_portable_record,
        portable_record_catalog,
    };

    #[test]
    fn output_envelope_is_owned_by_the_contract_portable_record() {
        let data = json!({
            "id": "b_fixture",
            "slug": "fixture",
            "name": "Fixture",
            "description": null,
            "created_at": 1,
            "updated_at": 2,
            "archived_at": null
        })
        .as_object()
        .cloned()
        .expect("board data object");
        let envelope = encode_portable_output_envelope("board", data)
            .expect("encode typed portable output envelope");
        assert_eq!(
            envelope,
            json!({
                "type": "board",
                "data": {
                    "id": "b_fixture",
                    "slug": "fixture",
                    "name": "Fixture",
                    "description": null,
                    "created_at": 1,
                    "updated_at": 2,
                    "archived_at": null
                }
            })
        );
    }

    #[test]
    fn every_portable_descriptor_routes_through_its_lane_adapter() {
        for descriptor in portable_record_catalog().expect("portable adapter catalog") {
            encode_portable_record(descriptor.contract.discriminator, Map::new())
                .expect("export lane adapter must own every descriptor");
            decode_portable_record(descriptor.contract.discriminator, Map::new())
                .expect("import lane adapter must own every descriptor");
        }
    }
}
