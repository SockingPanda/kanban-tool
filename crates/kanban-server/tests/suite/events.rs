use crate::common::*;

#[tokio::test]
async fn events_after_limit_returns_ordered_events_and_next_after() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let first = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("first"),
    )
    .context("first")?;
    let second = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("second"),
    )
    .context("second")?;
    let all = kanban_sqlite::api::list_events(&db_path, "default", None).context("events")?;
    let after = all
        .iter()
        .find(|event| event.task_id.as_deref() == Some(&first.id))
        .context("first task event")?
        .id;
    let app = test.router();

    let (status, json) = get_json(
        app,
        &format!("/api/v1/events?board=default&after={after}&limit=1"),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let events = json["data"].as_array().context("events array")?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["task_id"], second.id);
    assert!(events[0]["id"].as_i64().context("value")? > after);
    assert_eq!(json["meta"]["next_after"], events[0]["id"]);
    assert!(
        events[0]["event_id"]
            .as_str()
            .context("value")?
            .starts_with("e_")
    );
    assert_eq!(events[0]["kind"], "task.created");
    assert_ne!(first.id, second.id);
    Ok(())
}

#[tokio::test]
async fn events_filters_by_task_id_for_detail_timeline() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let first = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("first timeline"),
    )
    .context("first")?;
    let second = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("second timeline"),
    )
    .context("second")?;
    kanban_sqlite::api::block_task(
        &db_path,
        "default",
        "seed",
        &first.id,
        "waiting on local input",
        None,
        false,
    )
    .context("block first")?;
    let app = test.router();

    let (status, json) = get_json(
        app,
        &format!("/api/v1/events?board=default&task_id={}", first.id),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let events = json["data"].as_array().context("events array")?;
    assert!(events.len() >= 2);
    assert!(
        events
            .iter()
            .all(|event| event["task_id"].as_str() == Some(first.id.as_str()))
    );
    assert!(!events.iter().any(|event| event["task_id"] == second.id));
    assert!(events.iter().any(|event| event["kind"] == "task.blocked"));
    Ok(())
}
