use std::str::FromStr;

use axum::{Json, extract::rejection::JsonRejection, http::HeaderMap};
use kanban_core::TaskStatus;
use kanban_entity::Predicate;
use serde::Deserialize;
use serde_json::json;

use crate::dto::*;
use crate::error::{ApiError, extractor_error, invalid_input};
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActorBody {
    pub(crate) actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommentBody {
    pub(crate) author: Option<String>,
    pub(crate) body: String,
    pub(crate) kind: Option<String>,
    pub(crate) author_type: Option<String>,
    pub(crate) agent_type: Option<String>,
    pub(crate) metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClaimBody {
    pub(crate) actor: Option<String>,
    #[serde(default = "default_claim_ttl_ms")]
    pub(crate) ttl_ms: i64,
    pub(crate) worker_profile: Option<String>,
    pub(crate) metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpecifyBody {
    pub(crate) actor: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) scheduled_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HeartbeatBody {
    pub(crate) actor: Option<String>,
    pub(crate) claim_token: String,
    #[serde(default = "default_claim_ttl_ms")]
    pub(crate) ttl_ms: i64,
    pub(crate) note: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct TokenBody {
    pub(crate) actor: Option<String>,
    pub(crate) claim_token: Option<String>,
    #[serde(default)]
    pub(crate) force: bool,
    pub(crate) summary: Option<String>,
    pub(crate) result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReclaimBody {
    pub(crate) actor: Option<String>,
    #[serde(default)]
    pub(crate) force: bool,
    pub(crate) to_status: Option<TaskStatus>,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlockBody {
    pub(crate) actor: Option<String>,
    pub(crate) reason: String,
    pub(crate) claim_token: Option<String>,
    #[serde(default)]
    pub(crate) force: bool,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArchiveBody {
    pub(crate) actor: Option<String>,
    #[serde(default)]
    pub(crate) force: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddDependencyBody {
    pub(crate) parent_task_id: String,
    pub(crate) actor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EventsQuery {
    #[serde(default = "default_board")]
    pub(crate) board: String,
    pub(crate) task_id: Option<String>,
    #[serde(default)]
    pub(crate) after: i64,
    #[serde(default = "default_limit")]
    pub(crate) limit: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StatsQuery {
    #[serde(default = "default_board")]
    pub(crate) board: String,
}

pub(crate) fn default_claim_ttl_ms() -> i64 {
    300_000
}

fn default_limit() -> usize {
    100
}

pub(crate) fn default_board() -> String {
    "default".to_owned()
}

pub(crate) fn events_snapshot(
    state: &AppState,
    query: &EventsQuery,
) -> Result<(Vec<EventDto>, i64), ApiError> {
    let board = kanban_sqlite::get_board_including_archived(state.db_path(), &query.board)?;
    let limit = query.limit.min(1000);
    let events = kanban_sqlite::list_events_after(
        state.db_path(),
        &query.board,
        kanban_sqlite::EventListOptions {
            task_ref: query.task_id.clone(),
            after: query.after,
            limit,
        },
    )?;
    let next_after = events.last().map_or(query.after, |event| event.id);
    let data = events
        .into_iter()
        .map(|event| EventDto {
            id: event.id,
            event_id: event.event_id,
            board_id: board.id.clone(),
            task_id: event.task_id,
            run_id: event.run_id,
            kind: event.kind,
            actor: event.actor,
            payload: serde_json::from_str(&event.payload_json).unwrap_or_else(|_| json!({})),
            created_at: event.created_at,
        })
        .collect();
    Ok((data, next_after))
}

pub(crate) fn dependencies_dto(
    state: &AppState,
    task_id: &str,
) -> Result<DependenciesDto, ApiError> {
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), task_id)?;
    let edges = kanban_sqlite::list_dependencies(state.db_path(), &task.board_id, task_id)?;
    let mut parents = Vec::new();
    let mut children = Vec::new();
    for (parent_id, child_id) in edges {
        if child_id == task_id {
            parents.push(TaskDto::from(kanban_sqlite::get_task_by_id_global(
                state.db_path(),
                &parent_id,
            )?));
        }
        if parent_id == task_id {
            children.push(TaskDto::from(kanban_sqlite::get_task_by_id_global(
                state.db_path(),
                &child_id,
            )?));
        }
    }
    Ok(DependenciesDto { parents, children })
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

pub(crate) fn parse_task_sort(sort: Option<&str>) -> Result<kanban_sqlite::TaskListSort, ApiError> {
    let sort = match sort.unwrap_or("position") {
        "seq" => kanban_sqlite::TaskListSort::Seq,
        "-seq" => kanban_sqlite::TaskListSort::SeqDesc,
        "title" => kanban_sqlite::TaskListSort::Title,
        "-title" => kanban_sqlite::TaskListSort::TitleDesc,
        "status" => kanban_sqlite::TaskListSort::Status,
        "-status" => kanban_sqlite::TaskListSort::StatusDesc,
        "position" => kanban_sqlite::TaskListSort::Position,
        "-position" => kanban_sqlite::TaskListSort::PositionDesc,
        "priority" => kanban_sqlite::TaskListSort::Priority,
        "-priority" => kanban_sqlite::TaskListSort::PriorityDesc,
        "assignee" => kanban_sqlite::TaskListSort::Assignee,
        "-assignee" => kanban_sqlite::TaskListSort::AssigneeDesc,
        "scheduled_at" => kanban_sqlite::TaskListSort::ScheduledAt,
        "-scheduled_at" => kanban_sqlite::TaskListSort::ScheduledAtDesc,
        "created_at" => kanban_sqlite::TaskListSort::CreatedAt,
        "-created_at" => kanban_sqlite::TaskListSort::CreatedAtDesc,
        "updated_at" => kanban_sqlite::TaskListSort::UpdatedAt,
        "-updated_at" => kanban_sqlite::TaskListSort::UpdatedAtDesc,
        "due_at" => kanban_sqlite::TaskListSort::DueAt,
        "-due_at" => kanban_sqlite::TaskListSort::DueAtDesc,
        value => return Err(invalid_input(format!("unsupported sort: {value}"))),
    };
    Ok(sort)
}

pub(crate) fn parse_priority_filters(raw_query: Option<&str>) -> Result<Vec<i64>, ApiError> {
    let Some(raw_query) = raw_query else {
        return Ok(Vec::new());
    };
    let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(raw_query)
        .map_err(|error| invalid_input(error.to_string()))?;
    pairs
        .into_iter()
        .filter_map(|(key, value)| (key == "priority").then_some(value))
        .map(|value| {
            let value = value
                .trim()
                .parse::<i64>()
                .map_err(|_| invalid_input(format!("invalid priority filter: {value}")))?;
            kanban_sqlite::validate_priority(value).map_err(ApiError::from)?;
            Ok(value)
        })
        .collect()
}

pub(crate) fn parse_status_filters(raw_query: Option<&str>) -> Result<Vec<TaskStatus>, ApiError> {
    let Some(raw_query) = raw_query else {
        return Ok(Vec::new());
    };
    let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(raw_query)
        .map_err(|error| invalid_input(error.to_string()))?;
    pairs
        .into_iter()
        .filter_map(|(key, value)| (key == "status").then_some(value))
        .map(|value| TaskStatus::from_str(value.trim()).map_err(ApiError::from))
        .collect()
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

pub(crate) fn patch_from_value(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<kanban_sqlite::TaskPatch, ApiError> {
    const ALLOWED: &[&str] = &[
        "title",
        "description",
        "assignee",
        "priority",
        "scheduled_at",
        "due_at",
        "metadata_json",
        "metadata",
        "max_retries",
        "expected_lock_version",
        "actor",
    ];
    for key in object.keys() {
        if !ALLOWED.contains(&key.as_str()) {
            return Err(invalid_input(format!("unknown patch field: {key}")));
        }
    }

    let mut patch = kanban_sqlite::TaskPatch::default();
    if let Some(value) = object.get("title") {
        patch.title = Some(string_field(value, "title")?);
    }
    if object.contains_key("description") {
        patch.description = Some(optional_string_field(
            object.get("description"),
            "description",
        )?);
    }
    if object.contains_key("assignee") {
        patch.assignee = Some(optional_string_field(object.get("assignee"), "assignee")?);
    }
    if let Some(value) = object.get("priority") {
        patch.priority = Some(
            value
                .as_i64()
                .ok_or_else(|| invalid_input("priority must be an integer"))?,
        );
    }
    if object.contains_key("scheduled_at") {
        patch.scheduled_at = Some(optional_i64_field(
            object.get("scheduled_at"),
            "scheduled_at",
        )?);
    }
    if object.contains_key("due_at") {
        patch.due_at = Some(optional_i64_field(object.get("due_at"), "due_at")?);
    }
    if let Some(value) = object.get("metadata") {
        patch.metadata_json = Some(value.to_string());
    }
    if let Some(value) = object.get("metadata_json") {
        patch.metadata_json = Some(string_field(value, "metadata_json")?);
    }
    if let Some(value) = object.get("expected_lock_version") {
        patch.expected_lock_version = Some(
            value
                .as_i64()
                .ok_or_else(|| invalid_input("expected_lock_version must be an integer"))?,
        );
    }
    Ok(patch)
}

pub(crate) fn retry_policy_from_value(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<Option<i64>>, ApiError> {
    if !object.contains_key("max_retries") {
        return Ok(None);
    }
    optional_i64_field(object.get("max_retries"), "max_retries").map(Some)
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

pub(crate) fn string_field(value: &serde_json::Value, field: &str) -> Result<String, ApiError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid_input(format!("{field} must be a string")))
}

pub(crate) fn optional_string_field(
    value: Option<&serde_json::Value>,
    field: &str,
) -> Result<Option<String>, ApiError> {
    match value {
        Some(serde_json::Value::Null) => Ok(None),
        Some(value) => string_field(value, field).map(Some),
        None => Ok(None),
    }
}

pub(crate) fn optional_i64_field(
    value: Option<&serde_json::Value>,
    field: &str,
) -> Result<Option<i64>, ApiError> {
    match value {
        Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value
            .as_i64()
            .map(Some)
            .ok_or_else(|| invalid_input(format!("{field} must be an integer epoch ms"))),
        None => Ok(None),
    }
}
