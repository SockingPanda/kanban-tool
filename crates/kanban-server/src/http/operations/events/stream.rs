use std::{collections::VecDeque, convert::Infallible, future::Future, time::Duration};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Query, State, rejection::QueryRejection},
    http::{HeaderMap, HeaderValue, header},
    response::Response,
    routing::get,
};
use futures_util::stream;
use kanban_protocol::{
    SSE_HEARTBEAT_EVENT, SseHeartbeatData, StreamEventData, StreamEventsQuery, parse_event_cursor,
    validate_event_cursor,
};
use kanban_service::EventListOptions as ApplicationEventListOptions;
use kanban_service::KanbanError;
use tokio::{sync::watch, time::Instant};

use super::list::api_event;
use crate::{error::ApiError, state::AppState};

const EVENT_PAGE_LIMIT: usize = 128;
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const EVENT_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

/// 返回持久 SSE 流；每次查询只向前读取当前 cursor 之后的事件。
pub(crate) async fn stream_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<StreamEventsQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) =
        query.map_err(|error| KanbanError::InvalidInput(format!("查询参数无效：{error}")))?;
    let after = resolve_cursor(&headers, query.after)?;
    let board = state.application().get_board(&query.board).await?;
    if board.archived_at.is_some() {
        return Err(KanbanError::NotFound(format!("看板 {}", query.board)).into());
    }
    let limit = query.limit.min(EVENT_PAGE_LIMIT);
    let initial_page = state
        .application()
        .list_events(
            &board.slug,
            ApplicationEventListOptions {
                task_id: query.task_id.clone(),
                after,
                limit,
            },
        )
        .await?;
    let initial_page_full = initial_page.events.len() == limit;
    let mut cursor = after;
    let mut pending = VecDeque::new();
    for event in initial_page.events {
        cursor = event.id;
        pending.push_back(Bytes::from(sse_frame(api_event(event)?)?));
    }
    let initial_empty = pending.is_empty();
    if initial_empty {
        pending.push_back(Bytes::from(sse_heartbeat_frame()?));
    }
    let stream_state = EventStreamState {
        state: state.clone(),
        board: board.slug,
        task_id: query.task_id,
        cursor,
        limit,
        pending,
        first_poll: false,
        catchup_immediate: initial_page_full,
        empty_heartbeat_sent: initial_empty,
        next_heartbeat: Instant::now() + EVENT_HEARTBEAT_INTERVAL,
        shutdown: state.event_stream_shutdown_receiver(),
    };
    let body_stream = stream::unfold(stream_state, |mut stream_state| async move {
        stream_state
            .next_frame()
            .await
            .map(|frame| (Ok::<Bytes, Infallible>(frame), stream_state))
    });

    let mut response = Response::new(Body::from_stream(body_stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response.headers_mut().insert(
        header::HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    Ok(response)
}

fn resolve_cursor(headers: &HeaderMap, after: i64) -> Result<i64, ApiError> {
    let values = headers.get_all("last-event-id");
    if values.iter().nth(1).is_some() {
        return Err(KanbanError::InvalidInput("Last-Event-ID 不得重复".to_owned()).into());
    }
    let Some(value) = values.iter().next() else {
        return Ok(after);
    };
    let value = value
        .to_str()
        .map_err(|_| KanbanError::InvalidInput("Last-Event-ID 必须是 UTF-8 文本".to_owned()))?;
    let header_cursor = parse_event_cursor(value).map_err(KanbanError::InvalidInput)?;
    Ok(after.max(header_cursor))
}

/// Headers are committed before the first database poll. A storage or event
/// conversion failure therefore terminates the body, allowing the client to
/// reconnect from its last received exclusive cursor instead of receiving a
/// misleading success frame.
struct EventStreamState {
    state: AppState,
    board: String,
    task_id: Option<String>,
    cursor: i64,
    limit: usize,
    pending: VecDeque<Bytes>,
    first_poll: bool,
    catchup_immediate: bool,
    empty_heartbeat_sent: bool,
    next_heartbeat: Instant,
    shutdown: watch::Receiver<bool>,
}

impl EventStreamState {
    async fn next_frame(&mut self) -> Option<Bytes> {
        loop {
            if *self.shutdown.borrow() {
                return None;
            }
            if Instant::now() >= self.next_heartbeat {
                self.next_heartbeat = Instant::now() + EVENT_HEARTBEAT_INTERVAL;
                self.empty_heartbeat_sent = true;
                return Some(Bytes::from(sse_heartbeat_frame().ok()?));
            }
            if let Some(frame) = self.pending.pop_front() {
                return Some(frame);
            }
            if !self.first_poll && !self.catchup_immediate {
                self.wait_for_poll_or_heartbeat().await?;
            }
            self.first_poll = false;
            self.catchup_immediate = false;

            let page = match self.poll_events().await {
                PollWatchdog::Shutdown => return None,
                PollWatchdog::Heartbeat => {
                    self.empty_heartbeat_sent = true;
                    return Some(Bytes::from(sse_heartbeat_frame().ok()?));
                }
                PollWatchdog::Value(page) => match page {
                    Ok(page) => page,
                    Err(error) => {
                        tracing::warn!(error = %error, "SSE 轮询在响应提交后失败，关闭连接");
                        return None;
                    }
                },
            };
            if page.events.is_empty() {
                if !self.empty_heartbeat_sent {
                    self.empty_heartbeat_sent = true;
                    self.next_heartbeat = Instant::now() + EVENT_HEARTBEAT_INTERVAL;
                    return Some(Bytes::from(sse_heartbeat_frame().ok()?));
                }
                continue;
            }
            self.catchup_immediate = page.events.len() == self.limit;
            for event in page.events {
                self.cursor = event.id;
                let event = match api_event(event) {
                    Ok(event) => event,
                    Err(error) => {
                        tracing::warn!(error = ?error, "SSE 事件转换在响应提交后失败，关闭连接");
                        return None;
                    }
                };
                let frame = match sse_frame(event) {
                    Ok(frame) => frame,
                    Err(error) => {
                        tracing::warn!(error = ?error, "SSE 事件序列化在响应提交后失败，关闭连接");
                        return None;
                    }
                };
                self.pending.push_back(Bytes::from(frame));
            }
        }
    }

    async fn poll_events(
        &mut self,
    ) -> PollWatchdog<Result<kanban_service::EventListPage, KanbanError>> {
        let state = self.state.clone();
        let board = self.board.clone();
        let task_id = self.task_id.clone();
        let after = self.cursor;
        let limit = self.limit;
        self.await_poll_with_heartbeat(async move {
            state
                .application()
                .list_events(
                    &board,
                    ApplicationEventListOptions {
                        task_id,
                        after,
                        limit,
                    },
                )
                .await
        })
        .await
    }

    async fn await_poll_with_heartbeat<F, T>(&mut self, future: F) -> PollWatchdog<T>
    where
        F: Future<Output = T>,
    {
        tokio::pin!(future);
        loop {
            let until_heartbeat = self
                .next_heartbeat
                .saturating_duration_since(Instant::now());
            tokio::select! {
                changed = self.shutdown.changed() => {
                    if changed.is_err() || *self.shutdown.borrow() {
                        return PollWatchdog::Shutdown;
                    }
                }
                value = &mut future => return PollWatchdog::Value(value),
                _ = tokio::time::sleep(until_heartbeat) => {
                    self.next_heartbeat = Instant::now() + EVENT_HEARTBEAT_INTERVAL;
                    return PollWatchdog::Heartbeat;
                }
            }
        }
    }

    async fn wait_for_poll_or_heartbeat(&mut self) -> Option<()> {
        let until_heartbeat = self
            .next_heartbeat
            .saturating_duration_since(Instant::now());
        tokio::select! {
            changed = self.shutdown.changed() => {
                if changed.is_err() || *self.shutdown.borrow() {
                    None
                } else {
                    Some(())
                }
            }
            _ = tokio::time::sleep(EVENT_POLL_INTERVAL.min(until_heartbeat)) => Some(()),
        }
    }
}

enum PollWatchdog<T> {
    Value(T),
    Heartbeat,
    Shutdown,
}

fn sse_heartbeat_frame() -> Result<String, ApiError> {
    let data = serde_json::to_string(&SseHeartbeatData::default())
        .map_err(|error| KanbanError::Storage(format!("SSE heartbeat 序列化失败: {error}")))?;
    Ok(format!("event: {SSE_HEARTBEAT_EVENT}\ndata: {data}\n\n"))
}

fn sse_frame(event: StreamEventData) -> Result<String, ApiError> {
    validate_event_cursor(event.id)
        .map_err(|message| KanbanError::Storage(format!("事件 cursor 无效：{message}")))?;
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
    use std::time::Duration;

    use super::*;
    use crate::http::operations::test_support::*;
    use kanban_protocol::{
        ApiErrorCode, ErrorEnvelope, MAX_SAFE_EVENT_CURSOR, StreamEventData,
        event_payload::EventPayload,
    };

    async fn next_frame(body: &mut Body) -> String {
        let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
            .await
            .expect("SSE frame timeout")
            .expect("SSE stream ended")
            .expect("SSE body error");
        String::from_utf8(frame.into_data().expect("SSE data frame").to_vec())
            .expect("SSE frame UTF-8")
    }

    fn parse_business_frame(frame: &str) -> (i64, String, StreamEventData) {
        assert!(frame.ends_with("\n\n"));
        let lines = frame.trim_end_matches('\n').lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        let event = lines[0].strip_prefix("event: ").unwrap().to_owned();
        let id = lines[1].strip_prefix("id: ").unwrap().parse().unwrap();
        let data = serde_json::from_str(lines[2].strip_prefix("data: ").unwrap()).unwrap();
        (id, event, data)
    }

    async fn create_task_on_board(router: &axum::Router, board: &str, id: &str) {
        let response = router
            .clone()
            .oneshot(json_request(
                &format!("/api/v1/boards/{board}/tasks"),
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

    async fn create_task(router: &axum::Router, id: &str) {
        create_task_on_board(router, "default", id).await;
    }

    async fn create_board(router: &axum::Router, slug: &str) {
        let response = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards",
                serde_json::json!({
                    "slug": slug,
                    "name": "SSE board",
                    "description": null,
                    "actor": "tester"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    async fn assert_stream_error(
        router: &axum::Router,
        uri: &str,
        status: StatusCode,
        code: ApiErrorCode,
    ) {
        let response = router
            .clone()
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{uri}");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(error.error.code, code, "{uri}");
    }

    async fn current_cursor(router: &axum::Router) -> i64 {
        let response = router
            .clone()
            .oneshot(
                Request::get("/api/v1/events?board=default&after=0&limit=1000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let events: ListEventsResponse = serde_json::from_slice(&body).unwrap();
        events.data.last().map_or(0, |event| event.id)
    }

    #[tokio::test]
    async fn stream_events_sends_catchup_then_named_heartbeat_until_shutdown() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        let after = current_cursor(&router).await;
        create_task(&router, "t_sse_first").await;
        create_task(&router, "t_sse_second").await;

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/stream/events?after={after}&limit=1"))
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
        let mut body = response.into_body();
        let first = parse_business_frame(&next_frame(&mut body).await);
        assert_eq!(first.1, "task.created");
        assert_eq!(first.2.kind, first.1);
        assert_eq!(first.0, first.2.id);

        let second = parse_business_frame(&next_frame(&mut body).await);
        assert_eq!(second.1, "task.created");
        assert!(second.0 > first.0);
        assert_eq!(second.0, second.2.id);

        let heartbeat = next_frame(&mut body).await;
        assert_eq!(heartbeat, "event: kb-heartbeat\ndata: {}\n\n");
        state.begin_event_stream_shutdown();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), body.frame())
                .await
                .expect("shutdown timeout")
                .is_none()
        );
    }

    #[tokio::test]
    async fn stream_events_paginates_more_than_one_thousand_events_without_loss() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        let after = current_cursor(&router).await;
        for index in 0..1_001 {
            create_task(&router, &format!("t_sse_page_{index:04}")).await;
        }

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/stream/events?after={after}&limit=1000"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        let mut previous = after;
        for _ in 0..1_001 {
            let (id, event, data) = parse_business_frame(&next_frame(&mut body).await);
            assert_eq!(event, "task.created");
            assert!(id > previous);
            assert_eq!(id, data.id);
            previous = id;
        }
        assert_eq!(
            next_frame(&mut body).await,
            "event: kb-heartbeat\ndata: {}\n\n"
        );
        drop(body);
        state.begin_event_stream_shutdown();
    }

    #[tokio::test(start_paused = true)]
    async fn stream_events_heartbeat_deadline_is_fifteen_seconds() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        let after = current_cursor(&router).await;
        let mut stream = EventStreamState {
            state: state.clone(),
            board: "default".to_owned(),
            task_id: None,
            cursor: after,
            limit: EVENT_PAGE_LIMIT,
            pending: VecDeque::new(),
            first_poll: true,
            catchup_immediate: false,
            empty_heartbeat_sent: false,
            next_heartbeat: Instant::now() + EVENT_HEARTBEAT_INTERVAL,
            shutdown: state.event_stream_shutdown_receiver(),
        };
        assert_eq!(
            stream.next_frame().await.unwrap(),
            Bytes::from("event: kb-heartbeat\ndata: {}\n\n")
        );

        let pending = tokio::spawn(async move { stream.next_frame().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(14_999)).await;
        tokio::task::yield_now().await;
        assert!(!pending.is_finished());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(
            pending.await.unwrap().unwrap(),
            Bytes::from("event: kb-heartbeat\ndata: {}\n\n")
        );
        state.begin_event_stream_shutdown();
    }

    #[tokio::test(start_paused = true)]
    async fn stream_events_watchdog_breaks_a_stalled_poll_at_heartbeat_deadline() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let mut stream = EventStreamState {
            state: state.clone(),
            board: "default".to_owned(),
            task_id: None,
            cursor: 0,
            limit: EVENT_PAGE_LIMIT,
            pending: VecDeque::new(),
            first_poll: false,
            catchup_immediate: false,
            empty_heartbeat_sent: false,
            next_heartbeat: Instant::now() + EVENT_HEARTBEAT_INTERVAL,
            shutdown: state.event_stream_shutdown_receiver(),
        };
        let pending = tokio::spawn(async move {
            stream
                .await_poll_with_heartbeat(tokio::time::sleep(Duration::from_secs(60)))
                .await
        });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_millis(14_999)).await;
        tokio::task::yield_now().await;
        assert!(!pending.is_finished());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(matches!(pending.await.unwrap(), PollWatchdog::Heartbeat));
        state.begin_event_stream_shutdown();
    }

    #[tokio::test]
    async fn stream_events_uses_newer_cursor_from_query_or_last_event_id() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        let after = current_cursor(&router).await;
        create_task(&router, "t_sse_filter").await;
        create_task(&router, "t_sse_filter_second").await;

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/stream/events?after={after}&limit=1000"))
                    .header("last-event-id", (after + 1).to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        let first = parse_business_frame(&next_frame(&mut body).await);
        assert_eq!(first.2.task_id.as_deref(), Some("t_sse_filter_second"));
        assert_eq!(first.0, after + 2);
        drop(body);

        let response = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/stream/events?after={}&limit=1000",
                    after + 2
                ))
                .header("last-event-id", (after + 1).to_string())
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        assert_eq!(
            next_frame(&mut body).await,
            "event: kb-heartbeat\ndata: {}\n\n"
        );
        drop(body);
        state.begin_event_stream_shutdown();
    }

    #[tokio::test]
    async fn stream_events_filters_task_events_before_heartbeat() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        let after = current_cursor(&router).await;
        create_task(&router, "t_sse_filter_other").await;
        create_task(&router, "t_sse_filter_target").await;

        let response = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/stream/events?task_id=t_sse_filter_target&after={after}&limit=1000"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        let target = parse_business_frame(&next_frame(&mut body).await);
        assert_eq!(target.2.task_id.as_deref(), Some("t_sse_filter_target"));
        assert_eq!(
            next_frame(&mut body).await,
            "event: kb-heartbeat\ndata: {}\n\n"
        );
        drop(body);
        state.begin_event_stream_shutdown();
    }

    #[tokio::test]
    async fn stream_events_keeps_global_id_gaps_ordered_and_board_isolated() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        let after = current_cursor(&router).await;
        create_board(&router, "sse-gap-other").await;
        create_task_on_board(&router, "sse-gap-other", "t_sse_gap_other").await;
        create_task(&router, "t_sse_gap_default").await;

        let response = router
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/stream/events?board=default&after={after}&limit=1"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        let frame = parse_business_frame(&next_frame(&mut body).await);
        assert!(frame.0 > after);
        assert_eq!(frame.2.board_id, "b_default");
        assert_eq!(frame.2.task_id.as_deref(), Some("t_sse_gap_default"));
        assert_eq!(
            next_frame(&mut body).await,
            "event: kb-heartbeat\ndata: {}\n\n"
        );
        drop(body);
        state.begin_event_stream_shutdown();
    }

    #[tokio::test]
    async fn stream_events_replays_events_after_disconnect() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        let after = current_cursor(&router).await;
        create_task(&router, "t_sse_reconnect_first").await;

        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/stream/events?after={after}&limit=100"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        let first = parse_business_frame(&next_frame(&mut body).await);
        let cursor = first.0;
        drop(body);

        create_task(&router, "t_sse_reconnect_second").await;
        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/stream/events?after={cursor}&limit=100"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let mut body = response.into_body();
        let replay = parse_business_frame(&next_frame(&mut body).await);
        assert!(replay.0 > cursor);
        assert_eq!(replay.2.task_id.as_deref(), Some("t_sse_reconnect_second"));
        drop(body);
        state.begin_event_stream_shutdown();
    }

    #[tokio::test]
    async fn stream_events_transitions_from_empty_heartbeat_to_concurrent_event() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        let after = current_cursor(&router).await;
        let response = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/stream/events?after={after}&limit=1000"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.into_body();
        assert_eq!(
            next_frame(&mut body).await,
            "event: kb-heartbeat\ndata: {}\n\n"
        );
        create_task(&router, "t_sse_live").await;
        let started = Instant::now();
        let frame = parse_business_frame(&next_frame(&mut body).await);
        assert!(
            started.elapsed() <= Duration::from_secs(2),
            "mutation-to-stream latency exceeded 2s: {:?}",
            started.elapsed()
        );
        assert_eq!(frame.2.task_id.as_deref(), Some("t_sse_live"));
        drop(body);
        state.begin_event_stream_shutdown();
    }

    #[tokio::test]
    async fn stream_events_rejects_malformed_or_unsafe_last_event_id() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        for cursor in ["wat", "9007199254740992", "-1"] {
            let response = router
                .clone()
                .oneshot(
                    Request::get("/api/v1/stream/events?after=0")
                        .header("last-event-id", cursor)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "cursor={cursor}"
            );
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
            assert_eq!(error.error.code, ApiErrorCode::InvalidInput);
        }
        let response = router
            .clone()
            .oneshot(
                Request::get("/api/v1/stream/events?after=0")
                    .header("last-event-id", "1")
                    .header("last-event-id", "2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let error: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(error.error.code, ApiErrorCode::InvalidInput);
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
        state.begin_event_stream_shutdown();
    }

    #[tokio::test]
    async fn stream_events_rejects_invalid_board_and_task_scopes_before_headers() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let router = build_router(state.clone());
        create_board(&router, "sse-archive").await;
        create_board(&router, "sse-other").await;
        create_task_on_board(&router, "sse-other", "t_sse_other_board").await;

        let archived = router
            .clone()
            .oneshot(json_request(
                "/api/v1/boards/sse-archive/archive",
                serde_json::json!({"actor": "tester"}),
            ))
            .await
            .unwrap();
        assert_eq!(archived.status(), StatusCode::OK);

        assert_stream_error(
            &router,
            "/api/v1/stream/events?board=missing&after=0",
            StatusCode::NOT_FOUND,
            ApiErrorCode::NotFound,
        )
        .await;
        assert_stream_error(
            &router,
            "/api/v1/stream/events?board=%20&after=0",
            StatusCode::BAD_REQUEST,
            ApiErrorCode::InvalidInput,
        )
        .await;
        assert_stream_error(
            &router,
            "/api/v1/stream/events?board=sse-archive&after=0",
            StatusCode::NOT_FOUND,
            ApiErrorCode::NotFound,
        )
        .await;
        assert_stream_error(
            &router,
            "/api/v1/stream/events?board=default&task_id=t_sse_missing&after=0",
            StatusCode::NOT_FOUND,
            ApiErrorCode::NotFound,
        )
        .await;
        assert_stream_error(
            &router,
            "/api/v1/stream/events?board=default&task_id=t_sse_other_board&after=0",
            StatusCode::NOT_FOUND,
            ApiErrorCode::NotFound,
        )
        .await;
        state.begin_event_stream_shutdown();
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
        let parsed = parse_business_frame(&frame);
        assert_eq!(parsed.0, 17);
        assert_eq!(parsed.1, "plugin.future.event");
        assert_eq!(
            parsed.2.payload,
            EventPayload::Unknown(serde_json::json!({
                "opaque": [1, {"nested": true}],
            }))
        );

        let invalid = StreamEventData {
            kind: "plugin.future\nevent".into(),
            ..parsed.2.clone()
        };
        assert!(sse_frame(invalid).is_err());

        let boundary = StreamEventData {
            id: MAX_SAFE_EVENT_CURSOR,
            ..parsed.2.clone()
        };
        assert!(sse_frame(boundary).is_ok());
        for id in [-1, MAX_SAFE_EVENT_CURSOR.saturating_add(1)] {
            let overflow = StreamEventData {
                id,
                ..parsed.2.clone()
            };
            assert!(sse_frame(overflow).is_err(), "id={id}");
        }
    }
}
