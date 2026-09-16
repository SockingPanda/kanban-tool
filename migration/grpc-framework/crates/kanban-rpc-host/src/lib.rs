//! 原生 gRPC 和 gRPC-Web 共享 service；没有 REST 转发或 SSE fallback。
mod frames;
mod service;
mod refresh;
pub use service::RpcApp;
pub use frames::{to_card, to_status};
use std::{error::Error, time::Duration};
use tokio::{net::TcpListener, sync::watch};
use kanban_rpc_proto::v1::{board_service_server::BoardServiceServer, task_service_server::TaskServiceServer};

pub type HostResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
pub async fn wait_stop(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow() { return; }
        if rx.changed().await.is_err() { return; }
    }
}
/// 演示宿主和测试入口。同一个 listener 同时接受原生 gRPC 与 gRPC-Web。
/// 接入 main 时由现有 serve 拥有 listener/关闭流程，不启动第二个 canonical host。
pub async fn serve(listener: TcpListener, app: RpcApp, stop: watch::Receiver<bool>) -> HostResult<()> {
    use http::{HeaderName, Method, header};
    use tonic::transport::Server;
    use tower_http::cors::{AllowOrigin, CorsLayer};
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "只允许 loopback listener").into());
    }
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(app.origins().to_vec()))
        .allow_methods([Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, HeaderName::from_static("x-grpc-web"),
            HeaderName::from_static("x-user-agent"), HeaderName::from_static("grpc-timeout")])
        .expose_headers([HeaderName::from_static("grpc-status"), HeaderName::from_static("grpc-message"),
            HeaderName::from_static("kb-error-bin")]);
    let shutdown_app = app.clone();
    let mut graceful = stop.clone();
    let mut forced = stop;
    let server = Server::builder()
        .accept_http1(true)
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(BoardServiceServer::new(app.clone()).max_decoding_message_size(64 * 1024).max_encoding_message_size(4 * 1024 * 1024))
        .add_service(kanban_rpc_proto::v1::workspace_service_server::WorkspaceServiceServer::new(app.clone()).max_decoding_message_size(4096).max_encoding_message_size(4096))
        .add_service(TaskServiceServer::new(app).max_decoding_message_size(64 * 1024).max_encoding_message_size(64 * 1024))
        .serve_with_incoming_shutdown(tokio_stream::wrappers::TcpListenerStream::new(listener), async move {
            wait_stop(&mut graceful).await;
            shutdown_app.stop().await;
        });
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => result.map_err(Into::into),
        _ = wait_stop(&mut forced) => {
            match tokio::time::timeout(Duration::from_secs(5), &mut server).await {
                Ok(result) => result.map_err(Into::into),
                Err(_) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "RPC graceful shutdown 超时，已停止等待").into()),
            }
        }
    }
}

/// 提供给现有 Axum host 的 Tower service，避免合并不同版本的 Axum Router。
/// host 必须开启 HTTP/2、执行 Origin 策略，并在关闭时调用 RpcApp::stop。
pub fn workspace_service(app: RpcApp) -> tonic_web::GrpcWebService<kanban_rpc_proto::v1::workspace_service_server::WorkspaceServiceServer<RpcApp>> {
    use tower::Layer;
    let service = kanban_rpc_proto::v1::workspace_service_server::WorkspaceServiceServer::new(app)
        .max_decoding_message_size(4096).max_encoding_message_size(4096);
    tonic_web::GrpcWebLayer::new().layer(service)
}
