use crate::common::*;

#[tokio::test]
async fn subtasks_create_list_update_delete_and_mark_not_required() -> anyhow::Result<()> {
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
        &format!("/api/v1/tasks/{}/subtasks", parent.id),
        json!({"title":"child","description":"child spec","priority":2,"required":true,"position":2048,"actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["task_id"], parent.id);
    assert_eq!(json["data"]["execution_plan"]["state"], "planned");
    assert_eq!(json["data"]["subtasks"][0]["parent_task_id"], parent.id);
    assert_eq!(json["data"]["subtasks"][0]["child_task"]["title"], "child");
    assert_eq!(json["data"]["subtasks"][0]["child_task"]["priority"], 2);
    assert_eq!(json["data"]["subtasks"][0]["position"], 2048);
    assert_eq!(json["data"]["subtasks"][0]["required"], true);
    assert!(
        json["data"]["subtasks"][0]["child_task"]
            .get("claim_token")
            .is_none()
    );
    let child_id = json["data"]["subtasks"][0]["child_task"]["id"]
        .as_str()
        .context("child id")?
        .to_owned();

    let (status, json) = get_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/subtasks", parent.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["subtasks"][0]["child_task"]["id"], child_id);

    let (status, json) = patch_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/subtasks/{}", parent.id, child_id),
        json!({"position":4096,"required":false,"actor":"api-actor"}),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["subtasks"][0]["position"], 4096);
    assert_eq!(json["data"]["subtasks"][0]["required"], false);
    assert_eq!(json["data"]["execution_plan"]["state"], "unplanned");

    let (status, json) = delete_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/subtasks/{}", parent.id, child_id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json["data"]["subtasks"]
            .as_array()
            .context("subtasks")?
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
async fn subtask_attach_errors_use_unified_envelope() -> anyhow::Result<()> {
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
        &format!("/api/v1/tasks/{}/subtasks/attach", parent.id),
        json!({"child_task_id":"missing","actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/subtasks/attach", parent.id),
        json!({"child_task_id":other.id,"actor":"api-actor"}),
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

    let (status, _json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/subtasks/attach", parent.id),
        json!({"child_task_id":child.id,"actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/subtasks/attach", child.id),
        json!({"child_task_id":parent.id,"actor":"api-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("cycle")
    );
    Ok(())
}
