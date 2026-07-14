use axum::{
    Json,
    extract::{
        FromRequestParts, Path, Query, RawQuery, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode, request::Parts},
};
use kanban_application::api::{self as application_api, TaskPlanFilter};
use kanban_contract::{
    ApiCreateTaskStatus, ApiLabel, ApiTask, ApiTaskPriority, ApiTaskStatus, CreateTaskPath,
    CreateTaskRequest, CreateTaskResponse, CreatedLabelsMeta, DataEnvelope, DeleteResponse,
    DeleteResult, LabelOntologyReviewMeta, ListTasksByStatusData, ListTasksByStatusPath,
    ListTasksByStatusQuery, ListTasksByStatusResponse, ListTasksStatusWindow,
    MAX_TASK_READ_ASSIGNEE_CHARS, MAX_TASK_READ_LABEL_CHARS, MAX_TASK_READ_LABELS,
    MAX_TASK_READ_LIMIT, MAX_TASK_READ_PLAN_FILTERS, MAX_TASK_READ_PRIORITIES,
    MAX_TASK_READ_Q_CHARS, MAX_TASK_READ_QUERY_BYTES, MAX_TASK_READ_QUERY_PAIRS,
    MAX_TASK_READ_STATUSES, MetadataEnvelope, OptionalMetadataEnvelope, SignalFilterMeta,
    TaskOntologyDetails, TaskOntologyDetailsMeta, TaskReadLabel, TaskReadPlanFilter, TaskReadSort,
};
use kanban_contract::{
    GetTaskPath, GetTaskQuery, GetTaskResponse, ListTasksPath, ListTasksQuery, ListTasksResponse,
    UpdateTaskPath, UpdateTaskRequest, UpdateTaskResponse,
};
use kanban_core::{KanbanError, TaskStatus};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value as JsonValue;
use std::{collections::BTreeSet, str::FromStr};

use crate::dto::{
    LabelAtomExplainDto, api_label_from_record, api_task_from_record, api_task_status_from_core,
    task_status_from_api,
};
use crate::error::{ApiError, extractor_error, invalid_input, validate_page_bounds};
use crate::helper::{HelperKind, helper_degraded_message, resolve_helper, run_helper_json};
use crate::state::AppState;

use super::shared::{actor, metadata_json};

const _: () = {
    assert!(MAX_TASK_READ_LIMIT == kanban_sqlite::service::MAX_TASK_LIST_LIMIT);
};

trait TaskReadQueryTarget: Default {
    fn statuses(&mut self) -> &mut Vec<ApiTaskStatus>;
    fn priorities(&mut self) -> &mut Vec<ApiTaskPriority>;
    fn labels(&mut self) -> &mut Vec<TaskReadLabel>;
    fn plan_filters(&mut self) -> &mut Vec<TaskReadPlanFilter>;
    fn set_assignee(&mut self, value: Option<String>);
    fn set_q(&mut self, value: Option<String>);
    fn set_include_archived(&mut self, value: bool);
    fn set_limit(&mut self, value: usize);
    fn set_offset(&mut self, value: usize);
    fn set_sort(&mut self, value: TaskReadSort);
}

macro_rules! impl_task_read_query_target {
    ($query:ty) => {
        impl TaskReadQueryTarget for $query {
            fn statuses(&mut self) -> &mut Vec<ApiTaskStatus> {
                &mut self.status
            }

            fn priorities(&mut self) -> &mut Vec<ApiTaskPriority> {
                &mut self.priority
            }

            fn labels(&mut self) -> &mut Vec<TaskReadLabel> {
                &mut self.label
            }

            fn plan_filters(&mut self) -> &mut Vec<TaskReadPlanFilter> {
                &mut self.plan_filter
            }

            fn set_assignee(&mut self, value: Option<String>) {
                self.assignee = value;
            }

            fn set_q(&mut self, value: Option<String>) {
                self.q = value;
            }

            fn set_include_archived(&mut self, value: bool) {
                self.include_archived = value;
            }

            fn set_limit(&mut self, value: usize) {
                self.limit = value;
            }

            fn set_offset(&mut self, value: usize) {
                self.offset = value;
            }

            fn set_sort(&mut self, value: TaskReadSort) {
                self.sort = value;
            }
        }
    };
}

impl_task_read_query_target!(ListTasksQuery);
impl_task_read_query_target!(ListTasksByStatusQuery);

#[derive(Debug)]
pub(crate) struct ListTasksRequest {
    path: ListTasksPath,
    query: ListTasksQuery,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for ListTasksRequest
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(path) = Path::<ListTasksPath>::from_request_parts(parts, state)
            .await
            .map_err(extractor_error)?;
        let query = parse_task_read_query::<ListTasksQuery>(parts.uri.query())?;
        Ok(Self { path, query })
    }
}

#[derive(Debug)]
pub(crate) struct ListTasksByStatusRequest {
    path: ListTasksByStatusPath,
    query: ListTasksByStatusQuery,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for ListTasksByStatusRequest
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(path) = Path::<ListTasksByStatusPath>::from_request_parts(parts, state)
            .await
            .map_err(extractor_error)?;
        let query = parse_task_read_query::<ListTasksByStatusQuery>(parts.uri.query())?;
        Ok(Self { path, query })
    }
}

fn decode_query_component(encoded: &str) -> Result<String, ApiError> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' => {
                let Some(high) = bytes.get(index + 1).and_then(|byte| hex_value(*byte)) else {
                    return Err(invalid_input("malformed percent-encoding in query"));
                };
                let Some(low) = bytes.get(index + 2).and_then(|byte| hex_value(*byte)) else {
                    return Err(invalid_input("malformed percent-encoding in query"));
                };
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| invalid_input("query is not valid UTF-8"))
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn scalar_parameter(seen: &mut BTreeSet<&'static str>, name: &'static str) -> Result<(), ApiError> {
    if seen.insert(name) {
        Ok(())
    } else {
        Err(invalid_input(format!(
            "duplicate scalar query parameter: {name}"
        )))
    }
}

fn push_repeated_distinct<T: PartialEq>(
    values: &mut Vec<T>,
    value: T,
    name: &'static str,
    maximum: usize,
) -> Result<(), ApiError> {
    if values.len() >= maximum {
        return Err(invalid_input(format!(
            "too many {name} query parameters: maximum is {maximum}"
        )));
    }
    if values.contains(&value) {
        return Err(invalid_input(format!(
            "duplicate repeated query parameter value: {name}"
        )));
    }
    values.push(value);
    Ok(())
}

fn bounded_optional(
    value: String,
    name: &'static str,
    maximum_chars: usize,
) -> Result<Option<String>, ApiError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > maximum_chars {
        return Err(invalid_input(format!(
            "{name} exceeds the maximum of {maximum_chars} Unicode characters"
        )));
    }
    Ok(Some(value.to_owned()))
}

fn bounded_label(value: String) -> Result<TaskReadLabel, ApiError> {
    if value.chars().count() > MAX_TASK_READ_LABEL_CHARS {
        return Err(invalid_input(format!(
            "label exceeds the maximum of {MAX_TASK_READ_LABEL_CHARS} Unicode characters"
        )));
    }
    TaskReadLabel::new(value)
        .ok_or_else(|| invalid_input("label must contain a non-whitespace character"))
}

fn parse_task_read_query<T: TaskReadQueryTarget>(raw_query: Option<&str>) -> Result<T, ApiError> {
    let mut query = T::default();
    let mut scalar_parameters = BTreeSet::new();
    let Some(raw_query) = raw_query else {
        return Ok(query);
    };
    if raw_query.is_empty() {
        return Ok(query);
    }
    if raw_query.len() > MAX_TASK_READ_QUERY_BYTES {
        return Err(invalid_input(format!(
            "task-read raw query exceeds {MAX_TASK_READ_QUERY_BYTES} bytes"
        )));
    }
    let encoded_pairs = raw_query.split('&').collect::<Vec<_>>();
    if encoded_pairs.len() > MAX_TASK_READ_QUERY_PAIRS {
        return Err(invalid_input(format!(
            "task-read query exceeds {MAX_TASK_READ_QUERY_PAIRS} parameter pairs"
        )));
    }

    for encoded_pair in encoded_pairs {
        let (encoded_key, encoded_value) =
            encoded_pair.split_once('=').unwrap_or((encoded_pair, ""));
        let key = decode_query_component(encoded_key)?;
        let value = decode_query_component(encoded_value)?;
        match key.as_str() {
            "status" => {
                let status = ApiTaskStatus::from_str(value.trim())
                    .map_err(|_| invalid_input(format!("unknown status filter: {value}")))?;
                push_repeated_distinct(query.statuses(), status, "status", MAX_TASK_READ_STATUSES)?;
            }
            "priority" => {
                let raw_priority = value.trim();
                let priority = raw_priority
                    .parse::<u8>()
                    .ok()
                    .and_then(ApiTaskPriority::new)
                    .ok_or_else(|| invalid_input("priority must be one of P0, P1, P2, P3"))?;
                push_repeated_distinct(
                    query.priorities(),
                    priority,
                    "priority",
                    MAX_TASK_READ_PRIORITIES,
                )?;
            }
            "label" => {
                let label = bounded_label(value)?;
                push_repeated_distinct(query.labels(), label, "label", MAX_TASK_READ_LABELS)?;
            }
            "plan_filter" => {
                let plan_filter = TaskReadPlanFilter::from_str(value.trim())
                    .map_err(|_| invalid_input(format!("unknown plan_filter {value}")))?;
                push_repeated_distinct(
                    query.plan_filters(),
                    plan_filter,
                    "plan_filter",
                    MAX_TASK_READ_PLAN_FILTERS,
                )?;
            }
            "assignee" => {
                scalar_parameter(&mut scalar_parameters, "assignee")?;
                query.set_assignee(bounded_optional(
                    value,
                    "assignee",
                    MAX_TASK_READ_ASSIGNEE_CHARS,
                )?);
            }
            "q" => {
                scalar_parameter(&mut scalar_parameters, "q")?;
                query.set_q(bounded_optional(value, "q", MAX_TASK_READ_Q_CHARS)?);
            }
            "include_archived" => {
                scalar_parameter(&mut scalar_parameters, "include_archived")?;
                query.set_include_archived(
                    value
                        .parse::<bool>()
                        .map_err(|_| invalid_input(format!("invalid include_archived: {value}")))?,
                );
            }
            "limit" => {
                scalar_parameter(&mut scalar_parameters, "limit")?;
                query.set_limit(
                    value
                        .parse::<usize>()
                        .map_err(|_| invalid_input(format!("invalid limit: {value}")))?,
                );
            }
            "offset" => {
                scalar_parameter(&mut scalar_parameters, "offset")?;
                query.set_offset(
                    value
                        .parse::<usize>()
                        .map_err(|_| invalid_input(format!("invalid offset: {value}")))?,
                );
            }
            "sort" => {
                scalar_parameter(&mut scalar_parameters, "sort")?;
                query.set_sort(
                    TaskReadSort::from_str(&value)
                        .map_err(|_| invalid_input(format!("unsupported sort: {value}")))?,
                );
            }
            _ => {
                return Err(invalid_input(format!(
                    "unknown task-read query parameter: {key}"
                )));
            }
        }
    }
    Ok(query)
}

fn task_plan_filter_from_contract(filter: TaskReadPlanFilter) -> TaskPlanFilter {
    match filter {
        TaskReadPlanFilter::PlanNeeded => TaskPlanFilter::PlanNeeded,
        TaskReadPlanFilter::HasSteps => TaskPlanFilter::HasSteps,
        TaskReadPlanFilter::IncompleteRequiredSteps => TaskPlanFilter::IncompleteRequiredSteps,
    }
}

fn task_sort_from_contract(sort: TaskReadSort) -> application_api::TaskListSort {
    match sort {
        TaskReadSort::Seq => application_api::TaskListSort::Seq,
        TaskReadSort::SeqDesc => application_api::TaskListSort::SeqDesc,
        TaskReadSort::Title => application_api::TaskListSort::Title,
        TaskReadSort::TitleDesc => application_api::TaskListSort::TitleDesc,
        TaskReadSort::Status => application_api::TaskListSort::Status,
        TaskReadSort::StatusDesc => application_api::TaskListSort::StatusDesc,
        TaskReadSort::Position => application_api::TaskListSort::Position,
        TaskReadSort::PositionDesc => application_api::TaskListSort::PositionDesc,
        TaskReadSort::Priority => application_api::TaskListSort::Priority,
        TaskReadSort::PriorityDesc => application_api::TaskListSort::PriorityDesc,
        TaskReadSort::Assignee => application_api::TaskListSort::Assignee,
        TaskReadSort::AssigneeDesc => application_api::TaskListSort::AssigneeDesc,
        TaskReadSort::ScheduledAt => application_api::TaskListSort::ScheduledAt,
        TaskReadSort::ScheduledAtDesc => application_api::TaskListSort::ScheduledAtDesc,
        TaskReadSort::DueAt => application_api::TaskListSort::DueAt,
        TaskReadSort::DueAtDesc => application_api::TaskListSort::DueAtDesc,
        TaskReadSort::CreatedAt => application_api::TaskListSort::CreatedAt,
        TaskReadSort::CreatedAtDesc => application_api::TaskListSort::CreatedAtDesc,
        TaskReadSort::UpdatedAt => application_api::TaskListSort::UpdatedAt,
        TaskReadSort::UpdatedAtDesc => application_api::TaskListSort::UpdatedAtDesc,
    }
}

fn task_get_includes_ontology(include: Option<&str>) -> bool {
    include.is_some_and(|include| {
        include
            .split(',')
            .map(str::trim)
            .any(|item| item == "ontology")
    })
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

fn label_contract_from<T, S>(value: S) -> Result<T, ApiError>
where
    T: DeserializeOwned,
    S: Serialize,
{
    let mut value = serde_json::to_value(value).map_err(|error| {
        ApiError(KanbanError::Storage(format!(
            "label contract serialization failed: {error}"
        )))
    })?;
    kanban_sqlite::api::naturalize_structured_metadata(&mut value)?;
    serde_json::from_value(value).map_err(|error| {
        ApiError(KanbanError::Storage(format!(
            "label contract conversion failed: {error}"
        )))
    })
}

pub(crate) async fn list_tasks(
    State(state): State<AppState>,
    ListTasksRequest { path, query }: ListTasksRequest,
) -> Result<Json<ListTasksResponse>, ApiError> {
    validate_page_bounds(query.limit, MAX_TASK_READ_LIMIT, query.offset)?;
    let statuses = query.status.into_iter().map(task_status_from_api).collect();
    let priorities = query
        .priority
        .into_iter()
        .map(|priority| i64::from(priority.get()))
        .collect();
    let plan_filters = query
        .plan_filter
        .into_iter()
        .map(task_plan_filter_from_contract)
        .collect();
    let application = state.application();
    let page = application_api::list_tasks_page(
        &application,
        &path.board,
        application_api::TaskListOptions {
            statuses,
            priorities,
            labels: query
                .label
                .into_iter()
                .map(TaskReadLabel::into_string)
                .collect(),
            plan_filters,
            include_archived: query.include_archived,
            assignee: query.assignee,
            search: query.q,
            sort: task_sort_from_contract(query.sort),
            limit: query.limit,
            offset: query.offset,
        },
    )?;
    let total = page.total;
    let tasks = page
        .tasks
        .into_iter()
        .map(api_task_from_record)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListTasksResponse {
        data: tasks,
        meta: kanban_contract::TotalPaginationMeta {
            limit: query.limit,
            offset: query.offset,
            total,
        },
    }))
}

pub(crate) async fn list_tasks_by_status(
    State(state): State<AppState>,
    ListTasksByStatusRequest { path, query }: ListTasksByStatusRequest,
) -> Result<Json<ListTasksByStatusResponse>, ApiError> {
    validate_page_bounds(query.limit, MAX_TASK_READ_LIMIT, query.offset)?;
    let statuses = query
        .status
        .into_iter()
        .map(task_status_from_api)
        .collect::<Vec<_>>();
    let priorities = query
        .priority
        .into_iter()
        .map(|priority| i64::from(priority.get()))
        .collect::<Vec<_>>();
    let plan_filters = query
        .plan_filter
        .into_iter()
        .map(task_plan_filter_from_contract)
        .collect::<Vec<_>>();
    let sort = task_sort_from_contract(query.sort);
    let application = state.application();
    let labels = query
        .label
        .into_iter()
        .map(TaskReadLabel::into_string)
        .collect::<Vec<_>>();
    let mut windows = Vec::with_capacity(statuses.len());
    for status in statuses {
        let page = application_api::list_tasks_page(
            &application,
            &path.board,
            application_api::TaskListOptions {
                statuses: vec![status],
                priorities: priorities.clone(),
                labels: labels.clone(),
                plan_filters: plan_filters.clone(),
                include_archived: query.include_archived,
                assignee: query.assignee.clone(),
                search: query.q.clone(),
                sort,
                limit: query.limit,
                offset: query.offset,
            },
        )?;
        windows.push(ListTasksStatusWindow {
            status: api_task_status_from_core(status),
            tasks: page
                .tasks
                .into_iter()
                .map(api_task_from_record)
                .collect::<Result<Vec<_>, _>>()?,
            page: kanban_contract::TotalPaginationMeta {
                limit: query.limit,
                offset: query.offset,
                total: page.total,
            },
        });
    }
    Ok(Json(ListTasksByStatusResponse {
        data: ListTasksByStatusData { statuses: windows },
        meta: kanban_contract::OffsetPaginationMeta {
            limit: query.limit,
            offset: query.offset,
        },
    }))
}

pub(crate) async fn create_task(
    State(state): State<AppState>,
    Path(path): Path<CreateTaskPath>,
    headers: HeaderMap,
    body: Result<Json<CreateTaskRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateTaskResponse>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let input = kanban_sqlite::api::CreateTask {
        title: body.title,
        description: body.description,
        status: body.status.map(|status| match status {
            ApiCreateTaskStatus::Triage => TaskStatus::Triage,
            ApiCreateTaskStatus::Todo => TaskStatus::Todo,
            ApiCreateTaskStatus::Scheduled => TaskStatus::Scheduled,
            ApiCreateTaskStatus::Ready => TaskStatus::Ready,
        }),
        assignee: body.assignee,
        priority: body.priority,
        scheduled_at: body.scheduled_at,
        due_at: body.due_at,
        max_retries: body.max_retries,
        metadata_json: metadata_json(
            body.metadata
                .map(|value| serde_json::Value::Object(value.into_iter().collect())),
        )?,
    };
    let task = kanban_sqlite::api::create_task_with_labels_and_dependencies(
        state.db_path(),
        &path.board,
        &actor,
        input,
        &body.labels,
        &body.depends_on,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(CreateTaskResponse {
            data: api_task_from_record(task)?,
        }),
    ))
}

pub(crate) async fn list_board_labels(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
) -> Result<Json<kanban_contract::ListBoardLabelsResponse>, ApiError> {
    Ok(Json(DataEnvelope::new(
        kanban_sqlite::api::list_labels(state.db_path(), &board)?
            .into_iter()
            .map(api_label_from_record)
            .collect(),
    )))
}

pub(crate) async fn create_board_label(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    body: Result<Json<kanban_contract::CreateBoardLabelRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<kanban_contract::CreateBoardLabelResponse>), ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let label = kanban_sqlite::api::create_label(
        state.db_path(),
        &board,
        kanban_sqlite::api::CreateLabel {
            name: body.name,
            color: body.color,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope::new(api_label_from_record(label))),
    ))
}

pub(crate) async fn list_label_semantics(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
) -> Result<Json<kanban_contract::ListLabelSemanticsResponse>, ApiError> {
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::list_label_semantics(state.db_path(), &board)?,
    ))?))
}

pub(crate) async fn get_label_semantics(
    State(state): State<AppState>,
    Path(kanban_contract::LabelSemanticsPath { board, label_id }): Path<
        kanban_contract::LabelSemanticsPath,
    >,
) -> Result<Json<kanban_contract::GetLabelSemanticsResponse>, ApiError> {
    let label_id = require_label_id_path(label_id)?;
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::get_label_semantics_by_id(state.db_path(), &board, &label_id)?,
    ))?))
}

pub(crate) async fn upsert_label_semantics(
    State(state): State<AppState>,
    Path(kanban_contract::LabelSemanticsPath { board, label_id }): Path<
        kanban_contract::LabelSemanticsPath,
    >,
    headers: HeaderMap,
    body: Result<Json<kanban_contract::UpsertLabelSemanticsRequest>, JsonRejection>,
) -> Result<Json<kanban_contract::UpsertLabelSemanticsResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let label_id = require_label_id_path(label_id)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let mut options = kanban_sqlite::api::LabelSemanticsMutationOptions::manual_actor(actor);
    options.reason = body.reason;
    options.source_signal_ids = body.source_signal_ids;
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::upsert_label_semantics_by_id_with_options(
            state.db_path(),
            &board,
            &label_id,
            kanban_sqlite::api::UpsertLabelSemantics {
                label_ref: label_id.clone(),
                expected_semantics_hash: body.expected_semantics_hash,
                replace: body.replace,
                description: body.description,
                applies_when: body.applies_when.unwrap_or_default(),
                excludes_when: body.excludes_when.unwrap_or_default(),
                positive_examples: body.positive_examples.unwrap_or_default(),
                negative_examples: body.negative_examples.unwrap_or_default(),
                remove_applies_when: body.remove_applies_when,
                remove_excludes_when: body.remove_excludes_when,
                remove_positive_examples: body.remove_positive_examples,
                remove_negative_examples: body.remove_negative_examples,
            },
            options,
        )?,
    ))?))
}

pub(crate) async fn delete_label_semantics(
    State(state): State<AppState>,
    Path(kanban_contract::LabelSemanticsPath { board, label_id }): Path<
        kanban_contract::LabelSemanticsPath,
    >,
    headers: HeaderMap,
    query: Result<Query<kanban_contract::DeleteLabelSemanticsQuery>, QueryRejection>,
) -> Result<Json<DeleteResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let label_id = require_label_id_path(label_id)?;
    let mut options = kanban_sqlite::api::LabelSemanticsMutationOptions::manual_actor(actor(
        None, &headers, &state,
    ));
    options.reason = Some(query.reason);
    kanban_sqlite::api::clear_label_semantics_by_id_with_options(
        state.db_path(),
        &board,
        &label_id,
        query.expected_semantics_hash,
        options,
    )?;
    Ok(Json(DataEnvelope::new(DeleteResult { deleted: true })))
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
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
) -> Result<Json<kanban_contract::ListLabelAtomsResponse>, ApiError> {
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::list_label_atoms(state.db_path(), &board)?,
    ))?))
}

pub(crate) async fn explain_label_atom(
    State(state): State<AppState>,
    Path(kanban_contract::LabelAtomPath { board, atom_ref }): Path<kanban_contract::LabelAtomPath>,
) -> Result<Json<kanban_contract::ExplainLabelAtomResponse>, ApiError> {
    Ok(Json(label_contract_from(DataEnvelope::new(
        LabelAtomExplainDto::try_from(kanban_sqlite::api::explain_label_atom(
            state.db_path(),
            &board,
            &atom_ref,
        )?)?,
    ))?))
}

pub(crate) async fn label_atom_index_status(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
) -> Result<Json<kanban_contract::LabelAtomIndexStatusResponse>, ApiError> {
    let result = label_atom_index_status_for_state(state, board).await?;
    Ok(Json(label_contract_from(DataEnvelope::new(result))?))
}

pub(crate) async fn rebuild_label_atom_index(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
) -> Result<Json<kanban_contract::RebuildLabelAtomIndexResponse>, ApiError> {
    let result = rebuild_label_atom_index_for_state(state, board).await?;
    Ok(Json(label_contract_from(DataEnvelope::new(result))?))
}

pub(crate) async fn query_label_atom_index(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    query: Result<Query<kanban_contract::LabelAtomIndexQuery>, QueryRejection>,
) -> Result<Json<kanban_contract::QueryLabelAtomIndexResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::api::MAX_TASK_LIST_LIMIT, 0)?;
    let text = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let vector_json = query
        .vector_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (text, vector_json) {
        (Some(_), Some(_)) => {
            return Err(invalid_input("q and vector_json are mutually exclusive"));
        }
        (None, None) => return Err(invalid_input("q or vector_json is required")),
        _ => {}
    }
    let polarity = query
        .polarity
        .as_deref()
        .map(parse_label_atom_polarity)
        .transpose()?;
    let result = query_label_atom_index_for_state(
        state,
        board,
        LabelAtomIndexHelperQuery {
            text: text.map(str::to_owned),
            vector_json: vector_json.map(str::to_owned),
            embedding_model: query.embedding_model,
            include_vector: query.include_vector,
            polarity,
            limit: query.limit,
        },
    )
    .await?;
    Ok(Json(label_contract_from(DataEnvelope::new(result))?))
}

pub(crate) async fn list_task_labels(
    State(state): State<AppState>,
    Path(path): Path<kanban_contract::ListTaskLabelsPath>,
) -> Result<Json<DataEnvelope<Vec<ApiLabel>>>, ApiError> {
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &path.task_id)?;
    Ok(Json(DataEnvelope::new(
        task.labels.into_iter().map(api_label_from_record).collect(),
    )))
}

pub(crate) async fn suggest_task_labels(
    State(state): State<AppState>,
    Path(kanban_contract::TaskLabelSurfacePath { task_id }): Path<
        kanban_contract::TaskLabelSurfacePath,
    >,
    query: Result<Query<kanban_contract::LabelSuggestionQuery>, QueryRejection>,
) -> Result<Json<kanban_contract::SuggestTaskLabelsResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let options = label_suggestion_options(query)?;
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
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
    Ok(Json(label_contract_from(DataEnvelope::new(result))?))
}

pub(crate) async fn add_task_label(
    State(state): State<AppState>,
    Path(path): Path<kanban_contract::AddTaskLabelPath>,
    headers: HeaderMap,
    body: Result<Json<kanban_contract::AddTaskLabelRequest>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<OptionalMetadataEnvelope<ApiTask, CreatedLabelsMeta<ApiLabel>>>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let label_names = body.label_names().map_err(invalid_input)?;
    let result = kanban_sqlite::api::add_task_labels_by_id_with_options(
        state.db_path(),
        &actor,
        &path.task_id,
        &label_names,
        body.create_missing,
    )?;
    let created_labels = result
        .created_labels
        .into_iter()
        .map(api_label_from_record)
        .collect::<Vec<_>>();
    let meta = if created_labels.is_empty() {
        None
    } else {
        Some(CreatedLabelsMeta { created_labels })
    };
    Ok((
        StatusCode::CREATED,
        Json(OptionalMetadataEnvelope::new(
            api_task_from_record(result.task)?,
            meta,
        )),
    ))
}

pub(crate) async fn bootstrap_task_label(
    State(state): State<AppState>,
    Path(kanban_contract::TaskLabelSurfacePath { task_id }): Path<
        kanban_contract::TaskLabelSurfacePath,
    >,
    headers: HeaderMap,
    body: Result<Json<kanban_contract::BootstrapTaskLabelRequest>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<kanban_contract::BootstrapTaskLabelResponse>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let result = kanban_sqlite::api::bootstrap_task_label_by_id(
        state.db_path(),
        &actor,
        &task_id,
        kanban_sqlite::api::BootstrapTaskLabel {
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
        Json(DataEnvelope::new(kanban_contract::BootstrapTaskLabelData {
            task: api_task_from_record(result.task)?,
            semantics: label_contract_from(result.semantics)?,
        })),
    ))
}

pub(crate) async fn propose_task_label(
    State(state): State<AppState>,
    Path(kanban_contract::TaskLabelSurfacePath { task_id }): Path<
        kanban_contract::TaskLabelSurfacePath,
    >,
    query: Result<Query<kanban_contract::LabelSuggestionQuery>, QueryRejection>,
    headers: HeaderMap,
    body: Result<Json<kanban_contract::ProposeTaskLabelRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<kanban_contract::ProposeTaskLabelResponse>), ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let options = label_suggestion_options(query)?;
    let body = match body {
        Ok(Json(body)) => body,
        Err(JsonRejection::MissingJsonContentType(_)) => kanban_contract::ProposeTaskLabelRequest {
            proposal: None,
            actor: None,
            source_signal_ids: Vec::new(),
            ontology_actor: None,
            allow_retarget: false,
            retarget_reason: None,
        },
        Err(error) => return Err(extractor_error(error)),
    };
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let create_options = kanban_sqlite::api::LabelProposalCreateOptions {
        source_signal_ids: body.source_signal_ids,
        ontology_actor: body.ontology_actor.map(label_ontology_actor_input),
        allow_retarget: body.allow_retarget,
        retarget_reason: body.retarget_reason,
    };
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    let label_state = state.clone();
    let label_board = task.board_slug;
    let label_task_id = task.id;
    let label_actor = actor;
    let label_candidate = body.proposal.map(label_contract_from).transpose()?;
    let attempt = tokio::task::spawn_blocking(move || {
        propose_task_label_for_state(
            &label_state,
            &label_board,
            &label_actor,
            &label_task_id,
            label_candidate,
            options,
            create_options,
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
        Json(label_contract_from(DataEnvelope::new(attempt))?),
    ))
}

fn label_suggestion_options(
    query: kanban_contract::LabelSuggestionQuery,
) -> Result<kanban_sqlite::api::LabelSuggestionOptions, ApiError> {
    validate_label_suggestion_bound("limit", query.limit)?;
    validate_label_suggestion_bound("candidate_limit", query.candidate_limit)?;
    validate_label_suggestion_bound("atom_limit", query.atom_limit)?;
    validate_label_suggestion_bound("max_selected_labels", query.max_selected_labels)?;
    Ok(kanban_sqlite::api::LabelSuggestionOptions {
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
    validate_page_bounds(value, kanban_sqlite::api::MAX_TASK_LIST_LIMIT, 0)
}

pub(crate) async fn list_task_label_proposals(
    State(state): State<AppState>,
    Path(kanban_contract::TaskLabelSurfacePath { task_id }): Path<
        kanban_contract::TaskLabelSurfacePath,
    >,
) -> Result<Json<kanban_contract::ListTaskLabelProposalsResponse>, ApiError> {
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    let proposals = kanban_sqlite::api::list_label_proposals(
        state.db_path(),
        &task.board_slug,
        kanban_sqlite::api::LabelProposalListOptions {
            task_ref: Some(task.id),
            status: None,
        },
    )?;
    Ok(Json(label_contract_from(DataEnvelope::new(proposals))?))
}

pub(crate) async fn record_label_ontology_observation(
    State(state): State<AppState>,
    Path(kanban_contract::TaskLabelSurfacePath { task_id }): Path<
        kanban_contract::TaskLabelSurfacePath,
    >,
    body: Result<Json<kanban_contract::RecordLabelOntologyObservationRequest>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<kanban_contract::RecordLabelOntologyObservationResponse>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    let observation = kanban_sqlite::api::record_label_ontology_observation(
        state.db_path(),
        &task.board_slug,
        &task.id,
        label_ontology_observation_input(body)?,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(label_contract_from(DataEnvelope::new(observation))?),
    ))
}

pub(crate) async fn list_signals(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<kanban_contract::ListSignalsResponse>, ApiError> {
    let query = parse_signal_query(raw_query.as_deref())?;
    validate_page_bounds(query.limit, kanban_sqlite::api::MAX_TASK_LIST_LIMIT, 0)?;
    let signals = kanban_sqlite::api::list_signals(
        state.db_path(),
        &board,
        kanban_sqlite::api::SignalListOptions {
            statuses: parse_signal_status_filters(raw_query.as_deref())?,
            kinds: parse_string_filters(raw_query.as_deref(), "kind")?,
            task_ref: query.task_ref,
            include_all: query.include_all,
            limit: query.limit,
        },
    )?;
    Ok(Json(label_contract_from(MetadataEnvelope::new(
        signals,
        SignalFilterMeta {
            include_all: query.include_all,
            limit: query.limit,
        },
    ))?))
}

pub(crate) async fn review_signals(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<kanban_contract::ReviewSignalsResponse>, ApiError> {
    let query = parse_signal_query(raw_query.as_deref())?;
    validate_page_bounds(query.limit, kanban_sqlite::api::MAX_TASK_LIST_LIMIT, 0)?;
    let signals = kanban_sqlite::api::review_signals(
        state.db_path(),
        &board,
        kanban_sqlite::api::SignalListOptions {
            statuses: parse_signal_status_filters(raw_query.as_deref())?,
            kinds: parse_string_filters(raw_query.as_deref(), "kind")?,
            task_ref: query.task_ref,
            include_all: query.include_all,
            limit: query.limit,
        },
    )?;
    Ok(Json(label_contract_from(MetadataEnvelope::new(
        signals,
        SignalFilterMeta {
            include_all: query.include_all,
            limit: query.limit,
        },
    ))?))
}

pub(crate) async fn get_signal(
    State(state): State<AppState>,
    Path(kanban_contract::SignalPath { signal_id }): Path<kanban_contract::SignalPath>,
) -> Result<Json<kanban_contract::GetSignalResponse>, ApiError> {
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::get_signal_by_id(state.db_path(), &signal_id)?,
    ))?))
}

pub(crate) async fn list_label_ontology_signals(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    RawQuery(raw_query): RawQuery,
) -> Result<Json<kanban_contract::LabelOntologySignalsResponse>, ApiError> {
    let query = parse_label_ontology_signal_query(raw_query.as_deref())?;
    validate_page_bounds(query.limit, kanban_sqlite::api::MAX_TASK_LIST_LIMIT, 0)?;
    let signals = kanban_sqlite::api::list_label_ontology_signals(
        state.db_path(),
        &board,
        kanban_sqlite::api::LabelOntologySignalListOptions {
            statuses: parse_label_ontology_status_filters(raw_query.as_deref())?,
            kinds: parse_label_ontology_kind_filters(raw_query.as_deref())?,
            task_ref: query.task_ref,
            target_label_ref: query.target_label_ref,
            proposed_label_name: query.proposed_label_name,
            include_all: query.include_all,
            limit: query.limit,
        },
    )?;
    let data = signals
        .into_iter()
        .map(label_contract_from)
        .collect::<Result<Vec<kanban_contract::LabelOntologySignalWire>, ApiError>>()?;
    Ok(Json(kanban_contract::LabelOntologySignalsResponse {
        data,
        meta: kanban_contract::LimitMeta { limit: query.limit },
    }))
}

pub(crate) async fn review_label_ontology(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    query: Result<Query<kanban_contract::LabelOntologyReviewQuery>, QueryRejection>,
) -> Result<Json<kanban_contract::ReviewLabelOntologyResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    validate_page_bounds(query.limit, kanban_sqlite::api::MAX_TASK_LIST_LIMIT, 0)?;
    let group_by = match query.group_by {
        kanban_contract::LabelOntologyReviewGroupByWire::Label => {
            kanban_sqlite::api::LabelOntologyReviewGroupBy::Label
        }
        kanban_contract::LabelOntologyReviewGroupByWire::CandidateAtom => {
            kanban_sqlite::api::LabelOntologyReviewGroupBy::CandidateAtom
        }
        kanban_contract::LabelOntologyReviewGroupByWire::ProposedLabel => {
            kanban_sqlite::api::LabelOntologyReviewGroupBy::ProposedLabel
        }
        kanban_contract::LabelOntologyReviewGroupByWire::Cluster => {
            kanban_sqlite::api::LabelOntologyReviewGroupBy::Cluster
        }
    };
    let groups = kanban_sqlite::api::review_label_ontology(
        state.db_path(),
        &board,
        kanban_sqlite::api::LabelOntologyReviewOptions {
            group_by,
            include_all: query.include_all,
            limit: query.limit,
        },
    )?;
    Ok(Json(label_contract_from(MetadataEnvelope::new(
        groups,
        LabelOntologyReviewMeta {
            group_by: group_by.to_string(),
            include_all: query.include_all,
            limit: query.limit,
        },
    ))?))
}

pub(crate) async fn get_label_ontology_signal(
    State(state): State<AppState>,
    Path(kanban_contract::SignalPath { signal_id }): Path<kanban_contract::SignalPath>,
) -> Result<Json<kanban_contract::GetLabelOntologySignalResponse>, ApiError> {
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::get_label_ontology_signal(state.db_path(), &signal_id)?,
    ))?))
}

pub(crate) async fn create_label_ontology_action(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    body: Result<Json<kanban_contract::LabelOntologyActionRequest>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<kanban_contract::LabelOntologyActionResponse>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let action = kanban_sqlite::api::create_label_ontology_action(
        state.db_path(),
        &board,
        label_ontology_action_input(body)?,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(label_contract_from(DataEnvelope::new(action))?),
    ))
}

pub(crate) async fn apply_label_ontology_atom(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    body: Result<Json<kanban_contract::ApplyLabelOntologyAtomRequest>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<kanban_contract::LabelOntologyActionResponse>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let allow_retarget = body.allow_retarget;
    let retarget_reason = body.retarget_reason.clone();
    let action = kanban_sqlite::api::apply_label_ontology_atom_with_options(
        state.db_path(),
        &board,
        label_ontology_atom_apply_input(body),
        kanban_sqlite::api::LabelOntologyRetargetOptions {
            allow_retarget,
            retarget_reason,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(label_contract_from(DataEnvelope::new(action))?),
    ))
}

pub(crate) async fn revert_label_ontology_mutation(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    body: Result<Json<kanban_contract::RevertLabelOntologyMutationRequest>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<kanban_contract::LabelOntologyActionResponse>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let action = kanban_sqlite::api::revert_label_ontology_mutation(
        state.db_path(),
        &board,
        label_ontology_revert_input(body),
    )?;
    Ok((
        StatusCode::CREATED,
        Json(label_contract_from(DataEnvelope::new(action))?),
    ))
}

pub(crate) async fn validate_label_ontology_action(
    State(state): State<AppState>,
    Path(kanban_contract::BoardLabelPath { board }): Path<kanban_contract::BoardLabelPath>,
    body: Result<Json<kanban_contract::ValidateLabelOntologyActionRequest>, JsonRejection>,
) -> Result<
    (
        StatusCode,
        Json<kanban_contract::LabelOntologyActionResponse>,
    ),
    ApiError,
> {
    let Json(body) = body.map_err(extractor_error)?;
    let action = kanban_sqlite::api::validate_label_ontology_action(
        state.db_path(),
        &board,
        label_ontology_validation_input(body)?,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(label_contract_from(DataEnvelope::new(action))?),
    ))
}

pub(crate) async fn get_label_proposal(
    State(state): State<AppState>,
    Path(kanban_contract::ProposalPath { proposal_id }): Path<kanban_contract::ProposalPath>,
) -> Result<Json<kanban_contract::GetLabelProposalResponse>, ApiError> {
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::get_label_proposal(state.db_path(), &proposal_id)?,
    ))?))
}

pub(crate) async fn accept_label_proposal(
    State(state): State<AppState>,
    Path(kanban_contract::ProposalPath { proposal_id }): Path<kanban_contract::ProposalPath>,
    headers: HeaderMap,
    body: Result<Json<kanban_contract::LabelProposalDecisionRequest>, JsonRejection>,
) -> Result<Json<kanban_contract::LabelProposalDecisionResponse>, ApiError> {
    let body = optional_decision_body(body)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let ontology_actor = body.ontology_actor.map(label_ontology_actor_input);
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::accept_label_proposal_with_options(
            state.db_path(),
            &actor,
            &proposal_id,
            body.reason,
            kanban_sqlite::api::LabelProposalDecisionOptions {
                source_signal_ids: body.source_signal_ids,
                ontology_actor,
                allow_retarget: body.allow_retarget,
                retarget_reason: body.retarget_reason,
            },
        )?,
    ))?))
}

pub(crate) async fn reject_label_proposal(
    State(state): State<AppState>,
    Path(kanban_contract::ProposalPath { proposal_id }): Path<kanban_contract::ProposalPath>,
    headers: HeaderMap,
    body: Result<Json<kanban_contract::LabelProposalDecisionRequest>, JsonRejection>,
) -> Result<Json<kanban_contract::LabelProposalDecisionResponse>, ApiError> {
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
    if body.allow_retarget || body.retarget_reason.is_some() {
        return Err(invalid_input(
            "retarget options are only supported when accepting label proposals",
        ));
    }
    let actor = actor(body.actor.as_deref(), &headers, &state);
    Ok(Json(label_contract_from(DataEnvelope::new(
        kanban_sqlite::api::reject_label_proposal(
            state.db_path(),
            &actor,
            &proposal_id,
            body.reason,
        )?,
    ))?))
}

fn optional_decision_body(
    body: Result<Json<kanban_contract::LabelProposalDecisionRequest>, JsonRejection>,
) -> Result<kanban_contract::LabelProposalDecisionRequest, ApiError> {
    match body {
        Ok(Json(body)) => Ok(body),
        Err(JsonRejection::MissingJsonContentType(_)) => {
            Ok(kanban_contract::LabelProposalDecisionRequest {
                reason: None,
                actor: None,
                source_signal_ids: Vec::new(),
                ontology_actor: None,
                allow_retarget: false,
                retarget_reason: None,
            })
        }
        Err(error) => Err(extractor_error(error)),
    }
}

fn label_ontology_observation_input(
    body: kanban_contract::RecordLabelOntologyObservationRequest,
) -> Result<kanban_sqlite::api::LabelOntologyRecordInput, ApiError> {
    let (agent_candidates_json, _) = json_body_field(
        "agent_candidates",
        body.agent_candidates,
        JsonBodyShape::Array,
        empty_json_array(),
    )?;
    let (suggestion_snapshot_json, suggestion_snapshot) = json_body_field(
        "suggestion_snapshot",
        body.suggestion_snapshot,
        JsonBodyShape::Object,
        empty_json_object(),
    )?;
    let (final_decision_json, _) = json_body_field(
        "final_decision",
        body.final_decision,
        JsonBodyShape::Object,
        empty_json_object(),
    )?;
    let diagnostics_json = derive_diagnostics_json(body.diagnostics, &suggestion_snapshot)?;
    Ok(kanban_sqlite::api::LabelOntologyRecordInput {
        actor: kanban_sqlite::api::LabelOntologyActor {
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
    body: kanban_contract::LabelOntologySignalRequest,
) -> Result<kanban_sqlite::api::LabelOntologySignalInput, ApiError> {
    let (related_labels_json, _) = json_body_field(
        "related_labels",
        body.related_labels,
        JsonBodyShape::Array,
        empty_json_array(),
    )?;
    let (proposal_json, _) = json_body_field(
        "proposal",
        body.proposal,
        JsonBodyShape::Object,
        empty_json_object(),
    )?;
    Ok(kanban_sqlite::api::LabelOntologySignalInput {
        kind: label_contract_from(body.kind)?,
        target_label_ref: body.target_label_ref,
        related_labels_json,
        proposed_action: label_contract_from(body.proposed_action)?,
        candidate_atom: body.candidate_atom.map(|candidate| {
            kanban_sqlite::api::LabelOntologyCandidateAtomInput {
                polarity: candidate.polarity,
                kind: candidate.kind,
                text: candidate.text,
            }
        }),
        proposed_label_name: body.proposed_label_name,
        proposal_json,
        agent_selected: body.agent_selected,
        suggest_state: body.suggest_state.map(label_contract_from).transpose()?,
        suggest_score: body.suggest_score,
        suggest_rank: body.suggest_rank,
        final_selected: body.final_selected,
        rationale: body.rationale,
        confidence: body.confidence,
        signal_key: body.signal_key,
    })
}

fn label_ontology_actor_input(
    body: kanban_contract::LabelOntologyActorWire,
) -> kanban_sqlite::api::LabelOntologyActor {
    kanban_sqlite::api::LabelOntologyActor {
        name: body.name,
        actor_type: body.actor_type,
        agent_type: body.agent_type,
    }
}

fn label_ontology_action_input(
    body: kanban_contract::LabelOntologyActionRequest,
) -> Result<kanban_sqlite::api::LabelOntologyActionInput, ApiError> {
    Ok(kanban_sqlite::api::LabelOntologyActionInput {
        actor: label_ontology_actor_input(body.actor),
        action_type: label_contract_from(body.action_type)?,
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
        change_json: optional_json_body_field("change", body.change, JsonBodyShape::Object)?,
        validation_status: body
            .validation_status
            .map(label_contract_from)
            .transpose()?,
        validation_json: optional_json_body_field(
            "validation",
            body.validation,
            JsonBodyShape::Object,
        )?,
    })
}

fn label_ontology_atom_apply_input(
    body: kanban_contract::ApplyLabelOntologyAtomRequest,
) -> kanban_sqlite::api::LabelOntologyAtomApplyInput {
    kanban_sqlite::api::LabelOntologyAtomApplyInput {
        actor: label_ontology_actor_input(body.actor),
        signal_ids: body.signal_ids,
        label_ref: body.label_ref,
        kind: body.kind,
        text: body.text,
        reason: body.reason,
    }
}

fn label_ontology_revert_input(
    body: kanban_contract::RevertLabelOntologyMutationRequest,
) -> kanban_sqlite::api::LabelOntologyRevertInput {
    kanban_sqlite::api::LabelOntologyRevertInput {
        actor: label_ontology_actor_input(body.actor),
        target_action_id: body.target_action_id,
        expected_current_hash: body.expected_current_hash,
        reason: body.reason,
    }
}

fn label_ontology_validation_input(
    body: kanban_contract::ValidateLabelOntologyActionRequest,
) -> Result<kanban_sqlite::api::LabelOntologyValidationInput, ApiError> {
    Ok(kanban_sqlite::api::LabelOntologyValidationInput {
        actor: label_ontology_actor_input(body.actor),
        parent_action_id: body.parent_action_id,
        signal_ids: body.signal_ids,
        reason: body.reason,
        validation_status: label_contract_from(body.validation_status)?,
        validation_json: required_json_body_field(
            "validation",
            body.validation,
            JsonBodyShape::Object,
        )?,
    })
}

fn json_body_field(
    new_name: &str,
    new_value: kanban_contract::JsonBodyFieldWire,
    shape: JsonBodyShape,
    default_value: JsonValue,
) -> Result<(String, JsonValue), ApiError> {
    let value = optional_json_body_value(new_name, new_value, shape)?.unwrap_or(default_value);
    let text = json_body_to_string(&value)?;
    Ok((text, value))
}

fn optional_json_body_field(
    new_name: &str,
    new_value: kanban_contract::JsonBodyFieldWire,
    shape: JsonBodyShape,
) -> Result<Option<String>, ApiError> {
    optional_json_body_value(new_name, new_value, shape)?
        .map(|value| json_body_to_string(&value))
        .transpose()
}

fn required_json_body_field(
    new_name: &str,
    new_value: kanban_contract::JsonBodyFieldWire,
    shape: JsonBodyShape,
) -> Result<String, ApiError> {
    optional_json_body_field(new_name, new_value, shape)?
        .ok_or_else(|| invalid_input(format!("{new_name} is required")))
}

fn optional_json_body_value(
    new_name: &str,
    new_value: kanban_contract::JsonBodyFieldWire,
    shape: JsonBodyShape,
) -> Result<Option<JsonValue>, ApiError> {
    if let kanban_contract::JsonBodyFieldWire::Present(value) = new_value {
        return ensure_json_body_shape(value, new_name, shape).map(Some);
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
    diagnostics: kanban_contract::JsonBodyFieldWire,
    snapshot: &JsonValue,
) -> Result<String, ApiError> {
    let supplied = optional_json_body_value("diagnostics", diagnostics, JsonBodyShape::Array)?;
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

fn parse_string_filters(
    raw_query: Option<&str>,
    filter_name: &str,
) -> Result<Vec<String>, ApiError> {
    let Some(raw_query) = raw_query else {
        return Ok(Vec::new());
    };
    let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(raw_query)
        .map_err(|error| invalid_input(error.to_string()))?;
    Ok(pairs
        .into_iter()
        .filter_map(|(key, value)| (key == filter_name).then_some(value))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect())
}

fn query_without_repeated_filters(raw_query: Option<&str>) -> Result<String, ApiError> {
    let Some(raw_query) = raw_query else {
        return Ok(String::new());
    };
    let pairs = serde_urlencoded::from_str::<Vec<(String, String)>>(raw_query)
        .map_err(|error| invalid_input(error.to_string()))?;
    serde_urlencoded::to_string(
        pairs
            .into_iter()
            .filter(|(key, _)| key != "status" && key != "kind")
            .collect::<Vec<_>>(),
    )
    .map_err(|error| invalid_input(error.to_string()))
}

fn parse_signal_query(raw_query: Option<&str>) -> Result<kanban_contract::SignalQuery, ApiError> {
    let sanitized = query_without_repeated_filters(raw_query)?;
    let mut query = serde_urlencoded::from_str::<kanban_contract::SignalQuery>(&sanitized)
        .map_err(|error| invalid_input(error.to_string()))?;
    query.status = parse_string_filters(raw_query, "status")?;
    query.kind = parse_string_filters(raw_query, "kind")?;
    Ok(query)
}

fn parse_label_ontology_signal_query(
    raw_query: Option<&str>,
) -> Result<kanban_contract::LabelOntologySignalQuery, ApiError> {
    let sanitized = query_without_repeated_filters(raw_query)?;
    let mut query =
        serde_urlencoded::from_str::<kanban_contract::LabelOntologySignalQuery>(&sanitized)
            .map_err(|error| invalid_input(error.to_string()))?;
    query.status = parse_string_filters(raw_query, "status")?;
    query.kind = parse_string_filters(raw_query, "kind")?;
    Ok(query)
}

fn parse_label_ontology_status_filters(
    raw_query: Option<&str>,
) -> Result<Vec<kanban_sqlite::api::LabelOntologySignalStatus>, ApiError> {
    parse_label_ontology_filters(raw_query, "status")
}

fn parse_signal_status_filters(
    raw_query: Option<&str>,
) -> Result<Vec<kanban_sqlite::api::SignalStatus>, ApiError> {
    parse_label_ontology_filters(raw_query, "status")
}

fn parse_label_ontology_kind_filters(
    raw_query: Option<&str>,
) -> Result<Vec<kanban_sqlite::api::LabelOntologySignalKind>, ApiError> {
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
    options: kanban_sqlite::api::LabelSuggestionOptions,
) -> Result<kanban_sqlite::api::LabelSuggestionResult, ApiError> {
    let store = subprocess_vector_store_for_state(state, board)?;
    kanban_sqlite::api::provider::suggest_task_labels_with(
        state.db_path(),
        board,
        task_id,
        &store,
        options,
    )
    .map_err(ApiError::from)
}

fn subprocess_vector_store_for_state(
    state: &AppState,
    board: &str,
) -> Result<kanban_vector::SubprocessVectorStore, ApiError> {
    let store = kanban_vector::SubprocessVectorStore::new(
        resolve_helper(state, HelperKind::Vector),
        state.db_path().to_path_buf(),
        board.to_owned(),
        state.vector_config_path().map(std::path::Path::to_path_buf),
    );
    let Some(config) = kanban_local::resolved_vector_config(state.vector_config_path())
        .map_err(|error| invalid_input(error.to_string()))?
    else {
        return Ok(store);
    };
    Ok(store.with_embedding_model(config.model))
}

fn propose_task_label_for_state(
    state: &AppState,
    board: &str,
    actor: &str,
    task_id: &str,
    candidate: Option<kanban_sqlite::api::LabelProposalCandidate>,
    options: kanban_sqlite::api::LabelSuggestionOptions,
    create_options: kanban_sqlite::api::LabelProposalCreateOptions,
) -> Result<kanban_sqlite::api::LabelProposalAttempt, ApiError> {
    match candidate {
        Some(candidate) => {
            let provider =
                kanban_sqlite::api::provider::ManualLabelProposalProvider::new(candidate);
            let store = subprocess_vector_store_for_state(state, board)?;
            kanban_sqlite::api::provider::propose_task_label_with_store_and_create_options(
                state.db_path(),
                board,
                actor,
                task_id,
                &provider,
                &store,
                kanban_sqlite::api::LabelProposalProposeOptions {
                    suggestion: options,
                    create: create_options,
                },
            )
        }
        None => {
            let store = subprocess_vector_store_for_state(state, board)?;
            kanban_sqlite::api::provider::propose_task_label_with_store_and_create_options(
                state.db_path(),
                board,
                actor,
                task_id,
                &kanban_sqlite::api::provider::DisabledLabelProposalProvider,
                &store,
                kanban_sqlite::api::LabelProposalProposeOptions {
                    suggestion: options,
                    create: create_options,
                },
            )
        }
    }
    .map_err(ApiError::from)
}

async fn label_atom_index_status_for_state(
    state: AppState,
    board: String,
) -> Result<kanban_vector::VectorStoreStatus, ApiError> {
    let args =
        super::vector::vector_helper_args(&state, &board, &["label-atoms-status".to_owned()]);
    match run_helper_json::<kanban_contract::VectorHelperStatusResponse>(
        state,
        HelperKind::Vector,
        args,
    )
    .await
    {
        Ok(status) => Ok(super::vector::vector_store_status_from_helper(status)),
        Err(error) if error.is_status_degraded() => {
            Ok(super::vector::vector_store_status_from_helper(
                super::vector::degraded_vector_status(&error),
            ))
        }
        Err(error) => Err(error.into()),
    }
}

async fn rebuild_label_atom_index_for_state(
    state: AppState,
    board: String,
) -> Result<kanban_vector::VectorStoreStatus, ApiError> {
    let args =
        super::vector::vector_helper_args(&state, &board, &["rebuild-label-atoms".to_owned()]);
    match run_helper_json::<kanban_contract::VectorHelperStatusResponse>(
        state,
        HelperKind::Vector,
        args,
    )
    .await
    {
        Ok(status) => Ok(super::vector::vector_store_status_from_helper(status)),
        Err(error) if error.is_helper_missing() => Err(invalid_input(helper_degraded_message(
            HelperKind::Vector,
            &error,
        ))),
        Err(error) => Err(error.into()),
    }
}

struct LabelAtomIndexHelperQuery {
    text: Option<String>,
    vector_json: Option<String>,
    embedding_model: Option<String>,
    include_vector: bool,
    polarity: Option<String>,
    limit: usize,
}

async fn query_label_atom_index_for_state(
    state: AppState,
    board: String,
    query: LabelAtomIndexHelperQuery,
) -> Result<JsonValue, ApiError> {
    let mut command_args = vec!["query-label-atoms".to_owned()];
    if let Some(text) = query.text {
        command_args.push("--text".to_owned());
        command_args.push(text);
    } else if let Some(vector_json) = query.vector_json {
        command_args.push("--vector-json".to_owned());
        command_args.push(vector_json);
    }
    command_args.push("--limit".to_owned());
    command_args.push(query.limit.to_string());
    if let Some(embedding_model) = query.embedding_model {
        command_args.push("--embedding-model".to_owned());
        command_args.push(embedding_model);
    }
    if let Some(polarity) = query.polarity {
        command_args.push("--polarity".to_owned());
        command_args.push(polarity);
    }
    if query.include_vector {
        command_args.push("--include-vector".to_owned());
    }
    let args = super::vector::vector_helper_args(&state, &board, &command_args);
    match run_helper_json::<kanban_contract::VectorHelperQueryLabelAtomsResponse>(
        state,
        HelperKind::Vector,
        args,
    )
    .await
    {
        Ok(hits) => serde_json::to_value(hits).map_err(|error| {
            ApiError(kanban_core::KanbanError::Storage(format!(
                "failed to encode vector helper response: {error}"
            )))
        }),
        Err(error) if error.is_helper_missing() => Err(invalid_input(helper_degraded_message(
            HelperKind::Vector,
            &error,
        ))),
        Err(error) => Err(error.into()),
    }
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
    Path(path): Path<kanban_contract::RemoveTaskLabelPath>,
    headers: HeaderMap,
) -> Result<Json<DataEnvelope<ApiTask>>, ApiError> {
    let actor = actor(None, &headers, &state);
    let task = kanban_sqlite::api::remove_task_label_by_id(
        state.db_path(),
        &actor,
        &path.task_id,
        &path.label_id,
    )?;
    Ok(Json(DataEnvelope::new(api_task_from_record(task)?)))
}

pub(crate) async fn get_task(
    State(state): State<AppState>,
    Path(GetTaskPath { task_id }): Path<GetTaskPath>,
    query: Result<Query<GetTaskQuery>, QueryRejection>,
) -> Result<Json<GetTaskResponse>, ApiError> {
    let Query(query) = query.map_err(extractor_error)?;
    let include_ontology = task_get_includes_ontology(query.include.as_deref());
    let task = kanban_sqlite::api::get_task_by_id_global(state.db_path(), &task_id)?;
    let meta = if include_ontology {
        let ontology_summary = label_contract_from(
            kanban_sqlite::api::task_ontology_summary_by_id_global(state.db_path(), &task_id)?,
        )?;
        Some(TaskOntologyDetailsMeta {
            details: TaskOntologyDetails { ontology_summary },
        })
    } else {
        None
    };
    Ok(Json(OptionalMetadataEnvelope::new(
        api_task_from_record(task)?,
        meta,
    )))
}

pub(crate) async fn update_task(
    State(state): State<AppState>,
    Path(UpdateTaskPath { task_id }): Path<UpdateTaskPath>,
    headers: HeaderMap,
    body: Result<Json<UpdateTaskRequest>, JsonRejection>,
) -> Result<Json<UpdateTaskResponse>, ApiError> {
    let Json(body) = body.map_err(extractor_error)?;
    let actor = actor(body.actor.as_deref(), &headers, &state);
    let metadata_json = body
        .metadata
        .map(|value| value.map_or_else(|| "null".to_owned(), |value| value.to_string()));
    let patch = kanban_sqlite::api::TaskPatch {
        title: body.title,
        description: body.description,
        assignee: body.assignee,
        priority: body.priority,
        scheduled_at: body.scheduled_at,
        due_at: body.due_at,
        max_retries: body.max_retries,
        metadata_json,
        expected_lock_version: body.expected_lock_version,
    };
    let task = kanban_sqlite::api::update_task_by_id(state.db_path(), &actor, &task_id, patch)?;
    Ok(Json(DataEnvelope::new(api_task_from_record(task)?)))
}
