//! 标签本体 operation 的 host adapter。
//!
//! HTTP、CLI 与 MCP 都只传递 JSON command；这里是唯一把 command 解析为
//! Turso store input 的边界，避免入口各自复制事务或 board 解析逻辑。

use crate::LabelOntologyOperations;
use crate::{
    LabelProposalDecisionInput, LabelProposalInput, LabelSuggestionOptions, OntologyActionInput,
    OntologyApplyAtomInput, OntologyObservationInput, OntologyRevertInput, OntologyValidateInput,
    TursoStore, UpsertLabelSemanticsInput,
};
use kanban_core::{KanbanError, Result};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::adapter::{TursoApplicationStore, store_error};

fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|error| {
        KanbanError::InvalidInput(format!("invalid label ontology input: {error}"))
    })
}

fn text(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| KanbanError::InvalidInput(format!("{field} is required")))
}

fn optional_text(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}

fn options(value: Value) -> Result<LabelSuggestionOptions> {
    let object = value.as_object().cloned().unwrap_or_default();
    let options = LabelSuggestionOptions {
        output_limit: object
            .get("output_limit")
            .or_else(|| object.get("limit"))
            .and_then(Value::as_u64)
            .unwrap_or(5) as usize,
        candidate_limit: object
            .get("candidate_limit")
            .and_then(Value::as_u64)
            .unwrap_or(32) as usize,
        atom_limit: object
            .get("atom_limit")
            .and_then(Value::as_u64)
            .unwrap_or(80) as usize,
        max_selected_labels: object
            .get("max_selected_labels")
            .and_then(Value::as_u64)
            .unwrap_or(4) as usize,
        min_score: object
            .get("min_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.15) as f32,
    };
    Ok(options)
}

fn actor_aliases(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut()
        && let Some(actor) = object.get_mut("actor").and_then(Value::as_object_mut)
        && actor.get("actor_type").is_none()
        && let Some(actor_type) = actor.remove("type")
    {
        actor.insert("actor_type".to_owned(), actor_type);
    }
    value
}

impl LabelOntologyOperations for TursoApplicationStore {
    async fn label_ontology(&self, operation: &str, board: &str, input: Value) -> Result<Value> {
        let value = actor_aliases(input);
        let result = match operation {
            "list_semantics" => json!(
                self.store
                    .list_label_semantics(board)
                    .await
                    .map_err(store_error)?
            ),
            "get_semantics" => {
                let label_ref = text(&value, "label_ref")?;
                json!(
                    self.store
                        .get_label_semantics(board, &label_ref)
                        .await
                        .map_err(store_error)?
                )
            }
            "upsert_semantics" => {
                let label_ref = text(&value, "label_ref")?;
                let command: UpsertLabelSemanticsInput = decode(value)?;
                json!(
                    self.store
                        .upsert_label_semantics(board, &label_ref, command)
                        .await
                        .map_err(store_error)?
                )
            }
            "delete_semantics" => {
                let label_ref = text(&value, "label_ref")?;
                let expected_hash = text(&value, "expected_semantics_hash")?;
                let reason = text(&value, "reason")?;
                let actor = optional_text(&value, "actor").unwrap_or_else(|| "user".to_owned());
                json!(
                    self.store
                        .delete_label_semantics(board, &label_ref, &expected_hash, &reason, &actor)
                        .await
                        .map_err(store_error)?
                )
            }
            "list_atoms" => json!(
                self.store
                    .list_label_atoms(board)
                    .await
                    .map_err(store_error)?
            ),
            "explain_atom" => {
                let atom_ref = text(&value, "atom_ref")?;
                json!(
                    self.store
                        .explain_label_atom(board, &atom_ref)
                        .await
                        .map_err(store_error)?
                )
            }
            "index_status" => json!(
                self.store
                    .label_atom_index_status(board)
                    .await
                    .map_err(store_error)?
            ),
            "rebuild_atom_index" => json!(
                self.store
                    .rebuild_label_atom_index(board)
                    .await
                    .map_err(store_error)?
            ),
            "query_atom_index" => {
                let query = optional_text(&value, "query");
                let polarity = optional_text(&value, "polarity");
                let limit = value.get("limit").and_then(Value::as_u64).unwrap_or(24) as usize;
                self.store
                    .query_label_atom_index(board, query.as_deref(), polarity.as_deref(), limit)
                    .await
                    .map_err(store_error)?
            }
            "suggest_labels" => {
                let task_ref = text(&value, "task_ref")?;
                let command = options(value.get("options").cloned().unwrap_or_else(|| json!({})))?;
                json!(
                    self.store
                        .suggest_task_labels(board, &task_ref, command)
                        .await
                        .map_err(store_error)?
                )
            }
            "propose_label" => {
                let task_ref = text(&value, "task_ref")?;
                let command: LabelProposalInput = decode(value)?;
                json!(
                    self.store
                        .propose_task_label(board, &task_ref, command)
                        .await
                        .map_err(store_error)?
                )
            }
            "list_proposals" => {
                let task_ref = optional_text(&value, "task_ref");
                let status = optional_text(&value, "status");
                json!(
                    self.store
                        .list_label_proposals(board, task_ref.as_deref(), status.as_deref())
                        .await
                        .map_err(store_error)?
                )
            }
            "get_proposal" => {
                let proposal_id = text(&value, "proposal_id")?;
                json!(
                    self.store
                        .get_label_proposal(&proposal_id)
                        .await
                        .map_err(store_error)?
                )
            }
            "decide_proposal" => {
                let command: LabelProposalDecisionInput = decode(value)?;
                json!(
                    self.store
                        .decide_label_proposal(command)
                        .await
                        .map_err(store_error)?
                )
            }
            "record_observation" => {
                let command: OntologyObservationInput = decode(value)?;
                json!(
                    self.store
                        .record_label_ontology_observation(board, command)
                        .await
                        .map_err(store_error)?
                )
            }
            "list_signals" => {
                let statuses = value
                    .get("statuses")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                let kinds = value
                    .get("kinds")
                    .and_then(Value::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                let task_ref = optional_text(&value, "task_ref");
                let target = optional_text(&value, "target_label_ref");
                let proposed = optional_text(&value, "proposed_label_name");
                let include_all = value
                    .get("include_all")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let limit = value.get("limit").and_then(Value::as_u64).unwrap_or(100) as usize;
                json!(
                    self.store
                        .list_label_ontology_signals(
                            board,
                            &statuses,
                            &kinds,
                            (task_ref.as_deref(), target.as_deref(), proposed.as_deref()),
                            include_all,
                            limit
                        )
                        .await
                        .map_err(store_error)?
                )
            }
            "get_signal" => {
                let signal_id = text(&value, "signal_id")?;
                json!(
                    self.store
                        .get_label_ontology_signal(&signal_id)
                        .await
                        .map_err(store_error)?
                )
            }
            "review_signals" => {
                let group_by =
                    optional_text(&value, "group_by").unwrap_or_else(|| "target_label".to_owned());
                let include_all = value
                    .get("include_all")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let limit = value.get("limit").and_then(Value::as_u64).unwrap_or(100) as usize;
                json!(
                    self.store
                        .review_label_ontology(board, &group_by, include_all, limit)
                        .await
                        .map_err(store_error)?
                )
            }
            "create_action" => {
                let command: OntologyActionInput = decode(value)?;
                json!(
                    self.store
                        .create_label_ontology_action(board, command)
                        .await
                        .map_err(store_error)?
                )
            }
            "apply_atom" => {
                let command: OntologyApplyAtomInput = decode(value)?;
                json!(
                    self.store
                        .apply_label_ontology_atom(board, command)
                        .await
                        .map_err(store_error)?
                )
            }
            "revert_mutation" => {
                let command: OntologyRevertInput = decode(value)?;
                json!(
                    self.store
                        .revert_label_ontology_mutation(board, command)
                        .await
                        .map_err(store_error)?
                )
            }
            "validate_action" => {
                let command: OntologyValidateInput = decode(value)?;
                json!(
                    self.store
                        .validate_label_ontology_action(board, command)
                        .await
                        .map_err(store_error)?
                )
            }
            "quality" => {
                let limit = value
                    .get("sample_limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(20) as usize;
                json!(
                    self.store
                        .label_ontology_quality(board, limit)
                        .await
                        .map_err(store_error)?
                )
            }
            _ => {
                return Err(KanbanError::InvalidInput(format!(
                    "unknown label ontology operation: {operation}"
                )));
            }
        };
        Ok(result)
    }
}

#[allow(dead_code)]
fn _store_type_is_reachable(_: &TursoStore) {}
