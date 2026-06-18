use axum::{
    Json,
    extract::{
        Path, Query, RawQuery, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode},
};
use kanban_core::TaskStatus;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value as JsonValue, json};
use std::str::FromStr;

use crate::dto::{Envelope, LabelDto, TaskDto};
use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
use crate::state::AppState;

use super::shared::{
    actor, metadata_json, parse_priority_filters, parse_status_filters, parse_task_sort,
    patch_from_value, retry_policy_from_value,
};

#[derive(Debug, Deserialize)]
pub(crate) struct TaskListQuery {
    #[serde(default)]
    include_archived: bool,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    assignee: Option<String>,
    q: Option<String>,
    search: Option<String>,
    sort: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TaskGetQuery {
    include: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LabelSuggestionQuery {
    #[serde(default = "default_label_suggestion_limit")]
    limit: usize,
    #[serde(default = "default_label_suggestion_candidate_limit")]
    candidate_limit: usize,
    #[serde(default = "default_label_suggestion_atom_limit")]
    atom_limit: usize,
    #[serde(default = "default_label_suggestion_max_selected_labels")]
    max_selected_labels: usize,
    #[serde(default = "default_label_suggestion_min_score")]
    min_score: f32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LabelAtomIndexQuery {
    q: String,
    polarity: Option<String>,
    #[serde(default = "default_label_atom_query_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LabelOntologySignalQuery {
    #[serde(default)]
    include_all: bool,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(alias = "task")]
    task_ref: Option<String>,
    #[serde(alias = "label")]
    target_label_ref: Option<String>,
    #[serde(alias = "proposed_label")]
    proposed_label_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LabelOntologyReviewQuery {
    #[serde(default = "default_label_ontology_review_group_by")]
    group_by: String,
    #[serde(default)]
    include_all: bool,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    100
}

fn default_label_ontology_review_group_by() -> String {
    "label".to_owned()
}

fn task_get_includes_ontology(include: Option<&str>) -> bool {
    include.is_some_and(|include| {
        include
            .split(',')
            .map(str::trim)
            .any(|item| item == "ontology")
    })
}

fn default_label_suggestion_limit() -> usize {
    kanban_sqlite::LabelSuggestionOptions::default().output_limit
}

fn default_label_suggestion_candidate_limit() -> usize {
    kanban_sqlite::LabelSuggestionOptions::default().candidate_limit
}

fn default_label_suggestion_atom_limit() -> usize {
    kanban_sqlite::LabelSuggestionOptions::default().atom_limit
}

fn default_label_suggestion_max_selected_labels() -> usize {
    kanban_sqlite::LabelSuggestionOptions::default().max_selected_labels
}

fn default_label_suggestion_min_score() -> f32 {
    kanban_sqlite::LabelSuggestionOptions::default().min_score
}

#[derive(Debug, Default)]
enum JsonBodyField {
    #[default]
    Missing,
    Present(JsonValue),
}

impl JsonBodyField {
    fn is_present(&self) -> bool {
        matches!(self, JsonBodyField::Present(_))
    }

    fn into_value(self) -> Option<JsonValue> {
        match self {
            JsonBodyField::Missing => None,
            JsonBodyField::Present(value) => Some(value),
        }
    }
}

impl<'de> Deserialize<'de> for JsonBodyField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(JsonBodyField::Present(JsonValue::deserialize(
            deserializer,
        )?))
    }
}

#[derive(Clone, Copy)]
enum JsonBodyShape {
    Array,
    Object,
}

impl JsonBodyShape {
    fn name(self) -> &'static str {
        match self {
            JsonBodyShape::Array => "array",
            JsonBodyShape::Object => "object",
        }
    }
}

fn default_label_atom_query_limit() -> usize {
    24
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateTaskBody {
    title: String,
    description: Option<String>,
    status: Option<TaskStatus>,
    assignee: Option<String>,
    #[serde(default = "kanban_sqlite::default_priority")]
    priority: i64,
    scheduled_at: Option<i64>,
    due_at: Option<i64>,
    max_retries: Option<i64>,
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateLabelBody {
    name: String,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AddTaskLabelBody {
    name: Option<String>,
    names: Option<Vec<String>>,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BootstrapTaskLabelBody {
    name: String,
    description: Option<String>,
    #[serde(default)]
    applies_when: Vec<String>,
    #[serde(default)]
    excludes_when: Vec<String>,
    #[serde(default)]
    positive_examples: Vec<String>,
    #[serde(default)]
    negative_examples: Vec<String>,
    actor: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BootstrapTaskLabelDto {
    task: TaskDto,
    semantics: kanban_sqlite::LabelSemanticsRecord,
}

impl AddTaskLabelBody {
    fn label_names(&self) -> Result<Vec<String>, ApiError> {
        match (&self.name, &self.names) {
            (Some(_), Some(_)) => Err(invalid_input("provide either name or names, not both")),
            (Some(name), None) => Ok(vec![name.clone()]),
            (None, Some(names)) if names.is_empty() => {
                Err(invalid_input("names must contain at least one label"))
            }
            (None, Some(names)) => Ok(names.clone()),
            (None, None) => Err(invalid_input("name or names is required")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelProposalBody {
    proposal: Option<kanban_sqlite::LabelProposalCandidate>,
    actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelProposalDecisionBody {
    reason: Option<String>,
    actor: Option<String>,
    #[serde(default)]
    source_signal_ids: Vec<String>,
    ontology_actor: Option<LabelOntologyActorBody>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelOntologyActorBody {
    name: String,
    #[serde(rename = "type")]
    actor_type: String,
    agent_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelOntologyCandidateAtomBody {
    polarity: String,
    kind: String,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelOntologySignalBody {
    kind: kanban_sqlite::LabelOntologySignalKind,
    target_label_ref: Option<String>,
    #[serde(default)]
    related_labels: JsonBodyField,
    related_labels_json: Option<String>,
    proposed_action: kanban_sqlite::LabelOntologyProposedAction,
    candidate_atom: Option<LabelOntologyCandidateAtomBody>,
    proposed_label_name: Option<String>,
    #[serde(default)]
    proposal: JsonBodyField,
    proposal_json: Option<String>,
    agent_selected: bool,
    suggest_state: Option<kanban_sqlite::LabelOntologySuggestState>,
    suggest_score: Option<f64>,
    suggest_rank: Option<i64>,
    final_selected: bool,
    rationale: String,
    confidence: Option<f64>,
    signal_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelOntologyObservationBody {
    actor: LabelOntologyActorBody,
    #[serde(default)]
    agent_candidates: JsonBodyField,
    agent_candidates_json: Option<String>,
    #[serde(default)]
    suggestion_snapshot: JsonBodyField,
    suggestion_snapshot_json: Option<String>,
    #[serde(default)]
    final_decision: JsonBodyField,
    final_decision_json: Option<String>,
    suggest_coverage: Option<f64>,
    suggest_coverage_cosine: Option<f64>,
    suggest_residual_norm: Option<f64>,
    suggest_needs_new_label: Option<bool>,
    suggest_degraded: Option<bool>,
    #[serde(default)]
    diagnostics: JsonBodyField,
    diagnostics_json: Option<String>,
    capture_fingerprint: Option<String>,
    signals: Vec<LabelOntologySignalBody>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelOntologyActionBody {
    actor: LabelOntologyActorBody,
    action_type: kanban_sqlite::LabelOntologyActionType,
    signal_ids: Vec<String>,
    reason: String,
    superseded_by_signal_id: Option<String>,
    parent_action_id: Option<String>,
    target_label_ref: Option<String>,
    result_label_ref: Option<String>,
    result_atom_id: Option<String>,
    result_atom_content_hash: Option<String>,
    result_proposal_id: Option<String>,
    canonical_before_hash: Option<String>,
    canonical_after_hash: Option<String>,
    #[serde(default)]
    change: JsonBodyField,
    change_json: Option<String>,
    validation_status: Option<kanban_sqlite::LabelOntologyValidationStatus>,
    #[serde(default)]
    validation: JsonBodyField,
    validation_json: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelOntologyAtomApplyBody {
    actor: LabelOntologyActorBody,
    signal_ids: Vec<String>,
    label_ref: String,
    kind: String,
    text: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelOntologyValidationBody {
    actor: LabelOntologyActorBody,
    parent_action_id: String,
    #[serde(default)]
    signal_ids: Vec<String>,
    reason: String,
    validation_status: kanban_sqlite::LabelOntologyValidationStatus,
    #[serde(default)]
    validation: JsonBodyField,
    validation_json: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpsertLabelSemanticsBody {
    description: Option<String>,
    #[serde(default)]
    applies_when: Vec<String>,
    #[serde(default)]
    excludes_when: Vec<String>,
    #[serde(default)]
    positive_examples: Vec<String>,
    #[serde(default)]
    negative_examples: Vec<String>,
}

pub(crate) async fn list_tasks(
    State(state): State<AppState>,
    Path(board): Path<String>,
    RawQuery(raw_query): RawQuery,
    query: Result<Query<TaskListQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<TaskDto>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(
        query.limit,
        kanban_sqlite::MAX_TASK_LIST_LIMIT,
        query.offset,
    )?;
    let statuses = parse_status_filters(raw_query.as_deref())?;
    let priorities = parse_priority_filters(raw_query.as_deref())?;
    let labels = parse_label_filters(raw_query.as_deref())?;
    let assignee = query
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let search = query
        .q
        .as_deref()
        .or(query.search.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let sort = parse_task_sort(query.sort.as_deref())?;
    let page = kanban_sqlite::list_tasks_page(
        state.db_path(),
        &board,
        kanban_sqlite::TaskListOptions {
            statuses,
            priorities,
            labels,
            include_archived: query.include_archived,
            assignee,
            search,
            sort,
            limit: query.limit,
            offset: query.offset,
        },
    )?;
    let tasks = page.tasks.into_iter().map(TaskDto::from).collect();
    Ok(Json(Envelope {
        data: tasks,
        meta: Some(json!({ "limit": query.limit, "offset": query.offset, "total": page.total })),
    }))
}

pub(crate) fn parse_label_filters(raw_query: Option<&str>) -> Result<Vec<String>, ApiError> {
    let Some(raw_query) = raw_query else {
        return Ok(Vec::new());
    };
    let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(raw_query)
        .map_err(|error| invalid_input(error.to_string()))?;
    Ok(pairs
        .into_iter()
        .filter_map(|(key, value)| {
            (key == "label")
                .then(|| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        })
        .collect())
}

pub(crate) async fn create_task(
    State(state): State<AppState>,
    Path(board): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CreateTaskBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<TaskDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let input = kanban_sqlite::CreateTask {
        title: body.title,
        description: body.description,
        status: body.status,
        assignee: body.assignee,
        priority: body.priority,
        scheduled_at: body.scheduled_at,
        due_at: body.due_at,
        max_retries: body.max_retries,
        metadata_json: metadata_json(body.metadata)?,
    };
    let task = kanban_sqlite::create_task_with_labels_and_dependencies(
        state.db_path(),
        &board,
        &actor,
        input,
        &body.labels,
        &body.depends_on,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: TaskDto::from(task),
            meta: None,
        }),
    ))
}

pub(crate) async fn list_board_labels(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<Vec<LabelDto>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::list_labels(state.db_path(), &board)?
            .into_iter()
            .map(LabelDto::from)
            .collect(),
        meta: None,
    }))
}

pub(crate) async fn create_board_label(
    State(state): State<AppState>,
    Path(board): Path<String>,
    body: Result<Json<CreateLabelBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<LabelDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let label = kanban_sqlite::create_label(
        state.db_path(),
        &board,
        kanban_sqlite::CreateLabel {
            name: body.name,
            color: body.color,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: LabelDto::from(label),
            meta: None,
        }),
    ))
}

pub(crate) async fn list_label_semantics(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::LabelSemanticsRecord>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::list_label_semantics(state.db_path(), &board)?,
        meta: None,
    }))
}

pub(crate) async fn get_label_semantics(
    State(state): State<AppState>,
    Path((board, label_id)): Path<(String, String)>,
) -> Result<Json<Envelope<kanban_sqlite::LabelSemanticsRecord>>, ApiError> {
    let label_id = require_label_id_path(label_id)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::get_label_semantics_by_id(state.db_path(), &board, &label_id)?,
        meta: None,
    }))
}

pub(crate) async fn upsert_label_semantics(
    State(state): State<AppState>,
    Path((board, label_id)): Path<(String, String)>,
    body: Result<Json<UpsertLabelSemanticsBody>, JsonRejection>,
) -> Result<Json<Envelope<kanban_sqlite::LabelSemanticsRecord>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let label_id = require_label_id_path(label_id)?;
    Ok(Json(Envelope {
        data: kanban_sqlite::upsert_label_semantics_by_id(
            state.db_path(),
            &board,
            &label_id,
            kanban_sqlite::UpsertLabelSemantics {
                label_ref: label_id.clone(),
                description: body.description,
                applies_when: body.applies_when,
                excludes_when: body.excludes_when,
                positive_examples: body.positive_examples,
                negative_examples: body.negative_examples,
            },
        )?,
        meta: None,
    }))
}

pub(crate) async fn delete_label_semantics(
    State(state): State<AppState>,
    Path((board, label_id)): Path<(String, String)>,
) -> Result<Json<Envelope<serde_json::Value>>, ApiError> {
    let label_id = require_label_id_path(label_id)?;
    kanban_sqlite::delete_label_semantics_by_id(state.db_path(), &board, &label_id)?;
    Ok(Json(Envelope {
        data: json!({ "deleted": true }),
        meta: None,
    }))
}

fn require_label_id_path(label_id: String) -> Result<String, ApiError> {
    let label_id = label_id.trim();
    if label_id.starts_with("l_") {
        Ok(label_id.to_owned())
    } else {
        Err(invalid_input("label_id must be a canonical l_ id"))
    }
}

pub(crate) async fn list_label_atoms(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::LabelAtomRecord>>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::list_label_atoms(state.db_path(), &board)?,
        meta: None,
    }))
}

pub(crate) async fn explain_label_atom(
    State(state): State<AppState>,
    Path((board, atom_ref)): Path<(String, String)>,
) -> Result<Json<Envelope<kanban_sqlite::LabelAtomExplainRecord>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::explain_label_atom(state.db_path(), &board, &atom_ref)?,
        meta: None,
    }))
}

pub(crate) async fn label_atom_index_status(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<kanban_vector::VectorStoreStatus>>, ApiError> {
    let index_state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        label_atom_index_status_for_state(&index_state, &board)
    })
    .await
    .map_err(|error| {
        ApiError(kanban_core::KanbanError::Storage(format!(
            "label atom index status worker failed: {error}"
        )))
    })??;
    Ok(Json(Envelope {
        data: result,
        meta: None,
    }))
}

pub(crate) async fn rebuild_label_atom_index(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Envelope<kanban_vector::VectorStoreStatus>>, ApiError> {
    let index_state = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        rebuild_label_atom_index_for_state(&index_state, &board)
    })
    .await
    .map_err(|error| {
        ApiError(kanban_core::KanbanError::Storage(format!(
            "label atom index rebuild worker failed: {error}"
        )))
    })??;
    Ok(Json(Envelope {
        data: result,
        meta: None,
    }))
}

pub(crate) async fn query_label_atom_index(
    State(state): State<AppState>,
    Path(board): Path<String>,
    query: Result<Query<LabelAtomIndexQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<kanban_vector::LabelAtomHit>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    let text = query.q.trim();
    if text.is_empty() {
        return Err(invalid_input("q is required"));
    }
    let polarity = query
        .polarity
        .as_deref()
        .map(parse_label_atom_polarity)
        .transpose()?;
    let index_state = state.clone();
    let index_board = board;
    let index_text = text.to_owned();
    let index_polarity = polarity;
    let index_limit = query.limit;
    let result = tokio::task::spawn_blocking(move || {
        query_label_atom_index_for_state(
            &index_state,
            &index_board,
            &index_text,
            index_polarity,
            index_limit,
        )
    })
    .await
    .map_err(|error| {
        ApiError(kanban_core::KanbanError::Storage(format!(
            "label atom index query worker failed: {error}"
        )))
    })??;
    Ok(Json(Envelope {
        data: result,
        meta: None,
    }))
}

pub(crate) async fn list_task_labels(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<Vec<LabelDto>>>, ApiError> {
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    Ok(Json(Envelope {
        data: task.labels.into_iter().map(LabelDto::from).collect(),
        meta: None,
    }))
}

pub(crate) async fn suggest_task_labels(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    query: Result<Query<LabelSuggestionQuery>, QueryRejection>,
) -> Result<Json<Envelope<kanban_sqlite::LabelSuggestionResult>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let options = label_suggestion_options(query)?;
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let label_state = state.clone();
    let label_board = task.board_slug;
    let label_task_id = task_id;
    let result = tokio::task::spawn_blocking(move || {
        suggest_task_labels_for_state(&label_state, &label_board, &label_task_id, options)
    })
    .await
    .map_err(|error| {
        ApiError(kanban_core::KanbanError::Storage(format!(
            "label suggestion worker failed: {error}"
        )))
    })??;
    Ok(Json(Envelope {
        data: result,
        meta: None,
    }))
}

pub(crate) async fn add_task_label(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<AddTaskLabelBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<TaskDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let label_names = body.label_names()?;
    let task =
        kanban_sqlite::add_task_labels_by_id(state.db_path(), &actor, &task_id, &label_names)?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: TaskDto::from(task),
            meta: None,
        }),
    ))
}

pub(crate) async fn bootstrap_task_label(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<BootstrapTaskLabelBody>, JsonRejection>,
) -> Result<(StatusCode, Json<Envelope<BootstrapTaskLabelDto>>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let result = kanban_sqlite::bootstrap_task_label_by_id(
        state.db_path(),
        &actor,
        &task_id,
        kanban_sqlite::BootstrapTaskLabel {
            name: body.name,
            description: body.description,
            applies_when: body.applies_when,
            excludes_when: body.excludes_when,
            positive_examples: body.positive_examples,
            negative_examples: body.negative_examples,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: BootstrapTaskLabelDto {
                task: TaskDto::from(result.task),
                semantics: result.semantics,
            },
            meta: None,
        }),
    ))
}

pub(crate) async fn propose_task_label(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    query: Result<Query<LabelSuggestionQuery>, QueryRejection>,
    headers: HeaderMap,
    body: Result<Json<LabelProposalBody>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<Envelope<kanban_sqlite::LabelProposalAttempt>>,
    ),
    ApiError,
> {
    let Query(query) = query.map_err(extractor_error)?;
    let options = label_suggestion_options(query)?;
    let body = match body {
        Ok(Json(body)) => body,
        Err(JsonRejection::MissingJsonContentType(_)) => LabelProposalBody {
            proposal: None,
            actor: None,
        },
        Err(error) => return Err(extractor_error(error)),
    };
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let label_state = state.clone();
    let label_board = task.board_slug;
    let label_task_id = task.id;
    let label_actor = actor;
    let label_candidate = body.proposal;
    let attempt = tokio::task::spawn_blocking(move || {
        propose_task_label_for_state(
            &label_state,
            &label_board,
            &label_actor,
            &label_task_id,
            label_candidate,
            options,
        )
    })
    .await
    .map_err(|error| {
        ApiError(kanban_core::KanbanError::Storage(format!(
            "label proposal worker failed: {error}"
        )))
    })??;
    Ok((
        if attempt.proposal.is_some() {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(Envelope {
            data: attempt,
            meta: None,
        }),
    ))
}

fn label_suggestion_options(
    query: LabelSuggestionQuery,
) -> Result<kanban_sqlite::LabelSuggestionOptions, ApiError> {
    validate_label_suggestion_bound("limit", query.limit)?;
    validate_label_suggestion_bound("candidate_limit", query.candidate_limit)?;
    validate_label_suggestion_bound("atom_limit", query.atom_limit)?;
    validate_label_suggestion_bound("max_selected_labels", query.max_selected_labels)?;
    Ok(kanban_sqlite::LabelSuggestionOptions {
        output_limit: query.limit,
        candidate_limit: query.candidate_limit,
        atom_limit: query.atom_limit,
        max_selected_labels: query.max_selected_labels,
        min_score: query.min_score,
    })
}

fn validate_label_suggestion_bound(name: &str, value: usize) -> Result<(), ApiError> {
    if value == 0 {
        return Err(invalid_input(format!("{name} must be >= 1")));
    }
    validate_page_bounds(value, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)
}

pub(crate) async fn list_task_label_proposals(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::LabelSemanticProposalRecord>>>, ApiError> {
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let proposals = kanban_sqlite::list_label_proposals(
        state.db_path(),
        &task.board_slug,
        kanban_sqlite::LabelProposalListOptions {
            task_ref: Some(task.id),
            status: None,
        },
    )?;
    Ok(Json(Envelope {
        data: proposals,
        meta: None,
    }))
}

pub(crate) async fn record_label_ontology_observation(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    body: Result<Json<LabelOntologyObservationBody>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<Envelope<kanban_sqlite::LabelOntologyObservationRecord>>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let observation = kanban_sqlite::record_label_ontology_observation(
        state.db_path(),
        &task.board_slug,
        &task.id,
        label_ontology_observation_input(body)?,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: observation,
            meta: None,
        }),
    ))
}

pub(crate) async fn list_label_ontology_signals(
    State(state): State<AppState>,
    Path(board): Path<String>,
    RawQuery(raw_query): RawQuery,
    query: Result<Query<LabelOntologySignalQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::LabelOntologySignalRecord>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    let signals = kanban_sqlite::list_label_ontology_signals(
        state.db_path(),
        &board,
        kanban_sqlite::LabelOntologySignalListOptions {
            statuses: parse_label_ontology_status_filters(raw_query.as_deref())?,
            kinds: parse_label_ontology_kind_filters(raw_query.as_deref())?,
            task_ref: query.task_ref,
            target_label_ref: query.target_label_ref,
            proposed_label_name: query.proposed_label_name,
            include_all: query.include_all,
            limit: query.limit,
        },
    )?;
    Ok(Json(Envelope {
        data: signals,
        meta: Some(json!({ "limit": query.limit })),
    }))
}

pub(crate) async fn review_label_ontology(
    State(state): State<AppState>,
    Path(board): Path<String>,
    query: Result<Query<LabelOntologyReviewQuery>, QueryRejection>,
) -> Result<Json<Envelope<Vec<kanban_sqlite::LabelOntologyReviewGroup>>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::MAX_TASK_LIST_LIMIT, 0)?;
    let group_by = kanban_sqlite::LabelOntologyReviewGroupBy::from_str(&query.group_by)?;
    let groups = kanban_sqlite::review_label_ontology(
        state.db_path(),
        &board,
        kanban_sqlite::LabelOntologyReviewOptions {
            group_by,
            include_all: query.include_all,
            limit: query.limit,
        },
    )?;
    Ok(Json(Envelope {
        data: groups,
        meta: Some(json!({
            "group_by": group_by,
            "include_all": query.include_all,
            "limit": query.limit
        })),
    }))
}

pub(crate) async fn get_label_ontology_signal(
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
) -> Result<Json<Envelope<kanban_sqlite::LabelOntologySignalDetail>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::get_label_ontology_signal(state.db_path(), &signal_id)?,
        meta: None,
    }))
}

pub(crate) async fn create_label_ontology_action(
    State(state): State<AppState>,
    Path(board): Path<String>,
    body: Result<Json<LabelOntologyActionBody>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<Envelope<kanban_sqlite::LabelOntologyActionRecord>>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let action = kanban_sqlite::create_label_ontology_action(
        state.db_path(),
        &board,
        label_ontology_action_input(body)?,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: action,
            meta: None,
        }),
    ))
}

pub(crate) async fn apply_label_ontology_atom(
    State(state): State<AppState>,
    Path(board): Path<String>,
    body: Result<Json<LabelOntologyAtomApplyBody>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<Envelope<kanban_sqlite::LabelOntologyActionRecord>>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let action = kanban_sqlite::apply_label_ontology_atom(
        state.db_path(),
        &board,
        label_ontology_atom_apply_input(body),
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: action,
            meta: None,
        }),
    ))
}

pub(crate) async fn validate_label_ontology_action(
    State(state): State<AppState>,
    Path(board): Path<String>,
    body: Result<Json<LabelOntologyValidationBody>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<Envelope<kanban_sqlite::LabelOntologyActionRecord>>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let action = kanban_sqlite::validate_label_ontology_action(
        state.db_path(),
        &board,
        label_ontology_validation_input(body)?,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(Envelope {
            data: action,
            meta: None,
        }),
    ))
}

pub(crate) async fn get_label_proposal(
    State(state): State<AppState>,
    Path(proposal_id): Path<String>,
) -> Result<Json<Envelope<kanban_sqlite::LabelSemanticProposalRecord>>, ApiError> {
    Ok(Json(Envelope {
        data: kanban_sqlite::get_label_proposal(state.db_path(), &proposal_id)?,
        meta: None,
    }))
}

pub(crate) async fn accept_label_proposal(
    State(state): State<AppState>,
    Path(proposal_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<LabelProposalDecisionBody>, JsonRejection>,
) -> Result<Json<Envelope<kanban_sqlite::LabelSemanticProposalRecord>>, ApiError> {
    let body = optional_decision_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let ontology_actor = body.ontology_actor.map(label_ontology_actor_input);
    Ok(Json(Envelope {
        data: kanban_sqlite::accept_label_proposal_with_options(
            state.db_path(),
            &actor,
            &proposal_id,
            body.reason,
            kanban_sqlite::LabelProposalDecisionOptions {
                source_signal_ids: body.source_signal_ids,
                ontology_actor,
            },
        )?,
        meta: None,
    }))
}

pub(crate) async fn reject_label_proposal(
    State(state): State<AppState>,
    Path(proposal_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<LabelProposalDecisionBody>, JsonRejection>,
) -> Result<Json<Envelope<kanban_sqlite::LabelSemanticProposalRecord>>, ApiError> {
    let body = optional_decision_body(body)?;
    if !body.source_signal_ids.is_empty() {
        return Err(invalid_input(
            "source_signal_ids are only supported when accepting label proposals",
        ));
    }
    if body.ontology_actor.is_some() {
        return Err(invalid_input(
            "ontology_actor is only supported when accepting label proposals",
        ));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    Ok(Json(Envelope {
        data: kanban_sqlite::reject_label_proposal(
            state.db_path(),
            &actor,
            &proposal_id,
            body.reason,
        )?,
        meta: None,
    }))
}

fn optional_decision_body(
    body: Result<Json<LabelProposalDecisionBody>, JsonRejection>,
) -> Result<LabelProposalDecisionBody, ApiError> {
    match body {
        Ok(Json(body)) => Ok(body),
        Err(JsonRejection::MissingJsonContentType(_)) => Ok(LabelProposalDecisionBody {
            reason: None,
            actor: None,
            source_signal_ids: Vec::new(),
            ontology_actor: None,
        }),
        Err(error) => Err(extractor_error(error)),
    }
}

fn label_ontology_observation_input(
    body: LabelOntologyObservationBody,
) -> Result<kanban_sqlite::LabelOntologyRecordInput, ApiError> {
    let (agent_candidates_json, _) = coalesce_json_body_field(
        "agent_candidates",
        body.agent_candidates,
        "agent_candidates_json",
        body.agent_candidates_json,
        JsonBodyShape::Array,
        empty_json_array(),
    )?;
    let (suggestion_snapshot_json, suggestion_snapshot) = coalesce_json_body_field(
        "suggestion_snapshot",
        body.suggestion_snapshot,
        "suggestion_snapshot_json",
        body.suggestion_snapshot_json,
        JsonBodyShape::Object,
        empty_json_object(),
    )?;
    let (final_decision_json, _) = coalesce_json_body_field(
        "final_decision",
        body.final_decision,
        "final_decision_json",
        body.final_decision_json,
        JsonBodyShape::Object,
        empty_json_object(),
    )?;
    let diagnostics_json = derive_diagnostics_json(
        body.diagnostics,
        body.diagnostics_json,
        &suggestion_snapshot,
    )?;
    Ok(kanban_sqlite::LabelOntologyRecordInput {
        actor: kanban_sqlite::LabelOntologyActor {
            name: body.actor.name,
            actor_type: body.actor.actor_type,
            agent_type: body.actor.agent_type,
        },
        agent_candidates_json,
        suggestion_snapshot_json,
        final_decision_json,
        suggest_coverage: derive_snapshot_f64(
            body.suggest_coverage,
            &suggestion_snapshot,
            "coverage",
            "suggest_coverage",
        )?,
        suggest_coverage_cosine: derive_snapshot_f64(
            body.suggest_coverage_cosine,
            &suggestion_snapshot,
            "coverage_cosine",
            "suggest_coverage_cosine",
        )?,
        suggest_residual_norm: derive_snapshot_f64(
            body.suggest_residual_norm,
            &suggestion_snapshot,
            "residual_norm",
            "suggest_residual_norm",
        )?,
        suggest_needs_new_label: derive_snapshot_bool(
            body.suggest_needs_new_label,
            &suggestion_snapshot,
            "needs_new_label",
            "suggest_needs_new_label",
        )?
        .unwrap_or(false),
        suggest_degraded: derive_snapshot_bool(
            body.suggest_degraded,
            &suggestion_snapshot,
            "degraded",
            "suggest_degraded",
        )?
        .unwrap_or(false),
        diagnostics_json,
        capture_fingerprint: body.capture_fingerprint,
        signals: body
            .signals
            .into_iter()
            .map(label_ontology_signal_input)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn label_ontology_signal_input(
    body: LabelOntologySignalBody,
) -> Result<kanban_sqlite::LabelOntologySignalInput, ApiError> {
    let (related_labels_json, _) = coalesce_json_body_field(
        "related_labels",
        body.related_labels,
        "related_labels_json",
        body.related_labels_json,
        JsonBodyShape::Array,
        empty_json_array(),
    )?;
    let (proposal_json, _) = coalesce_json_body_field(
        "proposal",
        body.proposal,
        "proposal_json",
        body.proposal_json,
        JsonBodyShape::Object,
        empty_json_object(),
    )?;
    Ok(kanban_sqlite::LabelOntologySignalInput {
        kind: body.kind,
        target_label_ref: body.target_label_ref,
        related_labels_json,
        proposed_action: body.proposed_action,
        candidate_atom: body.candidate_atom.map(|candidate| {
            kanban_sqlite::LabelOntologyCandidateAtomInput {
                polarity: candidate.polarity,
                kind: candidate.kind,
                text: candidate.text,
            }
        }),
        proposed_label_name: body.proposed_label_name,
        proposal_json,
        agent_selected: body.agent_selected,
        suggest_state: body.suggest_state,
        suggest_score: body.suggest_score,
        suggest_rank: body.suggest_rank,
        final_selected: body.final_selected,
        rationale: body.rationale,
        confidence: body.confidence,
        signal_key: body.signal_key,
    })
}

fn label_ontology_actor_input(body: LabelOntologyActorBody) -> kanban_sqlite::LabelOntologyActor {
    kanban_sqlite::LabelOntologyActor {
        name: body.name,
        actor_type: body.actor_type,
        agent_type: body.agent_type,
    }
}

fn label_ontology_action_input(
    body: LabelOntologyActionBody,
) -> Result<kanban_sqlite::LabelOntologyActionInput, ApiError> {
    Ok(kanban_sqlite::LabelOntologyActionInput {
        actor: label_ontology_actor_input(body.actor),
        action_type: body.action_type,
        signal_ids: body.signal_ids,
        reason: body.reason,
        superseded_by_signal_id: body.superseded_by_signal_id,
        parent_action_id: body.parent_action_id,
        target_label_ref: body.target_label_ref,
        result_label_ref: body.result_label_ref,
        result_atom_id: body.result_atom_id,
        result_atom_content_hash: body.result_atom_content_hash,
        result_proposal_id: body.result_proposal_id,
        canonical_before_hash: body.canonical_before_hash,
        canonical_after_hash: body.canonical_after_hash,
        change_json: coalesce_optional_json_body_field(
            "change",
            body.change,
            "change_json",
            body.change_json,
            JsonBodyShape::Object,
        )?,
        validation_status: body.validation_status,
        validation_json: coalesce_optional_json_body_field(
            "validation",
            body.validation,
            "validation_json",
            body.validation_json,
            JsonBodyShape::Object,
        )?,
    })
}

fn label_ontology_atom_apply_input(
    body: LabelOntologyAtomApplyBody,
) -> kanban_sqlite::LabelOntologyAtomApplyInput {
    kanban_sqlite::LabelOntologyAtomApplyInput {
        actor: label_ontology_actor_input(body.actor),
        signal_ids: body.signal_ids,
        label_ref: body.label_ref,
        kind: body.kind,
        text: body.text,
        reason: body.reason,
    }
}

fn label_ontology_validation_input(
    body: LabelOntologyValidationBody,
) -> Result<kanban_sqlite::LabelOntologyValidationInput, ApiError> {
    Ok(kanban_sqlite::LabelOntologyValidationInput {
        actor: label_ontology_actor_input(body.actor),
        parent_action_id: body.parent_action_id,
        signal_ids: body.signal_ids,
        reason: body.reason,
        validation_status: body.validation_status,
        validation_json: coalesce_required_json_body_field(
            "validation",
            body.validation,
            "validation_json",
            body.validation_json,
            JsonBodyShape::Object,
        )?,
    })
}

fn coalesce_json_body_field(
    new_name: &str,
    new_value: JsonBodyField,
    legacy_name: &str,
    legacy_value: Option<String>,
    shape: JsonBodyShape,
    default_value: JsonValue,
) -> Result<(String, JsonValue), ApiError> {
    let value =
        coalesce_optional_json_body_value(new_name, new_value, legacy_name, legacy_value, shape)?
            .unwrap_or(default_value);
    let text = json_body_to_string(&value)?;
    Ok((text, value))
}

fn coalesce_optional_json_body_field(
    new_name: &str,
    new_value: JsonBodyField,
    legacy_name: &str,
    legacy_value: Option<String>,
    shape: JsonBodyShape,
) -> Result<Option<String>, ApiError> {
    coalesce_optional_json_body_value(new_name, new_value, legacy_name, legacy_value, shape)?
        .map(|value| json_body_to_string(&value))
        .transpose()
}

fn coalesce_required_json_body_field(
    new_name: &str,
    new_value: JsonBodyField,
    legacy_name: &str,
    legacy_value: Option<String>,
    shape: JsonBodyShape,
) -> Result<String, ApiError> {
    coalesce_optional_json_body_field(new_name, new_value, legacy_name, legacy_value, shape)?
        .ok_or_else(|| invalid_input(format!("{new_name} is required")))
}

fn coalesce_optional_json_body_value(
    new_name: &str,
    new_value: JsonBodyField,
    legacy_name: &str,
    legacy_value: Option<String>,
    shape: JsonBodyShape,
) -> Result<Option<JsonValue>, ApiError> {
    if new_value.is_present() && legacy_value.is_some() {
        return Err(invalid_input(format!(
            "{new_name} and {legacy_name} cannot both be supplied"
        )));
    }
    if let Some(value) = new_value.into_value() {
        return ensure_json_body_shape(value, new_name, shape).map(Some);
    }
    if let Some(raw) = legacy_value {
        let value = serde_json::from_str::<JsonValue>(&raw).map_err(|error| {
            invalid_input(format!("{legacy_name} must contain valid JSON: {error}"))
        })?;
        return ensure_json_body_shape(value, legacy_name, shape).map(Some);
    }
    Ok(None)
}

fn ensure_json_body_shape(
    value: JsonValue,
    field_name: &str,
    shape: JsonBodyShape,
) -> Result<JsonValue, ApiError> {
    let ok = match shape {
        JsonBodyShape::Array => value.is_array(),
        JsonBodyShape::Object => value.is_object(),
    };
    if ok {
        Ok(value)
    } else {
        Err(invalid_input(format!(
            "{field_name} must be a JSON {}",
            shape.name()
        )))
    }
}

fn derive_snapshot_f64(
    supplied: Option<f64>,
    snapshot: &JsonValue,
    snapshot_field: &str,
    supplied_field: &str,
) -> Result<Option<f64>, ApiError> {
    let derived = optional_snapshot_f64(snapshot, snapshot_field)?;
    if let (Some(supplied), Some(derived)) = (supplied, derived)
        && (supplied - derived).abs() > f64::EPSILON
    {
        return Err(invalid_input(format!(
            "{supplied_field} conflicts with suggestion_snapshot.{snapshot_field}"
        )));
    }
    Ok(supplied.or(derived))
}

fn derive_snapshot_bool(
    supplied: Option<bool>,
    snapshot: &JsonValue,
    snapshot_field: &str,
    supplied_field: &str,
) -> Result<Option<bool>, ApiError> {
    let derived = optional_snapshot_bool(snapshot, snapshot_field)?;
    if let (Some(supplied), Some(derived)) = (supplied, derived)
        && supplied != derived
    {
        return Err(invalid_input(format!(
            "{supplied_field} conflicts with suggestion_snapshot.{snapshot_field}"
        )));
    }
    Ok(supplied.or(derived))
}

fn derive_diagnostics_json(
    diagnostics: JsonBodyField,
    diagnostics_json: Option<String>,
    snapshot: &JsonValue,
) -> Result<String, ApiError> {
    let supplied = coalesce_optional_json_body_value(
        "diagnostics",
        diagnostics,
        "diagnostics_json",
        diagnostics_json,
        JsonBodyShape::Array,
    )?;
    let derived = optional_snapshot_array(snapshot, "diagnostics")?;
    if let (Some(supplied), Some(derived)) = (&supplied, &derived)
        && supplied != derived
    {
        return Err(invalid_input(
            "diagnostics conflicts with suggestion_snapshot.diagnostics",
        ));
    }
    let value = supplied.or(derived).unwrap_or_else(empty_json_array);
    json_body_to_string(&value)
}

fn optional_snapshot_f64(snapshot: &JsonValue, field: &str) -> Result<Option<f64>, ApiError> {
    let Some(value) = snapshot.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_f64()
        .map(Some)
        .ok_or_else(|| invalid_input(format!("suggestion_snapshot.{field} must be a JSON number")))
}

fn optional_snapshot_bool(snapshot: &JsonValue, field: &str) -> Result<Option<bool>, ApiError> {
    let Some(value) = snapshot.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value.as_bool().map(Some).ok_or_else(|| {
        invalid_input(format!(
            "suggestion_snapshot.{field} must be a JSON boolean"
        ))
    })
}

fn optional_snapshot_array(
    snapshot: &JsonValue,
    field: &str,
) -> Result<Option<JsonValue>, ApiError> {
    let Some(value) = snapshot.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    ensure_json_body_shape(
        value.clone(),
        &format!("suggestion_snapshot.{field}"),
        JsonBodyShape::Array,
    )
    .map(Some)
}

fn json_body_to_string(value: &JsonValue) -> Result<String, ApiError> {
    serde_json::to_string(value).map_err(|error| invalid_input(error.to_string()))
}

fn empty_json_array() -> JsonValue {
    JsonValue::Array(Vec::new())
}

fn empty_json_object() -> JsonValue {
    JsonValue::Object(serde_json::Map::new())
}

fn parse_label_ontology_status_filters(
    raw_query: Option<&str>,
) -> Result<Vec<kanban_sqlite::LabelOntologySignalStatus>, ApiError> {
    parse_label_ontology_filters(raw_query, "status")
}

fn parse_label_ontology_kind_filters(
    raw_query: Option<&str>,
) -> Result<Vec<kanban_sqlite::LabelOntologySignalKind>, ApiError> {
    parse_label_ontology_filters(raw_query, "kind")
}

fn parse_label_ontology_filters<T>(
    raw_query: Option<&str>,
    filter_name: &str,
) -> Result<Vec<T>, ApiError>
where
    T: FromStr,
    T::Err: Into<kanban_core::KanbanError>,
{
    let Some(raw_query) = raw_query else {
        return Ok(Vec::new());
    };
    let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(raw_query)
        .map_err(|error| invalid_input(error.to_string()))?;
    pairs
        .into_iter()
        .filter_map(|(key, value)| (key == filter_name).then_some(value))
        .map(|value| {
            value
                .trim()
                .parse::<T>()
                .map_err(Into::into)
                .map_err(ApiError)
        })
        .collect()
}

fn suggest_task_labels_for_state(
    state: &AppState,
    board: &str,
    task_id: &str,
    options: kanban_sqlite::LabelSuggestionOptions,
) -> Result<kanban_sqlite::LabelSuggestionResult, ApiError> {
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = super::shared::configured_lancedb_store(state)? {
            return kanban_sqlite::suggest_task_labels_with(
                state.db_path(),
                board,
                task_id,
                &store,
                options,
            )
            .map_err(ApiError::from);
        }
    }
    kanban_sqlite::suggest_task_labels(state.db_path(), board, task_id, options)
        .map_err(ApiError::from)
}

fn propose_task_label_for_state(
    state: &AppState,
    board: &str,
    actor: &str,
    task_id: &str,
    candidate: Option<kanban_sqlite::LabelProposalCandidate>,
    options: kanban_sqlite::LabelSuggestionOptions,
) -> Result<kanban_sqlite::LabelProposalAttempt, ApiError> {
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = super::shared::configured_lancedb_store(state)? {
            return match candidate {
                Some(candidate) => {
                    let provider = kanban_sqlite::ManualLabelProposalProvider::new(candidate);
                    kanban_sqlite::propose_task_label_with_store(
                        state.db_path(),
                        board,
                        actor,
                        task_id,
                        &provider,
                        &store,
                        options,
                    )
                }
                None => kanban_sqlite::propose_task_label_with_store(
                    state.db_path(),
                    board,
                    actor,
                    task_id,
                    &kanban_sqlite::DisabledLabelProposalProvider,
                    &store,
                    options,
                ),
            }
            .map_err(ApiError::from);
        }
    }
    match candidate {
        Some(candidate) => {
            let provider = kanban_sqlite::ManualLabelProposalProvider::new(candidate);
            kanban_sqlite::propose_task_label_with(
                state.db_path(),
                board,
                actor,
                task_id,
                &provider,
                options,
            )
        }
        None => kanban_sqlite::propose_task_label_with(
            state.db_path(),
            board,
            actor,
            task_id,
            &kanban_sqlite::DisabledLabelProposalProvider,
            options,
        ),
    }
    .map_err(ApiError::from)
}

fn label_atom_index_status_for_state(
    state: &AppState,
    board: &str,
) -> Result<kanban_vector::VectorStoreStatus, ApiError> {
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = super::shared::configured_lancedb_store(state)? {
            return kanban_sqlite::label_atom_index_status_with(state.db_path(), board, &store)
                .map_err(ApiError::from);
        }
    }
    kanban_sqlite::label_atom_index_status(state.db_path(), board).map_err(ApiError::from)
}

fn rebuild_label_atom_index_for_state(
    state: &AppState,
    board: &str,
) -> Result<kanban_vector::VectorStoreStatus, ApiError> {
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = super::shared::configured_lancedb_store(state)? {
            return kanban_sqlite::rebuild_label_atom_index_with(state.db_path(), board, &store)
                .map_err(ApiError::from);
        }
    }
    #[cfg(not(feature = "vector-lancedb"))]
    let _ = (state, board);
    Err(invalid_input(
        "label atom index rebuild requires a configured label atom vector store",
    ))
}

fn query_label_atom_index_for_state(
    state: &AppState,
    board: &str,
    text: &str,
    polarity: Option<String>,
    limit: usize,
) -> Result<Vec<kanban_vector::LabelAtomHit>, ApiError> {
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = super::shared::configured_lancedb_store(state)? {
            return kanban_sqlite::query_label_atom_index_with(
                state.db_path(),
                board,
                &store,
                kanban_vector::LabelAtomQuery {
                    text: text.to_owned(),
                    limit,
                    board_id: None,
                    embedding_model: None,
                    polarity,
                },
            )
            .map_err(ApiError::from);
        }
    }
    #[cfg(not(feature = "vector-lancedb"))]
    let _ = (state, board, text, polarity, limit);
    Err(invalid_input(
        "label atom index query requires a configured label atom vector store",
    ))
}

fn parse_label_atom_polarity(value: &str) -> Result<String, ApiError> {
    match value.trim() {
        "positive" => Ok("positive".to_owned()),
        "negative" => Ok("negative".to_owned()),
        other => Err(invalid_input(format!(
            "unsupported label atom polarity: {other}"
        ))),
    }
}

pub(crate) async fn remove_task_label(
    State(state): State<AppState>,
    Path((task_id, label_ref)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let actor = actor(None, &headers, &state);
    let task =
        kanban_sqlite::remove_task_label_by_id(state.db_path(), &actor, &task_id, &label_ref)?;
    Ok(Json(Envelope {
        data: TaskDto::from(task),
        meta: None,
    }))
}

pub(crate) async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    query: Result<Query<TaskGetQuery>, QueryRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let include_ontology = task_get_includes_ontology(query.include.as_deref());
    let task = kanban_sqlite::get_task_by_id_global(state.db_path(), &task_id)?;
    let meta = if include_ontology {
        Some(json!({
            "details": {
                "ontology_summary": kanban_sqlite::task_ontology_summary_by_id_global(
                    state.db_path(),
                    &task_id,
                )?
            }
        }))
    } else {
        None
    };
    Ok(Json(Envelope {
        data: TaskDto::from(task),
        meta,
    }))
}

pub(crate) async fn update_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<serde_json::Value>, JsonRejection>,
) -> Result<Json<Envelope<TaskDto>>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let object = body
        .as_object()
        .ok_or_else(|| invalid_input("request body must be a JSON object"))?;
    for forbidden in ["status", "claim_token", "current_run_id", "completed_at"] {
        if object.contains_key(forbidden) {
            return Err(invalid_input(format!("{forbidden} cannot be patched")));
        }
    }
    let body_actor = object.get("actor").and_then(|value| value.as_str());
    let actor = actor(body_actor, &headers, &state);
    let retry_policy = retry_policy_from_value(object)?;
    let mut patch = patch_from_value(object)?;
    patch.max_retries = retry_policy;
    let task = kanban_sqlite::update_task_by_id(state.db_path(), &actor, &task_id, patch)?;
    Ok(Json(Envelope {
        data: TaskDto::from(task),
        meta: None,
    }))
}
