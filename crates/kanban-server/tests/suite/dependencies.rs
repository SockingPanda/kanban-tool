use crate::common::*;

#[tokio::test]
async fn dependencies_add_remove_list_and_cycle_error() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("parent"),
    )
    .context("parent")?;
    let child = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("child"),
    )
    .context("child")?;
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", child.id),
        json!({"parent_task_id":parent.id,"actor":"dep-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["parents"][0]["id"], parent.id);
    assert_eq!(
        json["data"]["children"].as_array().context("value")?.len(),
        0
    );

    let event_count_before_duplicate =
        kanban_sqlite::list_events(&db_path, "default", Some(&child.id))?
            .into_iter()
            .filter(|event| event.kind == "dependency.added")
            .count();
    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", child.id),
        json!({"parent_task_id":parent.id,"actor":"dep-actor"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["parents"][0]["id"], parent.id);
    let event_count_after_duplicate =
        kanban_sqlite::list_events(&db_path, "default", Some(&child.id))?
            .into_iter()
            .filter(|event| event.kind == "dependency.added")
            .count();
    assert_eq!(event_count_after_duplicate, event_count_before_duplicate);

    let (status, json) = get_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", parent.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["children"][0]["id"], child.id);

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", parent.id),
        json!({"parent_task_id":child.id}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "dependency_cycle");

    let (status, json) = delete_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies/{}", child.id, parent.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json["data"]["parents"]
            .as_array()
            .context("value")?
            .is_empty()
    );
    Ok(())
}

#[tokio::test]
async fn dependency_add_rejects_unknown_json_fields() -> anyhow::Result<()> {
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
    let app = test.router();

    let (status, json) = post_json(
        app,
        &format!("/api/v1/tasks/{}/dependencies", child.id),
        json!({"parent_task_id":parent.id,"parent_task":"typo"}),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    Ok(())
}
