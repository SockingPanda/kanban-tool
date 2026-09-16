use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use kanban_client::KanbanClient;
use kanban_server::{AppState, ShutdownSignal, serve_with_dispatcher_shutdown};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, watch},
    task::{JoinHandle, JoinSet},
    time::{sleep, timeout},
};

pub const WAIT: Duration = Duration::from_secs(5);

pub fn input<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

pub fn task_request(id: &str) -> kanban_protocol::CreateTaskRequest {
    input(
        json!({"task_id": id, "idempotency_key": format!("create:{id}"), "title": "原生 RPC 任务", "description": "详细说明", "status": "todo", "metadata": {"wide": 9007199254740993_i64, "maximum": u64::MAX}}),
    )
}

pub struct Host {
    pub directory: tempfile::TempDir,
    pub client: KanbanClient,
    stop: watch::Sender<ShutdownSignal>,
    server: Option<JoinHandle<std::io::Result<()>>>,
}

impl Host {
    pub async fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("canonical.db"), "fixture-host")
            .await
            .unwrap();
        let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let (stop, receiver) = watch::channel(ShutdownSignal::Running);
        let server = tokio::spawn(serve_with_dispatcher_shutdown(
            address, state, None, receiver,
        ));
        let client = KanbanClient::new(format!("http://{address}"), "原生作者").unwrap();
        timeout(WAIT, async {
            loop {
                if client.health().await.is_ok() {
                    break;
                }
                assert!(!server.is_finished(), "隔离 Host 提前退出");
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        Self {
            directory,
            client,
            stop,
            server: Some(server),
        }
    }

    pub async fn finish(mut self) {
        self.stop.send_replace(ShutdownSignal::Graceful);
        timeout(WAIT, self.server.take().unwrap())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
        }
    }
}

pub struct Http2Peer {
    pub client: KanbanClient,
    pub accepted: Arc<AtomicUsize>,
    cut: mpsc::Sender<()>,
    server: JoinHandle<()>,
}

impl Http2Peer {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = KanbanClient::new(
            format!("http://{}", listener.local_addr().unwrap()),
            "代理作者",
        )
        .unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = accepted.clone();
        let (cut, mut receiver) = mpsc::channel(1);
        let server = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    Some(()) = receiver.recv() => {
                        connections.abort_all();
                        while connections.join_next().await.is_some() {}
                    }
                    incoming = listener.accept() => {
                        let (mut downstream, _) = incoming.unwrap();
                        count.fetch_add(1, Ordering::SeqCst);
                        connections.spawn(async move {
                            let mut preface = [0; 24];
                            downstream.read_exact(&mut preface).await.unwrap();
                            assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
                            downstream.write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0]).await.unwrap();
                            loop {
                                let (kind, stream, _) = next_http2_frame(&mut downstream).await;
                                if kind == 1 && stream > 0 {
                                    // :status 200 与 application/grpc；空 protobuf 是有效的空 ListBoardsResponse。
                                    let mut headers = vec![0x88, 0x0f, 0x10, 16];
                                    headers.extend_from_slice(b"application/grpc");
                                    write_http2_frame(&mut downstream, 1, 4, stream, &headers).await;
                                    write_http2_frame(&mut downstream, 0, 0, stream, &[0, 0, 0, 0, 0]).await;
                                    let mut trailers = vec![0, 11];
                                    trailers.extend_from_slice(b"grpc-status");
                                    trailers.extend_from_slice(&[1, b'0']);
                                    write_http2_frame(&mut downstream, 1, 5, stream, &trailers).await;
                                }
                            }
                        });
                    }
                    Some(result) = connections.join_next(), if !connections.is_empty() => { result.unwrap(); }
                }
            }
        });
        Self {
            client,
            accepted,
            cut,
            server,
        }
    }

    pub async fn disconnect(&self) {
        self.cut.send(()).await.unwrap();
    }
}

impl Drop for Http2Peer {
    fn drop(&mut self) {
        self.server.abort();
    }
}

pub async fn next_http2_frame(socket: &mut TcpStream) -> (u8, u32, Vec<u8>) {
    timeout(WAIT, async {
        let mut header = [0; 9];
        socket.read_exact(&mut header).await.unwrap();
        let len =
            usize::from(header[0]) << 16 | usize::from(header[1]) << 8 | usize::from(header[2]);
        assert!(len <= 16_384);
        let stream = u32::from_be_bytes(header[5..9].try_into().unwrap()) & 0x7fff_ffff;
        let mut payload = vec![0; len];
        socket.read_exact(&mut payload).await.unwrap();
        (header[3], stream, payload)
    })
    .await
    .expect("原生 HTTP/2 frame 超时")
}

pub async fn stalled_request() -> (
    JoinHandle<Result<Vec<kanban_protocol::ApiBoard>, kanban_client::ClientError>>,
    TcpStream,
    u32,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = KanbanClient::new(
        format!("http://{}", listener.local_addr().unwrap()),
        "cancel",
    )
    .unwrap();
    let request = tokio::spawn(async move { client.list_boards(false).await });
    let (mut socket, _) = timeout(WAIT, listener.accept()).await.unwrap().unwrap();
    let mut preface = [0; 24];
    socket.read_exact(&mut preface).await.unwrap();
    assert_eq!(&preface, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
    socket
        .write_all(&[0, 0, 0, 4, 0, 0, 0, 0, 0])
        .await
        .unwrap();
    let stream_id = loop {
        let (kind, stream, _) = next_http2_frame(&mut socket).await;
        if kind == 1 {
            assert_ne!(stream, 0);
            break stream;
        }
    };
    (request, socket, stream_id)
}

async fn write_http2_frame(
    socket: &mut TcpStream,
    kind: u8,
    flags: u8,
    stream: u32,
    payload: &[u8],
) {
    let mut header = [0; 9];
    let length = u32::try_from(payload.len()).unwrap().to_be_bytes();
    header[..3].copy_from_slice(&length[1..]);
    header[3] = kind;
    header[4] = flags;
    header[5..].copy_from_slice(&stream.to_be_bytes());
    socket.write_all(&header).await.unwrap();
    socket.write_all(payload).await.unwrap();
}
