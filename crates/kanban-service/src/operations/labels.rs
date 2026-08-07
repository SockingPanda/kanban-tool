use std::collections::HashSet;

use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};

use crate::{
    AddTaskLabelsInput, BootstrapTaskLabelInput, BootstrapTaskLabelVerification,
    DeleteBoardLabelInput, KanbanService, LabelAtomRecord, LabelRecord, LabelSemanticsRecord,
    TaskRecord, VectorConfigureCommand,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBoardLabelCommand {
    pub board: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTaskLabelsCommand {
    pub task_id: String,
    pub names: Vec<String>,
    pub create_missing: bool,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTaskLabelsRecord {
    pub task: TaskRecord,
    pub created_labels: Vec<LabelRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveTaskLabelCommand {
    pub task_id: String,
    pub label_ref: String,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteBoardLabelCommand {
    pub board: String,
    pub label_ref: String,
    pub force: bool,
    pub actor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteBoardLabelRecord {
    pub label: LabelRecord,
    pub forced: bool,
    pub removed_task_bindings: i64,
    pub removed_semantics: bool,
    pub removed_atoms: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapTaskLabelCommand {
    pub task_id: String,
    pub name: String,
    pub description: Option<String>,
    pub applies_when: Vec<String>,
    pub excludes_when: Vec<String>,
    pub positive_examples: Vec<String>,
    pub negative_examples: Vec<String>,
    pub actor: String,
    pub verify: bool,
    pub min_verify_score: f32,
    pub vector_config: Option<VectorConfigureCommand>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapTaskLabelRecord {
    pub task: TaskRecord,
    pub semantics: LabelSemanticsRecord,
    pub verification: Option<BootstrapTaskLabelVerification>,
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_board_labels(&self, board: &str) -> Result<Vec<LabelRecord>> {
        let board = required_trimmed(board, "board is required")?;
        self.store
            .list_board_labels(board)
            .await
            .map_err(crate::error::store_error)
            .map(|labels| {
                labels
                    .into_iter()
                    .map(crate::operations::application_label)
                    .collect()
            })
    }

    pub async fn create_board_label(
        &self,
        command: CreateBoardLabelCommand,
    ) -> Result<LabelRecord> {
        let board = required_trimmed(&command.board, "board is required")?;
        let name = required_trimmed(&command.name, "label name is required")?;
        let color = command
            .color
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let _mutation = self.mutation_gate.lock().await;
        self.store
            .create_board_label(
                board,
                crate::CreateLabelInput {
                    id: new_typed_id("l"),
                    name: name.to_owned(),
                    color,
                    created_at: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::error::store_error)
            .map(crate::operations::application_label)
    }

    pub async fn list_task_labels(&self, task_id: &str) -> Result<Vec<LabelRecord>> {
        let task_id = global_task_id(task_id)?;
        self.store
            .list_task_labels(task_id)
            .await
            .map_err(crate::error::store_error)
            .map(|labels| {
                labels
                    .into_iter()
                    .map(crate::operations::application_label)
                    .collect()
            })
    }

    pub async fn add_task_labels(
        &self,
        command: AddTaskLabelsCommand,
    ) -> Result<AddTaskLabelsRecord> {
        let task_id = global_task_id(&command.task_id)?;
        let names = normalize_names(&command.names)?;
        let actor = required_trimmed(&command.actor, "actor is required")?;
        let label_ids = names.iter().map(|_| new_typed_id("l")).collect();
        let event_ids = names.iter().map(|_| new_event_id()).collect();
        let now = self.clock.now_ms();
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .add_task_labels(
                task_id,
                AddTaskLabelsInput {
                    names,
                    label_ids,
                    event_ids,
                    create_missing: command.create_missing,
                    actor: actor.to_owned(),
                    now,
                },
            )
            .await
            .map_err(crate::error::store_error)?;
        Ok(AddTaskLabelsRecord {
            task: super::application_task(record.task)?,
            created_labels: record
                .created_labels
                .into_iter()
                .map(crate::operations::application_label)
                .collect(),
        })
    }

    pub async fn remove_task_label(&self, command: RemoveTaskLabelCommand) -> Result<TaskRecord> {
        let task_id = global_task_id(&command.task_id)?;
        let label_ref = required_trimmed(&command.label_ref, "label id is required")?;
        let actor = required_trimmed(&command.actor, "actor is required")?;
        let _mutation = self.mutation_gate.lock().await;
        self.store
            .remove_task_label(
                task_id,
                crate::RemoveTaskLabelInput {
                    label_ref: label_ref.to_owned(),
                    event_id: new_event_id(),
                    actor: actor.to_owned(),
                    now: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::error::store_error)
            .and_then(super::application_task)
    }

    pub async fn delete_board_label(
        &self,
        command: DeleteBoardLabelCommand,
    ) -> Result<DeleteBoardLabelRecord> {
        let board = required_trimmed(&command.board, "board is required")?;
        let label_ref = required_trimmed(&command.label_ref, "label id is required")?;
        let actor = required_trimmed(&command.actor, "actor is required")?;
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .delete_board_label(
                board,
                DeleteBoardLabelInput {
                    label_ref: label_ref.to_owned(),
                    event_id: new_event_id(),
                    actor: actor.to_owned(),
                    force: command.force,
                    now: self.clock.now_ms(),
                },
            )
            .await
            .map_err(crate::error::store_error)?;
        Ok(DeleteBoardLabelRecord {
            label: crate::operations::application_label(record.label),
            forced: record.forced,
            removed_task_bindings: record.removed_task_bindings,
            removed_semantics: record.removed_semantics,
            removed_atoms: record.removed_atoms,
        })
    }

    pub async fn bootstrap_task_label(
        &self,
        command: BootstrapTaskLabelCommand,
    ) -> Result<BootstrapTaskLabelRecord> {
        let task_id = global_task_id(&command.task_id)?;
        let actor = required_trimmed(&command.actor, "actor is required")?;
        let verify = command.verify || command.vector_config.is_some();
        if verify && !(0.0..=1.0).contains(&command.min_verify_score) {
            return Err(KanbanError::InvalidInput(
                "min_verify_score 必须在 0 到 1 之间".to_owned(),
            ));
        }
        let verification_snapshot = if verify {
            let task = self
                .store
                .get_task_global(task_id)
                .await
                .map_err(crate::error::store_error)?;
            let ontology_digest = self
                .store
                .bootstrap_ontology_digest(&task.board_id)
                .await
                .map_err(crate::error::store_error)?;
            let task_suggest_digest = crate::store_operations::bootstrap_task_suggest_digest(
                &task.title,
                task.description.as_deref(),
            );
            let label_state_digest = self
                .store
                .bootstrap_label_state_digest(&task.board_id, &command.name)
                .await
                .map_err(crate::error::store_error)?;
            Some((
                task.lock_version,
                task_suggest_digest,
                label_state_digest,
                ontology_digest,
            ))
        } else {
            None
        };
        // provider 调用在 gate 外执行；canonical transaction 仍由 gate 串行化，并在
        // store 内以 task/label/ontology 快照做最后一次 CAS。
        let verification = if verify {
            let config = match command.vector_config.clone() {
                Some(config) => crate::vector::VectorConfig {
                    provider: config.provider,
                    endpoint: config.endpoint,
                    model: config.model,
                    dimensions: config.dimensions,
                },
                None => self
                    .store
                    .vector_config()
                    .await
                    .map_err(crate::error::store_error)?
                    .ok_or_else(|| {
                        KanbanError::InvalidInput(
                            "label bootstrap verification 需要已配置的 vector provider；请传入 vector config，或先配置 vector"
                                .to_owned(),
                        )
                    })?,
            };
            Some(
                crate::vector::verify_bootstrap_task_label(
                    &self.store,
                    task_id,
                    &BootstrapTaskLabelInput {
                        label_id: "l_bootstrap_verify".to_owned(),
                        event_id: "evt_bootstrap_verify".to_owned(),
                        name: command.name.clone(),
                        description: command.description.clone(),
                        applies_when: command.applies_when.clone(),
                        excludes_when: command.excludes_when.clone(),
                        positive_examples: command.positive_examples.clone(),
                        negative_examples: command.negative_examples.clone(),
                        actor: actor.to_owned(),
                        now: self.clock.now_ms(),
                        expected_task_lock_version: None,
                        expected_task_suggest_digest: None,
                        expected_label_state_digest: None,
                        expected_ontology_digest: None,
                        verification_context: None,
                    },
                    &config,
                    command.min_verify_score,
                )
                .await
                .map_err(crate::error::store_error)?,
            )
        } else {
            None
        };
        let _mutation = self.mutation_gate.lock().await;
        let record = self
            .store
            .bootstrap_task_label(
                task_id,
                BootstrapTaskLabelInput {
                    label_id: new_typed_id("l"),
                    event_id: new_event_id(),
                    name: command.name,
                    description: command.description,
                    applies_when: command.applies_when,
                    excludes_when: command.excludes_when,
                    positive_examples: command.positive_examples,
                    negative_examples: command.negative_examples,
                    actor: actor.to_owned(),
                    now: self.clock.now_ms(),
                    expected_task_lock_version: verification_snapshot
                        .as_ref()
                        .map(|(lock_version, _, _, _)| *lock_version),
                    expected_task_suggest_digest: verification_snapshot
                        .as_ref()
                        .map(|(_, digest, _, _)| digest.clone()),
                    expected_label_state_digest: verification_snapshot
                        .as_ref()
                        .map(|(_, _, digest, _)| digest.clone()),
                    expected_ontology_digest: verification_snapshot
                        .as_ref()
                        .map(|(_, _, _, digest)| digest.clone()),
                    verification_context: verification
                        .as_ref()
                        .map(verification_context_json)
                        .transpose()?,
                },
            )
            .await
            .map_err(crate::error::store_error)?;
        Ok(BootstrapTaskLabelRecord {
            task: super::application_task(record.task)?,
            semantics: application_semantics(record.semantics),
            verification,
        })
    }
}

fn global_task_id(value: &str) -> Result<&str> {
    let value = value.trim();
    if !value.starts_with("t_") || value.len() <= 2 {
        return Err(KanbanError::InvalidInput(
            "task_id must be a global t_... id".to_owned(),
        ));
    }
    Ok(value)
}

fn verification_context_json(value: &BootstrapTaskLabelVerification) -> Result<String> {
    serde_json::to_string(&serde_json::json!({
        "bootstrap_verification": {
            "label_name": &value.label_name,
            "score": value.score,
            "source": &value.source,
            "min_score": value.min_score,
            "degraded": value.degraded,
            "diagnostics": &value.diagnostics,
        }
    }))
    .map_err(|error| KanbanError::InvalidInput(format!("verification context 无效：{error}")))
}

pub(crate) fn application_label(label: crate::domain::LabelRecord) -> LabelRecord {
    LabelRecord {
        id: label.id,
        board_id: label.board_id,
        name: label.name,
        color: label.color,
        created_at: label.created_at,
        updated_at: label.updated_at,
    }
}

fn application_semantics(value: crate::domain::LabelSemanticsRecord) -> LabelSemanticsRecord {
    LabelSemanticsRecord {
        label_id: value.label_id,
        board_id: value.board_id,
        label_name: value.label_name,
        semantics_hash: value.semantics_hash,
        description: value.description,
        applies_when: value.applies_when,
        excludes_when: value.excludes_when,
        positive_examples: value.positive_examples,
        negative_examples: value.negative_examples,
        created_at: value.created_at,
        updated_at: value.updated_at,
        atoms: value
            .atoms
            .into_iter()
            .map(|atom| LabelAtomRecord {
                id: atom.id,
                label_id: atom.label_id,
                board_id: atom.board_id,
                label_name: atom.label_name,
                polarity: atom.polarity,
                kind: atom.kind,
                text: atom.text,
                ordinal: atom.ordinal,
                content_hash: atom.content_hash,
                created_at: atom.created_at,
                updated_at: atom.updated_at,
            })
            .collect(),
    }
}

fn required_trimmed<'a>(value: &'a str, message: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(KanbanError::InvalidInput(message.to_owned()));
    }
    Ok(value)
}

fn normalize_names(names: &[String]) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(names.len());
    for name in names {
        let name = required_trimmed(name, "label name is required")?.to_owned();
        if seen.insert(name.clone()) {
            normalized.push(name);
        }
    }
    if normalized.is_empty() {
        return Err(KanbanError::InvalidInput(
            "at least one label name is required".to_owned(),
        ));
    }
    Ok(normalized)
}
