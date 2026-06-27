use crate::common::*;

#[tokio::test]
async fn steps_create_list_update_resolve_delete_and_mark_not_required() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("parent"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps", parent.id),
        json!({"title":"Draft test strategy","body":"write cases","required":true,"position":2048,"actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["task_id"], parent.id);
    assert_eq!(json["data"]["execution_plan"]["state"], "planned");
    assert_eq!(json["data"]["steps"][0]["parent_task_id"], parent.id);
    assert_eq!(json["data"]["steps"][0]["title"], "Draft test strategy");
    assert_eq!(json["data"]["steps"][0]["body"], "write cases");
    assert_eq!(
        json["data"]["steps"][0]["linked_task"],
        serde_json::Value::Null
    );
    assert_eq!(json["data"]["steps"][0]["status"], "todo");
    assert_eq!(json["data"]["steps"][0]["position"], 2048);
    assert_eq!(json["data"]["steps"][0]["required"], true);
    let step_id = json["data"]["steps"][0]["id"]
        .as_str()
        .context("step id")?
        .to_owned();

    let (status, json) =
        get_json(app.clone(), &format!("/api/v1/tasks/{}/steps", parent.id)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["steps"][0]["id"], step_id);

    let (status, json) = patch_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{}", parent.id, step_id),
        json!({"title":"Updated strategy","position":4096,"required":false,"actor":"api-actor"}),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["steps"][0]["title"], "Updated strategy");
    assert_eq!(json["data"]["steps"][0]["position"], 4096);
    assert_eq!(json["data"]["steps"][0]["required"], false);
    assert_eq!(json["data"]["execution_plan"]["state"], "planned");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{}/done", parent.id, step_id),
        json!({"note":"covered","actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["steps"][0]["status"], "done");
    assert_eq!(json["data"]["steps"][0]["resolution_note"], "covered");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{}/reopen", parent.id, step_id),
        json!({"reason":"needs another pass","actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["steps"][0]["status"], "todo");
    assert_eq!(
        json["data"]["steps"][0]["resolution_note"],
        serde_json::Value::Null
    );

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{}/skip", parent.id, step_id),
        json!({"reason":"covered elsewhere","actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["steps"][0]["status"], "skipped");
    assert_eq!(
        json["data"]["steps"][0]["resolution_note"],
        "covered elsewhere"
    );

    let (status, json) = delete_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{}", parent.id, step_id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json["data"]["steps"]
            .as_array()
            .context("steps")?
            .is_empty()
    );
    assert_eq!(json["data"]["execution_plan"]["state"], "unplanned");

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/execution-plan/not-required", parent.id),
        json!({"reason":"small text-only cleanup","actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["state"], "not_required");
    assert_eq!(json["data"]["reason"], "small text-only cleanup");
    Ok(())
}

#[tokio::test]
async fn linked_step_errors_use_unified_envelope() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("parent"),
    )?;
    let child = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("child"),
    )?;
    kanban_sqlite::create_board(
        &db_path,
        "seed",
        kanban_sqlite::CreateBoard {
            slug: "other".to_owned(),
            name: "Other".to_owned(),
            description: None,
        },
    )?;
    let other = kanban_sqlite::create_task(
        &db_path,
        "other",
        "seed",
        kanban_sqlite::CreateTask::ready("other child"),
    )?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps", parent.id),
        json!({"title":"missing link","linked_task_id":"t_missing","actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps", parent.id),
        json!({"title":"cross board link","linked_task_id":other.id,"actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("cross-board")
    );

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps", parent.id),
        json!({"title":"self link","linked_task_id":parent.id,"actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("parent task")
    );

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/steps", parent.id),
        json!({"title":"linked normal task","linked_task_id":child.id,"actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["steps"][0]["linked_task"]["id"], child.id);
    assert_eq!(json["data"]["steps"][0]["status"], "todo");
    Ok(())
}
