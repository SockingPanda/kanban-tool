use crate::common::*;

#[tokio::test]
async fn tasks_api_creates_task_and_event_with_body_actor_priority() {
    let test = TestApp::with_actor("default-actor");
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
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
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    let valid_parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("valid parent"),
    )
    .expect("valid parent");
    let app = test.router();

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
    let test = TestApp::new();
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
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
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
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
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?status=ready").await;

    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], ready.id);
    assert!(!tasks.iter().any(|task| task["id"] == todo.id));
}

#[tokio::test]
async fn tasks_api_lists_with_repeated_status_filters() {
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
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
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
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
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
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
    let app = test.router();

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
async fn tasks_api_rejects_unbounded_limit() {
    let test = TestApp::new();
    let app = test.router();

    let (status, json) = get_json(
        app,
        &format!("/api/v1/boards/default/tasks?limit={}", usize::MAX),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("limit must be <= 1000")
    );
}

#[tokio::test]
async fn tasks_api_gets_task_by_id() {
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("get by id"),
    )
    .expect("task");
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}", task.id)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["id"], task.id);
    assert_eq!(json["data"]["title"], "get by id");
    assert_task_dto_exposes_ui_fields_without_claim_token(&json["data"]);
}

#[tokio::test]
async fn tasks_api_returns_error_envelope_for_json_and_query_extractor_errors() {
    let test = TestApp::new();
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("before update"),
    )
    .expect("task");
    let app = test.router();

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
    assert_eq!(json["data"]["status"], "triage");

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("reject forbidden"),
    )
    .expect("task");
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("reject unknown"),
    )
    .expect("task");
    let app = test.router();

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
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("schedule me"),
    )
    .expect("task");
    let app = test.router();
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
async fn task_api_accepts_retry_policy_on_create_and_patch() {
    let test = TestApp::new();
    let app = test.router();

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
