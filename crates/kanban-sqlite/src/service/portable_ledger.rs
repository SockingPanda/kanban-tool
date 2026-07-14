//! Ledger/settings portable JSONL row adapters.

use kanban_core::{KanbanError, Result};
use serde_json::{Map, Value};

pub(crate) fn encode_record(
    discriminator: &str,
    mut data: Map<String, Value>,
) -> Result<Map<String, Value>> {
    // The central catalog uses an empty map solely as a lane-ownership probe. Real exported rows
    // are non-empty and are validated below.
    if data.is_empty() {
        json_fields(discriminator)?;
        return Ok(data);
    }
    for &(internal, external) in json_fields(discriminator)? {
        let raw = data.remove(internal).ok_or_else(|| {
            KanbanError::Storage(format!(
                "{discriminator} export row is missing JSON column {internal}"
            ))
        })?;
        let raw = raw.as_str().ok_or_else(|| {
            KanbanError::Storage(format!(
                "{discriminator}.{internal} must be SQLite JSON text"
            ))
        })?;
        let natural = serde_json::from_str(raw).map_err(|error| {
            KanbanError::Storage(format!(
                "{discriminator}.{internal} contains invalid canonical JSON: {error}"
            ))
        })?;
        data.insert(external.to_owned(), natural);
    }
    for field in integer_boolean_fields(discriminator)? {
        let value = data.get_mut(*field).ok_or_else(|| {
            KanbanError::Storage(format!(
                "{discriminator} export row is missing boolean column {field}"
            ))
        })?;
        *value = match value.as_i64() {
            Some(0) => Value::Bool(false),
            Some(1) => Value::Bool(true),
            _ => {
                return Err(KanbanError::Storage(format!(
                    "{discriminator}.{field} must be canonical SQLite boolean 0 or 1"
                )));
            }
        };
    }
    kanban_contract::jsonl_ledger::validate_output_data(discriminator, data).map_err(|error| {
        KanbanError::Storage(format!(
            "{discriminator} export row violates portable contract: {error}"
        ))
    })
}

pub(crate) fn decode_record(
    discriminator: &str,
    data: Map<String, Value>,
) -> Result<Map<String, Value>> {
    // `insert_jsonl_record` independently rejects empty data after this catalog ownership probe.
    if data.is_empty() {
        json_fields(discriminator)?;
        return Ok(data);
    }
    let mut data = kanban_contract::jsonl_ledger::validate_input_data(discriminator, data)
        .map_err(|error| {
            KanbanError::InvalidInput(format!(
                "{discriminator} import row violates portable contract: {error}"
            ))
        })?;
    for &(internal, external) in json_fields(discriminator)? {
        let natural = data
            .remove(external)
            .expect("validated JSON field is required");
        let encoded = serde_json::to_string(&natural).map_err(|error| {
            KanbanError::InvalidInput(format!(
                "{discriminator}.{external} cannot be encoded for SQLite: {error}"
            ))
        })?;
        data.insert(internal.to_owned(), Value::String(encoded));
    }
    for field in integer_boolean_fields(discriminator)? {
        let value = data
            .get_mut(*field)
            .expect("validated boolean field is required");
        *value = Value::Number(i64::from(value.as_bool().expect("validated boolean")).into());
    }
    Ok(data)
}

fn integer_boolean_fields(discriminator: &str) -> Result<&'static [&'static str]> {
    match discriminator {
        "label_ontology_observation" => Ok(&["suggest_needs_new_label", "suggest_degraded"]),
        "label_ontology_signal" => Ok(&["agent_selected", "final_selected"]),
        "label"
        | "label_semantics"
        | "label_atom"
        | "label_semantic_proposal"
        | "label_ontology_action"
        | "label_ontology_action_atom_effect"
        | "label_ontology_action_signal"
        | "signal_observation"
        | "signal"
        | "setting" => Ok(&[]),
        other => Err(KanbanError::Storage(format!(
            "ledger portable adapter does not own discriminator: {other}"
        ))),
    }
}

fn json_fields(discriminator: &str) -> Result<&'static [(&'static str, &'static str)]> {
    let fields: &[(&str, &str)] = match discriminator {
        "label"
        | "label_atom"
        | "label_ontology_action_atom_effect"
        | "label_ontology_action_signal"
        | "signal" => &[],
        "label_semantics" => &[
            ("applies_when", "applies_when"),
            ("excludes_when", "excludes_when"),
            ("positive_examples", "positive_examples"),
            ("negative_examples", "negative_examples"),
        ],
        "label_semantic_proposal" => &[
            ("applies_when", "applies_when"),
            ("excludes_when", "excludes_when"),
            ("positive_examples", "positive_examples"),
            ("negative_examples", "negative_examples"),
            ("diagnostics_json", "diagnostics"),
        ],
        "label_ontology_observation" => &[
            ("task_snapshot_json", "task_snapshot"),
            ("agent_candidates_json", "agent_candidates"),
            ("suggestion_snapshot_json", "suggestion_snapshot"),
            ("final_decision_json", "final_decision"),
            ("diagnostics_json", "diagnostics"),
        ],
        "label_ontology_signal" => &[
            ("related_labels_json", "related_labels"),
            ("proposal_json", "proposal"),
        ],
        "label_ontology_action" => &[("change_json", "change"), ("validation_json", "validation")],
        "signal_observation" => &[("evidence_json", "evidence")],
        "setting" => &[("value_json", "value")],
        other => {
            return Err(KanbanError::Storage(format!(
                "ledger portable adapter does not own discriminator: {other}"
            )));
        }
    };
    Ok(fields)
}
