use crate::common::*;
#[test]
fn list_events_response_contract_root_has_exact_api_envelope() -> anyhow::Result<()> {
    let value = serde_json::to_value(kanban_contract::ListEventsResponse::new(
        vec![kanban_contract::StreamEventData {
            id: 42,
            event_id: "e_fixture".to_owned(),
            board_id: "fixture-board".to_owned(),
            task_id: Some("t_fixture".to_owned()),
            run_id: None,
            kind: "task.created".to_owned(),
            actor: Some("fixture-actor".to_owned()),
            payload: kanban_contract::event_payload::EventPayload::from_kind_and_value(
                "task.created",
                serde_json::json!({"status":"todo"}),
            )?,
            created_at: 1700000000,
        }],
        kanban_contract::NextAfterMeta { next_after: 42 },
    ))?;
    assert_eq!(value["meta"]["next_after"], 42);
    assert_eq!(value["data"][0]["event_id"], "e_fixture");
    Ok(())
}

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
        app.clone(),
        &format!("/api/v1/events?board=default&after={after}&limit=1"),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let top_level = json.as_object().context("events response envelope")?;
    assert_eq!(top_level.len(), 2);
    assert!(top_level.contains_key("data"));
    assert!(top_level.contains_key("meta"));
    let envelope: kanban_contract::MetadataEnvelope<Value, kanban_contract::NextAfterMeta> =
        serde_json::from_value(json.clone()).context("events metadata envelope")?;
    let next_after = envelope.meta.next_after;
    let events = json["data"].as_array().context("events array")?;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["task_id"], second.id);
    assert!(events[0]["id"].as_i64().context("value")? > after);
    assert_eq!(next_after, events[0]["id"].as_i64().context("event id")?);
    assert_eq!(json["meta"]["next_after"], events[0]["id"]);
    assert!(
        events[0]["event_id"]
            .as_str()
            .context("value")?
            .starts_with("e_")
    );
    assert_eq!(events[0]["kind"], "task.created");
    assert_ne!(first.id, second.id);

    let (status, json) = get_json(
        app,
        &format!("/api/v1/events?board=default&after={next_after}&limit=1"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let top_level = json.as_object().context("empty events response envelope")?;
    assert_eq!(top_level.len(), 2);
    assert!(top_level.contains_key("data"));
    assert!(top_level.contains_key("meta"));
    let envelope: kanban_contract::MetadataEnvelope<Value, kanban_contract::NextAfterMeta> =
        serde_json::from_value(json.clone()).context("empty events metadata envelope")?;
    assert_eq!(envelope.meta.next_after, next_after);
    assert!(
        json["data"]
            .as_array()
            .context("empty events array")?
            .is_empty()
    );
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

#[test]
fn list_events_response_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    let fixture =
        include_str!("../../../../schemas/fixtures/api/list-events-response.v1.valid.json");
    let response: kanban_contract::ListEventsResponse = serde_json::from_str(fixture)?;
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].event_id, "e_fixture");
    assert_eq!(serde_json::to_string(&response)?, fixture.trim());
    Ok(())
}

#[tokio::test]
async fn list_events_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "events-fixture-actor",
        kanban_sqlite::api::CreateBoard {
            slug: "events-project".to_owned(),
            name: "Events Project".to_owned(),
            description: Some("non-default events contract board".to_owned()),
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "events-project",
        "events-fixture-actor",
        kanban_sqlite::api::CreateTask::ready("events fixture task"),
    )?;
    let (status, json) = get_json(
        test.router(),
        &format!("/api/v1/events?board=events-project&task_id={}", task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let response: kanban_contract::ListEventsResponse = serde_json::from_value(json)?;
    assert_eq!(response.data.len(), 1);
    assert!(response.data[0].board_id.starts_with("b_"));
    assert_eq!(response.data[0].task_id.as_deref(), Some(task.id.as_str()));
    assert_eq!(response.data[0].kind, "task.created");
    assert!(response.meta.next_after > 0);
    Ok(())
}

#[test]
fn list_events_handler_has_exact_api_response_ownership() -> anyhow::Result<()> {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/handlers/events.rs"
    ))?;
    let handler = source
        .split("pub(crate) async fn list_events")
        .nth(1)
        .and_then(|rest| rest.split("pub(crate) async fn stream_events").next())
        .context("list_events handler")?;
    assert!(source.contains("pub(crate) async fn list_events"));
    assert!(handler.contains("Json<ListEventsResponse>"));
    assert!(handler.contains("ListEventsResponse::new"));
    assert!(!handler.contains("MetadataEnvelope"));
    assert!(!handler.contains("StreamEventData"));
    Ok(())
}
