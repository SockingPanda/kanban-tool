pub use anyhow::{Context, Result};
pub use axum::{
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
};
pub use http_body_util::BodyExt;
pub use kanban_server::{AppState, build_desktop_router, build_router, build_serve_router};
pub use serde_json::{Value, json};
pub use tower::ServiceExt;

pub struct TestApp {
    _dir: tempfile::TempDir,
    db_path: std::path::PathBuf,
    vector_config_path: std::path::PathBuf,
    default_actor: String,
}

impl TestApp {
    pub fn new() -> Result<Self> {
        Self::with_actor("api-test")
    }

    pub fn with_actor(default_actor: &str) -> Result<Self> {
        let dir = tempfile::tempdir().context("tempdir")?;
        let db_path = dir.path().join("kb.db");
        let vector_config_path = dir.path().join("config.toml");
        kanban_local::write_project_config(
            &vector_config_path,
            &kanban_local::ProjectConfig::default(),
        )
        .context("write empty vector config")?;
        kanban_sqlite::init_database(&db_path, default_actor).context("init db")?;
        Ok(Self {
            _dir: dir,
            db_path,
            vector_config_path,
            default_actor: default_actor.to_owned(),
        })
    }

    pub fn db_path(&self) -> &std::path::Path {
        &self.db_path
    }

    pub fn dir_path(&self) -> &std::path::Path {
        self._dir.path()
    }

    pub fn router(&self) -> axum::Router {
        build_router(
            AppState::new(&self.db_path, self.default_actor.clone())
                .with_vector_config_path(&self.vector_config_path),
        )
    }

    pub fn desktop_router(&self) -> axum::Router {
        build_desktop_router(
            AppState::new(&self.db_path, self.default_actor.clone())
                .with_vector_config_path(&self.vector_config_path),
        )
    }

    pub fn serve_router(&self) -> axum::Router {
        build_serve_router(
            AppState::new(&self.db_path, self.default_actor.clone())
                .with_vector_config_path(&self.vector_config_path),
        )
    }
}

pub async fn get_json(app: axum::Router, uri: &str) -> Result<(StatusCode, Value)> {
    request_json(app, "GET", uri, None, None).await
}

pub async fn get_json_with_accept_language(
    app: axum::Router,
    uri: &str,
    accept_language: &str,
) -> Result<(StatusCode, Value)> {
    request_json_with_accept_language(app, "GET", uri, None, accept_language).await
}

pub async fn post_json(app: axum::Router, uri: &str, body: Value) -> Result<(StatusCode, Value)> {
    request_json(app, "POST", uri, Some(body), None).await
}

pub async fn patch_json(
    app: axum::Router,
    uri: &str,
    body: Value,
    actor_header: Option<&str>,
) -> Result<(StatusCode, Value)> {
    request_json(app, "PATCH", uri, Some(body), actor_header).await
}

pub async fn delete_json(app: axum::Router, uri: &str) -> Result<(StatusCode, Value)> {
    request_json(app, "DELETE", uri, None, None).await
}

pub async fn request_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    actor_header: Option<&str>,
) -> Result<(StatusCode, Value)> {
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
        .oneshot(builder.body(body).context("request")?)
        .await
        .context("response")?;
    response_json(response).await
}

pub async fn request_json_with_accept_language(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    accept_language: &str,
) -> Result<(StatusCode, Value)> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::ACCEPT_LANGUAGE, accept_language);
    if body.is_some() {
        builder = builder.header("Content-Type", "application/json");
    }
    let body = body
        .map(|value| Body::from(value.to_string()))
        .unwrap_or_else(Body::empty);
    let response = app
        .oneshot(builder.body(body).context("request")?)
        .await
        .context("response")?;
    response_json(response).await
}

pub async fn request_raw_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    raw_body: &str,
) -> Result<(StatusCode, Value)> {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("Content-Type", "application/json")
                .body(Body::from(raw_body.to_owned()))
                .context("request")?,
        )
        .await
        .context("response")?;
    response_json(response).await
}

pub async fn response_json(response: axum::response::Response) -> Result<(StatusCode, Value)> {
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .context("body")?
        .to_bytes();
    let value = serde_json::from_slice(&body).context("json body")?;
    Ok((status, value))
}

pub async fn get_raw(
    app: axum::Router,
    uri: &str,
) -> Result<(StatusCode, axum::http::HeaderMap, String)> {
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .context("request")?,
        )
        .await
        .context("response")?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .context("body")?
        .to_bytes();
    Ok((
        status,
        headers,
        String::from_utf8(body.to_vec()).context("utf8 body")?,
    ))
}

pub async fn options_raw(
    app: axum::Router,
    uri: &str,
    origin: &str,
) -> Result<(StatusCode, axum::http::HeaderMap)> {
    options_raw_for_method(app, uri, origin, "POST").await
}

pub async fn options_raw_for_method(
    app: axum::Router,
    uri: &str,
    origin: &str,
    request_method: &str,
) -> Result<(StatusCode, axum::http::HeaderMap)> {
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri(uri)
                .header(header::ORIGIN, origin)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, request_method)
                .header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    "content-type,x-kb-actor",
                )
                .body(Body::empty())
                .context("request")?,
        )
        .await
        .context("response")?;
    Ok((response.status(), response.headers().clone()))
}

pub fn create_ready_task_for_test(
    path: &std::path::Path,
    board: &str,
    actor: &str,
    title: &str,
) -> Result<kanban_sqlite::TaskRecord> {
    let task =
        kanban_sqlite::create_task(path, board, actor, kanban_sqlite::CreateTask::ready(title))?;
    mark_plan_not_required_for_test(path, board, actor, &task.id)
}

pub fn mark_plan_not_required_for_test(
    path: &std::path::Path,
    board: &str,
    actor: &str,
    task_id: &str,
) -> Result<kanban_sqlite::TaskRecord> {
    kanban_sqlite::mark_execution_plan_not_required(
        path,
        board,
        actor,
        task_id,
        "test fixture does not require steps",
    )?;
    kanban_sqlite::get_task(path, board, task_id).map_err(Into::into)
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
        "dependency_blocked",
        "unfinished_parent_count",
        "execution_plan_state",
        "required_step_count",
        "completed_required_step_count",
        "optional_step_count",
    ] {
        assert!(task.get(exposed).is_some(), "{exposed} must be exposed");
    }
}

pub fn future_epoch_ms() -> Result<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system time")
        .map(|duration| duration.as_millis() as i64 + 86_400_000)
}

pub fn set_task_updated_at(
    db_path: &std::path::Path,
    task_id: &str,
    updated_at: i64,
) -> Result<()> {
    let conn = kanban_sqlite::connect_file(db_path).context("connect db")?;
    let changed = conn
        .execute(
            "UPDATE tasks SET updated_at=?1 WHERE id=?2",
            (updated_at, task_id),
        )
        .context("set updated_at")?;
    assert_eq!(changed, 1);
    Ok(())
}
