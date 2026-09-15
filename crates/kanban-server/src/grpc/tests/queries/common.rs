//! 真 wire 测试夹具：读取正式帧并独立重建结果，与正式 unary 响应逐字节比较。
use super::{Body, Host, Query, pb};
use kanban_protocol::{self as dto, rpc::query::QUERY_CHUNK_BYTES};
use prost::Message;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tonic::transport::Channel;

pub(super) type Client = pb::kanban_service_client::KanbanServiceClient<Channel>;

pub(super) async fn client(host: &Host) -> Client {
    Client::connect(host.url.clone())
        .await
        .unwrap()
        .max_encoding_message_size(kanban_protocol::rpc::MAX_MESSAGE_BYTES)
        .max_decoding_message_size(kanban_protocol::rpc::MAX_MESSAGE_BYTES)
}

fn input<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

pub(super) async fn task(client: &mut Client, board: &str, title: &str) -> String {
    let response: dto::CreateTaskResponse = client
        .create_task(
            pb::CreateTaskRequest::from_parts(
                dto::CreateTaskPath {
                    board: board.into(),
                },
                (),
                input(json!({"title": title, "description":"G08 完整协议恢复", "status":"todo"})),
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    response.data.id
}

pub(super) async fn title(client: &mut Client, task: &str, title: &str) {
    client
        .update_task(
            pb::UpdateTaskRequest::from_parts(
                dto::UpdateTaskPath {
                    task_id: task.into(),
                },
                (),
                input(json!({"title": title})),
            )
            .unwrap(),
        )
        .await
        .unwrap();
}

pub(super) async fn comment(client: &mut Client, task: &str, body: &str) {
    client
        .create_comment(
            pb::CreateCommentRequest::from_parts(
                dto::CreateCommentPath {
                    task_id: task.into(),
                },
                (),
                input(json!({"body": body, "author": "G08 协议验证"})),
            )
            .unwrap(),
        )
        .await
        .unwrap();
}

pub(super) fn task_query(task: &str) -> Query {
    Query::GetTask(pb::GetTaskRequest {
        task_id: Some(task.into()),
        ..Default::default()
    })
}

pub(super) fn comments_query(task: &str) -> Query {
    Query::ListComments(pb::ListCommentsRequest {
        task_id: Some(task.into()),
    })
}

pub(super) async fn authoritative(client: &mut Client, query: &Query) -> Vec<u8> {
    use pb::query_result::Result;
    let result = match query {
        Query::GetTask(request) => {
            Result::GetTask(client.get_task(request.clone()).await.unwrap().into_inner())
        }
        Query::ListComments(request) => Result::ListComments(
            client
                .list_comments(request.clone())
                .await
                .unwrap()
                .into_inner(),
        ),
        Query::ListBoards(request) => {
            Result::ListBoards(client.list_boards(*request).await.unwrap().into_inner())
        }
        _ => panic!("夹具未声明的权威查询"),
    };
    pb::QueryResult {
        result: Some(result),
    }
    .encode_to_vec()
}

pub(super) fn request(query: Query, resume: Option<pb::QueryCursor>) -> pb::WatchQueriesRequest {
    pb::WatchQueriesRequest {
        protocol_version: kanban_protocol::rpc::query::PROTOCOL_VERSION,
        queries: vec![pb::QueryDefinition {
            client_query_id: "query".into(),
            projection_version: kanban_protocol::rpc::query::PROJECTION_VERSION,
            query: Some(query),
            resume,
            refresh: false,
        }],
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Transport {
    Native,
    Web,
}

impl Transport {
    pub(super) async fn watch(
        self,
        host: &Host,
        query: Query,
        resume: Option<pb::QueryCursor>,
    ) -> Wire {
        let request = request(query, resume);
        match self {
            Self::Native => Wire::Native(Box::new(
                pb::query_service_client::QueryServiceClient::connect(host.url.clone())
                    .await
                    .unwrap()
                    .watch_queries(request)
                    .await
                    .unwrap()
                    .into_inner(),
            )),
            Self::Web => {
                let response = reqwest::Client::new()
                    .post(format!("{}/kanban.v1.QueryService/WatchQueries", host.url))
                    .header("content-type", "application/grpc-web+proto")
                    .header("x-grpc-web", "1")
                    .header("origin", &host.url)
                    .body(super::super::frame(request))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.version(), reqwest::Version::HTTP_11);
                assert_eq!(response.status(), reqwest::StatusCode::OK);
                assert_eq!(response.headers()["access-control-allow-origin"], host.url);
                Wire::Web {
                    response: Box::new(response),
                    buffered: Vec::new(),
                }
            }
        }
    }
}

pub(super) enum Wire {
    Native(Box<tonic::Streaming<pb::QueryFrame>>),
    Web {
        response: Box<reqwest::Response>,
        buffered: Vec<u8>,
    },
}

impl Wire {
    pub(super) async fn next(&mut self) -> pb::QueryFrame {
        let frame = tokio::time::timeout(Duration::from_secs(5), async {
            match self {
                Self::Native(stream) => stream.message().await.unwrap().expect("意外 EOF"),
                Self::Web { response, buffered } => loop {
                    if buffered.len() >= 5 {
                        let length =
                            u32::from_be_bytes(buffered[1..5].try_into().unwrap()) as usize;
                        assert!(length <= kanban_protocol::rpc::query::MAX_QUERY_FRAME_BYTES);
                        if buffered.len() >= length + 5 {
                            assert_eq!(buffered[0], 0, "期望 binary protobuf 帧，非压缩或 trailer");
                            let frame = pb::QueryFrame::decode(&buffered[5..length + 5]).unwrap();
                            buffered.drain(..length + 5);
                            break frame;
                        }
                    }
                    buffered.extend(response.chunk().await.unwrap().expect("意外 gRPC-Web EOF"));
                },
            }
        })
        .await
        .unwrap();
        assert!(frame.encoded_len() <= kanban_protocol::rpc::query::MAX_QUERY_FRAME_BYTES);
        assert!(frame.client_query_id == "query" || matches!(frame.body, Some(Body::Heartbeat(_))));
        frame
    }
}

#[derive(Clone, Default)]
pub(super) struct Projection {
    pub bytes: Vec<u8>,
    pub cursor: Option<pb::QueryCursor>,
    pending: Option<(pb::QueryBegin, Vec<u8>, u32)>,
}

impl Projection {
    pub(super) fn discard_partial(&mut self) {
        self.pending = None;
    }

    pub(super) fn apply(&mut self, frame: pb::QueryFrame) -> Option<pb::QueryBegin> {
        match frame.body.unwrap() {
            Body::Begin(begin) => {
                assert!(self.pending.is_none());
                assert!(
                    begin
                        .cursor
                        .as_ref()
                        .is_some_and(|cursor| cursor.revision > 0)
                );
                assert_eq!(
                    begin.chunk_count as usize,
                    (begin.patch_size as usize).div_ceil(QUERY_CHUNK_BYTES)
                );
                if begin.snapshot {
                    assert!(begin.base.is_none());
                    assert_eq!(
                        (begin.offset, begin.delete_length, begin.patch_size),
                        (0, 0, begin.result_size)
                    );
                } else {
                    assert_eq!(begin.base, self.cursor);
                    let base = begin.base.as_ref().unwrap();
                    let cursor = begin.cursor.as_ref().unwrap();
                    assert_eq!((&base.epoch, &base.scope), (&cursor.epoch, &cursor.scope));
                    assert!(cursor.revision > base.revision);
                }
                self.pending = Some((begin, Vec::new(), 0));
            }
            Body::Chunk(chunk) => {
                let (begin, patch, index) = self.pending.as_mut().unwrap();
                assert_eq!(chunk.index, *index);
                assert!(chunk.index < begin.chunk_count);
                assert_eq!(
                    chunk.data.len(),
                    (begin.patch_size as usize - patch.len()).min(QUERY_CHUNK_BYTES)
                );
                patch.extend(chunk.data);
                *index += 1;
            }
            Body::End(end) => {
                let (begin, patch, count) = self.pending.take().unwrap();
                assert_eq!(end.cursor, begin.cursor);
                assert_eq!(count, begin.chunk_count);
                assert_eq!(patch.len() as u64, begin.patch_size);
                let bytes = if begin.snapshot {
                    patch
                } else {
                    let mut bytes = self.bytes.clone();
                    bytes.splice(
                        begin.offset as usize..(begin.offset + begin.delete_length) as usize,
                        patch,
                    );
                    bytes
                };
                assert_eq!(bytes.len() as u64, begin.result_size);
                assert_eq!(Sha256::digest(&bytes).as_slice(), begin.result_sha256);
                pb::QueryResult::decode(bytes.as_slice()).unwrap();
                self.bytes = bytes;
                self.cursor = end.cursor;
                return Some(begin);
            }
            Body::Ready(ready) => assert_eq!(ready.cursor, self.cursor),
            Body::Heartbeat(_) => {}
            Body::Failure(failure) => panic!("意外 query failure: {failure:?}"),
        }
        None
    }

    pub(super) async fn transaction(&mut self, wire: &mut Wire) -> pb::QueryBegin {
        loop {
            if let Some(begin) = self.apply(wire.next().await) {
                return begin;
            }
        }
    }
}

pub(super) async fn idle(host: &Host) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while host.state.grpc_probe.query_resources() != Some((0, 16, 0)) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("订阅清空后仍持有 Hub、permit 或完整字节");
}
