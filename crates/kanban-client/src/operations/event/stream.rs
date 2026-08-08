use std::io::Read;

use kanban_protocol::{
    MAX_SAFE_EVENT_CURSOR, SSE_HEARTBEAT_EVENT, SseHeartbeatData, StreamEventData,
    StreamEventsQuery,
};

use crate::{
    KanbanClient,
    error::ClientError,
    transport::{ResponseReader, encode_path_segment},
};

pub(crate) const MAX_LINE_BYTES: usize = 16 * 1024;
pub(crate) const MAX_FRAME_BYTES: usize = 256 * 1024;
pub(crate) const MAX_DATA_BYTES: usize = 128 * 1024;

/// 持久 SSE 连接上的增量项目。
#[derive(Debug)]
// 保持调用者可直接匹配 `Business(StreamEventData)`，不把 wire DTO 包装进额外指针。
#[allow(clippy::large_enum_variant)]
pub enum EventStreamItem {
    /// 带业务 cursor 的领域事件。
    Business(StreamEventData),
    /// 不推进业务 cursor 的连接保活控制事件。
    Heartbeat(SseHeartbeatData),
}

/// 持有 SSE response reader 的同步增量流。
pub struct EventStream {
    reader: ResponseReader,
    decoder: SseDecoder,
    closed: bool,
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventStream")
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl EventStream {
    /// 读取下一个业务或 heartbeat item；连接 EOF 会返回 [`ClientError::StreamClosed`]。
    pub fn next_item(&mut self) -> Result<EventStreamItem, ClientError> {
        let mut chunk = [0_u8; 4096];
        loop {
            if let Some(frame) = self.decoder.take_frame()? {
                if let Some(item) = parse_stream_frame(&frame)? {
                    return Ok(item);
                }
                continue;
            }
            if self.closed {
                return Err(ClientError::StreamClosed);
            }
            let bytes_read = self
                .reader
                .read(&mut chunk)
                .map_err(|error| ClientError::StreamRead(error.to_string()))?;
            if bytes_read == 0 {
                self.closed = true;
                if self.decoder.has_pending() {
                    return Err(ClientError::InvalidResponse(
                        "SSE stream 在完整帧结束前关闭".to_owned(),
                    ));
                }
                return Err(ClientError::StreamClosed);
            }
            self.decoder.push(&chunk[..bytes_read])?;
        }
    }
}

impl KanbanClient {
    /// 打开一次持久 SSE 连接；`after` 仍由 query 持有，重连 cursor 通过 `Last-Event-ID` 传递。
    pub fn open_event_stream(
        &self,
        query: &StreamEventsQuery,
        last_event_id: Option<i64>,
    ) -> Result<EventStream, ClientError> {
        validate_query(query)?;
        validate_last_event_id(last_event_id)?;
        let path = stream_events_path(query);
        let (content_type, reader) = self.get_stream(&path, "text/event-stream", last_event_id)?;
        if !is_event_stream_content_type(content_type.as_deref()) {
            return Err(ClientError::InvalidResponse(
                "SSE 响应 Content-Type 必须是 text/event-stream".to_owned(),
            ));
        }
        Ok(EventStream {
            reader,
            decoder: SseDecoder::default(),
            closed: false,
        })
    }
}

fn validate_query(query: &StreamEventsQuery) -> Result<(), ClientError> {
    if query.board.trim().is_empty() {
        return Err(ClientError::InvalidInput("board 不能为空".to_owned()));
    }
    if !(0..=MAX_SAFE_EVENT_CURSOR).contains(&query.after) {
        return Err(ClientError::InvalidInput(
            "after 必须是非负 JavaScript 安全整数".to_owned(),
        ));
    }
    if query.limit == 0 {
        return Err(ClientError::InvalidInput("limit 必须大于 0".to_owned()));
    }
    let task_id = query.task_id.as_deref().map(str::trim);
    if task_id.is_some_and(|task_id| !task_id.starts_with("t_") || task_id.len() <= 2) {
        return Err(ClientError::InvalidInput(
            "task_id 必须是全局 t_... ID".to_owned(),
        ));
    }
    Ok(())
}

fn validate_last_event_id(last_event_id: Option<i64>) -> Result<(), ClientError> {
    if last_event_id.is_some_and(|value| !(0..=MAX_SAFE_EVENT_CURSOR).contains(&value)) {
        return Err(ClientError::InvalidInput(
            "Last-Event-ID 必须是非负 JavaScript 安全整数".to_owned(),
        ));
    }
    Ok(())
}

fn stream_events_path(query: &StreamEventsQuery) -> String {
    let mut pairs = vec![format!("board={}", encode_path_segment(query.board.trim()))];
    if let Some(task_id) = query.task_id.as_deref().map(str::trim) {
        pairs.push(format!("task_id={}", encode_path_segment(task_id)));
    }
    pairs.push(format!("after={}", query.after));
    pairs.push(format!("limit={}", query.limit));
    format!("/api/v1/stream/events?{}", pairs.join("&"))
}

fn is_event_stream_content_type(content_type: Option<&str>) -> bool {
    content_type
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("text/event-stream"))
}

#[derive(Default)]
struct SseDecoder {
    pending: Vec<u8>,
}

impl SseDecoder {
    fn push(&mut self, bytes: &[u8]) -> Result<(), ClientError> {
        self.pending.extend_from_slice(bytes);
        if self.pending.len() > MAX_FRAME_BYTES && find_frame_end(&self.pending).is_none() {
            return Err(ClientError::InvalidResponse(format!(
                "SSE frame 超过 {} 字节上限",
                MAX_FRAME_BYTES
            )));
        }
        Ok(())
    }

    fn take_frame(&mut self) -> Result<Option<Vec<u8>>, ClientError> {
        let Some((frame_end, consumed)) = find_frame_end(&self.pending) else {
            return Ok(None);
        };
        if frame_end > MAX_FRAME_BYTES {
            return Err(ClientError::InvalidResponse(format!(
                "SSE frame 超过 {} 字节上限",
                MAX_FRAME_BYTES
            )));
        }
        let frame = self.pending.drain(..frame_end).collect::<Vec<_>>();
        self.pending.drain(..consumed - frame_end);
        Ok(Some(frame))
    }

    fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

fn find_frame_end(bytes: &[u8]) -> Option<(usize, usize)> {
    for index in 0..bytes.len() {
        let suffix = &bytes[index..];
        if suffix.starts_with(b"\n\n") {
            return Some((index, index + 2));
        }
        if suffix.starts_with(b"\r\n\r\n") {
            return Some((index, index + 4));
        }
        if suffix.starts_with(b"\n\r\n") || suffix.starts_with(b"\r\n\n") {
            return Some((index, index + 3));
        }
    }
    None
}

struct SseFrame {
    event_name: Option<String>,
    frame_id: Option<String>,
    data: Option<String>,
}

fn parse_frame(frame: &[u8]) -> Result<Option<SseFrame>, ClientError> {
    let frame = std::str::from_utf8(frame)
        .map_err(|error| ClientError::InvalidResponse(format!("SSE frame 不是 UTF-8：{error}")))?;
    let mut parsed = SseFrame {
        event_name: None,
        frame_id: None,
        data: None,
    };

    for line in frame.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.len() > MAX_LINE_BYTES {
            return Err(ClientError::InvalidResponse(format!(
                "SSE 字段行超过 {} 字节上限",
                MAX_LINE_BYTES
            )));
        }
        if line.contains('\r') {
            return Err(ClientError::InvalidResponse(
                "SSE 字段行含有非法 CR".to_owned(),
            ));
        }
        if line.starts_with(':') {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            ClientError::InvalidResponse("SSE frame 含有无法识别的字段行".to_owned())
        })?;
        let value = value.strip_prefix(' ').unwrap_or(value);
        match name {
            "event" => {
                if parsed.event_name.replace(value.to_owned()).is_some() {
                    return Err(ClientError::InvalidResponse(
                        "SSE frame 重复字段：event".to_owned(),
                    ));
                }
            }
            "id" => {
                if parsed.frame_id.replace(value.to_owned()).is_some() {
                    return Err(ClientError::InvalidResponse(
                        "SSE frame 重复字段：id".to_owned(),
                    ));
                }
            }
            "data" => {
                let had_data_line = parsed.data.is_some();
                let data = parsed.data.get_or_insert_with(String::new);
                if had_data_line {
                    data.push('\n');
                }
                data.push_str(value);
                if data.len() > MAX_DATA_BYTES {
                    return Err(ClientError::InvalidResponse(format!(
                        "SSE data 超过 {} 字节上限",
                        MAX_DATA_BYTES
                    )));
                }
            }
            _ => {
                return Err(ClientError::InvalidResponse(format!(
                    "SSE frame 含有未支持字段：{name}"
                )));
            }
        }
    }

    if parsed.event_name.is_none() && parsed.frame_id.is_none() && parsed.data.is_none() {
        return Ok(None);
    }
    Ok(Some(parsed))
}

fn parse_stream_frame(frame: &[u8]) -> Result<Option<EventStreamItem>, ClientError> {
    let Some(frame) = parse_frame(frame)? else {
        return Ok(None);
    };
    let event_name = frame
        .event_name
        .ok_or_else(|| ClientError::InvalidResponse("SSE frame 缺少 event".to_owned()))?;
    let data = frame
        .data
        .ok_or_else(|| ClientError::InvalidResponse("SSE frame 缺少 data".to_owned()))?;

    if event_name == SSE_HEARTBEAT_EVENT {
        if frame.frame_id.is_some() {
            return Err(ClientError::InvalidResponse(
                "SSE heartbeat 不得包含业务 id".to_owned(),
            ));
        }
        let heartbeat: SseHeartbeatData = serde_json::from_str(&data).map_err(|error| {
            ClientError::InvalidResponse(format!("SSE heartbeat data 无效：{error}"))
        })?;
        return Ok(Some(EventStreamItem::Heartbeat(heartbeat)));
    }

    let frame_id = frame
        .frame_id
        .ok_or_else(|| ClientError::InvalidResponse("SSE frame 缺少 id".to_owned()))?;
    let frame_id = parse_frame_id(&frame_id)?;
    let event: StreamEventData = serde_json::from_str(&data)
        .map_err(|error| ClientError::InvalidResponse(format!("SSE data 无效：{error}")))?;
    if event.id != frame_id {
        return Err(ClientError::InvalidResponse(
            "SSE frame id 与 data.id 不一致".to_owned(),
        ));
    }
    if event.kind != event_name {
        return Err(ClientError::InvalidResponse(
            "SSE frame event 与 data.kind 不一致".to_owned(),
        ));
    }
    Ok(Some(EventStreamItem::Business(event)))
}

fn parse_frame_id(value: &str) -> Result<i64, ClientError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ClientError::InvalidResponse(
            "SSE frame id 必须是十进制非负整数".to_owned(),
        ));
    }
    let value = value
        .parse::<i64>()
        .map_err(|_| ClientError::InvalidResponse("SSE frame id 超出整数范围".to_owned()))?;
    if value > MAX_SAFE_EVENT_CURSOR {
        return Err(ClientError::InvalidResponse(
            "SSE frame id 超出 JavaScript 安全整数范围".to_owned(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc::{self, Receiver};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    struct Fixture {
        address: String,
        request: Receiver<Vec<u8>>,
        closed: Receiver<bool>,
        thread: Option<JoinHandle<()>>,
    }

    impl Fixture {
        fn client(&self) -> KanbanClient {
            KanbanClient::new(&self.address, "fixture-actor").expect("loopback fixture URL")
        }

        fn join(mut self) {
            self.thread.take().expect("fixture thread").join().unwrap();
        }
    }

    fn fixture(response: Vec<u8>, wait_for_close: bool) -> Fixture {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind fixture");
        let address = format!("http://{}", listener.local_addr().unwrap());
        let (request_tx, request) = mpsc::channel();
        let (closed_tx, closed) = mpsc::channel();
        let thread = thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept fixture");
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .expect("request read timeout");
            let request = read_headers(&mut socket);
            request_tx.send(request).expect("capture request");
            socket.write_all(&response).expect("write fixture response");
            socket.flush().expect("flush fixture response");

            let closed_by_client = if wait_for_close {
                let mut buffer = [0_u8; 1024];
                loop {
                    match socket.read(&mut buffer) {
                        Ok(0) => break true,
                        Ok(_) => continue,
                        Err(_) => break false,
                    }
                }
            } else {
                true
            };
            closed_tx.send(closed_by_client).expect("capture close");
        });
        Fixture {
            address,
            request,
            closed,
            thread: Some(thread),
        }
    }

    fn read_headers(socket: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).expect("read request header");
            request.push(byte[0]);
        }
        request
    }

    fn response(content_type: &str, body: &[u8]) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes()
        .into_iter()
        .chain(body.iter().copied())
        .collect()
    }

    fn unknown_event_json(id: i64, kind: &str) -> String {
        serde_json::json!({
            "id": id,
            "event_id": "e_example",
            "board_id": "b_default",
            "task_id": null,
            "run_id": null,
            "kind": kind,
            "actor": "tester",
            "payload": {"future": true},
            "created_at": 123
        })
        .to_string()
    }

    #[test]
    fn opens_stream_with_query_and_last_event_id_header() {
        let body = format!(
            "event: future.event\nid: 9\ndata: {}\n\n",
            unknown_event_json(9, "future.event")
        );
        let fixture = fixture(
            response("text/event-stream; charset=utf-8", body.as_bytes()),
            false,
        );
        let query = StreamEventsQuery {
            board: " team/#1 ".to_owned(),
            task_id: Some(" t_event/1 ".to_owned()),
            after: 7,
            limit: 25,
        };
        let mut stream = fixture
            .client()
            .open_event_stream(&query, Some(41))
            .expect("open SSE stream");
        assert!(matches!(
            stream.next_item().unwrap(),
            EventStreamItem::Business(event) if event.id == 9
        ));

        let request = String::from_utf8(fixture.request.recv().unwrap()).unwrap();
        assert!(request.starts_with(
            "GET /api/v1/stream/events?board=team%2F%231&task_id=t_event%2F1&after=7&limit=25 HTTP/1.1\r\n"
        ));
        assert!(request.contains("Accept: text/event-stream\r\n"));
        assert!(request.contains("Last-Event-ID: 41\r\n"));
        assert!(request.contains("X-KB-Actor: fixture-actor\r\n"));
        fixture.join();
    }

    #[test]
    fn parses_unknown_event_losslessly_through_incremental_reader() {
        let data = unknown_event_json(9, "future.event");
        let body =
            format!(": keepalive\r\n\r\nevent: future.event\r\nid: 9\r\ndata: {data}\r\n\r\n");
        let fixture = fixture(response("text/event-stream", body.as_bytes()), false);
        let mut stream = fixture
            .client()
            .open_event_stream(
                &StreamEventsQuery {
                    board: "default".to_owned(),
                    task_id: None,
                    after: 0,
                    limit: 10,
                },
                None,
            )
            .unwrap();
        let EventStreamItem::Business(event) = stream.next_item().unwrap() else {
            panic!("business event expected");
        };
        assert_eq!(
            serde_json::to_value(&event.payload).unwrap(),
            serde_json::json!({"future": true})
        );
        fixture.join();
    }

    #[test]
    fn parses_typed_heartbeat_control_without_business_cursor() {
        let body = format!(
            ": transport comment\r\n\r\nevent: {SSE_HEARTBEAT_EVENT}\r\ndata: {{}}\r\n\r\n"
        );
        let fixture = fixture(response("text/event-stream", body.as_bytes()), false);
        let mut stream = fixture
            .client()
            .open_event_stream(
                &StreamEventsQuery {
                    board: "default".to_owned(),
                    task_id: None,
                    after: 0,
                    limit: 10,
                },
                None,
            )
            .unwrap();
        assert!(matches!(
            stream.next_item(),
            Ok(EventStreamItem::Heartbeat(heartbeat)) if heartbeat == SseHeartbeatData::default()
        ));
        fixture.join();
    }

    #[test]
    fn rejects_malformed_heartbeat_control_frames() {
        for body in [
            format!("event: {SSE_HEARTBEAT_EVENT}\nid: 9\ndata: {{}}\n\n"),
            format!("event: {SSE_HEARTBEAT_EVENT}\ndata: {{\"unexpected\":true}}\n\n"),
            format!("event: {SSE_HEARTBEAT_EVENT}\n\n"),
        ] {
            let fixture = fixture(response("text/event-stream", body.as_bytes()), false);
            let mut stream = fixture
                .client()
                .open_event_stream(
                    &StreamEventsQuery {
                        board: "default".to_owned(),
                        task_id: None,
                        after: 0,
                        limit: 10,
                    },
                    None,
                )
                .unwrap();
            assert!(stream.next_item().is_err(), "{body:?}");
            fixture.join();
        }
    }

    #[test]
    fn parses_a_frame_split_at_every_reader_chunk_boundary() {
        let data = unknown_event_json(9, "future.event");
        let body = format!("event: future.event\nid: 9\ndata: {data}\n\n");
        let mut decoder = SseDecoder::default();
        for byte in body.as_bytes() {
            decoder.push(std::slice::from_ref(byte)).unwrap();
        }
        let frame = decoder.take_frame().unwrap().unwrap();
        let Some(EventStreamItem::Business(event)) = parse_stream_frame(&frame).unwrap() else {
            panic!("business frame expected");
        };
        assert_eq!(event.id, 9);
    }

    #[test]
    fn accepts_multiline_data_and_both_line_endings() {
        let data = serde_json::to_string_pretty(&serde_json::json!({
            "id": 9,
            "event_id": "e_example",
            "board_id": "b_default",
            "task_id": null,
            "run_id": null,
            "kind": "future.event",
            "actor": "tester",
            "payload": {"future": true},
            "created_at": 123
        }))
        .unwrap();
        let data_lines = data
            .lines()
            .map(|line| format!("data: {line}"))
            .collect::<Vec<_>>()
            .join("\r\n");
        let body = format!("event: future.event\r\nid: 9\r\ndata: \r\n{data_lines}\r\n\r\n",);
        let fixture = fixture(response("text/event-stream", body.as_bytes()), false);
        let mut stream = fixture
            .client()
            .open_event_stream(
                &StreamEventsQuery {
                    board: "default".to_owned(),
                    task_id: None,
                    after: 0,
                    limit: 10,
                },
                None,
            )
            .unwrap();
        let EventStreamItem::Business(event) = stream.next_item().unwrap() else {
            panic!("business event expected");
        };
        assert_eq!(event.id, 9);
        fixture.join();
    }

    #[test]
    fn rejects_incomplete_or_mismatched_frames() {
        let data = unknown_event_json(9, "future.event");
        for body in [
            format!("event: future.event\nid: 9\ndata: {data}\n"),
            format!("event: other.event\nid: 9\ndata: {data}\n\n"),
            format!("event: future.event\nid: 8\ndata: {data}\n\n"),
            format!("event: future.event\nid: -1\ndata: {data}\n\n"),
            format!(
                "event: future.event\nid: {}\ndata: {data}\n\n",
                MAX_SAFE_EVENT_CURSOR + 1
            ),
            "event: future.event\nid: 9\ndata: {not-json}\n\n".to_owned(),
            format!(
                "event: task.updated\nid: 9\ndata: {}\n\n",
                serde_json::json!({
                    "id": 9,
                    "event_id": "e_example",
                    "board_id": "b_default",
                    "task_id": null,
                    "run_id": null,
                    "kind": "task.updated",
                    "actor": "tester",
                    "payload": {"unexpected": true},
                    "created_at": 123
                })
            ),
            "event: future.event\nid: 9\n\n".to_owned(),
            format!("event: future.event\nid: 9\nevent: future.event\ndata: {data}\n\n"),
            format!("event: future.event\nid: 9\ndata: {data}\nunknown: value\n\n"),
        ] {
            let fixture = fixture(response("text/event-stream", body.as_bytes()), false);
            let mut stream = fixture
                .client()
                .open_event_stream(
                    &StreamEventsQuery {
                        board: "default".to_owned(),
                        task_id: None,
                        after: 0,
                        limit: 10,
                    },
                    None,
                )
                .unwrap();
            assert!(stream.next_item().is_err(), "{body:?}");
            fixture.join();
        }
    }

    #[test]
    fn rejects_invalid_query_and_last_event_id_before_http() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        for query in [
            StreamEventsQuery {
                board: " ".to_owned(),
                task_id: None,
                after: 0,
                limit: 10,
            },
            StreamEventsQuery {
                board: "default".to_owned(),
                task_id: None,
                after: -1,
                limit: 10,
            },
            StreamEventsQuery {
                board: "default".to_owned(),
                task_id: None,
                after: MAX_SAFE_EVENT_CURSOR + 1,
                limit: 10,
            },
            StreamEventsQuery {
                board: "default".to_owned(),
                task_id: None,
                after: 0,
                limit: 0,
            },
            StreamEventsQuery {
                board: "default".to_owned(),
                task_id: Some("default#1".to_owned()),
                after: 0,
                limit: 10,
            },
        ] {
            assert_eq!(
                client.open_event_stream(&query, None).unwrap_err().code(),
                "invalid_input"
            );
        }
        for last_event_id in [-1, MAX_SAFE_EVENT_CURSOR + 1] {
            assert_eq!(
                client
                    .open_event_stream(
                        &StreamEventsQuery {
                            board: "default".to_owned(),
                            task_id: None,
                            after: 0,
                            limit: 10,
                        },
                        Some(last_event_id),
                    )
                    .unwrap_err()
                    .code(),
                "invalid_input"
            );
        }
    }

    #[test]
    fn requires_exact_event_stream_media_type() {
        for content_type in ["text/event-streamish", "application/json"] {
            let fixture = fixture(response(content_type, b""), false);
            let result = fixture.client().open_event_stream(
                &StreamEventsQuery {
                    board: "default".to_owned(),
                    task_id: None,
                    after: 0,
                    limit: 10,
                },
                None,
            );
            assert!(matches!(result, Err(ClientError::InvalidResponse(_))));
            fixture.join();
        }
    }

    #[test]
    fn reports_clean_eof_as_stream_closed() {
        let fixture = fixture(response("text/event-stream", b""), false);
        let mut stream = fixture
            .client()
            .open_event_stream(
                &StreamEventsQuery {
                    board: "default".to_owned(),
                    task_id: None,
                    after: 0,
                    limit: 10,
                },
                None,
            )
            .unwrap();
        assert!(matches!(stream.next_item(), Err(ClientError::StreamClosed)));
        fixture.join();
    }

    #[test]
    fn reports_reader_failures_as_stream_errors() {
        struct FailingReader;

        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("fixture read failure"))
            }
        }

        let mut stream = EventStream {
            reader: Box::new(FailingReader),
            decoder: SseDecoder::default(),
            closed: false,
        };
        assert!(matches!(
            stream.next_item(),
            Err(ClientError::StreamRead(message)) if message.contains("fixture read failure")
        ));
    }

    #[test]
    fn drops_stream_reader_and_closes_connection() {
        let fixture = fixture(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
                .to_vec(),
            true,
        );
        let stream = fixture
            .client()
            .open_event_stream(
                &StreamEventsQuery {
                    board: "default".to_owned(),
                    task_id: None,
                    after: 0,
                    limit: 10,
                },
                None,
            )
            .unwrap();
        drop(stream);
        assert!(fixture.closed.recv_timeout(Duration::from_secs(3)).unwrap());
        fixture.join();
    }

    #[test]
    fn rejects_oversized_frame_and_data_without_collecting_body() {
        let oversized_data = (0..17)
            .map(|_| format!("data: {}", "x".repeat(8 * 1024)))
            .collect::<Vec<_>>()
            .join("\n");
        for body in [
            format!("event: future.event\nid: 9\n{oversized_data}\n\n"),
            format!(
                "event: future.event\nid: 9\ndata: {}\n\n",
                "x".repeat(MAX_LINE_BYTES + 1)
            ),
            format!("{}\n\n", "x".repeat(MAX_FRAME_BYTES + 1)),
        ] {
            let fixture = fixture(response("text/event-stream", body.as_bytes()), false);
            let mut stream = fixture
                .client()
                .open_event_stream(
                    &StreamEventsQuery {
                        board: "default".to_owned(),
                        task_id: None,
                        after: 0,
                        limit: 10,
                    },
                    None,
                )
                .unwrap();
            assert!(matches!(
                stream.next_item(),
                Err(ClientError::InvalidResponse(_))
            ));
            fixture.join();
        }
    }
}
