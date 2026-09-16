//! Atlas 分支现有 host 的可选装配点。此模块不打开数据库、不绑定第二个端口。
use std::sync::Arc;
use axum::{Router, http::HeaderValue};
use kanban_grpc_adapter::KanbanAdapter;
use kanban_rpc_host::RpcApp;
use kanban_service::{KanbanError, Result};
use crate::AppState;

pub struct WorkspaceRpcMount {
    pub router: Router,
    /// 跟随唯一 host 的关闭流程调用 stop；不能交给 detached task 永久持有。
    pub runtime: RpcApp,
}

/// 暂只装配 WorkspaceService，原有业务 DTO 仍由后续契约任务迁移。
/// allowed_origins 必须由实际 listener、已验证的 runtime 与开发配置确定，不接受 wildcard。
pub fn workspace_rpc_mount(state: &AppState, allowed_origins: Vec<HeaderValue>) -> Result<WorkspaceRpcMount> {
    let adapter = Arc::new(KanbanAdapter::new(state.application().clone()));
    let runtime = RpcApp::new(Vec::new(), adapter.clone(), allowed_origins)
        .map_err(|error| KanbanError::InvalidInput(error.to_string()))?
        .with_refresh_source(adapter);
    let router = Router::new().route_service(
        "/kanban.framework.v1.WorkspaceService/WatchChanges",
        kanban_rpc_host::workspace_service(runtime.clone()),
    );
    Ok(WorkspaceRpcMount { router, runtime })
}
