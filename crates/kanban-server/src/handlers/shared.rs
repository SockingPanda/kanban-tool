use axum::{Json, extract::rejection::JsonRejection, http::HeaderMap};
use kanban_application::api as application_api;
use kanban_entity::Predicate;
use serde_json::json;

use crate::dto::*;
use crate::error::{ApiError, extractor_error, invalid_input};
use crate::state::AppState;

pub(crate) fn events_snapshot(
    state: &AppState,
    board_ref: &str,
    task_id: Option<String>,
    after: i64,
    requested_limit: usize,
) -> Result<(Vec<kanban_contract::StreamEventData>, i64), ApiError> {
    let board = kanban_sqlite::api::get_board_including_archived(state.db_path(), board_ref)?;
    let limit = requested_limit.min(1000);
    let application = state.application();
    let events = application_api::list_events_after(
        &application,
        board_ref,
        application_api::EventListOptions {
            task_ref: task_id,
            after,
            limit,
        },
    )?;
    let next_after = events.last().map_or(after, |event| event.id);
    let data = events
        .into_iter()
        .map(|event| stream_event_data(event, &board.id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((data, next_after))
}

fn stream_event_data(
    event: application_api::EventRecord,
    board_id: &str,
) -> Result<kanban_contract::StreamEventData, ApiError> {
    let raw_payload = serde_json::from_str(&event.payload_json).map_err(|error| {
        ApiError(kanban_core::KanbanError::Storage(format!(
            "event {} ({}) has malformed payload_json: {error}",
            event.event_id, event.kind
        )))
    })?;
    let payload =
        kanban_contract::event_payload::EventPayload::from_kind_and_value(&event.kind, raw_payload)
            .map_err(|error| {
                ApiError(kanban_core::KanbanError::Storage(format!(
                    "event {} ({}) violates its payload contract: {error}",
                    event.event_id, event.kind
                )))
            })?;
    Ok(kanban_contract::StreamEventData {
        id: event.id,
        event_id: event.event_id,
        board_id: board_id.to_owned(),
        task_id: event.task_id,
        run_id: event.run_id,
        kind: event.kind,
        actor: event.actor,
        payload,
        created_at: event.created_at,
    })
}

pub(crate) fn dependencies_dto(
    state: &AppState,
    task_id: &str,
) -> Result<kanban_contract::ApiDependencies, ApiError> {
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), task_id)?;
    let snapshot =
        kanban_sqlite::api::dependency_snapshot(state.db_path(), &task.board_id, task_id)?;
    let mut parents = Vec::new();
    let mut children = Vec::new();
    for edge in &snapshot.edges {
        if edge.child.id == task_id {
            let parent =
                kanban_sqlite::api::get_task_by_id_global(state.db_path(), &edge.parent.id)?;
            parents.push(api_task_from_record(parent)?);
        }
        if edge.parent.id == task_id {
            let child = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &edge.child.id)?;
            children.push(api_task_from_record(child)?);
        }
    }
    Ok(kanban_contract::ApiDependencies {
        task: api_dependency_task(snapshot.task),
        parents,
        children,
        edges: snapshot
            .edges
            .into_iter()
            .map(|edge| kanban_contract::ApiDependencyEdge {
                parent: api_dependency_task(edge.parent),
                child: api_dependency_task(edge.child),
            })
            .collect(),
    })
}

fn api_dependency_task(
    task: kanban_sqlite::api::DependencyTaskRecord,
) -> kanban_contract::ApiDependencyTask {
    kanban_contract::ApiDependencyTask {
        id: task.id,
        board_id: task.board_id,
        board_slug: task.board_slug,
        task_ref: task.task_ref,
        title: task.title,
        status: api_task_status_from_core(task.status),
    }
}

pub(crate) fn optional_json_body<T: Default>(
    body: Result<Json<T>, JsonRejection>,
) -> Result<T, ApiError> {
    match body {
        Ok(Json(body)) => Ok(body),
        Err(JsonRejection::MissingJsonContentType(_)) => Ok(T::default()),
        Err(error) => Err(extractor_error(error)),
    }
}

pub(crate) fn parse_predicate(value: &str) -> Result<Predicate, ApiError> {
    match value.trim() {
        "belongs_to_board" => Ok(Predicate::BelongsToBoard),
        "belongs_to_task" => Ok(Predicate::BelongsToTask),
        "depends_on" => Ok(Predicate::DependsOn),
        "produced_by" => Ok(Predicate::ProducedBy),
        "generated_by" => Ok(Predicate::GeneratedBy),
        "references_artifact" => Ok(Predicate::ReferencesArtifact),
        "related_to" => Ok(Predicate::RelatedTo),
        "uses_skill" => Ok(Predicate::UsesSkill),
        "uses_context" => Ok(Predicate::UsesContext),
        "derived_from" => Ok(Predicate::DerivedFrom),
        "supersedes" => Ok(Predicate::Supersedes),
        "similar_to" => Ok(Predicate::SimilarTo),
        "requires_review" => Ok(Predicate::RequiresReview),
        "waiting_for_user" => Ok(Predicate::WaitingForUser),
        other => Err(invalid_input(format!("unsupported predicate: {other}"))),
    }
}

pub(crate) fn actor(body_actor: Option<&str>, headers: &HeaderMap, state: &AppState) -> String {
    body_actor
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| {
            headers
                .get("X-KB-Actor")
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.trim().is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| state.default_actor().to_owned())
}

pub(crate) fn metadata_json(value: Option<serde_json::Value>) -> Result<String, ApiError> {
    Ok(value.unwrap_or_else(|| json!({})).to_string())
}
