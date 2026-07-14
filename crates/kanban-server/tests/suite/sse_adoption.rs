use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::common::*;

#[derive(Debug)]
struct ParsedSseFrame {
    id: i64,
    event: String,
    data: kanban_contract::StreamEventData,
}

fn parse_finite_sse(body: &str) -> anyhow::Result<Vec<ParsedSseFrame>> {
    if !body.ends_with("\n\n") {
        anyhow::bail!("finite SSE body did not close on a frame boundary");
    }
    body.trim_end_matches('\n')
        .split("\n\n")
        .map(|frame| {
            let lines = frame.lines().collect::<Vec<_>>();
            if lines.len() != 3 {
                anyhow::bail!("expected exactly event/id/data, got {lines:?}");
            }
            let event = lines[0]
                .strip_prefix("event: ")
                .context("event must be the first frame field")?
                .to_owned();
            let id = lines[1]
                .strip_prefix("id: ")
                .context("id must be the second frame field")?
                .parse::<i64>()?;
            let data = serde_json::from_str::<kanban_contract::StreamEventData>(
                lines[2]
                    .strip_prefix("data: ")
                    .context("data must be the third frame field")?,
            )?;
            if data.id != id || data.kind != event {
                anyhow::bail!(
                    "frame fields do not bind exact data: event={event} id={id} data={data:?}"
                );
            }
            Ok(ParsedSseFrame { id, event, data })
        })
        .collect()
}

#[test]
fn stream_events_query_dto_serializes_to_committed_fixture() -> anyhow::Result<()> {
    let value = serde_json::to_value(kanban_contract::StreamEventsQuery {
        board: "fixture-board".to_owned(),
        task_id: Some("t_fixture".to_owned()),
        after: 41,
        limit: 7,
    })?;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/sse/stream-events-query.v1.valid.json"
    ))?;
    assert_eq!(value, fixture);
    Ok(())
}

#[test]
fn stream_event_data_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    let fixture = include_str!("../../../../schemas/fixtures/sse/stream-event-data.v1.valid.json");
    let event: kanban_contract::StreamEventData = serde_json::from_str(fixture)?;
    assert_eq!(event.id, 42);
    assert_eq!(event.kind, "task.created");
    assert_eq!(serde_json::to_string(&event)?, fixture.trim());
    Ok(())
}

#[tokio::test]
async fn stream_events_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let fixture: kanban_contract::StreamEventsQuery = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/sse/stream-events-query.v1.valid.json"
    ))?;
    let uri = format!(
        "/api/v1/stream/events?board={}&task_id={}&after={}&limit={}",
        fixture.board,
        fixture.task_id.as_deref().context("task_id")?,
        fixture.after,
        fixture.limit
    );
    let response = test
        .router()
        .oneshot(Request::get(uri).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn stream_event_data_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture-board-actor",
        kanban_sqlite::api::CreateBoard {
            slug: "sse-project".to_owned(),
            name: "SSE Project".to_owned(),
            description: Some("non-default SSE contract board".to_owned()),
        },
    )?;
    let after = kanban_sqlite::api::list_events(test.db_path(), "sse-project", None)?
        .into_iter()
        .map(|event| event.id)
        .max()
        .context("board creation cursor")?;
    let first = kanban_sqlite::api::create_task(
        test.db_path(),
        "sse-project",
        "fixture-actor",
        kanban_sqlite::api::CreateTask::ready("first SSE sentinel"),
    )?;
    let second = kanban_sqlite::api::create_task(
        test.db_path(),
        "sse-project",
        "second-actor",
        kanban_sqlite::api::CreateTask::ready("second SSE sentinel"),
    )?;
    let response = test
        .router()
        .oneshot(
            Request::get(format!(
                "/api/v1/stream/events?board=sse-project&after={after}&limit=2"
            ))
            .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(response.into_body().collect().await?.to_bytes().to_vec())?;
    let frames = parse_finite_sse(&body)?;
    assert_eq!(
        frames.len(),
        2,
        "after and limit must select exactly two frames: {body}"
    );
    assert!(frames[0].id > after && frames[1].id > frames[0].id);
    assert_eq!(frames[0].event, "task.created");
    assert_eq!(frames[1].event, "task.created");
    assert_eq!(frames[0].data.task_id.as_deref(), Some(first.id.as_str()));
    assert_eq!(frames[1].data.task_id.as_deref(), Some(second.id.as_str()));
    assert_eq!(frames[0].data.actor.as_deref(), Some("fixture-actor"));
    assert_eq!(frames[1].data.actor.as_deref(), Some("second-actor"));
    assert_eq!(frames[0].data.payload, json!({"status": "todo"}));
    assert_eq!(frames[1].data.payload, json!({"status": "todo"}));
    assert_eq!(frames[0].data.run_id, None);
    assert_eq!(frames[1].data.run_id, None);
    assert_eq!(frames[0].data.board_id, frames[1].data.board_id);
    assert!(
        !body.contains(": keep-alive"),
        "finite snapshot has no heartbeat: {body}"
    );
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/sse/stream-event-data.v1.valid.json"
    ))?;
    let mut normalized = serde_json::to_value(&frames[0].data)?;
    normalized["id"] = fixture["id"].clone();
    normalized["event_id"] = fixture["event_id"].clone();
    normalized["board_id"] = fixture["board_id"].clone();
    normalized["task_id"] = fixture["task_id"].clone();
    normalized["created_at"] = fixture["created_at"].clone();
    assert_eq!(normalized, fixture);

    let wrong_order = body.replacen(
        "event: task.created\nid: ",
        "id: 1\nevent: task.created\n#",
        1,
    );
    assert!(parse_finite_sse(&wrong_order).is_err());
    let wrong_event = body.replacen("event: task.created", "event: task.completed", 1);
    assert!(parse_finite_sse(&wrong_event).is_err());
    let wrong_id = body.replacen(
        &format!("id: {}", frames[0].id),
        &format!("id: {}", frames[0].id + 10_000),
        1,
    );
    assert!(parse_finite_sse(&wrong_id).is_err());
    Ok(())
}

#[tokio::test]
async fn stream_events_rejects_unknown_or_duplicate_query_and_localizes_errors()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    for uri in [
        "/api/v1/stream/events?unknown=true",
        "/api/v1/stream/events?board=default&board=other",
    ] {
        let response = test
            .router()
            .oneshot(
                Request::get(uri)
                    .header("accept-language", "zh-CN")
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let json: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        assert_eq!(json["error"]["code"], "invalid_input");
        assert!(
            json["error"]["message"]
                .as_str()
                .context("message")?
                .contains("无效")
        );
    }
    Ok(())
}

#[tokio::test]
async fn stream_events_ignores_last_event_id_and_closes_without_heartbeat() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("last event id exclusion"),
    )?;
    let response = test
        .router()
        .oneshot(
            Request::get("/api/v1/stream/events?board=default&after=0&limit=100")
                .header("last-event-id", i64::MAX.to_string())
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(response.into_body().collect().await?.to_bytes().to_vec())?;
    assert!(
        body.contains(&task.id),
        "Last-Event-ID must remain ignored: {body}"
    );
    assert!(
        !body.contains(": keep-alive"),
        "finite snapshot has no heartbeat: {body}"
    );
    Ok(())
}

fn insert_raw_event(test: &TestApp, kind: &str, payload_json: &str) -> anyhow::Result<i64> {
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute_batch("PRAGMA ignore_check_constraints = ON")?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    let event_id = format!("e_raw_{}", kind.replace('.', "_"));
    conn.execute(
        "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, ?2, NULL, NULL, ?3, 'fixture', ?4, 1)",
        (&event_id, &board_id, kind, payload_json),
    )?;
    Ok(conn.last_insert_rowid())
}

#[tokio::test]
async fn stream_events_fails_closed_for_malformed_or_wrong_known_payload() -> anyhow::Result<()> {
    for (kind, payload) in [
        ("plugin.malformed", "{not-json"),
        ("task.created", r#"{"status":"not-a-status"}"#),
    ] {
        let test = TestApp::new()?;
        let id = insert_raw_event(&test, kind, payload)?;
        let response = test
            .router()
            .oneshot(
                Request::get(format!("/api/v1/stream/events?after={}", id - 1))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(
            response.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "{kind}"
        );
        let body = response.into_body().collect().await?.to_bytes();
        let error: serde_json::Value = serde_json::from_slice(&body)?;
        assert_eq!(error["error"]["code"], "internal", "{kind}: {error}");
    }
    Ok(())
}

#[tokio::test]
async fn stream_events_preserves_valid_unknown_payload_losslessly() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let payload = json!({"opaque": [1, {"nested": true}], "version": "future"});
    let id = insert_raw_event(&test, "plugin.future.event", &payload.to_string())?;
    let response = test
        .router()
        .oneshot(
            Request::get(format!("/api/v1/stream/events?after={}", id - 1)).body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(response.into_body().collect().await?.to_bytes().to_vec())?;
    let frames = parse_finite_sse(&body)?;
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].event, "plugin.future.event");
    assert_eq!(frames[0].data.payload, payload);
    Ok(())
}

#[tokio::test]
async fn stream_events_accepts_real_reclaim_payload_with_null_max_retries() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("nullable retry payload"),
    )?;
    kanban_sqlite::api::mark_execution_plan_not_required(
        test.db_path(),
        "default",
        "fixture",
        &task.id,
        "fixture has no decomposable steps",
    )?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "fixture", &task.id, 60_000)?;
    kanban_sqlite::api::reclaim_task(test.db_path(), "default", "fixture", &task.id, true)?;

    let response = test
        .router()
        .oneshot(
            Request::get(format!(
                "/api/v1/stream/events?board=default&task_id={}&after=0&limit=100",
                task.id
            ))
            .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(response.into_body().collect().await?.to_bytes().to_vec())?;
    let reclaimed = parse_finite_sse(&body)?
        .into_iter()
        .find(|frame| frame.event == "task.reclaimed")
        .context("real task.reclaimed frame")?;
    let payload = serde_json::to_value(reclaimed.data.payload)?;
    assert_eq!(payload["retry_count"], 1);
    assert_eq!(payload["max_retries"], serde_json::Value::Null);
    assert_eq!(payload["to_status"], "ready");
    assert_eq!(payload["reason"], "force reclaimed");
    Ok(())
}
