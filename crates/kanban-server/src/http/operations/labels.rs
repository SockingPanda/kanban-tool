use super::support::request_actor;
use crate::{
    error::ApiError,
    http::operations::tasks::support::{api_label, api_task},
    state::AppState,
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    routing::{delete, get},
};
use kanban_application::{AddTaskLabelsCommand, CreateBoardLabelCommand, RemoveTaskLabelCommand};
use kanban_contract::{
    AddTaskLabelPath, AddTaskLabelRequest, AddTaskLabelResponse, BoardLabelPath,
    CreateBoardLabelRequest, CreateBoardLabelResponse, DataEnvelope, ListBoardLabelsResponse,
    ListTaskLabelsPath, ListTaskLabelsResponse, RemoveTaskLabelPath, RemoveTaskLabelResponse,
};
use kanban_core::KanbanError;

pub(crate) async fn list_board_labels(
    State(state): State<AppState>,
    Path(BoardLabelPath { board }): Path<BoardLabelPath>,
) -> Result<Json<ListBoardLabelsResponse>, ApiError> {
    let labels = state.application().list_board_labels(&board).await?;
    Ok(Json(DataEnvelope {
        data: labels.into_iter().map(api_label).collect(),
    }))
}

pub(crate) async fn create_board_label(
    State(state): State<AppState>,
    Path(BoardLabelPath { board }): Path<BoardLabelPath>,
    body: Result<Json<CreateBoardLabelRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateBoardLabelResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let label = state
        .application()
        .create_board_label(CreateBoardLabelCommand {
            board,
            name: body.name,
            color: body.color,
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope {
            data: api_label(label),
        }),
    ))
}

pub(crate) async fn list_task_labels(
    State(state): State<AppState>,
    Path(ListTaskLabelsPath { task_id }): Path<ListTaskLabelsPath>,
) -> Result<Json<ListTaskLabelsResponse>, ApiError> {
    let labels = state.application().list_task_labels(&task_id).await?;
    Ok(Json(ListTaskLabelsResponse {
        data: labels.into_iter().map(api_label).collect(),
    }))
}

pub(crate) async fn add_task_labels(
    State(state): State<AppState>,
    Path(AddTaskLabelPath { task_id }): Path<AddTaskLabelPath>,
    headers: HeaderMap,
    body: Result<Json<AddTaskLabelRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AddTaskLabelResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let names = body
        .label_names()
        .map_err(|error| KanbanError::InvalidInput(error.to_owned()))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let record = state
        .application()
        .add_task_labels(AddTaskLabelsCommand {
            task_id,
            names,
            create_missing: body.create_missing,
            actor,
        })
        .await?;
    let meta = (!record.created_labels.is_empty()).then(|| kanban_contract::CreatedLabelsMeta {
        created_labels: record.created_labels.into_iter().map(api_label).collect(),
    });
    Ok((
        StatusCode::CREATED,
        Json(AddTaskLabelResponse {
            data: api_task(record.task)?,
            meta,
        }),
    ))
}

pub(crate) async fn remove_task_label(
    State(state): State<AppState>,
    Path(RemoveTaskLabelPath { task_id, label_id }): Path<RemoveTaskLabelPath>,
    headers: HeaderMap,
) -> Result<Json<RemoveTaskLabelResponse>, ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let task = state
        .application()
        .remove_task_label(RemoveTaskLabelCommand {
            task_id,
            label_ref: label_id,
            actor,
        })
        .await?;
    Ok(Json(RemoveTaskLabelResponse {
        data: api_task(task)?,
    }))
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/boards/:board/labels",
            get(list_board_labels).post(create_board_label),
        )
        .route(
            "/api/v1/tasks/:task_id/labels",
            get(list_task_labels).post(add_task_labels),
        )
        .route(
            "/api/v1/tasks/:task_id/labels/:label_id",
            delete(remove_task_label),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::operations::test_support::*;

    async fn create_task(router: &Router, task_id: &str) {
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": task_id,
                    "title": "label task",
                    "description": null,
                    "status": "todo",
                    "assignee": null,
                    "priority": 1,
                    "scheduled_at": null,
                    "due_at": null,
                    "max_retries": 2,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "label-test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn labels_round_trip_is_idempotent_and_emits_add_remove_events() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/labels",
                serde_json::json!({"name": "  urgent ", "color": "#d14"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let created: CreateBoardLabelResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(created.data.name, "urgent");
        assert_eq!(created.data.color.as_deref(), Some("#d14"));

        create_task(&router, "t_labels_http").await;

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_labels_http/labels",
                serde_json::json!({
                    "names": [" urgent ", "urgent"],
                    "create_missing": false,
                    "actor": "label-test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let added: AddTaskLabelResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(added.data.labels.len(), 1);
        assert!(added.meta.is_none());
        let label_id = added.data.labels[0].id.clone();

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_labels_http/labels",
                serde_json::json!({
                    "name": "urgent",
                    "create_missing": false,
                    "actor": "label-test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let duplicate: AddTaskLabelResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(duplicate.data.labels.len(), 1);
        assert_eq!(duplicate.data.labels[0].id, label_id);
        assert!(duplicate.meta.is_none());

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks/t_labels_http/labels")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let listed: ListTaskLabelsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(listed.data.len(), 1);
        assert_eq!(listed.data[0].id, label_id);

        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/api/v1/tasks/t_labels_http/labels/{label_id}"))
                    .header("x-kb-actor", "label-test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let removed: RemoveTaskLabelResponse = serde_json::from_slice(&body).unwrap();
        assert!(removed.data.labels.is_empty());

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events?board=default&task_id=t_labels_http")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let events: ListEventsResponse = serde_json::from_slice(&body).unwrap();
        let kinds: Vec<_> = events
            .data
            .iter()
            .map(|event| event.kind.as_str())
            .collect();
        assert_eq!(
            kinds,
            vec!["task.created", "task.label.added", "task.label.removed"]
        );
    }

    #[tokio::test]
    async fn labels_add_create_missing_returns_created_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        create_task(&router, "t_labels_create_missing").await;

        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_labels_create_missing/labels",
                serde_json::json!({
                    "name": "  generated ",
                    "create_missing": true,
                    "actor": "label-test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let added: AddTaskLabelResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(added.data.labels.len(), 1);
        let meta = added.meta.expect("created label metadata");
        assert_eq!(meta.created_labels.len(), 1);
        assert_eq!(meta.created_labels[0].name, "generated");

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/boards/default/labels")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let labels: ListBoardLabelsResponse = serde_json::from_slice(&body).unwrap();
        assert!(labels.data.iter().any(|label| label.name == "generated"));
    }
}
