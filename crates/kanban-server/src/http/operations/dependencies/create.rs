use super::super::support::request_actor;
use super::support::*;
use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    routing::post,
};
use kanban_application::AddDependencyCommand;
use kanban_core::KanbanError;
use kanban_protocol::{AddDependencyPath, AddDependencyRequest, AddDependencyResponse};

pub(crate) async fn add_dependency(
    State(state): State<AppState>,
    Path(AddDependencyPath { task_id }): Path<AddDependencyPath>,
    headers: HeaderMap,
    body: Result<Json<AddDependencyRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<AddDependencyResponse>), ApiError> {
    let Json(body) =
        body.map_err(|error| KanbanError::InvalidInput(format!("invalid JSON body: {error}")))?;
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let result = state
        .application()
        .add_dependency(AddDependencyCommand {
            child_task_id: task_id,
            parent_task_id: body.parent_task_id,
            actor,
        })
        .await?;
    let status = if result.added {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        Json(AddDependencyResponse {
            data: api_dependencies(result.dependencies)?,
        }),
    ))
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/tasks/:task_id/dependencies", post(add_dependency))
}
#[cfg(test)]
mod tests {
    use crate::http::operations::test_support::*;

    #[tokio::test]
    async fn dependency_create_and_list_use_the_shared_application_path() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        for (task_id, key, title) in [
            (
                "t_http_dependency_parent",
                "http-dependency-parent",
                "Parent",
            ),
            ("t_http_dependency_child", "http-dependency-child", "Child"),
        ] {
            let response = router
                .clone()
                .oneshot(json_request(
                    "/api/v1/boards/default/tasks",
                    serde_json::json!({
                        "task_id": task_id,
                        "idempotency_key": key,
                        "title": title,
                        "description": "dependency test",
                        "priority": 1,
                        "metadata": {},
                        "labels": [],
                        "depends_on": [],
                        "actor": "seed"
                    }),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        let first = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_dependency_child/dependencies",
                serde_json::json!({
                    "parent_task_id": "t_http_dependency_parent",
                    "actor": "test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        let first_body = first.into_body().collect().await.unwrap().to_bytes();
        let first: AddDependencyResponse = serde_json::from_slice(&first_body).unwrap();
        assert_eq!(first.data.task.id, "t_http_dependency_child");
        assert_eq!(first.data.parents.len(), 1);
        assert_eq!(first.data.parents[0].id, "t_http_dependency_parent");
        assert_eq!(first.data.edges.len(), 1);

        let replay = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_dependency_child/dependencies",
                serde_json::json!({
                    "parent_task_id": "t_http_dependency_parent",
                    "actor": "test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body = replay.into_body().collect().await.unwrap().to_bytes();
        let replay: AddDependencyResponse = serde_json::from_slice(&replay_body).unwrap();
        assert_eq!(replay.data, first.data);

        let listed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/tasks/t_http_dependency_child/dependencies")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::OK);
        let listed_body = listed.into_body().collect().await.unwrap().to_bytes();
        let listed: ListDependenciesResponse = serde_json::from_slice(&listed_body).unwrap();
        assert_eq!(listed.data, first.data);

        let parent_listed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/tasks/t_http_dependency_parent/dependencies")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(parent_listed.status(), StatusCode::OK);
        let parent_body = parent_listed
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let parent_listed: ListDependenciesResponse = serde_json::from_slice(&parent_body).unwrap();
        assert_eq!(parent_listed.data.children.len(), 1);
        assert_eq!(parent_listed.data.children[0].id, "t_http_dependency_child");

        let cycle = router
            .clone()
            .oneshot(json_request(
                "/api/v1/tasks/t_http_dependency_parent/dependencies",
                serde_json::json!({
                    "parent_task_id": "t_http_dependency_child",
                    "actor": "test"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(cycle.status(), StatusCode::CONFLICT);
        let cycle_body = cycle.into_body().collect().await.unwrap().to_bytes();
        let cycle: ErrorEnvelope = serde_json::from_slice(&cycle_body).unwrap();
        assert_eq!(cycle.error.code, ApiErrorCode::DependencyCycle);

        let removed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/tasks/t_http_dependency_child/dependencies/t_http_dependency_parent")
                    .header("X-KB-Actor", "remove-test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(removed.status(), StatusCode::OK);
        let removed_body = removed.into_body().collect().await.unwrap().to_bytes();
        let removed: kanban_protocol::RemoveDependencyResponse =
            serde_json::from_slice(&removed_body).unwrap();
        assert!(removed.data.parents.is_empty());
        assert!(removed.data.edges.is_empty());

        let replay = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/tasks/t_http_dependency_child/dependencies/t_http_dependency_parent")
                    .header("X-KB-Actor", "remove-replay")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::OK);

        let missing = router
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/tasks/t_http_dependency_missing/dependencies")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }
}
