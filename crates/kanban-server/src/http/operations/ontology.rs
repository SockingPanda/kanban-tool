//! Label semantics/ontology HTTP surface.

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
};
use kanban_core::KanbanError;
use serde_json::{Value, json};

use crate::{error::ApiError, state::AppState};

fn board_path(board: String) -> String {
    board
}

fn body_or_empty(body: Option<Json<Value>>) -> Value {
    body.map(|Json(value)| value).unwrap_or_else(|| json!({}))
}

fn actor_object(value: &mut Value) {
    if let Some(actor) = value.get_mut("actor").and_then(Value::as_object_mut) {
        if actor.get("actor_type").is_none() {
            if let Some(actor_type) = actor.remove("type") {
                actor.insert("actor_type".to_owned(), actor_type);
            }
        }
    }
    let ontology_actor = value
        .get_mut("ontology_actor")
        .and_then(Value::as_object_mut)
        .map(|actor| {
            if actor.get("actor_type").is_none() {
                if let Some(actor_type) = actor.remove("type") {
                    actor.insert("actor_type".to_owned(), actor_type);
                }
            }
            actor.clone()
        });
    if value.get("actor").is_none() {
        if let Some(actor) = ontology_actor {
            value["actor"] = Value::Object(actor);
        }
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
                if let Some(candidate) = object.remove("candidate_atom") {
                    if let Some(candidate) = candidate.as_object() {
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
    Ok(Json(json!({"data": value})))
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
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn suggestions(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(mut query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let board = query
        .remove("board")
        .unwrap_or_else(|| "default".to_owned());
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
    let board = query
        .remove("board")
        .unwrap_or_else(|| "default".to_owned());
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

pub(crate) async fn propose_for_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, ApiError> {
    let board = query.get("board").map(String::as_str).unwrap_or("default");
    let mut input = body_or_empty(body);
    input["task_ref"] = Value::String(task_id);
    normalize_proposal_body(&mut input);
    if input.get("actor").is_none() {
        input["actor"] = Value::String("user".to_owned());
    }
    let Json(value) = run(State(state), "propose_label", board, input).await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn record_observation(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let board = query.get("board").map(String::as_str).unwrap_or("default");
    body["task_ref"] = Value::String(task_id);
    normalize_observation_body(&mut body);
    let Json(value) = run(State(state), "record_observation", board, body).await?;
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
        let Json(value) = run(
            State(state),
            "quality",
            &board,
            json!({"sample_limit": sample_limit}),
        )
        .await?;
        return Ok(Json(json!({"data": value})));
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
        "default",
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
        "default",
        json!({"proposal_id": proposal_id}),
    )
    .await?;
    Ok(Json(json!({"data": value})))
}

pub(crate) async fn decide_proposal(
    State(state): State<AppState>,
    Path((proposal_id, decision)): Path<(String, String)>,
    Json(mut body): Json<Value>,
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
    body["accept"] = Value::Bool(decision == "accept");
    if body.get("actor").is_none() {
        body["actor"] = Value::String("user".to_owned());
    }
    let Json(value) = run(State(state), "decide_proposal", "default", body).await?;
    Ok(Json(json!({"data": value})))
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
            "/api/v1/boards/:board/labels/semantics",
            get(list_semantics),
        )
        .route(
            "/api/v1/boards/:board/labels/:label_id/semantics",
            get(get_semantics)
                .put(upsert_semantics)
                .delete(delete_semantics),
        )
        .route("/api/v1/boards/:board/labels/atoms", get(list_atoms))
        .route(
            "/api/v1/boards/:board/labels/atoms/:atom_ref/explain",
            get(explain_atom),
        )
        .route(
            "/api/v1/boards/:board/labels/atom-index/status",
            get(index_status),
        )
        .route(
            "/api/v1/boards/:board/labels/atom-index/rebuild",
            post(rebuild_index),
        )
        .route(
            "/api/v1/boards/:board/labels/atom-index/query",
            get(query_index),
        )
        .route(
            "/api/v1/tasks/:task_id/labels/suggestions",
            get(suggestions),
        )
        .route(
            "/api/v1/tasks/:task_id/label-proposals",
            get(list_proposals_for_task).post(propose_for_task),
        )
        .route(
            "/api/v1/tasks/:task_id/label-ontology/observations",
            post(record_observation),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/signals",
            get(list_signals),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/review",
            get(review_signals),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/actions",
            post(create_action),
        )
        .route(
            "/api/v1/boards/:board/label-ontology/apply/atom",
            post(apply_atom),
        )
        .route("/api/v1/boards/:board/label-ontology/revert", post(revert))
        .route(
            "/api/v1/boards/:board/label-ontology/validate",
            post(validate),
        )
        .route("/api/v1/label-ontology/signals/:signal_id", get(get_signal))
        .route("/api/v1/label-proposals/:proposal_id", get(get_proposal))
        .route(
            "/api/v1/label-proposals/:proposal_id/accept",
            post(decide_proposal),
        )
        .route(
            "/api/v1/label-proposals/:proposal_id/reject",
            post(decide_proposal),
        )
}
