use crate::common::*;

#[tokio::test]
async fn boards_lists_and_shows_default_board() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, json) = get_json(app.clone(), "/api/v1/boards").await?;
    assert_eq!(status, StatusCode::OK);
    let boards = json["data"].as_array().context("boards array")?;
    assert_eq!(boards.len(), 1);
    let board_id = boards[0]["id"].as_str().context("board id")?;
    assert!(board_id.starts_with("b_"));
    assert_eq!(boards[0]["slug"], "default");
    assert_eq!(boards[0]["name"], "Default");
    assert_eq!(boards[0]["description"], Value::Null);
    assert!(boards[0]["created_at"].is_i64());
    assert!(boards[0]["updated_at"].is_i64());
    assert_eq!(boards[0]["archived_at"], Value::Null);

    let (status, by_slug) = get_json(app.clone(), "/api/v1/boards/default").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(by_slug["data"]["id"], board_id);
    assert_eq!(by_slug["data"]["slug"], "default");

    let by_id_uri = format!("/api/v1/boards/{board_id}");
    let (status, by_id) = get_json(app, &by_id_uri).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(by_id["data"]["id"], board_id);
    assert_eq!(by_id["data"]["slug"], "default");
    Ok(())
}

#[tokio::test]
async fn boards_init_uses_custom_default_actor_for_seed_events() -> anyhow::Result<()> {
    let test = TestApp::with_actor("custom-init-actor")?;
    let events = kanban_sqlite::list_events(test.db_path(), "default", None)?;

    assert_eq!(events[0].kind, "board.created");
    assert_eq!(events[0].actor.as_deref(), Some("custom-init-actor"));
    Ok(())
}

#[tokio::test]
async fn boards_creates_and_archives_board() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let app = test.router();

    let (status, created) = post_json(
        app.clone(),
        "/api/v1/boards",
        json!({
            "slug": "project",
            "name": "Project Board",
            "description": "Local project",
            "actor": "api-user"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["data"]["slug"], "project");
    assert_eq!(created["data"]["name"], "Project Board");
    assert_eq!(created["data"]["description"], "Local project");

    let events = kanban_sqlite::list_events(&db_path, "project", None).context("events")?;
    assert_eq!(events[0].kind, "board.created");
    assert_eq!(events[0].actor.as_deref(), Some("api-user"));

    let (status, archived) = post_json(
        app.clone(),
        "/api/v1/boards/project/archive",
        json!({"actor": "api-user"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(archived["data"]["archived_at"].is_i64());

    let (status, list) = get_json(app.clone(), "/api/v1/boards").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["data"].as_array().context("value")?.len(), 1);

    let (status, _task_error) = post_json(
        app,
        "/api/v1/boards/project/tasks",
        json!({"title": "should reject", "description": "ready spec"}),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn boards_archive_rejects_running_work() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::create_board(
        &db_path,
        "api-test",
        kanban_sqlite::CreateBoard {
            slug: "busy".into(),
            name: "Busy".into(),
            description: None,
        },
    )
    .context("board")?;
    let task = kanban_sqlite::create_task(
        &db_path,
        "busy",
        "seed",
        kanban_sqlite::CreateTask::ready("running task"),
    )
    .context("task")?;
    kanban_sqlite::claim_task(&db_path, "busy", "worker", &task.id, 60_000).context("claim")?;
    let app = test.router();

    let (status, json) = post_json(app, "/api/v1/boards/busy/archive", json!({})).await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "invalid_transition");
    assert!(
        kanban_sqlite::get_board(&db_path, "busy")
            .context("board")?
            .archived_at
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn boards_duplicate_slug_returns_invalid_input() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, _created) = post_json(
        app.clone(),
        "/api/v1/boards",
        json!({
            "slug": "project",
            "name": "Project Board"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);

    let (status, json) = post_json(
        app,
        "/api/v1/boards",
        json!({
            "slug": "project",
            "name": "Duplicate Project Board"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("value")?
            .contains("board slug already exists")
    );
    Ok(())
}

#[tokio::test]
async fn task_dto_includes_board_slug_and_ref() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::create_board(
        &db_path,
        "api-test",
        kanban_sqlite::CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )
    .context("create board")?;
    let app = test.router();

    let (status, json) = post_json(
        app,
        "/api/v1/boards/project/tasks",
        json!({"title": "api project task", "description": "ready spec"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["board_slug"], "project");
    assert_eq!(json["data"]["ref"], "project#1");
    Ok(())
}

#[tokio::test]
async fn board_columns_lists_default_columns_in_position_order() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/boards/default/columns").await?;

    assert_eq!(status, StatusCode::OK);
    let columns = json["data"].as_array().context("columns array")?;
    let statuses: Vec<_> = columns
        .iter()
        .map(|column| column["status"].as_str().context("status"))
        .collect::<Result<_>>()?;
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
    Ok(())
}

#[tokio::test]
async fn archived_board_history_apis_remain_readable() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    kanban_sqlite::create_board(
        &db_path,
        "api-test",
        kanban_sqlite::CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )
    .context("board")?;
    let task = kanban_sqlite::create_task(
        &db_path,
        "project",
        "seed",
        kanban_sqlite::CreateTask::ready("archived history"),
    )
    .context("task")?;
    kanban_sqlite::create_comment(&db_path, &task.id, "operator", "history note", None)
        .context("comment")?;
    let claim = kanban_sqlite::claim_task(&db_path, "project", "worker", &task.id, 60_000)
        .context("claim")?;
    kanban_sqlite::complete_task(
        &db_path,
        "project",
        "worker",
        &task.id,
        Some(&claim.claim_token),
        false,
    )
    .context("complete")?;
    kanban_sqlite::archive_board(&db_path, "project", "api-test").context("archive board")?;
    let app = test.router();

    let (status, _comment_error) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/comments", task.id),
        json!({"author":"operator","body":"late write"}),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _specify_error) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/transitions/specify", task.id),
        json!({"description":"late spec"}),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, events) = get_json(
        app.clone(),
        &format!("/api/v1/events?board=project&task_id={}", task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        events["data"]
            .as_array()
            .context("value")?
            .iter()
            .any(|event| event["kind"] == "task.comment.created")
    );

    let (status, runs) = get_json(app.clone(), &format!("/api/v1/tasks/{}/runs", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(runs["data"][0]["id"], claim.run_id);

    let (status, comments) = get_json(app, &format!("/api/v1/tasks/{}/comments", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comments["data"][0]["body"], "history note");
    Ok(())
}
