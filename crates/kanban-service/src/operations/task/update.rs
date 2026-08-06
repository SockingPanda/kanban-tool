use kanban_core::{Clock, KanbanError, Result, new_event_id};

use crate::{KanbanService, TaskRecord};

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

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn update_task(&self, command: UpdateTaskCommand) -> Result<TaskRecord> {
        validate_update_task(&command)?;
        let task_id = command.task_id.trim();
        let actor = command.actor.trim();
        // 先序列化并校验 metadata，再获取 mutation gate，保证非法 JSON 没有副作用。
        let metadata_json = normalize_metadata(command.metadata)?;

        let _mutation = self.mutation_gate.lock().await;
        let current = self.get_task(task_id).await?;
        let expected_lock_version = command
            .expected_lock_version
            .unwrap_or(current.lock_version);
        self.store
            .update_task(
                task_id,
                crate::store_operations::UpdateTaskInput {
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
            .map_err(crate::error::store_error)
            .and_then(super::application_task)
    }
}

fn validate_update_task(command: &UpdateTaskCommand) -> Result<()> {
    let task_id = command.task_id.trim();
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(KanbanError::InvalidInput(
            "task_id 必须是全局 t_... ID".to_owned(),
        ));
    }
    if command.actor.trim().is_empty() {
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
    Ok(())
}

fn normalize_metadata(metadata: Option<Option<serde_json::Value>>) -> Result<Option<String>> {
    metadata
        .map(|metadata| match metadata {
            Some(value) => serde_json::to_string(&value)
                .map_err(|error| KanbanError::InvalidInput(format!("metadata 无效: {error}"))),
            None => Ok("null".to_owned()),
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use kanban_core::{Clock, KanbanError};

    use super::{UpdateTaskCommand, normalize_metadata, validate_update_task};

    #[derive(Clone, Copy)]
    struct FixedClock(i64);

    impl Clock for FixedClock {
        fn now_ms(&self) -> i64 {
            self.0
        }
    }

    fn command() -> UpdateTaskCommand {
        UpdateTaskCommand {
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
        }
    }

    #[test]
    fn update_normalizes_nullable_fields_and_metadata() {
        validate_update_task(&command()).unwrap();
        assert_eq!(
            normalize_metadata(Some(None)).unwrap().as_deref(),
            Some("null")
        );
    }

    #[test]
    fn update_rejects_non_positive_max_retries_before_store_call() {
        let mut command = command();
        command.title = None;
        command.description = None;
        command.metadata = None;
        command.max_retries = Some(Some(0));
        let error = validate_update_task(&command).unwrap_err();
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn concrete_task_crud_covers_idempotency_isolation_filters_and_cas() {
        let (_directory, store, _path) = crate::test_support::store("task-crud-service").await;
        store.initialize().await.expect("initialize");
        let service = crate::KanbanService::with_clock(store, FixedClock(100));
        let secondary = service
            .create_board(crate::CreateBoardCommand {
                slug: "secondary".into(),
                name: "Secondary".into(),
                description: None,
                actor: "tester".into(),
            })
            .await
            .expect("create secondary board");

        let command = crate::CreateTaskCommand {
            task_id: "t_service_crud".into(),
            board: " default ".into(),
            idempotency_key: Some("service-crud-1".into()),
            title: " Service CRUD ".into(),
            description: Some("ready spec".into()),
            requested_status: Some(crate::TaskStatus::Todo),
            assignee: Some("worker".into()),
            priority: 2,
            scheduled_at: None,
            due_at: None,
            max_retries: Some(2),
            metadata: std::collections::BTreeMap::from([(
                "source".into(),
                serde_json::json!("service-test"),
            )]),
            labels: Vec::new(),
            depends_on: Vec::new(),
            actor: "tester".into(),
        };
        let created = service
            .create_task(command.clone())
            .await
            .expect("create task");
        let replayed = service
            .create_task(command.clone())
            .await
            .expect("idempotent replay");
        assert_eq!(created.id, replayed.id);
        assert_eq!(created.title, "Service CRUD");
        assert_eq!(created.status, crate::TaskStatus::Todo);

        let conflict = service
            .create_task(crate::CreateTaskCommand {
                title: "Different payload".into(),
                ..command.clone()
            })
            .await
            .expect_err("different idempotent payload");
        assert!(matches!(
            conflict,
            crate::KanbanError::IdempotencyConflict(_)
        ));

        let secondary_task = service
            .create_task(crate::CreateTaskCommand {
                task_id: "t_secondary_crud".into(),
                board: secondary.slug.clone(),
                idempotency_key: None,
                title: "Secondary task".into(),
                description: Some("triage spec".into()),
                requested_status: Some(crate::TaskStatus::Triage),
                assignee: None,
                priority: 1,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata: std::collections::BTreeMap::new(),
                labels: Vec::new(),
                depends_on: Vec::new(),
                actor: "tester".into(),
            })
            .await
            .expect("create secondary task");
        assert_eq!(secondary_task.board_slug, "secondary");

        let shown = service.get_task(&created.id).await.expect("show task");
        assert_eq!(shown.id, created.id);
        let default_page = service
            .list_tasks("default", crate::TaskListOptions::default())
            .await
            .expect("list default board");
        assert!(default_page.tasks.iter().any(|task| task.id == created.id));
        assert!(
            !default_page
                .tasks
                .iter()
                .any(|task| task.id == secondary_task.id)
        );
        let todo_page = service
            .list_tasks(
                "default",
                crate::TaskListOptions {
                    statuses: vec![crate::TaskStatus::Todo],
                    ..crate::TaskListOptions::default()
                },
            )
            .await
            .expect("filter todo tasks");
        assert!(
            todo_page
                .tasks
                .iter()
                .all(|task| task.status == crate::TaskStatus::Todo)
        );
        assert!(todo_page.tasks.iter().any(|task| task.id == created.id));

        let updated = service
            .update_task(crate::UpdateTaskCommand {
                task_id: created.id.clone(),
                actor: " editor ".into(),
                expected_lock_version: None,
                title: Some(" Updated ".into()),
                description: Some(None),
                assignee: None,
                priority: None,
                scheduled_at: None,
                due_at: Some(None),
                max_retries: None,
                metadata: Some(None),
            })
            .await
            .expect("nullable update");
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.description, None);
        assert_eq!(updated.metadata_json, "null");
        assert_eq!(updated.due_at, None);
        assert_eq!(updated.lock_version, 1);

        let stale = service
            .update_task(crate::UpdateTaskCommand {
                task_id: created.id,
                actor: "editor".into(),
                expected_lock_version: Some(0),
                title: Some("stale".into()),
                description: None,
                assignee: None,
                priority: None,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata: None,
            })
            .await
            .expect_err("stale compare-and-set");
        assert!(matches!(stale, crate::KanbanError::InvalidTransition(_)));
    }
}
