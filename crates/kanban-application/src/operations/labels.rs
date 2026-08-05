use std::{collections::HashSet, future::Future};

use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};

use crate::{
    ApplicationService, ApplicationStore, LabelRecord, TaskRecord,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBoardLabelCommand {
    pub board: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateLabelRecord {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: i64,
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

pub trait BoardLabelList: ApplicationStore {
    fn list_board_labels(
        &self,
        board: &str,
    ) -> impl Future<Output = Result<Vec<LabelRecord>>> + Send;
}

pub trait BoardLabelCreate: ApplicationStore {
    fn create_board_label(
        &self,
        board: &str,
        input: CreateLabelRecord,
    ) -> impl Future<Output = Result<LabelRecord>> + Send;
}

pub trait TaskLabelList: ApplicationStore {
    fn list_task_labels(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<Vec<LabelRecord>>> + Send;
}

pub trait TaskLabelAdd: ApplicationStore {
    fn add_task_labels(
        &self,
        task_id: &str,
        input: AddTaskLabelsRecordInput,
    ) -> impl Future<Output = Result<AddTaskLabelsRecord>> + Send;
}

pub trait TaskLabelRemove: ApplicationStore {
    fn remove_task_label(
        &self,
        task_id: &str,
        input: crate::operations::RemoveTaskLabelRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddTaskLabelsRecordInput {
    pub names: Vec<String>,
    pub label_ids: Vec<String>,
    pub event_ids: Vec<String>,
    pub create_missing: bool,
    pub actor: String,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoveTaskLabelRecord {
    pub label_ref: String,
    pub event_id: String,
    pub actor: String,
    pub now: i64,
}

impl<S, C> ApplicationService<S, C>
where
    S: BoardLabelList,
    C: Clock,
{
    pub async fn list_board_labels(&self, board: &str) -> Result<Vec<LabelRecord>> {
        let board = required_trimmed(board, "board is required")?;
        self.store.list_board_labels(board).await
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: BoardLabelCreate,
    C: Clock,
{
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
                CreateLabelRecord {
                    id: new_typed_id("l"),
                    name: name.to_owned(),
                    color,
                    created_at: self.clock.now_ms(),
                },
            )
            .await
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskLabelList,
    C: Clock,
{
    pub async fn list_task_labels(&self, task_id: &str) -> Result<Vec<LabelRecord>> {
        let task_id = global_task_id(task_id)?;
        self.store.list_task_labels(task_id).await
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskLabelAdd,
    C: Clock,
{
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
        self.store
            .add_task_labels(
                task_id,
                AddTaskLabelsRecordInput {
                    names,
                    label_ids,
                    event_ids,
                    create_missing: command.create_missing,
                    actor: actor.to_owned(),
                    now,
                },
            )
            .await
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskLabelRemove,
    C: Clock,
{
    pub async fn remove_task_label(
        &self,
        command: RemoveTaskLabelCommand,
    ) -> Result<TaskRecord> {
        let task_id = global_task_id(&command.task_id)?;
        let label_ref = required_trimmed(&command.label_ref, "label id is required")?;
        let actor = required_trimmed(&command.actor, "actor is required")?;
        let _mutation = self.mutation_gate.lock().await;
        self.store
            .remove_task_label(
                task_id,
                RemoveTaskLabelRecord {
                    label_ref: label_ref.to_owned(),
                    event_id: new_event_id(),
                    actor: actor.to_owned(),
                    now: self.clock.now_ms(),
                },
            )
            .await
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl BoardLabelList for StubStore {
        async fn list_board_labels(&self, board: &str) -> Result<Vec<LabelRecord>> {
            assert_eq!(board, "default");
            Ok(Vec::new())
        }
    }

    impl BoardLabelCreate for StubStore {
        async fn create_board_label(
            &self,
            board: &str,
            input: CreateLabelRecord,
        ) -> Result<LabelRecord> {
            assert_eq!(board, "default");
            Ok(LabelRecord {
                id: input.id,
                board_id: "b_default".into(),
                name: input.name,
                color: input.color,
                created_at: input.created_at,
                updated_at: input.created_at,
            })
        }
    }

    impl TaskLabelList for StubStore {
        async fn list_task_labels(&self, task_id: &str) -> Result<Vec<LabelRecord>> {
            assert_eq!(task_id, "t_label");
            Ok(Vec::new())
        }
    }

    impl TaskLabelAdd for StubStore {
        async fn add_task_labels(
            &self,
            task_id: &str,
            input: AddTaskLabelsRecordInput,
        ) -> Result<AddTaskLabelsRecord> {
            assert_eq!(task_id, "t_label");
            assert_eq!(input.names, vec!["backend", "api"]);
            assert_eq!(input.now, 100);
            Ok(AddTaskLabelsRecord {
                task: crate::operations::test_support::task_for_id(task_id),
                created_labels: Vec::new(),
            })
        }
    }

    impl TaskLabelRemove for StubStore {
        async fn remove_task_label(
            &self,
            task_id: &str,
            input: RemoveTaskLabelRecord,
        ) -> Result<TaskRecord> {
            assert_eq!(task_id, "t_label");
            assert_eq!(input.label_ref, "l_backend");
            Ok(crate::operations::test_support::task_for_id(task_id))
        }
    }

    fn service() -> ApplicationService<StubStore, FixedClock> {
        ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        )
    }

    #[tokio::test]
    async fn label_commands_trim_and_dedupe_names() {
        let label = service()
            .create_board_label(CreateBoardLabelCommand {
                board: " default ".into(),
                name: " backend ".into(),
                color: Some(" #4477aa ".into()),
            })
            .await
            .unwrap();
        assert_eq!(label.name, "backend");
        assert_eq!(label.color.as_deref(), Some("#4477aa"));

        service()
            .add_task_labels(AddTaskLabelsCommand {
                task_id: " t_label ".into(),
                names: vec![" backend ".into(), "api".into(), "backend".into()],
                create_missing: true,
                actor: " tester ".into(),
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn label_commands_validate_global_task_ids() {
        let error = service()
            .list_task_labels("default#1")
            .await
            .expect_err("board-local task selectors are client responsibility");
        assert!(matches!(error, KanbanError::InvalidInput(message) if message.contains("global")));
    }
}
