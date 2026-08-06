use turso::transaction::TransactionBehavior;
use turso::{Connection, Row, transaction::Transaction};

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};
use serde_json::json;

use super::ontology::{
    ActionInsertInput, AtomBuildInput, build_atoms, insert_action, insert_atom_effect,
    mark_index_dirty, semantics_hash, semantics_snapshot_json,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateLabelInput {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTaskLabelsInput {
    pub names: Vec<String>,
    pub label_ids: Vec<String>,
    pub event_ids: Vec<String>,
    pub create_missing: bool,
    pub actor: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTaskLabelsRecord {
    pub task: TaskRecord,
    pub created_labels: Vec<LabelRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveTaskLabelInput {
    pub label_ref: String,
    pub event_id: String,
    pub actor: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeleteBoardLabelInput {
    pub label_ref: String,
    pub event_id: String,
    pub actor: String,
    pub force: bool,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeleteBoardLabelRecord {
    pub label: LabelRecord,
    pub forced: bool,
    pub removed_task_bindings: i64,
    pub removed_semantics: bool,
    pub removed_atoms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BootstrapTaskLabelInput {
    pub label_id: String,
    pub event_id: String,
    pub name: String,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
    pub actor: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BootstrapTaskLabelStoreRecord {
    pub task: TaskRecord,
    pub semantics: LabelSemanticsRecord,
}

impl TursoStore {
    /// 在一个立即事务中创建/复用 label、写入首个 semantics、记录 ontology action，
    /// 并将 label 幂等地挂到任务上。
    pub(crate) async fn bootstrap_task_label(
        &self,
        task_id: &str,
        input: BootstrapTaskLabelInput,
    ) -> Result<BootstrapTaskLabelStoreRecord, StoreError> {
        validate_task_id(task_id)?;
        validate_bootstrap_task_label_input(&input)?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board_id = active_task_board_id_in_transaction(&transaction, task_id).await?;
        let name = input.name.trim();
        let label = match label_by_name_in_transaction(&transaction, &board_id, name).await? {
            Some(label) => {
                let row = first_row(
                    transaction
                        .query(
                            "SELECT COUNT(*) FROM label_semantics WHERE board_id=:board AND label_id=:label",
                            [
                                (":board", board_id.as_str()),
                                (":label", label.id.as_str()),
                            ],
                        )
                        .await?,
                )
                .await?;
                if integer_value(row.get_value(0)?, "label semantics count")? > 0 {
                    return Err(StoreError::InvalidInput(format!(
                        "label bootstrap would replace existing semantics for label {}; use a dedicated semantics mutation or proposal adoption path",
                        label.name
                    )));
                }
                label
            }
            None => {
                transaction
                    .execute(
                        "INSERT INTO labels(id, board_id, name, color, created_at, updated_at) VALUES (:id, :board, :name, NULL, :now, :now)",
                        turso::named_params! {
                            ":id": input.label_id.as_str(),
                            ":board": board_id.as_str(),
                            ":name": name,
                            ":now": input.now,
                        },
                    )
                    .await?;
                label_by_id_in_transaction(&transaction, &board_id, &input.label_id)
                    .await?
                    .ok_or_else(|| StoreError::LabelNotFound(input.label_id.clone()))?
            }
        };
        let description = input
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let applies_when = normalize_bootstrap_list(&input.applies_when);
        let excludes_when = normalize_bootstrap_list(&input.excludes_when);
        let positive_examples = normalize_bootstrap_list(&input.positive_examples);
        let negative_examples = normalize_bootstrap_list(&input.negative_examples);
        let atoms = build_atoms(AtomBuildInput {
            label_id: &label.id,
            board_id: &board_id,
            label_name: &label.name,
            description: &description,
            applies: &applies_when,
            excludes: &excludes_when,
            positive: &positive_examples,
            negative: &negative_examples,
            now: input.now,
        });
        let after_hash = semantics_hash(
            &label.id,
            &label.name,
            &description,
            &applies_when,
            &excludes_when,
            &positive_examples,
            &negative_examples,
        );
        transaction
            .execute(
                "INSERT INTO label_semantics(label_id,board_id,description,applies_when,excludes_when,positive_examples,negative_examples,created_at,updated_at) VALUES (:label,:board,:description,:applies,:excludes,:positive,:negative,:now,:now)",
                turso::named_params! {
                    ":label": label.id.as_str(),
                    ":board": board_id.as_str(),
                    ":description": description.as_deref(),
                    ":applies": serde_json::to_string(&applies_when).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":excludes": serde_json::to_string(&excludes_when).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":positive": serde_json::to_string(&positive_examples).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":negative": serde_json::to_string(&negative_examples).unwrap_or_else(|_| "[]".to_owned()).as_str(),
                    ":now": input.now,
                },
            )
            .await?;
        for atom in &atoms {
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
        mark_index_dirty(&transaction, &board_id, input.now).await?;
        let action_id = insert_action(
            &transaction,
            ActionInsertInput {
                board_id: &board_id,
                action_type: "bootstrap_label",
                reason: "direct label bootstrap",
                signal_ids: &[],
                target_label_id: None,
                result_label_id: Some(&label.id),
                result_atom_id: None,
                result_atom_content_hash: None,
                result_proposal_id: None,
                before_hash: None,
                after_hash: Some(&after_hash),
                change_json: &json!({
                    "before": semantics_snapshot_json(None, &label.id, &label.name, ""),
                    "after": {
                        "label_id": label.id,
                        "label_name": label.name,
                        "description": description,
                        "applies_when": applies_when,
                        "excludes_when": excludes_when,
                        "positive_examples": positive_examples,
                        "negative_examples": negative_examples,
                        "semantics_hash": after_hash,
                    },
                }),
                validation_status: "not_required",
                validation_json: "{}",
                now: input.now,
                created_by: &input.actor,
                agent_type: None,
            },
        )
        .await?;
        for atom in &atoms {
            insert_atom_effect(
                &transaction,
                &board_id,
                &action_id,
                atom,
                "added",
                input.now,
            )
            .await?;
        }
        transaction
            .execute(
                "INSERT INTO task_events(event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at) VALUES (:event,:board,:task,NULL,'label.semantics.updated',:actor,:payload,:now)",
                turso::named_params! {
                    ":event": format!("e_ontology_semantics_{}", action_id.trim_start_matches("loa_")),
                    ":board": board_id.as_str(),
                    ":task": Option::<&str>::None,
                    ":actor": input.actor.as_str(),
                    ":payload": json!({"label_id": label.id, "action_id": action_id, "semantics_hash": after_hash}).to_string(),
                    ":now": input.now,
                },
            )
            .await?;
        let changed = transaction
            .execute(
                "INSERT INTO task_labels(board_id,task_id,label_id,created_at) VALUES (:board,:task,:label,:now) ON CONFLICT(task_id,label_id) DO NOTHING",
                turso::named_params! {
                    ":board": board_id.as_str(),
                    ":task": task_id,
                    ":label": label.id.as_str(),
                    ":now": input.now,
                },
            )
            .await?;
        if changed > 0 {
            transaction
                .execute(
                    "INSERT INTO task_events(event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at) VALUES (:event,:board,:task,NULL,'task.label.added',:actor,:payload,:now)",
                    turso::named_params! {
                        ":event": input.event_id.as_str(),
                        ":board": board_id.as_str(),
                        ":task": task_id,
                        ":actor": input.actor.as_str(),
                        ":payload": json!({"label_id": label.id, "label": label.name}).to_string(),
                        ":now": input.now,
                    },
                )
                .await?;
        }
        let mut task = task_in_transaction(&transaction, task_id).await?;
        task.labels = list_task_labels_in_transaction(&transaction, &board_id, task_id).await?;
        transaction.commit().await?;
        let semantics = self.get_label_semantics(&board_id, &label.id).await?;
        Ok(BootstrapTaskLabelStoreRecord { task, semantics })
    }

    pub async fn list_board_labels(
        &self,
        board_selector: &str,
    ) -> Result<Vec<LabelRecord>, StoreError> {
        let connection = self.connection().await?;
        let board_id = active_board_id(&connection, board_selector).await?;
        list_labels_on_connection(&connection, &board_id).await
    }

    pub async fn create_board_label(
        &self,
        board_selector: &str,
        input: CreateLabelInput,
    ) -> Result<LabelRecord, StoreError> {
        validate_create_label_input(&input)?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board_id = active_board_id_in_transaction(&transaction, board_selector).await?;
        let name = input.name.trim();
        if let Some(label) = label_by_name_in_transaction(&transaction, &board_id, name).await? {
            transaction.commit().await?;
            return Ok(label);
        }
        transaction
            .execute(
                "INSERT INTO labels(id, board_id, name, color, created_at, updated_at) VALUES (:id, :board_id, :name, :color, :created_at, :created_at)",
                (
                    (":id", input.id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":name", name),
                    (":color", input.color.as_deref()),
                    (":created_at", input.created_at),
                ),
            )
            .await?;
        let label = label_by_id_in_transaction(&transaction, &board_id, &input.id)
            .await?
            .ok_or_else(|| StoreError::LabelNotFound(input.id.clone()))?;
        transaction.commit().await?;
        Ok(label)
    }

    pub async fn list_task_labels(&self, task_id: &str) -> Result<Vec<LabelRecord>, StoreError> {
        validate_task_id(task_id)?;
        let connection = self.connection().await?;
        let board_id = active_task_board_id(&connection, task_id).await?;
        list_task_labels_on_connection(&connection, &board_id, task_id).await
    }

    pub async fn add_task_labels(
        &self,
        task_id: &str,
        input: AddTaskLabelsInput,
    ) -> Result<AddTaskLabelsRecord, StoreError> {
        validate_task_id(task_id)?;
        validate_add_task_labels_input(&input)?;
        if input.names.len() != input.label_ids.len() || input.names.len() != input.event_ids.len()
        {
            return Err(StoreError::InvalidInput(
                "label input vectors must have equal lengths".to_owned(),
            ));
        }
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board_id = active_task_board_id_in_transaction(&transaction, task_id).await?;
        let mut created_labels = Vec::new();

        for ((name, label_id), event_id) in input
            .names
            .iter()
            .zip(input.label_ids.iter())
            .zip(input.event_ids.iter())
        {
            let label = match label_by_name_in_transaction(&transaction, &board_id, name).await? {
                Some(label) => label,
                None if input.create_missing => {
                    transaction
                        .execute(
                            "INSERT INTO labels(id, board_id, name, color, created_at, updated_at) VALUES (:id, :board_id, :name, NULL, :created_at, :created_at)",
                            (
                                (":id", label_id.as_str()),
                                (":board_id", board_id.as_str()),
                                (":name", name.as_str()),
                                (":created_at", input.now),
                            ),
                        )
                        .await?;
                    let label = label_by_id_in_transaction(&transaction, &board_id, label_id)
                        .await?
                        .ok_or_else(|| StoreError::LabelNotFound(label_id.clone()))?;
                    created_labels.push(label.clone());
                    label
                }
                None => return Err(StoreError::LabelNotFound(name.clone())),
            };
            let changed = transaction
                .execute(
                    "INSERT INTO task_labels(board_id, task_id, label_id, created_at) VALUES (:board_id, :task_id, :label_id, :created_at) ON CONFLICT(task_id, label_id) DO NOTHING",
                    (
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":label_id", label.id.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;
            if changed > 0 {
                transaction
                    .execute(
                        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.label.added', :actor, json_object('label_id', :label_id, 'label', :label), :created_at)",
                        (
                            (":event_id", event_id.as_str()),
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                            (":actor", input.actor.as_str()),
                            (":label_id", label.id.as_str()),
                            (":label", label.name.as_str()),
                            (":created_at", input.now),
                        ),
                    )
                    .await?;
            }
        }

        let mut task = task_in_transaction(&transaction, task_id).await?;
        task.labels = list_task_labels_in_transaction(&transaction, &board_id, task_id).await?;
        transaction.commit().await?;
        Ok(AddTaskLabelsRecord {
            task,
            created_labels,
        })
    }

    pub async fn remove_task_label(
        &self,
        task_id: &str,
        input: RemoveTaskLabelInput,
    ) -> Result<TaskRecord, StoreError> {
        validate_task_id(task_id)?;
        validate_remove_task_label_input(&input)?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board_id = active_task_board_id_in_transaction(&transaction, task_id).await?;
        let label = resolve_label_in_transaction(&transaction, &board_id, &input.label_ref)
            .await?
            .ok_or_else(|| StoreError::LabelNotFound(input.label_ref.trim().to_owned()))?;
        let changed = transaction
            .execute(
                "DELETE FROM task_labels WHERE board_id = :board_id AND task_id = :task_id AND label_id = :label_id",
                (
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id),
                    (":label_id", label.id.as_str()),
                ),
            )
            .await?;
        if changed > 0 {
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.label.removed', :actor, json_object('label_id', :label_id, 'label', :label), :created_at)",
                    (
                        (":event_id", input.event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":actor", input.actor.as_str()),
                        (":label_id", label.id.as_str()),
                        (":label", label.name.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;
        }
        let mut task = task_in_transaction(&transaction, task_id).await?;
        task.labels = list_task_labels_in_transaction(&transaction, &board_id, task_id).await?;
        transaction.commit().await?;
        Ok(task)
    }

    /// 在同一个事务中删除 canonical label identity。
    ///
    /// 任务绑定只有在显式 `force` 时才会移除；semantics/atoms 必须先通过带
    /// CAS 与 reason 的 service operation 清理，identity delete 不得绕过 ontology 审计。
    pub(crate) async fn delete_board_label(
        &self,
        board_selector: &str,
        input: DeleteBoardLabelInput,
    ) -> Result<DeleteBoardLabelRecord, StoreError> {
        validate_delete_board_label_input(&input)?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let board_id = active_board_id_in_transaction(&transaction, board_selector).await?;
        let label = resolve_label_in_transaction(&transaction, &board_id, &input.label_ref)
            .await?
            .ok_or_else(|| StoreError::LabelNotFound(input.label_ref.trim().to_owned()))?;

        let removed_task_bindings =
            count_label_references(&transaction, "task_labels", &board_id, &label.id).await?;
        if removed_task_bindings > 0 && !input.force {
            return Err(StoreError::InvalidInput(format!(
                "label {} is attached to {} task(s); pass --force to delete it",
                label.name, removed_task_bindings
            )));
        }

        let removed_semantics =
            count_label_references(&transaction, "label_semantics", &board_id, &label.id).await?
                > 0;
        let removed_atoms =
            count_label_references(&transaction, "label_atoms", &board_id, &label.id).await?;
        if removed_semantics || removed_atoms > 0 {
            return Err(StoreError::InvalidInput(format!(
                "label {} has semantics or atoms; clear semantics with reason and expected hash before deleting identity",
                label.name
            )));
        }

        if removed_task_bindings > 0 {
            transaction
                .execute(
                    "DELETE FROM task_labels WHERE board_id = :board_id AND label_id = :label_id",
                    [
                        (":board_id", board_id.as_str()),
                        (":label_id", label.id.as_str()),
                    ],
                )
                .await?;
        }
        let changed = transaction
            .execute(
                "DELETE FROM labels WHERE board_id = :board_id AND id = :label_id",
                [
                    (":board_id", board_id.as_str()),
                    (":label_id", label.id.as_str()),
                ],
            )
            .await?;
        if changed != 1 {
            return Err(StoreError::LabelNotFound(label.id.clone()));
        }

        let payload = serde_json::json!({
            "label_id": label.id,
            "label": label.name,
            "forced": input.force,
            "removed_task_bindings": removed_task_bindings,
            "removed_semantics": false,
            "removed_atoms": 0,
        })
        .to_string();
        transaction
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, NULL, NULL, 'label.deleted', :actor, :payload_json, :created_at)",
                turso::named_params! {
                    ":event_id": input.event_id.as_str(),
                    ":board_id": board_id.as_str(),
                    ":actor": input.actor.as_str(),
                    ":payload_json": payload.as_str(),
                    ":created_at": input.now,
                },
            )
            .await?;
        transaction.commit().await?;

        Ok(DeleteBoardLabelRecord {
            label,
            forced: input.force,
            removed_task_bindings,
            removed_semantics: false,
            removed_atoms: 0,
        })
    }
}

fn validate_task_id(task_id: &str) -> Result<(), StoreError> {
    if !task_id.trim().starts_with("t_") || task_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    Ok(())
}

fn validate_create_label_input(input: &CreateLabelInput) -> Result<(), StoreError> {
    if !input.id.trim().starts_with("l_") || input.id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "label id must start with l_".to_owned(),
        ));
    }
    if input.name.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "label name is required".to_owned(),
        ));
    }
    Ok(())
}

fn validate_add_task_labels_input(input: &AddTaskLabelsInput) -> Result<(), StoreError> {
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if input.names.is_empty() {
        return Err(StoreError::InvalidInput(
            "at least one label name is required".to_owned(),
        ));
    }
    if input.names.iter().any(|name| name.trim().is_empty()) {
        return Err(StoreError::InvalidInput(
            "label name is required".to_owned(),
        ));
    }
    Ok(())
}

fn validate_remove_task_label_input(input: &RemoveTaskLabelInput) -> Result<(), StoreError> {
    if input.label_ref.trim().is_empty() {
        return Err(StoreError::InvalidInput("label id is required".to_owned()));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    Ok(())
}

fn validate_delete_board_label_input(input: &DeleteBoardLabelInput) -> Result<(), StoreError> {
    if input.label_ref.trim().is_empty() {
        return Err(StoreError::InvalidInput("label id is required".to_owned()));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event id must start with e_".to_owned(),
        ));
    }
    Ok(())
}

fn validate_bootstrap_task_label_input(input: &BootstrapTaskLabelInput) -> Result<(), StoreError> {
    if input.label_id.trim().is_empty() || !input.label_id.trim().starts_with("l_") {
        return Err(StoreError::InvalidInput(
            "label id must start with l_".to_owned(),
        ));
    }
    if input.name.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "label name is required".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() {
        return Err(StoreError::InvalidInput("actor is required".to_owned()));
    }
    if input
        .description
        .as_deref()
        .map(str::trim)
        .is_none_or(str::is_empty)
        && input
            .applies_when
            .iter()
            .chain(&input.excludes_when)
            .chain(&input.positive_examples)
            .chain(&input.negative_examples)
            .all(|value| value.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "label bootstrap requires description or semantic examples".to_owned(),
        ));
    }
    Ok(())
}

fn normalize_bootstrap_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

async fn count_label_references(
    transaction: &Transaction<'_>,
    table: &str,
    board_id: &str,
    label_id: &str,
) -> Result<i64, StoreError> {
    let sql =
        format!("SELECT COUNT(*) FROM {table} WHERE board_id = :board_id AND label_id = :label_id");
    let row = first_row(
        transaction
            .query(&sql, [(":board_id", board_id), (":label_id", label_id)])
            .await?,
    )
    .await?;
    integer_value(row.get_value(0)?, "label reference count")
}

async fn active_board_id(
    connection: &Connection,
    board_selector: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT id FROM boards WHERE (id = :selector OR slug = :selector) AND archived_at IS NULL LIMIT 1",
                [(":selector", board_selector.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(board_selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    text_value(row.get_value(0)?, "boards.id")
}

async fn active_board_id_in_transaction(
    transaction: &Transaction<'_>,
    board_selector: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT id FROM boards WHERE (id = :selector OR slug = :selector) AND archived_at IS NULL LIMIT 1",
                [(":selector", board_selector.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(board_selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    text_value(row.get_value(0)?, "boards.id")
}

async fn active_task_board_id(
    connection: &Connection,
    task_id: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT t.board_id, t.status, t.archived_at, b.archived_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                [(":task_id", task_id.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    active_task_board_from_row(row, task_id)
}

async fn active_task_board_id_in_transaction(
    transaction: &Transaction<'_>,
    task_id: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT t.board_id, t.status, t.archived_at, b.archived_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                [(":task_id", task_id.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    active_task_board_from_row(row, task_id)
}

fn active_task_board_from_row(row: Row, task_id: &str) -> Result<String, StoreError> {
    let board_id = text_value(row.get_value(0)?, "tasks.board_id")?;
    let status = text_value(row.get_value(1)?, "tasks.status")?;
    let task_archived = optional_integer_value(row.get_value(2)?, "tasks.archived_at")?;
    let board_archived = optional_integer_value(row.get_value(3)?, "boards.archived_at")?;
    if status == "archived" || task_archived.is_some() || board_archived.is_some() {
        return Err(StoreError::TaskNotFound(task_id.to_owned()));
    }
    Ok(board_id)
}

async fn list_labels_on_connection(
    connection: &Connection,
    board_id: &str,
) -> Result<Vec<LabelRecord>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id = :board_id ORDER BY name ASC, id ASC",
            [(":board_id", board_id)],
        )
        .await?;
    labels_from_rows(&mut rows).await
}

async fn list_task_labels_on_connection(
    connection: &Connection,
    board_id: &str,
    task_id: &str,
) -> Result<Vec<LabelRecord>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT l.id, l.board_id, l.name, l.color, l.created_at, l.updated_at FROM task_labels AS tl JOIN labels AS l ON l.id = tl.label_id AND l.board_id = tl.board_id WHERE tl.board_id = :board_id AND tl.task_id = :task_id ORDER BY l.name ASC, l.id ASC",
            [(":board_id", board_id), (":task_id", task_id)],
        )
        .await?;
    labels_from_rows(&mut rows).await
}

async fn labels_from_rows(rows: &mut turso::Rows) -> Result<Vec<LabelRecord>, StoreError> {
    let mut labels = Vec::new();
    while let Some(row) = rows.next().await? {
        labels.push(label_from_row(row)?);
    }
    Ok(labels)
}

pub(crate) async fn list_task_labels_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    task_id: &str,
) -> Result<Vec<LabelRecord>, StoreError> {
    let mut rows = transaction
        .query(
            "SELECT l.id, l.board_id, l.name, l.color, l.created_at, l.updated_at FROM task_labels AS tl JOIN labels AS l ON l.id = tl.label_id AND l.board_id = tl.board_id WHERE tl.board_id = :board_id AND tl.task_id = :task_id ORDER BY l.name ASC, l.id ASC",
            [(":board_id", board_id), (":task_id", task_id)],
        )
        .await?;
    labels_from_rows(&mut rows).await
}

async fn task_in_transaction(
    transaction: &Transaction<'_>,
    task_id: &str,
) -> Result<TaskRecord, StoreError> {
    let row = first_row(
        transaction
            .query(
                &format!("{TASK_SELECT} WHERE t.id = :task_id LIMIT 1"),
                [(":task_id", task_id)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    task_from_row(row)
}

async fn label_by_name_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    name: &str,
) -> Result<Option<LabelRecord>, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id = :board_id AND name = :name LIMIT 1",
                [(":board_id", board_id), (":name", name)],
            )
            .await?,
    )
    .await;
    match row {
        Ok(row) => Ok(Some(label_from_row(row)?)),
        Err(turso::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(StoreError::Turso(error)),
    }
}

async fn label_by_id_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    label_id: &str,
) -> Result<Option<LabelRecord>, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT id, board_id, name, color, created_at, updated_at FROM labels WHERE board_id = :board_id AND id = :label_id LIMIT 1",
                [(":board_id", board_id), (":label_id", label_id)],
            )
            .await?,
    )
    .await;
    match row {
        Ok(row) => Ok(Some(label_from_row(row)?)),
        Err(turso::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(StoreError::Turso(error)),
    }
}

pub(crate) async fn resolve_label_in_transaction(
    transaction: &Transaction<'_>,
    board_id: &str,
    label_ref: &str,
) -> Result<Option<LabelRecord>, StoreError> {
    let label_ref = label_ref.trim();
    if let Some(label) = label_by_name_in_transaction(transaction, board_id, label_ref).await? {
        return Ok(Some(label));
    }
    if label_ref.starts_with("l_") {
        return label_by_id_in_transaction(transaction, board_id, label_ref).await;
    }
    Ok(None)
}

fn label_from_row(row: Row) -> Result<LabelRecord, StoreError> {
    Ok(LabelRecord {
        id: text_value(row.get_value(0)?, "labels.id")?,
        board_id: text_value(row.get_value(1)?, "labels.board_id")?,
        name: text_value(row.get_value(2)?, "labels.name")?,
        color: optional_text_value(row.get_value(3)?, "labels.color")?,
        created_at: integer_value(row.get_value(4)?, "labels.created_at")?,
        updated_at: integer_value(row.get_value(5)?, "labels.updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{create_input, store};

    fn create_label_input(id: &str, name: &str) -> CreateLabelInput {
        CreateLabelInput {
            id: id.to_owned(),
            name: name.to_owned(),
            color: Some("#4477aa".to_owned()),
            created_at: 100,
        }
    }

    fn add_input(
        name: &str,
        label_id: &str,
        event_id: &str,
        create_missing: bool,
    ) -> AddTaskLabelsInput {
        AddTaskLabelsInput {
            names: vec![name.to_owned()],
            label_ids: vec![label_id.to_owned()],
            event_ids: vec![event_id.to_owned()],
            create_missing,
            actor: "label-test".to_owned(),
            now: 100,
        }
    }

    #[tokio::test]
    async fn board_labels_trim_names_and_keep_duplicate_create_idempotent() {
        let (_directory, store, _path) = store("labels-create").await;
        store.initialize().await.expect("initialize");

        let first = store
            .create_board_label("default", create_label_input("l_first", "  urgent  "))
            .await
            .expect("create label");
        assert_eq!(first.name, "urgent");
        assert_eq!(first.color.as_deref(), Some("#4477aa"));

        let duplicate = store
            .create_board_label("default", create_label_input("l_duplicate", "urgent"))
            .await
            .expect("duplicate create returns existing label");
        assert_eq!(duplicate.id, first.id);

        let labels = store
            .list_board_labels("default")
            .await
            .expect("list labels");
        assert_eq!(labels, vec![first]);
    }

    #[tokio::test]
    async fn task_label_attach_is_idempotent_and_emits_one_event() {
        let (_directory, store, _path) = store("labels-attach").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_labels_attach", Some("labels-1"), "Labels"),
            )
            .await
            .expect("create task");
        store
            .create_board_label("default", create_label_input("l_attach", "urgent"))
            .await
            .expect("create label");

        let added = store
            .add_task_labels(
                "t_labels_attach",
                add_input("urgent", "l_generated", "evt-label-add-1", false),
            )
            .await
            .expect("attach label");
        assert_eq!(added.task.labels.len(), 1);
        assert!(added.created_labels.is_empty());

        let duplicate = store
            .add_task_labels(
                "t_labels_attach",
                add_input("urgent", "l_generated-duplicate", "evt-label-add-2", false),
            )
            .await
            .expect("duplicate attach");
        assert_eq!(duplicate.task.labels, added.task.labels);

        let connection = store.connection().await.expect("connection");
        let row = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = 't_labels_attach' AND kind = 'task.label.added'",
                    (),
                )
                .await
                .expect("event count query"),
        )
        .await
        .expect("event count row");
        assert_eq!(
            integer_value(row.get_value(0).unwrap(), "event count").unwrap(),
            1
        );

        let removed = store
            .remove_task_label(
                "t_labels_attach",
                RemoveTaskLabelInput {
                    label_ref: "l_attach".to_owned(),
                    event_id: "evt-label-remove-1".to_owned(),
                    actor: "label-test".to_owned(),
                    now: 101,
                },
            )
            .await
            .expect("remove label");
        assert!(removed.labels.is_empty());
    }

    #[tokio::test]
    async fn task_labels_are_board_isolated_and_active_guarded() {
        let (_directory, store, _path) = store("labels-isolation").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other', 'other', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("insert second board");
        store
            .create_board_label("other", create_label_input("l_other", "shared"))
            .await
            .expect("create label on other board");
        store
            .create_task(
                "default",
                create_input("t_labels_isolation", Some("labels-2"), "Labels"),
            )
            .await
            .expect("create task");

        let missing = store
            .add_task_labels(
                "t_labels_isolation",
                add_input("shared", "l_default", "evt-label-missing", false),
            )
            .await
            .expect_err("other-board label must not cross the board boundary");
        assert!(matches!(missing, StoreError::LabelNotFound(name) if name == "shared"));

        let created = store
            .add_task_labels(
                "t_labels_isolation",
                add_input("shared", "l_default", "evt-label-created", true),
            )
            .await
            .expect("create default-board label");
        assert_eq!(created.created_labels.len(), 1);
        assert_eq!(created.task.labels.len(), 1);
        assert_eq!(created.task.labels[0].board_id, "b_default");

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 200 WHERE id = 't_labels_isolation'",
                (),
            )
            .await
            .expect("archive task");
        let archived = store
            .list_task_labels("t_labels_isolation")
            .await
            .expect_err("archived task must be guarded");
        assert!(
            matches!(archived, StoreError::TaskNotFound(task_id) if task_id == "t_labels_isolation")
        );
    }

    #[tokio::test]
    async fn canonical_label_delete_requires_force_for_bindings_and_records_event() {
        let (_directory, store, _path) = store("labels-delete").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_labels_delete", Some("labels-delete"), "Delete label"),
            )
            .await
            .expect("create task");
        let label = store
            .create_board_label("default", create_label_input("l_delete", "retired"))
            .await
            .expect("create label");
        store
            .add_task_labels(
                "t_labels_delete",
                add_input("retired", "l_unused", "evt-label-attach", false),
            )
            .await
            .expect("attach label");

        let blocked = store
            .delete_board_label(
                "default",
                DeleteBoardLabelInput {
                    label_ref: label.name.clone(),
                    event_id: "e_label_delete_blocked".to_owned(),
                    actor: "label-test".to_owned(),
                    force: false,
                    now: 200,
                },
            )
            .await
            .expect_err("bindings require force");
        assert!(blocked.to_string().contains("attached to 1 task(s)"));
        assert_eq!(store.list_board_labels("default").await.unwrap().len(), 1);

        let deleted = store
            .delete_board_label(
                "default",
                DeleteBoardLabelInput {
                    label_ref: label.id.clone(),
                    event_id: "e_label_delete_forced".to_owned(),
                    actor: "label-test".to_owned(),
                    force: true,
                    now: 201,
                },
            )
            .await
            .expect("force delete");
        assert_eq!(deleted.label, label);
        assert!(deleted.forced);
        assert_eq!(deleted.removed_task_bindings, 1);
        assert!(!deleted.removed_semantics);
        assert_eq!(deleted.removed_atoms, 0);
        assert!(store.list_board_labels("default").await.unwrap().is_empty());

        let connection = store.connection().await.expect("connection");
        let row = first_row(
            connection
                .query(
                    "SELECT kind, actor, payload_json FROM task_events WHERE event_id = 'e_label_delete_forced'",
                    (),
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(row.get_value(0).unwrap(), "event kind").unwrap(),
            "label.deleted"
        );
        assert_eq!(
            text_value(row.get_value(1).unwrap(), "event actor").unwrap(),
            "label-test"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &text_value(row.get_value(2).unwrap(), "event payload").unwrap(),
            )
            .unwrap()["removed_task_bindings"],
            1
        );
        let row = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_labels WHERE task_id = 't_labels_delete'",
                    (),
                )
                .await
                .expect("binding count query"),
        )
        .await
        .expect("binding count row");
        assert_eq!(
            integer_value(row.get_value(0).unwrap(), "binding count").unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn canonical_label_delete_rejects_semantics_and_keeps_board_isolation() {
        let (_directory, store, _path) = store("labels-delete-guard").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_other_delete', 'other-delete', 'Other', 1, 1)",
                (),
            )
            .await
            .expect("other board");
        let other = store
            .create_board_label(
                "other-delete",
                create_label_input("l_other_delete", "shared"),
            )
            .await
            .expect("other label");
        let default_label = store
            .create_board_label("default", create_label_input("l_default_delete", "shared"))
            .await
            .expect("default label");
        connection
            .execute(
                "INSERT INTO label_semantics(label_id, board_id, description, created_at, updated_at) VALUES (?1, ?2, ?3, 1, 1)",
                (
                    default_label.id.as_str(),
                    "b_default",
                    "must clear before deleting",
                ),
            )
            .await
            .expect("semantics");
        connection
            .execute(
                "INSERT INTO label_atoms(id, label_id, board_id, polarity, kind, text, ordinal, content_hash, created_at, updated_at) VALUES ('la_delete_guard', ?1, ?2, 'positive', 'description', 'guard', 0, 'hash-delete-guard', 1, 1)",
                (default_label.id.as_str(), "b_default"),
            )
            .await
            .expect("atom");

        let wrong_board = store
            .delete_board_label(
                "default",
                DeleteBoardLabelInput {
                    label_ref: other.id,
                    event_id: "e_label_delete_wrong_board".to_owned(),
                    actor: "label-test".to_owned(),
                    force: true,
                    now: 200,
                },
            )
            .await
            .expect_err("cross-board label must be isolated");
        assert!(
            matches!(wrong_board, StoreError::LabelNotFound(reference) if reference == "l_other_delete")
        );

        let guarded = store
            .delete_board_label(
                "default",
                DeleteBoardLabelInput {
                    label_ref: default_label.name.clone(),
                    event_id: "e_label_delete_guarded".to_owned(),
                    actor: "label-test".to_owned(),
                    force: true,
                    now: 201,
                },
            )
            .await
            .expect_err("semantics must be cleared through ontology operation");
        assert!(guarded.to_string().contains("has semantics or atoms"));
        assert_eq!(
            store.list_board_labels("default").await.unwrap(),
            vec![default_label]
        );
    }

    #[tokio::test]
    async fn bootstrap_writes_semantics_action_and_idempotent_task_binding() {
        let (_directory, store, _path) = store("labels-bootstrap").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_labels_bootstrap", Some("labels-bootstrap"), "Bootstrap"),
            )
            .await
            .expect("create task");

        let result = store
            .bootstrap_task_label(
                "t_labels_bootstrap",
                BootstrapTaskLabelInput {
                    label_id: "l_bootstrap".to_owned(),
                    event_id: "evt-bootstrap-label".to_owned(),
                    name: "  backend  ".to_owned(),
                    description: Some(" Backend implementation ".to_owned()),
                    applies_when: vec![" touches Rust ".to_owned()],
                    excludes_when: vec!["  ".to_owned()],
                    positive_examples: Vec::new(),
                    negative_examples: vec![" tests fail ".to_owned()],
                    actor: "label-test".to_owned(),
                    now: 100,
                },
            )
            .await
            .expect("bootstrap label");
        assert_eq!(result.task.labels.len(), 1);
        assert_eq!(result.task.labels[0].name, "backend");
        assert_eq!(result.semantics.label_name, "backend");
        assert_eq!(
            result.semantics.applies_when,
            vec!["touches Rust".to_owned()]
        );
        assert_eq!(
            result.semantics.negative_examples,
            vec!["tests fail".to_owned()]
        );
        assert!(!result.semantics.atoms.is_empty());

        let connection = store.connection().await.expect("connection");
        let row = first_row(
            connection
                .query(
                    "SELECT kind, COUNT(*) FROM task_events WHERE task_id = 't_labels_bootstrap' AND kind = 'task.label.added' GROUP BY kind",
                    (),
                )
                .await
                .expect("event count query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(row.get_value(0).unwrap(), "event kind").unwrap(),
            "task.label.added"
        );

        let replacement = store
            .bootstrap_task_label(
                "t_labels_bootstrap",
                BootstrapTaskLabelInput {
                    label_id: "l_bootstrap_2".to_owned(),
                    event_id: "evt-bootstrap-label-2".to_owned(),
                    name: "backend".to_owned(),
                    description: Some("replacement".to_owned()),
                    applies_when: Vec::new(),
                    excludes_when: Vec::new(),
                    positive_examples: Vec::new(),
                    negative_examples: Vec::new(),
                    actor: "label-test".to_owned(),
                    now: 101,
                },
            )
            .await
            .expect_err("bootstrap must not replace existing semantics");
        assert!(
            matches!(replacement, StoreError::InvalidInput(message) if message.contains("would replace existing semantics"))
        );
    }
}
