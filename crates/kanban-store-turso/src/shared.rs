use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use turso::Value;
use turso::{Connection, Row, Rows};

use crate::{domain::*, error::StoreError};

pub(crate) async fn first_row(mut rows: Rows) -> Result<Row, turso::Error> {
    let row = rows
        .next()
        .await?
        .ok_or(turso::Error::QueryReturnedNoRows)?;
    while rows.next().await?.is_some() {}
    Ok(row)
}

pub(crate) const TASK_SELECT: &str = "SELECT t.id, t.board_id, t.seq, t.idempotency_key, t.title, t.description, t.status, t.status_reason, t.assignee, t.priority, t.position, t.scheduled_at, t.due_at, t.created_by, t.created_at, t.updated_at, t.started_at, t.completed_at, t.archived_at, t.claim_token, t.claim_owner, t.claim_expires_at, t.last_heartbeat_at, t.current_run_id, t.retry_count, t.max_retries, t.result_summary, t.result_json, t.metadata_json, t.lock_version, b.slug, EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = t.board_id AND d.child_task_id = t.id AND p.status NOT IN ('done', 'archived')) AS dependency_blocked, (SELECT COUNT(*) FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = t.board_id AND d.child_task_id = t.id AND p.status NOT IN ('done', 'archived')) AS unfinished_parent_count, CASE WHEN EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id) THEN 'planned' WHEN EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = t.board_id AND ep.task_id = t.id AND ep.state = 'not_required') THEN 'not_required' ELSE 'unplanned' END AS execution_plan_state, (SELECT COUNT(*) FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 1) AS required_step_count, (SELECT COUNT(*) FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 1 AND s.status IN ('done', 'skipped')) AS completed_required_step_count, (SELECT COUNT(*) FROM task_steps AS s WHERE s.board_id = t.board_id AND s.parent_task_id = t.id AND s.required = 0) AS optional_step_count FROM tasks AS t JOIN boards AS b ON b.id = t.board_id";
pub(crate) fn task_from_row(row: Row) -> Result<TaskRecord, StoreError> {
    let board_slug = text_value(row.get_value(30)?, "boards.slug")?;
    let seq = integer_value(row.get_value(2)?, "tasks.seq")?;
    Ok(TaskRecord {
        id: text_value(row.get_value(0)?, "tasks.id")?,
        board_id: text_value(row.get_value(1)?, "tasks.board_id")?,
        board_slug: board_slug.clone(),
        task_ref: format!("{board_slug}#{seq}"),
        seq,
        idempotency_key: optional_text_value(row.get_value(3)?, "tasks.idempotency_key")?,
        title: text_value(row.get_value(4)?, "tasks.title")?,
        description: optional_text_value(row.get_value(5)?, "tasks.description")?,
        status: text_value(row.get_value(6)?, "tasks.status")?,
        status_reason: optional_text_value(row.get_value(7)?, "tasks.status_reason")?,
        assignee: optional_text_value(row.get_value(8)?, "tasks.assignee")?,
        priority: integer_value(row.get_value(9)?, "tasks.priority")?,
        position: integer_value(row.get_value(10)?, "tasks.position")?,
        scheduled_at: optional_integer_value(row.get_value(11)?, "tasks.scheduled_at")?,
        due_at: optional_integer_value(row.get_value(12)?, "tasks.due_at")?,
        created_by: text_value(row.get_value(13)?, "tasks.created_by")?,
        created_at: integer_value(row.get_value(14)?, "tasks.created_at")?,
        updated_at: integer_value(row.get_value(15)?, "tasks.updated_at")?,
        started_at: optional_integer_value(row.get_value(16)?, "tasks.started_at")?,
        completed_at: optional_integer_value(row.get_value(17)?, "tasks.completed_at")?,
        archived_at: optional_integer_value(row.get_value(18)?, "tasks.archived_at")?,
        claim_token: optional_text_value(row.get_value(19)?, "tasks.claim_token")?,
        claim_owner: optional_text_value(row.get_value(20)?, "tasks.claim_owner")?,
        claim_expires_at: optional_integer_value(row.get_value(21)?, "tasks.claim_expires_at")?,
        last_heartbeat_at: optional_integer_value(row.get_value(22)?, "tasks.last_heartbeat_at")?,
        current_run_id: optional_text_value(row.get_value(23)?, "tasks.current_run_id")?,
        retry_count: integer_value(row.get_value(24)?, "tasks.retry_count")?,
        max_retries: optional_integer_value(row.get_value(25)?, "tasks.max_retries")?,
        result_summary: optional_text_value(row.get_value(26)?, "tasks.result_summary")?,
        result_json: optional_text_value(row.get_value(27)?, "tasks.result_json")?,
        metadata_json: text_value(row.get_value(28)?, "tasks.metadata_json")?,
        lock_version: integer_value(row.get_value(29)?, "tasks.lock_version")?,
        dependency_blocked: integer_value(row.get_value(31)?, "tasks.dependency_blocked")? != 0,
        unfinished_parent_count: integer_value(
            row.get_value(32)?,
            "tasks.unfinished_parent_count",
        )?,
        execution_plan_state: text_value(row.get_value(33)?, "tasks.execution_plan_state")?,
        required_step_count: integer_value(row.get_value(34)?, "tasks.required_step_count")?,
        completed_required_step_count: integer_value(
            row.get_value(35)?,
            "tasks.completed_required_step_count",
        )?,
        optional_step_count: integer_value(row.get_value(36)?, "tasks.optional_step_count")?,
        labels: Vec::new(),
    })
}

pub(crate) fn comment_from_row(row: Row) -> Result<CommentRecord, StoreError> {
    Ok(CommentRecord {
        id: text_value(row.get_value(0)?, "task_comments.id")?,
        board_id: text_value(row.get_value(1)?, "task_comments.board_id")?,
        task_id: text_value(row.get_value(2)?, "task_comments.task_id")?,
        idempotency_key: optional_text_value(row.get_value(3)?, "task_comments.idempotency_key")?,
        author: text_value(row.get_value(4)?, "task_comments.author")?,
        author_type: text_value(row.get_value(5)?, "task_comments.author_type")?,
        agent_type: optional_text_value(row.get_value(6)?, "task_comments.agent_type")?,
        body: text_value(row.get_value(7)?, "task_comments.body")?,
        kind: text_value(row.get_value(8)?, "task_comments.kind")?,
        metadata_json: text_value(row.get_value(9)?, "task_comments.metadata_json")?,
        created_at: integer_value(row.get_value(10)?, "task_comments.created_at")?,
    })
}

pub(crate) async fn step_from_row(
    connection: &Connection,
    row: Row,
) -> Result<TaskStepRecord, StoreError> {
    let board_id = text_value(row.get_value(1)?, "task_steps.board_id")?;
    let linked_task_id = optional_text_value(row.get_value(6)?, "task_steps.linked_task_id")?;
    let linked_task = if let Some(linked_task_id) = linked_task_id {
        let linked_row = first_row(
            connection
                .query(
                    &format!(
                        "{TASK_SELECT} WHERE t.board_id = :board_id AND t.id = :task_id LIMIT 1"
                    ),
                    [
                        (":board_id", board_id.as_str()),
                        (":task_id", linked_task_id.as_str()),
                    ],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(linked_task_id.clone()),
            other => StoreError::Turso(other),
        })?;
        Some(task_from_row(linked_row)?)
    } else {
        None
    };
    Ok(TaskStepRecord {
        id: text_value(row.get_value(0)?, "task_steps.id")?,
        board_id,
        parent_task_id: text_value(row.get_value(2)?, "task_steps.parent_task_id")?,
        position: integer_value(row.get_value(3)?, "task_steps.position")?,
        title: text_value(row.get_value(4)?, "task_steps.title")?,
        body: optional_text_value(row.get_value(5)?, "task_steps.body")?,
        linked_task,
        required: integer_value(row.get_value(7)?, "task_steps.required")? != 0,
        status: text_value(row.get_value(8)?, "task_steps.status")?,
        resolution_note: optional_text_value(row.get_value(9)?, "task_steps.resolution_note")?,
        resolved_by: optional_text_value(row.get_value(10)?, "task_steps.resolved_by")?,
        resolved_at: optional_integer_value(row.get_value(11)?, "task_steps.resolved_at")?,
        created_by: text_value(row.get_value(12)?, "task_steps.created_by")?,
        created_at: integer_value(row.get_value(13)?, "task_steps.created_at")?,
        updated_by: text_value(row.get_value(14)?, "task_steps.updated_by")?,
        updated_at: integer_value(row.get_value(15)?, "task_steps.updated_at")?,
    })
}

pub(crate) fn run_from_row(row: Row) -> Result<TaskRunRecord, StoreError> {
    Ok(TaskRunRecord {
        id: text_value(row.get_value(0)?, "task_runs.id")?,
        board_id: text_value(row.get_value(1)?, "task_runs.board_id")?,
        task_id: text_value(row.get_value(2)?, "task_runs.task_id")?,
        status: text_value(row.get_value(3)?, "task_runs.status")?,
        worker_profile: optional_text_value(row.get_value(4)?, "task_runs.worker_profile")?,
        worker_pid: optional_integer_value(row.get_value(5)?, "task_runs.worker_pid")?,
        claim_token: text_value(row.get_value(6)?, "task_runs.claim_token")?,
        claim_owner: text_value(row.get_value(7)?, "task_runs.claim_owner")?,
        claim_expires_at: integer_value(row.get_value(8)?, "task_runs.claim_expires_at")?,
        started_at: integer_value(row.get_value(9)?, "task_runs.started_at")?,
        last_heartbeat_at: optional_integer_value(
            row.get_value(10)?,
            "task_runs.last_heartbeat_at",
        )?,
        finished_at: optional_integer_value(row.get_value(11)?, "task_runs.finished_at")?,
        exit_code: optional_integer_value(row.get_value(12)?, "task_runs.exit_code")?,
        summary: optional_text_value(row.get_value(13)?, "task_runs.summary")?,
        error: optional_text_value(row.get_value(14)?, "task_runs.error")?,
        log_path: optional_text_value(row.get_value(15)?, "task_runs.log_path")?,
        metadata_json: text_value(row.get_value(16)?, "task_runs.metadata_json")?,
    })
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

pub(crate) fn text_value(value: Value, field: &'static str) -> Result<String, StoreError> {
    match value {
        Value::Text(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

pub(crate) fn optional_text_value(
    value: Value,
    field: &'static str,
) -> Result<Option<String>, StoreError> {
    match value {
        Value::Text(value) => Ok(Some(value)),
        Value::Null => Ok(None),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

pub(crate) fn integer_value(value: Value, field: &'static str) -> Result<i64, StoreError> {
    match value {
        Value::Integer(value) => Ok(value),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

pub(crate) fn optional_integer_value(
    value: Value,
    field: &'static str,
) -> Result<Option<i64>, StoreError> {
    match value {
        Value::Integer(value) => Ok(Some(value)),
        Value::Null => Ok(None),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}
