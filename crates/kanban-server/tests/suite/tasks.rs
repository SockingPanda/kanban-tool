use crate::common::*;

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
    assert_eq!(task["status"], "ready");
    assert_eq!(task["assignee"], "worker-a");
    assert_eq!(task["priority"], 1);
    assert_task_dto_exposes_ui_fields_without_claim_token(task);
    assert_eq!(task["metadata_json"], r#"{"source":"test"}"#);

    let events = kanban_sqlite::list_events(
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
    let deps = kanban_sqlite::list_dependencies(&db_path, "default", child_id).context("deps")?;
    assert!(
        deps.iter()
            .any(|(parent_id, _child_id)| parent_id == &parent.id)
    );
    let events =
        kanban_sqlite::list_events(&db_path, "default", Some(child_id)).context("events")?;
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
    let tasks = kanban_sqlite::list_tasks(&db_path, "default", &[], false).context("tasks")?;
    assert!(tasks.iter().all(|task| task.title != "must not persist"));
    Ok(())
}

#[tokio::test]
async fn tasks_create_with_invalid_max_retries_rolls_back_task_and_events() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let before_events = kanban_sqlite::list_events(&db_path, "default", None)?.len();
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
    let tasks = kanban_sqlite::list_tasks(&db_path, "default", &[], false).context("tasks")?;
    assert!(
        tasks
            .iter()
            .all(|task| task.title != "invalid retry create"),
        "invalid create must not persist task"
    );
    let after_events = kanban_sqlite::list_events(&db_path, "default", None)?.len();
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
    let valid_parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("valid parent"),
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
    let tasks = kanban_sqlite::list_tasks(&db_path, "default", &[], false).context("tasks")?;
    let child = tasks.iter().find(|task| task.title == "partial child");
    assert!(child.is_none(), "failed create must roll back child task");
    let deps =
        kanban_sqlite::list_dependencies(&db_path, "default", &valid_parent.id).context("deps")?;
    assert!(
        deps.is_empty(),
        "failed create must roll back prior dependency edge"
    );
    Ok(())
}

#[tokio::test]
async fn tasks_create_accepts_labels_and_exposes_task_label_dto() -> anyhow::Result<()> {
    let test = TestApp::new()?;
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
    Ok(())
}

#[tokio::test]
async fn tasks_lists_non_archived_by_default_and_includes_archived_on_query() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let visible = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("visible task"),
    )
    .context("visible task")?;
    let archived = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("archived task"),
    )
    .context("archived task")?;
    kanban_sqlite::archive_task(&db_path, "default", "seed", &archived.id, false)
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
    let ready = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("ready task"),
    )
    .context("ready task")?;
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
    let ready = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("ready task"),
    )
    .context("ready task")?;
    let running = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("running task"),
    )
    .context("running task")?;
    kanban_sqlite::claim_task(&db_path, "default", "seed", &running.id, 60_000)
        .context("claim task")?;
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
    let oldest = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("oldest update"),
    )
    .context("oldest task")?;
    let newest = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("newest update"),
    )
    .context("newest task")?;
    let middle = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("middle update"),
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
    for (title, assignee, priority, labels) in [
        ("alpha bug", Some("alice"), 1, vec!["backend".to_owned()]),
        ("beta bug", Some("alice"), 3, vec!["backend".to_owned()]),
        ("alpha chore", Some("bob"), 2, vec!["frontend".to_owned()]),
    ] {
        kanban_sqlite::create_task_with_labels(
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
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("label route target"),
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

    let (status, json) =
        get_json(app.clone(), &format!("/api/v1/tasks/{}/labels", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"][0]["name"], "backend");

    let (status, json) =
        delete_json(app, &format!("/api/v1/tasks/{}/labels/{label_id}", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json["data"]["labels"]
            .as_array()
            .context("task labels")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn task_label_routes_use_task_board_and_reject_archived_targets() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::create_board(
        &db_path,
        "seed",
        kanban_sqlite::CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;
    let other_task = kanban_sqlite::create_task(
        &db_path,
        "other",
        "seed",
        kanban_sqlite::CreateTask::ready("non-default route target"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", other_task.id),
        json!({ "name": "backend", "actor": "api-labeler" }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["board_slug"], "other");
    assert_eq!(json["data"]["labels"][0]["name"], "backend");
    assert!(kanban_sqlite::list_labels(&db_path, "default")?.is_empty());
    let other_labels = kanban_sqlite::list_labels(&db_path, "other")?;
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

    let archived_task = kanban_sqlite::create_task(
        &db_path,
        "other",
        "seed",
        kanban_sqlite::CreateTask::ready("archived route target"),
    )?;
    kanban_sqlite::archive_task(&db_path, "other", "seed", &archived_task.id, false)?;
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", archived_task.id),
        json!({ "name": "blocked" }),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");

    kanban_sqlite::archive_board(&db_path, "other", "seed")?;
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
    let first = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("first api list task"),
    )
    .context("seed first task")?;
    let _second = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("title mentions 1 but must not match numeric search"),
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
        kanban_sqlite::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::CreateTask {
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
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("get by id"),
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

    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("valid priority"),
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
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("before update"),
    )
    .context("task")?;
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
        kanban_sqlite::list_events(&db_path, "default", Some(&task.id)).context("events")?;
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
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("reject forbidden"),
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
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("reject unknown"),
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
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("schedule me"),
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
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("before invalid retry patch"),
    )
    .context("task")?;
    let before_events = kanban_sqlite::list_events(&db_path, "default", Some(&task.id))?.len();
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
    let fresh = kanban_sqlite::get_task(&db_path, "default", &task.id)?;
    assert_eq!(fresh.title, "before invalid retry patch");
    assert_eq!(fresh.lock_version, task.lock_version);
    let after_events = kanban_sqlite::list_events(&db_path, "default", Some(&task.id))?.len();
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
