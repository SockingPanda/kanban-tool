//! 真实 transport 断开应立即释放查询资源，不等待下一次 mutation 或 heartbeat。
use super::{Host, pb, request};
use std::time::{Duration, Instant};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

async fn resources(host: &Host, hubs: usize, permits: usize) -> usize {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let (active, available, bytes) = host.state.grpc_probe.query_resources().unwrap();
            if active == hubs && available == permits && (hubs != 0) == (bytes != 0) {
                return bytes;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("真实断开必须在 heartbeat 之前回收 Hub、permit 和发送字节")
}

async fn http1_query(host: &Host) -> TcpStream {
    let authority = host.url.strip_prefix("http://").unwrap();
    let mut socket = TcpStream::connect(authority).await.unwrap();
    let body = super::super::frame(request());
    let head = format!(
        "POST /kanban.v1.QueryService/WatchQueries HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/grpc-web+proto\r\nx-grpc-web: 1\r\nContent-Length: {}\r\n\r\n",
        body.len(),
    );
    socket.write_all(head.as_bytes()).await.unwrap();
    socket.write_all(&body).await.unwrap();
    let mut response = [0; 12];
    tokio::time::timeout(Duration::from_secs(2), socket.read_exact(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&response, b"HTTP/1.1 200");
    socket
}

#[tokio::test]
async fn http1_grpc_web_disconnect_releases_query_before_heartbeat() {
    let host = Host::start().await;
    let mut first = http1_query(&host).await;
    let second = http1_query(&host).await;
    resources(&host, 1, 14).await;

    // Host 不接受 HTTP/1 half-close；发送 FIN 后仍保留 socket，证明回收不依赖客户端 Drop。
    let started = Instant::now();
    first.shutdown().await.unwrap();
    resources(&host, 1, 15).await;
    drop(second);
    resources(&host, 0, 16).await;
    println!(
        "G07_QUERY_DISCONNECT transport=http1-grpc-web connections=2 reclaimed_ms={:.3} hubs=0 available_permits=16 retained_bytes=0 half_closed_peer_still_held=true",
        started.elapsed().as_secs_f64() * 1000.0,
    );
    drop(first);
    host.finish().await;
}

#[tokio::test]
async fn h2_stream_reset_releases_query_while_channel_remains_connected() {
    let host = Host::start().await;
    let mut client = pb::query_service_client::QueryServiceClient::connect(host.url.clone())
        .await
        .unwrap();
    let mut first = client.watch_queries(request()).await.unwrap().into_inner();
    let mut second = client.watch_queries(request()).await.unwrap().into_inner();
    first.message().await.unwrap().unwrap();
    second.message().await.unwrap().unwrap();
    resources(&host, 1, 14).await;

    let started = Instant::now();
    drop(first);
    resources(&host, 1, 15).await;
    drop(second);
    resources(&host, 0, 16).await;
    println!(
        "G07_QUERY_DISCONNECT transport=http2-rst-stream connections=2 reclaimed_ms={:.3} hubs=0 available_permits=16 retained_bytes=0 client_channel_still_held=true",
        started.elapsed().as_secs_f64() * 1000.0,
    );
    // 相同 channel 仍可继续请求，取消一条流没有误关整条连接。
    let mut replacement = client.watch_queries(request()).await.unwrap().into_inner();
    replacement.message().await.unwrap().unwrap();
    resources(&host, 1, 15).await;
    drop(replacement);
    resources(&host, 0, 16).await;
    drop(client);
    host.finish().await;
}
