//! 同一个真实 Host 的原生 HTTP/2、gRPC-Web、写入提示和退出验证。
mod boundaries;
mod business;
mod mutations;
mod queries;
use std::time::Duration;

use kanban_protocol::rpc::v1::{self as pb, query_service_client::QueryServiceClient};
use prost::Message;
use tokio::{sync::watch, task::JoinHandle};
use tonic::{Code, transport::Channel};

use crate::{AppState, ShutdownSignal, serve_with_dispatcher_shutdown};

struct Host {
    _directory: tempfile::TempDir,
    state: AppState,
    board: String,
    url: String,
    stop: watch::Sender<ShutdownSignal>,
    server: JoinHandle<std::io::Result<()>>,
}

impl Host {
    async fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("canonical.db"), "grpc-test")
            .await
            .unwrap();
        let board = state.application().get_board("default").await.unwrap().id;
        let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = reservation.local_addr().unwrap();
        drop(reservation);
        let (stop, receiver) = watch::channel(ShutdownSignal::Running);
        let server = tokio::spawn(serve_with_dispatcher_shutdown(
            addr,
            state.clone(),
            None,
            receiver,
        ));
        let result = Self {
            _directory: directory,
            state,
            board,
            url: format!("http://{addr}"),
            stop,
            server,
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if reqwest::get(format!("{}/health", result.url)).await.is_ok() {
                    break;
                }
                assert!(!result.server.is_finished(), "Host 提前结束");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        result
    }

    async fn client(&self) -> QueryServiceClient<Channel> {
        QueryServiceClient::connect(self.url.clone()).await.unwrap()
    }

    fn request(&self) -> pb::WatchQueriesRequest {
        query_request(pb::query_definition::Query::ListTasks(
            pb::ListTasksRequest {
                board: Some(self.board.clone()),
                ..Default::default()
            },
        ))
    }

    async fn finish(self) {
        self.stop.send_replace(ShutdownSignal::Graceful);
        tokio::time::timeout(Duration::from_secs(5), self.server)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

fn frame(value: impl Message) -> Vec<u8> {
    let bytes = value.encode_to_vec();
    let mut result = vec![0];
    result.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_be_bytes());
    result.extend(bytes);
    result
}

fn query_request(query: pb::query_definition::Query) -> pb::WatchQueriesRequest {
    pb::WatchQueriesRequest {
        protocol_version: 1,
        queries: vec![pb::QueryDefinition {
            client_query_id: "query".into(),
            projection_version: 1,
            query: Some(query),
            resume: None,
            refresh: false,
        }],
    }
}

async fn ready(stream: &mut tonic::Streaming<pb::QueryFrame>) -> pb::QueryCursor {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let frame = stream.message().await.unwrap().unwrap();
            if let Some(pb::query_frame::Body::Ready(ready)) = frame.body {
                return ready.cursor.unwrap();
            }
        }
    })
    .await
    .unwrap()
}

async fn closed(stream: &mut tonic::Streaming<pb::QueryFrame>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while stream.message().await.is_ok_and(|frame| frame.is_some()) {}
    })
    .await
    .expect("Host 退出后 QueryService 未关闭");
}
