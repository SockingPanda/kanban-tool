use crate::{error::ApiError, state::AppState};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use kanban_protocol::{
    ListEventsQuery, ListEventsResponse, NextAfterMeta, StreamEventData,
    event_payload::EventPayload,
};
use kanban_service::KanbanError;
use kanban_service::{EventListOptions as ApplicationEventListOptions, EventRecord};
use serde_json::Value;

pub(crate) async fn list_events(
    State(state): State<AppState>,
    query: Result<Query<ListEventsQuery>, QueryRejection>,
) -> Result<Json<ListEventsResponse>, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("query 无效：{error}")))?;
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
    let data = page
        .events
        .into_iter()
        .map(api_event)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(ListEventsResponse::new(
        data,
        NextAfterMeta {
            next_after: page.next_after,
        },
    )))
}

pub(crate) fn api_event(event: EventRecord) -> Result<StreamEventData, ApiError> {
    let payload_value: Value = serde_json::from_str(&event.payload_json).map_err(|error| {
        KanbanError::Storage(format!(
            "存储的 event payload 对 {} 不是有效 JSON：{error}",
            event.event_id
        ))
    })?;
    let payload =
        EventPayload::from_kind_and_value(&event.kind, payload_value).map_err(|error| {
            KanbanError::Storage(format!(
                "存储的 event payload 对 {} 无效：{error}",
                event.kind
            ))
        })?;
    Ok(StreamEventData {
        id: event.id,
        event_id: event.event_id,
        board_id: event.board_id,
        task_id: event.task_id,
        run_id: event.run_id,
        kind: event.kind,
        actor: event.actor,
        payload,
        created_at: event.created_at,
    })
}

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v1/events", get(list_events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::operations::test_support::*;
    use kanban_protocol::{ApiErrorCode, ErrorEnvelope, event_payload::TaskStatus};

    #[tokio::test]
    async fn list_events_returns_typed_task_event_and_cursor() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/default/tasks",
                serde_json::json!({
                    "task_id": "t_event_http",
                    "title": "event",
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

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let events: ListEventsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(events.data.len(), 1);
        assert_eq!(events.data[0].kind, "task.created");
        assert_eq!(events.data[0].task_id.as_deref(), Some("t_event_http"));
        assert_eq!(events.data[0].run_id, None);
        assert_eq!(events.data[0].actor.as_deref(), Some("tester"));
        assert!(matches!(
            &events.data[0].payload,
            EventPayload::TaskStatus(payload) if payload.status == TaskStatus::Todo
        ));
        assert_eq!(events.meta.next_after, events.data[0].id);
    }

    #[tokio::test]
    async fn list_events_rejects_invalid_query_with_standard_error_envelope() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state);
        for uri in [
            "/api/v1/events?board=default&board=other",
            "/api/v1/events?unknown=1",
            "/api/v1/events?after=-1",
            "/api/v1/events?task_id=default%231",
        ] {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
            assert_eq!(error.error.code, ApiErrorCode::InvalidInput);
        }
    }

    #[test]
    fn event_mapping_preserves_unknown_payload_and_fails_closed() {
        let unknown = api_event(EventRecord {
            id: 1,
            event_id: "e_unknown".into(),
            board_id: "b_default".into(),
            task_id: None,
            run_id: None,
            kind: "future.event".into(),
            actor: None,
            payload_json: r#"{"nested":[1,{"opaque":true}]}"#.into(),
            created_at: 2,
        })
        .unwrap();
        assert_eq!(
            unknown.payload,
            EventPayload::Unknown(serde_json::json!({
                "nested": [1, {"opaque": true}]
            }))
        );

        let invalid_known = api_event(EventRecord {
            id: 2,
            event_id: "e_invalid".into(),
            board_id: "b_default".into(),
            task_id: None,
            run_id: None,
            kind: "task.created".into(),
            actor: None,
            payload_json: r#"{"status":"not-a-status"}"#.into(),
            created_at: 2,
        })
        .unwrap_err();
        assert!(matches!(invalid_known.0, KanbanError::Storage(_)));

        let malformed = api_event(EventRecord {
            id: 3,
            event_id: "e_malformed".into(),
            board_id: "b_default".into(),
            task_id: None,
            run_id: None,
            kind: "future.event".into(),
            actor: None,
            payload_json: "not-json".into(),
            created_at: 2,
        })
        .unwrap_err();
        assert!(matches!(malformed.0, KanbanError::Storage(_)));
    }
}
