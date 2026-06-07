use axum::{
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use kanban_server::{AppState, build_desktop_router, build_router};
use serde_json::{Value, json};
use tower::ServiceExt;

fn temp_db() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("kb.db");
    kanban_sqlite::init_database(&db_path, "api-test").expect("init db");
    (dir, db_path)
}

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    request_json(app, "GET", uri, None, None).await
}

async fn post_json(app: axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    request_json(app, "POST", uri, Some(body), None).await
}

async fn patch_json(
    app: axum::Router,
    uri: &str,
    body: Value,
    actor_header: Option<&str>,
) -> (StatusCode, Value) {
    request_json(app, "PATCH", uri, Some(body), actor_header).await
}

async fn delete_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    request_json(app, "DELETE", uri, None, None).await
}

async fn request_json(
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

async fn request_raw_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: &str,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_owned()))
                .expect("request"),
        )
        .await
        .expect("response");
    response_json(response).await
}

async fn response_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = serde_json::from_slice(&body).expect("json body");
    (status, json)
}

async fn get_raw(app: axum::Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, String) {
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
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        headers,
        String::from_utf8(body.to_vec()).expect("utf8 body"),
    )
}

async fn options_raw(
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

fn assert_task_dto_exposes_ui_fields_without_claim_token(task: &Value) {
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

#[tokio::test]
async fn default_router_does_not_enable_browser_cors_for_mutations() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (_status, headers) =
        options_raw(app, "/api/v1/boards/default/tasks", "http://127.0.0.1:1420").await;

    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
}

#[tokio::test]
async fn desktop_router_allows_only_local_desktop_origins() {
    let (_dir, db_path) = temp_db();
    let app = build_desktop_router(AppState::new(db_path, "api-test"));

    let (status, headers) = options_raw(
        app.clone(),
        "/api/v1/boards/default/tasks",
        "http://127.0.0.1:1420",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&HeaderValue::from_static("http://127.0.0.1:1420"))
    );

    let (_status, headers) =
        options_raw(app, "/api/v1/boards/default/tasks", "http://example.com").await;
    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
}

fn future_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_millis() as i64
        + 86_400_000
}

fn set_task_updated_at(db_path: &std::path::Path, task_id: &str, updated_at: i64) {
    let conn = kanban_sqlite::connect_file(db_path).expect("connect db");
    let changed = conn
        .execute(
            "UPDATE tasks SET updated_at=?1 WHERE id=?2",
            (updated_at, task_id),
        )
        .expect("set updated_at");
    assert_eq!(changed, 1);
}

#[tokio::test]
async fn health_reports_ok_database_and_version() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, "/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["ok"], true);
    assert_eq!(json["data"]["db"], "ok");
    assert_eq!(json["data"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(json.get("error").is_none());
}

#[tokio::test]
async fn boards_api_lists_and_shows_default_board() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app.clone(), "/api/v1/boards").await;
    assert_eq!(status, StatusCode::OK);
    let boards = json["data"].as_array().expect("boards array");
    assert_eq!(boards.len(), 1);
    let board_id = boards[0]["id"].as_str().expect("board id");
    assert!(board_id.starts_with("b_"));
    assert_eq!(boards[0]["slug"], "default");
    assert_eq!(boards[0]["name"], "Default");
    assert_eq!(boards[0]["description"], Value::Null);
    assert!(boards[0]["created_at"].is_i64());
    assert!(boards[0]["updated_at"].is_i64());
    assert_eq!(boards[0]["archived_at"], Value::Null);

    let (status, by_slug) = get_json(app.clone(), "/api/v1/boards/default").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(by_slug["data"]["id"], board_id);
    assert_eq!(by_slug["data"]["slug"], "default");

    let by_id_uri = format!("/api/v1/boards/{board_id}");
    let (status, by_id) = get_json(app, &by_id_uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(by_id["data"]["id"], board_id);
    assert_eq!(by_id["data"]["slug"], "default");
}

#[tokio::test]
async fn board_columns_api_lists_default_columns_in_position_order() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, "/api/v1/boards/default/columns").await;

    assert_eq!(status, StatusCode::OK);
    let columns = json["data"].as_array().expect("columns array");
    let statuses: Vec<_> = columns
        .iter()
        .map(|column| column["status"].as_str().expect("status"))
        .collect();
    assert_eq!(
        statuses,
        [
            "triage",
            "todo",
            "scheduled",
            "ready",
            "running",
            "blocked",
            "review",
            "done",
            "archived"
        ]
    );
    assert_eq!(columns[0]["title"], "Triage");
    assert_eq!(columns[0]["position"], 10);
    assert_eq!(columns[0]["hidden"], false);
    assert_eq!(columns[0]["wip_limit"], Value::Null);
    assert_eq!(columns[8]["hidden"], true);
}

#[tokio::test]
async fn tasks_api_creates_task_and_event_with_body_actor_priority() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path.clone(), "default-actor"));

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({
            "title": "HTTP create",
            "description": "ready spec",
            "status": "ready",
            "assignee": "worker-a",
            "priority": 10,
            "scheduled_at": null,
            "due_at": null,
            "metadata": {"source": "test"},
            "actor": "body-actor"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(json.get("error").is_none());
    let task = &json["data"];
    assert!(task["id"].as_str().expect("task id").starts_with("t_"));
    assert_eq!(task["title"], "HTTP create");
    assert_eq!(task["description"], "ready spec");
    assert_eq!(task["status"], "ready");
    assert_eq!(task["assignee"], "worker-a");
    assert_eq!(task["priority"], 10);
    assert_task_dto_exposes_ui_fields_without_claim_token(task);
    assert_eq!(task["metadata_json"], r#"{"source":"test"}"#);

    let events =
        kanban_sqlite::list_events(&db_path, "default", Some(task["id"].as_str().unwrap()))
            .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "task.created");
    assert_eq!(events[0].actor.as_deref(), Some("body-actor"));
}

#[tokio::test]
async fn tasks_api_creates_task_with_dependencies_and_degrades_ready_to_todo() {
    let (_dir, db_path) = temp_db();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "unfinished parent".to_owned(),
            description: Some("spec".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .expect("parent");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({
            "title": "dependent child",
            "description": "ready spec",
            "status": "ready",
            "depends_on": [parent.id]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["status"], "todo");
    let child_id = json["data"]["id"].as_str().expect("child id");
    let deps = kanban_sqlite::list_dependencies(&db_path, "default", child_id).expect("deps");
    assert!(
        deps.iter()
            .any(|(parent_id, _child_id)| parent_id == &parent.id)
    );
    let events = kanban_sqlite::list_events(&db_path, "default", Some(child_id)).expect("events");
    assert!(events.iter().any(|event| event.kind == "dependency.added"));
}

#[tokio::test]
async fn tasks_api_create_with_missing_dependency_rolls_back_task() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({
            "title": "must not persist",
            "description": "ready spec",
            "status": "ready",
            "depends_on": ["t_missing_parent"]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");
    let tasks = kanban_sqlite::list_tasks(&db_path, "default", &[], false).expect("tasks");
    assert!(tasks.iter().all(|task| task.title != "must not persist"));
}

#[tokio::test]
async fn tasks_api_create_with_multiple_dependencies_rolls_back_prior_edges_on_later_failure() {
    let (_dir, db_path) = temp_db();
    let valid_parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("valid parent"),
    )
    .expect("valid parent");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({
            "title": "partial child",
            "description": "ready spec",
            "status": "ready",
            "depends_on": [valid_parent.id, "t_missing_parent"]
        }),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");
    let tasks = kanban_sqlite::list_tasks(&db_path, "default", &[], false).expect("tasks");
    let child = tasks.iter().find(|task| task.title == "partial child");
    assert!(child.is_none(), "failed create must roll back child task");
    let deps =
        kanban_sqlite::list_dependencies(&db_path, "default", &valid_parent.id).expect("deps");
    assert!(
        deps.is_empty(),
        "failed create must roll back prior dependency edge"
    );
}

#[tokio::test]
async fn tasks_api_rejects_unsupported_create_fields() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({ "title": "labels not yet supported", "labels": ["backend"] }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
}

#[tokio::test]
async fn tasks_api_lists_non_archived_by_default_and_includes_archived_on_query() {
    let (_dir, db_path) = temp_db();
    let visible = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("visible task"),
    )
    .expect("visible task");
    let archived = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("archived task"),
    )
    .expect("archived task");
    kanban_sqlite::archive_task(&db_path, "default", "seed", &archived.id, false).expect("archive");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app.clone(), "/api/v1/boards/default/tasks").await;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], visible.id);
    assert_eq!(json["meta"]["limit"], 100);
    assert_eq!(json["meta"]["offset"], 0);

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?include_archived=true").await;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().any(|task| task["id"] == archived.id));
}

#[tokio::test]
async fn tasks_api_lists_with_single_status_filter() {
    let (_dir, db_path) = temp_db();
    let ready = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("ready task"),
    )
    .expect("ready task");
    let todo = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "todo task".to_owned(),
            description: Some("todo details".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .expect("todo task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?status=ready").await;

    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], ready.id);
    assert!(!tasks.iter().any(|task| task["id"] == todo.id));
}

#[tokio::test]
async fn tasks_api_lists_with_repeated_status_filters() {
    let (_dir, db_path) = temp_db();
    let ready = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("ready task"),
    )
    .expect("ready task");
    let running = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("running task"),
    )
    .expect("running task");
    kanban_sqlite::claim_task(&db_path, "default", "seed", &running.id, 60_000)
        .expect("claim task");
    let todo = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "todo task".to_owned(),
            description: Some("todo details".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .expect("todo task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(
        app,
        "/api/v1/boards/default/tasks?status=ready&status=running",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().any(|task| task["id"] == ready.id));
    assert!(tasks.iter().any(|task| task["id"] == running.id));
    assert!(!tasks.iter().any(|task| task["id"] == todo.id));
}

#[tokio::test]
async fn tasks_api_sorts_by_updated_at_ascending_and_descending() {
    let (_dir, db_path) = temp_db();
    let oldest = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("oldest update"),
    )
    .expect("oldest task");
    let newest = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("newest update"),
    )
    .expect("newest task");
    let middle = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("middle update"),
    )
    .expect("middle task");
    set_task_updated_at(&db_path, &oldest.id, 1_000);
    set_task_updated_at(&db_path, &newest.id, 3_000);
    set_task_updated_at(&db_path, &middle.id, 2_000);
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) =
        get_json(app.clone(), "/api/v1/boards/default/tasks?sort=updated_at").await;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().expect("tasks array");
    let ids: Vec<_> = tasks.iter().map(|task| task["id"].clone()).collect();
    assert_eq!(
        ids,
        [oldest.id.clone(), middle.id.clone(), newest.id.clone()]
    );

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?sort=-updated_at").await;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().expect("tasks array");
    let ids: Vec<_> = tasks.iter().map(|task| task["id"].clone()).collect();
    assert_eq!(ids, [newest.id, middle.id, oldest.id]);
}

#[tokio::test]
async fn tasks_api_lists_with_assignee_search_sort_and_rejects_label_filter() {
    let (_dir, db_path) = temp_db();
    for (title, assignee, priority) in [
        ("alpha bug", Some("alice"), 10),
        ("beta bug", Some("alice"), 30),
        ("alpha chore", Some("bob"), 20),
    ] {
        kanban_sqlite::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::CreateTask {
                title: title.to_owned(),
                description: Some(format!("{title} details")),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: assignee.map(str::to_owned),
                priority,
                scheduled_at: None,
                due_at: None,
                metadata_json: "{}".to_owned(),
            },
        )
        .expect("seed task");
    }
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/tasks?assignee=alice&q=bug&sort=-priority",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0]["title"], "beta bug");
    assert_eq!(tasks[1]["title"], "alpha bug");

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?label=backend").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
}

#[tokio::test]
async fn tasks_api_gets_task_by_id() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("get by id"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}", task.id)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["id"], task.id);
    assert_eq!(json["data"]["title"], "get by id");
    assert_task_dto_exposes_ui_fields_without_claim_token(&json["data"]);
}

#[tokio::test]
async fn tasks_api_returns_error_envelope_for_json_and_query_extractor_errors() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    for (status, json) in [
        request_raw_json(app.clone(), "POST", "/api/v1/boards/default/tasks", "{").await,
        post_json(
            app.clone(),
            "/api/v1/boards/default/tasks",
            json!({"description":"missing title"}),
        )
        .await,
        post_json(
            app.clone(),
            "/api/v1/boards/default/tasks",
            json!({"title":"bad priority","priority":"high"}),
        )
        .await,
        get_json(app, "/api/v1/boards/default/tasks?status=bogus").await,
    ] {
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_input");
        assert!(json["error"]["message"].is_string());
    }
}

#[tokio::test]
async fn tasks_api_patches_editable_fields_and_uses_header_actor_when_body_actor_absent() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("before update"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path.clone(), "default-actor"));

    let (status, json) = patch_json(
        app.clone(),
        &format!("/api/v1/tasks/{}", task.id),
        json!({
            "title": "after update",
            "description": null,
            "assignee": "worker-b",
            "priority": 20,
            "due_at": future_epoch_ms(),
            "metadata": {"updated": true},
            "expected_lock_version": task.lock_version
        }),
        Some("header-actor"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["title"], "after update");
    assert_eq!(json["data"]["description"], Value::Null);
    assert_eq!(json["data"]["assignee"], "worker-b");
    assert_eq!(json["data"]["priority"], 20);
    assert_eq!(json["data"]["metadata_json"], r#"{"updated":true}"#);
    assert_eq!(json["data"]["status"], "ready");

    let events = kanban_sqlite::list_events(&db_path, "default", Some(&task.id)).expect("events");
    assert_eq!(events.last().expect("updated event").kind, "task.updated");
    assert_eq!(
        events.last().expect("updated event").actor.as_deref(),
        Some("header-actor")
    );

    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{}", task.id),
        json!({
            "title": "stale update",
            "expected_lock_version": task.lock_version
        }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .expect("message")
            .contains("lock_version mismatch")
    );
}

#[tokio::test]
async fn tasks_api_patch_rejects_forbidden_status_and_claim_fields() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("reject forbidden"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    for forbidden in ["status", "claim_token", "completed_at", "current_run_id"] {
        let (status, json) = patch_json(
            app.clone(),
            &format!("/api/v1/tasks/{}", task.id),
            json!({ forbidden: "bad" }),
            None,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{forbidden} must be rejected"
        );
        assert_eq!(json["error"]["code"], "invalid_input");
    }
}

#[tokio::test]
async fn tasks_api_patch_rejects_unknown_fields() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("reject unknown"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{}", task.id),
        json!({ "unexpected": true }),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
}

#[tokio::test]
async fn tasks_api_patch_future_scheduled_at_recomputes_status_to_scheduled() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("schedule me"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));
    let scheduled_at = future_epoch_ms();

    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{}", task.id),
        json!({ "scheduled_at": scheduled_at }),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["scheduled_at"], scheduled_at);
    assert_eq!(json["data"]["status"], "scheduled");
}

#[tokio::test]
async fn transitions_api_claim_returns_token_run_and_running_task() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("claim me"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"actor":"worker-a","ttl_ms":300000,"worker_profile":"profile-a"}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["task"]["status"], "running");
    assert_task_dto_exposes_ui_fields_without_claim_token(&json["data"]["task"]);
    let token = json["data"]["claim_token"].as_str().expect("claim token");
    assert!(token.starts_with("claim_"));
    let run = &json["data"]["run"];
    assert!(run["id"].as_str().expect("run id").starts_with("r_"));
    assert_eq!(run["task_id"], task.id);
    assert_eq!(run["status"], "running");
    assert_eq!(run["worker_profile"], "profile-a");
}

#[tokio::test]
async fn transitions_api_heartbeat_extends_claim_and_rejects_bad_token() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("heartbeat me"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 1_000).expect("claim");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":claim.claim_token,"ttl_ms":300000,"note":"still running"}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "running");
    assert!(
        json["data"]["last_heartbeat_at"].as_i64().unwrap()
            >= claim.task.last_heartbeat_at.unwrap()
    );
    assert!(json["data"].get("claim_token").is_none());
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":"wrong","ttl_ms":300000}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "claim_token_mismatch");
}

#[tokio::test]
async fn transitions_api_complete_moves_running_done_and_promotes_child() {
    let (_dir, db_path) = temp_db();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("parent"),
    )
    .expect("parent");
    let child = kanban_sqlite::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("child"),
        std::slice::from_ref(&parent.id),
    )
    .expect("child");
    assert_eq!(child.status, kanban_core::TaskStatus::Todo);
    let claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &parent.id, 60_000)
        .expect("claim");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/complete", parent.id),
        json!({"claim_token":claim.claim_token,"summary":"done","force":false}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");
    let child = kanban_sqlite::get_task_by_id_global(&db_path, &child.id).expect("child");
    assert_eq!(child.status, kanban_core::TaskStatus::Ready);
}

#[tokio::test]
async fn transitions_api_submit_review_moves_running_to_review_and_review_is_not_claimable() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("review me"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000).expect("claim");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/submit-review", task.id),
        json!({"claim_token":claim.claim_token,"summary":"needs review"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "review");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"ttl_ms":60000}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "invalid_transition");
}

#[tokio::test]
async fn transitions_api_block_unblock_recomputes_target_status() {
    let (_dir, db_path) = temp_db();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "parent".into(),
            description: Some("spec".into()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .expect("parent");
    let child = kanban_sqlite::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked child"),
        std::slice::from_ref(&parent.id),
    )
    .expect("child");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/block", child.id),
        json!({"reason":"waiting"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "blocked");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/unblock", child.id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "todo");
}

#[tokio::test]
async fn transitions_api_archive_hides_task_from_default_list() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("archive me"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/archive", task.id),
        json!({"force":false}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "archived");

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn dependencies_api_add_remove_list_and_cycle_error() {
    let (_dir, db_path) = temp_db();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("parent"),
    )
    .expect("parent");
    let child = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("child"),
    )
    .expect("child");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", child.id),
        json!({"parent_task_id":parent.id,"actor":"dep-actor"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["parents"][0]["id"], parent.id);
    assert_eq!(json["data"]["children"].as_array().unwrap().len(), 0);

    let (status, json) = get_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", parent.id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["children"][0]["id"], child.id);

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", parent.id),
        json!({"parent_task_id":child.id}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "dependency_cycle");

    let (status, json) = delete_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies/{}", child.id, parent.id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"]["parents"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn runs_api_lists_task_runs_without_claim_token() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("runs"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000).expect("claim");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/runs", task.id)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"][0]["id"], claim.run_id);
    assert_eq!(json["data"][0]["status"], "running");
    assert!(json["data"][0].get("claim_token").is_none());
}

#[tokio::test]
async fn comments_api_creates_and_lists_task_comments() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("commented"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({"author":"operator","body":"handoff note"}),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(json["data"]["id"].as_str().unwrap().starts_with("c_"));
    assert_eq!(json["data"]["task_id"], task.id);
    assert_eq!(json["data"]["author"], "operator");
    assert_eq!(json["data"]["body"], "handoff note");
    assert_eq!(json["data"]["kind"], "text");

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await;

    assert_eq!(status, StatusCode::OK);
    let comments = json["data"].as_array().expect("comments array");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0]["body"], "handoff note");

    let events = kanban_sqlite::list_events(&db_path, "default", Some(&task.id)).expect("events");
    assert!(
        events
            .iter()
            .any(|event| event.kind == "task.comment.created")
    );
}

#[tokio::test]
async fn run_log_api_reads_dispatch_log_content_without_claim_token() {
    let temp = tempfile::tempdir().expect("tempdir");
    let db_path = temp.path().join("kb.db");
    let log_dir = temp.path().join("logs");
    kanban_sqlite::init_database(&db_path, "api-test").expect("init db");
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("logged run"),
    )
    .expect("task");
    let result = kanban_sqlite::dispatch_once(
        &db_path,
        "default",
        kanban_sqlite::DispatchOptions {
            actor: "dispatcher".into(),
            command: "printf 'hello log\\n'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 60_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::FinishPolicy::Done,
            on_failure: kanban_sqlite::FinishPolicy::Blocked,
            log_dir,
        },
    )
    .expect("dispatch");
    assert_eq!(result.task_id.as_deref(), Some(task.id.as_str()));
    let run_id = result.run_id.expect("run id");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, &format!("/api/v1/runs/{run_id}/log")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["run_id"], run_id);
    assert_eq!(json["data"]["content"], "hello log\n");
    assert_eq!(json["data"]["truncated"], false);
    assert!(json["data"].get("claim_token").is_none());
}

#[tokio::test]
async fn task_api_accepts_retry_policy_on_create_and_patch() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/tasks",
        json!({
            "title":"retry via api",
            "description":"ready spec",
            "max_retries":2
        }),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    let task_id = json["data"]["id"].as_str().unwrap();
    assert_eq!(json["data"]["max_retries"], 2);

    let (status, json) = patch_json(
        app.clone(),
        &format!("/api/v1/tasks/{task_id}"),
        json!({"max_retries":null}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"]["max_retries"].is_null());

    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{task_id}"),
        json!({"max_retries":1}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["max_retries"], 1);
}

#[tokio::test]
async fn stats_api_reports_stale_claims_and_blocked_reason_counts() {
    let (_dir, db_path) = temp_db();
    let stale = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("stale claim"),
    )
    .expect("stale task");
    kanban_sqlite::claim_task(&db_path, "default", "worker", &stale.id, -1).expect("claim");
    let blocked_a = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked a"),
    )
    .expect("blocked a");
    let blocked_b = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocked b"),
    )
    .expect("blocked b");
    kanban_sqlite::block_task(
        &db_path,
        "default",
        "seed",
        &blocked_a.id,
        "waiting on operator",
        None,
        true,
    )
    .expect("block a");
    kanban_sqlite::block_task(
        &db_path,
        "default",
        "seed",
        &blocked_b.id,
        "waiting on operator",
        None,
        true,
    )
    .expect("block b");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, "/api/v1/stats?board=default").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["stale_claims"][0]["task_id"], stale.id);
    assert_eq!(
        json["data"]["blocked_reasons"][0]["reason"],
        "waiting on operator"
    );
    assert_eq!(json["data"]["blocked_reasons"][0]["count"], 2);
    assert!(
        json["data"]["status_counts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|count| count["status"] == "running" && count["count"] == 1)
    );
}

#[tokio::test]
async fn maintenance_api_reports_doctor_and_checkpoint_results() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(app.clone(), "/api/v1/maintenance/doctor", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["integrity_check"], "ok");
    assert_eq!(json["data"]["dependency_cycles"], 0);
    assert_eq!(json["data"]["archived_dependency_edges"], 0);
    assert_eq!(json["data"]["missing_run_logs"], 0);

    let (status, json) = post_json(app, "/api/v1/maintenance/checkpoint", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["busy"], 0);
    assert!(json["data"].get("checkpointed_frames").is_some());
}

#[tokio::test]
async fn specify_transition_recomputes_triage_to_ready_scheduled_and_todo() {
    let (_dir, db_path) = temp_db();
    let ready_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "needs spec".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .expect("ready task");
    let scheduled_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "needs schedule".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .expect("scheduled task");
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "parent".into(),
            description: Some("not done".into()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .expect("parent");
    let todo_task = kanban_sqlite::create_task_with_dependencies(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "blocked spec".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
        std::slice::from_ref(&parent.id),
    )
    .expect("todo task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/specify", ready_task.id),
        json!({"description":"ready spec"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let future = future_epoch_ms();
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/specify", scheduled_task.id),
        json!({"description":"scheduled spec","scheduled_at":future}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "scheduled");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/specify", todo_task.id),
        json!({"description":"todo spec"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "todo");
}

#[tokio::test]
async fn reclaim_transition_force_and_expired_close_active_run() {
    let (_dir, db_path) = temp_db();
    let force_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("force reclaim"),
    )
    .expect("force task");
    let expired_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("expired reclaim"),
    )
    .expect("expired task");
    let maxed_task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("max retries reclaim"),
    )
    .expect("maxed task");
    let force_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &force_task.id, 60_000)
            .expect("force claim");
    let expired_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &expired_task.id, -1)
            .expect("expired claim");
    let maxed_claim = kanban_sqlite::claim_task(&db_path, "default", "worker", &maxed_task.id, -1)
        .expect("maxed claim");
    let conn = kanban_sqlite::connect_file(&db_path).expect("connect");
    conn.execute(
        "UPDATE tasks SET retry_count=0, max_retries=1 WHERE id=?1",
        (&maxed_task.id,),
    )
    .expect("set max retries");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/reclaim", force_task.id),
        json!({"force":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/reclaim", expired_task.id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "ready");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/reclaim", maxed_task.id),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "blocked");
    assert_eq!(json["data"]["status_reason"], "max retries reached");

    let runs = kanban_sqlite::list_runs(&db_path, "default", None).expect("runs");
    let force_run = runs
        .iter()
        .find(|run| run.id == force_claim.run_id)
        .unwrap();
    let expired_run = runs
        .iter()
        .find(|run| run.id == expired_claim.run_id)
        .unwrap();
    let maxed_run = runs
        .iter()
        .find(|run| run.id == maxed_claim.run_id)
        .unwrap();
    assert_eq!(force_run.status, "canceled");
    assert_eq!(expired_run.status, "expired");
    assert_eq!(maxed_run.status, "expired");
}

#[tokio::test]
async fn claim_uses_requested_worker_profile_in_response_run() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("profile claim"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"worker_profile":"reviewer"}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["run"]["worker_profile"], "reviewer");
}

#[tokio::test]
async fn complete_with_summary_stores_task_run_summary_and_result() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("complete summary"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000).expect("claim");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/complete", task.id),
        json!({"claim_token":claim.claim_token,"summary":"done summary"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");

    let stored = kanban_sqlite::get_task_by_id_global(&db_path, &task.id).expect("task");
    let runs = kanban_sqlite::list_runs(&db_path, "default", Some(&task.id)).expect("runs");
    assert_eq!(stored.result_summary.as_deref(), Some("done summary"));
    assert_eq!(runs[0].summary.as_deref(), Some("done summary"));

    let other = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("reject result"),
    )
    .expect("other");
    let other_claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &other.id, 60_000).expect("claim");
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/complete", other.id),
        json!({"claim_token":other_claim.claim_token,"result":{"ok":true}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "done");
    let stored = kanban_sqlite::get_task_by_id_global(&db_path, &other.id).expect("task");
    assert_eq!(stored.result_json.as_deref(), Some(r#"{"ok":true}"#));
}

#[tokio::test]
async fn submit_review_with_summary_stores_run_summary() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("review summary"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000).expect("claim");
    let app = build_router(AppState::new(db_path.clone(), "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/submit-review", task.id),
        json!({"claim_token":claim.claim_token,"summary":"ready for review"}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "review");
    let runs = kanban_sqlite::list_runs(&db_path, "default", Some(&task.id)).expect("runs");
    assert_eq!(runs[0].summary.as_deref(), Some("ready for review"));
}

#[tokio::test]
async fn runs_api_gets_run_by_id_without_claim_token() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("get run"),
    )
    .expect("task");
    let claim =
        kanban_sqlite::claim_task(&db_path, "default", "worker", &task.id, 60_000).expect("claim");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, &format!("/api/v1/runs/{}", claim.run_id)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["id"], claim.run_id);
    assert_eq!(json["data"]["task_id"], task.id);
    assert!(json["data"].get("claim_token").is_none());
}

#[tokio::test]
async fn heartbeat_non_running_wrong_token_returns_invalid_transition_not_token_mismatch() {
    let (_dir, db_path) = temp_db();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("heartbeat not running"),
    )
    .expect("task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/heartbeat", task.id),
        json!({"claim_token":"wrong"}),
    )
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "invalid_transition");
}

#[tokio::test]
async fn events_api_after_limit_returns_ordered_events_and_next_after() {
    let (_dir, db_path) = temp_db();
    let first = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("first"),
    )
    .expect("first");
    let second = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("second"),
    )
    .expect("second");
    let all = kanban_sqlite::list_events(&db_path, "default", None).expect("events");
    let after = all
        .iter()
        .find(|event| event.task_id.as_deref() == Some(&first.id))
        .expect("first task event")
        .id;
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(
        app,
        &format!("/api/v1/events?board=default&after={after}&limit=1"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let events = json["data"].as_array().expect("events array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["task_id"], second.id);
    assert!(events[0]["id"].as_i64().unwrap() > after);
    assert_eq!(json["meta"]["next_after"], events[0]["id"]);
    assert!(events[0]["event_id"].as_str().unwrap().starts_with("e_"));
    assert_eq!(events[0]["kind"], "task.created");
    assert_ne!(first.id, second.id);
}

#[tokio::test]
async fn events_api_filters_by_task_id_for_detail_timeline() {
    let (_dir, db_path) = temp_db();
    let first = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("first timeline"),
    )
    .expect("first");
    let second = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("second timeline"),
    )
    .expect("second");
    kanban_sqlite::block_task(
        &db_path,
        "default",
        "seed",
        &first.id,
        "waiting on local input",
        None,
        false,
    )
    .expect("block first");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(
        app,
        &format!("/api/v1/events?board=default&task_id={}", first.id),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let events = json["data"].as_array().expect("events array");
    assert!(events.len() >= 2);
    assert!(
        events
            .iter()
            .all(|event| event["task_id"].as_str() == Some(first.id.as_str()))
    );
    assert!(!events.iter().any(|event| event["task_id"] == second.id));
    assert!(events.iter().any(|event| event["kind"] == "task.blocked"));
}

#[tokio::test]
async fn stream_events_sse_returns_finite_snapshot_with_id_event_and_data_frames() {
    let (_dir, db_path) = temp_db();
    let first = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("first sse"),
    )
    .expect("first");
    let second = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("second sse"),
    )
    .expect("second");
    let all = kanban_sqlite::list_events(&db_path, "default", None).expect("events");
    let after = all
        .iter()
        .find(|event| event.task_id.as_deref() == Some(&first.id))
        .expect("first task event")
        .id;
    let second_event = all
        .iter()
        .find(|event| event.task_id.as_deref() == Some(&second.id))
        .expect("second task event");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, headers, body) = get_raw(
        app,
        &format!("/api/v1/stream/events?board=default&after={after}&limit=1"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    assert!(body.contains(&format!("id: {}", second_event.id)), "{body}");
    assert!(body.contains("event: task.created"), "{body}");
    assert!(body.contains("data: "), "{body}");
    assert!(
        body.contains(&format!(r#""id":{}"#, second_event.id)),
        "{body}"
    );
    assert!(
        body.contains(&format!(r#""task_id":"{}""#, second.id)),
        "{body}"
    );
    assert!(
        !body.contains(&first.id),
        "after must exclude the first task event: {body}"
    );
}
