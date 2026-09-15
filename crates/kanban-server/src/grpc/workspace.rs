//! 正式 WorkspaceService 与迁移期间共享的刷新运行时。
use futures_util::{Stream, StreamExt};
use kanban_protocol::rpc::v1 as pb;
use kanban_rpc_host::RpcApp;
use kanban_rpc_proto::v1 as framework;
use std::pin::Pin;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub(super) struct WorkspaceRpc(pub RpcApp);

#[tonic::async_trait]
impl pb::workspace_service_server::WorkspaceService for WorkspaceRpc {
    type WatchChangesStream =
        Pin<Box<dyn Stream<Item = Result<pb::WorkspaceChangeFrame, Status>> + Send>>;

    async fn watch_changes(
        &self,
        request: Request<pb::WatchChangesRequest>,
    ) -> Result<Response<Self::WatchChangesStream>, Status> {
        let request = request.map(|request| framework::WatchChangesRequest {
            board_id: request.board_id,
            protocol_version: request.protocol_version,
        });
        let response =
            framework::workspace_service_server::WorkspaceService::watch_changes(&self.0, request)
                .await?;
        let stream = response.into_inner().map(|frame| frame.map(convert));
        Ok(Response::new(Box::pin(stream)))
    }
}

fn convert(frame: framework::WorkspaceChangeFrame) -> pb::WorkspaceChangeFrame {
    use framework::workspace_change_frame::Body as Source;
    use pb::workspace_change_frame::Body as Target;
    pb::WorkspaceChangeFrame {
        board_id: frame.board_id,
        epoch: frame.epoch,
        sequence: frame.sequence,
        body: frame.body.map(|body| match body {
            Source::Invalidated(reason) => Target::Invalidated(pb::QueriesInvalidated {
                reason: reason.reason,
            }),
            Source::Heartbeat(_) => Target::Heartbeat(pb::ConnectionHeartbeat {}),
        }),
    }
}
