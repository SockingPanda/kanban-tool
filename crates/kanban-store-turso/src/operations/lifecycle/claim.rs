use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimTaskInput {
    pub expected_lock_version: i64,
    pub owner: String,
    pub claim_token: String,
    pub run_id: String,
    pub event_id: String,
    pub worker_profile: String,
    pub metadata_json: String,
    pub log_path: Option<String>,
    pub now: i64,
    pub claim_expires_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimTaskRecord {
    pub task: TaskRecord,
    pub run: TaskRunRecord,
    pub claim_token: String,
    pub claim_expires_at: i64,
}
impl TursoStore {
    pub async fn claim_task(
        &self,
        task_id: &str,
        input: ClaimTaskInput,
    ) -> Result<ClaimTaskRecord, StoreError> {
        validate_claim_task_input(task_id, &input)?;
        let owner = input.owner.trim().to_owned();
        let claim_token = input.claim_token.trim().to_owned();
        let run_id = input.run_id.trim().to_owned();
        let event_id = input.event_id.trim().to_owned();
        let worker_profile = input.worker_profile.trim().to_owned();
        let log_path = input.log_path.as_deref().map(str::trim).map(str::to_owned);
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let metadata_valid = first_row(
            transaction
                .query(
                    "SELECT json_valid(:metadata_json)",
                    [(":metadata_json", input.metadata_json.as_str())],
                )
                .await?,
        )
        .await?;
        if integer_value(
            metadata_valid.get_value(0)?,
            "task_runs.metadata_json_valid",
        )? == 0
        {
            return Err(StoreError::InvalidInput(
                "metadata_json must be valid JSON".to_owned(),
            ));
        }

        let task = first_row(
                transaction
                    .query(
                        "SELECT t.board_id, t.status, t.archived_at, b.archived_at, t.lock_version, t.title, t.description, t.scheduled_at, t.claim_token, t.claim_owner, t.claim_expires_at, t.current_run_id FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                        [(":task_id", task_id)],
                    )
                    .await?,
            )
            .await
            .map_err(|error| match error {
                turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
                other => StoreError::Turso(other),
            })?;
        let board_id = text_value(task.get_value(0)?, "tasks.board_id")?;
        let status = text_value(task.get_value(1)?, "tasks.status")?;
        let task_archived_at = optional_integer_value(task.get_value(2)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(3)?, "boards.archived_at")?;
        if status == "archived" || task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot be claimed".to_owned(),
            ));
        }

        let lock_version = integer_value(task.get_value(4)?, "tasks.lock_version")?;
        if lock_version != input.expected_lock_version {
            return Err(StoreError::ClaimConflict(
                "lock_version mismatch".to_owned(),
            ));
        }
        if status != "ready" {
            return Err(StoreError::InvalidTransition(
                "task is not ready".to_owned(),
            ));
        }
        let existing_claim_token = optional_text_value(task.get_value(8)?, "tasks.claim_token")?;
        let existing_claim_owner = optional_text_value(task.get_value(9)?, "tasks.claim_owner")?;
        let existing_claim_expires_at =
            optional_integer_value(task.get_value(10)?, "tasks.claim_expires_at")?;
        let existing_run_id = optional_text_value(task.get_value(11)?, "tasks.current_run_id")?;
        if existing_claim_token.is_some()
            || existing_claim_owner.is_some()
            || existing_claim_expires_at.is_some()
            || existing_run_id.is_some()
        {
            return Err(StoreError::ClaimConflict(
                "task is already claimed".to_owned(),
            ));
        }

        let title = text_value(task.get_value(5)?, "tasks.title")?;
        let description = optional_text_value(task.get_value(6)?, "tasks.description")?;
        if title.trim().is_empty()
            || description
                .as_deref()
                .is_none_or(|description| description.trim().is_empty())
        {
            return Err(StoreError::InvalidTransition(
                "task spec is incomplete".to_owned(),
            ));
        }
        let scheduled_at = optional_integer_value(task.get_value(7)?, "tasks.scheduled_at")?;
        if scheduled_at.is_some_and(|scheduled_at| scheduled_at > input.now) {
            return Err(StoreError::InvalidTransition(
                "scheduled_at is in the future".to_owned(),
            ));
        }

        let dependency_blocked = first_row(
                transaction
                    .query(
                        "SELECT EXISTS (SELECT 1 FROM task_dependencies AS d JOIN tasks AS p ON p.id = d.parent_task_id AND p.board_id = d.board_id WHERE d.board_id = :board_id AND d.child_task_id = :task_id AND p.status NOT IN ('done', 'archived'))",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
        if integer_value(
            dependency_blocked.get_value(0)?,
            "task_dependencies.unfinished_parent",
        )? != 0
        {
            return Err(StoreError::InvalidTransition(
                "dependency blocked".to_owned(),
            ));
        }

        let execution_plan_ready = first_row(
                transaction
                    .query(
                        "SELECT EXISTS (SELECT 1 FROM task_steps AS s WHERE s.board_id = :board_id AND s.parent_task_id = :task_id) OR EXISTS (SELECT 1 FROM task_execution_plans AS ep WHERE ep.board_id = :board_id AND ep.task_id = :task_id AND ep.state = 'not_required')",
                        [
                            (":board_id", board_id.as_str()),
                            (":task_id", task_id),
                        ],
                    )
                    .await?,
            )
            .await?;
        if integer_value(
            execution_plan_ready.get_value(0)?,
            "task_execution_plans.ready",
        )? == 0
        {
            return Err(StoreError::InvalidTransition(
                "execution plan is required".to_owned(),
            ));
        }

        let changed = transaction
                .execute(
                    "UPDATE tasks SET status = 'running', claim_owner = :claim_owner, claim_token = :claim_token, claim_expires_at = :claim_expires_at, last_heartbeat_at = :last_heartbeat_at, current_run_id = :current_run_id, started_at = COALESCE(started_at, :started_at), updated_at = :updated_at, lock_version = lock_version + 1 WHERE id = :task_id AND board_id = :board_id AND status = 'ready' AND claim_token IS NULL AND claim_owner IS NULL AND claim_expires_at IS NULL AND current_run_id IS NULL AND lock_version = :expected_lock_version",
                    (
                        (":claim_owner", owner.as_str()),
                        (":claim_token", claim_token.as_str()),
                        (":claim_expires_at", input.claim_expires_at),
                        (":last_heartbeat_at", input.now),
                        (":current_run_id", run_id.as_str()),
                        (":started_at", input.now),
                        (":updated_at", input.now),
                        (":task_id", task_id),
                        (":board_id", board_id.as_str()),
                        (":expected_lock_version", input.expected_lock_version),
                    ),
                )
                .await?;
        if changed != 1 {
            return Err(StoreError::ClaimConflict(
                "claim compare-and-set failed".to_owned(),
            ));
        }

        transaction
                .execute(
                    "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, log_path, metadata_json) VALUES (:run_id, :board_id, :task_id, 'running', :worker_profile, NULL, :claim_token, :claim_owner, :claim_expires_at, :started_at, :last_heartbeat_at, :log_path, :metadata_json)",
                    (
                        (":run_id", run_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":worker_profile", worker_profile.as_str()),
                        (":claim_token", claim_token.as_str()),
                        (":claim_owner", owner.as_str()),
                        (":claim_expires_at", input.claim_expires_at),
                        (":started_at", input.now),
                        (":last_heartbeat_at", input.now),
                        (":log_path", log_path.as_deref()),
                        (":metadata_json", input.metadata_json.as_str()),
                    ),
                )
                .await?;

        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, 'task.claimed', :actor, json_object('claim_owner', :claim_owner, 'metadata', json(:metadata_json)), :created_at)",
                    (
                        (":event_id", event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":run_id", run_id.as_str()),
                        (":actor", owner.as_str()),
                        (":claim_owner", owner.as_str()),
                        (":metadata_json", input.metadata_json.as_str()),
                        (":created_at", input.now),
                    ),
                )
                .await?;

        let claimed_task = task_from_row(
            first_row(
                transaction
                    .query(
                        &format!(
                            "{TASK_SELECT} WHERE t.board_id = :board_id AND t.id = :task_id LIMIT 1"
                        ),
                        [(":board_id", board_id.as_str()), (":task_id", task_id)],
                    )
                    .await?,
            )
            .await?,
        )?;
        let run = run_from_row(
                first_row(
                    transaction
                        .query(
                            "SELECT id, board_id, task_id, status, worker_profile, worker_pid, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, finished_at, exit_code, summary, error, log_path, metadata_json FROM task_runs WHERE board_id = :board_id AND id = :run_id LIMIT 1",
                            [(":board_id", board_id.as_str()), (":run_id", run_id.as_str())],
                        )
                        .await?,
                )
                .await?,
            )?;

        transaction.commit().await?;
        Ok(ClaimTaskRecord {
            task: claimed_task,
            run,
            claim_token,
            claim_expires_at: input.claim_expires_at,
        })
    }
}

pub(crate) fn validate_claim_task_input(
    task_id: &str,
    input: &ClaimTaskInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if input.expected_lock_version < 0 {
        return Err(StoreError::InvalidInput(
            "expected_lock_version must be non-negative".to_owned(),
        ));
    }
    if input.owner.trim().is_empty() {
        return Err(StoreError::InvalidInput("owner is required".to_owned()));
    }
    if input.claim_token.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "claim_token is required".to_owned(),
        ));
    }
    if !input.run_id.trim().starts_with("r_") || input.run_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "run_id must start with r_".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.worker_profile.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "worker_profile is required".to_owned(),
        ));
    }
    if input
        .log_path
        .as_deref()
        .is_some_and(|log_path| log_path.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "log_path must not be empty".to_owned(),
        ));
    }
    if input.now < 0 {
        return Err(StoreError::InvalidInput(
            "now must be non-negative".to_owned(),
        ));
    }
    if input.claim_expires_at <= input.now {
        return Err(StoreError::InvalidInput(
            "claim_expires_at must be after now".to_owned(),
        ));
    }
    Ok(())
}
