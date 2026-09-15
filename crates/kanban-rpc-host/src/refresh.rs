//! Atlas 分页前端迁移期的 typed query invalidation；无 SSE、无 REST 转发。
use crate::{RpcApp, frames::error, wait_stop};
use kanban_rpc_proto::{PROTOCOL_VERSION, v1 as pb};
use std::{pin::Pin, time::Duration};
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

enum RefreshWake {
    Stop,
    Changed(Result<(), tokio::sync::watch::error::RecvError>),
    Heartbeat,
}

#[tonic::async_trait]
impl pb::workspace_service_server::WorkspaceService for RpcApp {
    type WatchChangesStream =
        Pin<Box<dyn Stream<Item = Result<pb::WorkspaceChangeFrame, Status>> + Send + 'static>>;

    async fn watch_changes(
        &self,
        request: Request<pb::WatchChangesRequest>,
    ) -> Result<Response<Self::WatchChangesStream>, Status> {
        self.origin(&request)?;
        let input = request.into_inner();
        if input.protocol_version != PROTOCOL_VERSION {
            return Err(Status::failed_precondition("不支持的订阅协议版本"));
        }
        kanban_live_core::validate_board(&input.board_id).map_err(error)?;
        let source = self
            .refresh_source
            .clone()
            .ok_or_else(|| Status::unimplemented("宿主未装配 RefreshSource"))?;
        let _permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("最多 16 个活跃订阅"))?;
        let mut shutdown = self.shutdown.subscribe();
        if *shutdown.borrow() {
            return Err(Status::unavailable("宿主正在关闭"));
        }
        // 必须在 header 前验证作用域，先订阅再读 fence，覆盖初始化时的并发提交。
        let mut changes = source.subscribe_refreshes();
        let mut initial_shutdown = shutdown.clone();
        tokio::select! {
            _ = wait_stop(&mut initial_shutdown) => return Err(Status::unavailable("宿主正在关闭")),
            result = tokio::time::timeout(Duration::from_secs(10), source.check_board(&input.board_id)) => {
                result.map_err(|_| Status::deadline_exceeded("看板验证超时"))?.map_err(error)?;
            }
        }
        let board = input.board_id;
        let epoch = ulid::Ulid::new().to_string();
        let output = async_stream::try_stream! {
            let _permit = _permit;
            let mut sequence = 1u64;
            yield invalidated(&board, &epoch, sequence, pb::RefreshReason::Attached);
            loop {
                let wake = tokio::select! {
                    biased;
                    _ = wait_stop(&mut shutdown) => RefreshWake::Stop,
                    changed = changes.changed() => RefreshWake::Changed(changed),
                    _ = tokio::time::sleep(Duration::from_secs(15)) => RefreshWake::Heartbeat,
                };
                match wake {
                    RefreshWake::Stop => break,
                    RefreshWake::Changed(changed) => {
                        changed.map_err(|_| Status::unavailable("刷新源已关闭"))?;
                        let stopped = tokio::select! {
                            _ = wait_stop(&mut shutdown) => true,
                            _ = tokio::time::sleep(Duration::from_millis(10)) => false,
                        };
                        if stopped { break; }
                        // 读 fence 期间的新提示必须保留给下一轮。
                        changes.borrow_and_update();
                        let checked = tokio::select! {
                            _ = wait_stop(&mut shutdown) => None,
                            result = tokio::time::timeout(Duration::from_secs(10), source.check_board(&board)) => Some(result),
                        };
                        let Some(checked) = checked else { break; };
                        checked.map_err(|_| Status::deadline_exceeded("看板验证超时"))?.map_err(error)?;
                        sequence = sequence.checked_add(1).ok_or_else(|| Status::out_of_range("刷新序号已耗尽，请重新连接"))?;
                        yield invalidated(&board, &epoch, sequence, pb::RefreshReason::WriteHint);
                    }
                    RefreshWake::Heartbeat => {
                        yield pb::WorkspaceChangeFrame { board_id: board.clone(), epoch: epoch.clone(), sequence,
                            body: Some(pb::workspace_change_frame::Body::Heartbeat(pb::ConnectionHeartbeat {})) };
                    }
                }
            }
        };
        Ok(Response::new(Box::pin(output)))
    }
}
fn invalidated(
    board: &str,
    epoch: &str,
    sequence: u64,
    reason: pb::RefreshReason,
) -> pb::WorkspaceChangeFrame {
    pb::WorkspaceChangeFrame {
        board_id: board.into(),
        epoch: epoch.into(),
        sequence,
        body: Some(pb::workspace_change_frame::Body::Invalidated(
            pb::QueriesInvalidated {
                reason: reason as i32,
            },
        )),
    }
}
