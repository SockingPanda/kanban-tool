//! 真实 tonic/gRPC-Web 互通测试源码。本交付环境没有 Rust，执行状态见 validation。
use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use kanban_live_core::*;
use kanban_rpc_proto::v1 as pb;
use kanban_rpc_host::{RpcApp, serve};
use tokio::sync::watch;
use prost::Message;

struct Source { changed: watch::Sender<u64>, reads: AtomicUsize }
impl RefreshSource for Source {
    fn subscribe_refreshes(&self) -> watch::Receiver<u64> { self.changed.subscribe() }
    fn check_board<'a>(&'a self, board: &'a str) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if board == "b_default" { Ok(()) } else { Err(LiveError::NotFound(board.into())) }
        })
    }
}
impl TaskCommands for Source {
    fn update_title(&self, _: UpdateTitle) -> BoxFuture<'_, Result<Card>> {
        Box::pin(async { Err(LiveError::Invalid("此测试不写任务".into())) })
    }
}
async fn fixture(enabled: bool) -> (String, Arc<Source>, watch::Sender<bool>, tokio::task::JoinHandle<kanban_rpc_host::HostResult<()>>) {
    let source = Arc::new(Source { changed: watch::channel(0).0, reads: AtomicUsize::new(0) });
    let app = RpcApp::new(vec![], source.clone(), vec!["http://127.0.0.1:1421".parse().unwrap()]).unwrap();
    let app = if enabled { app.with_refresh_source(source.clone()) } else { app };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx,rx) = watch::channel(false);
    let server = tokio::spawn(serve(listener,app,rx));
    (url,source,tx,server)
}
fn request() -> pb::WatchChangesRequest { pb::WatchChangesRequest { board_id: "b_default".into(), protocol_version: 1 } }
#[tokio::test]
async fn changes_without_card_diffs_still_invalidate_and_shutdown_closes_stream() {
    let (url,source,stop,server)=fixture(true).await;
    let mut client=pb::workspace_service_client::WorkspaceServiceClient::connect(url).await.unwrap();
    let mut stream=client.watch_changes(request()).await.unwrap().into_inner();
    let first=stream.message().await.unwrap().unwrap();
    assert_eq!(first.sequence,1);
    assert!(matches!(first.body,Some(pb::workspace_change_frame::Body::Invalidated(pb::QueriesInvalidated{reason:1}))));
    source.changed.send_modify(|v|*v+=1);
    let second=tokio::time::timeout(Duration::from_secs(2),stream.message()).await.unwrap().unwrap().unwrap();
    assert_eq!(second.sequence,2);assert_eq!(second.epoch,first.epoch);
    stop.send_replace(true);
    assert!(tokio::time::timeout(Duration::from_secs(2),stream.message()).await.unwrap().unwrap().is_none());
    server.await.unwrap().unwrap();
}
#[tokio::test]
async fn reconnect_always_invalidates_in_a_new_epoch() {
    let (url,_source,stop,server)=fixture(true).await;
    let mut client=pb::workspace_service_client::WorkspaceServiceClient::connect(url).await.unwrap();
    let mut stream=client.watch_changes(request()).await.unwrap().into_inner();let first=stream.message().await.unwrap().unwrap();drop(stream);
    let mut stream=client.watch_changes(request()).await.unwrap().into_inner();let second=stream.message().await.unwrap().unwrap();
    assert_eq!(second.sequence,1);assert_ne!(first.epoch,second.epoch);
    drop(stream);stop.send_replace(true);server.await.unwrap().unwrap();
}
#[tokio::test]
async fn missing_source_is_unimplemented_not_sse_fallback() {
    let (url,_source,stop,server)=fixture(false).await;
    let mut client=pb::workspace_service_client::WorkspaceServiceClient::connect(url).await.unwrap();
    assert_eq!(client.watch_changes(request()).await.unwrap_err().code(),tonic::Code::Unimplemented);
    stop.send_replace(true);server.await.unwrap().unwrap();
}
#[tokio::test]
async fn rejects_bad_board_and_origin_before_streaming() {
    let (url,_source,stop,server)=fixture(true).await;
    let mut client=pb::workspace_service_client::WorkspaceServiceClient::connect(url).await.unwrap();
    assert_eq!(client.watch_changes(pb::WatchChangesRequest {board_id:"b_missing".into(),protocol_version:1}).await.unwrap_err().code(),tonic::Code::NotFound);
    let mut req=tonic::Request::new(request());req.metadata_mut().insert("origin","https://untrusted.invalid".parse().unwrap());
    assert_eq!(client.watch_changes(req).await.unwrap_err().code(),tonic::Code::PermissionDenied);
    stop.send_replace(true);server.await.unwrap().unwrap();
}
#[tokio::test]
async fn grpc_web_consumes_refresh_as_protobuf_not_text_events() {
    let (url,_source,stop,server)=fixture(true).await;
    let data=request().encode_to_vec();let mut body=vec![0];body.extend_from_slice(&(data.len()as u32).to_be_bytes());body.extend(data);
    let mut response=reqwest::Client::new().post(format!("{url}/kanban.framework.v1.WorkspaceService/WatchChanges"))
        .header("content-type","application/grpc-web+proto").header("x-grpc-web","1")
        .header("origin","http://127.0.0.1:1421").body(body).send().await.unwrap();
    assert!(response.status().is_success());
    let mut bytes=Vec::new();
    let message=tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            bytes.extend(response.chunk().await.unwrap().expect("stream closed"));
            if bytes.len()<5 {continue;}
            let n=u32::from_be_bytes(bytes[1..5].try_into().unwrap())as usize;
            if bytes.len()>=5+n {assert_eq!(bytes[0],0);break pb::WorkspaceChangeFrame::decode(&bytes[5..5+n]).unwrap();}
        }
    }).await.unwrap();
    assert_eq!(message.sequence,1);assert_eq!(message.board_id,"b_default");
    drop(response);stop.send_replace(true);server.await.unwrap().unwrap();
}
