use super::support::*;
use crate::application::{
    comments::support::api_comment, dependencies::support::api_dependencies,
    events::list::api_event, support::api_run,
};
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    GetTaskDetailsResponse, GetTaskPath, GetTaskQuery, GetTaskResponse, TaskDetailAggregate,
    TaskDetailOntology, TaskOntologySignalSummary, TaskOntologySummary,
};
use kanban_service::KanbanError;
pub(crate) async fn get_task(
    state: AppState,
    GetTaskPath { task_id }: GetTaskPath,
    query: GetTaskQuery,
) -> Result<GetTaskResponse, ApiError> {
    if query
        .include
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return Err(KanbanError::InvalidInput(
            "详情和 ontology 请使用 GetTaskDetails RPC".to_owned(),
        )
        .into());
    }
    let task = state.application().get_task(&task_id).await?;
    Ok(GetTaskResponse::new(api_task(task)?, None))
}

pub(crate) async fn get_task_details(
    state: AppState,
    GetTaskPath { task_id }: GetTaskPath,
) -> Result<GetTaskDetailsResponse, ApiError> {
    let detail = state.application().get_task_details(&task_id).await?;
    Ok(GetTaskDetailsResponse {
        data: api_task_details(detail)?,
    })
}
fn api_task_details(
    detail: kanban_service::TaskDetailRecord,
) -> Result<TaskDetailAggregate, ApiError> {
    let labels = detail.labels.into_iter().map(api_label).collect();
    let task = api_task(detail.task)?;
    let dependencies = api_dependencies(detail.dependencies)?;
    let execution_plan = api_execution_plan(detail.execution_plan);
    let steps = detail
        .steps
        .into_iter()
        .map(api_task_step)
        .collect::<Result<Vec<_>, _>>()?;
    let comments = detail
        .comments
        .into_iter()
        .map(api_comment)
        .collect::<Result<Vec<_>, _>>()?;
    let runs = detail
        .runs
        .into_iter()
        .map(api_run)
        .collect::<Result<Vec<_>, _>>()?;
    let events = detail
        .events
        .into_iter()
        .map(api_event)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TaskDetailAggregate {
        task,
        labels,
        dependencies,
        execution_plan,
        steps,
        comments,
        runs,
        events,
        ontology: TaskDetailOntology {
            summary: detail.ontology.summary.map(api_task_ontology_summary),
            degraded: detail.ontology.degraded,
            diagnostics: detail.ontology.diagnostics,
        },
    })
}
fn api_task_ontology_summary(
    summary: kanban_service::TaskOntologySummaryRecord,
) -> TaskOntologySummary {
    TaskOntologySummary {
        task_id: summary.task_id,
        observation_count: summary.observation_count,
        signal_count: summary.signal_count,
        open_count: summary.open_count,
        confirmed_count: summary.confirmed_count,
        resolved_count: summary.resolved_count,
        rejected_count: summary.rejected_count,
        superseded_count: summary.superseded_count,
        degraded_count: summary.degraded_count,
        stale_count: summary.stale_count,
        suggest_input_drift_count: summary.suggest_input_drift_count,
        legacy_incomparable_count: summary.legacy_incomparable_count,
        incomparable_count: summary.incomparable_count,
        action_count: summary.action_count,
        oldest_open_confirmed_signal_at: summary.oldest_open_confirmed_signal_at,
        oldest_open_confirmed_signal_age_ms: summary.oldest_open_confirmed_signal_age_ms,
        latest_signal_at: summary.latest_signal_at,
        latest_action_at: summary.latest_action_at,
        current_suggest_input_hash: summary.current_suggest_input_hash,
        sample_signals: summary
            .sample_signals
            .into_iter()
            .map(api_task_ontology_signal_summary)
            .collect(),
    }
}
fn api_task_ontology_signal_summary(
    signal: kanban_service::TaskOntologySignalSummaryRecord,
) -> TaskOntologySignalSummary {
    TaskOntologySignalSummary {
        id: signal.id,
        kind: signal.kind,
        status: signal.status,
        proposed_action: signal.proposed_action,
        target_label_id: signal.target_label_id,
        target_label_name: signal.target_label_name,
        candidate_atom_polarity: signal.candidate_atom_polarity,
        candidate_atom_kind: signal.candidate_atom_kind,
        candidate_text: signal.candidate_text,
        candidate_content_hash: signal.candidate_content_hash,
        proposed_label_name: signal.proposed_label_name,
        proposed_label_name_normalized: signal.proposed_label_name_normalized,
        suggest_score: signal.suggest_score,
        suggest_rank: signal.suggest_rank,
        degraded: signal.degraded,
        stale: signal.stale,
        legacy_incomparable: signal.legacy_incomparable,
        suggest_input_drift: signal.suggest_input_drift,
        created_at: signal.created_at,
        updated_at: signal.updated_at,
        latest_action_at: signal.latest_action_at,
        action_count: signal.action_count,
    }
}
