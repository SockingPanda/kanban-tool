use crate::ClientError;
use kanban_protocol::{MAX_SAFE_EVENT_CURSOR, StreamEventsQuery};

pub(super) fn validate_frame_scope(
    frame: &kanban_protocol::rpc::v1::QueryFrame,
    query_id: &str,
) -> Result<(), ClientError> {
    use kanban_protocol::rpc::v1::query_frame::Body;

    let stream_heartbeat =
        frame.client_query_id.is_empty() && matches!(frame.body.as_ref(), Some(Body::Heartbeat(_)));
    if frame.client_query_id == query_id || stream_heartbeat {
        Ok(())
    } else {
        Err(super::invalid("事件流收到其他查询的 frame"))
    }
}

pub(super) fn validate_query(
    query: &StreamEventsQuery,
    last_event_id: Option<i64>,
) -> Result<(), ClientError> {
    if query.board.trim().is_empty() {
        return Err(ClientError::InvalidInput("board 不能为空".to_owned()));
    }
    if !(0..=MAX_SAFE_EVENT_CURSOR).contains(&query.after)
        || last_event_id.is_some_and(|cursor| !(0..=MAX_SAFE_EVENT_CURSOR).contains(&cursor))
    {
        return Err(ClientError::InvalidInput(
            "事件 cursor 必须是非负 JavaScript 安全整数".to_owned(),
        ));
    }
    if query.limit == 0 {
        return Err(ClientError::InvalidInput("limit 必须大于 0".to_owned()));
    }
    let task_id = query.task_id.as_deref().map(str::trim);
    if task_id.is_some_and(|id| !id.starts_with("t_") || id.len() <= 2) {
        return Err(ClientError::InvalidInput(
            "task_id 必须是全局 t_... ID".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KanbanClient;

    #[test]
    fn only_heartbeat_can_omit_query_scope() {
        use kanban_protocol::rpc::v1::{QueryFrame, QueryHeartbeat, QueryReady, query_frame::Body};

        let mut frame = QueryFrame {
            client_query_id: String::new(),
            body: Some(Body::Heartbeat(QueryHeartbeat {})),
        };
        assert!(validate_frame_scope(&frame, "events").is_ok());
        frame.body = Some(Body::Ready(QueryReady { cursor: None }));
        assert!(validate_frame_scope(&frame, "events").is_err());
        frame.client_query_id = "other".into();
        assert!(validate_frame_scope(&frame, "events").is_err());
        frame.client_query_id = "events".into();
        assert!(validate_frame_scope(&frame, "events").is_ok());
    }

    #[tokio::test]
    async fn malformed_cursors_and_scope_are_rejected_before_connecting() {
        let client = KanbanClient::new(crate::DEFAULT_SERVER_URL, "test").unwrap();
        let query = StreamEventsQuery {
            board: "default".to_owned(),
            task_id: None,
            after: 0,
            limit: 10,
        };
        for cursor in [-1, MAX_SAFE_EVENT_CURSOR + 1] {
            assert_eq!(
                client
                    .open_event_stream(&query, Some(cursor))
                    .await
                    .unwrap_err()
                    .code(),
                "invalid_input"
            );
        }
        for query in [
            StreamEventsQuery {
                board: " ".into(),
                ..query.clone()
            },
            StreamEventsQuery {
                task_id: Some("default#1".into()),
                ..query.clone()
            },
            StreamEventsQuery {
                limit: 0,
                ..query.clone()
            },
            StreamEventsQuery { after: -1, ..query },
        ] {
            assert_eq!(
                client
                    .open_event_stream(&query, None)
                    .await
                    .unwrap_err()
                    .code(),
                "invalid_input"
            );
        }
        assert!(client.channel.get().is_none());
    }
}
