use std::collections::VecDeque;

use kanban_protocol::{
    ListEventsQuery, MAX_SAFE_EVENT_CURSOR, SseHeartbeatData, StreamEventData, StreamEventsQuery,
    rpc::v1::{self, workspace_change_frame::Body},
};
use tonic::Streaming;

use crate::{ClientError, KanbanClient, transport::UNARY_TIMEOUT};

/// 领域事件与不推进业务 cursor 的连接保活项目。
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EventStreamItem {
    Business(StreamEventData),
    Heartbeat(SseHeartbeatData),
}

/// 持有原生 RPC 订阅和一页事件的异步流；丢弃时释放订阅，没有后台读取任务。
///
/// `WatchChanges` 提供写入提示，`ListEvents` 提供持久 cursor 与领域事件。
/// 先完成订阅再读取事件页，覆盖初始化期间的并发写入。
pub struct EventStream {
    client: KanbanClient,
    stream: Streaming<v1::WorkspaceChangeFrame>,
    query: ListEventsQuery,
    pending: VecDeque<StreamEventData>,
    read_more: bool,
    epoch: String,
    sequence: u64,
    closed: bool,
}

impl EventStream {
    /// 读取下一项。取消当前读取不会丢失已缓冲的事件；连接结束返回 `StreamClosed`。
    pub async fn next_item(&mut self) -> Result<EventStreamItem, ClientError> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(EventStreamItem::Business(event));
            }
            if self.closed {
                return Err(ClientError::StreamClosed);
            }
            if self.read_more {
                self.read_page().await?;
                continue;
            }
            let frame = match self.stream.message().await {
                Ok(Some(frame)) => frame,
                Ok(None) => {
                    self.closed = true;
                    return Err(ClientError::StreamClosed);
                }
                Err(error) => {
                    self.closed = true;
                    return Err(ClientError::status(error));
                }
            };
            match self.accept_frame(frame)? {
                Body::Invalidated(_) => self.read_more = true,
                Body::Heartbeat(_) => {
                    return Ok(EventStreamItem::Heartbeat(SseHeartbeatData::default()));
                }
            }
        }
    }

    async fn read_page(&mut self) -> Result<(), ClientError> {
        let response = self.client.list_events(&self.query).await?;
        let mut previous = self.query.after;
        for event in &response.data {
            if event.id <= previous
                || event.id > MAX_SAFE_EVENT_CURSOR
                || event.board_id != self.query.board
                || self
                    .query
                    .task_id
                    .as_ref()
                    .is_some_and(|task| event.task_id.as_ref() != Some(task))
            {
                return Err(ClientError::InvalidResponse(
                    "事件页的 cursor 或作用域无效".to_owned(),
                ));
            }
            previous = event.id;
        }
        if response.meta.next_after != previous || response.data.len() > self.query.limit {
            return Err(ClientError::InvalidResponse(
                "事件页的 next_after 或大小无效".to_owned(),
            ));
        }
        self.read_more = response.data.len() == self.query.limit;
        self.query.after = response.meta.next_after;
        self.pending.extend(response.data);
        Ok(())
    }

    fn accept_frame(&mut self, frame: v1::WorkspaceChangeFrame) -> Result<Body, ClientError> {
        if frame.board_id != self.query.board || frame.epoch.is_empty() {
            return Err(ClientError::InvalidResponse(
                "订阅 frame 的作用域或 epoch 无效".to_owned(),
            ));
        }
        let body = frame
            .body
            .ok_or_else(|| ClientError::InvalidResponse("订阅 frame 缺少 body".to_owned()))?;
        let expected = match &body {
            Body::Invalidated(value) => {
                let reason = if self.sequence == 0 {
                    v1::RefreshReason::Attached
                } else {
                    v1::RefreshReason::WriteHint
                };
                if value.reason != reason as i32 {
                    return Err(ClientError::InvalidResponse(
                        "订阅 frame 的 reason 无效".to_owned(),
                    ));
                }
                self.sequence.checked_add(1)
            }
            Body::Heartbeat(_) if self.sequence > 0 => Some(self.sequence),
            Body::Heartbeat(_) => None,
        };
        if expected != Some(frame.sequence) || (!self.epoch.is_empty() && self.epoch != frame.epoch)
        {
            return Err(ClientError::InvalidResponse(
                "订阅 frame 的 sequence 或 epoch 不连续".to_owned(),
            ));
        }
        self.sequence = frame.sequence;
        self.epoch = frame.epoch;
        Ok(body)
    }
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventStream")
            .field("board", &self.query.board)
            .field("after", &self.query.after)
            .field("buffered", &self.pending.len())
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl KanbanClient {
    /// 打开事件订阅；两个 cursor 取较新者，重连仍由调用者持有最后交付的事件 ID。
    pub async fn open_event_stream(
        &self,
        query: &StreamEventsQuery,
        last_event_id: Option<i64>,
    ) -> Result<EventStream, ClientError> {
        validate_query(query, last_event_id)?;
        let board = self.get_board(query.board.trim()).await?.id;
        let mut client =
            v1::workspace_service_client::WorkspaceServiceClient::new(self.channel().await?)
                .max_decoding_message_size(4096)
                .max_encoding_message_size(4096);
        // 只给建流握手设置本地预算，不将 unary deadline 附到长期订阅上。
        let stream = tokio::time::timeout(
            UNARY_TIMEOUT,
            client.watch_changes(self.request(v1::WatchChangesRequest {
                board_id: board.clone(),
                protocol_version: 1,
            })),
        )
        .await
        .map_err(|_| ClientError::unary_timeout())?
        .map_err(ClientError::status)?
        .into_inner();
        let mut output = EventStream {
            client: self.clone(),
            stream,
            query: ListEventsQuery {
                board,
                task_id: query.task_id.as_deref().map(|id| id.trim().to_owned()),
                after: query.after.max(last_event_id.unwrap_or(0)),
                limit: query.limit.min(1000),
            },
            pending: VecDeque::new(),
            read_more: true,
            epoch: String::new(),
            sequence: 0,
            closed: false,
        };
        let attached = tokio::time::timeout(UNARY_TIMEOUT, output.stream.message())
            .await
            .map_err(|_| ClientError::unary_timeout())?
            .map_err(ClientError::status)?
            .ok_or(ClientError::StreamClosed)?;
        output.accept_frame(attached)?;
        output.read_page().await?;
        Ok(output)
    }
}

fn validate_query(
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
