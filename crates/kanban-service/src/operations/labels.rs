use std::collections::HashSet;

use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};

use crate::{AddTaskLabelsInput, KanbanService, LabelRecord, TaskRecord};

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
