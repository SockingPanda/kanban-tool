use std::future::Future;

use kanban_core::{Clock, KanbanError, Result, new_event_id};

use crate::{ApplicationService, ApplicationStore, TaskRecord};

/// 可安全修改的任务字段。状态、claim 和完成信息不在这里出现。
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateTaskCommand {
    pub task_id: String,
    pub actor: String,
    pub expected_lock_version: Option<i64>,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub assignee: Option<Option<String>>,
    pub priority: Option<i64>,
    pub scheduled_at: Option<Option<i64>>,
    pub due_at: Option<Option<i64>>,
    pub max_retries: Option<Option<i64>>,
    pub metadata: Option<Option<serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateTaskRecord {
    pub expected_lock_version: i64,
    pub actor: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub assignee: Option<Option<String>>,
    pub priority: Option<i64>,
    pub scheduled_at: Option<Option<i64>>,
    pub due_at: Option<Option<i64>>,
    pub max_retries: Option<Option<i64>>,
    pub metadata_json: Option<String>,
    pub event_id: String,
    pub now: i64,
}

pub trait TaskUpdate: ApplicationStore {
    fn get_task(&self, task_id: &str) -> impl Future<Output = Result<TaskRecord>> + Send;

    fn update_task(
        &self,
        task_id: &str,
        input: UpdateTaskRecord,
    ) -> impl Future<Output = Result<TaskRecord>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskUpdate,
    C: Clock,
{
    pub async fn update_task(&self, command: UpdateTaskCommand) -> Result<TaskRecord> {
        let task_id = command.task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id 必须是全局 t_... ID".to_owned(),
            ));
        }
        let actor = command.actor.trim();
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput("actor 不能为空".to_owned()));
        }
        if command
            .expected_lock_version
            .is_some_and(|version| version < 0)
        {
            return Err(KanbanError::InvalidInput(
                "expected_lock_version 不能为负数".to_owned(),
            ));
        }
        if command.title.is_none()
            && command.description.is_none()
            && command.assignee.is_none()
            && command.priority.is_none()
            && command.scheduled_at.is_none()
            && command.due_at.is_none()
            && command.max_retries.is_none()
            && command.metadata.is_none()
        {
            return Err(KanbanError::InvalidInput("至少需要一个任务字段".to_owned()));
        }
        if command
            .title
            .as_deref()
            .is_some_and(|title| title.trim().is_empty())
        {
            return Err(KanbanError::InvalidInput("title 不能为空".to_owned()));
        }
        if command
            .priority
            .is_some_and(|priority| !(0..=3).contains(&priority))
        {
            return Err(KanbanError::InvalidInput(
                "priority 必须在 0 到 3 之间".to_owned(),
            ));
        }
        if command
            .max_retries
            .flatten()
            .is_some_and(|max_retries| max_retries <= 0)
        {
            return Err(KanbanError::InvalidInput(
                "max_retries 必须大于 0".to_owned(),
            ));
        }
        // 先序列化并校验 metadata，再获取 mutation gate，保证非法 JSON 没有副作用。
        let metadata_json = command
            .metadata
            .map(|metadata| match metadata {
                Some(value) => serde_json::to_string(&value)
                    .map_err(|error| KanbanError::InvalidInput(format!("metadata 无效: {error}"))),
                None => Ok("null".to_owned()),
            })
            .transpose()?;

        let _mutation = self.mutation_gate.lock().await;
        let current = self.store.get_task(task_id).await?;
        let expected_lock_version = command
            .expected_lock_version
            .unwrap_or(current.lock_version);
        self.store
            .update_task(
                task_id,
                UpdateTaskRecord {
                    expected_lock_version,
                    actor: actor.to_owned(),
                    title: command.title.map(|title| title.trim().to_owned()),
                    description: command.description.map(|description| {
                        description.map(|description| description.trim().to_owned())
                    }),
                    assignee: command
                        .assignee
                        .map(|assignee| assignee.map(|assignee| assignee.trim().to_owned())),
                    priority: command.priority,
                    scheduled_at: command.scheduled_at,
                    due_at: command.due_at,
                    max_retries: command.max_retries,
                    metadata_json,
                    event_id: new_event_id(),
                    now: self.clock.now_ms(),
                },
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicUsize};

    use kanban_core::{KanbanError, Result};

    use crate::operations::test_support::{FixedClock, StubStore, task_for_id};
    use crate::*;

    impl TaskUpdate for StubStore {
        async fn get_task(&self, task_id: &str) -> Result<TaskRecord> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(task_for_id(task_id))
        }

        async fn update_task(&self, task_id: &str, input: UpdateTaskRecord) -> Result<TaskRecord> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(task_id, "t_update");
            assert_eq!(input.expected_lock_version, 0);
            assert_eq!(input.metadata_json.as_deref(), Some("null"));
            assert_eq!(input.description, Some(None));
            let mut task = task_for_id(task_id);
            task.title = input.title.expect("title");
            task.description = input.description.flatten();
            task.metadata_json = input.metadata_json.expect("metadata");
            task.lock_version += 1;
            Ok(task)
        }
    }

    #[tokio::test]
    async fn update_defaults_expected_lock_and_preserves_explicit_null() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::clone(&calls),
            },
            FixedClock(100),
        );
        let updated = service
            .update_task(UpdateTaskCommand {
                task_id: " t_update ".into(),
                actor: " editor ".into(),
                expected_lock_version: None,
                title: Some(" Updated ".into()),
                description: Some(None),
                assignee: None,
                priority: None,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata: Some(None),
            })
            .await
            .expect("update task");
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.description, None);
        assert_eq!(updated.metadata_json, "null");
        assert_eq!(updated.lock_version, 1);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn update_rejects_non_positive_max_retries_before_store_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::clone(&calls),
            },
            FixedClock(100),
        );
        let result = service
            .update_task(UpdateTaskCommand {
                task_id: "t_update".into(),
                actor: "editor".into(),
                expected_lock_version: None,
                title: None,
                description: None,
                assignee: None,
                priority: None,
                scheduled_at: None,
                due_at: None,
                max_retries: Some(Some(0)),
                metadata: None,
            })
            .await;
        assert!(matches!(result, Err(KanbanError::InvalidInput(_))));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}
