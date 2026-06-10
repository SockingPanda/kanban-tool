use crate::common::*;

#[tokio::test]
async fn stream_events_sse_returns_finite_snapshot_with_id_event_and_data_frames()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let first = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("first sse"),
    )
    .context("first")?;
    let second = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("second sse"),
    )
    .context("second")?;
    let all = kanban_sqlite::list_events(&db_path, "default", None).context("events")?;
    let after = all
        .iter()
        .find(|event| event.task_id.as_deref() == Some(&first.id))
        .context("first task event")?
        .id;
    let second_event = all
        .iter()
        .find(|event| event.task_id.as_deref() == Some(&second.id))
        .context("second task event")?;
    let app = test.router();

    let (status, headers, body) = get_raw(
        app,
        &format!("/api/v1/stream/events?board=default&after={after}&limit=1"),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()
            .context("value")?
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
    Ok(())
}
