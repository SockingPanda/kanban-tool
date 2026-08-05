use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::QueryRejection},
    response::{IntoResponse, Response},
    routing::get,
};
use kanban_contract::{
    GetTaskDetailsResponse, GetTaskPath, GetTaskQuery, GetTaskResponse, TaskDetailAggregate,
    TaskDetailOntology, TaskOntologySignalSummary, TaskOntologySummary,
};
use kanban_core::KanbanError;

use crate::http::operations::{
    comments::support::api_comment, dependencies::support::api_dependencies,
    events::list::api_event, support::api_run,
};

pub(crate) async fn get_task(
    State(state): State<AppState>,
    Path(GetTaskPath { task_id }): Path<GetTaskPath>,
    query: Result<Query<GetTaskQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("invalid query: {error}")))?;
    if let Some(include) = query.include.as_deref() {
        let includes = include.split(',').map(str::trim).collect::<Vec<_>>();
        if let Some(unsupported) = includes
            .iter()
            .find(|part| !part.is_empty() && !matches!(**part, "details" | "ontology"))
        {
            return Err(KanbanError::InvalidInput(format!(
                "unsupported task include: {unsupported}"
            ))
            .into());
        }
        if includes
            .iter()
            .any(|part| matches!(*part, "details" | "ontology"))
        {
            let detail = state.application().get_task_details(&task_id).await?;
            return Ok(Json(GetTaskDetailsResponse {
                data: api_task_details(detail)?,
            })
            .into_response());
        }
    }
    let task = state.application().get_task(&task_id).await?;
    Ok(Json(GetTaskResponse::new(api_task(task)?, None)).into_response())
}

fn api_task_details(
    detail: kanban_application::TaskDetailRecord,
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
    summary: kanban_application::TaskOntologySummaryRecord,
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
    signal: kanban_application::TaskOntologySignalSummaryRecord,
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

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id", get(get_task))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn task_show_details_returns_canonical_aggregate_and_degraded_ontology_meta() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let created = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_show_details",
                    "title": "Details",
                    "description": "detail task",
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/t_show_details?include=details,ontology")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let details: GetTaskDetailsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(details.data.task.id, "t_show_details");
        assert!(
            details
                .data
                .events
                .iter()
                .any(|event| event.kind == "task.created")
        );
        assert!(details.data.ontology.degraded);
    }
}
