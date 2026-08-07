//! label semantics/ontology HTTP surface。

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post},
};
use kanban_service::KanbanError;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use kanban_protocol::{
    DataEnvelope, ListBoardLabelProposalsPath, ListBoardLabelProposalsQuery,
    ListBoardLabelProposalsResponse,
    cli_labels::{CliLabelOntologyQuality, CliLabelOntologyQualityOutput},
};

use crate::{error::ApiError, state::AppState};

/// signal 和 proposal 的 ID 全局唯一；这些读取会先解析 canonical record，
/// 因此不会假定它们属于 default board。
const GLOBAL_RECORD_SCOPE: &str = "__global_record__";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelOntologyQualityRecordTransport {
    board_id: String,
    denominator_json: String,
    disagreement_json: String,
    rates_json: String,
    precision_recall_json: String,
    warnings_json: String,
}

fn quality_component<T: DeserializeOwned>(field: &str, raw: &str) -> Result<T, ApiError> {
    serde_json::from_str(raw).map_err(|error| {
        ApiError(KanbanError::Storage(format!(
            "label ontology quality {field} 无法解码：{error}"
        )))
    })
}

fn quality_output(value: Value) -> Result<CliLabelOntologyQualityOutput, ApiError> {
    let raw: LabelOntologyQualityRecordTransport =
        serde_json::from_value(value).map_err(|error| {
            ApiError(KanbanError::Storage(format!(
                "label ontology quality record 无法解码：{error}"
            )))
        })?;
    Ok(DataEnvelope::new(CliLabelOntologyQuality {
        board_id: raw.board_id,
        denominator: quality_component("denominator_json", &raw.denominator_json)?,
        disagreement: quality_component("disagreement_json", &raw.disagreement_json)?,
        rates: quality_component("rates_json", &raw.rates_json)?,
        precision_recall: quality_component("precision_recall_json", &raw.precision_recall_json)?,
        warnings: quality_component("warnings_json", &raw.warnings_json)?,
    }))
}

fn board_path(board: String) -> String {
    board
}

fn body_or_empty(body: Option<Json<Value>>) -> Value {
    body.map(|Json(value)| value).unwrap_or_else(|| json!({}))
}

fn actor_object(value: &mut Value) {
    if let Some(actor) = value.get_mut("actor").and_then(Value::as_object_mut)
        && actor.get("actor_type").is_none()
        && let Some(actor_type) = actor.remove("type")
    {
        actor.insert("actor_type".to_owned(), actor_type);
    }
    let ontology_actor = value
        .get_mut("ontology_actor")
        .and_then(Value::as_object_mut)
        .map(|actor| {
            if actor.get("actor_type").is_none()
                && let Some(actor_type) = actor.remove("type")
            {
                actor.insert("actor_type".to_owned(), actor_type);
            }
            actor.clone()
        });
    if value.get("actor").is_none()
        && let Some(actor) = ontology_actor
    {
        value["actor"] = Value::Object(actor);
    }
}

fn normalize_proposal_body(value: &mut Value) {
    actor_object(value);
    if let Some(proposal) = value.get("proposal").and_then(Value::as_object).cloned() {
        for (key, item) in proposal {
            value[key] = item;
        }
    }
    if value.get("actor").and_then(Value::as_str).is_some() {
        let actor = value.get("actor").and_then(Value::as_str).unwrap_or("user");
        value["actor"] = Value::String(actor.to_owned());
    } else if let Some(actor) = value.get("actor").and_then(Value::as_object) {
        let name = actor
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("user")
            .to_owned();
        value["actor"] = Value::String(name);
    }
}

fn normalize_observation_body(value: &mut Value) {
    actor_object(value);
    let aliases = [
        ("agent_candidates", "agent_candidates_json"),
        ("suggestion_snapshot", "suggestion_snapshot_json"),
        ("final_decision", "final_decision_json"),
        ("diagnostics", "diagnostics_json"),
    ];
    for (from, to) in aliases {
        if let Some(item) = value.get(from).cloned() {
            value[to] = Value::String(item.to_string());
        }
    }
    if value.get("suggest_needs_new_label").is_none() {
        value["suggest_needs_new_label"] = Value::Bool(false);
    }
    if value.get("suggest_degraded").is_none() {
        value["suggest_degraded"] = Value::Bool(false);
    }
    if let Some(signals) = value.get_mut("signals").and_then(Value::as_array_mut) {
        for signal in signals {
            if let Some(object) = signal.as_object_mut() {
                if let Some(kind) = object.get("kind").and_then(Value::as_str) {
                    object.insert("kind".to_owned(), Value::String(kind.to_owned()));
                }
                if let Some(item) = object.remove("related_labels") {
                    object.insert(
                        "related_labels_json".to_owned(),
                        Value::String(item.to_string()),
                    );
                }
                if let Some(item) = object.remove("proposal") {
                    object.insert("proposal_json".to_owned(), Value::String(item.to_string()));
                }
                if let Some(candidate) = object.remove("candidate_atom")
                    && let Some(candidate) = candidate.as_object()
                {
                    object.insert(
                        "candidate_atom_polarity".to_owned(),
                        candidate.get("polarity").cloned().unwrap_or(Value::Null),
                    );
                    object.insert(
                        "candidate_atom_kind".to_owned(),
                        candidate.get("kind").cloned().unwrap_or(Value::Null),
                    );
                    object.insert(
                        "candidate_text".to_owned(),
                        candidate.get("text").cloned().unwrap_or(Value::Null),
                    );
                }
                if object.get("related_labels_json").is_none() {
                    object.insert(
                        "related_labels_json".to_owned(),
                        Value::String("[]".to_owned()),
                    );
                }
                if object.get("proposal_json").is_none() {
                    object.insert("proposal_json".to_owned(), Value::String("{}".to_owned()));
                }
                if object.get("agent_selected").is_none() {
                    object.insert("agent_selected".to_owned(), Value::Bool(false));
                }
                if object.get("final_selected").is_none() {
                    object.insert("final_selected".to_owned(), Value::Bool(false));
                }
                if object.get("rationale").is_none() {
                    object.insert("rationale".to_owned(), Value::String(String::new()));
                }
                if object.get("proposed_action").is_none() {
                    object.insert(
                        "proposed_action".to_owned(),
                        Value::String("observe".to_owned()),
                    );
                }
            }
        }
    }
}

fn normalize_action_body(value: &mut Value) {
    actor_object(value);
    if let Some(item) = value.get("change").cloned() {
        value["change_json"] = Value::String(item.to_string());
    }
    if let Some(item) = value.get("validation").cloned() {
        value["validation_json"] = Value::String(item.to_string());
    }
    if value.get("change_json").is_none() {
        value["change_json"] = Value::String("{}".to_owned());
    }
    if value.get("validation_json").is_none() {
        value["validation_json"] = Value::String("{}".to_owned());
    }
}

fn contractize(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(contractize).collect()),
        Value::Object(object) => {
            let mut output = serde_json::Map::new();
            for (key, value) in object {
                let value = contractize(value);
                let target = match key.as_str() {
                    "task_snapshot_json" => "task_snapshot",
                    "agent_candidates_json" => "agent_candidates",
                    "suggestion_snapshot_json" => "suggestion_snapshot",
                    "final_decision_json" => "final_decision",
                    "diagnostics_json" => "diagnostics",
                    "related_labels_json" => "related_labels",
                    "proposal_json" => "proposal",
                    "change_json" => "change",
                    "validation_json" => "validation",
                    "manual_json" => "manual",
                    "summary_json" => "summary",
                    "cases_json" => "cases",
                    "labels_json" => "labels",
                    "candidate_atom_variants_json" => "candidate_atom_variants",
                    _ => key.as_str(),
                };
                if target == "labels" && value.is_string() {
                    let labels = value
                        .as_str()
                        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                        .unwrap_or_default()
                        .into_iter()
                        .map(|id| json!({"id": id, "name": Value::Null}))
                        .collect::<Vec<_>>();
                    output.insert(target.to_owned(), Value::Array(labels));
                } else if key.ends_with("_json") && value.is_string() {
                    let parsed = value
                        .as_str()
                        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                        .unwrap_or(Value::Null);
                    output.insert(target.to_owned(), parsed);
                } else {
                    output.insert(target.to_owned(), value);
                }
            }
            if let Some(status) = output
                .get("validation_status")
                .and_then(Value::as_str)
                .map(str::to_owned)
            {
                output
                    .entry("validation_effective_outcome".to_owned())
                    .or_insert_with(|| Value::String(status.clone()));
                output
                    .entry("validation_latest_attempt_id".to_owned())
                    .or_insert(Value::Null);
            }
            Value::Object(output)
        }
        other => other,
    }
}

async fn run(
    state: State<AppState>,
    operation: &str,
    board: &str,
    input: Value,
) -> Result<Json<Value>, ApiError> {
    let value = state
        .application()
        .label_ontology(operation, board, input)
        .await?;
    Ok(Json(contractize(value)))
}

pub(crate) async fn list_semantics(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let Json(value) = run(
        State(state),
        "list_semantics",
        &board_path(board),
        json!({}),
    )
    .await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn get_semantics(
    State(state): State<AppState>,
    Path((board, label_id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let Json(value) = run(
        State(state),
        "get_semantics",
        &board,
        json!({"label_ref": label_id}),
    )
    .await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn upsert_semantics(
    State(state): State<AppState>,
    Path((board, label_id)): Path<(String, String)>,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    body["label_ref"] = Value::String(label_id);
    if body.get("actor").is_none() {
        body["actor"] = Value::String("http".to_owned());
    }
    let Json(value) = run(State(state), "upsert_semantics", &board, body).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn delete_semantics(
    State(state): State<AppState>,
    Path((board, label_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let mut input = json!(query);
    input["label_ref"] = Value::String(label_id);
    let Json(value) = run(State(state), "delete_semantics", &board, input).await?;
    Ok(Json(json!({"data": {"deleted": value}})))
}

pub(crate) async fn list_atoms(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let Json(value) = run(State(state), "list_atoms", &board, json!({})).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn explain_atom(
    State(state): State<AppState>,
    Path((board, atom_ref)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let Json(value) = run(
        State(state),
        "explain_atom",
        &board,
        json!({"atom_ref": atom_ref}),
    )
    .await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn index_status(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let Json(value) = run(State(state), "index_status", &board, json!({})).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn rebuild_index(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let Json(value) = run(State(state), "rebuild_atom_index", &board, json!({})).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn query_index(
    State(state): State<AppState>,
    Path(board): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let input = json!({
        "query": query.get("q").or_else(|| query.get("query")),
        "polarity": query.get("polarity"),
        "limit": query.get("limit").and_then(|value| value.parse::<usize>().ok()).unwrap_or(24),
    });
    let Json(value) = run(State(state), "query_atom_index", &board, input).await?;
    let data = value.get("data").cloned().unwrap_or(value);
    Ok(Json(json!({"data": data})))
}

async fn resolve_task_board(
    state: &AppState,
    task_ref: &str,
    requested_board: Option<&str>,
) -> Result<String, ApiError> {
    let task = state.application().get_task(task_ref).await?;
    if let Some(requested_board) = requested_board
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && requested_board != task.board_id
        && requested_board != task.board_slug
    {
        return Err(ApiError(KanbanError::InvalidInput(format!(
            "task {task_ref} 不属于 board {requested_board}"
        ))));
    }
    Ok(task.board_id)
}

pub(crate) async fn suggestions(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(mut query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let requested_board = query.get("board").map(String::as_str);
    let board = resolve_task_board(&state, &task_id, requested_board).await?;
    query.remove("board");
    query.insert("task_ref".to_owned(), task_id);
    let Json(value) = run(
        State(state),
        "suggest_labels",
        &board,
        json!({"task_ref": query.get("task_ref"), "options": query}),
    )
    .await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn list_proposals_for_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(mut query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let requested_board = query.get("board").map(String::as_str);
    let board = resolve_task_board(&state, &task_id, requested_board).await?;
    query.remove("board");
    let value = run(
        State(state),
        "list_proposals",
        &board,
        json!({"task_ref": task_id, "status": query.get("status")}),
    )
    .await?
    .0;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn list_proposals_for_board(
    State(state): State<AppState>,
    Path(ListBoardLabelProposalsPath { board }): Path<ListBoardLabelProposalsPath>,
    query: Result<Query<ListBoardLabelProposalsQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<ListBoardLabelProposalsResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("查询参数无效：{error}")))?;
    let mut input = serde_json::Map::new();
    if let Some(status) = query.status {
        let status = serde_json::to_value(status).map_err(|error| {
            KanbanError::InvalidInput(format!("label proposal status 无法编码：{error}"))
        })?;
        input.insert("status".to_owned(), status);
    }
    let Json(value) = run(State(state), "list_proposals", &board, Value::Object(input)).await?;
    let proposals = serde_json::from_value(value).map_err(|error| {
        KanbanError::Storage(format!("label proposals response 无法解码：{error}"))
    })?;
    Ok(Json(DataEnvelope::new(proposals)))
}

pub(crate) async fn propose_for_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    let requested_board = query.get("board").map(String::as_str);
    let board = resolve_task_board(&state, &task_id, requested_board).await?;
    let mut input = body_or_empty(body);
    input["task_ref"] = Value::String(task_id);
    normalize_proposal_body(&mut input);
    if input.get("actor").is_none() {
        input["actor"] = Value::String("user".to_owned());
    }
    let Json(value) = run(State(state), "propose_label", &board, input).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn record_observation(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let requested_board = query.get("board").map(String::as_str);
    let board = resolve_task_board(&state, &task_id, requested_board).await?;
    body["task_ref"] = Value::String(task_id);
    normalize_observation_body(&mut body);
    let Json(value) = run(State(state), "record_observation", &board, body).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn list_signals(
    State(state): State<AppState>,
    Path(board): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let statuses = query
        .get("status")
        .map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let kinds = query
        .get("kind")
        .map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let input = json!({"statuses": statuses, "kinds": kinds, "task_ref": query.get("task_ref"), "target_label_ref": query.get("target_label_ref"), "proposed_label_name": query.get("proposed_label_name"), "include_all": query.get("include_all").is_some_and(|v| v == "true"), "limit": query.get("limit").and_then(|v| v.parse::<usize>().ok()).unwrap_or(100)});
    let Json(value) = run(State(state), "list_signals", &board, input).await?;
    Ok(Json(
        json!({"data": value, "meta": {"include_all": query.get("include_all").is_some_and(|v| v == "true"), "limit": query.get("limit").and_then(|v| v.parse::<usize>().ok()).unwrap_or(100)}}),
    ))
}

pub(crate) async fn review_signals(
    State(state): State<AppState>,
    Path(board): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    if query.get("quality").is_some_and(|value| value == "true") {
        let sample_limit = query
            .get("sample_limit")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(20);
        let value = state
            .application()
            .label_ontology("quality", &board, json!({"sample_limit": sample_limit}))
            .await?;
        let quality = quality_output(value)?;
        let quality = serde_json::to_value(quality).map_err(|error| {
            ApiError(KanbanError::Storage(format!(
                "label ontology quality 无法编码：{error}"
            )))
        })?;
        return Ok(Json(quality));
    }
    let group_by = query
        .get("group_by")
        .cloned()
        .unwrap_or_else(|| "label".to_owned());
    let group_by = match group_by.as_str() {
        "candidate_atom" | "proposed_label" | "cluster" | "label" => group_by,
        _ => "label".to_owned(),
    };
    let include_all = query.get("include_all").is_some_and(|v| v == "true");
    let limit = query
        .get("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100);
    let Json(value) = run(
        State(state),
        "review_signals",
        &board,
        json!({"group_by": group_by, "include_all": include_all, "limit": limit}),
    )
    .await?;
    Ok(Json(
        json!({"data": value, "meta": {"group_by": group_by, "include_all": include_all, "limit": limit}}),
    ))
}

pub(crate) async fn get_signal(
    State(state): State<AppState>,
    Path(signal_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let Json(value) = run(
        State(state),
        "get_signal",
        GLOBAL_RECORD_SCOPE,
        json!({"signal_id": signal_id}),
    )
    .await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn get_proposal(
    State(state): State<AppState>,
    Path(proposal_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let Json(value) = run(
        State(state),
        "get_proposal",
        GLOBAL_RECORD_SCOPE,
        json!({"proposal_id": proposal_id}),
    )
    .await?;
    Ok(Json(json!({"data": value})))
}

async fn decide_proposal(
    State(state): State<AppState>,
    proposal_id: String,
    Json(mut body): Json<Value>,
    accept: bool,
) -> Result<Json<Value>, ApiError> {
    actor_object(&mut body);
    if let Some(actor) = body.get("actor").and_then(Value::as_object) {
        body["actor"] = Value::String(
            actor
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("user")
                .to_owned(),
        );
    }
    body["proposal_id"] = Value::String(proposal_id);
    body["accept"] = Value::Bool(accept);
    if body.get("actor").is_none() {
        body["actor"] = Value::String("user".to_owned());
    }
    let Json(value) = run(State(state), "decide_proposal", GLOBAL_RECORD_SCOPE, body).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn accept_proposal(
    State(state): State<AppState>,
    Path(proposal_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    decide_proposal(State(state), proposal_id, Json(body), true).await
}

pub(crate) async fn reject_proposal(
    State(state): State<AppState>,
    Path(proposal_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    decide_proposal(State(state), proposal_id, Json(body), false).await
}

pub(crate) async fn create_action(
    State(state): State<AppState>,
    Path(board): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let mut body = body;
    normalize_action_body(&mut body);
    let Json(value) = run(State(state), "create_action", &board, body).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn apply_atom(
    State(state): State<AppState>,
    Path(board): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let mut body = body;
    actor_object(&mut body);
    if body.get("polarity").is_none() {
        let polarity = match body.get("kind").and_then(Value::as_str) {
            Some("excludes_when") | Some("negative_example") => "negative",
            _ => "positive",
        };
        body["polarity"] = Value::String(polarity.to_owned());
    }
    let Json(value) = run(State(state), "apply_atom", &board, body).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn revert(
    State(state): State<AppState>,
    Path(board): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let mut body = body;
    actor_object(&mut body);
    let Json(value) = run(State(state), "revert_mutation", &board, body).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn validate(
    State(state): State<AppState>,
    Path(board): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let mut body = body;
    actor_object(&mut body);
    if let Some(item) = body.get("validation").cloned() {
        body["validation_json"] = Value::String(item.to_string());
    }
    if body.get("validation_json").is_none() {
        body["validation_json"] = Value::String("{}".to_owned());
    }
    let Json(value) = run(State(state), "validate_action", &board, body).await?;
    Ok(Json(json!({"data": value})))
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/boards/:board/labels/semantics",
            ),
            get(list_semantics),
        )
        .route(
            crate::http::operations::registered_paths(
                "/api/v1/boards/:board/labels/:label_id/semantics",
                &[
                    kanban_protocol::HttpMethod::Get,
                    kanban_protocol::HttpMethod::Put,
                    kanban_protocol::HttpMethod::Delete,
                ],
            ),
            get(get_semantics)
                .put(upsert_semantics)
                .delete(delete_semantics),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/boards/:board/labels/atoms",
            ),
            get(list_atoms),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/boards/:board/labels/atoms/:atom_ref/explain",
            ),
            get(explain_atom),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/boards/:board/labels/atom-index/status",
            ),
            get(index_status),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/boards/:board/labels/atom-index/rebuild",
            ),
            post(rebuild_index),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/boards/:board/labels/atom-index/query",
            ),
            get(query_index),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/tasks/:task_id/labels/suggestions",
            ),
            get(suggestions),
        )
        .route(
            crate::http::operations::registered_paths(
                "/api/v1/tasks/:task_id/label-proposals",
                &[
                    kanban_protocol::HttpMethod::Get,
                    kanban_protocol::HttpMethod::Post,
                ],
            ),
            get(list_proposals_for_task).post(propose_for_task),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/boards/:board/label-proposals",
            ),
            get(list_proposals_for_board),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/tasks/:task_id/label-ontology/observations",
            ),
            post(record_observation),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/boards/:board/label-ontology/signals",
            ),
            get(list_signals),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/boards/:board/label-ontology/review",
            ),
            get(review_signals),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/boards/:board/label-ontology/actions",
            ),
            post(create_action),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/boards/:board/label-ontology/apply/atom",
            ),
            post(apply_atom),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/boards/:board/label-ontology/revert",
            ),
            post(revert),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/boards/:board/label-ontology/validate",
            ),
            post(validate),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/label-ontology/signals/:signal_id",
            ),
            get(get_signal),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Get,
                "/api/v1/label-proposals/:proposal_id",
            ),
            get(get_proposal),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/label-proposals/:proposal_id/accept",
            ),
            post(accept_proposal),
        )
        .route(
            crate::http::operations::registered_path(
                kanban_protocol::HttpMethod::Post,
                "/api/v1/label-proposals/:proposal_id/reject",
            ),
            post(reject_proposal),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::operations::test_support::*;
    use axum::Router;
    use kanban_protocol::{
        ApiErrorCode, CreateBoardResponse, CreateTaskResponse, ErrorEnvelope,
        LabelProposalDecisionResponse, ListBoardLabelProposalsResponse,
        ListTaskLabelProposalsResponse, ProposeTaskLabelResponse,
        cli_labels::CliLabelOntologyQualityOutput,
    };

    async fn create_board(router: &Router, board: &str) {
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards",
                serde_json::json!({
                    "slug": board,
                    "name": format!("{board} board"),
                    "description": "label proposal HTTP test",
                    "actor": "tester"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let _: CreateBoardResponse =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
    }

    async fn create_task(router: &Router, board: &str, task_id: &str) {
        let response = router
            .clone()
            .oneshot(json_request(
                &format!("/api/v1/boards/{board}/tasks"),
                serde_json::json!({
                    "task_id": task_id,
                    "title": format!("{task_id} title"),
                    "description": "label proposal HTTP test",
                    "priority": 1,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "tester"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let _: CreateTaskResponse =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
    }

    async fn create_proposal(
        router: &Router,
        board: &str,
        task_id: &str,
        name: &str,
    ) -> kanban_protocol::LabelSemanticProposalWire {
        let response = router
            .clone()
            .oneshot(json_request(
                &format!("/api/v1/tasks/{task_id}/label-proposals?board={board}"),
                serde_json::json!({
                    "name": name,
                    "description": "proposal fixture",
                    "applies_when": ["HTTP route"],
                    "excludes_when": [],
                    "positive_examples": [],
                    "negative_examples": [],
                    "actor": "tester"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let created: ProposeTaskLabelResponse =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        created.data.proposal.expect("explicit proposal candidate")
    }

    async fn list_board_proposals(
        router: &Router,
        board: &str,
        status: Option<&str>,
    ) -> ListBoardLabelProposalsResponse {
        let uri = status.map_or_else(
            || format!("/api/v1/boards/{board}/label-proposals"),
            |status| format!("/api/v1/boards/{board}/label-proposals?status={status}"),
        );
        let response = router
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }

    async fn list_task_proposals(
        router: &Router,
        board: &str,
        task_id: &str,
        status: Option<&str>,
    ) -> ListTaskLabelProposalsResponse {
        let uri = status.map_or_else(
            || format!("/api/v1/tasks/{task_id}/label-proposals?board={board}"),
            |status| {
                format!("/api/v1/tasks/{task_id}/label-proposals?board={board}&status={status}")
            },
        );
        let response = router
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }

    #[tokio::test]
    async fn board_proposal_list_uses_the_board_scoped_service_path() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards/default/label-proposals?status=proposed")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let proposals: ListBoardLabelProposalsResponse = serde_json::from_slice(&body).unwrap();
        assert!(proposals.data.is_empty());
    }

    #[tokio::test]
    async fn board_proposal_list_is_isolated_and_filters_status_with_typed_scopes() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        create_board(&router, "other").await;
        create_task(&router, "default", "t_board_default_accepted").await;
        create_task(&router, "default", "t_board_default_proposed").await;
        create_task(&router, "other", "t_board_other_proposed").await;

        let accepted = create_proposal(
            &router,
            "default",
            "t_board_default_accepted",
            "default accepted",
        )
        .await;
        let proposed = create_proposal(
            &router,
            "default",
            "t_board_default_proposed",
            "default proposed",
        )
        .await;
        let other =
            create_proposal(&router, "other", "t_board_other_proposed", "other proposed").await;

        let response = router
            .clone()
            .oneshot(json_request(
                &format!("/api/v1/label-proposals/{}/accept", accepted.id),
                serde_json::json!({"reason": "test", "actor": "tester"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let decided: LabelProposalDecisionResponse =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(matches!(
            decided.data.status,
            kanban_protocol::LabelProposalStatusWire::Accepted
        ));

        let all_default = list_board_proposals(&router, "default", None).await;
        assert_eq!(all_default.data.len(), 2);
        assert!(
            all_default
                .data
                .iter()
                .all(|item| item.board_id == accepted.board_id)
        );
        assert!(all_default.data.iter().any(|item| item.id == accepted.id));
        assert!(all_default.data.iter().any(|item| item.id == proposed.id));
        assert!(all_default.data.iter().all(|item| item.id != other.id));

        let accepted_default = list_board_proposals(&router, "default", Some("accepted")).await;
        assert_eq!(accepted_default.data.len(), 1);
        assert_eq!(accepted_default.data[0].id, accepted.id);
        assert!(matches!(
            accepted_default.data[0].status,
            kanban_protocol::LabelProposalStatusWire::Accepted
        ));

        let proposed_default = list_board_proposals(&router, "default", Some("proposed")).await;
        assert_eq!(proposed_default.data.len(), 1);
        assert_eq!(proposed_default.data[0].id, proposed.id);

        let all_other = list_board_proposals(&router, "other", None).await;
        assert_eq!(all_other.data.len(), 1);
        assert_eq!(all_other.data[0].id, other.id);
        assert_eq!(all_other.data[0].board_id, other.board_id);

        let task_default = list_task_proposals(
            &router,
            "default",
            "t_board_default_accepted",
            Some("accepted"),
        )
        .await;
        assert_eq!(task_default.data.len(), 1);
        assert_eq!(task_default.data[0].id, accepted.id);

        let task_other =
            list_task_proposals(&router, "other", "t_board_other_proposed", None).await;
        assert_eq!(task_other.data.len(), 1);
        assert_eq!(task_other.data[0].id, other.id);
    }

    #[tokio::test]
    async fn board_proposal_list_rejects_invalid_queries_with_error_envelopes() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        for (uri, expected_status, expected_code) in [
            (
                "/api/v1/boards/default/label-proposals?status=unknown",
                StatusCode::BAD_REQUEST,
                ApiErrorCode::InvalidInput,
            ),
            (
                "/api/v1/boards/default/label-proposals?unexpected=1",
                StatusCode::BAD_REQUEST,
                ApiErrorCode::InvalidInput,
            ),
            (
                "/api/v1/boards/not-found/label-proposals",
                StatusCode::NOT_FOUND,
                ApiErrorCode::NotFound,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected_status, "{uri}");
            let error: ErrorEnvelope =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(error.error.code, expected_code, "{uri}");
            assert!(!error.error.message.trim().is_empty(), "{uri}");
        }
    }

    #[tokio::test]
    async fn quality_route_returns_typed_cli_contract() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .uri(
                        "/api/v1/boards/default/label-ontology/review?quality=true&sample_limit=10",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(status, StatusCode::OK);
        let quality: CliLabelOntologyQualityOutput = serde_json::from_slice(&body).unwrap();
        assert_eq!(quality.data.board_id, "b_default");
        assert_eq!(
            quality.data.denominator.source,
            "label_ontology_observations"
        );
        assert_eq!(quality.data.denominator.observation_count, 0);
    }
}
