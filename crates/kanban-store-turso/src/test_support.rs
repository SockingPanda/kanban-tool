use std::path::PathBuf;

use turso::Connection;

use crate::TaskRecord;
pub(crate) use crate::shared::{Value, first_row, integer_value, optional_text_value, text_value};
pub(crate) use crate::{
    AddDependencyInput, ArchiveTaskInput, BlockTaskInput, ClaimTaskInput, CompleteTaskInput,
    CreateCommentInput, CreateStepInput, CreateTaskInput, HeartbeatTaskInput,
    MarkExecutionPlanNotRequiredInput, PromoteTaskInput, ReclaimExpiredTaskInput, ReclaimTaskInput,
    ReleaseTaskInput, RemoveDependencyInput, ReopenTaskInput, SpecifyTaskInput, StoreError,
    SubmitReviewTaskInput, TaskExecutionPlanRecord, TaskListOptions, TaskListSort, TaskPlanFilter,
    TursoStore, UnblockTaskInput, UpdateStepInput, UpdateTaskInput,
};

pub(crate) async fn store(name: &str) -> (tempfile::TempDir, TursoStore, PathBuf) {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join(format!("{name}.db"));
    let store = TursoStore::open(&path).await.expect("open Turso database");
    (directory, store, path)
}

pub(crate) fn create_input(
    id: &str,
    idempotency_key: Option<&str>,
    title: &str,
) -> CreateTaskInput {
    CreateTaskInput {
        id: id.to_owned(),
        idempotency_key: idempotency_key.map(str::to_owned),
        title: title.to_owned(),
        status: "todo".to_owned(),
        description: Some("description".to_owned()),
        assignee: Some("agent".to_owned()),
        priority: 1,
        scheduled_at: Some(100),
        due_at: Some(200),
        max_retries: Some(2),
        metadata_json: r#"{"source":"test"}"#.to_owned(),
        labels: Vec::new(),
        depends_on: Vec::new(),
        created_by: "tester".to_owned(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn comment_input(
    id: &str,
    idempotency_key: Option<&str>,
    author: &str,
    author_type: &str,
    agent_type: Option<&str>,
    body: &str,
    kind: &str,
    metadata_json: &str,
    event_id: &str,
    created_at: i64,
) -> CreateCommentInput {
    CreateCommentInput {
        id: id.to_owned(),
        idempotency_key: idempotency_key.map(str::to_owned),
        author: author.to_owned(),
        author_type: author_type.to_owned(),
        agent_type: agent_type.map(str::to_owned),
        body: body.to_owned(),
        kind: kind.to_owned(),
        metadata_json: metadata_json.to_owned(),
        event_id: event_id.to_owned(),
        created_at,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn step_input(
    id: &str,
    key: Option<&str>,
    title: &str,
    position: Option<i64>,
    actor: &str,
    expected_lock_version: i64,
    expected_plan_state: &str,
    target_status: &str,
    event_id: &str,
    plan_event_id: &str,
    recompute_event_id: &str,
    created_at: i64,
) -> CreateStepInput {
    CreateStepInput {
        id: id.to_owned(),
        idempotency_key: key.map(str::to_owned),
        title: title.to_owned(),
        body: Some("body".to_owned()),
        linked_task_id: None,
        position,
        required: true,
        created_by: actor.to_owned(),
        event_id: event_id.to_owned(),
        plan_event_id: plan_event_id.to_owned(),
        recompute_event_id: recompute_event_id.to_owned(),
        created_at,
        expected_lock_version,
        expected_plan_state: expected_plan_state.to_owned(),
        target_status: target_status.to_owned(),
    }
}

pub(crate) fn plan_input(
    reason: &str,
    actor: &str,
    event_id: &str,
    updated_at: i64,
) -> MarkExecutionPlanNotRequiredInput {
    MarkExecutionPlanNotRequiredInput {
        reason: reason.to_owned(),
        actor: actor.to_owned(),
        event_id: event_id.to_owned(),
        updated_at,
    }
}

pub(crate) fn promote_input(
    expected_lock_version: i64,
    actor: &str,
    event_id: &str,
    updated_at: i64,
) -> PromoteTaskInput {
    PromoteTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        event_id: event_id.to_owned(),
        updated_at,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn claim_input(
    expected_lock_version: i64,
    owner: &str,
    claim_token: &str,
    run_id: &str,
    event_id: &str,
    metadata_json: &str,
    now: i64,
    ttl_ms: i64,
) -> ClaimTaskInput {
    ClaimTaskInput {
        expected_lock_version,
        owner: owner.to_owned(),
        claim_token: claim_token.to_owned(),
        run_id: run_id.to_owned(),
        event_id: event_id.to_owned(),
        worker_profile: "manual".to_owned(),
        metadata_json: metadata_json.to_owned(),
        log_path: None,
        now,
        claim_expires_at: now.saturating_add(ttl_ms),
    }
}

pub(crate) fn heartbeat_input(
    expected_lock_version: i64,
    actor: &str,
    claim_token: &str,
    event_id: &str,
    note: Option<&str>,
    now: i64,
    claim_expires_at: i64,
) -> HeartbeatTaskInput {
    HeartbeatTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        claim_token: claim_token.to_owned(),
        event_id: event_id.to_owned(),
        note: note.map(str::to_owned),
        now,
        claim_expires_at,
    }
}

pub(crate) fn release_input(
    expected_lock_version: i64,
    actor: &str,
    claim_token: &str,
    event_id: &str,
    now: i64,
) -> ReleaseTaskInput {
    ReleaseTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        claim_token: claim_token.to_owned(),
        event_id: event_id.to_owned(),
        now,
    }
}

pub(crate) fn reclaim_input(
    expected_lock_version: i64,
    actor: &str,
    event_id: &str,
    target_status: &str,
    retry_count: i64,
    reason: &str,
    now: i64,
) -> ReclaimExpiredTaskInput {
    ReclaimExpiredTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        event_id: event_id.to_owned(),
        target_status: target_status.to_owned(),
        retry_count,
        reason: reason.to_owned(),
        now,
    }
}

pub(crate) fn submit_review_input(
    expected_lock_version: i64,
    actor: &str,
    claim_token: Option<&str>,
    force: bool,
    summary: Option<&str>,
    now: i64,
    event_id: &str,
) -> SubmitReviewTaskInput {
    SubmitReviewTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        claim_token: claim_token.map(str::to_owned),
        force,
        summary: summary.map(str::to_owned),
        now,
        event_id: event_id.to_owned(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn complete_input(
    expected_lock_version: i64,
    actor: &str,
    claim_token: Option<&str>,
    force: bool,
    summary: Option<&str>,
    result_json: Option<&str>,
    now: i64,
    event_id: &str,
) -> CompleteTaskInput {
    CompleteTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        claim_token: claim_token.map(str::to_owned),
        force,
        summary: summary.map(str::to_owned),
        result_json: result_json.map(str::to_owned),
        now,
        event_id: event_id.to_owned(),
    }
}

pub(crate) fn block_input(
    expected_lock_version: i64,
    actor: &str,
    claim_token: Option<&str>,
    force: bool,
    reason: &str,
    now: i64,
    event_id: &str,
) -> BlockTaskInput {
    BlockTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        claim_token: claim_token.map(str::to_owned),
        force,
        reason: reason.to_owned(),
        now,
        event_id: event_id.to_owned(),
    }
}

pub(crate) fn specify_input(
    expected_lock_version: i64,
    actor: &str,
    description: Option<&str>,
    scheduled_at: Option<i64>,
    event_id: &str,
    now: i64,
) -> SpecifyTaskInput {
    SpecifyTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        description: description.map(str::to_owned),
        scheduled_at,
        event_id: event_id.to_owned(),
        now,
    }
}

pub(crate) fn unblock_input(
    expected_lock_version: i64,
    actor: &str,
    event_id: &str,
    now: i64,
) -> UnblockTaskInput {
    UnblockTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        event_id: event_id.to_owned(),
        now,
    }
}

pub(crate) fn reopen_input(
    expected_lock_version: i64,
    actor: &str,
    reason: &str,
    event_id: &str,
    now: i64,
) -> ReopenTaskInput {
    ReopenTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        reason: reason.to_owned(),
        event_id: event_id.to_owned(),
        now,
    }
}

pub(crate) fn archive_input(
    expected_lock_version: i64,
    actor: &str,
    force: bool,
    event_id: &str,
    now: i64,
) -> ArchiveTaskInput {
    ArchiveTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        force,
        event_id: event_id.to_owned(),
        now,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn explicit_reclaim_input(
    expected_lock_version: i64,
    actor: &str,
    force: bool,
    target_status: &str,
    retry_count: i64,
    reason: &str,
    event_id: &str,
    now: i64,
) -> ReclaimTaskInput {
    ReclaimTaskInput {
        expected_lock_version,
        actor: actor.to_owned(),
        force,
        target_status: target_status.to_owned(),
        retry_count,
        reason: reason.to_owned(),
        event_id: event_id.to_owned(),
        now,
    }
}

pub(crate) async fn ready_task_for_claim(
    store: &TursoStore,
    task_id: &str,
    idempotency_key: &str,
    title: &str,
) -> TaskRecord {
    store
        .create_task(
            "default",
            create_input(task_id, Some(idempotency_key), title),
        )
        .await
        .expect("create claim task");
    store
        .mark_execution_plan_not_required(
            task_id,
            plan_input(
                "No claim plan",
                "planner",
                &format!("e_{task_id}_plan"),
                100,
            ),
        )
        .await
        .expect("mark claim plan not required");
    store
        .promote_task(
            task_id,
            promote_input(0, "promoter", &format!("e_{task_id}_promote"), 200),
        )
        .await
        .expect("promote claim task")
}

pub(crate) async fn count_rows(connection: &Connection, table: &str) -> i64 {
    let mut rows = connection
        .query(&format!("SELECT COUNT(*) FROM {table}"), ())
        .await
        .expect("count rows");
    let row = rows.next().await.expect("count row").expect("count result");
    integer_value(row.get_value(0).expect("count value"), "count").expect("integer count")
}
