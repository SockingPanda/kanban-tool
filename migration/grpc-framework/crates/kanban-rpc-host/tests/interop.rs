//! 真网络测试。需要 Rust 依赖安装后运行；当前交付环境未执行。
use std::{sync::Arc, time::Duration};
use kanban_live_core::*;
use kanban_rpc_proto::v1::*;
use kanban_rpc_host::{RpcApp, serve};
use prost::Message;
use tokio::sync::watch;

struct NoCommands;
impl TaskCommands for NoCommands {
    fn update_title(&self, _: UpdateTitle) -> BoxFuture<'_, Result<Card>> {
        Box::pin(async { Err(LiveError::Invalid("此测试不执行命令".into())) })
    }
}
fn card() -> Card { Card { id: "t_demo".into(), title: "A".into(), status: Status::Todo,
    priority: 1, position: 0, seq: 1, lock_version: 0 } }
async fn fixture() -> (String, Arc<Hub>, watch::Sender<bool>, tokio::task::JoinHandle<kanban_rpc_host::HostResult<()>>) {
    let hub = Arc::new(Hub::new("b_default", Limits::default()).unwrap());
    hub.publish(vec![card()]).await.unwrap();
    let app = RpcApp::new(vec![hub.clone()], Arc::new(NoCommands), vec!["http://127.0.0.1:5173".parse().unwrap()]).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (stop, rx) = watch::channel(false);
    let server = tokio::spawn(serve(listener, app, rx));
    (url, hub, stop, server)
}
fn web_body(value: impl Message) -> Vec<u8> {
    let data = value.encode_to_vec();
    let mut frame = vec![0]; frame.extend_from_slice(&(data.len() as u32).to_be_bytes()); frame.extend(data); frame
}
#[tokio::test]
async fn native_and_grpc_web_use_same_protobuf_service() {
    let (url, _hub, stop, server) = fixture().await;
    let mut native = board_service_client::BoardServiceClient::connect(url.clone()).await.unwrap();
    let expected = native.get_board(GetBoardRequest { board_id: "b_default".into() }).await.unwrap().into_inner();
    let response = reqwest::Client::new().post(format!("{url}/kanban.framework.v1.BoardService/GetBoard"))
        .header("content-type", "application/grpc-web+proto").header("x-grpc-web", "1")
        .header("origin", "http://127.0.0.1:5173")
        .body(web_body(GetBoardRequest { board_id: "b_default".into() })).send().await.unwrap();
    assert!(response.status().is_success());
    let bytes = response.bytes().await.unwrap();
    assert_eq!(bytes[0], 0);
    let length = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
    let actual = GetBoardResponse::decode(&bytes[5..5 + length]).unwrap();
    assert_eq!(actual, expected);
    assert!(String::from_utf8_lossy(&bytes[5 + length..]).contains("grpc-status: 0"));
    stop.send_replace(true); server.await.unwrap().unwrap();
}
#[tokio::test]
async fn native_stream_sends_snapshot_then_atomic_delta() {
    let (url, hub, stop, server) = fixture().await;
    let mut native = board_service_client::BoardServiceClient::connect(url).await.unwrap();
    let mut stream = native.watch_board(WatchBoardRequest { board_id: "b_default".into(), resume: None, protocol_version: 1 }).await.unwrap().into_inner();
    let mut count = 0;
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), stream.message()).await.unwrap().unwrap().unwrap();
        count += 1;
        if matches!(frame.body, Some(board_frame::Body::SnapshotCommit(_))) { break; }
    }
    assert_eq!(count, 3);
    hub.publish(vec![Card { title: "B".into(), lock_version: 1, ..card() }]).await.unwrap();
    let frame = tokio::time::timeout(Duration::from_secs(2), stream.message()).await.unwrap().unwrap().unwrap();
    let Some(board_frame::Body::Delta(delta)) = frame.body else { panic!("delta expected") };
    assert_eq!((delta.base_revision, delta.revision), (1, 2));
    assert_eq!(delta.upserts[0].title, "B");
    drop(stream); stop.send_replace(true); server.await.unwrap().unwrap();
}
#[tokio::test]
async fn unknown_origin_is_rejected_before_command_dispatch() {
    let (url, _hub, stop, server) = fixture().await;
    let mut native = task_service_client::TaskServiceClient::connect(url).await.unwrap();
    let mut request = tonic::Request::new(UpdateTaskTitleRequest::default());
    request.metadata_mut().insert("origin", "https://untrusted.invalid".parse().unwrap());
    assert_eq!(native.update_task_title(request).await.unwrap_err().code(), tonic::Code::PermissionDenied);
    stop.send_replace(true); server.await.unwrap().unwrap();
}
