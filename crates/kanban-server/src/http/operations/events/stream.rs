use axum::{
    Router,
    body::Body,
    extract::{Query, State, rejection::QueryRejection},
    http::{HeaderValue, header},
    response::Response,
    routing::get,
};
use kanban_protocol::{StreamEventData, StreamEventsQuery};
use kanban_service::EventListOptions as ApplicationEventListOptions;
use kanban_service::KanbanError;

use super::list::api_event;
use crate::{error::ApiError, state::AppState};

/// 返回当前事件表的有限 SSE 快照；游标由 `after` 查询参数控制。
pub(crate) async fn stream_events(
    State(state): State<AppState>,
    query: Result<Query<StreamEventsQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("查询参数无效：{error}")))?;
    let page = state
        .application()
        .list_events(
            &query.board,
            ApplicationEventListOptions {
                task_id: query.task_id,
                after: query.after,
                limit: query.limit,
            },
        )
        .await?;

    let mut body = String::new();
    for event in page.events {
        body.push_str(&sse_frame(api_event(event)?)?);
    }

    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok(response)
}

fn sse_frame(event: StreamEventData) -> Result<String, ApiError> {
    if event.kind.contains(['\r', '\n']) {
        return Err(KanbanError::Storage("事件类型包含非法换行符".to_owned()).into());
    }
    let data = serde_json::to_string(&event)
        .map_err(|error| KanbanError::Storage(format!("事件数据序列化失败: {error}")))?;
    let mut frame = String::new();
    frame.push_str("event: ");
    frame.push_str(&event.kind);
    frame.push_str("\nid: ");
    frame.push_str(&event.id.to_string());
    frame.push_str("\ndata: ");
    frame.push_str(&data);
    frame.push_str("\n\n");
    Ok(frame)
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        crate::http::operations::registered_path(
            kanban_protocol::HttpMethod::Get,
            "/api/v1/stream/events",
        ),
        get(stream_events),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::operations::test_support::*;
    use kanban_protocol::{
        ApiErrorCode, ErrorEnvelope, StreamEventData, event_payload::EventPayload,
    };

    fn parse_frames(body: &str) -> Vec<(i64, String, StreamEventData)> {
        assert!(body.ends_with("\n\n"));
        body.trim_end_matches('\n')
            .split("\n\n")
            .map(|frame| {
                let lines = frame.lines().collect::<Vec<_>>();
                assert_eq!(lines.len(), 3);
                let event = lines[0].strip_prefix("event: ").unwrap().to_owned();
                let id = lines[1].strip_prefix("id: ").unwrap().parse().unwrap();
                let data = serde_json::from_str(lines[2].strip_prefix("data: ").unwrap()).unwrap();
                (id, event, data)
            })
            .collect()
    }

    async fn create_task(router: &axum::Router, id: &str) {
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": id,
                    "title": "SSE task",
                    "description": null,
                    "status": "todo",
                    "assignee": null,
                    "priority": 1,
                    "scheduled_at": null,
                    "due_at": null,
                    "max_retries": 2,
                    "metadata": {},
                    "labels": [],
                    "depends_on": [],
                    "actor": "tester"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn stream_events_returns_finite_snapshot_in_event_id_data_order() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        create_task(&router, "t_sse_first").await;
        create_task(&router, "t_sse_second").await;

        let response = router
            .oneshot(
                Request::get("/api/v1/stream/events?after=1&limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/event-stream"
        );
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        let frames = parse_frames(&body);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].1, "task.created");
        assert_eq!(frames[0].2.kind, frames[0].1);
        assert_eq!(frames[0].0, frames[0].2.id);
        assert!(!body.contains(": keep-alive"));
    }

    #[tokio::test]
    async fn stream_events_ignores_last_event_id_and_filters_task() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        create_task(&router, "t_sse_filter").await;

        let response = router
            .oneshot(
                Request::get("/api/v1/stream/events?task_id=t_sse_filter&after=0&limit=1000")
                    .header("last-event-id", i64::MAX.to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        let frames = parse_frames(&body);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].2.task_id.as_deref(), Some("t_sse_filter"));
    }

    #[tokio::test]
    async fn stream_events_closes_empty_snapshot_without_heartbeat() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let response = router
            .oneshot(
                Request::get("/api/v1/stream/events?after=0&limit=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn stream_events_rejects_invalid_query_with_error_envelope() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let response = router
            .oneshot(
                Request::get("/api/v1/stream/events?after=-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::InvalidInput);
    }

    #[test]
    fn sse_frame_preserves_unknown_payload_and_rejects_field_injection() {
        let unknown = StreamEventData {
            id: 17,
            event_id: "e_unknown".into(),
            board_id: "b_default".into(),
            task_id: None,
            run_id: None,
            kind: "plugin.future.event".into(),
            actor: Some("tester".into()),
            payload: EventPayload::Unknown(serde_json::json!({
                "opaque": [1, {"nested": true}],
            })),
            created_at: 42,
        };
        let frame = sse_frame(unknown).unwrap();
        let parsed = parse_frames(&frame);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, 17);
        assert_eq!(parsed[0].1, "plugin.future.event");
        assert_eq!(
            parsed[0].2.payload,
            EventPayload::Unknown(serde_json::json!({
                "opaque": [1, {"nested": true}],
            }))
        );

        let invalid = StreamEventData {
            kind: "plugin.future\nevent".into(),
            ..parsed[0].2.clone()
        };
        assert!(sse_frame(invalid).is_err());
    }
}
