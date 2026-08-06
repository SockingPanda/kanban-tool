//! Turso 中 label semantics、atom 与 ontology ledger 的原子操作。
//!
//! 该模块只负责 canonical SQL 与事务边界。向量 provider 不在这里，索引
//! rebuild/status 明确返回 degraded 状态，避免把 projection 成功误报为 canonical 成功。

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{Value as JsonValue, json};
use turso::{Connection, Row, transaction::TransactionBehavior};

use crate::{
    db::TursoStore,
    domain::{
        LabelAtomExplainActionRecord, LabelAtomExplainRecord, LabelAtomExplainSignalRecord,
        LabelAtomExplainValidationRecord, LabelAtomIndexStatusRecord, LabelAtomRecord,
        LabelOntologyActionRecord, LabelOntologyObservationRecord, LabelOntologyQualityRecord,
        LabelOntologyReviewGroupRecord, LabelOntologySignalDetailRecord, LabelOntologySignalRecord,
        LabelProposalAttemptRecord, LabelSemanticProposalRecord, LabelSemanticsRecord,
        LabelSuggestionCandidateRecord, LabelSuggestionEvidenceRecord, LabelSuggestionResultRecord,
    },
    error::StoreError,
    shared::{
        Value, first_row, integer_value, now_ms, optional_integer_value, optional_text_value,
        text_value,
    },
};

pub(crate) const LABEL_ATOM_INDEX_STORE: &str = "vector_label_atoms";
const MAX_LIST_LIMIT: i64 = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UpsertLabelSemanticsInput {
    pub expected_semantics_hash: Option<String>,
    #[serde(default)]
    pub replace: bool,
    pub description: Option<String>,
    pub applies_when: Option<Vec<String>>,
    pub excludes_when: Option<Vec<String>>,
    pub positive_examples: Option<Vec<String>>,
    pub negative_examples: Option<Vec<String>>,
    #[serde(default)]
    pub remove_applies_when: Vec<String>,
    #[serde(default)]
    pub remove_excludes_when: Vec<String>,
    #[serde(default)]
    pub remove_positive_examples: Vec<String>,
    #[serde(default)]
    pub remove_negative_examples: Vec<String>,
    pub actor: String,
    pub reason: Option<String>,
    #[serde(default)]
    pub source_signal_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LabelSuggestionOptions {
    pub output_limit: usize,
    pub candidate_limit: usize,
    pub atom_limit: usize,
    pub max_selected_labels: usize,
    pub min_score: f32,
}

impl Default for LabelSuggestionOptions {
    fn default() -> Self {
        Self {
            output_limit: 5,
            candidate_limit: 32,
            atom_limit: 80,
            max_selected_labels: 4,
            min_score: 0.15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LabelProposalInput {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub applies_when: Vec<String>,
    #[serde(default)]
    pub excludes_when: Vec<String>,
    #[serde(default)]
    pub positive_examples: Vec<String>,
    #[serde(default)]
    pub negative_examples: Vec<String>,
    pub actor: String,
    #[serde(default)]
    pub source_signal_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LabelProposalDecisionInput {
    pub proposal_id: String,
    pub accept: bool,
    pub reason: Option<String>,
    pub actor: String,
    #[serde(default)]
    pub source_signal_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OntologyActorInput {
    pub name: String,
    pub actor_type: String,
    pub agent_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OntologySignalInput {
    pub kind: String,
    pub target_label_ref: Option<String>,
    #[serde(default)]
    pub related_labels_json: String,
    pub proposed_action: String,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub proposed_label_name: Option<String>,
    #[serde(default)]
    pub proposal_json: String,
    #[serde(default)]
    pub agent_selected: bool,
    pub suggest_state: Option<String>,
    pub suggest_score: Option<f64>,
    pub suggest_rank: Option<i64>,
    #[serde(default)]
    pub final_selected: bool,
    pub rationale: String,
    pub confidence: Option<f64>,
    pub signal_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OntologyObservationInput {
    pub actor: OntologyActorInput,
    pub task_ref: String,
    #[serde(default)]
    pub agent_candidates_json: String,
    #[serde(default)]
    pub suggestion_snapshot_json: String,
    #[serde(default)]
    pub final_decision_json: String,
    pub suggest_coverage: Option<f64>,
    pub suggest_coverage_cosine: Option<f64>,
    pub suggest_residual_norm: Option<f64>,
    #[serde(default)]
    pub suggest_needs_new_label: bool,
    #[serde(default)]
    pub suggest_degraded: bool,
    #[serde(default)]
    pub diagnostics_json: String,
    pub capture_fingerprint: Option<String>,
    pub signals: Vec<OntologySignalInput>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OntologyActionInput {
    pub actor: OntologyActorInput,
    pub action_type: String,
    #[serde(default)]
    pub signal_ids: Vec<String>,
    pub reason: String,
    pub superseded_by_signal_id: Option<String>,
    pub parent_action_id: Option<String>,
    pub target_label_ref: Option<String>,
    pub result_label_ref: Option<String>,
    pub result_atom_id: Option<String>,
    pub result_atom_content_hash: Option<String>,
    pub result_proposal_id: Option<String>,
    pub canonical_before_hash: Option<String>,
    pub canonical_after_hash: Option<String>,
    #[serde(default)]
    pub change_json: String,
    pub validation_status: Option<String>,
    #[serde(default)]
    pub validation_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OntologyApplyAtomInput {
    pub actor: OntologyActorInput,
    #[serde(default)]
    pub signal_ids: Vec<String>,
    pub label_ref: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OntologyRevertInput {
    pub actor: OntologyActorInput,
    pub target_action_id: String,
    pub expected_current_hash: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OntologyValidateInput {
    pub actor: OntologyActorInput,
    pub parent_action_id: String,
    #[serde(default)]
    pub signal_ids: Vec<String>,
    pub reason: String,
    pub validation_status: String,
    #[serde(default)]
    pub validation_json: String,
}

impl TursoStore {
    async fn ontology_board_id(&self, board: &str) -> Result<String, StoreError> {
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT id FROM boards WHERE id=:board OR slug=:board LIMIT 1",
                    [(":board", board.trim())],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(board.to_owned()),
            other => StoreError::Turso(other),
        })?;
        text_value(row.get_value(0)?, "boards.id")
    }

    async fn ontology_label(
        &self,
        board_id: &str,
        label_ref: &str,
    ) -> Result<(String, String), StoreError> {
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT id,name FROM labels WHERE board_id=:board AND (id=:ref OR name=:ref) LIMIT 1",
                    [(":board", board_id), (":ref", label_ref.trim())],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::InvalidInput(format!("label does not exist on board: {label_ref}"))
            }
            other => StoreError::Turso(other),
        })?;
        Ok((
            text_value(row.get_value(0)?, "labels.id")?,
            text_value(row.get_value(1)?, "labels.name")?,
        ))
    }

    async fn ontology_task(
        &self,
        board_id: &str,
        task_ref: &str,
    ) -> Result<(String, String, String, Option<String>), StoreError> {
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT t.id,b.slug||'#'||t.seq,t.title,t.description FROM tasks t JOIN boards b ON b.id=t.board_id WHERE t.board_id=:board AND (t.id=:ref OR b.slug||'#'||t.seq=:ref) LIMIT 1",
                    [(":board", board_id), (":ref", task_ref.trim())],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::TaskNotFound(task_ref.to_owned())
            }
            other => StoreError::Turso(other),
        })?;
        Ok((
            text_value(row.get_value(0)?, "tasks.id")?,
            text_value(row.get_value(1)?, "tasks.ref")?,
            text_value(row.get_value(2)?, "tasks.title")?,
            optional_text_value(row.get_value(3)?, "tasks.description")?,
        ))
    }

    /// 按看板更新时间顺序读取全部规范 semantics。
    pub async fn list_label_semantics(
        &self,
        board: &str,
    ) -> Result<Vec<LabelSemanticsRecord>, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let connection = self.connection().await?;
        let mut rows = connection
            .query(
                "SELECT s.label_id,s.board_id,l.name,s.description,s.applies_when,s.excludes_when,s.positive_examples,s.negative_examples,s.created_at,s.updated_at FROM label_semantics s JOIN labels l ON l.id=s.label_id AND l.board_id=s.board_id WHERE s.board_id=:board ORDER BY s.updated_at DESC,s.label_id ASC",
                [(":board", board_id.as_str())],
            )
            .await?;
        let mut records = Vec::new();
        while let Some(row) = rows.next().await? {
            records.push(self.semantics_from_row(&connection, row).await?);
        }
        Ok(records)
    }

    pub async fn get_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
    ) -> Result<LabelSemanticsRecord, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let (label_id, _) = self.ontology_label(&board_id, label_ref).await?;
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT s.label_id,s.board_id,l.name,s.description,s.applies_when,s.excludes_when,s.positive_examples,s.negative_examples,s.created_at,s.updated_at FROM label_semantics s JOIN labels l ON l.id=s.label_id AND l.board_id=s.board_id WHERE s.board_id=:board AND s.label_id=:label LIMIT 1",
                    [(":board", board_id.as_str()), (":label", label_id.as_str())],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::InvalidInput(format!("label semantics not found: {label_ref}"))
            }
            other => StoreError::Turso(other),
        })?;
        self.semantics_from_row(&connection, row).await
    }

    async fn semantics_from_row(
        &self,
        connection: &Connection,
        row: Row,
    ) -> Result<LabelSemanticsRecord, StoreError> {
        let label_id = text_value(row.get_value(0)?, "label_semantics.label_id")?;
        let board_id = text_value(row.get_value(1)?, "label_semantics.board_id")?;
        let label_name = text_value(row.get_value(2)?, "labels.name")?;
        let description = optional_text_value(row.get_value(3)?, "label_semantics.description")?;
        let applies_when = json_strings(row.get_value(4)?, "label_semantics.applies_when")?;
        let excludes_when = json_strings(row.get_value(5)?, "label_semantics.excludes_when")?;
        let positive_examples =
            json_strings(row.get_value(6)?, "label_semantics.positive_examples")?;
        let negative_examples =
            json_strings(row.get_value(7)?, "label_semantics.negative_examples")?;
        let created_at = integer_value(row.get_value(8)?, "label_semantics.created_at")?;
        let updated_at = integer_value(row.get_value(9)?, "label_semantics.updated_at")?;
        let semantics_hash = semantics_hash(
            &label_id,
            &label_name,
            &description,
            &applies_when,
            &excludes_when,
            &positive_examples,
            &negative_examples,
        );
        let atoms = self
            .label_atoms_for_label(connection, &board_id, &label_id)
            .await?;
        Ok(LabelSemanticsRecord {
            label_id,
            board_id,
            label_name,
            semantics_hash,
            description,
            applies_when,
            excludes_when,
            positive_examples,
            negative_examples,
            created_at,
            updated_at,
            atoms,
        })
    }

    async fn label_atoms_for_label(
        &self,
        connection: &Connection,
        board_id: &str,
        label_id: &str,
    ) -> Result<Vec<LabelAtomRecord>, StoreError> {
        let mut rows = connection
            .query(
                "SELECT a.id,a.label_id,a.board_id,l.name,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id WHERE a.board_id=:board AND a.label_id=:label ORDER BY a.ordinal ASC,a.id ASC",
                [(":board", board_id), (":label", label_id)],
            )
            .await?;
        let mut atoms = Vec::new();
        while let Some(row) = rows.next().await? {
            atoms.push(atom_from_row(row)?);
        }
        Ok(atoms)
    }

    pub async fn upsert_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
        input: UpsertLabelSemanticsInput,
    ) -> Result<LabelSemanticsRecord, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let (label_id, label_name) = self.ontology_label(&board_id, label_ref).await?;
        let current = self.get_label_semantics(board, label_ref).await.ok();
        let current_hash = current.as_ref().map(|value| value.semantics_hash.as_str());
        if let Some(expected) = input
            .expected_semantics_hash
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            && Some(expected) != current_hash
        {
            return Err(StoreError::InvalidInput(format!(
                "label semantics hash mismatch: expected {expected}, current {}",
                current_hash.unwrap_or("<none>")
            )));
        }
        let mut description = current.as_ref().and_then(|value| value.description.clone());
        let mut applies_when = current
            .as_ref()
            .map(|value| value.applies_when.clone())
            .unwrap_or_default();
        let mut excludes_when = current
            .as_ref()
            .map(|value| value.excludes_when.clone())
            .unwrap_or_default();
        let mut positive_examples = current
            .as_ref()
            .map(|value| value.positive_examples.clone())
            .unwrap_or_default();
        let mut negative_examples = current
            .as_ref()
            .map(|value| value.negative_examples.clone())
            .unwrap_or_default();
        if input.replace {
            if !input.remove_applies_when.is_empty()
                || !input.remove_excludes_when.is_empty()
                || !input.remove_positive_examples.is_empty()
                || !input.remove_negative_examples.is_empty()
            {
                return Err(StoreError::InvalidInput(
                    "remove_* cannot be combined with replace semantics".to_owned(),
                ));
            }
            description = input.description.clone();
            applies_when = normalize_list(input.applies_when.unwrap_or_default());
            excludes_when = normalize_list(input.excludes_when.unwrap_or_default());
            positive_examples = normalize_list(input.positive_examples.unwrap_or_default());
            negative_examples = normalize_list(input.negative_examples.unwrap_or_default());
        } else {
            if let Some(value) = input.description {
                description = normalize_optional(value);
            }
            remove_items(&mut applies_when, &input.remove_applies_when);
            remove_items(&mut excludes_when, &input.remove_excludes_when);
            remove_items(&mut positive_examples, &input.remove_positive_examples);
            remove_items(&mut negative_examples, &input.remove_negative_examples);
            append_items(&mut applies_when, input.applies_when.unwrap_or_default());
            append_items(&mut excludes_when, input.excludes_when.unwrap_or_default());
            append_items(
                &mut positive_examples,
                input.positive_examples.unwrap_or_default(),
            );
            append_items(
                &mut negative_examples,
                input.negative_examples.unwrap_or_default(),
            );
        }
        let now = now_ms();
        let old_atoms = current
            .as_ref()
            .map(|value| value.atoms.clone())
            .unwrap_or_default();
        let new_atoms = build_atoms(AtomBuildInput {
            label_id: &label_id,
            board_id: &board_id,
            label_name: &label_name,
            description: &description,
            applies: &applies_when,
            excludes: &excludes_when,
            positive: &positive_examples,
            negative: &negative_examples,
            now,
        });
        let after_hash = semantics_hash(
            &label_id,
            &label_name,
            &description,
            &applies_when,
            &excludes_when,
            &positive_examples,
            &negative_examples,
        );
        let before_json = semantics_snapshot_json(
            current.as_ref(),
            &label_id,
            &label_name,
            current_hash.unwrap_or(""),
        );
        let after_json = json!({
            "label_id": label_id,
            "label_name": label_name,
            "description": description,
            "applies_when": applies_when,
            "excludes_when": excludes_when,
            "positive_examples": positive_examples,
            "negative_examples": negative_examples,
            "semantics_hash": after_hash,
        });
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        transaction
            .execute(
                "INSERT INTO label_semantics(label_id,board_id,description,applies_when,excludes_when,positive_examples,negative_examples,created_at,updated_at) VALUES (:label,:board,:description,:applies,:excludes,:positive,:negative,COALESCE((SELECT created_at FROM label_semantics WHERE label_id=:label),:now),:now) ON CONFLICT(label_id) DO UPDATE SET description=excluded.description,applies_when=excluded.applies_when,excludes_when=excluded.excludes_when,positive_examples=excluded.positive_examples,negative_examples=excluded.negative_examples,updated_at=excluded.updated_at",
                turso::named_params! {
                    ":label": label_id.as_str(),
                    ":board": board_id.as_str(),
                    ":description": description.as_deref(),
                    ":applies": serde_json::to_string(&applies_when).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":excludes": serde_json::to_string(&excludes_when).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":positive": serde_json::to_string(&positive_examples).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":negative": serde_json::to_string(&negative_examples).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":now": now,
                },
            )
            .await?;
        transaction
            .execute(
                "DELETE FROM label_atoms WHERE board_id=:board AND label_id=:label",
                turso::named_params! { ":board": board_id.as_str(), ":label": label_id.as_str() },
            )
            .await?;
        for atom in &new_atoms {
            transaction
                .execute(
                    "INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at) VALUES (:id,:label,:board,:polarity,:kind,:text,:ordinal,:hash,:created,:updated)",
                    turso::named_params! {
                        ":id": atom.id.as_str(),
                        ":label": atom.label_id.as_str(),
                        ":board": atom.board_id.as_str(),
                        ":polarity": atom.polarity.as_str(),
                        ":kind": atom.kind.as_str(),
                        ":text": atom.text.as_str(),
                        ":ordinal": atom.ordinal,
                        ":hash": atom.content_hash.as_str(),
                        ":created": atom.created_at,
                        ":updated": atom.updated_at,
                    },
                )
                .await?;
        }
        mark_index_dirty(&transaction, &board_id, now).await?;
        let action_id = insert_action(
            &transaction,
            ActionInsertInput {
                board_id: &board_id,
                action_type: "update_semantics",
                reason: input.reason.as_deref().unwrap_or("update label semantics"),
                signal_ids: &input.source_signal_ids,
                target_label_id: Some(&label_id),
                result_label_id: None,
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: None,
                before_hash: current_hash,
                after_hash: Some(&after_hash),
                change_json: &json!({"before": before_json, "after": after_json}),
                validation_status: "not_required",
                validation_json: "{}",
                now,
                created_by: &input.actor,
                agent_type: None,
            },
        )
        .await?;
        for atom in &old_atoms {
            if !new_atoms
                .iter()
                .any(|value| value.content_hash == atom.content_hash)
            {
                insert_atom_effect(&transaction, &board_id, &action_id, atom, "removed", now)
                    .await?;
            }
        }
        for atom in &new_atoms {
            if !old_atoms
                .iter()
                .any(|value| value.content_hash == atom.content_hash)
            {
                insert_atom_effect(&transaction, &board_id, &action_id, atom, "added", now).await?;
            }
        }
        let event_id = format!(
            "e_ontology_semantics_{}",
            action_id.trim_start_matches("loa_")
        );
        transaction
            .execute(
                "INSERT INTO task_events(event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at) VALUES (:event,:board,NULL,NULL,'label.semantics.updated',:actor,:payload,:created)",
                turso::named_params! {
                    ":event": event_id.as_str(),
                    ":board": board_id.as_str(),
                    ":actor": input.actor.as_str(),
                    ":payload": json!({"label_id": label_id, "action_id": action_id, "semantics_hash": after_hash}).to_string().as_str(),
                    ":created": now,
                },
            )
            .await?;
        transaction.commit().await?;
        self.get_label_semantics(board, &label_id).await
    }

    pub async fn delete_label_semantics(
        &self,
        board: &str,
        label_ref: &str,
        expected_hash: &str,
        reason: &str,
        actor: &str,
    ) -> Result<bool, StoreError> {
        if reason.trim().is_empty() {
            return Err(StoreError::InvalidInput("reason is required".to_owned()));
        }
        let current = self.get_label_semantics(board, label_ref).await?;
        if current.semantics_hash != expected_hash.trim() {
            return Err(StoreError::InvalidInput(format!(
                "label semantics hash mismatch: expected {}, current {}",
                expected_hash, current.semantics_hash
            )));
        }
        let board_id = current.board_id.clone();
        let now = now_ms();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        transaction
            .execute(
                "DELETE FROM label_semantics WHERE board_id=:board AND label_id=:label",
                turso::named_params! { ":board": board_id.as_str(), ":label": current.label_id.as_str() },
            )
            .await?;
        transaction
            .execute(
                "DELETE FROM label_atoms WHERE board_id=:board AND label_id=:label",
                turso::named_params! { ":board": board_id.as_str(), ":label": current.label_id.as_str() },
            )
            .await?;
        mark_index_dirty(&transaction, &board_id, now).await?;
        let action_id = insert_action(
            &transaction,
            ActionInsertInput {
                board_id: &board_id,
                action_type: "update_semantics",
                reason,
                signal_ids: &[],
                target_label_id: Some(&current.label_id),
                result_label_id: None,
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: None,
                before_hash: Some(&current.semantics_hash),
                after_hash: None,
                change_json: &json!({"before": semantics_snapshot_json(Some(&current), &current.label_id, &current.label_name, &current.semantics_hash), "after": null}),
                validation_status: "not_required",
                validation_json: "{}",
                now,
                created_by: actor,
                agent_type: None,
            },
        )
        .await?;
        for atom in &current.atoms {
            insert_atom_effect(&transaction, &board_id, &action_id, atom, "removed", now).await?;
        }
        let event_id = format!(
            "e_ontology_semantics_delete_{}",
            action_id.trim_start_matches("loa_")
        );
        transaction
            .execute(
                "INSERT INTO task_events(event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at) VALUES (:event,:board,NULL,NULL,'label.semantics.deleted',:actor,:payload,:created)",
                turso::named_params! {
                    ":event": event_id.as_str(),
                    ":board": board_id.as_str(),
                    ":actor": actor,
                    ":payload": json!({"label_id": current.label_id, "action_id": action_id}).to_string().as_str(),
                    ":created": now,
                },
            )
            .await?;
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn list_label_atoms(&self, board: &str) -> Result<Vec<LabelAtomRecord>, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let connection = self.connection().await?;
        let mut rows = connection
            .query(
                "SELECT a.id,a.label_id,a.board_id,l.name,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id WHERE a.board_id=:board ORDER BY a.label_id,a.ordinal,a.id",
                turso::named_params! { ":board": board_id.as_str() },
            )
            .await?;
        let mut atoms = Vec::new();
        while let Some(row) = rows.next().await? {
            atoms.push(atom_from_row(row)?);
        }
        Ok(atoms)
    }

    pub async fn explain_label_atom(
        &self,
        board: &str,
        atom_ref: &str,
    ) -> Result<LabelAtomExplainRecord, StoreError> {
        if atom_ref.trim().is_empty() {
            return Err(StoreError::InvalidInput(
                "label atom ref is required".to_owned(),
            ));
        }
        let board_id = self.ontology_board_id(board).await?;
        let connection = self.connection().await?;
        let atom = first_row(
            connection
                .query(
                    "SELECT a.id,a.label_id,a.board_id,l.name,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id WHERE a.board_id=:board AND (a.id=:ref OR a.content_hash=:ref) LIMIT 1",
                    [(":board", board_id.as_str()), (":ref", atom_ref.trim())],
                )
                .await?,
        )
        .await
        .ok()
        .map(atom_from_row)
        .transpose()?;
        let current_semantics = match atom.as_ref() {
            Some(atom) => self.get_label_semantics(board, &atom.label_id).await.ok(),
            None => None,
        };
        let content_hash = atom
            .as_ref()
            .map(|value| value.content_hash.as_str())
            .unwrap_or(atom_ref.trim());
        let mut action_rows = connection
            .query(
                "SELECT DISTINCT a.id,a.board_id,a.parent_action_id,a.action_type,a.reason,a.target_label_id,a.result_label_id,a.result_atom_id,a.result_atom_content_hash,a.result_proposal_id,a.canonical_before_hash,a.canonical_after_hash,a.change_json,a.validation_requirement,a.validation_status,a.validation_json,a.created_by,a.created_by_type,a.agent_type,a.created_at FROM label_ontology_actions a LEFT JOIN label_ontology_action_atom_effects e ON e.action_id=a.id AND e.board_id=a.board_id WHERE a.board_id=:board AND (a.result_atom_id=:ref OR a.result_atom_content_hash=:hash OR e.atom_id_snapshot=:ref OR e.atom_content_hash=:hash) ORDER BY a.created_at ASC,a.id ASC",
                [(":board", board_id.as_str()), (":ref", atom_ref.trim()), (":hash", content_hash)],
            )
            .await?;
        let mut actions = Vec::new();
        while let Some(row) = action_rows.next().await? {
            actions.push(LabelAtomExplainActionRecord {
                action: self.action_from_row(&connection, row).await?,
                matched_by: if atom_ref.trim() == content_hash {
                    "content_hash".to_owned()
                } else {
                    "id".to_owned()
                },
            });
        }
        let mut supporting_signals = Vec::new();
        let mut signal_rows = connection
            .query(
                "SELECT id FROM label_ontology_signals WHERE board_id=:board AND candidate_content_hash=:hash ORDER BY created_at ASC,id ASC",
                [(":board", board_id.as_str()), (":hash", content_hash)],
            )
            .await?;
        while let Some(row) = signal_rows.next().await? {
            let signal_id = text_value(row.get_value(0)?, "signals.id")?;
            let detail = self.get_label_ontology_signal(&signal_id).await?;
            let warnings =
                serde_json::from_str(&detail.observation.diagnostics_json).unwrap_or_default();
            supporting_signals.push(LabelAtomExplainSignalRecord {
                task_id: detail.observation.task_id.clone(),
                task_ref_snapshot: detail.observation.task_ref_snapshot.clone(),
                suggest_input_stale: false,
                suggest_degraded: detail.observation.suggest_degraded,
                warnings,
                signal: detail.signal,
                observation: detail.observation,
            });
        }
        let mut validation_history = Vec::new();
        for action in &actions {
            let mut validation_rows = connection
                .query(
                    "SELECT id,board_id,parent_action_id,action_type,reason,target_label_id,result_label_id,result_atom_id,result_atom_content_hash,result_proposal_id,canonical_before_hash,canonical_after_hash,change_json,validation_requirement,validation_status,validation_json,created_by,created_by_type,agent_type,created_at FROM label_ontology_actions WHERE board_id=:board AND action_type='validate' AND parent_action_id=:parent ORDER BY created_at ASC,id ASC",
                    [(":board", board_id.as_str()), (":parent", action.action.id.as_str())],
                )
                .await?;
            while let Some(row) = validation_rows.next().await? {
                let validation = self.action_from_row(&connection, row).await?;
                let validation_json =
                    serde_json::from_str::<JsonValue>(&validation.validation_json)
                        .unwrap_or_else(|_| json!({}));
                validation_history.push(LabelAtomExplainValidationRecord {
                    parent_action_id: action.action.id.clone(),
                    validation_status: validation.validation_status.clone(),
                    manual_json: validation_json
                        .get("manual")
                        .cloned()
                        .unwrap_or_else(|| json!({}))
                        .to_string(),
                    summary_json: validation_json
                        .get("summary")
                        .cloned()
                        .unwrap_or_else(|| json!({}))
                        .to_string(),
                    cases_json: validation_json
                        .get("cases")
                        .cloned()
                        .unwrap_or_else(|| json!([]))
                        .to_string(),
                    warnings: Vec::new(),
                    action: validation,
                });
            }
        }
        if atom.is_none() && actions.is_empty() {
            return Err(StoreError::InvalidInput(format!(
                "label atom not found: {atom_ref}"
            )));
        }
        let legacy_untracked = atom.is_some() && actions.is_empty();
        let legacy_reason = atom
            .as_ref()
            .filter(|_| legacy_untracked)
            .map(|_| "no ontology provenance action, atom effect, or legacy result atom reference matches this atom id or content hash".to_owned());
        Ok(LabelAtomExplainRecord {
            query: atom_ref.trim().to_owned(),
            atom,
            current_semantics,
            provenance_actions: actions,
            supporting_signals,
            validation_history,
            legacy_untracked,
            legacy_reason,
        })
    }

    pub async fn label_atom_index_status(
        &self,
        board: &str,
    ) -> Result<LabelAtomIndexStatusRecord, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let connection = self.connection().await?;
        let board_row = first_row(
            connection
                .query(
                    "SELECT dirty FROM label_atom_index_boards WHERE store_name=:store AND board_id=:board LIMIT 1",
                    [(":store", LABEL_ATOM_INDEX_STORE), (":board", board_id.as_str())],
                )
                .await?,
        )
        .await
        .ok();
        let dirty = board_row.as_ref().map(|row| {
            integer_value(
                row.get_value(0).unwrap_or(Value::Null),
                "label_atom_index_boards.dirty",
            )
            .unwrap_or(1)
                != 0
        });
        Ok(LabelAtomIndexStatusRecord {
            backend: LABEL_ATOM_INDEX_STORE.to_owned(),
            enabled: false,
            message: "label atom vector provider unavailable; canonical atoms remain queryable"
                .to_owned(),
            diagnostics: vec![
                "vector_provider_unavailable".to_owned(),
                "degraded".to_owned(),
            ],
            dirty,
            board_dirty: dirty,
            generation: None,
        })
    }

    pub async fn rebuild_label_atom_index(
        &self,
        board: &str,
    ) -> Result<LabelAtomIndexStatusRecord, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let now = now_ms();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        transaction
            .execute(
                "INSERT INTO label_atom_index_boards(store_name,board_id,dirty,last_rebuild_at,last_error,updated_at) VALUES (:store,:board,1,NULL,:error,:now) ON CONFLICT(store_name,board_id) DO UPDATE SET dirty=1,last_error=excluded.last_error,updated_at=excluded.updated_at",
                turso::named_params! {
                    ":store": LABEL_ATOM_INDEX_STORE,
                    ":board": board_id.as_str(),
                    ":error": "vector provider unavailable; rebuild deferred",
                    ":now": now,
                },
            )
            .await?;
        transaction.commit().await?;
        self.label_atom_index_status(board).await
    }

    pub async fn query_label_atom_index(
        &self,
        board: &str,
        query: Option<&str>,
        polarity: Option<&str>,
        limit: usize,
    ) -> Result<JsonValue, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let limit = limit.clamp(1, MAX_LIST_LIMIT as usize);
        let connection = self.connection().await?;
        let mut sql = "SELECT a.id,a.label_id,l.name,a.board_id,a.polarity,a.kind,a.text,a.ordinal,a.content_hash FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id WHERE a.board_id=?1".to_owned();
        let mut params = vec![Value::Text(board_id.clone())];
        if let Some(polarity) = polarity.filter(|value| !value.trim().is_empty()) {
            let index = params.len() + 1;
            sql.push_str(&format!(" AND a.polarity=?{index}"));
            params.push(Value::Text(polarity.to_owned()));
        }
        if let Some(q) = query.filter(|value| !value.trim().is_empty()) {
            let index = params.len() + 1;
            sql.push_str(&format!(" AND lower(a.text) LIKE lower(?{index})"));
            params.push(Value::Text(format!("%{}%", q.trim())));
        }
        let index = params.len() + 1;
        sql.push_str(&format!(" ORDER BY a.ordinal,a.id LIMIT ?{index}"));
        params.push(Value::Integer(limit as i64));
        let mut rows = connection
            .query(&sql, turso::params_from_iter(params))
            .await?;
        let mut hits = Vec::new();
        while let Some(row) = rows.next().await? {
            hits.push(json!({
                "atom_id": text_value(row.get_value(0)?, "label_atoms.id")?,
                "label_id": text_value(row.get_value(1)?, "label_atoms.label_id")?,
                "label_name": text_value(row.get_value(2)?, "labels.name")?,
                "board_id": text_value(row.get_value(3)?, "label_atoms.board_id")?,
                "polarity": text_value(row.get_value(4)?, "label_atoms.polarity")?,
                "kind": text_value(row.get_value(5)?, "label_atoms.kind")?,
                "text": text_value(row.get_value(6)?, "label_atoms.text")?,
                "ordinal": integer_value(row.get_value(7)?, "label_atoms.ordinal")?,
                "content_hash": text_value(row.get_value(8)?, "label_atoms.content_hash")?,
                "embedding_model": "unavailable",
                "distance": 0.0,
            }));
        }
        Ok(json!({"data": hits, "degraded": true, "diagnostics": ["vector_provider_unavailable"]}))
    }

    pub async fn suggest_task_labels(
        &self,
        board: &str,
        task_ref: &str,
        options: LabelSuggestionOptions,
    ) -> Result<LabelSuggestionResultRecord, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let (task_id, _, title, description) = self.ontology_task(&board_id, task_ref).await?;
        let query = format!("{} {}", title, description.unwrap_or_default()).to_lowercase();
        let atoms = self.list_label_atoms(board).await?;
        let mut grouped = BTreeMap::<String, LabelSuggestionCandidateRecord>::new();
        for atom in atoms.into_iter().take(options.atom_limit.max(1)) {
            let haystack = atom.text.to_lowercase();
            let score = token_overlap(&query, &haystack);
            if score <= 0.0 {
                continue;
            }
            let evidence = LabelSuggestionEvidenceRecord {
                atom_id: atom.id.clone(),
                label_id: atom.label_id.clone(),
                label_name: atom.label_name.clone(),
                polarity: atom.polarity.clone(),
                kind: atom.kind.clone(),
                text: atom.text.clone(),
                score,
            };
            let entry = grouped.entry(atom.label_id.clone()).or_insert_with(|| {
                LabelSuggestionCandidateRecord {
                    label_id: atom.label_id.clone(),
                    label_name: atom.label_name.clone(),
                    score: 0.0,
                    weight: 0.0,
                    already_applied: false,
                    evidence_atoms: Vec::new(),
                    negative_evidence_atoms: Vec::new(),
                }
            });
            entry.score = entry.score.max(score);
            entry.weight += score;
            if atom.polarity == "negative" {
                entry.negative_evidence_atoms.push(evidence);
            } else {
                entry.evidence_atoms.push(evidence);
            }
        }
        let mut candidates = grouped.into_values().collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.label_id.cmp(&right.label_id))
        });
        candidates.truncate(options.candidate_limit.max(1));
        let selected = candidates
            .iter()
            .filter(|candidate| candidate.score >= options.min_score)
            .take(options.max_selected_labels.max(1))
            .cloned()
            .collect::<Vec<_>>();
        let coverage = selected
            .iter()
            .map(|value| value.score)
            .sum::<f32>()
            .min(1.0);
        Ok(LabelSuggestionResultRecord {
            task_id,
            board_id,
            selected_labels: selected,
            candidates,
            coverage,
            coverage_cosine: 0.0,
            residual_norm: 1.0 - coverage,
            needs_new_label: coverage < options.min_score,
            reason_codes: vec!["lexical_fallback".to_owned()],
            degraded: true,
            diagnostics: vec!["vector_provider_unavailable".to_owned()],
        })
    }

    pub async fn propose_task_label(
        &self,
        board: &str,
        task_ref: &str,
        input: LabelProposalInput,
    ) -> Result<LabelProposalAttemptRecord, StoreError> {
        let options = LabelSuggestionOptions::default();
        let suggestion = self.suggest_task_labels(board, task_ref, options).await?;
        let top1 = suggestion.candidates.first();
        let Some(name) = input.name.filter(|value| !value.trim().is_empty()) else {
            return Ok(LabelProposalAttemptRecord {
                task_id: suggestion.task_id,
                board_id: suggestion.board_id,
                proposal: None,
                degraded: true,
                diagnostics: vec!["label_proposal_provider_unavailable".to_owned()],
                heuristic_coverage: suggestion.coverage,
                heuristic_coverage_cosine: suggestion.coverage_cosine,
                heuristic_residual_norm: suggestion.residual_norm,
                top1_existing_label_id: top1.map(|value| value.label_id.clone()),
                top1_existing_label_name: top1.map(|value| value.label_name.clone()),
            });
        };
        let board_id = suggestion.board_id.clone();
        let task_id = suggestion.task_id.clone();
        let now = now_ms();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let proposal_id = format!("lp_{}", ulid::Ulid::new());
        let diagnostics = vec![
            "manual_candidate".to_owned(),
            "vector_provider_unavailable".to_owned(),
        ];
        transaction
            .execute(
                "INSERT INTO label_semantic_proposals(id,board_id,task_id,status,name,description,applies_when,excludes_when,positive_examples,negative_examples,heuristic_coverage,heuristic_residual_norm,heuristic_coverage_cosine,top1_existing_label_id,top1_existing_label_name,diagnostics_json,created_by,created_at,updated_at) VALUES (:id,:board,:task,'proposed',:name,:description,:applies,:excludes,:positive,:negative,:coverage,:residual,:cosine,:top_id,:top_name,:diagnostics,:actor,:now,:now)",
                turso::named_params! {
                    ":id": proposal_id.as_str(),
                    ":board": board_id.as_str(),
                    ":task": task_id.as_str(),
                    ":name": name.trim(),
                    ":description": input.description.as_deref(),
                    ":applies": serde_json::to_string(&normalize_list(input.applies_when)).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":excludes": serde_json::to_string(&normalize_list(input.excludes_when)).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":positive": serde_json::to_string(&normalize_list(input.positive_examples)).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":negative": serde_json::to_string(&normalize_list(input.negative_examples)).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":coverage": suggestion.coverage,
                    ":residual": suggestion.residual_norm,
                    ":cosine": suggestion.coverage_cosine,
                    ":top_id": top1.map(|value| value.label_id.as_str()),
                    ":top_name": top1.map(|value| value.label_name.as_str()),
                    ":diagnostics": serde_json::to_string(&diagnostics).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":actor": input.actor.as_str(),
                    ":now": now,
                },
            )
            .await?;
        let _action_id = insert_action(
            &transaction,
            ActionInsertInput {
                board_id: &board_id,
                action_type: "create_label_proposal",
                reason: "create label proposal",
                signal_ids: &input.source_signal_ids,
                target_label_id: None,
                result_label_id: None,
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: Some(&proposal_id),
                before_hash: None,
                after_hash: None,
                change_json: &json!({"proposal_id": proposal_id, "name": name}),
                validation_status: "not_required",
                validation_json: "{}",
                now,
                created_by: &input.actor,
                agent_type: None,
            },
        )
        .await?;
        transaction.commit().await?;
        let proposal = self.get_label_proposal(&proposal_id).await?;
        Ok(LabelProposalAttemptRecord {
            task_id,
            board_id,
            proposal: Some(proposal),
            degraded: true,
            diagnostics,
            heuristic_coverage: suggestion.coverage,
            heuristic_coverage_cosine: suggestion.coverage_cosine,
            heuristic_residual_norm: suggestion.residual_norm,
            top1_existing_label_id: top1.map(|value| value.label_id.clone()),
            top1_existing_label_name: top1.map(|value| value.label_name.clone()),
        })
    }

    pub async fn list_label_proposals(
        &self,
        board: &str,
        task_ref: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<LabelSemanticProposalRecord>, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let task_id = match task_ref {
            Some(task_ref) => Some(self.ontology_task(&board_id, task_ref).await?.0),
            None => None,
        };
        self.query_proposals(Some(&board_id), task_id.as_deref(), status)
            .await
    }

    pub async fn get_label_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<LabelSemanticProposalRecord, StoreError> {
        let records = self.query_proposals(None, None, None).await?;
        records
            .into_iter()
            .find(|value| value.id == proposal_id)
            .ok_or_else(|| {
                StoreError::InvalidInput(format!("label proposal not found: {proposal_id}"))
            })
    }

    async fn query_proposals(
        &self,
        board_id: Option<&str>,
        task_id: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<LabelSemanticProposalRecord>, StoreError> {
        let connection = self.connection().await?;
        let mut sql = "SELECT id,board_id,task_id,status,name,description,applies_when,excludes_when,positive_examples,negative_examples,heuristic_coverage,heuristic_residual_norm,heuristic_coverage_cosine,top1_existing_label_id,top1_existing_label_name,diagnostics_json,created_by,decision_reason,resolved_label_id,created_at,updated_at,decided_at FROM label_semantic_proposals WHERE 1=1".to_owned();
        let mut params = Vec::<Value>::new();
        if let Some(board_id) = board_id {
            let index = params.len() + 1;
            sql.push_str(&format!(" AND board_id=?{index}"));
            params.push(Value::Text(board_id.to_owned()));
        }
        if let Some(task_id) = task_id {
            let index = params.len() + 1;
            sql.push_str(&format!(" AND task_id=?{index}"));
            params.push(Value::Text(task_id.to_owned()));
        }
        if let Some(status) = status {
            let index = params.len() + 1;
            sql.push_str(&format!(" AND status=?{index}"));
            params.push(Value::Text(status.to_owned()));
        }
        sql.push_str(" ORDER BY created_at DESC,id DESC");
        let mut rows = connection
            .query(&sql, turso::params_from_iter(params))
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(proposal_from_row(row)?);
        }
        Ok(result)
    }

    pub async fn decide_label_proposal(
        &self,
        input: LabelProposalDecisionInput,
    ) -> Result<LabelSemanticProposalRecord, StoreError> {
        let proposal = self.get_label_proposal(&input.proposal_id).await?;
        let status = if input.accept { "accepted" } else { "rejected" };
        let now = now_ms();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        transaction
            .execute(
                "UPDATE label_semantic_proposals SET status=:status,decision_reason=:reason,decided_at=:now,updated_at=:now WHERE id=:id AND status='proposed'",
                turso::named_params! {
                    ":status": status,
                    ":reason": input.reason.as_deref(),
                    ":now": now,
                    ":id": input.proposal_id.as_str(),
                },
            )
            .await?;
        let _action_id = insert_action(
            &transaction,
            ActionInsertInput {
                board_id: &proposal.board_id,
                action_type: if input.accept { "confirm" } else { "reject" },
                reason: input.reason.as_deref().unwrap_or("proposal decision"),
                signal_ids: &input.source_signal_ids,
                target_label_id: None,
                result_label_id: None,
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: Some(&proposal.id),
                before_hash: None,
                after_hash: None,
                change_json: &json!({"proposal_id": proposal.id, "status": status}),
                validation_status: "not_required",
                validation_json: "{}",
                now,
                created_by: &input.actor,
                agent_type: None,
            },
        )
        .await?;
        transaction.commit().await?;
        self.get_label_proposal(&input.proposal_id).await
    }

    pub async fn record_label_ontology_observation(
        &self,
        board: &str,
        input: OntologyObservationInput,
    ) -> Result<LabelOntologyObservationRecord, StoreError> {
        if input.signals.is_empty() {
            return Err(StoreError::InvalidInput(
                "at least one ontology signal is required".to_owned(),
            ));
        }
        let board_id = self.ontology_board_id(board).await?;
        let (task_id, task_ref, title, description) =
            self.ontology_task(&board_id, &input.task_ref).await?;
        validate_actor(&input.actor)?;
        let task_snapshot =
            json!({"id": task_id, "ref": task_ref, "title": title, "description": description});
        let agent_candidates = json_array_or_default(&input.agent_candidates_json)?;
        let suggestion_snapshot = json_object_or_default(&input.suggestion_snapshot_json)?;
        let final_decision = json_object_or_default(&input.final_decision_json)?;
        let diagnostics = json_array_or_default(&input.diagnostics_json)?;
        let signal_fingerprint = serde_json::to_string(
            &input
                .signals
                .iter()
                .map(|signal| (&signal.kind, &signal.signal_key, &signal.rationale))
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default();
        let capture = input
            .capture_fingerprint
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                fnv_hash(&format!(
                    "{}\n{}\n{}\n{}",
                    task_snapshot, agent_candidates, suggestion_snapshot, signal_fingerprint
                ))
            });
        {
            let connection = self.connection().await?;
            let existing = first_row(
                connection
                    .query(
                        "SELECT id FROM label_ontology_observations WHERE board_id=:board AND capture_fingerprint=:fingerprint LIMIT 1",
                        [ (":board", board_id.as_str()), (":fingerprint", capture.as_str()) ],
                    )
                    .await?,
            )
            .await
            .ok();
            if let Some(row) = existing {
                let observation_id = text_value(row.get_value(0)?, "observations.id")?;
                return self.observation_by_id(&board_id, &observation_id).await;
            }
        }
        let mut resolved_targets = Vec::with_capacity(input.signals.len());
        for signal in &input.signals {
            resolved_targets.push(match signal.target_label_ref.as_deref() {
                Some(value) => {
                    let (id, name) = self.ontology_label(&board_id, value).await?;
                    (Some(id), Some(name))
                }
                None => (None, None),
            });
        }
        let now = now_ms();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let observation_id = format!("lor_{}", ulid::Ulid::new());
        transaction.execute(
            "INSERT INTO label_ontology_observations(id,board_id,task_id,task_ref_snapshot,task_snapshot_json,agent_candidates_json,suggestion_snapshot_json,final_decision_json,suggest_coverage,suggest_coverage_cosine,suggest_residual_norm,suggest_needs_new_label,suggest_degraded,diagnostics_json,capture_fingerprint,suggest_input_hash,created_by,created_by_type,agent_type,created_at) VALUES (:id,:board,:task,:task_ref,:task_snapshot,:candidates,:suggestion,:decision,:coverage,:cosine,:residual,:needs,:degraded,:diagnostics,:fingerprint,:input_hash,:actor,:actor_type,:agent_type,:now)",
            turso::named_params! {
                ":id": observation_id.as_str(), ":board": board_id.as_str(), ":task": task_id.as_str(), ":task_ref": task_ref.as_str(),
                ":task_snapshot": task_snapshot.to_string().as_str(), ":candidates": agent_candidates.to_string().as_str(), ":suggestion": suggestion_snapshot.to_string().as_str(), ":decision": final_decision.to_string().as_str(),
                ":coverage": input.suggest_coverage, ":cosine": input.suggest_coverage_cosine, ":residual": input.suggest_residual_norm, ":needs": i64::from(input.suggest_needs_new_label), ":degraded": i64::from(input.suggest_degraded), ":diagnostics": diagnostics.to_string().as_str(), ":fingerprint": capture.as_str(), ":input_hash": Option::<&str>::None, ":actor": input.actor.name.as_str(), ":actor_type": input.actor.actor_type.as_str(), ":agent_type": input.actor.agent_type.as_deref(), ":now": now,
            },
        ).await?;
        for (index, signal) in input.signals.iter().enumerate() {
            let signal_id = format!(
                "los_{}_{}",
                observation_id.trim_start_matches("lor_"),
                index
            );
            let signal_key = signal
                .signal_key
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("{}-{index}", signal.kind));
            let (target_label_id, target_label_name) = resolved_targets[index].clone();
            let candidate_hash = match (
                &signal.candidate_atom_polarity,
                &signal.candidate_atom_kind,
                &signal.candidate_text,
            ) {
                (Some(polarity), Some(kind), Some(text)) => Some(fnv_hash(&format!(
                    "{polarity}\n{kind}\n{}",
                    normalize_text(text)
                ))),
                _ => None,
            };
            transaction.execute(
                "INSERT INTO label_ontology_signals(id,board_id,observation_id,kind,status,target_label_id,target_label_name_snapshot,related_labels_json,proposed_action,candidate_atom_polarity,candidate_atom_kind,candidate_text,candidate_content_hash,proposed_label_name,proposed_label_name_normalized,proposal_json,agent_selected,suggest_state,suggest_score,suggest_rank,final_selected,rationale,confidence,signal_key,created_at,updated_at) VALUES (:id,:board,:observation,:kind,'open',:target,:target_name,:related,:action,:polarity,:atom_kind,:text,:hash,:proposal_name,:proposal_name_norm,:proposal,:agent_selected,:suggest_state,:score,:rank,:final_selected,:rationale,:confidence,:signal_key,:now,:now)",
                turso::named_params! {
                    ":id": signal_id.as_str(), ":board": board_id.as_str(), ":observation": observation_id.as_str(), ":kind": signal.kind.as_str(), ":target": target_label_id.as_deref(), ":target_name": target_label_name.as_deref(), ":related": json_array_or_default(&signal.related_labels_json)?.to_string().as_str(), ":action": signal.proposed_action.as_str(), ":polarity": signal.candidate_atom_polarity.as_deref(), ":atom_kind": signal.candidate_atom_kind.as_deref(), ":text": signal.candidate_text.as_deref().map(normalize_text).as_deref(), ":hash": candidate_hash.as_deref(), ":proposal_name": signal.proposed_label_name.as_deref(), ":proposal_name_norm": signal.proposed_label_name.as_deref().map(normalize_label_name).as_deref(), ":proposal": json_object_or_default(&signal.proposal_json)?.to_string().as_str(), ":agent_selected": i64::from(signal.agent_selected), ":suggest_state": signal.suggest_state.as_deref(), ":score": signal.suggest_score, ":rank": signal.suggest_rank, ":final_selected": i64::from(signal.final_selected), ":rationale": signal.rationale.trim(), ":confidence": signal.confidence, ":signal_key": signal_key.as_str(), ":now": now,
                },
            ).await?;
        }
        transaction.commit().await?;
        self.observation_by_id(&board_id, &observation_id).await
    }

    pub async fn list_label_ontology_signals(
        &self,
        board: &str,
        statuses: &[String],
        kinds: &[String],
        label_filters: (Option<&str>, Option<&str>, Option<&str>),
        include_all: bool,
        limit: usize,
    ) -> Result<Vec<LabelOntologySignalRecord>, StoreError> {
        let (task_ref, target_label_ref, proposed_label_name) = label_filters;
        let board_id = self.ontology_board_id(board).await?;
        let task_id = match task_ref {
            Some(value) => Some(self.ontology_task(&board_id, value).await?.0),
            None => None,
        };
        let target_label_id = match target_label_ref {
            Some(value) => Some(self.ontology_label(&board_id, value).await?.0),
            None => None,
        };
        let connection = self.connection().await?;
        let mut sql = "SELECT id,observation_id,board_id,kind,status,target_label_id,target_label_name_snapshot,related_labels_json,proposed_action,candidate_atom_polarity,candidate_atom_kind,candidate_text,candidate_content_hash,proposed_label_name,proposed_label_name_normalized,proposal_json,agent_selected,suggest_state,suggest_score,suggest_rank,final_selected,rationale,confidence,signal_key,superseded_by_signal_id,status_reason,created_at,updated_at,reviewed_at,closed_at FROM label_ontology_signals WHERE board_id=:board".to_owned();
        let mut params: Vec<Value> = vec![Value::Text(board_id.clone())];
        let board_placeholder = "?1";
        sql = sql.replace(":board", board_placeholder);
        if !include_all {
            sql.push_str(" AND status IN ('open','confirmed')");
        }
        if !statuses.is_empty() {
            sql.push_str(" AND status IN (");
            for (index, status) in statuses.iter().enumerate() {
                if index > 0 {
                    sql.push(',');
                }
                let placeholder = params.len() + 1;
                sql.push_str(&format!("?{placeholder}"));
                params.push(Value::Text(status.clone()));
            }
            sql.push(')');
        }
        if !kinds.is_empty() {
            sql.push_str(" AND kind IN (");
            for (index, kind) in kinds.iter().enumerate() {
                if index > 0 {
                    sql.push(',');
                }
                let placeholder = params.len() + 1;
                sql.push_str(&format!("?{placeholder}"));
                params.push(Value::Text(kind.clone()));
            }
            sql.push(')');
        }
        if let Some(task_id) = task_id {
            let board_placeholder = params.len() + 1;
            let task_placeholder = board_placeholder + 1;
            sql.push_str(&format!(" AND observation_id IN (SELECT id FROM label_ontology_observations WHERE board_id=?{board_placeholder} AND task_id=?{task_placeholder})"));
            params.push(Value::Text(board_id.clone()));
            params.push(Value::Text(task_id));
        }
        if let Some(label_id) = target_label_id {
            let placeholder = params.len() + 1;
            sql.push_str(&format!(" AND target_label_id=?{placeholder}"));
            params.push(Value::Text(label_id));
        }
        if let Some(name) = proposed_label_name {
            let placeholder = params.len() + 1;
            sql.push_str(&format!(
                " AND proposed_label_name_normalized=?{placeholder}"
            ));
            params.push(Value::Text(normalize_label_name(name)));
        }
        let limit_placeholder = params.len() + 1;
        sql.push_str(&format!(
            " ORDER BY created_at ASC,id ASC LIMIT ?{limit_placeholder}"
        ));
        params.push(Value::Integer(
            limit.clamp(1, MAX_LIST_LIMIT as usize) as i64
        ));
        let mut rows = connection
            .query(&sql, turso::params_from_iter(params))
            .await?;
        let mut result = Vec::new();
        while let Some(row) = rows.next().await? {
            result.push(signal_from_row(row)?);
        }
        Ok(result)
    }

    pub async fn get_label_ontology_signal(
        &self,
        signal_id: &str,
    ) -> Result<LabelOntologySignalDetailRecord, StoreError> {
        let connection = self.connection().await?;
        let row = first_row(connection.query("SELECT id,observation_id,board_id,kind,status,target_label_id,target_label_name_snapshot,related_labels_json,proposed_action,candidate_atom_polarity,candidate_atom_kind,candidate_text,candidate_content_hash,proposed_label_name,proposed_label_name_normalized,proposal_json,agent_selected,suggest_state,suggest_score,suggest_rank,final_selected,rationale,confidence,signal_key,superseded_by_signal_id,status_reason,created_at,updated_at,reviewed_at,closed_at FROM label_ontology_signals WHERE id=:id LIMIT 1", [(":id", signal_id)]).await?).await.map_err(|error| match error { turso::Error::QueryReturnedNoRows => StoreError::InvalidInput(format!("ontology signal not found: {signal_id}")), other => StoreError::Turso(other) })?;
        let signal = signal_from_row(row)?;
        let observation = self
            .observation_by_id(&signal.board_id, &signal.observation_id)
            .await?;
        let mut action_rows = connection.query("SELECT a.id,a.board_id,a.parent_action_id,a.action_type,a.reason,a.target_label_id,a.result_label_id,a.result_atom_id,a.result_atom_content_hash,a.result_proposal_id,a.canonical_before_hash,a.canonical_after_hash,a.change_json,a.validation_requirement,a.validation_status,a.validation_json,a.created_by,a.created_by_type,a.agent_type,a.created_at FROM label_ontology_actions a JOIN label_ontology_action_signals x ON x.action_id=a.id AND x.board_id=a.board_id WHERE x.signal_id=:signal AND x.board_id=:board ORDER BY a.created_at ASC,a.id ASC", [(":signal", signal_id), (":board", signal.board_id.as_str())]).await?;
        let mut actions = Vec::new();
        while let Some(row) = action_rows.next().await? {
            actions.push(self.action_from_row(&connection, row).await?);
        }
        Ok(LabelOntologySignalDetailRecord {
            signal,
            observation,
            actions,
        })
    }

    pub async fn review_label_ontology(
        &self,
        board: &str,
        group_by: &str,
        include_all: bool,
        limit: usize,
    ) -> Result<Vec<LabelOntologyReviewGroupRecord>, StoreError> {
        let signals = self
            .list_label_ontology_signals(
                board,
                &[],
                &[],
                (None, None, None),
                include_all,
                MAX_LIST_LIMIT as usize,
            )
            .await?;
        let mut groups = BTreeMap::<String, Vec<LabelOntologySignalRecord>>::new();
        for signal in signals {
            let key = match group_by {
                "candidate_atom" | "candidate-atom" => format!(
                    "{}:{}:{}",
                    signal.candidate_atom_polarity.as_deref().unwrap_or(""),
                    signal.candidate_atom_kind.as_deref().unwrap_or(""),
                    signal.candidate_content_hash.as_deref().unwrap_or("")
                ),
                "proposed_label" | "proposed-label" => signal
                    .proposed_label_name_normalized
                    .clone()
                    .unwrap_or_default(),
                _ => signal
                    .target_label_id
                    .clone()
                    .unwrap_or_else(|| "unassigned".to_owned()),
            };
            groups.entry(key).or_default().push(signal);
        }
        let mut result = Vec::new();
        for (key, values) in groups {
            result.push(review_group(group_by, key, values));
        }
        result.sort_by(|left, right| {
            right
                .signal_count
                .cmp(&left.signal_count)
                .then_with(|| left.key.cmp(&right.key))
        });
        result.truncate(limit.clamp(1, MAX_LIST_LIMIT as usize));
        Ok(result)
    }

    pub async fn create_label_ontology_action(
        &self,
        board: &str,
        input: OntologyActionInput,
    ) -> Result<LabelOntologyActionRecord, StoreError> {
        validate_actor(&input.actor)?;
        if input.reason.trim().is_empty() {
            return Err(StoreError::InvalidInput("reason is required".to_owned()));
        }
        let board_id = self.ontology_board_id(board).await?;
        let target_label_id = match input.target_label_ref.as_deref() {
            Some(value) => Some(self.ontology_label(&board_id, value).await?.0),
            None => None,
        };
        let result_label_id = match input.result_label_ref.as_deref() {
            Some(value) => Some(self.ontology_label(&board_id, value).await?.0),
            None => None,
        };
        if let Some(parent) = input.parent_action_id.as_deref() {
            let connection = self.connection().await?;
            let row = first_row(
                connection
                    .query(
                        "SELECT board_id FROM label_ontology_actions WHERE id=:id",
                        [(":id", parent)],
                    )
                    .await?,
            )
            .await
            .map_err(|_| StoreError::InvalidInput("parent action not found".to_owned()))?;
            if text_value(row.get_value(0)?, "label_ontology_actions.board_id")? != board_id {
                return Err(StoreError::InvalidInput(
                    "parent action belongs to another board".to_owned(),
                ));
            }
        }
        let now = now_ms();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        for signal_id in &input.signal_ids {
            let row = first_row(
                transaction
                    .query(
                        "SELECT id FROM label_ontology_signals WHERE board_id=:board AND id=:id LIMIT 1",
                        [(":board", board_id.as_str()), (":id", signal_id.as_str())],
                    )
                    .await?,
            )
            .await;
            if row.is_err() {
                return Err(StoreError::InvalidInput(format!(
                    "ontology signal does not exist on board: {signal_id}"
                )));
            }
        }
        if matches!(
            input.action_type.as_str(),
            "confirm" | "reject" | "resolve_no_change" | "supersede"
        ) {
            update_signal_status(
                &transaction,
                &board_id,
                &input.signal_ids,
                &input.action_type,
                input.superseded_by_signal_id.as_deref(),
                input.reason.as_str(),
                now,
            )
            .await?;
        }
        let change_json = json_string_or_object(&input.change_json)?;
        let validation_json = json_string_or_object(&input.validation_json)?.to_string();
        let action_id = insert_action(
            &transaction,
            ActionInsertInput {
                board_id: &board_id,
                action_type: &input.action_type,
                reason: &input.reason,
                signal_ids: &input.signal_ids,
                target_label_id: target_label_id.as_deref(),
                result_label_id: result_label_id.as_deref(),
                result_atom_id: input.result_atom_id.as_deref(),
                result_atom_content_hash: input.result_atom_content_hash.as_deref(),
                result_proposal_id: input.result_proposal_id.as_deref(),
                before_hash: input.canonical_before_hash.as_deref(),
                after_hash: input.canonical_after_hash.as_deref(),
                change_json: &change_json,
                validation_status: input.validation_status.as_deref().unwrap_or("not_required"),
                validation_json: &validation_json,
                now,
                created_by: &input.actor.name,
                agent_type: input.actor.agent_type.as_deref(),
            },
        )
        .await?;
        transaction.commit().await?;
        self.action_by_id(&board_id, &action_id).await
    }

    pub async fn apply_label_ontology_atom(
        &self,
        board: &str,
        input: OntologyApplyAtomInput,
    ) -> Result<LabelOntologyActionRecord, StoreError> {
        validate_actor(&input.actor)?;
        if input.reason.trim().is_empty() {
            return Err(StoreError::InvalidInput("reason is required".to_owned()));
        }
        let board_id = self.ontology_board_id(board).await?;
        let (label_id, label_name) = self.ontology_label(&board_id, &input.label_ref).await?;
        validate_atom_kind(&input.polarity, &input.kind, &input.text)?;
        let now = now_ms();
        let current = self.get_label_semantics(board, &label_id).await.ok();
        let before_hash = current.as_ref().map(|value| value.semantics_hash.clone());
        let mut applies = current
            .as_ref()
            .map(|v| v.applies_when.clone())
            .unwrap_or_default();
        let mut excludes = current
            .as_ref()
            .map(|v| v.excludes_when.clone())
            .unwrap_or_default();
        let mut positive = current
            .as_ref()
            .map(|v| v.positive_examples.clone())
            .unwrap_or_default();
        let mut negative = current
            .as_ref()
            .map(|v| v.negative_examples.clone())
            .unwrap_or_default();
        let description = current.as_ref().and_then(|v| v.description.clone());
        match (input.polarity.as_str(), input.kind.as_str()) {
            ("positive", "applies_when") => append_items(&mut applies, vec![input.text.clone()]),
            ("negative", "excludes_when") => append_items(&mut excludes, vec![input.text.clone()]),
            ("positive", "positive_example") => {
                append_items(&mut positive, vec![input.text.clone()])
            }
            ("negative", "negative_example") => {
                append_items(&mut negative, vec![input.text.clone()])
            }
            ("positive", "name") | ("positive", "description") => {
                return Err(StoreError::InvalidInput(
                    "name/description atoms are derived and cannot be applied directly".to_owned(),
                ));
            }
            _ => {
                return Err(StoreError::InvalidInput(
                    "invalid atom polarity/kind".to_owned(),
                ));
            }
        }
        let atoms = build_atoms(AtomBuildInput {
            label_id: &label_id,
            board_id: &board_id,
            label_name: &label_name,
            description: &description,
            applies: &applies,
            excludes: &excludes,
            positive: &positive,
            negative: &negative,
            now,
        });
        let atom = atoms
            .iter()
            .find(|value| {
                value.text == normalize_text(&input.text)
                    && value.polarity == input.polarity
                    && value.kind == input.kind
            })
            .cloned()
            .ok_or_else(|| StoreError::InvalidInput("atom was not produced".to_owned()))?;
        let after_hash = semantics_hash(
            &label_id,
            &label_name,
            &description,
            &applies,
            &excludes,
            &positive,
            &negative,
        );
        let changed = before_hash.as_deref() != Some(after_hash.as_str());
        let before_snapshot = semantics_snapshot_json(
            current.as_ref(),
            &label_id,
            &label_name,
            before_hash.as_deref().unwrap_or(""),
        );
        let after_snapshot = json!({
            "label_id": label_id,
            "label_name": label_name,
            "description": description,
            "applies_when": applies,
            "excludes_when": excludes,
            "positive_examples": positive,
            "negative_examples": negative,
            "semantics_hash": after_hash,
        });
        let action_type = if changed {
            if input.polarity == "positive" {
                "add_positive_atom"
            } else {
                "add_negative_atom"
            }
        } else {
            "adopt_existing_atom"
        };
        let validation_status = if changed { "pending" } else { "not_required" };
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        for signal_id in &input.signal_ids {
            let row = first_row(
                transaction
                    .query(
                        "SELECT id FROM label_ontology_signals WHERE board_id=:board AND id=:id LIMIT 1",
                        [(":board", board_id.as_str()), (":id", signal_id.as_str())],
                    )
                    .await?,
            )
            .await;
            if row.is_err() {
                return Err(StoreError::InvalidInput(format!(
                    "ontology signal does not exist on board: {signal_id}"
                )));
            }
        }
        if changed {
            transaction.execute("INSERT INTO label_semantics(label_id,board_id,description,applies_when,excludes_when,positive_examples,negative_examples,created_at,updated_at) VALUES (:label,:board,:description,:applies,:excludes,:positive,:negative,COALESCE((SELECT created_at FROM label_semantics WHERE label_id=:label),:now),:now) ON CONFLICT(label_id) DO UPDATE SET description=excluded.description,applies_when=excluded.applies_when,excludes_when=excluded.excludes_when,positive_examples=excluded.positive_examples,negative_examples=excluded.negative_examples,updated_at=excluded.updated_at", turso::named_params! {":label":label_id.as_str(),":board":board_id.as_str(),":description":description.as_deref(),":applies":serde_json::to_string(&applies).unwrap_or_else(|_|"[]".to_owned()),":excludes":serde_json::to_string(&excludes).unwrap_or_else(|_|"[]".to_owned()),":positive":serde_json::to_string(&positive).unwrap_or_else(|_|"[]".to_owned()),":negative":serde_json::to_string(&negative).unwrap_or_else(|_|"[]".to_owned()),":now":now}).await?;
            transaction
                .execute(
                    "DELETE FROM label_atoms WHERE board_id=:board AND label_id=:label",
                    [(":board", board_id.as_str()), (":label", label_id.as_str())],
                )
                .await?;
            for atom_value in &atoms {
                transaction.execute("INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at) VALUES (:id,:label,:board,:polarity,:kind,:text,:ordinal,:hash,:created,:updated)", turso::named_params! {":id": atom_value.id.as_str(), ":label": atom_value.label_id.as_str(), ":board": atom_value.board_id.as_str(), ":polarity": atom_value.polarity.as_str(), ":kind": atom_value.kind.as_str(), ":text": atom_value.text.as_str(), ":ordinal": atom_value.ordinal, ":hash": atom_value.content_hash.as_str(), ":created": atom_value.created_at, ":updated": atom_value.updated_at}).await?;
            }
            mark_index_dirty(&transaction, &board_id, now).await?;
        }
        let action_id = insert_action(
            &transaction,
            ActionInsertInput {
                board_id: &board_id,
                action_type,
                reason: &input.reason,
                signal_ids: &input.signal_ids,
                target_label_id: Some(&label_id),
                result_label_id: None,
                result_atom_id: Some(&atom.id),
                result_atom_content_hash: Some(&atom.content_hash),
                result_proposal_id: None,
                before_hash: before_hash.as_deref(),
                after_hash: Some(&after_hash),
                change_json: &json!({
                    "label": {"id": label_id, "name": label_name},
                    "added_atom": {"id": atom.id, "content_hash": atom.content_hash, "polarity": atom.polarity, "kind": atom.kind, "text": atom.text},
                    "changed": changed,
                    "canonical_changed": changed,
                    "provenance_only": !changed,
                    "requested_action_type": if input.polarity == "positive" { "add_positive_atom" } else { "add_negative_atom" },
                    "before": before_snapshot,
                    "after": after_snapshot,
                    "retarget_override": null,
                }),
                validation_status,
                validation_json: "{}",
                now,
                created_by: &input.actor.name,
                agent_type: input.actor.agent_type.as_deref(),
            },
        )
        .await?;
        if changed {
            insert_atom_effect(&transaction, &board_id, &action_id, &atom, "added", now).await?;
        }
        transaction.commit().await?;
        self.action_by_id(&board_id, &action_id).await
    }

    pub async fn revert_label_ontology_mutation(
        &self,
        board: &str,
        input: OntologyRevertInput,
    ) -> Result<LabelOntologyActionRecord, StoreError> {
        validate_actor(&input.actor)?;
        if input.reason.trim().is_empty() {
            return Err(StoreError::InvalidInput("reason is required".to_owned()));
        }
        let board_id = self.ontology_board_id(board).await?;
        let target = self
            .action_by_id(&board_id, &input.target_action_id)
            .await?;
        if let Some(expected) = input.expected_current_hash.as_deref() {
            let current = target
                .target_label_id
                .as_deref()
                .map(|label| async { self.get_label_semantics(board, label).await.ok() });
            let current_hash = if let Some(future) = current {
                future.await.map(|v| v.semantics_hash)
            } else {
                None
            };
            if current_hash.as_deref() != Some(expected) {
                return Err(StoreError::InvalidInput(format!(
                    "current canonical hash mismatch: expected {expected}, current {}",
                    current_hash.as_deref().unwrap_or("<none>")
                )));
            }
        }
        let change: JsonValue =
            serde_json::from_str(&target.change_json).unwrap_or_else(|_| json!({}));
        let before = change.get("before").cloned().unwrap_or(JsonValue::Null);
        let after = change.get("after").cloned().unwrap_or(JsonValue::Null);
        let label_id = target
            .target_label_id
            .clone()
            .ok_or_else(|| StoreError::InvalidInput("target action has no label".to_owned()))?;
        let current = self.get_label_semantics(board, &label_id).await.ok();
        let snapshot = if before.is_object() {
            before.clone()
        } else {
            after.clone()
        };
        let new_description = snapshot
            .get("description")
            .and_then(JsonValue::as_str)
            .map(str::to_owned);
        let mut applies: Vec<String> = snapshot
            .get("applies_when")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let mut excludes: Vec<String> = snapshot
            .get("excludes_when")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let mut positive: Vec<String> = snapshot
            .get("positive_examples")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let mut negative: Vec<String> = snapshot
            .get("negative_examples")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let before_hash = current.as_ref().map(|value| value.semantics_hash.clone());
        if snapshot.is_null() {
            applies.clear();
            excludes.clear();
            positive.clear();
            negative.clear();
        }
        let now = now_ms();
        let label_name = current
            .as_ref()
            .map(|value| value.label_name.clone())
            .unwrap_or_else(|| label_id.clone());
        let description_ref = new_description.clone();
        let atoms = build_atoms(AtomBuildInput {
            label_id: &label_id,
            board_id: &board_id,
            label_name: &label_name,
            description: &description_ref,
            applies: &applies,
            excludes: &excludes,
            positive: &positive,
            negative: &negative,
            now,
        });
        let after_hash = if snapshot.is_null() {
            None
        } else {
            Some(semantics_hash(
                &label_id,
                &label_name,
                &description_ref,
                &applies,
                &excludes,
                &positive,
                &negative,
            ))
        };
        let before_snapshot = semantics_snapshot_json(
            current.as_ref(),
            &label_id,
            &label_name,
            before_hash.as_deref().unwrap_or(""),
        );
        let after_snapshot = if snapshot.is_null() {
            JsonValue::Null
        } else {
            json!({
                "label_id": label_id,
                "label_name": label_name,
                "description": description_ref,
                "applies_when": applies,
                "excludes_when": excludes,
                "positive_examples": positive,
                "negative_examples": negative,
                "semantics_hash": after_hash,
            })
        };
        let mut connection = self.connection().await?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        if snapshot.is_null() {
            tx.execute(
                "DELETE FROM label_semantics WHERE board_id=:board AND label_id=:label",
                turso::named_params! { ":board": board_id.as_str(), ":label": label_id.as_str() },
            )
            .await?;
        } else {
            tx.execute("INSERT INTO label_semantics(label_id,board_id,description,applies_when,excludes_when,positive_examples,negative_examples,created_at,updated_at) VALUES (:label,:board,:description,:applies,:excludes,:positive,:negative,COALESCE((SELECT created_at FROM label_semantics WHERE label_id=:label),:now),:now) ON CONFLICT(label_id) DO UPDATE SET description=excluded.description,applies_when=excluded.applies_when,excludes_when=excluded.excludes_when,positive_examples=excluded.positive_examples,negative_examples=excluded.negative_examples,updated_at=excluded.updated_at", turso::named_params! {":label": label_id.as_str(), ":board": board_id.as_str(), ":description": new_description.as_deref(), ":applies": serde_json::to_string(&applies).unwrap_or_else(|_|"[]".to_owned()), ":excludes": serde_json::to_string(&excludes).unwrap_or_else(|_|"[]".to_owned()), ":positive": serde_json::to_string(&positive).unwrap_or_else(|_|"[]".to_owned()), ":negative": serde_json::to_string(&negative).unwrap_or_else(|_|"[]".to_owned()), ":now": now}).await?;
        }
        tx.execute(
            "DELETE FROM label_atoms WHERE board_id=:board AND label_id=:label",
            turso::named_params! { ":board": board_id.as_str(), ":label": label_id.as_str() },
        )
        .await?;
        if !snapshot.is_null() {
            for atom in &atoms {
                tx.execute(
                    "INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at) VALUES (:id,:label,:board,:polarity,:kind,:text,:ordinal,:hash,:created,:updated)",
                    turso::named_params! {
                        ":id": atom.id.as_str(), ":label": atom.label_id.as_str(), ":board": atom.board_id.as_str(),
                        ":polarity": atom.polarity.as_str(), ":kind": atom.kind.as_str(), ":text": atom.text.as_str(),
                        ":ordinal": atom.ordinal, ":hash": atom.content_hash.as_str(), ":created": atom.created_at, ":updated": atom.updated_at,
                    },
                )
                .await?;
            }
        }
        mark_index_dirty(&tx, &board_id, now).await?;
        let old_atoms = current
            .as_ref()
            .map(|value| value.atoms.clone())
            .unwrap_or_default();
        let old_hashes = old_atoms
            .iter()
            .map(|value| value.content_hash.as_str())
            .collect::<BTreeSet<_>>();
        let new_hashes = atoms
            .iter()
            .map(|value| value.content_hash.as_str())
            .collect::<BTreeSet<_>>();
        let added_atoms = new_hashes.difference(&old_hashes).count();
        let removed_atoms = old_hashes.difference(&new_hashes).count();
        let action_id = insert_action(
            &tx,
            ActionInsertInput {
                board_id: &board_id,
                action_type: "revert_ontology_mutation",
                reason: &input.reason,
                signal_ids: &target.signal_ids,
                target_label_id: Some(&label_id),
                result_label_id: None,
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: None,
                before_hash: before_hash.as_deref(),
                after_hash: after_hash.as_deref(),
                change_json: &json!({
                    "reverted_action_id": target.id,
                    "reverted_action_type": target.action_type,
                    "label": {"id": label_id, "name": label_name},
                    "expected_current_hash": input.expected_current_hash,
                    "reverted_canonical_before_hash": target.canonical_before_hash,
                    "reverted_canonical_after_hash": target.canonical_after_hash,
                    "before_revert": before_snapshot,
                    "after_revert": after_snapshot,
                    "atom_effect_counts": {"added": added_atoms, "removed": removed_atoms},
                    "legacy_warning": JsonValue::Null,
                    "index_dirty": true,
                }),
                validation_status: "pending",
                validation_json: &json!({
                    "state": "pending_revert_validation",
                    "reverted_action_id": target.id,
                    "reverted_action_type": target.action_type,
                })
                .to_string(),
                now,
                created_by: &input.actor.name,
                agent_type: input.actor.agent_type.as_deref(),
            },
        )
        .await?;
        for atom in &old_atoms {
            if !new_hashes.contains(atom.content_hash.as_str()) {
                insert_atom_effect(&tx, &board_id, &action_id, atom, "removed", now).await?;
            }
        }
        for atom in &atoms {
            if !old_hashes.contains(atom.content_hash.as_str()) {
                insert_atom_effect(&tx, &board_id, &action_id, atom, "added", now).await?;
            }
        }
        tx.commit().await?;
        self.action_by_id(&board_id, &action_id).await
    }

    pub async fn validate_label_ontology_action(
        &self,
        board: &str,
        input: OntologyValidateInput,
    ) -> Result<LabelOntologyActionRecord, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let parent = self
            .action_by_id(&board_id, &input.parent_action_id)
            .await?;
        let now = now_ms();
        let mut connection = self.connection().await?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        tx.execute("UPDATE label_ontology_actions SET validation_status=:status,validation_json=:validation,validation_requirement=CASE WHEN :status='not_required' THEN 'none' ELSE 'required' END WHERE id=:id AND board_id=:board",[(":status",input.validation_status.as_str()),(":validation",json_object_or_default(&input.validation_json)?.to_string().as_str()),(":id",input.parent_action_id.as_str()),(":board",board_id.as_str())]).await?;
        let action_id = insert_action(
            &tx,
            ActionInsertInput {
                board_id: &board_id,
                action_type: "validate",
                reason: &input.reason,
                signal_ids: &input.signal_ids,
                target_label_id: parent.target_label_id.as_deref(),
                result_label_id: None,
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: None,
                before_hash: None,
                after_hash: None,
                change_json: &json!({"parent_action_id":parent.id,"validation_status":input.validation_status}),
                validation_status: &input.validation_status,
                validation_json: &input.validation_json,
                now,
                created_by: &input.actor.name,
                agent_type: input.actor.agent_type.as_deref(),
            },
        )
        .await?;
        tx.commit().await?;
        self.action_by_id(&board_id, &action_id).await
    }

    pub async fn label_ontology_quality(
        &self,
        board: &str,
        sample_limit: usize,
    ) -> Result<LabelOntologyQualityRecord, StoreError> {
        let board_id = self.ontology_board_id(board).await?;
        let connection = self.connection().await?;
        let row=first_row(connection.query("SELECT COUNT(*),COUNT(DISTINCT task_id),COALESCE(SUM(CASE WHEN suggest_degraded=0 THEN 1 ELSE 0 END),0),MIN(created_at),MAX(created_at) FROM label_ontology_observations WHERE board_id=:board",[(":board",board_id.as_str())]).await?).await?;
        let observation_count = integer_value(row.get_value(0)?, "observations.count")?;
        let task_count = integer_value(row.get_value(1)?, "observations.tasks")?;
        let degraded = integer_value(row.get_value(2)?, "observations.agreement")?;
        let first = optional_integer_value(row.get_value(3)?, "observations.first")?;
        let latest = optional_integer_value(row.get_value(4)?, "observations.latest")?;
        let mut rows=connection.query("SELECT task_ref_snapshot FROM label_ontology_observations WHERE board_id=:board ORDER BY created_at DESC LIMIT :limit",[(":board",board_id.as_str()),(":limit",sample_limit.clamp(1,MAX_LIST_LIMIT as usize).to_string().as_str())]).await?;
        let mut refs = Vec::new();
        while let Some(row) = rows.next().await? {
            refs.push(text_value(
                row.get_value(0)?,
                "observations.task_ref_snapshot",
            )?);
        }
        let mut by_kind = BTreeMap::new();
        let mut signal_rows=connection.query("SELECT kind,COUNT(*) FROM label_ontology_signals WHERE board_id=:board GROUP BY kind",[(":board",board_id.as_str())]).await?;
        while let Some(row) = signal_rows.next().await? {
            by_kind.insert(
                text_value(row.get_value(0)?, "signals.kind")?,
                integer_value(row.get_value(1)?, "signals.count")?,
            );
        }
        Ok(LabelOntologyQualityRecord{board_id,denominator_json:json!({"source":"label_ontology_observations","description":"captured observations","observation_count":observation_count,"distinct_task_count":task_count,"agreement_observation_count":degraded,"agreement_task_count":degraded,"degraded_observation_count":observation_count-degraded,"first_observed_at":first,"latest_observed_at":latest,"sample_task_refs":refs}).to_string(),disagreement_json:json!({"signal_count":by_kind.values().sum::<i64>(),"distinct_task_count":0,"by_kind":by_kind,"by_status":{}}).to_string(),rates_json:json!({"disagreement_task_rate":null,"disagreement_task_rate_basis":"not_available_without_ground_truth"}).to_string(),precision_recall_json:json!({"available":false,"reason":"ground truth is not stored"}).to_string(),warnings_json:json!(["vector provider unavailable diagnostics do not affect canonical observation counts"]).to_string()})
    }

    async fn observation_by_id(
        &self,
        board_id: &str,
        observation_id: &str,
    ) -> Result<LabelOntologyObservationRecord, StoreError> {
        let connection = self.connection().await?;
        let row=first_row(connection.query("SELECT id,board_id,task_id,task_ref_snapshot,task_snapshot_json,suggest_input_hash,agent_candidates_json,suggestion_snapshot_json,final_decision_json,suggest_coverage,suggest_coverage_cosine,suggest_residual_norm,suggest_needs_new_label,suggest_degraded,diagnostics_json,capture_fingerprint,created_by,created_by_type,agent_type,created_at FROM label_ontology_observations WHERE board_id=:board AND id=:id LIMIT 1",[(":board",board_id),(":id",observation_id)]).await?).await.map_err(|error|match error{turso::Error::QueryReturnedNoRows=>StoreError::InvalidInput(format!("ontology observation not found: {observation_id}")),other=>StoreError::Turso(other)})?;
        let observation = observation_from_row(row)?;
        let mut rows=connection.query("SELECT id,observation_id,board_id,kind,status,target_label_id,target_label_name_snapshot,related_labels_json,proposed_action,candidate_atom_polarity,candidate_atom_kind,candidate_text,candidate_content_hash,proposed_label_name,proposed_label_name_normalized,proposal_json,agent_selected,suggest_state,suggest_score,suggest_rank,final_selected,rationale,confidence,signal_key,superseded_by_signal_id,status_reason,created_at,updated_at,reviewed_at,closed_at FROM label_ontology_signals WHERE board_id=:board AND observation_id=:observation ORDER BY created_at ASC,id ASC",[(":board",board_id),(":observation",observation_id)]).await?;
        let mut signals = Vec::new();
        while let Some(row) = rows.next().await? {
            signals.push(signal_from_row(row)?);
        }
        Ok(LabelOntologyObservationRecord {
            signals,
            ..observation
        })
    }

    async fn action_by_id(
        &self,
        board_id: &str,
        action_id: &str,
    ) -> Result<LabelOntologyActionRecord, StoreError> {
        let connection = self.connection().await?;
        let row=first_row(connection.query("SELECT id,board_id,parent_action_id,action_type,reason,target_label_id,result_label_id,result_atom_id,result_atom_content_hash,result_proposal_id,canonical_before_hash,canonical_after_hash,change_json,validation_requirement,validation_status,validation_json,created_by,created_by_type,agent_type,created_at FROM label_ontology_actions WHERE board_id=:board AND id=:id LIMIT 1",[(":board",board_id),(":id",action_id)]).await?).await.map_err(|error|match error{turso::Error::QueryReturnedNoRows=>StoreError::InvalidInput(format!("ontology action not found: {action_id}")),other=>StoreError::Turso(other)})?;
        self.action_from_row(&connection, row).await
    }

    async fn action_from_row(
        &self,
        connection: &Connection,
        row: Row,
    ) -> Result<LabelOntologyActionRecord, StoreError> {
        let id = text_value(row.get_value(0)?, "actions.id")?;
        let board_id = text_value(row.get_value(1)?, "actions.board_id")?;
        let mut links=connection.query("SELECT signal_id FROM label_ontology_action_signals WHERE board_id=:board AND action_id=:action ORDER BY signal_id",[(":board",board_id.as_str()),(":action",id.as_str())]).await?;
        let mut signal_ids = Vec::new();
        while let Some(row) = links.next().await? {
            signal_ids.push(text_value(row.get_value(0)?, "action_signals.signal_id")?);
        }
        Ok(LabelOntologyActionRecord {
            id,
            board_id,
            parent_action_id: optional_text_value(row.get_value(2)?, "actions.parent_action_id")?,
            action_type: text_value(row.get_value(3)?, "actions.action_type")?,
            reason: text_value(row.get_value(4)?, "actions.reason")?,
            target_label_id: optional_text_value(row.get_value(5)?, "actions.target_label_id")?,
            result_label_id: optional_text_value(row.get_value(6)?, "actions.result_label_id")?,
            result_atom_id: optional_text_value(row.get_value(7)?, "actions.result_atom_id")?,
            result_atom_content_hash: optional_text_value(
                row.get_value(8)?,
                "actions.result_atom_content_hash",
            )?,
            result_proposal_id: optional_text_value(
                row.get_value(9)?,
                "actions.result_proposal_id",
            )?,
            canonical_before_hash: optional_text_value(
                row.get_value(10)?,
                "actions.canonical_before_hash",
            )?,
            canonical_after_hash: optional_text_value(
                row.get_value(11)?,
                "actions.canonical_after_hash",
            )?,
            change_json: text_value(row.get_value(12)?, "actions.change_json")?,
            validation_requirement: text_value(
                row.get_value(13)?,
                "actions.validation_requirement",
            )?,
            validation_status: text_value(row.get_value(14)?, "actions.validation_status")?,
            validation_json: text_value(row.get_value(15)?, "actions.validation_json")?,
            created_by: text_value(row.get_value(16)?, "actions.created_by")?,
            created_by_type: text_value(row.get_value(17)?, "actions.created_by_type")?,
            agent_type: optional_text_value(row.get_value(18)?, "actions.agent_type")?,
            created_at: integer_value(row.get_value(19)?, "actions.created_at")?,
            signal_ids,
        })
    }
}

fn atom_from_row(row: Row) -> Result<LabelAtomRecord, StoreError> {
    Ok(LabelAtomRecord {
        id: text_value(row.get_value(0)?, "label_atoms.id")?,
        label_id: text_value(row.get_value(1)?, "label_atoms.label_id")?,
        board_id: text_value(row.get_value(2)?, "label_atoms.board_id")?,
        label_name: text_value(row.get_value(3)?, "labels.name")?,
        polarity: text_value(row.get_value(4)?, "label_atoms.polarity")?,
        kind: text_value(row.get_value(5)?, "label_atoms.kind")?,
        text: text_value(row.get_value(6)?, "label_atoms.text")?,
        ordinal: integer_value(row.get_value(7)?, "label_atoms.ordinal")?,
        content_hash: text_value(row.get_value(8)?, "label_atoms.content_hash")?,
        created_at: integer_value(row.get_value(9)?, "label_atoms.created_at")?,
        updated_at: integer_value(row.get_value(10)?, "label_atoms.updated_at")?,
    })
}

fn proposal_from_row(row: Row) -> Result<LabelSemanticProposalRecord, StoreError> {
    Ok(LabelSemanticProposalRecord {
        id: text_value(row.get_value(0)?, "proposals.id")?,
        board_id: text_value(row.get_value(1)?, "proposals.board_id")?,
        task_id: text_value(row.get_value(2)?, "proposals.task_id")?,
        status: text_value(row.get_value(3)?, "proposals.status")?,
        name: text_value(row.get_value(4)?, "proposals.name")?,
        description: optional_text_value(row.get_value(5)?, "proposals.description")?,
        applies_when: json_strings(row.get_value(6)?, "proposals.applies_when")?,
        excludes_when: json_strings(row.get_value(7)?, "proposals.excludes_when")?,
        positive_examples: json_strings(row.get_value(8)?, "proposals.positive_examples")?,
        negative_examples: json_strings(row.get_value(9)?, "proposals.negative_examples")?,
        heuristic_coverage: number_value(row.get_value(10)?, "proposals.coverage")? as f32,
        heuristic_residual_norm: number_value(row.get_value(11)?, "proposals.residual")? as f32,
        heuristic_coverage_cosine: number_value(row.get_value(12)?, "proposals.cosine")? as f32,
        top1_existing_label_id: optional_text_value(row.get_value(13)?, "proposals.top1_id")?,
        top1_existing_label_name: optional_text_value(row.get_value(14)?, "proposals.top1_name")?,
        diagnostics: json_strings(row.get_value(15)?, "proposals.diagnostics")?,
        created_by: text_value(row.get_value(16)?, "proposals.created_by")?,
        decision_reason: optional_text_value(row.get_value(17)?, "proposals.reason")?,
        resolved_label_id: optional_text_value(row.get_value(18)?, "proposals.resolved_label_id")?,
        created_at: integer_value(row.get_value(19)?, "proposals.created_at")?,
        updated_at: integer_value(row.get_value(20)?, "proposals.updated_at")?,
        decided_at: optional_integer_value(row.get_value(21)?, "proposals.decided_at")?,
    })
}

fn observation_from_row(row: Row) -> Result<LabelOntologyObservationRecord, StoreError> {
    Ok(LabelOntologyObservationRecord {
        id: text_value(row.get_value(0)?, "observations.id")?,
        board_id: text_value(row.get_value(1)?, "observations.board_id")?,
        task_id: text_value(row.get_value(2)?, "observations.task_id")?,
        task_ref_snapshot: text_value(row.get_value(3)?, "observations.task_ref_snapshot")?,
        task_snapshot_json: text_value(row.get_value(4)?, "observations.task_snapshot_json")?,
        suggest_input_hash: optional_text_value(
            row.get_value(5)?,
            "observations.suggest_input_hash",
        )?,
        agent_candidates_json: text_value(row.get_value(6)?, "observations.agent_candidates_json")?,
        suggestion_snapshot_json: text_value(
            row.get_value(7)?,
            "observations.suggestion_snapshot_json",
        )?,
        final_decision_json: text_value(row.get_value(8)?, "observations.final_decision_json")?,
        suggest_coverage: number_optional(row.get_value(9)?, "observations.coverage")?,
        suggest_coverage_cosine: number_optional(row.get_value(10)?, "observations.cosine")?,
        suggest_residual_norm: number_optional(row.get_value(11)?, "observations.residual")?,
        suggest_needs_new_label: integer_value(row.get_value(12)?, "observations.needs")? != 0,
        suggest_degraded: integer_value(row.get_value(13)?, "observations.degraded")? != 0,
        diagnostics_json: text_value(row.get_value(14)?, "observations.diagnostics")?,
        capture_fingerprint: text_value(row.get_value(15)?, "observations.fingerprint")?,
        created_by: text_value(row.get_value(16)?, "observations.created_by")?,
        created_by_type: text_value(row.get_value(17)?, "observations.created_by_type")?,
        agent_type: optional_text_value(row.get_value(18)?, "observations.agent_type")?,
        created_at: integer_value(row.get_value(19)?, "observations.created_at")?,
        signals: Vec::new(),
    })
}

fn signal_from_row(row: Row) -> Result<LabelOntologySignalRecord, StoreError> {
    Ok(LabelOntologySignalRecord {
        id: text_value(row.get_value(0)?, "signals.id")?,
        observation_id: text_value(row.get_value(1)?, "signals.observation_id")?,
        board_id: text_value(row.get_value(2)?, "signals.board_id")?,
        kind: text_value(row.get_value(3)?, "signals.kind")?,
        status: text_value(row.get_value(4)?, "signals.status")?,
        target_label_id: optional_text_value(row.get_value(5)?, "signals.target_label_id")?,
        target_label_name_snapshot: optional_text_value(
            row.get_value(6)?,
            "signals.target_label_name_snapshot",
        )?,
        related_labels_json: text_value(row.get_value(7)?, "signals.related_labels_json")?,
        proposed_action: text_value(row.get_value(8)?, "signals.proposed_action")?,
        candidate_atom_polarity: optional_text_value(
            row.get_value(9)?,
            "signals.candidate_atom_polarity",
        )?,
        candidate_atom_kind: optional_text_value(
            row.get_value(10)?,
            "signals.candidate_atom_kind",
        )?,
        candidate_text: optional_text_value(row.get_value(11)?, "signals.candidate_text")?,
        candidate_content_hash: optional_text_value(
            row.get_value(12)?,
            "signals.candidate_content_hash",
        )?,
        proposed_label_name: optional_text_value(
            row.get_value(13)?,
            "signals.proposed_label_name",
        )?,
        proposed_label_name_normalized: optional_text_value(
            row.get_value(14)?,
            "signals.proposed_label_name_normalized",
        )?,
        proposal_json: text_value(row.get_value(15)?, "signals.proposal_json")?,
        agent_selected: integer_value(row.get_value(16)?, "signals.agent_selected")? != 0,
        suggest_state: optional_text_value(row.get_value(17)?, "signals.suggest_state")?,
        suggest_score: number_optional(row.get_value(18)?, "signals.score")?,
        suggest_rank: optional_integer_value(row.get_value(19)?, "signals.rank")?,
        final_selected: integer_value(row.get_value(20)?, "signals.final_selected")? != 0,
        rationale: text_value(row.get_value(21)?, "signals.rationale")?,
        confidence: number_optional(row.get_value(22)?, "signals.confidence")?,
        signal_key: text_value(row.get_value(23)?, "signals.signal_key")?,
        superseded_by_signal_id: optional_text_value(row.get_value(24)?, "signals.superseded")?,
        status_reason: optional_text_value(row.get_value(25)?, "signals.status_reason")?,
        created_at: integer_value(row.get_value(26)?, "signals.created_at")?,
        updated_at: integer_value(row.get_value(27)?, "signals.updated_at")?,
        reviewed_at: optional_integer_value(row.get_value(28)?, "signals.reviewed_at")?,
        closed_at: optional_integer_value(row.get_value(29)?, "signals.closed_at")?,
    })
}

fn number_value(value: Value, field: &'static str) -> Result<f64, StoreError> {
    match value {
        Value::Integer(value) => Ok(value as f64),
        Value::Real(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}
fn number_optional(value: Value, field: &'static str) -> Result<Option<f64>, StoreError> {
    match value {
        Value::Null => Ok(None),
        Value::Integer(value) => Ok(Some(value as f64)),
        Value::Real(value) => Ok(Some(value)),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}
fn json_strings(value: Value, field: &'static str) -> Result<Vec<String>, StoreError> {
    let text = text_value(value, field)?;
    serde_json::from_str::<Vec<String>>(&text).map_err(|_| StoreError::InvalidStoredValue { field })
}
fn json_array_or_default(raw: &str) -> Result<JsonValue, StoreError> {
    let value = if raw.trim().is_empty() {
        json!([])
    } else {
        serde_json::from_str(raw)
            .map_err(|_| StoreError::InvalidInput("expected JSON array".to_owned()))?
    };
    if !value.is_array() {
        return Err(StoreError::InvalidInput("expected JSON array".to_owned()));
    }
    Ok(value)
}
fn json_object_or_default(raw: &str) -> Result<JsonValue, StoreError> {
    let value = if raw.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(raw)
            .map_err(|_| StoreError::InvalidInput("expected JSON object".to_owned()))?
    };
    if !value.is_object() {
        return Err(StoreError::InvalidInput("expected JSON object".to_owned()));
    }
    Ok(value)
}
fn json_string_or_object(raw: &str) -> Result<JsonValue, StoreError> {
    json_object_or_default(raw)
}
fn normalize_optional(value: String) -> Option<String> {
    let value = normalize_text(&value);
    (!value.is_empty()).then_some(value)
}
fn normalize_text(value: &str) -> String {
    value
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
fn normalize_list(values: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        let value = normalize_text(&value);
        if !value.is_empty() && !result.contains(&value) {
            result.push(value);
        }
    }
    result
}
fn remove_items(values: &mut Vec<String>, remove: &[String]) {
    let remove = remove
        .iter()
        .map(|v| normalize_text(v))
        .collect::<BTreeSet<_>>();
    values.retain(|v| !remove.contains(v));
}
fn append_items(values: &mut Vec<String>, items: Vec<String>) {
    for value in normalize_list(items) {
        if !values.contains(&value) {
            values.push(value);
        }
    }
}
fn fnv_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
fn semantics_hash(
    label_id: &str,
    label_name: &str,
    description: &Option<String>,
    applies: &[String],
    excludes: &[String],
    positive: &[String],
    negative: &[String],
) -> String {
    fnv_hash(&json!({"label_id":label_id,"label_name":label_name,"description":description,"applies_when":applies,"excludes_when":excludes,"positive_examples":positive,"negative_examples":negative}).to_string())
}
fn semantics_snapshot_json(
    current: Option<&LabelSemanticsRecord>,
    label_id: &str,
    label_name: &str,
    hash: &str,
) -> JsonValue {
    current.map(|v|json!({"label_id":v.label_id,"label_name":v.label_name,"description":v.description,"applies_when":v.applies_when,"excludes_when":v.excludes_when,"positive_examples":v.positive_examples,"negative_examples":v.negative_examples,"semantics_hash":hash})).unwrap_or_else(||json!({"label_id":label_id,"label_name":label_name,"description":null,"applies_when":[],"excludes_when":[],"positive_examples":[],"negative_examples":[],"semantics_hash":hash}))
}
struct AtomBuildInput<'a> {
    label_id: &'a str,
    board_id: &'a str,
    label_name: &'a str,
    description: &'a Option<String>,
    applies: &'a [String],
    excludes: &'a [String],
    positive: &'a [String],
    negative: &'a [String],
    now: i64,
}

fn build_atoms(input: AtomBuildInput<'_>) -> Vec<LabelAtomRecord> {
    let AtomBuildInput {
        label_id,
        board_id,
        label_name,
        description,
        applies,
        excludes,
        positive,
        negative,
        now,
    } = input;
    let mut atoms = Vec::new();
    let mut ordinal = 0_i64;
    let mut add = |polarity: &str, kind: &str, text: &str| {
        let text = normalize_text(text);
        if text.is_empty() {
            return;
        }
        let hash = fnv_hash(&format!("{label_id}\n{polarity}\n{kind}\n{text}"));
        if atoms
            .iter()
            .any(|value: &LabelAtomRecord| value.content_hash == hash)
        {
            return;
        }
        atoms.push(LabelAtomRecord {
            id: format!("la_{hash}"),
            label_id: label_id.to_owned(),
            board_id: board_id.to_owned(),
            label_name: label_name.to_owned(),
            polarity: polarity.to_owned(),
            kind: kind.to_owned(),
            text,
            ordinal,
            content_hash: hash,
            created_at: now,
            updated_at: now,
        });
        ordinal += 1;
    };
    add("positive", "name", label_name);
    if let Some(value) = description {
        add("positive", "description", value);
    }
    for value in applies {
        add("positive", "applies_when", value);
    }
    for value in positive {
        add("positive", "positive_example", value);
    }
    for value in excludes {
        add("negative", "excludes_when", value);
    }
    for value in negative {
        add("negative", "negative_example", value);
    }
    atoms
}
fn token_overlap(query: &str, atom: &str) -> f32 {
    let query = query.split_whitespace().collect::<BTreeSet<_>>();
    let atom = atom.split_whitespace().collect::<BTreeSet<_>>();
    if query.is_empty() || atom.is_empty() {
        return 0.0;
    }
    query.intersection(&atom).count() as f32 / atom.len().max(1) as f32
}
fn normalize_label_name(value: &str) -> String {
    value.trim().to_lowercase()
}
fn validate_actor(actor: &OntologyActorInput) -> Result<(), StoreError> {
    if actor.name.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "actor.name is required".to_owned(),
        ));
    }
    if !matches!(actor.actor_type.as_str(), "user" | "agent") {
        return Err(StoreError::InvalidInput(
            "actor.type must be user or agent".to_owned(),
        ));
    }
    Ok(())
}
fn validate_atom_kind(polarity: &str, kind: &str, text: &str) -> Result<(), StoreError> {
    if text.trim().is_empty() {
        return Err(StoreError::InvalidInput("atom text is required".to_owned()));
    }
    if !matches!(
        (polarity, kind),
        ("positive", "applies_when" | "positive_example")
            | ("negative", "excludes_when" | "negative_example")
    ) {
        return Err(StoreError::InvalidInput(
            "invalid atom polarity/kind".to_owned(),
        ));
    }
    Ok(())
}
fn review_group(
    group_by: &str,
    key: String,
    signals: Vec<LabelOntologySignalRecord>,
) -> LabelOntologyReviewGroupRecord {
    let mut status = BTreeMap::<String, i64>::new();
    let task_refs = BTreeSet::new();
    let mut ids = Vec::new();
    let mut labels = BTreeSet::new();
    let mut proposals = BTreeSet::new();
    let mut oldest = i64::MAX;
    let mut latest = 0_i64;
    let mut score_sum = 0.0;
    let mut score_count = 0_i64;
    for signal in &signals {
        *status.entry(signal.status.clone()).or_default() += 1;
        ids.push(signal.id.clone());
        if let Some(name) = &signal.proposed_label_name_normalized {
            proposals.insert(name.clone());
        }
        if let Some(label) = &signal.target_label_id {
            labels.insert(label.clone());
        }
        oldest = oldest.min(signal.created_at);
        latest = latest.max(signal.created_at);
        if let Some(score) = signal.suggest_score {
            score_sum += score;
            score_count += 1;
        }
    }
    LabelOntologyReviewGroupRecord {
        group_by: group_by.to_owned(),
        key,
        label_id: signals.first().and_then(|v| v.target_label_id.clone()),
        label_name: signals
            .first()
            .and_then(|v| v.target_label_name_snapshot.clone()),
        candidate_atom_polarity: signals
            .first()
            .and_then(|v| v.candidate_atom_polarity.clone()),
        candidate_atom_kind: signals.first().and_then(|v| v.candidate_atom_kind.clone()),
        candidate_text: signals.first().and_then(|v| v.candidate_text.clone()),
        candidate_content_hash: signals
            .first()
            .and_then(|v| v.candidate_content_hash.clone()),
        proposed_label_name: signals.first().and_then(|v| v.proposed_label_name.clone()),
        proposed_label_name_normalized: signals
            .first()
            .and_then(|v| v.proposed_label_name_normalized.clone()),
        cluster_key: None,
        cluster_reason: None,
        task_count: task_refs.len() as i64,
        signal_count: signals.len() as i64,
        open_count: *status.get("open").unwrap_or(&0),
        confirmed_count: *status.get("confirmed").unwrap_or(&0),
        resolved_count: *status.get("resolved").unwrap_or(&0),
        rejected_count: *status.get("rejected").unwrap_or(&0),
        superseded_count: *status.get("superseded").unwrap_or(&0),
        degraded_count: 0,
        average_score: (score_count > 0).then_some(score_sum / score_count as f64),
        median_score: None,
        oldest_signal_at: if oldest == i64::MAX { 0 } else { oldest },
        latest_signal_at: latest,
        sample_task_refs: task_refs.into_iter().take(20).collect(),
        signal_ids: ids,
        action_count: 0,
        action_ids: Vec::new(),
        proposal_ids: proposals.into_iter().collect(),
        labels_json: serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_owned()),
        candidate_atom_variants_json: "[]".to_owned(),
    }
}

async fn mark_index_dirty(
    transaction: &turso::transaction::Transaction<'_>,
    board_id: &str,
    now: i64,
) -> Result<(), StoreError> {
    transaction.execute("INSERT INTO label_atom_index_boards(store_name,board_id,dirty,last_rebuild_at,last_error,updated_at) VALUES (:store,:board,1,NULL,NULL,:now) ON CONFLICT(store_name,board_id) DO UPDATE SET dirty=1,updated_at=excluded.updated_at", turso::named_params! { ":store": LABEL_ATOM_INDEX_STORE, ":board": board_id, ":now": now }).await?;
    Ok(())
}

struct ActionInsertInput<'a> {
    board_id: &'a str,
    action_type: &'a str,
    reason: &'a str,
    signal_ids: &'a [String],
    target_label_id: Option<&'a str>,
    result_label_id: Option<&'a str>,
    result_atom_id: Option<&'a str>,
    result_atom_content_hash: Option<&'a str>,
    result_proposal_id: Option<&'a str>,
    before_hash: Option<&'a str>,
    after_hash: Option<&'a str>,
    change_json: &'a JsonValue,
    validation_status: &'a str,
    validation_json: &'a str,
    now: i64,
    created_by: &'a str,
    agent_type: Option<&'a str>,
}

async fn insert_action(
    transaction: &turso::transaction::Transaction<'_>,
    input: ActionInsertInput<'_>,
) -> Result<String, StoreError> {
    let ActionInsertInput {
        board_id,
        action_type,
        reason,
        signal_ids,
        target_label_id,
        result_label_id,
        result_atom_id,
        result_atom_content_hash,
        result_proposal_id,
        before_hash,
        after_hash,
        change_json,
        validation_status,
        validation_json,
        now,
        created_by,
        agent_type,
    } = input;
    let action_id = format!("loa_{}", ulid::Ulid::new());
    let actor_type = if agent_type.is_some() {
        "agent"
    } else {
        "user"
    };
    transaction.execute("INSERT INTO label_ontology_actions(id,board_id,action_type,reason,target_label_id,result_label_id,result_atom_id,result_atom_content_hash,result_proposal_id,canonical_before_hash,canonical_after_hash,change_json,validation_status,validation_json,validation_requirement,created_by,created_by_type,agent_type,created_at) VALUES (:id,:board,:type,:reason,:target,:result_label,:atom,:atom_hash,:proposal,:before,:after,:change,:status,:validation,CASE WHEN :status='not_required' THEN 'none' ELSE 'required' END,:actor,:actor_type,:agent_type,:now)", turso::named_params! { ":id": action_id.as_str(), ":board": board_id, ":type": action_type, ":reason": reason.trim(), ":target": target_label_id, ":result_label": result_label_id, ":atom": result_atom_id, ":atom_hash": result_atom_content_hash, ":proposal": result_proposal_id, ":before": before_hash, ":after": after_hash, ":change": change_json.to_string().as_str(), ":status": validation_status, ":validation": validation_json, ":actor": created_by, ":actor_type": actor_type, ":agent_type": agent_type, ":now": now }).await?;
    for signal_id in signal_ids {
        transaction.execute("INSERT INTO label_ontology_action_signals(board_id,action_id,signal_id,created_at) SELECT :board,:action,id,:now FROM label_ontology_signals WHERE board_id=:board AND id=:signal", turso::named_params! { ":board": board_id, ":action": action_id.as_str(), ":signal": signal_id.as_str(), ":now": now }).await?;
    }
    Ok(action_id)
}

async fn insert_atom_effect(
    transaction: &turso::transaction::Transaction<'_>,
    board_id: &str,
    action_id: &str,
    atom: &LabelAtomRecord,
    effect: &str,
    now: i64,
) -> Result<(), StoreError> {
    transaction.execute("INSERT OR IGNORE INTO label_ontology_action_atom_effects(board_id,action_id,label_id_snapshot,atom_id_snapshot,atom_content_hash,polarity,kind,text,effect,created_at) VALUES (:board,:action,:label,:atom,:hash,:polarity,:kind,:text,:effect,:now)", turso::named_params! { ":board": board_id, ":action": action_id, ":label": atom.label_id.as_str(), ":atom": atom.id.as_str(), ":hash": atom.content_hash.as_str(), ":polarity": atom.polarity.as_str(), ":kind": atom.kind.as_str(), ":text": atom.text.as_str(), ":effect": effect, ":now": now }).await?;
    Ok(())
}

async fn update_signal_status(
    transaction: &turso::transaction::Transaction<'_>,
    board_id: &str,
    signal_ids: &[String],
    action_type: &str,
    superseded_by: Option<&str>,
    reason: &str,
    now: i64,
) -> Result<(), StoreError> {
    let status = match action_type {
        "confirm" => "confirmed",
        "reject" => "rejected",
        "resolve_no_change" => "resolved",
        "supersede" => "superseded",
        _ => return Ok(()),
    };
    for signal_id in signal_ids {
        transaction.execute("UPDATE label_ontology_signals SET status=:status,status_reason=:reason,superseded_by_signal_id=COALESCE(:superseded,superseded_by_signal_id),reviewed_at=:now,closed_at=CASE WHEN :status IN ('resolved','rejected','superseded') THEN :now ELSE closed_at END,updated_at=:now WHERE board_id=:board AND id=:id", turso::named_params! { ":status": status, ":reason": reason, ":superseded": superseded_by, ":now": now, ":board": board_id, ":id": signal_id.as_str() }).await?;
    }
    Ok(())
}
