pub use axum::{
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
};
pub use http_body_util::BodyExt;
pub use kanban_server::{AppState, build_desktop_router, build_router};
pub use serde_json::{Value, json};
pub use tower::ServiceExt;

pub struct TestApp {
    _dir: tempfile::TempDir,
    db_path: std::path::PathBuf,
    default_actor: String,
}

impl TestApp {
    pub fn new() -> Self {
        Self::with_actor("api-test")
    }

    pub fn with_actor(default_actor: &str) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("kb.db");
        kanban_sqlite::init_database(&db_path, "api-test").expect("init db");
        Self {
            _dir: dir,
            db_path,
            default_actor: default_actor.to_owned(),
        }
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    pub fn dir_path(&self) -> &std::path::Path {
        self._dir.path()
    }

    pub fn router(&self) -> axum::Router {
        build_router(AppState::new(&self.db_path, self.default_actor.clone()))
    }

    pub fn desktop_router(&self) -> axum::Router {
        build_desktop_router(AppState::new(&self.db_path, self.default_actor.clone()))
    }
}

pub async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    request_json(app, "GET", uri, None, None).await
}

pub async fn post_json(app: axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    request_json(app, "POST", uri, Some(body), None).await
}

pub async fn patch_json(
    app: axum::Router,
    uri: &str,
    body: Value,
    actor_header: Option<&str>,
) -> (StatusCode, Value) {
    request_json(app, "PATCH", uri, Some(body), actor_header).await
}

pub async fn delete_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    request_json(app, "DELETE", uri, None, None).await
}

pub async fn request_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    actor_header: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("Content-Type", "application/json");
    }
    if let Some(actor) = actor_header {
        builder = builder.header("X-KB-Actor", actor);
    }
    let body = body
        .map(|value| Body::from(value.to_string()))
        .unwrap_or_else(Body::empty);
    let response = app
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("response");
    response_json(response).await
}

pub async fn request_raw_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    raw_body: &str,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("Content-Type", "application/json")
                .body(Body::from(raw_body.to_owned()))
                .expect("request"),
        )
        .await
        .expect("response");
    response_json(response).await
}

pub async fn response_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let value = serde_json::from_slice(&body).expect("json body");
    (status, value)
}

pub async fn get_raw(app: axum::Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        String::from_utf8(body.to_vec()).expect("utf8 body"),
    )
}

pub async fn options_raw(
    app: axum::Router,
    uri: &str,
    origin: &str,
) -> (StatusCode, axum::http::HeaderMap) {
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri(uri)
                .header(header::ORIGIN, origin)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    "content-type,x-kb-actor",
                )
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    (response.status(), response.headers().clone())
}

pub fn assert_task_dto_exposes_ui_fields_without_claim_token(task: &Value) {
    assert!(
        task.get("claim_token").is_none(),
        "claim_token must not be exposed"
    );
    for exposed in [
        "claim_owner",
        "claim_expires_at",
        "current_run_id",
        "completed_at",
        "archived_at",
        "retry_count",
        "max_retries",
        "result_summary",
    ] {
        assert!(task.get(exposed).is_some(), "{exposed} must be exposed");
    }
}

pub fn future_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_millis() as i64
        + 86_400_000
}

pub fn set_task_updated_at(db_path: &std::path::Path, task_id: &str, updated_at: i64) {
    let conn = kanban_sqlite::connect_file(db_path).expect("connect db");
    let changed = conn
        .execute(
            "UPDATE tasks SET updated_at=?1 WHERE id=?2",
            (updated_at, task_id),
        )
        .expect("set updated_at");
    assert_eq!(changed, 1);
}
