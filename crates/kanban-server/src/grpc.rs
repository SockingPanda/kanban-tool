//! 唯一 Host 的 RPC 装配与生命周期，不打开数据库、不绑定第二个端口。
use crate::AppState;
use axum::{Router, http::HeaderValue};
use tower::Layer;
mod adapter;
mod business;
mod context;
mod workspace;
use adapter::KanbanAdapter;
#[cfg(test)]
pub(crate) use adapter::probe::SourceProbe;
use kanban_rpc_host::RpcApp;
use kanban_service::{KanbanError, Result};
use std::sync::Arc;

pub struct WorkspaceRpcMount {
    pub router: Router,
    /// 跟随唯一 host 的关闭流程调用 stop；不能交给 detached task 永久持有。
    pub runtime: RpcApp,
}

/// 在唯一 Host 装配正式的具名业务 RPC 与 WorkspaceService。
/// allowed_origins 必须由实际 listener、已验证的 runtime 与开发配置确定，不接受 wildcard。
pub fn workspace_rpc_mount(
    state: &AppState,
    allowed_origins: Vec<HeaderValue>,
) -> Result<WorkspaceRpcMount> {
    let adapter = KanbanAdapter::new(state.application().clone());
    #[cfg(test)]
    let adapter = adapter.with_probe(state.grpc_probe.clone());
    let adapter = Arc::new(adapter);
    let runtime = RpcApp::new(Vec::new(), adapter.clone(), allowed_origins)
        .map_err(|error| KanbanError::InvalidInput(error.to_string()))?
        .with_refresh_source(adapter)
        .with_shutdown(state.stream_shutdown_sender());
    let router = Router::new().route_service(
        "/kanban.framework.v1.WorkspaceService/WatchChanges",
        kanban_rpc_host::workspace_service(runtime.clone()),
    );
    let business = kanban_protocol::rpc::v1::kanban_service_server::KanbanServiceServer::new(
        business::BusinessRpc {
            state: state.clone(),
        },
    )
    .max_decoding_message_size(kanban_protocol::rpc::MAX_MESSAGE_BYTES)
    .max_encoding_message_size(kanban_protocol::rpc::MAX_MESSAGE_BYTES);
    let workspace =
        kanban_protocol::rpc::v1::workspace_service_server::WorkspaceServiceServer::new(
            workspace::WorkspaceRpc(runtime.clone()),
        )
        .max_decoding_message_size(4096)
        .max_encoding_message_size(4096);
    let router = router
        .route_service(
            "/kanban.v1.KanbanService/*method",
            tonic_web::GrpcWebLayer::new().layer(kanban_rpc_host::Deadline::new(business)),
        )
        .route_service(
            "/kanban.v1.WorkspaceService/WatchChanges",
            tonic_web::GrpcWebLayer::new().layer(kanban_rpc_host::Deadline::new(workspace)),
        );
    Ok(WorkspaceRpcMount { router, runtime })
}

/// 即使 HTTP future 提前返回或被取消，也同步关闭所有订阅；正常退出再等待 Hub 收尾。
pub(crate) struct RpcLifetime(pub RpcApp);

impl RpcLifetime {
    pub(crate) async fn stop(&self) {
        self.0.stop().await;
    }
}

impl Drop for RpcLifetime {
    fn drop(&mut self) {
        self.0.begin_shutdown();
    }
}

#[cfg(test)]
mod tests;
