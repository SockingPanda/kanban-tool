use std::collections::VecDeque;

use kanban_protocol::{
    EventHeartbeatData, ListEventsQuery, StreamEventData, StreamEventsQuery,
    rpc::{
        query,
        v1::{self, query_frame::Body},
    },
};
use tonic::Streaming;

use crate::{ClientError, KanbanClient, transport::UNARY_TIMEOUT};

mod projection;
mod validation;
use projection::Projection;

const QUERY_ID: &str = "events";

/// 领域事件与不推进业务 cursor 的连接保活项目。
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EventStreamItem {
    Business(StreamEventData),
    Heartbeat(EventHeartbeatData),
}

/// 在 QueryService 中跟随一页完整事件投影；消费满页后移动查询窗口。
/// 丢弃订阅即取消 RPC；没有后台读取任务，也没有写提示后的 unary 回读。
pub struct EventStream {
    client: KanbanClient,
    stream: Option<Streaming<v1::QueryFrame>>,
    query: ListEventsQuery,
    projection: Projection,
    pending: VecDeque<StreamEventData>,
    delivered_after: i64,
    window_end: Option<i64>,
    closed: bool,
}

impl EventStream {
    /// 取消等待保留完整已提交投影和未交付事件；中途快照不会推进 durable cursor。
    pub async fn next_item(&mut self) -> Result<EventStreamItem, ClientError> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                self.delivered_after = event.id;
                return Ok(EventStreamItem::Business(event));
            }
            if self.closed {
                return Err(ClientError::StreamClosed);
            }
            if let Some(after) = self.window_end.take() {
                self.query.after = after;
                // 先释放原窗口占用的订阅槽；取消建流时下一次仍会从这个窗口继续。
                self.stream = None;
                self.projection = Projection::default();
            }
            if self.stream.is_none() {
                self.stream = Some(open_window(&self.client, &self.query, true).await?);
            }
            if let Some(heartbeat) = self.read_frame().await? {
                return Ok(heartbeat);
            }
        }
    }

    async fn read_frame(&mut self) -> Result<Option<EventStreamItem>, ClientError> {
        let stream = self.stream.as_mut().ok_or(ClientError::StreamClosed)?;
        let frame = match stream.message().await {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                self.closed = true;
                self.stream = None;
                return Err(ClientError::StreamClosed);
            }
            Err(error) => {
                self.closed = true;
                self.stream = None;
                return Err(ClientError::status(error));
            }
        };
        if let Err(error) = validation::validate_frame_scope(&frame, QUERY_ID) {
            self.closed = true;
            self.stream = None;
            return Err(error);
        }
        let result = self.accept_frame(frame.body);
        if result.is_err() {
            self.closed = true;
            self.stream = None;
        }
        result
    }

    fn accept_frame(&mut self, body: Option<Body>) -> Result<Option<EventStreamItem>, ClientError> {
        match body.ok_or_else(|| invalid("查询 frame 缺少 body"))? {
            Body::Begin(begin) => self.projection.begin(begin)?,
            Body::Chunk(chunk) => self.projection.chunk(chunk)?,
            Body::End(end) => {
                let page = self.projection.end(end, &self.query)?;
                let buffered_after = self
                    .pending
                    .back()
                    .map_or(self.delivered_after, |event| event.id);
                self.window_end =
                    (page.data.len() == self.query.limit).then_some(page.meta.next_after);
                self.pending.extend(
                    page.data
                        .into_iter()
                        .filter(|event| event.id > buffered_after),
                );
            }
            Body::Ready(ready) => {
                if self.projection.cursor() != ready.cursor.as_ref() || ready.cursor.is_none() {
                    return Err(invalid("查询 Ready 与完整投影不一致"));
                }
            }
            Body::Heartbeat(_) => {
                if self.projection.cursor().is_some() {
                    return Ok(Some(EventStreamItem::Heartbeat(
                        EventHeartbeatData::default(),
                    )));
                }
            }
            Body::Failure(failure) => return Err(query_failure(failure)),
        }
        Ok(None)
    }
}

impl std::fmt::Debug for EventStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventStream")
            .field("board", &self.query.board)
            .field("after", &self.delivered_after)
            .field("buffered", &self.pending.len())
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl KanbanClient {
    /// 两个 durable event cursor 取较新者；重连从调用者最后交付的事件 ID 继续。
    pub async fn open_event_stream(
        &self,
        query: &StreamEventsQuery,
        resume_after: Option<i64>,
    ) -> Result<EventStream, ClientError> {
        validation::validate_query(query, resume_after)?;
        let board = self.get_board(query.board.trim()).await?.id;
        let query = ListEventsQuery {
            board,
            task_id: query.task_id.as_deref().map(|id| id.trim().to_owned()),
            after: query.after.max(resume_after.unwrap_or(0)),
            limit: query.limit.min(1000),
        };
        let mut output = EventStream {
            stream: Some(open_window(self, &query, false).await?),
            delivered_after: query.after,
            query,
            client: self.clone(),
            projection: Projection::default(),
            pending: VecDeque::new(),
            window_end: None,
            closed: false,
        };
        tokio::time::timeout(UNARY_TIMEOUT, async {
            while output.projection.cursor().is_none() {
                output.read_frame().await?;
            }
            Ok::<_, ClientError>(())
        })
        .await
        .map_err(|_| ClientError::unary_timeout())??;
        Ok(output)
    }
}

async fn open_window(
    client: &KanbanClient,
    window: &ListEventsQuery,
    replacing: bool,
) -> Result<Streaming<v1::QueryFrame>, ClientError> {
    let request = v1::ListEventsRequest::from_parts((), window.clone(), ())
        .map_err(ClientError::request_codec)?;
    let request = v1::WatchQueriesRequest {
        protocol_version: query::PROTOCOL_VERSION,
        queries: vec![v1::QueryDefinition {
            client_query_id: QUERY_ID.into(),
            projection_version: query::PROJECTION_VERSION,
            resume: None,
            refresh: false,
            query: Some(v1::query_definition::Query::ListEvents(request)),
        }],
    };
    tokio::time::timeout(UNARY_TIMEOUT, async {
        let mut rpc = v1::query_service_client::QueryServiceClient::new(client.channel().await?)
            .max_decoding_message_size(query::MAX_QUERY_FRAME_BYTES)
            .max_encoding_message_size(query::MAX_QUERY_FRAME_BYTES);
        loop {
            match rpc.watch_queries(client.request(request.clone())).await {
                Ok(response) => return Ok(response.into_inner()),
                // 原满页连接的 RST_STREAM 可能尚在传输；短暂等待释放相同额度。
                Err(status) if replacing && status.code() == tonic::Code::ResourceExhausted => {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                Err(status) => return Err(ClientError::status(status)),
            }
        }
    })
    .await
    .map_err(|_| ClientError::unary_timeout())?
}

fn query_failure(failure: v1::QueryFailure) -> ClientError {
    let Some(detail) = failure.error else {
        return invalid("查询错误缺少业务 detail");
    };
    let Ok(code) = v1::DtoApiErrorCode::try_from(detail.code) else {
        return invalid("查询错误 code 无效");
    };
    let Ok(code) = code.try_into() else {
        return invalid("查询错误 code 未指定");
    };
    ClientError::status(kanban_protocol::rpc::encode_status(
        kanban_protocol::ErrorBody {
            code,
            message: detail.message,
        },
    ))
}

fn invalid(message: impl Into<String>) -> ClientError {
    ClientError::InvalidResponse(message.into())
}
