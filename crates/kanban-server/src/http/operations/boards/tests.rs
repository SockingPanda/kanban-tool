use crate::http::operations::test_support::*;

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn board_routes_cover_create_duplicate_archive_and_active_only_get() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("kanban.db"), "test")
        .await
        .unwrap();
    let router = build_router(state);
    let request = serde_json::json!({
        "slug": "http-board",
        "name": "HTTP 看板",
        "description": "route test",
        "actor": "http-test"
    });

    let response = router
        .clone()
        .oneshot(json_request("/api/v1/boards", request.clone()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let created: CreateBoardResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(created.data.slug, "http-board");
    assert_eq!(created.data.archived_at, None);

    let response = router
        .clone()
        .oneshot(get_request("/api/v1/boards/http-board"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let shown: GetBoardResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(shown.data.id, created.data.id);

    let response = router
        .clone()
        .oneshot(json_request("/api/v1/boards", request))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
    assert_eq!(error.error.code, ApiErrorCode::InvalidInput);
    assert!(error.error.message.contains("看板 slug 已存在"));

    let response = router
        .clone()
        .oneshot(json_request(
            "/api/v1/boards/http-board/archive",
            serde_json::json!({"actor": "http-archiver"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let archived: ArchiveBoardResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(archived.data.id, created.data.id);
    assert!(archived.data.archived_at.is_some());

    let response = router
        .clone()
        .oneshot(get_request("/api/v1/boards/http-board"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
    assert_eq!(error.error.code, ApiErrorCode::NotFound);
    assert!(error.error.message.contains("看板 http-board"));

    let response = router
        .oneshot(json_request(
            "/api/v1/boards/http-board/tasks",
            serde_json::json!({
                "title": "归档后任务",
                "actor": "http-test",
                "labels": [],
                "depends_on": []
            }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
    assert_eq!(error.error.code, ApiErrorCode::InvalidTransition);
    assert!(error.error.message.contains("已归档看板不能创建任务"));
}
