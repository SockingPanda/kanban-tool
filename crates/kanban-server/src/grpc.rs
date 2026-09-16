//! 唯一 Host 的 RPC 装配与生命周期，不打开数据库、不绑定第二个端口。
use crate::AppState;
use axum::Router;
use tower::Layer;
mod business;
mod context;
mod deadline;
mod extensions;
#[cfg(test)]
mod probe;
mod query;
#[cfg(test)]
pub(crate) use probe::SourceProbe;

pub(crate) const RPC_ROUTES: &[&str] = &[
    "/kanban.v1.KanbanService/*method",
    "/kanban.v1.QueryService/WatchQueries",
    "/kanban.extensions.v1.ObjectService/*method",
    "/kanban.extensions.v1.FileService/*method",
];

pub struct RpcMount {
    pub router: Router,
    /// 跟随唯一 host 的关闭流程调用 stop；不能交给 detached task 永久持有。
    pub runtime: HostRpcRuntime,
}

#[derive(Clone)]
pub struct HostRpcRuntime {
    query: query::QueryRuntime,
    extensions: extensions::ExtensionRuntime,
}

impl HostRpcRuntime {
    pub fn begin_shutdown(&self) {
        self.query.begin_shutdown();
        self.extensions.begin_shutdown();
    }

    pub async fn stop(&self) {
        self.begin_shutdown();
        self.query.stop().await;
        self.extensions.stop().await;
    }
}

/// 在唯一 Host 装配正式具名业务 RPC 与完整查询流；来源策略由外层 Host 统一检查。
pub fn rpc_mount(state: &AppState) -> RpcMount {
    let business = kanban_protocol::rpc::v1::kanban_service_server::KanbanServiceServer::new(
        business::BusinessRpc {
            state: state.clone(),
        },
    )
    .max_decoding_message_size(kanban_protocol::rpc::MAX_MESSAGE_BYTES)
    .max_encoding_message_size(kanban_protocol::rpc::MAX_MESSAGE_BYTES);
    let queries = query::QueryRuntime::new(state.clone());
    let query_service =
        kanban_protocol::rpc::v1::query_service_server::QueryServiceServer::new(queries.clone())
            .max_decoding_message_size(256 * 1024)
            .max_encoding_message_size(kanban_protocol::rpc::query::MAX_QUERY_FRAME_BYTES);
    let (extension_router, extensions) = extensions::mount(state);
    let router = Router::new()
        .merge(extension_router)
        .route_service(
            RPC_ROUTES[0],
            tonic_web::GrpcWebLayer::new().layer(deadline::Deadline::new(business)),
        )
        .route_service(
            RPC_ROUTES[1],
            tonic_web::GrpcWebLayer::new().layer(query_service),
        );
    RpcMount {
        router,
        runtime: HostRpcRuntime {
            query: queries,
            extensions,
        },
    }
}

/// 即使 HTTP future 提前返回或被取消，也同步关闭所有订阅；正常退出再等待 Hub 收尾。
pub(crate) struct RpcLifetime(pub HostRpcRuntime);

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
