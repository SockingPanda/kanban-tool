//! 同一个真实 Host 的原生 HTTP/2、gRPC-Web、写入提示和退出验证。
mod business;
mod refresh;
use std::{net::SocketAddr, time::Duration};

use kanban_rpc_proto::v1::{self as pb, workspace_service_client::WorkspaceServiceClient};
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

    async fn client(&self) -> WorkspaceServiceClient<Channel> {
        WorkspaceServiceClient::connect(self.url.clone())
            .await
            .unwrap()
    }

    fn request(&self) -> pb::WatchChangesRequest {
        pb::WatchChangesRequest {
            board_id: self.board.clone(),
            protocol_version: 1,
        }
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

async fn next(stream: &mut tonic::Streaming<pb::WorkspaceChangeFrame>) -> pb::WorkspaceChangeFrame {
    tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn native_authority_and_web_frames_share_host_and_graceful_shutdown() {
    let host = Host::start().await;
    // tonic 原生 HTTP/2 使用 :authority，不能要求额外 HTTP/1 Host header。
    let mut native = host.client().await;
    let mut stream = native
        .watch_changes(host.request())
        .await
        .unwrap()
        .into_inner();
    let first = next(&mut stream).await;
    assert_eq!(first.board_id, host.board);
    assert_eq!(first.sequence, 1);
    let mut web = reqwest::Client::new()
        .post(format!(
            "{}/kanban.framework.v1.WorkspaceService/WatchChanges",
            host.url
        ))
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .header("origin", &host.url)
        .body(frame(host.request()))
        .send()
        .await
        .unwrap();
    assert_eq!(web.status(), reqwest::StatusCode::OK);
    assert_eq!(web.headers()["access-control-allow-origin"], host.url);
    assert!(
        web.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/grpc-web")
    );
    let mut bytes = Vec::new();
    let expected = loop {
        bytes.extend(
            tokio::time::timeout(Duration::from_secs(5), web.chunk())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
        );
        if bytes.len() >= 5 {
            let len = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
            if bytes.len() >= len + 5 {
                break len;
            }
        }
    };
    assert_eq!(bytes[0], 0);
    let received = pb::WorkspaceChangeFrame::decode(&bytes[5..expected + 5]).unwrap();
    assert_eq!(received.board_id, first.board_id);
    assert_eq!(received.body, first.body);
    host.stop.send_replace(ShutdownSignal::Graceful);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), stream.message())
            .await
            .unwrap()
            .unwrap()
            .is_none()
    );
    while let Some(chunk) = tokio::time::timeout(Duration::from_secs(5), web.chunk())
        .await
        .unwrap()
        .unwrap()
    {
        bytes.extend(chunk);
    }
    let trailer = &bytes[5 + expected..];
    assert_eq!(trailer[0], 0x80);
    let len = u32::from_be_bytes(trailer[1..5].try_into().unwrap()) as usize;
    assert_eq!(trailer.len(), len + 5);
    assert!(
        std::str::from_utf8(&trailer[5..])
            .unwrap()
            .split("\r\n")
            .any(|line| line
                .split_once(':')
                .is_some_and(|(key, value)| key == "grpc-status" && value.trim() == "0"))
    );
    host.finish().await;
}

#[tokio::test]
async fn non_card_http_mutation_notifies_native_stream_from_same_service_gate() {
    let host = Host::start().await;
    let http = reqwest::Client::new();
    let created = http.post(format!("{}/api/v1/boards/default/tasks", host.url))
        .header("content-type", "application/json")
        .body(r#"{"task_id":"t_grpc_comment","title":"RPC fixture","priority":1,"labels":[],"depends_on":[]}"#)
        .send().await.unwrap();
    assert_eq!(created.status(), reqwest::StatusCode::CREATED);
    let task = host
        .state
        .application()
        .get_task("t_grpc_comment")
        .await
        .unwrap();
    let mut native = host.client().await;
    let mut stream = native
        .watch_changes(host.request())
        .await
        .unwrap()
        .into_inner();
    let first = next(&mut stream).await;
    let response = http
        .post(format!("{}/api/v1/tasks/t_grpc_comment/comments", host.url))
        .header("content-type", "application/json")
        .body(r#"{"author":"writer","author_type":"agent","kind":"note","body":"non-card change"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let changed = next(&mut stream).await;
    assert_eq!(changed.epoch, first.epoch);
    assert_eq!(changed.sequence, 2);
    assert_eq!(
        host.state
            .application()
            .get_task("t_grpc_comment")
            .await
            .unwrap()
            .lock_version,
        task.lock_version
    );
    drop(stream);
    host.finish().await;
}

#[tokio::test]
async fn rejects_invalid_origin_board_version_and_exposes_rpc_cors_metadata() {
    let host = Host::start().await;
    let mut native = host.client().await;
    let mut request = host.request();
    request.protocol_version = 2;
    assert_eq!(
        native.watch_changes(request).await.unwrap_err().code(),
        Code::FailedPrecondition
    );
    let mut request = host.request();
    request.board_id = "b_missing".into();
    assert_eq!(
        native.watch_changes(request).await.unwrap_err().code(),
        Code::NotFound
    );
    let denied = reqwest::Client::new()
        .post(format!(
            "{}/kanban.framework.v1.WorkspaceService/WatchChanges",
            host.url
        ))
        .header("origin", "https://untrusted.invalid")
        .header("content-type", "application/grpc-web+proto")
        .body(frame(host.request()))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), reqwest::StatusCode::BAD_REQUEST);
    let options = reqwest::Client::new()
        .request(
            reqwest::Method::OPTIONS,
            format!(
                "{}/kanban.framework.v1.WorkspaceService/WatchChanges",
                host.url
            ),
        )
        .header("origin", &host.url)
        .header("access-control-request-method", "POST")
        .header(
            "access-control-request-headers",
            "content-type,x-grpc-web,x-user-agent,grpc-timeout,x-kb-actor",
        )
        .send()
        .await
        .unwrap();
    assert!(options.status().is_success());
    let allowed = options.headers()["access-control-allow-headers"]
        .to_str()
        .unwrap();
    for header in [
        "content-type",
        "x-grpc-web",
        "x-user-agent",
        "grpc-timeout",
        "x-kb-actor",
    ] {
        assert!(allowed.contains(header));
    }
    host.finish().await;
}

#[tokio::test]
async fn native_and_web_share_subscription_budget_and_cancellation_releases_it() {
    let host = Host::start().await;
    let mut client = host.client().await;
    let mut streams = Vec::new();
    for _ in 0..16 {
        let mut stream = client
            .watch_changes(host.request())
            .await
            .unwrap()
            .into_inner();
        next(&mut stream).await;
        streams.push(stream);
    }
    assert_eq!(
        client
            .watch_changes(host.request())
            .await
            .unwrap_err()
            .code(),
        Code::ResourceExhausted
    );
    let web = reqwest::Client::new()
        .post(format!(
            "{}/kanban.framework.v1.WorkspaceService/WatchChanges",
            host.url
        ))
        .header("content-type", "application/grpc-web+proto")
        .body(frame(host.request()))
        .send()
        .await
        .unwrap();
    assert_eq!(web.headers()["grpc-status"], "8");
    drop(streams.pop());
    let replacement = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match client.watch_changes(host.request()).await {
                Ok(response) => break response.into_inner(),
                Err(error) if error.code() == Code::ResourceExhausted => {
                    tokio::time::sleep(Duration::from_millis(10)).await
                }
                Err(error) => panic!("unexpected status: {error}"),
            }
        }
    })
    .await
    .unwrap();
    drop(replacement);
    drop(streams);
    host.finish().await;
}

#[tokio::test]
async fn force_shutdown_and_cancelled_host_signal_and_release_streams() {
    let host = Host::start().await;
    let mut native = host.client().await;
    let mut stream = native
        .watch_changes(host.request())
        .await
        .unwrap()
        .into_inner();
    next(&mut stream).await;
    host.stop.send_replace(ShutdownSignal::Force);
    let error = tokio::time::timeout(Duration::from_secs(5), host.server)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert!(*host.state.event_stream_shutdown_receiver().borrow());
    let ended = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap();
    assert!(ended.is_err() || ended.unwrap().is_none());

    let host = Host::start().await;
    let mut native = host.client().await;
    let mut stream = native
        .watch_changes(host.request())
        .await
        .unwrap()
        .into_inner();
    next(&mut stream).await;
    host.server.abort();
    assert!(host.server.await.unwrap_err().is_cancelled());
    assert!(*host.state.event_stream_shutdown_receiver().borrow());
    let ended = tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap();
    assert!(ended.is_err() || ended.unwrap().is_none());
}

#[tokio::test]
async fn plain_shutdown_entrypoint_closes_active_rpc_stream() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("canonical.db"), "test")
        .await
        .unwrap();
    let board = state.application().get_board("default").await.unwrap().id;
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr: SocketAddr = reservation.local_addr().unwrap();
    drop(reservation);
    let (send, receive) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(crate::serve_with_shutdown(addr, state, async {
        let _ = receive.await;
    }));
    let mut client = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(client) = WorkspaceServiceClient::connect(format!("http://{addr}")).await {
                break client;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut stream = client
        .watch_changes(pb::WatchChangesRequest {
            board_id: board,
            protocol_version: 1,
        })
        .await
        .unwrap()
        .into_inner();
    next(&mut stream).await;
    send.send(()).unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), stream.message())
            .await
            .unwrap()
            .unwrap()
            .is_none()
    );
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn rpc_timeout_covers_stream_lifetime_and_rejects_invalid_metadata() {
    let host = Host::start().await;
    let mut native = host.client().await;
    let mut request = tonic::Request::new(host.request());
    request.set_timeout(Duration::from_millis(100));
    let mut stream = native.watch_changes(request).await.unwrap().into_inner();
    next(&mut stream).await;
    let error = tokio::time::timeout(Duration::from_secs(2), stream.message())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code(), Code::DeadlineExceeded);

    let web = reqwest::Client::new()
        .post(format!(
            "{}/kanban.framework.v1.WorkspaceService/WatchChanges",
            host.url
        ))
        .header("content-type", "application/grpc-web+proto")
        .header("grpc-timeout", "100m")
        .body(frame(host.request()))
        .send()
        .await
        .unwrap();
    let bytes = tokio::time::timeout(Duration::from_secs(2), web.bytes())
        .await
        .unwrap()
        .unwrap();
    let first = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
    let trailer = &bytes[5 + first..];
    assert_eq!(trailer[0], 0x80);
    assert!(
        std::str::from_utf8(&trailer[5..])
            .unwrap()
            .split("\r\n")
            .any(|line| line
                .split_once(':')
                .is_some_and(|(key, value)| key == "grpc-status" && value.trim() == "4"))
    );
    for bad in ["1x", "123456789m", "-1S", ""] {
        let mut request = tonic::Request::new(host.request());
        request
            .metadata_mut()
            .insert("grpc-timeout", bad.parse().unwrap());
        assert_eq!(
            native.watch_changes(request).await.unwrap_err().code(),
            Code::InvalidArgument
        );
    }
    host.finish().await;
}

/// 由专用 recipe 准备生成 JS 后运行，普通 Cargo gate 不隐式安装 Node 依赖。
#[tokio::test]
#[ignore = "运行 just grpc-fetch-check 准备生成客户端"]
async fn generated_connect_client_uses_fetch_against_current_host() {
    let host = Host::start().await;
    let package =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migration/grpc-framework/web");
    let output = tokio::time::timeout(Duration::from_secs(15), tokio::process::Command::new("node")
        .current_dir(package).arg("--input-type=module").arg("-e").arg(r#"
import assert from 'node:assert/strict';
import {createClient, Code, ConnectError} from '@connectrpc/connect';
import {createGrpcWebTransport} from '@connectrpc/connect-web';
import {WorkspaceService} from './dist/gen/kanban/framework/v1/board_pb.js';
const [baseUrl, boardId] = process.argv.slice(1);
const requests = [];
const client = createClient(WorkspaceService, createGrpcWebTransport({
    baseUrl,
    fetch: async (input, init) => {
        const headers = new Headers(init.headers);
        headers.set('origin', baseUrl);
        requests.push({url:String(input), contentType:headers.get('content-type')});
        return fetch(input, {...init, headers});
    },
}));
const controller = new AbortController();
let attached = false;
try {
    for await (const frame of client.watchChanges({boardId, protocolVersion:1}, {signal:controller.signal})) {
        assert.equal(frame.boardId, boardId);
        assert.equal(frame.sequence, 1n);
        assert.equal(frame.body.case, 'invalidated');
        attached = true;
        controller.abort();
    }
} catch(error) { assert.equal(ConnectError.from(error).code, Code.Canceled); }
assert(attached);
assert.equal(requests.length, 1);
assert(requests[0].url.endsWith('/kanban.framework.v1.WorkspaceService/WatchChanges'));
assert.equal(requests[0].contentType, 'application/grpc-web+proto');
console.log('generated Connect client: binary Fetch stream, typed frame and cancellation passed');
"#).arg(&host.url).arg(&host.board).output()).await.unwrap().unwrap();
    assert!(
        output.status.success(),
        "Fetch 验证失败: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("{}", String::from_utf8_lossy(&output.stdout));
    host.finish().await;
}
