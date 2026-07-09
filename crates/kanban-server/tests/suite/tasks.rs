use crate::common::*;
use kanban_sqlite::api::LabelProposalCandidate;

#[derive(Debug, PartialEq, Eq)]
struct HttpTaskCreateLabelCounts {
    tasks: i64,
    labels: i64,
    task_labels: i64,
    task_events: i64,
}

fn http_task_create_label_counts(
    path: &std::path::Path,
) -> anyhow::Result<HttpTaskCreateLabelCounts> {
    let conn = kanban_test_support::connect_file(path)?;
    let count_rows = |table: &str| -> anyhow::Result<i64> {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .context("count rows")
    };
    Ok(HttpTaskCreateLabelCounts {
        tasks: count_rows("tasks")?,
        labels: count_rows("labels")?,
        task_labels: count_rows("task_labels")?,
        task_events: count_rows("task_events")?,
    })
}

#[tokio::test]
async fn tasks_by_status_returns_per_status_windows() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    create_ready_task_for_test(&db_path, "default", "seed", "ready alpha")
        .context("seed ready alpha")?;
    create_ready_task_for_test(&db_path, "default", "seed", "ready beta")
        .context("seed ready beta")?;
    kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "todo gamma".to_owned(),
            description: Some("ready spec".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .context("seed blocked gamma")?;
    let app = test.router();

    let (status, json) = get_json(
        app,
        "/api/v1/boards/default/tasks/by-status?status=ready&status=todo&limit=1",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["meta"]["limit"], 1);
    assert_eq!(json["meta"]["offset"], 0);
    let windows = json["data"]["statuses"]
        .as_array()
        .context("status windows")?;
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0]["status"], "ready");
    assert_eq!(
        windows[0]["tasks"].as_array().context("ready tasks")?.len(),
        1
    );
    assert_eq!(
        windows[0]["page"],
        json!({"limit": 1, "offset": 0, "total": 2})
    );
    assert_eq!(windows[1]["status"], "todo");
    assert_eq!(
        windows[1]["tasks"].as_array().context("todo tasks")?.len(),
        1
    );
    assert_eq!(
        windows[1]["page"],
        json!({"limit": 1, "offset": 0, "total": 1})
    );
    Ok(())
}

#[tokio::test]
async fn task_list_error_message_uses_accept_language_without_changing_code() -> anyhow::Result<()>
{
    let test = TestApp::new()?;

    let (zh_status, zh_json) = get_json_with_accept_language(
        test.router(),
        "/api/v1/boards/default/tasks?limit=1001",
        "zh-CN,zh;q=0.9",
    )
    .await?;
    assert_eq!(zh_status, StatusCode::BAD_REQUEST);
    assert_eq!(zh_json["error"]["code"], "invalid_input");
    assert_eq!(
        zh_json["error"]["message"],
        "输入无效：limit 必须小于等于 1000"
    );

    let (en_status, en_json) = get_json_with_accept_language(
        test.router(),
        "/api/v1/boards/default/tasks?limit=1001",
        "en-US,en;q=0.8",
    )
    .await?;
    assert_eq!(en_status, StatusCode::BAD_REQUEST);
    assert_eq!(en_json["error"]["code"], "invalid_input");
    assert_eq!(
        en_json["error"]["message"],
        "invalid input: limit must be <= 1000"
    );
    Ok(())
}

#[tokio::test]
async fn tasks_creates_task_and_event_with_body_actor_priority() -> anyhow::Result<()> {
    let test = TestApp::with_actor("default-actor")?;
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
            "priority": 1,
            "scheduled_at": null,
            "due_at": null,
            "metadata": {"source": "test"},
            "actor": "body-actor"
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert!(json.get("error").is_none());
    let task = &json["data"];
    assert!(task["id"].as_str().context("task id")?.starts_with("t_"));
    assert_eq!(task["title"], "HTTP create");
    assert_eq!(task["description"], "ready spec");
    assert_eq!(task["status"], "todo");
    assert_eq!(task["execution_plan_state"], "unplanned");
    assert_eq!(task["assignee"], "worker-a");
    assert_eq!(task["priority"], 1);
    assert_task_dto_exposes_ui_fields_without_claim_token(task);
    assert_eq!(task["metadata_json"], r#"{"source":"test"}"#);

    let events = kanban_sqlite::api::list_events(
        &db_path,
        "default",
        Some(task["id"].as_str().context("value")?),
    )
    .context("events")?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "task.created");
    assert_eq!(events[0].actor.as_deref(), Some("body-actor"));
    Ok(())
}

#[tokio::test]
async fn tasks_creates_task_with_dependencies_and_degrades_ready_to_todo() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let parent = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "unfinished parent".to_owned(),
            description: Some("spec".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .context("parent")?;
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
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["status"], "todo");
    let child_id = json["data"]["id"].as_str().context("child id")?;
    let deps =
        kanban_sqlite::api::list_dependencies(&db_path, "default", child_id).context("deps")?;
    assert!(
        deps.iter()
            .any(|(parent_id, _child_id)| parent_id == &parent.id)
    );
    let events =
        kanban_sqlite::api::list_events(&db_path, "default", Some(child_id)).context("events")?;
    assert!(events.iter().any(|event| event.kind == "dependency.added"));
    Ok(())
}

#[tokio::test]
async fn tasks_create_with_missing_dependency_rolls_back_task() -> anyhow::Result<()> {
    let test = TestApp::new()?;
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
    .await?;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");
    let tasks = kanban_sqlite::api::list_tasks(&db_path, "default", &[], false).context("tasks")?;
    assert!(tasks.iter().all(|task| task.title != "must not persist"));
    Ok(())
}

#[tokio::test]
async fn tasks_create_with_invalid_max_retries_rolls_back_task_and_events() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let before_events = kanban_sqlite::api::list_events(&db_path, "default", None)?.len();
    let app = test.router();

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({
            "title": "invalid retry create",
            "description": "ready spec",
            "max_retries": 0
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    let tasks = kanban_sqlite::api::list_tasks(&db_path, "default", &[], false).context("tasks")?;
    assert!(
        tasks
            .iter()
            .all(|task| task.title != "invalid retry create"),
        "invalid create must not persist task"
    );
    let after_events = kanban_sqlite::api::list_events(&db_path, "default", None)?.len();
    assert_eq!(
        after_events, before_events,
        "invalid create must not write events"
    );
    Ok(())
}

#[tokio::test]
async fn tasks_create_with_multiple_dependencies_rolls_back_prior_edges_on_later_failure()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let valid_parent = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("valid parent"),
    )
    .context("valid parent")?;
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
    .await?;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");
    let tasks = kanban_sqlite::api::list_tasks(&db_path, "default", &[], false).context("tasks")?;
    let child = tasks.iter().find(|task| task.title == "partial child");
    assert!(child.is_none(), "failed create must roll back child task");
    let deps = kanban_sqlite::api::list_dependencies(&db_path, "default", &valid_parent.id)
        .context("deps")?;
    assert!(
        deps.is_empty(),
        "failed create must roll back prior dependency edge"
    );
    Ok(())
}

#[tokio::test]
async fn tasks_create_with_missing_label_returns_invalid_input_without_writes() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let before = http_task_create_label_counts(&db_path)?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({
            "title": "missing label create",
            "description": "ready spec",
            "labels": ["missing"]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("label missing does not exist")
    );
    assert_eq!(http_task_create_label_counts(&db_path)?, before);
    Ok(())
}

#[tokio::test]
async fn tasks_create_with_mixed_existing_and_missing_labels_rolls_back_atomically()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let backend = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let before = http_task_create_label_counts(&db_path)?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({
            "title": "partial label create",
            "description": "ready spec",
            "labels": ["backend", "missing"]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("label missing does not exist")
    );
    assert_eq!(http_task_create_label_counts(&db_path)?, before);
    assert_eq!(
        kanban_sqlite::api::list_labels(&db_path, "default")?
            .into_iter()
            .map(|label| label.id)
            .collect::<Vec<_>>(),
        [backend.id]
    );
    Ok(())
}

#[tokio::test]
async fn tasks_create_accepts_labels_and_exposes_task_label_dto() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    for name in ["backend", "api"] {
        kanban_sqlite::api::create_label(
            &db_path,
            "default",
            kanban_sqlite::api::CreateLabel {
                name: name.to_owned(),
                color: None,
            },
        )?;
    }
    let app = test.router();

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/tasks",
        json!({
            "title": "labels are supported",
            "description": "ready spec",
            "labels": ["backend", "api"]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    let labels = json["data"]["labels"].as_array().context("labels array")?;
    assert_eq!(labels.len(), 2);
    let names: Vec<_> = labels.iter().map(|label| label["name"].clone()).collect();
    assert_eq!(names, [json!("api"), json!("backend")]);
    assert_eq!(
        kanban_sqlite::api::list_labels(&db_path, "default")?.len(),
        2
    );
    let events = kanban_sqlite::api::list_events(
        &db_path,
        "default",
        Some(json["data"]["id"].as_str().context("task id")?),
    )?;
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == "task.label.added")
            .count(),
        2
    );
    Ok(())
}

#[tokio::test]
async fn tasks_label_bootstrap_returns_task_and_semantics() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("bootstrap API task"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels/bootstrap", task.id),
        json!({
            "name": "database",
            "description": "Database persistence work",
            "applies_when": ["touches SQLite migrations"],
            "positive_examples": ["new table migration"],
            "actor": "api-body"
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["task"]["id"], task.id);
    assert_eq!(json["data"]["task"]["labels"][0]["name"], "database");
    assert_eq!(json["data"]["semantics"]["label_name"], "database");
    assert_eq!(
        json["data"]["semantics"]["description"],
        "Database persistence work"
    );
    assert!(
        json["data"]["semantics"]["atoms"]
            .as_array()
            .context("atoms")?
            .iter()
            .any(|atom| atom["kind"] == "applies_when")
    );
    assert_task_dto_exposes_ui_fields_without_claim_token(&json["data"]["task"]);

    let (status, labels) = get_json(app, &format!("/api/v1/tasks/{}/labels", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(labels["data"][0]["name"], "database");
    Ok(())
}

#[tokio::test]
async fn tasks_lists_non_archived_by_default_and_includes_archived_on_query() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let visible = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("visible task"),
    )
    .context("visible task")?;
    let archived = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("archived task"),
    )
    .context("archived task")?;
    kanban_sqlite::api::archive_task(&db_path, "default", "seed", &archived.id, false)
        .context("archive")?;
    let app = test.router();

    let (status, json) = get_json(app.clone(), "/api/v1/boards/default/tasks").await?;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], visible.id);
    assert_eq!(json["meta"]["limit"], 100);
    assert_eq!(json["meta"]["offset"], 0);

    let (status, json) =
        get_json(app, "/api/v1/boards/default/tasks?include_archived=true").await?;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().any(|task| task["id"] == archived.id));
    Ok(())
}

#[tokio::test]
async fn tasks_lists_with_single_status_filter() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let ready = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ready task"),
    )
    .context("ready task")?;
    kanban_sqlite::api::mark_execution_plan_not_required(
        &db_path,
        "default",
        "seed",
        &ready.id,
        "status filter fixture",
    )
    .context("mark ready not required")?;
    let todo = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "todo task".to_owned(),
            description: Some("todo details".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .context("todo task")?;
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?status=ready").await?;

    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], ready.id);
    assert!(!tasks.iter().any(|task| task["id"] == todo.id));
    Ok(())
}

#[tokio::test]
async fn tasks_lists_with_repeated_status_filters() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let ready = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ready task"),
    )
    .context("ready task")?;
    kanban_sqlite::api::mark_execution_plan_not_required(
        &db_path,
        "default",
        "seed",
        &ready.id,
        "status filter fixture",
    )
    .context("mark ready not required")?;
    let running = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("running task"),
    )
    .context("running task")?;
    kanban_sqlite::api::mark_execution_plan_not_required(
        &db_path,
        "default",
        "seed",
        &running.id,
        "status filter fixture",
    )
    .context("mark running not required")?;
    kanban_sqlite::api::claim_task(&db_path, "default", "seed", &running.id, 60_000)
        .context("claim task")?;
    let todo = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "todo task".to_owned(),
            description: Some("todo details".to_owned()),
            status: Some(kanban_core::TaskStatus::Todo),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .context("todo task")?;
    let app = test.router();

    let (status, json) = get_json(
        app,
        "/api/v1/boards/default/tasks?status=ready&status=running",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().any(|task| task["id"] == ready.id));
    assert!(tasks.iter().any(|task| task["id"] == running.id));
    assert!(!tasks.iter().any(|task| task["id"] == todo.id));
    Ok(())
}

#[tokio::test]
async fn tasks_sorts_by_updated_at_ascending_and_descending() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let oldest = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("oldest update"),
    )
    .context("oldest task")?;
    let newest = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("newest update"),
    )
    .context("newest task")?;
    let middle = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("middle update"),
    )
    .context("middle task")?;
    set_task_updated_at(&db_path, &oldest.id, 1_000)?;
    set_task_updated_at(&db_path, &newest.id, 3_000)?;
    set_task_updated_at(&db_path, &middle.id, 2_000)?;
    let app = test.router();

    let (status, json) =
        get_json(app.clone(), "/api/v1/boards/default/tasks?sort=updated_at").await?;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    let ids: Vec<_> = tasks.iter().map(|task| task["id"].clone()).collect();
    assert_eq!(
        ids,
        [oldest.id.clone(), middle.id.clone(), newest.id.clone()]
    );

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?sort=-updated_at").await?;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    let ids: Vec<_> = tasks.iter().map(|task| task["id"].clone()).collect();
    assert_eq!(ids, [newest.id, middle.id, oldest.id]);
    Ok(())
}

#[tokio::test]
async fn tasks_lists_with_assignee_search_sort_and_label_filter() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    for name in ["backend", "frontend"] {
        kanban_sqlite::api::create_label(
            &db_path,
            "default",
            kanban_sqlite::api::CreateLabel {
                name: name.to_owned(),
                color: None,
            },
        )?;
    }
    for (title, assignee, priority, labels) in [
        ("alpha bug", Some("alice"), 1, vec!["backend".to_owned()]),
        ("beta bug", Some("alice"), 3, vec!["backend".to_owned()]),
        ("alpha chore", Some("bob"), 2, vec!["frontend".to_owned()]),
    ] {
        kanban_sqlite::api::create_task_with_labels(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::api::CreateTask {
                title: title.to_owned(),
                description: Some(format!("{title} details")),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: assignee.map(str::to_owned),
                priority,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".to_owned(),
            },
            &labels,
        )
        .context("seed task")?;
    }
    let app = test.router();

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/tasks?assignee=alice&q=bug&sort=-priority",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0]["title"], "beta bug");
    assert_eq!(tasks[1]["title"], "alpha bug");
    assert_eq!(tasks[0]["labels"][0]["name"], "backend");

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?label=backend").await?;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("label tasks array")?;
    assert_eq!(tasks.len(), 2);
    assert!(
        tasks
            .iter()
            .all(|task| task["labels"][0]["name"] == "backend")
    );
    Ok(())
}

#[tokio::test]
async fn labels_routes_create_list_add_and_remove_task_labels() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("label route target"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/labels",
        json!({ "name": "backend", "color": "#225577" }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let label_id = json["data"]["id"].as_str().context("label id")?.to_owned();
    assert_eq!(json["data"]["name"], "backend");

    let (status, json) = get_json(app.clone(), "/api/v1/boards/default/labels").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"].as_array().context("board labels")?.len(), 1);

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", task.id),
        json!({ "name": "backend", "actor": "api-labeler" }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["labels"][0]["id"], label_id);

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", task.id),
        json!({ "name": "frontend", "actor": "api-labeler" }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("label frontend does not exist")
    );
    assert_eq!(
        kanban_sqlite::api::list_labels(&db_path, "default")?.len(),
        1
    );

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", task.id),
        json!({
            "names": ["frontend", "api", "frontend"],
            "create_missing": true,
            "actor": "api-labeler"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let created_label_names: Vec<_> = json["meta"]["created_labels"]
        .as_array()
        .context("created labels")?
        .iter()
        .map(|label| label["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(created_label_names, ["frontend", "api"]);
    let label_names: Vec<_> = json["data"]["labels"]
        .as_array()
        .context("task labels")?
        .iter()
        .map(|label| label["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(label_names, ["api", "backend", "frontend"]);

    let (status, json) =
        get_json(app.clone(), &format!("/api/v1/tasks/{}/labels", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"].as_array().context("task labels")?.len(), 3);

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", task.id),
        json!({ "name": "backend", "names": ["frontend"] }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", task.id),
        json!({ "names": [] }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");

    let (status, json) =
        delete_json(app, &format!("/api/v1/tasks/{}/labels/{label_id}", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    let remaining_label_names: Vec<_> = json["data"]["labels"]
        .as_array()
        .context("task labels")?
        .iter()
        .map(|label| label["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(remaining_label_names, ["api", "frontend"]);
    Ok(())
}

#[tokio::test]
async fn task_label_suggestions_route_returns_degraded_json_without_provider() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("label suggestion route target"),
    )?;
    let app = test.router();

    let (status, json) = get_json(
        app,
        &format!(
            "/api/v1/tasks/{}/labels/suggestions?limit=3&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15",
            task.id
        ),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["task_id"], task.id);
    assert_eq!(json["data"]["degraded"], true);
    assert_eq!(json["data"]["needs_new_label"], false);
    assert_eq!(
        json["data"]["reason_codes"],
        json!(["degraded_result", "vector_store_disabled"])
    );
    assert!(
        json["data"]["selected_labels"]
            .as_array()
            .context("selected labels")?
            .is_empty()
    );
    assert!(
        json["data"]["diagnostics"]
            .as_array()
            .context("diagnostics")?
            .iter()
            .any(|value| value == "vector_store_disabled")
    );
    Ok(())
}

#[tokio::test]
async fn task_label_proposal_route_degrades_without_provider() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("label proposal route degraded target"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!(
            "/api/v1/tasks/{}/label-proposals?limit=3&candidate_limit=32&atom_limit=80&max_selected_labels=4&min_score=0.15",
            task.id
        ),
        json!({}),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["proposal"], serde_json::Value::Null);
    assert_eq!(json["data"]["degraded"], true);
    assert!(
        json["data"]["diagnostics"]
            .as_array()
            .context("diagnostics")?
            .iter()
            .any(|value| value == "label_proposal_provider_unavailable")
    );
    assert!(kanban_sqlite::api::list_labels(&db_path, "default")?.is_empty());
    Ok(())
}

#[tokio::test]
async fn task_label_proposal_route_with_candidate_degrades_without_polluting_truth()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("label proposal route candidate degraded target"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/label-proposals", task.id),
        json!({
            "proposal": {
                "name": "workflow",
                "description": "Workflow classification",
                "applies_when": ["classifies execution flow"],
                "excludes_when": ["UI-only polish"],
                "positive_examples": ["triage work queue"],
                "negative_examples": ["CSS tweak"]
            },
            "actor": "api-test-proposer"
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["proposal"], serde_json::Value::Null);
    assert_eq!(json["data"]["degraded"], true);
    let diagnostics = json["data"]["diagnostics"]
        .as_array()
        .context("diagnostics")?;
    assert!(
        diagnostics
            .iter()
            .any(|value| value == "label_proposal_residual_validation_unavailable"),
        "{diagnostics:?}"
    );
    let conn = kanban_test_support::connect_file(&db_path)?;
    let proposal_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM label_semantic_proposals", [], |row| {
            row.get(0)
        })?;
    assert_eq!(proposal_count, 0);
    assert!(kanban_sqlite::api::list_labels(&db_path, "default")?.is_empty());
    Ok(())
}

#[tokio::test]
async fn task_label_proposal_route_accepts_and_rejects_without_task_binding() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("label proposal route target"),
    )?;
    let app = test.router();

    let proposal_id = seed_proposed_label_proposal(
        &db_path,
        &task.id,
        LabelProposalCandidate {
            name: "database".to_owned(),
            description: Some("Database persistence work".to_owned()),
            applies_when: vec!["touches SQLite migrations".to_owned()],
            excludes_when: vec!["UI-only".to_owned()],
            positive_examples: vec!["new table".to_owned()],
            negative_examples: vec!["CSS".to_owned()],
        },
    )?;

    let (status, json) = get_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/label-proposals", task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"].as_array().context("proposals")?.len(), 1);

    let (status, json) = get_json(
        app.clone(),
        &format!("/api/v1/label-proposals/{proposal_id}"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["name"], "database");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/label-proposals/{proposal_id}/accept"),
        json!({ "reason": "覆盖不足，接受", "actor": "api-reviewer" }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "accepted");
    assert!(json["data"]["resolved_label_id"].as_str().is_some());
    assert!(
        kanban_sqlite::api::get_task(&db_path, "default", &task.id)?
            .labels
            .is_empty()
    );
    let label_id = json["data"]["resolved_label_id"]
        .as_str()
        .context("resolved label id")?;
    let semantics = kanban_sqlite::api::get_label_semantics(&db_path, "default", label_id)?;
    let atom = semantics
        .atoms
        .iter()
        .find(|atom| atom.kind == "applies_when")
        .context("applies_when atom")?;
    let (status, explained) = get_json(
        app.clone(),
        &format!("/api/v1/boards/default/labels/atoms/{}/explain", atom.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(explained["data"]["legacy_untracked"], false);
    assert!(
        explained["data"]["provenance_actions"]
            .as_array()
            .context("provenance actions")?
            .iter()
            .any(
                |provenance| provenance["action"]["action_type"] == "bootstrap_label"
                    && provenance["action"]["result_proposal_id"] == proposal_id
            ),
        "{explained}"
    );

    let reject_id = seed_proposed_label_proposal(
        &db_path,
        &task.id,
        LabelProposalCandidate {
            name: "release".to_owned(),
            description: Some("Release workflow".to_owned()),
            applies_when: vec!["packaging".to_owned()],
            ..LabelProposalCandidate::default()
        },
    )?;
    let (status, json) = post_json(
        app,
        &format!("/api/v1/label-proposals/{}/reject", reject_id),
        json!({ "reason": "不采用" }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["status"], "rejected");
    Ok(())
}

#[tokio::test]
async fn label_ontology_observation_and_signal_routes_round_trip() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let label = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology API route target"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "agent_candidates_json": json!([
                {"label": "cli", "confidence": 0.92}
            ]).to_string(),
            "suggestion_snapshot_json": json!({
                "selected_labels": [],
                "candidates": []
            }).to_string(),
            "final_decision_json": json!({"accepted_labels": ["cli"]}).to_string(),
            "suggest_coverage": 0.61,
            "suggest_coverage_cosine": 0.74,
            "suggest_residual_norm": 0.39,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
            "capture_fingerprint": "api-ontology-run",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "cli",
                "related_labels_json": "[]",
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "extends CLI subcommands, arguments, help output, or JSON behavior"
                },
                "proposed_label_name": null,
                "proposal_json": "{}",
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.08,
                "suggest_rank": 4,
                "final_selected": true,
                "rationale": "The task expands the CLI surface although suggest scored cli weakly.",
                "confidence": 0.91,
                "signal_key": "cli-false-negative-api"
            }]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["task_id"], task.id);
    assert_eq!(json["data"]["created_by"], "label-agent");
    let suggest_input_hash = json["data"]["suggest_input_hash"]
        .as_str()
        .context("suggest_input_hash")?;
    assert_eq!(suggest_input_hash.len(), 16);
    let signal_id = json["data"]["signals"][0]["id"]
        .as_str()
        .context("signal id")?
        .to_owned();
    assert_eq!(json["data"]["signals"][0]["target_label_id"], label.id);

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/boards/default/label-ontology/signals?status=open&kind=false_negative&task_ref={}&target_label_ref=cli&limit=10",
            task.id
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    let signals = json["data"].as_array().context("signals")?;
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0]["id"], signal_id);

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/boards/default/label-ontology/signals?status=open&kind=false_negative&task={}&label=cli&limit=10",
            task.id
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    let signals = json["data"].as_array().context("signals")?;
    assert_eq!(signals.len(), 1);
    assert_eq!(signals[0]["id"], signal_id);

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/review?group_by=label&limit=10",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["meta"]["group_by"], "label");
    assert_eq!(json["meta"]["include_all"], false);
    let groups = json["data"].as_array().context("review groups")?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["group_by"], "label");
    assert_eq!(groups[0]["label_id"], label.id);
    assert_eq!(groups[0]["label_name"], "cli");
    assert_eq!(groups[0]["task_count"], 1);
    assert_eq!(groups[0]["signal_count"], 1);
    assert_eq!(groups[0]["signal_ids"][0], signal_id);

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/review?group_by=candidate-atom&limit=10",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    let groups = json["data"].as_array().context("candidate atom groups")?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["group_by"], "candidate_atom");
    assert_eq!(
        groups[0]["candidate_text"],
        "extends CLI subcommands, arguments, help output, or JSON behavior"
    );
    assert_eq!(groups[0]["labels"][0]["id"], label.id);

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/review?group_by=cluster&limit=10",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["meta"]["group_by"], "cluster");
    let groups = json["data"].as_array().context("cluster groups")?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0]["group_by"], "cluster");
    assert_eq!(
        groups[0]["cluster_key"],
        "candidate:kind:false_negative|action:add_positive_atom|target:cli|proposed:none|text:extends cli subcommands arguments help output or json behavior"
    );
    assert_eq!(groups[0]["cluster_reason"], "normalized_candidate_text");
    assert_eq!(groups[0]["task_count"], 1);
    assert_eq!(groups[0]["signal_ids"][0], signal_id);

    let (status, json) =
        get_json(app, &format!("/api/v1/label-ontology/signals/{signal_id}")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["signal"]["id"], signal_id);
    assert_eq!(json["data"]["observation"]["task_id"], task.id);
    assert_eq!(
        json["data"]["observation"]["suggest_input_hash"],
        suggest_input_hash
    );
    assert!(
        json["data"]["actions"]
            .as_array()
            .context("actions")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn generic_signal_routes_filter_and_show_board_signals() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("generic signal target"),
    )?;
    let conn = kanban_test_support::connect_file(&db_path)?;
    for (observation_id, signal_id, status, title, created_at) in [
        (
            "obs_generic_open",
            "sig_generic_open",
            "open",
            "Open CLI friction",
            100_i64,
        ),
        (
            "obs_generic_confirmed",
            "sig_generic_confirmed",
            "confirmed",
            "Confirmed CLI friction",
            200_i64,
        ),
        (
            "obs_generic_resolved",
            "sig_generic_resolved",
            "resolved",
            "Resolved CLI friction",
            300_i64,
        ),
    ] {
        let evidence_json = format!(r#"{{"status":"{status}","command":"kanban task create"}}"#);
        conn.execute(
            "INSERT INTO signal_observations(id, board_id, task_id, task_ref_snapshot, actor, agent_type, source, evidence_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, 'codex', 'codex', 'api-test', ?5, ?6)",
            (
                observation_id,
                task.board_id.as_str(),
                task.id.as_str(),
                task.task_ref.as_str(),
                evidence_json.as_str(),
                created_at,
            ),
        )?;
        conn.execute(
            "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, severity, status, dedupe_key, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'agent_cli_friction', ?4, 'Signal summary', 'info', ?5, ?6, ?7, ?7)",
            (
                signal_id,
                task.board_id.as_str(),
                observation_id,
                title,
                status,
                format!("dedupe-{signal_id}"),
                created_at,
            ),
        )?;
    }
    drop(conn);

    let app = test.router();
    let (status, json) = get_json(app.clone(), "/api/v1/boards/default/signals?limit=10").await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["meta"]["include_all"], false);
    let signals = json["data"].as_array().context("signals")?;
    assert_eq!(
        signals
            .iter()
            .map(|signal| signal["id"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["sig_generic_confirmed", "sig_generic_open"]
    );
    assert_eq!(
        signals[0]["observation"]["task_ref_snapshot"],
        task.task_ref
    );

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/signals?include_all=true&limit=10",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["data"].as_array().context("all signals")?.len(), 3);

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/boards/default/signals/review?status=resolved&kind=agent_cli_friction&task={}&limit=10",
            task.id
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    let resolved = json["data"].as_array().context("resolved signals")?;
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0]["id"], "sig_generic_resolved");

    let (status, json) = get_json(app, "/api/v1/signals/sig_generic_open").await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["data"]["title"], "Open CLI friction");
    assert_eq!(json["data"]["observation"]["actor"], "codex");
    assert_eq!(
        json["data"]["observation"]["evidence_json"],
        r#"{"status":"open","command":"kanban task create"}"#
    );
    Ok(())
}

#[tokio::test]
async fn label_ontology_observation_accepts_natural_json_fields() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let label = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology natural API body"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
            "agent_candidates": [{"label": "cli", "confidence": 0.92}],
            "suggestion_snapshot": {
                "selected_labels": [],
                "coverage": 0.61,
                "coverage_cosine": 0.74,
                "residual_norm": 0.39,
                "needs_new_label": false,
                "degraded": true,
                "diagnostics": ["label_atom_index_dirty"]
            },
            "final_decision": {"accepted_labels": ["cli"]},
            "capture_fingerprint": "api-ontology-natural-json",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "cli",
                "related_labels": [],
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "extends CLI subcommands through natural JSON"
                },
                "proposed_label_name": null,
                "proposal": {},
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.08,
                "suggest_rank": 4,
                "final_selected": true,
                "rationale": "The task expands the CLI surface.",
                "confidence": 0.91,
                "signal_key": "cli-natural-json"
            }]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert_eq!(json["data"]["suggest_coverage"], 0.61);
    assert_eq!(json["data"]["suggest_coverage_cosine"], 0.74);
    assert_eq!(json["data"]["suggest_residual_norm"], 0.39);
    assert_eq!(json["data"]["suggest_needs_new_label"], false);
    assert_eq!(json["data"]["suggest_degraded"], true);
    assert_eq!(json["data"]["signals"][0]["target_label_id"], label.id);
    assert_eq!(
        json["data"]["signals"][0]["target_label_name_snapshot"],
        "cli"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            json["data"]["agent_candidates_json"]
                .as_str()
                .context("agent candidates json")?
        )?,
        json!([{"label": "cli", "confidence": 0.92}])
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            json["data"]["diagnostics_json"]
                .as_str()
                .context("diagnostics json")?
        )?,
        json!(["label_atom_index_dirty"])
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            json["data"]["signals"][0]["related_labels_json"]
                .as_str()
                .context("related labels json")?
        )?,
        json!([])
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            json["data"]["signals"][0]["proposal_json"]
                .as_str()
                .context("proposal json")?
        )?,
        json!({})
    );
    Ok(())
}

#[tokio::test]
async fn label_ontology_observation_rejects_duplicate_json_fields() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology duplicate API body"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
            "agent_candidates_json": "[]",
            "suggestion_snapshot": {},
            "suggestion_snapshot_json": "{}",
            "final_decision_json": "{}",
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
            "signals": []
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error"]["code"], "invalid_input");
    let message = json["error"]["message"].as_str().context("error message")?;
    assert!(message.contains("suggestion_snapshot"), "{message}");
    assert!(message.contains("suggestion_snapshot_json"), "{message}");
    Ok(())
}

#[tokio::test]
async fn label_ontology_observation_rejects_conflicting_snapshot_metrics() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology conflicting snapshot metric"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
            "agent_candidates": [],
            "suggestion_snapshot": {
                "coverage": 0.61,
                "coverage_cosine": 0.74,
                "residual_norm": 0.39,
                "needs_new_label": false,
                "degraded": false,
                "diagnostics": []
            },
            "final_decision": {},
            "suggest_coverage": 0.9,
            "signals": []
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error"]["code"], "invalid_input");
    let message = json["error"]["message"].as_str().context("error message")?;
    assert!(message.contains("suggest_coverage"), "{message}");
    assert!(
        message.contains("suggestion_snapshot.coverage"),
        "{message}"
    );
    Ok(())
}

#[tokio::test]
async fn label_ontology_observation_rejects_invalid_natural_json_shape() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology invalid natural JSON shape"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
            "agent_candidates": [],
            "suggestion_snapshot": {},
            "final_decision": [],
            "diagnostics": [],
            "signals": []
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error"]["code"], "invalid_input");
    let message = json["error"]["message"].as_str().context("error message")?;
    assert!(message.contains("final_decision"), "{message}");
    assert!(message.contains("JSON object"), "{message}");
    Ok(())
}

#[tokio::test]
async fn label_ontology_observation_route_rejects_invalid_signal_contract() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology API invalid signal target"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "agent_candidates_json": "[]",
            "suggestion_snapshot_json": "{}",
            "final_decision_json": "{}",
            "suggest_coverage": 0.61,
            "suggest_coverage_cosine": 0.74,
            "suggest_residual_norm": 0.39,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
            "capture_fingerprint": "api-invalid-signal-contract",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "cli",
                "related_labels_json": "[]",
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "negative",
                    "kind": "applies_when",
                    "text": "does not touch CLI behavior"
                },
                "proposed_label_name": null,
                "proposal_json": "{}",
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.08,
                "suggest_rank": 4,
                "final_selected": true,
                "rationale": "The atom contract is inconsistent.",
                "confidence": 0.91,
                "signal_key": "api-invalid-signal-contract"
            }]
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("candidate atom polarity")
    );
    let conn = kanban_test_support::connect_file(&db_path)?;
    let observation_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM label_ontology_observations",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(observation_count, 0);
    Ok(())
}

#[tokio::test]
async fn label_ontology_apply_atom_route_rejects_incompatible_source_signal() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology API incompatible source signal"),
    )?;
    let observation = kanban_sqlite::api::record_label_ontology_observation(
        &db_path,
        "default",
        &task.id,
        kanban_sqlite::api::LabelOntologyRecordInput {
            actor: kanban_sqlite::api::LabelOntologyActor {
                name: "label-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            agent_candidates_json: json!([{"label": "cli", "confidence": 0.92}]).to_string(),
            suggestion_snapshot_json: json!({"selected_labels": []}).to_string(),
            final_decision_json: json!({"accepted_labels": ["cli"]}).to_string(),
            suggest_coverage: Some(0.61),
            suggest_coverage_cosine: Some(0.74),
            suggest_residual_norm: Some(0.39),
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics_json: json!([]).to_string(),
            capture_fingerprint: Some("api-incompatible-source-signal".to_owned()),
            signals: vec![kanban_sqlite::api::LabelOntologySignalInput {
                kind: kanban_sqlite::api::LabelOntologySignalKind::FalseNegative,
                target_label_ref: Some("cli".to_owned()),
                related_labels_json: json!([]).to_string(),
                proposed_action: kanban_sqlite::api::LabelOntologyProposedAction::AddPositiveAtom,
                candidate_atom: Some(kanban_sqlite::api::LabelOntologyCandidateAtomInput {
                    polarity: "positive".to_owned(),
                    kind: "applies_when".to_owned(),
                    text: "extends CLI subcommands, arguments, help output, or JSON behavior"
                        .to_owned(),
                }),
                proposed_label_name: None,
                proposal_json: json!({}).to_string(),
                agent_selected: true,
                suggest_state: Some(kanban_sqlite::api::LabelOntologySuggestState::Candidate),
                suggest_score: Some(0.08),
                suggest_rank: Some(4),
                final_selected: true,
                rationale: "The task expands the CLI surface.".to_owned(),
                confidence: Some(0.91),
                signal_key: Some("api-incompatible-source-signal".to_owned()),
            }],
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    kanban_sqlite::api::create_label_ontology_action(
        &db_path,
        "default",
        kanban_sqlite::api::LabelOntologyActionInput {
            actor: kanban_sqlite::api::LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            action_type: kanban_sqlite::api::LabelOntologyActionType::Confirm,
            signal_ids: vec![signal_id.clone()],
            reason: "valid false negative".to_owned(),
            superseded_by_signal_id: None,
            parent_action_id: None,
            target_label_ref: None,
            result_label_ref: None,
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: None,
            canonical_before_hash: None,
            canonical_after_hash: None,
            change_json: None,
            validation_status: None,
            validation_json: None,
        },
    )?;
    let conn = kanban_test_support::connect_file(&db_path)?;
    let action_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM label_ontology_actions", [], |row| {
            row.get(0)
        })?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/label-ontology/apply/atom",
        json!({
            "actor": {
                "name": "reviewer",
                "type": "user",
                "agent_type": null
            },
            "signal_ids": [signal_id],
            "label_ref": "cli",
            "kind": "excludes_when",
            "text": "only updates unrelated release notes",
            "reason": "This should fail because the source signal requested a positive atom."
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("proposed action add_positive_atom")
    );
    let post_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM label_ontology_actions", [], |row| {
            row.get(0)
        })?;
    assert_eq!(post_count, action_count);
    assert!(kanban_sqlite::api::list_label_atoms(&db_path, "default")?.is_empty());
    Ok(())
}

#[tokio::test]
async fn label_ontology_action_apply_and_validate_routes_round_trip() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let cli_label = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology API action route target"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "agent_candidates_json": "[]",
            "suggestion_snapshot_json": "{}",
            "final_decision_json": "{}",
            "suggest_coverage": null,
            "suggest_coverage_cosine": null,
            "suggest_residual_norm": null,
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
            "capture_fingerprint": "api-ontology-action-run",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "cli",
                "related_labels_json": "[]",
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "changes the local CLI command surface"
                },
                "proposed_label_name": null,
                "proposal_json": "{}",
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.12,
                "suggest_rank": 2,
                "final_selected": true,
                "rationale": "The task changes CLI behavior.",
                "confidence": 0.88,
                "signal_key": "cli-action-api"
            }]
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let signal_id = json["data"]["signals"][0]["id"]
        .as_str()
        .context("signal id")?
        .to_owned();

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/actions",
        json!({
            "actor": {
                "name": "reviewer",
                "type": "user",
                "agent_type": null
            },
            "action_type": "confirm",
            "signal_ids": [signal_id],
            "reason": "valid false negative",
            "superseded_by_signal_id": null,
            "parent_action_id": null,
            "target_label_ref": null,
            "result_label_ref": null,
            "result_atom_id": null,
            "result_atom_content_hash": null,
            "result_proposal_id": null,
            "canonical_before_hash": null,
            "canonical_after_hash": null,
            "change_json": null,
            "validation_status": null,
            "validation_json": null
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert_eq!(json["data"]["action_type"], "confirm");
    assert_eq!(json["data"]["signal_ids"], json!([signal_id]));

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/apply/atom",
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "signal_ids": [signal_id],
            "label_ref": "cli",
            "kind": "applies_when",
            "text": "changes the local CLI command surface",
            "reason": "apply confirmed atom",
            "allow_retarget": true,
            "retarget_reason": "API caller explicitly audited this source signal retarget."
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert_eq!(json["data"]["action_type"], "add_positive_atom");
    assert_eq!(json["data"]["validation_requirement"], "required");
    assert_eq!(json["data"]["validation_status"], "pending");
    assert_eq!(json["data"]["validation_effective_outcome"], "pending");
    assert_eq!(
        json["data"]["validation_latest_attempt_id"],
        serde_json::Value::Null
    );
    let change: serde_json::Value = serde_json::from_str(
        json["data"]["change_json"]
            .as_str()
            .context("change_json")?,
    )?;
    assert_eq!(
        change["retarget_override"]["reason"],
        "API caller explicitly audited this source signal retarget."
    );
    let apply_action_id = json["data"]["id"]
        .as_str()
        .context("apply action id")?
        .to_owned();
    let result_atom_id = json["data"]["result_atom_id"]
        .as_str()
        .context("result atom id")?
        .to_owned();
    let result_atom_content_hash = json["data"]["result_atom_content_hash"]
        .as_str()
        .context("result atom content hash")?
        .to_owned();

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/validate",
        json!({
            "actor": {
                "name": "reviewer",
                "type": "user",
                "agent_type": null
            },
            "parent_action_id": apply_action_id.clone(),
            "signal_ids": [],
            "reason": "empty evidence cannot pass validation",
            "validation_status": "passed",
            "validation_json": "{}"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error"]["code"], "invalid_input");

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/validate",
        json!({
            "actor": {
                "name": "reviewer",
                "type": "user",
                "agent_type": null
            },
            "parent_action_id": apply_action_id.clone(),
            "signal_ids": [],
            "reason": "external failed diagnostics should not resolve the signal",
            "validation_status": "failed",
            "validation": {"cases": []}
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert_eq!(json["data"]["action_type"], "validate");
    assert_eq!(json["data"]["validation_status"], "failed");
    assert_eq!(json["data"]["validation_effective_outcome"], "failed");
    let failed_validation_id = json["data"]["id"]
        .as_str()
        .context("failed validation id")?
        .to_owned();

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/validate",
        json!({
            "actor": {
                "name": "reviewer",
                "type": "user",
                "agent_type": null
            },
            "parent_action_id": apply_action_id.clone(),
            "signal_ids": [],
            "reason": "atom improves suggestion behavior",
            "validation_status": "passed",
            "validation": json!({
                "evidence_type": "trusted_automated",
                "embedding_model": "test-embedding-v1",
                "solver_options": {"candidate_limit": 24, "atom_limit": 64},
                "index": {"status": "ready", "dirty": false, "generation": 7},
                "cases": [{
                    "signal_id": signal_id.clone(),
                    "case_type": "positive_atom",
                    "passed": true,
                    "target_label_id": &cli_label.id,
                    "before": {
                        "target": {
                            "label_id": &cli_label.id,
                            "selected": false,
                            "score": 0.12
                        },
                        "coverage": 0.61
                    },
                    "after": {
                        "degraded": false,
                        "target": {
                            "label_id": &cli_label.id,
                            "selected": true,
                            "score": 0.73
                        },
                        "coverage": 0.78,
                        "evidence_atoms": [{
                            "id": result_atom_id,
                            "content_hash": result_atom_content_hash,
                            "label_id": &cli_label.id
                        }]
                    }
                }]
            })
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("trusted evidence collected by the kanban tool"),
        "{json}"
    );

    let (status, json) = get_json(
        app.clone(),
        &format!("/api/v1/label-ontology/signals/{signal_id}"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["signal"]["status"], "confirmed");
    let actions = json["data"]["actions"].as_array().context("actions")?;
    assert_eq!(actions.len(), 3);
    let apply_action = actions
        .iter()
        .find(|action| action["id"].as_str() == Some(apply_action_id.as_str()))
        .context("apply action in signal detail")?;
    assert_eq!(apply_action["validation_effective_outcome"], "failed");
    assert_eq!(
        apply_action["validation_latest_attempt_id"].as_str(),
        Some(failed_validation_id.as_str())
    );

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {
                "name": "label-agent",
                "type": "agent",
                "agent_type": "local"
            },
            "agent_candidates_json": "[]",
            "suggestion_snapshot_json": "{}",
            "final_decision_json": "{}",
            "suggest_coverage": 0.2,
            "suggest_coverage_cosine": 0.3,
            "suggest_residual_norm": 0.8,
            "suggest_needs_new_label": true,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
            "capture_fingerprint": "api-proposal-gap",
            "signals": [{
                "kind": "vocabulary_gap",
                "target_label_ref": null,
                "related_labels_json": "[]",
                "proposed_action": "bootstrap_label",
                "candidate_atom": null,
                "proposed_label_name": "ontology-ledger",
                "proposal_json": "{\"name\":\"ontology-ledger\"}",
                "agent_selected": true,
                "suggest_state": "absent",
                "suggest_score": null,
                "suggest_rank": null,
                "final_selected": true,
                "rationale": "Existing labels do not express ontology ledger storage.",
                "confidence": 0.86,
                "signal_key": "api-proposal-gap"
            }]
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    let gap_signal_id = json["data"]["signals"][0]["id"]
        .as_str()
        .context("gap signal id")?
        .to_owned();
    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/signals?proposed_label=ontology-ledger&limit=10",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    let proposed_label_signals = json["data"].as_array().context("proposed label signals")?;
    assert_eq!(proposed_label_signals.len(), 1);
    assert_eq!(proposed_label_signals[0]["id"], gap_signal_id);

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/actions",
        json!({
            "actor": {
                "name": "reviewer",
                "type": "user",
                "agent_type": null
            },
            "action_type": "confirm",
            "signal_ids": [gap_signal_id],
            "reason": "valid vocabulary gap"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    let proposal_id = seed_proposed_label_proposal(
        &db_path,
        &task.id,
        LabelProposalCandidate {
            name: "ontology-ledger".to_owned(),
            description: Some("Label ontology ledger work".to_owned()),
            applies_when: vec!["records ontology observations and signals".to_owned()],
            positive_examples: vec!["creates label ontology ledger tables".to_owned()],
            ..LabelProposalCandidate::default()
        },
    )?;
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/label-proposals/{proposal_id}/accept"),
        json!({
            "reason": "Bootstrap from confirmed vocabulary-gap signal.",
            "actor": "api-reviewer",
            "source_signal_ids": [gap_signal_id],
            "ontology_actor": {
                "name": "ontology-agent",
                "type": "agent",
                "agent_type": "codex"
            },
            "allow_retarget": true,
            "retarget_reason": "API reviewer explicitly audited proposal source signal retarget."
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["data"]["status"], "accepted");
    assert!(json["data"]["resolved_label_id"].as_str().is_some());
    let (status, json) = get_json(
        app,
        &format!("/api/v1/label-ontology/signals/{gap_signal_id}"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["data"]["signal"]["status"], "confirmed");
    let bootstrap = json["data"]["actions"]
        .as_array()
        .context("gap actions")?
        .iter()
        .find(|action| action["action_type"] == "bootstrap_label")
        .context("bootstrap action")?;
    assert_eq!(bootstrap["created_by"], "ontology-agent");
    assert_eq!(bootstrap["created_by_type"], "agent");
    assert_eq!(bootstrap["agent_type"], "codex");
    let change: serde_json::Value =
        serde_json::from_str(bootstrap["change_json"].as_str().context("change_json")?)?;
    assert_eq!(
        change["retarget_override"]["reason"],
        "API reviewer explicitly audited proposal source signal retarget."
    );
    Ok(())
}

#[tokio::test]
async fn label_ontology_revert_route_round_trip() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    kanban_sqlite::api::upsert_label_semantics(
        &db_path,
        "default",
        kanban_sqlite::api::UpsertLabelSemantics {
            label_ref: "cli".to_owned(),
            description: Some("Command-line interface behavior".to_owned()),
            ..kanban_sqlite::api::UpsertLabelSemantics::default()
        },
    )?;
    let before_semantics = kanban_sqlite::api::get_label_semantics(&db_path, "default", "cli")?;
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology API revert route target"),
    )?;
    let observation = kanban_sqlite::api::record_label_ontology_observation(
        &db_path,
        "default",
        &task.id,
        kanban_sqlite::api::LabelOntologyRecordInput {
            actor: kanban_sqlite::api::LabelOntologyActor {
                name: "label-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            agent_candidates_json: "[]".to_owned(),
            suggestion_snapshot_json: "{}".to_owned(),
            final_decision_json: "{}".to_owned(),
            suggest_coverage: None,
            suggest_coverage_cosine: None,
            suggest_residual_norm: None,
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics_json: "[]".to_owned(),
            capture_fingerprint: Some("api-ontology-revert-route".to_owned()),
            signals: vec![kanban_sqlite::api::LabelOntologySignalInput {
                kind: kanban_sqlite::api::LabelOntologySignalKind::FalseNegative,
                target_label_ref: Some("cli".to_owned()),
                related_labels_json: "[]".to_owned(),
                proposed_action: kanban_sqlite::api::LabelOntologyProposedAction::AddPositiveAtom,
                candidate_atom: Some(kanban_sqlite::api::LabelOntologyCandidateAtomInput {
                    polarity: "positive".to_owned(),
                    kind: "applies_when".to_owned(),
                    text: "changes the local CLI command surface".to_owned(),
                }),
                proposed_label_name: None,
                proposal_json: "{}".to_owned(),
                agent_selected: true,
                suggest_state: Some(kanban_sqlite::api::LabelOntologySuggestState::Candidate),
                suggest_score: Some(0.12),
                suggest_rank: Some(2),
                final_selected: true,
                rationale: "The task changes CLI behavior.".to_owned(),
                confidence: Some(0.88),
                signal_key: Some("api-ontology-revert".to_owned()),
            }],
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    kanban_sqlite::api::create_label_ontology_action(
        &db_path,
        "default",
        kanban_sqlite::api::LabelOntologyActionInput {
            actor: kanban_sqlite::api::LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            action_type: kanban_sqlite::api::LabelOntologyActionType::Confirm,
            signal_ids: vec![signal_id.clone()],
            reason: "valid false negative".to_owned(),
            superseded_by_signal_id: None,
            parent_action_id: None,
            target_label_ref: None,
            result_label_ref: None,
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: None,
            canonical_before_hash: None,
            canonical_after_hash: None,
            change_json: None,
            validation_status: None,
            validation_json: None,
        },
    )?;
    let applied = kanban_sqlite::api::apply_label_ontology_atom(
        &db_path,
        "default",
        kanban_sqlite::api::LabelOntologyAtomApplyInput {
            actor: kanban_sqlite::api::LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            signal_ids: vec![signal_id.clone()],
            label_ref: "cli".to_owned(),
            kind: "applies_when".to_owned(),
            text: "changes the local CLI command surface".to_owned(),
            reason: "apply confirmed atom".to_owned(),
        },
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/label-ontology/revert",
        json!({
            "actor": {
                "name": "reviewer",
                "type": "user",
                "agent_type": null
            },
            "target_action_id": applied.id.clone(),
            "expected_current_hash": applied.canonical_after_hash.clone(),
            "reason": "Revert the test ontology mutation."
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{json}");
    assert_eq!(json["data"]["action_type"], "revert_ontology_mutation");
    assert_eq!(json["data"]["parent_action_id"], applied.id);
    assert_eq!(json["data"]["signal_ids"], json!([signal_id]));
    let restored_semantics = kanban_sqlite::api::get_label_semantics(&db_path, "default", "cli")?;
    assert_eq!(
        restored_semantics.semantics_hash,
        before_semantics.semantics_hash
    );
    assert_eq!(restored_semantics.description, before_semantics.description);
    assert_eq!(
        restored_semantics.applies_when,
        before_semantics.applies_when
    );
    assert_eq!(
        restored_semantics.excludes_when,
        before_semantics.excludes_when
    );
    assert_eq!(
        restored_semantics.positive_examples,
        before_semantics.positive_examples
    );
    assert_eq!(
        restored_semantics.negative_examples,
        before_semantics.negative_examples
    );
    assert_eq!(
        restored_semantics
            .atoms
            .iter()
            .map(|atom| (
                atom.polarity.as_str(),
                atom.kind.as_str(),
                atom.text.as_str(),
                atom.ordinal,
                atom.content_hash.as_str(),
            ))
            .collect::<Vec<_>>(),
        before_semantics
            .atoms
            .iter()
            .map(|atom| (
                atom.polarity.as_str(),
                atom.kind.as_str(),
                atom.text.as_str(),
                atom.ordinal,
                atom.content_hash.as_str(),
            ))
            .collect::<Vec<_>>()
    );

    Ok(())
}

#[tokio::test]
async fn label_ontology_structure_plan_route_is_not_available() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/boards/default/label-ontology/structure-plan")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "actor": {
                            "name": "structure-agent",
                            "type": "agent",
                            "agent_type": "local"
                        },
                        "signal_ids": ["los_missing"],
                        "action_type": "merge_labels",
                        "target_label_ref": "cli",
                        "related_label_refs": ["command-surface"],
                        "reason": "Structure plans are no longer public write entries."
                    })
                    .to_string(),
                ))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let action_count: i64 = kanban_test_support::connect_file(&db_path)?.query_row(
        "SELECT COUNT(*) FROM label_ontology_actions",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(action_count, 0);

    Ok(())
}

#[tokio::test]
async fn label_ontology_action_route_rejects_generic_mutation_action_type() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology API mutation guard target"),
    )?;
    let app = test.router();

    let observation = kanban_sqlite::api::record_label_ontology_observation(
        &db_path,
        "default",
        &task.id,
        kanban_sqlite::api::LabelOntologyRecordInput {
            actor: kanban_sqlite::api::LabelOntologyActor {
                name: "label-agent".to_owned(),
                actor_type: "agent".to_owned(),
                agent_type: Some("local".to_owned()),
            },
            agent_candidates_json: "[]".to_owned(),
            suggestion_snapshot_json: "{}".to_owned(),
            final_decision_json: "{}".to_owned(),
            suggest_coverage: None,
            suggest_coverage_cosine: None,
            suggest_residual_norm: None,
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics_json: "[]".to_owned(),
            capture_fingerprint: Some("api-generic-mutation-guard".to_owned()),
            signals: vec![kanban_sqlite::api::LabelOntologySignalInput {
                kind: kanban_sqlite::api::LabelOntologySignalKind::FalseNegative,
                target_label_ref: Some("cli".to_owned()),
                related_labels_json: "[]".to_owned(),
                proposed_action: kanban_sqlite::api::LabelOntologyProposedAction::AddPositiveAtom,
                candidate_atom: Some(kanban_sqlite::api::LabelOntologyCandidateAtomInput {
                    polarity: "positive".to_owned(),
                    kind: "applies_when".to_owned(),
                    text: "changes the local CLI command surface".to_owned(),
                }),
                proposed_label_name: None,
                proposal_json: "{}".to_owned(),
                agent_selected: true,
                suggest_state: Some(kanban_sqlite::api::LabelOntologySuggestState::Candidate),
                suggest_score: Some(0.12),
                suggest_rank: Some(2),
                final_selected: true,
                rationale: "The task changes CLI behavior.".to_owned(),
                confidence: Some(0.88),
                signal_key: Some("api-generic-mutation-guard".to_owned()),
            }],
        },
    )?;
    let signal_id = observation.signals[0].id.clone();
    kanban_sqlite::api::create_label_ontology_action(
        &db_path,
        "default",
        kanban_sqlite::api::LabelOntologyActionInput {
            actor: kanban_sqlite::api::LabelOntologyActor {
                name: "reviewer".to_owned(),
                actor_type: "user".to_owned(),
                agent_type: None,
            },
            action_type: kanban_sqlite::api::LabelOntologyActionType::Confirm,
            signal_ids: vec![signal_id.clone()],
            reason: "valid false negative".to_owned(),
            superseded_by_signal_id: None,
            parent_action_id: None,
            target_label_ref: None,
            result_label_ref: None,
            result_atom_id: None,
            result_atom_content_hash: None,
            result_proposal_id: None,
            canonical_before_hash: None,
            canonical_after_hash: None,
            change_json: None,
            validation_status: None,
            validation_json: None,
        },
    )?;

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/label-ontology/actions",
        json!({
            "actor": {
                "name": "reviewer",
                "type": "user",
                "agent_type": null
            },
            "action_type": "add_positive_atom",
            "signal_ids": [signal_id],
            "reason": "generic endpoint must not record canonical mutations",
            "superseded_by_signal_id": null,
            "parent_action_id": null,
            "target_label_ref": "cli",
            "result_label_ref": null,
            "result_atom_id": "la_fabricated",
            "result_atom_content_hash": "deadbeefdeadbeef",
            "result_proposal_id": null,
            "canonical_before_hash": "before",
            "canonical_after_hash": "after",
            "change_json": json!({"fabricated": true}).to_string(),
            "validation_status": "pending",
            "validation_json": null
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("dedicated canonical mutation endpoint")
    );

    Ok(())
}

#[tokio::test]
async fn label_ontology_action_routes_reject_unknown_json_fields() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    for (path, body) in [
        (
            "/api/v1/boards/default/label-ontology/actions",
            json!({
                "actor": {"name": "reviewer", "type": "user", "agent_type": null},
                "action_type": "confirm",
                "signal_ids": ["los_missing"],
                "reason": "reviewed",
                "unexpected": true
            }),
        ),
        (
            "/api/v1/boards/default/label-ontology/apply/atom",
            json!({
                "actor": {"name": "reviewer", "type": "user", "agent_type": null},
                "signal_ids": ["los_missing"],
                "label_ref": "cli",
                "kind": "applies_when",
                "text": "CLI work",
                "reason": "apply",
                "unexpected": true
            }),
        ),
        (
            "/api/v1/boards/default/label-ontology/validate",
            json!({
                "actor": {"name": "reviewer", "type": "user", "agent_type": null},
                "parent_action_id": "loa_missing",
                "signal_ids": [],
                "reason": "validated",
                "validation_status": "passed",
                "validation_json": "{}",
                "unexpected": true
            }),
        ),
    ] {
        let (status, json) = post_json(app.clone(), path, body).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {json}");
        assert_eq!(json["error"]["code"], "invalid_input");
    }
    Ok(())
}

fn seed_proposed_label_proposal(
    db_path: &std::path::Path,
    task_id: &str,
    candidate: LabelProposalCandidate,
) -> anyhow::Result<String> {
    let conn = kanban_test_support::connect_file(db_path)?;
    let board_id: String =
        conn.query_row("SELECT board_id FROM tasks WHERE id=?1", [task_id], |row| {
            row.get(0)
        })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;
    let proposal_id = format!("lp_api_{}_{}", candidate.name, now);
    let applies_when = serde_json::to_string(&candidate.applies_when)?;
    let excludes_when = serde_json::to_string(&candidate.excludes_when)?;
    let positive_examples = serde_json::to_string(&candidate.positive_examples)?;
    let negative_examples = serde_json::to_string(&candidate.negative_examples)?;
    conn.execute(
        "INSERT INTO label_semantic_proposals(
            id, board_id, task_id, status, name, description, applies_when, excludes_when,
            positive_examples, negative_examples, heuristic_coverage, heuristic_residual_norm,
            diagnostics_json, created_by, created_at, updated_at
        ) VALUES (?1, ?2, ?3, 'proposed', ?4, ?5, ?6, ?7, ?8, ?9, 0.0, 1.0, '[]', ?10, ?11, ?11)",
        (
            &proposal_id,
            &board_id,
            task_id,
            &candidate.name,
            candidate.description.as_deref(),
            &applies_when,
            &excludes_when,
            &positive_examples,
            &negative_examples,
            "api-test-proposer",
            now,
        ),
    )?;
    Ok(proposal_id)
}

#[tokio::test]
async fn board_label_semantics_and_atom_routes_round_trip() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let app = test.router();
    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/labels",
        json!({ "name": "team/backend" }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["name"], "team/backend");
    let label_id = json["data"]["id"].as_str().context("label id")?.to_owned();

    let (status, json) = request_json(
        app.clone(),
        "PUT",
        &format!("/api/v1/boards/default/labels/{label_id}/semantics"),
        Some(json!({
            "description": "Backend service work",
            "applies_when": ["touches Rust service code"],
            "excludes_when": ["CSS-only"],
            "positive_examples": ["add API handler"],
            "negative_examples": ["adjust spacing"]
        })),
        None,
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["label_name"], "team/backend");
    assert_eq!(json["data"]["description"], "Backend service work");
    assert_eq!(
        json["data"]["applies_when"],
        json!(["touches Rust service code"])
    );
    assert!(
        json["data"]["atoms"]
            .as_array()
            .context("atoms")?
            .iter()
            .any(|atom| atom["polarity"] == "negative" && atom["text"] == "CSS-only")
    );

    let (status, json) = get_json(
        app.clone(),
        &format!("/api/v1/boards/default/labels/{label_id}/semantics"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["label_name"], "team/backend");
    let seed_hash = json["data"]["semantics_hash"]
        .as_str()
        .context("semantics hash")?
        .to_owned();

    let (status, json) = request_json(
        app.clone(),
        "PUT",
        &format!("/api/v1/boards/default/labels/{label_id}/semantics"),
        Some(json!({
            "expected_semantics_hash": seed_hash,
            "applies_when": ["serves kanban API routes"],
            "remove_excludes_when": ["CSS-only"],
            "reason": "HTTP patch guardrail test"
        })),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json["data"]["applies_when"],
        json!(["touches Rust service code", "serves kanban API routes"])
    );
    assert_eq!(json["data"]["excludes_when"], json!([]));
    assert_eq!(
        json["data"]["positive_examples"],
        json!(["add API handler"])
    );
    assert_eq!(json["data"]["negative_examples"], json!(["adjust spacing"]));
    let patched_hash = json["data"]["semantics_hash"]
        .as_str()
        .context("patched semantics hash")?
        .to_owned();
    let (status, json) = request_json(
        app.clone(),
        "PUT",
        &format!("/api/v1/boards/default/labels/{label_id}/semantics"),
        Some(json!({
            "expected_semantics_hash": seed_hash,
            "applies_when": ["stale writer addition"]
        })),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "conflict");

    let (status, json) = request_json(
        app.clone(),
        "PUT",
        &format!("/api/v1/boards/default/labels/{label_id}/semantics"),
        Some(json!({
            "expected_semantics_hash": patched_hash,
            "replace": true,
            "description": "Backend replacement semantics"
        })),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["description"], "Backend replacement semantics");
    assert_eq!(json["data"]["applies_when"], json!([]));
    assert_eq!(json["data"]["positive_examples"], json!([]));
    let replacement_hash = json["data"]["semantics_hash"]
        .as_str()
        .context("replacement semantics hash")?
        .to_owned();

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/labels/team-backend/semantics",
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("label_id must be a canonical l_ id")
    );

    let (status, json) = get_json(app.clone(), "/api/v1/boards/default/labels/semantics").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"].as_array().context("semantics")?.len(), 1);

    let (status, json) = get_json(app.clone(), "/api/v1/boards/default/labels/atoms").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json["data"]
            .as_array()
            .context("atoms")?
            .iter()
            .any(|atom| atom["kind"] == "description"
                && atom["text"]
                    .as_str()
                    .is_some_and(|text| text.contains("Backend replacement semantics")))
    );

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/labels/atom-index/status",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], false);

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/labels/atom-index/rebuild",
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("vector helper")
    );

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/labels/atom-index/query?q=backend&polarity=positive",
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");

    let (status, json) = delete_json(
        app.clone(),
        &format!("/api/v1/boards/default/labels/{label_id}/semantics"),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");

    let (status, json) = delete_json(
        app.clone(),
        &format!(
            "/api/v1/boards/default/labels/{label_id}/semantics?expected_semantics_hash=not-the-current-semantics-hash&reason=http-stale-clear"
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "conflict");
    assert!(
        !kanban_sqlite::api::list_label_atoms(&db_path, "default")?.is_empty(),
        "stale clear must not remove atoms"
    );

    let (status, json) = delete_json(
        app.clone(),
        &format!(
            "/api/v1/boards/default/labels/{label_id}/semantics?expected_semantics_hash={replacement_hash}&reason=http-clear"
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["deleted"], true);
    assert!(kanban_sqlite::api::list_label_atoms(&db_path, "default")?.is_empty());
    Ok(())
}

#[tokio::test]
async fn label_atom_explain_route_returns_legacy_untracked_for_unprovenanced_atom()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

    let label = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "team/backend".to_owned(),
            color: None,
        },
    )?;
    kanban_sqlite::api::upsert_label_semantics_by_id(
        &db_path,
        "default",
        &label.id,
        kanban_sqlite::api::UpsertLabelSemantics {
            label_ref: label.id.clone(),
            description: Some("Backend service work".to_owned()),
            applies_when: vec!["touches Rust service code".to_owned()],
            excludes_when: vec![],
            positive_examples: vec!["add API handler".to_owned()],
            negative_examples: vec![],
            ..kanban_sqlite::api::UpsertLabelSemantics::default()
        },
    )?;
    kanban_test_support::connect_file(&db_path)?
        .execute("DELETE FROM label_ontology_actions", [])?;
    let atom = kanban_sqlite::api::list_label_atoms(&db_path, "default")?
        .into_iter()
        .find(|atom| atom.kind == "positive_example")
        .context("positive atom")?;

    let (status, json) = get_json(
        app,
        &format!("/api/v1/boards/default/labels/atoms/{}/explain", atom.id),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["query"], atom.id);
    assert_eq!(json["data"]["atom"]["id"], atom.id);
    assert_eq!(json["data"]["current_semantics"]["label_id"], label.id);
    assert_eq!(json["data"]["legacy_untracked"], true);
    assert_eq!(json["data"]["provenance_actions"], json!([]));
    assert!(
        json["data"]["legacy_reason"]
            .as_str()
            .context("legacy reason")?
            .contains("no ontology provenance action")
    );
    Ok(())
}

#[tokio::test]
async fn board_label_semantics_paths_resolve_exact_label_ids_only() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

    let name_prefixed = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "l_bug".to_owned(),
            color: None,
        },
    )?;
    let (status, json) = request_json(
        app.clone(),
        "PUT",
        "/api/v1/boards/default/labels/l_bug/semantics",
        Some(json!({
            "description": "This path segment is a name, not a persisted id"
        })),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");
    assert!(
        kanban_sqlite::api::get_label_semantics(&db_path, "default", &name_prefixed.name).is_err()
    );

    let canonical = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "canonical".to_owned(),
            color: None,
        },
    )?;
    let colliding_name = kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: canonical.id.clone(),
            color: None,
        },
    )?;

    let (status, json) = request_json(
        app.clone(),
        "PUT",
        &format!("/api/v1/boards/default/labels/{}/semantics", canonical.id),
        Some(json!({
            "description": "Canonical id wins over a label name collision"
        })),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["label_id"], canonical.id);
    assert_eq!(json["data"]["label_name"], canonical.name);

    let (status, json) = get_json(
        app,
        &format!("/api/v1/boards/default/labels/{}/semantics", canonical.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["label_id"], canonical.id);
    assert_eq!(json["data"]["label_name"], canonical.name);
    assert!(
        kanban_sqlite::api::get_label_semantics(&db_path, "default", &colliding_name.name).is_err()
    );
    Ok(())
}

#[tokio::test]
async fn task_label_routes_use_task_board_and_reject_archived_targets() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::api::create_board(
        &db_path,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;
    let other_task = kanban_sqlite::api::create_task(
        &db_path,
        "other",
        "seed",
        kanban_sqlite::api::CreateTask::ready("non-default route target"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", other_task.id),
        json!({ "name": "backend", "create_missing": true, "actor": "api-labeler" }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["board_slug"], "other");
    assert_eq!(json["data"]["labels"][0]["name"], "backend");
    assert_eq!(json["meta"]["created_labels"][0]["name"], "backend");
    assert!(kanban_sqlite::api::list_labels(&db_path, "default")?.is_empty());
    let other_labels = kanban_sqlite::api::list_labels(&db_path, "other")?;
    assert_eq!(other_labels[0].name, "backend");

    let (status, json) = delete_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels/backend", other_task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json["data"]["labels"]
            .as_array()
            .context("labels")?
            .is_empty()
    );

    let archived_task = kanban_sqlite::api::create_task(
        &db_path,
        "other",
        "seed",
        kanban_sqlite::api::CreateTask::ready("archived route target"),
    )?;
    kanban_sqlite::api::archive_task(&db_path, "other", "seed", &archived_task.id, false)?;
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", archived_task.id),
        json!({ "name": "blocked" }),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");

    kanban_sqlite::api::archive_board(&db_path, "other", "seed")?;
    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/labels", other_task.id),
        json!({ "name": "blocked" }),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");
    Ok(())
}

#[tokio::test]
async fn tasks_list_search_matches_task_refs_exactly() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let first = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("first api list task"),
    )
    .context("seed first task")?;
    let _second = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("title mentions 1 but must not match numeric search"),
    )
    .context("seed second task")?;
    let app = test.router();

    for query in ["1", "%231", "default%231", first.id.as_str()] {
        let (status, json) = get_json(
            app.clone(),
            &format!("/api/v1/boards/default/tasks?q={query}&limit=10"),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{query}: {json}");
        let tasks = json["data"].as_array().context("tasks array")?;
        assert_eq!(tasks.len(), 1, "{query}: {json}");
        assert_eq!(tasks[0]["id"], first.id, "{query}: {json}");
    }

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?q=other%231&limit=10").await?;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"].as_array().context("tasks array")?.is_empty());
    Ok(())
}

#[tokio::test]
async fn tasks_lists_with_priority_filters_and_table_sort_fields() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    for (title, priority, assignee) in [
        ("bravo", 2, Some("worker-b")),
        ("alpha", 0, Some("worker-a")),
        ("charlie", 3, None),
    ] {
        kanban_sqlite::api::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::api::CreateTask {
                title: title.to_owned(),
                description: Some(format!("{title} details")),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: assignee.map(str::to_owned),
                priority,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".to_owned(),
            },
        )
        .context("seed task")?;
    }
    let app = test.router();

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/tasks?priority=0&priority=2&sort=title",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    let titles: Vec<_> = tasks.iter().map(|task| task["title"].clone()).collect();
    assert_eq!(titles, [json!("alpha"), json!("bravo")]);

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?sort=-assignee").await?;
    assert_eq!(status, StatusCode::OK);
    let tasks = json["data"].as_array().context("tasks array")?;
    let assignees: Vec<_> = tasks.iter().map(|task| task["assignee"].clone()).collect();
    assert_eq!(
        assignees,
        [json!("worker-b"), json!("worker-a"), Value::Null]
    );
    Ok(())
}

#[tokio::test]
async fn tasks_accepts_list_view_sort_contract_for_ref_title_and_status() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    for (title, status) in [
        ("charlie contract sort", kanban_core::TaskStatus::Ready),
        ("alpha contract sort", kanban_core::TaskStatus::Ready),
        ("bravo contract sort", kanban_core::TaskStatus::Todo),
    ] {
        let task = kanban_sqlite::api::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::api::CreateTask {
                title: title.to_owned(),
                description: Some(format!("{title} details")),
                status: Some(status),
                assignee: None,
                priority: 3,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".to_owned(),
            },
        )
        .context("seed task")?;
        if status == kanban_core::TaskStatus::Ready {
            kanban_sqlite::api::mark_execution_plan_not_required(
                &db_path,
                "default",
                "seed",
                &task.id,
                "sort fixture",
            )
            .context("mark ready not required")?;
        }
    }
    let app = test.router();

    for (sort, field, expected) in [
        ("seq", "seq", vec![json!(1), json!(2), json!(3)]),
        ("-seq", "seq", vec![json!(3), json!(2), json!(1)]),
        (
            "title",
            "title",
            vec![
                json!("alpha contract sort"),
                json!("bravo contract sort"),
                json!("charlie contract sort"),
            ],
        ),
        (
            "-title",
            "title",
            vec![
                json!("charlie contract sort"),
                json!("bravo contract sort"),
                json!("alpha contract sort"),
            ],
        ),
        (
            "status",
            "status",
            vec![json!("todo"), json!("ready"), json!("ready")],
        ),
        (
            "-status",
            "status",
            vec![json!("ready"), json!("ready"), json!("todo")],
        ),
    ] {
        let (status, json) = get_json(
            app.clone(),
            &format!("/api/v1/boards/default/tasks?sort={sort}"),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{sort}");
        let tasks = json["data"].as_array().context("tasks array")?;
        let actual: Vec<_> = tasks.iter().map(|task| task[field].clone()).collect();
        assert_eq!(actual, expected, "{sort}");
    }
    Ok(())
}

#[tokio::test]
async fn tasks_rejects_invalid_priority_filter() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?priority=9").await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("priority must be one of P0, P1, P2, P3")
    );
    Ok(())
}

#[tokio::test]
async fn tasks_rejects_unbounded_limit() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, json) = get_json(
        app,
        &format!("/api/v1/boards/default/tasks?limit={}", usize::MAX),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("value")?
            .contains("limit must be <= 1000")
    );
    Ok(())
}

#[tokio::test]
async fn tasks_gets_task_by_id() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("get by id"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = get_json(app, &format!("/api/v1/tasks/{}", task.id)).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["id"], task.id);
    assert_eq!(json["data"]["title"], "get by id");
    assert_task_dto_exposes_ui_fields_without_claim_token(&json["data"]);
    Ok(())
}

#[tokio::test]
async fn tasks_gets_ontology_summary_only_when_included() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::api::create_label(
        &db_path,
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "cli".to_owned(),
            color: None,
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("ontology summary via api"),
    )
    .context("task")?;
    let app = test.router();

    let (status, observation) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/label-ontology/observations", task.id),
        json!({
            "actor": {"name": "label-agent", "type": "agent", "agent_type": "local"},
            "agent_candidates_json": "[]",
            "suggestion_snapshot_json": "{}",
            "final_decision_json": "{}",
            "suggest_needs_new_label": false,
            "suggest_degraded": false,
            "diagnostics_json": "[]",
            "capture_fingerprint": "api-task-ontology-summary",
            "signals": [{
                "kind": "false_negative",
                "target_label_ref": "cli",
                "related_labels_json": "[]",
                "proposed_action": "add_positive_atom",
                "candidate_atom": {
                    "polarity": "positive",
                    "kind": "applies_when",
                    "text": "changes task detail ontology summary"
                },
                "proposal_json": "{}",
                "agent_selected": true,
                "suggest_state": "candidate",
                "suggest_score": 0.22,
                "suggest_rank": 2,
                "final_selected": true,
                "rationale": "Task detail should expose ontology summary.",
                "signal_key": "api-task-ontology-summary"
            }]
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{observation}");
    let signal_id = observation["data"]["signals"][0]["id"]
        .as_str()
        .context("signal id")?;

    let (status, json) = get_json(app.clone(), &format!("/api/v1/tasks/{}", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(json.get("meta").is_none());

    let (status, json) =
        get_json(app, &format!("/api/v1/tasks/{}?include=ontology", task.id)).await?;
    assert_eq!(status, StatusCode::OK, "{json}");
    let summary = &json["meta"]["details"]["ontology_summary"];
    assert_eq!(summary["signal_count"], 1);
    assert_eq!(summary["open_count"], 1);
    assert_eq!(summary["sample_signals"][0]["id"], signal_id);
    assert_eq!(
        summary["sample_signals"][0]["proposed_action"],
        "add_positive_atom"
    );
    Ok(())
}

#[tokio::test]
async fn tasks_returns_error_envelope_for_json_and_query_extractor_errors() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    for (status, json) in [
        request_raw_json(app.clone(), "POST", "/api/v1/boards/default/tasks", "{").await?,
        post_json(
            app.clone(),
            "/api/v1/boards/default/tasks",
            json!({"description":"missing title"}),
        )
        .await?,
        post_json(
            app.clone(),
            "/api/v1/boards/default/tasks",
            json!({"title":"bad priority","priority":"high"}),
        )
        .await?,
        get_json(app, "/api/v1/boards/default/tasks?status=bogus").await?,
    ] {
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], "invalid_input");
        assert!(json["error"]["message"].is_string());
    }
    Ok(())
}

#[tokio::test]
async fn tasks_rejects_priority_outside_p0_p3() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/tasks",
        json!({"title":"bad priority","description":"ready spec","priority":70}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert_eq!(
        json["error"]["message"],
        "invalid input: priority must be one of P0, P1, P2, P3"
    );

    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("valid priority"),
    )?;
    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{}", task.id),
        json!({"priority": -1}),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert_eq!(
        json["error"]["message"],
        "invalid input: priority must be one of P0, P1, P2, P3"
    );
    Ok(())
}

#[tokio::test]
async fn tasks_patches_editable_fields_and_uses_header_actor_when_body_actor_absent()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("before update"),
    )
    .context("task")?;
    kanban_sqlite::api::mark_execution_plan_not_required(
        &db_path,
        "default",
        "seed",
        &task.id,
        "patch fixture",
    )
    .context("mark not required")?;
    let task =
        kanban_sqlite::api::get_task(&db_path, "default", &task.id).context("task after plan")?;
    let app = test.router();

    let (status, json) = patch_json(
        app.clone(),
        &format!("/api/v1/tasks/{}", task.id),
        json!({
            "title": "after update",
            "description": null,
            "assignee": "worker-b",
            "priority": 2,
            "due_at": future_epoch_ms()?,
            "metadata": {"updated": true},
            "expected_lock_version": task.lock_version
        }),
        Some("header-actor"),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["title"], "after update");
    assert_eq!(json["data"]["description"], Value::Null);
    assert_eq!(json["data"]["assignee"], "worker-b");
    assert_eq!(json["data"]["priority"], 2);
    assert_eq!(json["data"]["metadata_json"], r#"{"updated":true}"#);
    assert_eq!(json["data"]["status"], "triage");

    let events =
        kanban_sqlite::api::list_events(&db_path, "default", Some(&task.id)).context("events")?;
    assert_eq!(events.last().context("updated event")?.kind, "task.updated");
    assert_eq!(
        events.last().context("updated event")?.actor.as_deref(),
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
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("lock_version mismatch")
    );
    Ok(())
}

#[tokio::test]
async fn tasks_patch_rejects_forbidden_status_and_claim_fields() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("reject forbidden"),
    )
    .context("task")?;
    let app = test.router();

    for forbidden in ["status", "claim_token", "completed_at", "current_run_id"] {
        let (status, json) = patch_json(
            app.clone(),
            &format!("/api/v1/tasks/{}", task.id),
            json!({ forbidden: "bad" }),
            None,
        )
        .await?;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{forbidden} must be rejected"
        );
        assert_eq!(json["error"]["code"], "invalid_input");
    }
    Ok(())
}

#[tokio::test]
async fn tasks_patch_rejects_unknown_fields() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("reject unknown"),
    )
    .context("task")?;
    let app = test.router();

    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{}", task.id),
        json!({ "unexpected": true }),
        None,
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    Ok(())
}

#[tokio::test]
async fn tasks_patch_future_scheduled_at_recomputes_status_to_scheduled() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("schedule me"),
    )
    .context("task")?;
    let app = test.router();
    let scheduled_at = future_epoch_ms()?;

    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{}", task.id),
        json!({ "scheduled_at": scheduled_at }),
        None,
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["scheduled_at"], scheduled_at);
    assert_eq!(json["data"]["status"], "scheduled");
    Ok(())
}

#[tokio::test]
async fn tasks_patch_with_invalid_max_retries_rolls_back_task_and_events() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("before invalid retry patch"),
    )
    .context("task")?;
    let before_events = kanban_sqlite::api::list_events(&db_path, "default", Some(&task.id))?.len();
    let app = test.router();

    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{}", task.id),
        json!({
            "title": "after invalid retry patch",
            "max_retries": 0
        }),
        None,
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    let fresh = kanban_sqlite::api::get_task(&db_path, "default", &task.id)?;
    assert_eq!(fresh.title, "before invalid retry patch");
    assert_eq!(fresh.lock_version, task.lock_version);
    let after_events = kanban_sqlite::api::list_events(&db_path, "default", Some(&task.id))?.len();
    assert_eq!(
        after_events, before_events,
        "invalid patch must not write events"
    );
    Ok(())
}

#[tokio::test]
async fn task_accepts_retry_policy_on_create_and_patch() -> anyhow::Result<()> {
    let test = TestApp::new()?;
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
    .await?;

    assert_eq!(status, StatusCode::CREATED);
    let task_id = json["data"]["id"].as_str().context("value")?;
    assert_eq!(json["data"]["max_retries"], 2);

    let (status, json) = patch_json(
        app.clone(),
        &format!("/api/v1/tasks/{task_id}"),
        json!({"max_retries":null}),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"]["max_retries"].is_null());

    let (status, json) = patch_json(
        app,
        &format!("/api/v1/tasks/{task_id}"),
        json!({"max_retries":1}),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["max_retries"], 1);
    Ok(())
}
