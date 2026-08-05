use kanban_contract::{StreamEventData, StreamEventsQuery};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

impl KanbanClient {
    /// 读取 host 当前可见的有限 SSE 事件快照。
    pub fn stream_events_snapshot(
        &self,
        query: &StreamEventsQuery,
    ) -> Result<Vec<StreamEventData>, ClientError> {
        validate_query(query)?;
        let path = stream_events_path(query);
        let (content_type, body) = self.get_text(&path, "text/event-stream")?;
        if !content_type
            .as_deref()
            .is_some_and(|value| value.starts_with("text/event-stream"))
        {
            return Err(ClientError::InvalidResponse(
                "SSE 响应缺少 text/event-stream Content-Type".to_owned(),
            ));
        }
        parse_sse_snapshot(&body)
    }
}

fn validate_query(query: &StreamEventsQuery) -> Result<(), ClientError> {
    if query.board.trim().is_empty() {
        return Err(ClientError::InvalidInput("board 不能为空".to_owned()));
    }
    if query.after < 0 {
        return Err(ClientError::InvalidInput("after 不能为负数".to_owned()));
    }
    let task_id = query.task_id.as_deref().map(str::trim);
    if task_id.is_some_and(|task_id| !task_id.starts_with("t_") || task_id.len() <= 2) {
        return Err(ClientError::InvalidInput(
            "task_id 必须是全局 t_... ID".to_owned(),
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

fn parse_sse_snapshot(body: &str) -> Result<Vec<StreamEventData>, ClientError> {
    if body.is_empty() {
        return Ok(Vec::new());
    }
    if body.contains('\r') || !body.ends_with("\n\n") {
        return Err(ClientError::InvalidResponse(
            "SSE 快照不是完整的 LF 分隔帧".to_owned(),
        ));
    }

    body.trim_end_matches('\n')
        .split("\n\n")
        .map(parse_sse_frame)
        .collect()
}

fn parse_sse_frame(frame: &str) -> Result<StreamEventData, ClientError> {
    let mut event_name = None;
    let mut frame_id = None;
    let mut data = None;

    for line in frame.lines() {
        let (name, value) = line.split_once(": ").ok_or_else(|| {
            ClientError::InvalidResponse("SSE frame 含有无法识别的字段行".to_owned())
        })?;
        let slot = match name {
            "event" => &mut event_name,
            "id" => &mut frame_id,
            "data" => &mut data,
            _ => {
                return Err(ClientError::InvalidResponse(format!(
                    "SSE frame 含有未支持字段：{name}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(ClientError::InvalidResponse(format!(
                "SSE frame 重复字段：{name}"
            )));
        }
    }

    let event_name = event_name
        .ok_or_else(|| ClientError::InvalidResponse("SSE frame 缺少 event".to_owned()))?;
    let frame_id = frame_id
        .ok_or_else(|| ClientError::InvalidResponse("SSE frame 缺少 id".to_owned()))?
        .parse::<i64>()
        .map_err(|_| ClientError::InvalidResponse("SSE frame id 不是整数".to_owned()))?;
    let data =
        data.ok_or_else(|| ClientError::InvalidResponse("SSE frame 缺少 data".to_owned()))?;
    let event: StreamEventData = serde_json::from_str(data)
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
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn path_order_and_encoding_are_stable() {
        let path = stream_events_path(&StreamEventsQuery {
            board: " team/#1 ".to_owned(),
            task_id: Some(" t_event/1 ".to_owned()),
            after: 7,
            limit: 25,
        });
        assert_eq!(
            path,
            "/api/v1/stream/events?board=team%2F%231&task_id=t_event%2F1&after=7&limit=25"
        );
    }

    #[test]
    fn parses_unknown_event_losslessly_through_contract() {
        let data = unknown_event_json(9, "future.event");
        let frames =
            parse_sse_snapshot(&format!("event: future.event\nid: 9\ndata: {data}\n\n")).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].kind, "future.event");
        assert_eq!(frames[0].id, 9);
        assert_eq!(
            serde_json::to_value(&frames[0].payload).unwrap(),
            serde_json::json!({"future": true})
        );
    }

    #[test]
    fn rejects_incomplete_or_mismatched_frames() {
        let data = unknown_event_json(9, "future.event");
        for body in [
            format!("event: future.event\nid: 9\ndata: {data}\n"),
            format!("event: other.event\nid: 9\ndata: {data}\n\n"),
            format!("event: future.event\nid: 8\ndata: {data}\n\n"),
            "event: future.event\nid: 9\n\n".to_owned(),
        ] {
            assert!(parse_sse_snapshot(&body).is_err(), "{body:?}");
        }
    }

    #[test]
    fn rejects_invalid_query_before_http() {
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
                task_id: Some("default#1".to_owned()),
                after: 0,
                limit: 10,
            },
        ] {
            assert_eq!(
                client.stream_events_snapshot(&query).unwrap_err().code(),
                "invalid_input"
            );
        }
    }
}
