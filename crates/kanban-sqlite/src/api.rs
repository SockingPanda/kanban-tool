//! Explicit adapter-facing facade for SQLite-backed use cases.
//!
//! This module is an allowlist for new adapter contract paths. The
//! `service` module remains the implementation owner for transactions,
//! state-machine guards, canonical writes, events, runs, and provenance.
//! The crate root still re-exports legacy service symbols for existing
//! callers.

pub use crate::service::{
    ClaimResult, CreateLabel, CreateTask, DispatchOptions, DispatchResult, EventListOptions,
    EventRecord, FinishPolicy, LabelOntologySignalDetail, LabelOntologySignalStatus, LabelRecord,
    LabelSemanticsRecord, MAX_TASK_LIST_LIMIT, RunRecord, StepPlanState, TaskListOptions,
    TaskListPage, TaskListSort, TaskPatch, TaskPlanFilter, TaskRecord, add_task_label,
    add_task_label_by_id, archive_task, block_task, claim_task, claim_task_with_profile,
    claim_task_with_profile_and_metadata, complete_task, complete_task_with_summary,
    complete_task_with_summary_and_result, create_label, create_label_with_actor, create_task,
    default_priority, dispatch_once, get_label_ontology_signal, get_label_semantics_by_id,
    get_task, get_task_by_id_global, heartbeat_task, heartbeat_task_with_note, list_events,
    list_events_after, list_labels, list_runs, list_tasks, list_tasks_page,
    mark_execution_plan_not_required, promote_task, reopen_task, submit_review_task,
    submit_review_task_with_summary, unblock_task,
};
