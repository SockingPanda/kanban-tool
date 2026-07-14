use kanban_contract::{
    ApiExecutionPlanState, ApiLabel, ApiTask, ApiTaskPriority, ApiTaskStatus, BlockedReasonCount,
    DoctorDerivedStore, DoctorIssue, DoctorReport, QueueStats, SearchStatus, StaleClaim,
    StatusCount,
};
use kanban_core::{KanbanError, TaskStatus};
use kanban_sqlite::api::{LabelRecord, StepPlanState, TaskRecord};
use serde::Serialize;

use crate::error::ApiError;

pub fn queue_stats_from_record(
    value: kanban_sqlite::api::QueueStats,
) -> Result<QueueStats, KanbanError> {
    Ok(QueueStats {
        board_id: value.board_id,
        generated_at: value.generated_at,
        status_counts: value
            .status_counts
            .into_iter()
            .map(|item| {
                Ok(StatusCount {
                    status: item.status.parse().map_err(|_| {
                        KanbanError::Storage(format!(
                            "invalid persisted task status in queue stats: {}",
                            item.status
                        ))
                    })?,
                    count: item.count,
                })
            })
            .collect::<Result<Vec<_>, KanbanError>>()?,
        stale_claims: value
            .stale_claims
            .into_iter()
            .map(|item| StaleClaim {
                task_id: item.task_id,
                seq: item.seq,
                title: item.title,
                claim_owner: item.claim_owner,
                claim_expires_at: item.claim_expires_at,
                last_heartbeat_at: item.last_heartbeat_at,
                current_run_id: item.current_run_id,
                retry_count: item.retry_count,
                max_retries: item.max_retries,
            })
            .collect(),
        blocked_reasons: value
            .blocked_reasons
            .into_iter()
            .map(|item| BlockedReasonCount {
                reason: item.reason,
                count: item.count,
            })
            .collect(),
        unplanned_active_tasks: value.unplanned_active_tasks,
        active_parents_with_incomplete_required_steps: value
            .active_parents_with_incomplete_required_steps,
    })
}

fn doctor_issue_from_record(value: kanban_sqlite::api::DoctorIssue) -> DoctorIssue {
    DoctorIssue {
        severity: value.severity,
        code: value.code,
        message: value.message,
        record_ids: value.record_ids,
    }
}

fn doctor_store_from_record(
    value: kanban_sqlite::api::DoctorDerivedStoreReport,
) -> DoctorDerivedStore {
    DoctorDerivedStore {
        store_name: value.store_name,
        schema_version: value.schema_version,
        last_event_id: value.last_event_id,
        dirty: value.dirty,
        last_error: value.last_error,
        pending_outbox: value.pending_outbox,
        running_outbox: value.running_outbox,
        failed_outbox: value.failed_outbox,
    }
}

pub fn doctor_report_from_record(value: kanban_sqlite::api::DoctorReport) -> DoctorReport {
    DoctorReport {
        ok: value.ok,
        integrity_check: value.integrity_check,
        migration_version: value.migration_version,
        user_version: value.user_version,
        expired_running_tasks: value.expired_running_tasks,
        running_tasks_without_active_run: value.running_tasks_without_active_run,
        orphan_running_runs: value.orphan_running_runs,
        dependency_cycles: value.dependency_cycles,
        archived_dependency_edges: value.archived_dependency_edges,
        missing_run_logs: value.missing_run_logs,
        suspicious_run_log_paths: value.suspicious_run_log_paths,
        executable_dependency_violations: value.executable_dependency_violations,
        executable_spec_violations: value.executable_spec_violations,
        executable_schedule_violations: value.executable_schedule_violations,
        unplanned_active_tasks: value.unplanned_active_tasks,
        active_parents_with_incomplete_required_steps: value
            .active_parents_with_incomplete_required_steps,
        outbox_pending: value.outbox_pending,
        outbox_running: value.outbox_running,
        outbox_failed: value.outbox_failed,
        derived_dirty_stores: value.derived_dirty_stores,
        derived_error_stores: value.derived_error_stores,
        derived_stores: value
            .derived_stores
            .into_iter()
            .map(doctor_store_from_record)
            .collect(),
        consistency_errors: value.consistency_errors,
        consistency_warnings: value.consistency_warnings,
        consistency_issues: value
            .consistency_issues
            .into_iter()
            .map(doctor_issue_from_record)
            .collect(),
        ontology_ledger_errors: value.ontology_ledger_errors,
        ontology_ledger_warnings: value.ontology_ledger_warnings,
        ontology_ledger_issues: value
            .ontology_ledger_issues
            .into_iter()
            .map(doctor_issue_from_record)
            .collect(),
    }
}

pub fn search_status_from_record(value: kanban_search::SearchIndexStatus) -> SearchStatus {
    SearchStatus {
        backend: value.backend,
        derived_index: value.derived_index,
        stale: value.stale,
        index_version: value.index_version,
        last_event_id: value.last_event_id,
        index_lag_events: value.index_lag_events,
        message: value.message,
    }
}

pub(super) const fn api_task_status_from_core(status: TaskStatus) -> ApiTaskStatus {
    match status {
        TaskStatus::Triage => ApiTaskStatus::Triage,
        TaskStatus::Todo => ApiTaskStatus::Todo,
        TaskStatus::Scheduled => ApiTaskStatus::Scheduled,
        TaskStatus::Ready => ApiTaskStatus::Ready,
        TaskStatus::Running => ApiTaskStatus::Running,
        TaskStatus::Blocked => ApiTaskStatus::Blocked,
        TaskStatus::Review => ApiTaskStatus::Review,
        TaskStatus::Done => ApiTaskStatus::Done,
        TaskStatus::Archived => ApiTaskStatus::Archived,
    }
}

pub(super) const fn task_status_from_api(status: ApiTaskStatus) -> TaskStatus {
    match status {
        ApiTaskStatus::Triage => TaskStatus::Triage,
        ApiTaskStatus::Todo => TaskStatus::Todo,
        ApiTaskStatus::Scheduled => TaskStatus::Scheduled,
        ApiTaskStatus::Ready => TaskStatus::Ready,
        ApiTaskStatus::Running => TaskStatus::Running,
        ApiTaskStatus::Blocked => TaskStatus::Blocked,
        ApiTaskStatus::Review => TaskStatus::Review,
        ApiTaskStatus::Done => TaskStatus::Done,
        ApiTaskStatus::Archived => TaskStatus::Archived,
    }
}

pub(super) const fn api_execution_plan_state_from_record(
    state: StepPlanState,
) -> ApiExecutionPlanState {
    match state {
        StepPlanState::Unplanned => ApiExecutionPlanState::Unplanned,
        StepPlanState::Planned => ApiExecutionPlanState::Planned,
        StepPlanState::NotRequired => ApiExecutionPlanState::NotRequired,
    }
}

pub(super) fn api_label_from_record(label: LabelRecord) -> ApiLabel {
    let LabelRecord {
        id,
        board_id,
        name,
        color,
        created_at,
        updated_at,
    } = label;
    ApiLabel {
        id,
        board_id,
        name,
        color,
        created_at,
        updated_at,
    }
}

pub(super) fn api_task_from_record(task: TaskRecord) -> Result<ApiTask, ApiError> {
    let TaskRecord {
        id,
        board_id,
        board_slug,
        task_ref,
        seq,
        title,
        description,
        status,
        status_reason,
        assignee,
        priority,
        position,
        scheduled_at,
        due_at,
        created_by,
        created_at,
        updated_at,
        started_at,
        completed_at,
        archived_at,
        claim_token: _,
        claim_owner,
        claim_expires_at,
        last_heartbeat_at,
        current_run_id,
        retry_count,
        max_retries,
        result_summary,
        result_json,
        metadata_json,
        lock_version,
        dependency_blocked,
        unfinished_parent_count,
        execution_plan_state,
        required_step_count,
        completed_required_step_count,
        optional_step_count,
        labels,
    } = task;
    let priority = ApiTaskPriority::try_from(priority).map_err(|priority| {
        ApiError(KanbanError::Storage(format!(
            "task record {id} has invalid priority {priority}; expected 0..=3"
        )))
    })?;
    let result = result_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| {
            ApiError(KanbanError::Storage(format!(
                "task record {id} has invalid result_json: {error}"
            )))
        })?;
    let metadata = serde_json::from_str(&metadata_json).map_err(|error| {
        ApiError(KanbanError::Storage(format!(
            "task record {id} has invalid metadata_json: {error}"
        )))
    })?;
    Ok(ApiTask {
        id,
        board_id,
        board_slug,
        task_ref,
        seq,
        title,
        description,
        status: api_task_status_from_core(status),
        status_reason,
        assignee,
        priority,
        position,
        scheduled_at,
        due_at,
        created_by,
        created_at,
        updated_at,
        started_at,
        completed_at,
        archived_at,
        claim_owner,
        claim_expires_at,
        last_heartbeat_at,
        current_run_id,
        retry_count,
        max_retries,
        result_summary,
        result,
        metadata,
        lock_version,
        dependency_blocked,
        unfinished_parent_count,
        execution_plan_state: api_execution_plan_state_from_record(execution_plan_state),
        required_step_count,
        completed_required_step_count,
        optional_step_count,
        labels: labels.into_iter().map(api_label_from_record).collect(),
    })
}

#[derive(Debug, Serialize)]
pub(super) struct LabelAtomExplainDto {
    pub(super) query: String,
    pub(super) atom: Option<kanban_sqlite::api::LabelAtomRecord>,
    pub(super) current_semantics: Option<kanban_sqlite::api::LabelSemanticsRecord>,
    pub(super) provenance_actions: Vec<kanban_sqlite::api::LabelAtomExplainAction>,
    pub(super) supporting_signals: Vec<LabelAtomExplainSignalDto>,
    pub(super) validation_history: Vec<kanban_sqlite::api::LabelAtomExplainValidation>,
    pub(super) legacy_untracked: bool,
    pub(super) legacy_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct LabelAtomExplainSignalDto {
    pub(super) signal: kanban_sqlite::api::LabelOntologySignalRecord,
    pub(super) observation: kanban_sqlite::api::LabelOntologyObservationRecord,
    pub(super) source_task: ApiTask,
    pub(super) task_ref_snapshot: String,
    pub(super) suggest_input_stale: bool,
    pub(super) suggest_degraded: bool,
    pub(super) warnings: Vec<String>,
}

impl TryFrom<kanban_sqlite::api::LabelAtomExplainSignal> for LabelAtomExplainSignalDto {
    type Error = ApiError;

    fn try_from(value: kanban_sqlite::api::LabelAtomExplainSignal) -> Result<Self, Self::Error> {
        let kanban_sqlite::api::LabelAtomExplainSignal {
            signal,
            observation,
            source_task,
            task_ref_snapshot,
            suggest_input_stale,
            suggest_degraded,
            warnings,
        } = value;
        Ok(Self {
            signal,
            observation,
            source_task: api_task_from_record(source_task)?,
            task_ref_snapshot,
            suggest_input_stale,
            suggest_degraded,
            warnings,
        })
    }
}

impl TryFrom<kanban_sqlite::api::LabelAtomExplainRecord> for LabelAtomExplainDto {
    type Error = ApiError;

    fn try_from(value: kanban_sqlite::api::LabelAtomExplainRecord) -> Result<Self, Self::Error> {
        let kanban_sqlite::api::LabelAtomExplainRecord {
            query,
            atom,
            current_semantics,
            provenance_actions,
            supporting_signals,
            validation_history,
            legacy_untracked,
            legacy_reason,
        } = value;
        Ok(Self {
            query,
            atom,
            current_semantics,
            provenance_actions,
            supporting_signals: supporting_signals
                .into_iter()
                .map(LabelAtomExplainSignalDto::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            validation_history,
            legacy_untracked,
            legacy_reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label_record() -> LabelRecord {
        LabelRecord {
            id: "label-1".to_owned(),
            board_id: "board-1".to_owned(),
            name: "backend".to_owned(),
            color: None,
            created_at: 1,
            updated_at: 2,
        }
    }

    fn task_record(priority: i64) -> TaskRecord {
        TaskRecord {
            id: "task-1".to_owned(),
            board_id: "board-1".to_owned(),
            board_slug: "default".to_owned(),
            task_ref: "default#1".to_owned(),
            seq: 1,
            title: "adapter task".to_owned(),
            description: None,
            status: TaskStatus::Ready,
            status_reason: None,
            assignee: None,
            priority,
            position: 1024,
            scheduled_at: None,
            due_at: None,
            created_by: "tester".to_owned(),
            created_at: 1,
            updated_at: 2,
            started_at: None,
            completed_at: None,
            archived_at: None,
            claim_token: Some("never-leak".to_owned()),
            claim_owner: None,
            claim_expires_at: None,
            last_heartbeat_at: None,
            current_run_id: None,
            retry_count: 0,
            max_retries: None,
            result_summary: None,
            result_json: None,
            metadata_json: "{}".to_owned(),
            lock_version: 0,
            dependency_blocked: false,
            unfinished_parent_count: 0,
            execution_plan_state: StepPlanState::Planned,
            required_step_count: 1,
            completed_required_step_count: 0,
            optional_step_count: 0,
            labels: vec![label_record()],
        }
    }

    #[test]
    fn task_status_adapter_is_explicit_and_exhaustive() {
        let cases = [
            (TaskStatus::Triage, ApiTaskStatus::Triage),
            (TaskStatus::Todo, ApiTaskStatus::Todo),
            (TaskStatus::Scheduled, ApiTaskStatus::Scheduled),
            (TaskStatus::Ready, ApiTaskStatus::Ready),
            (TaskStatus::Running, ApiTaskStatus::Running),
            (TaskStatus::Blocked, ApiTaskStatus::Blocked),
            (TaskStatus::Review, ApiTaskStatus::Review),
            (TaskStatus::Done, ApiTaskStatus::Done),
            (TaskStatus::Archived, ApiTaskStatus::Archived),
        ];
        for (core, api) in cases {
            assert_eq!(api_task_status_from_core(core), api);
            assert_eq!(task_status_from_api(api), core);
        }
    }

    #[test]
    fn execution_plan_adapter_is_explicit_and_exhaustive() {
        let cases = [
            (StepPlanState::Unplanned, ApiExecutionPlanState::Unplanned),
            (StepPlanState::Planned, ApiExecutionPlanState::Planned),
            (
                StepPlanState::NotRequired,
                ApiExecutionPlanState::NotRequired,
            ),
        ];
        for (record, api) in cases {
            assert_eq!(api_execution_plan_state_from_record(record), api);
        }
    }

    #[test]
    fn task_record_adapter_checks_priority_and_never_exposes_claim_token() {
        let task = api_task_from_record(task_record(3)).expect("valid task record");
        assert_eq!(task.priority.get(), 3);
        assert_eq!(task.labels, vec![api_label_from_record(label_record())]);

        let value = serde_json::to_value(task).expect("serialize ApiTask");
        assert!(value.get("claim_token").is_none());
        assert_eq!(value["description"], serde_json::Value::Null);
        assert_eq!(value["labels"][0]["color"], serde_json::Value::Null);

        for invalid in [-1, 4, i64::MAX] {
            assert!(
                api_task_from_record(task_record(invalid)).is_err(),
                "adapter 必须拒绝非法 priority {invalid}",
            );
        }
    }

    #[test]
    fn label_record_adapter_maps_every_public_field() {
        let label = api_label_from_record(label_record());
        assert_eq!(
            label,
            ApiLabel {
                id: "label-1".to_owned(),
                board_id: "board-1".to_owned(),
                name: "backend".to_owned(),
                color: None,
                created_at: 1,
                updated_at: 2,
            }
        );
    }

    const NULLABLE_TASK_JSON_FIELDS: &[&str] = &[
        "description",
        "status_reason",
        "assignee",
        "scheduled_at",
        "due_at",
        "started_at",
        "completed_at",
        "archived_at",
        "claim_owner",
        "claim_expires_at",
        "last_heartbeat_at",
        "current_run_id",
        "max_retries",
        "result_summary",
        "result",
    ];

    fn sentinel_label_record() -> LabelRecord {
        LabelRecord {
            id: "label-id-sentinel".to_owned(),
            board_id: "label-board-id-sentinel".to_owned(),
            name: "label-name-sentinel".to_owned(),
            color: Some("#123456".to_owned()),
            created_at: 701,
            updated_at: 702,
        }
    }

    fn expected_sentinel_label() -> ApiLabel {
        ApiLabel {
            id: "label-id-sentinel".to_owned(),
            board_id: "label-board-id-sentinel".to_owned(),
            name: "label-name-sentinel".to_owned(),
            color: Some("#123456".to_owned()),
            created_at: 701,
            updated_at: 702,
        }
    }

    fn sentinel_task_record(priority: i64) -> TaskRecord {
        TaskRecord {
            id: "task-id-sentinel".to_owned(),
            board_id: "task-board-id-sentinel".to_owned(),
            board_slug: "task-board-slug-sentinel".to_owned(),
            task_ref: "task-board-slug-sentinel#314".to_owned(),
            seq: 314,
            title: "task-title-sentinel".to_owned(),
            description: Some("task-description-sentinel".to_owned()),
            status: TaskStatus::Blocked,
            status_reason: Some("task-status-reason-sentinel".to_owned()),
            assignee: Some("task-assignee-sentinel".to_owned()),
            priority,
            position: 2718,
            scheduled_at: Some(101),
            due_at: Some(102),
            created_by: "task-created-by-sentinel".to_owned(),
            created_at: 103,
            updated_at: 104,
            started_at: Some(105),
            completed_at: Some(106),
            archived_at: Some(107),
            claim_token: Some("claim-token-secret-sentinel".to_owned()),
            claim_owner: Some("claim-owner-sentinel".to_owned()),
            claim_expires_at: Some(108),
            last_heartbeat_at: Some(109),
            current_run_id: Some("current-run-id-sentinel".to_owned()),
            retry_count: 7,
            max_retries: Some(8),
            result_summary: Some("result-summary-sentinel".to_owned()),
            result_json: Some("{\"result\":\"sentinel\"}".to_owned()),
            metadata_json: "{\"metadata\":\"sentinel\"}".to_owned(),
            lock_version: 11,
            dependency_blocked: true,
            unfinished_parent_count: 12,
            execution_plan_state: StepPlanState::NotRequired,
            required_step_count: 13,
            completed_required_step_count: 14,
            optional_step_count: 15,
            labels: vec![sentinel_label_record()],
        }
    }

    fn nullable_sentinel_task_record(priority: i64) -> TaskRecord {
        let mut task = sentinel_task_record(priority);
        task.description = None;
        task.status_reason = None;
        task.assignee = None;
        task.scheduled_at = None;
        task.due_at = None;
        task.started_at = None;
        task.completed_at = None;
        task.archived_at = None;
        task.claim_owner = None;
        task.claim_expires_at = None;
        task.last_heartbeat_at = None;
        task.current_run_id = None;
        task.max_retries = None;
        task.result_summary = None;
        task.result_json = None;
        task.labels[0].color = None;
        task
    }

    #[test]
    fn task_record_adapter_maps_every_public_field_with_unique_sentinels() {
        let task = api_task_from_record(sentinel_task_record(2)).expect("valid task record");
        assert_eq!(
            task,
            ApiTask {
                id: "task-id-sentinel".to_owned(),
                board_id: "task-board-id-sentinel".to_owned(),
                board_slug: "task-board-slug-sentinel".to_owned(),
                task_ref: "task-board-slug-sentinel#314".to_owned(),
                seq: 314,
                title: "task-title-sentinel".to_owned(),
                description: Some("task-description-sentinel".to_owned()),
                status: ApiTaskStatus::Blocked,
                status_reason: Some("task-status-reason-sentinel".to_owned()),
                assignee: Some("task-assignee-sentinel".to_owned()),
                priority: ApiTaskPriority::new(2).expect("priority fixture"),
                position: 2718,
                scheduled_at: Some(101),
                due_at: Some(102),
                created_by: "task-created-by-sentinel".to_owned(),
                created_at: 103,
                updated_at: 104,
                started_at: Some(105),
                completed_at: Some(106),
                archived_at: Some(107),
                claim_owner: Some("claim-owner-sentinel".to_owned()),
                claim_expires_at: Some(108),
                last_heartbeat_at: Some(109),
                current_run_id: Some("current-run-id-sentinel".to_owned()),
                retry_count: 7,
                max_retries: Some(8),
                result_summary: Some("result-summary-sentinel".to_owned()),
                result: Some(serde_json::json!({"result": "sentinel"})),
                metadata: serde_json::json!({"metadata": "sentinel"}),
                lock_version: 11,
                dependency_blocked: true,
                unfinished_parent_count: 12,
                execution_plan_state: ApiExecutionPlanState::NotRequired,
                required_step_count: 13,
                completed_required_step_count: 14,
                optional_step_count: 15,
                labels: vec![expected_sentinel_label()],
            }
        );

        let value = serde_json::to_value(task).expect("serialize ApiTask");
        assert_eq!(value["ref"], "task-board-slug-sentinel#314");
        assert_eq!(value["status"], "blocked");
        assert_eq!(value["priority"], 2);
        assert_eq!(value["execution_plan_state"], "not_required");
        assert_eq!(value["labels"][0]["color"], "#123456");
        assert_eq!(value["result"], serde_json::json!({"result": "sentinel"}));
        assert_eq!(
            value["metadata"],
            serde_json::json!({"metadata": "sentinel"})
        );
        assert!(value.get("result_json").is_none());
        assert!(value.get("metadata_json").is_none());
        assert!(value.get("claim_token").is_none());
    }

    #[test]
    fn task_record_adapter_preserves_all_required_nulls_and_omits_secret() {
        let task =
            api_task_from_record(nullable_sentinel_task_record(0)).expect("nullable task record");
        let value = serde_json::to_value(task).expect("serialize nullable ApiTask");
        for field in NULLABLE_TASK_JSON_FIELDS {
            assert_eq!(value[*field], serde_json::Value::Null, "field {field}");
        }
        assert_eq!(value["labels"][0]["color"], serde_json::Value::Null);
        assert!(value.get("claim_token").is_none());
    }

    #[test]
    fn task_record_sentinel_adapter_rejects_every_invalid_priority() {
        for invalid in [-1, 4, i64::MAX] {
            assert!(
                api_task_from_record(sentinel_task_record(invalid)).is_err(),
                "adapter 必须拒绝非法 priority {invalid}",
            );
        }
    }

    #[test]
    fn task_record_adapter_rejects_malformed_persisted_json() {
        let mut invalid_result = sentinel_task_record(2);
        invalid_result.result_json = Some("{".to_owned());
        assert!(api_task_from_record(invalid_result).is_err());

        let mut invalid_metadata = sentinel_task_record(2);
        invalid_metadata.metadata_json = "{".to_owned();
        assert!(api_task_from_record(invalid_metadata).is_err());
    }

    #[test]
    fn label_record_adapter_maps_unique_sentinels_to_exact_public_json() {
        let label = api_label_from_record(sentinel_label_record());
        assert_eq!(label, expected_sentinel_label());
        assert_eq!(
            serde_json::to_value(label).expect("serialize ApiLabel"),
            serde_json::json!({
                "id": "label-id-sentinel",
                "board_id": "label-board-id-sentinel",
                "name": "label-name-sentinel",
                "color": "#123456",
                "created_at": 701,
                "updated_at": 702
            })
        );
    }

    #[test]
    fn queue_stats_adapter_rejects_invalid_persisted_status() {
        let error = queue_stats_from_record(kanban_sqlite::api::QueueStats {
            board_id: "board-1".to_owned(),
            generated_at: 1,
            status_counts: vec![kanban_sqlite::api::StatusCount {
                status: "not-a-task-status".to_owned(),
                count: 1,
            }],
            stale_claims: Vec::new(),
            blocked_reasons: Vec::new(),
            unplanned_active_tasks: 0,
            active_parents_with_incomplete_required_steps: 0,
        })
        .expect_err("invalid persisted status must be reported");

        assert!(
            error
                .to_string()
                .contains("invalid persisted task status in queue stats"),
            "{error}"
        );
    }
}
