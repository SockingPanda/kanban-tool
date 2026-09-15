use std::{collections::BTreeMap, pin::Pin, sync::Arc, time::Duration};
use http::HeaderValue;
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};
use tokio_stream::Stream;
use prost::Message;
use kanban_live_core::{Hub, LiveError, ReadNext, Resume, TaskCommands, UpdateTitle};
use kanban_rpc_proto::{PROTOCOL_VERSION, v1 as pb};
use crate::frames::{envelope, error, to_card, to_cursor};

#[derive(Clone)]
pub struct RpcApp {
    hubs: Arc<BTreeMap<String, Arc<Hub>>>,
    commands: Arc<dyn TaskCommands>,
    origins: Arc<Vec<HeaderValue>>,
    pub(crate) slots: Arc<Semaphore>,
    pub(crate) refresh_source: Option<Arc<dyn kanban_live_core::RefreshSource>>,
    pub(crate) shutdown: tokio::sync::watch::Sender<bool>,
}
impl RpcApp {
    pub fn new(hubs: Vec<Arc<Hub>>, commands: Arc<dyn TaskCommands>, origins: Vec<HeaderValue>) -> Result<Self, LiveError> {
        let mut by_id = BTreeMap::new();
        for hub in hubs {
            if by_id.insert(hub.board_id().to_owned(), hub).is_some() { return Err(LiveError::Invalid("重复 Hub".into())); }
        }
        if origins.iter().any(|origin| origin == "*" || origin == "null") {
            return Err(LiveError::Invalid("不得允许 wildcard/null Origin".into()));
        }
        Ok(Self { hubs: Arc::new(by_id), commands, origins: Arc::new(origins), slots: Arc::new(Semaphore::new(16)), refresh_source: None, shutdown: tokio::sync::watch::channel(false).0 })
    }
    /// 由唯一 host 注入已有 application；不再打开数据库。
    pub fn with_refresh_source(mut self, source: Arc<dyn kanban_live_core::RefreshSource>) -> Self {
        self.refresh_source = Some(source); self
    }
    pub fn origins(&self) -> &[HeaderValue] { &self.origins }
    pub async fn stop(&self) { self.shutdown.send_replace(true); for hub in self.hubs.values() { hub.stop().await; } }
    fn hub(&self, board: &str) -> Result<Arc<Hub>, Status> {
        self.hubs.get(board).cloned().ok_or_else(|| error(LiveError::NotFound(board.into())))
    }
    pub(crate) fn origin<T>(&self, request: &Request<T>) -> Result<(), Status> {
        // CORS 只限制浏览器读取响应，不能替代执行前的 Origin 拒绝。
        let all_origins = request.metadata().get_all("origin");
        let mut origins = all_origins.iter();
        if let Some(origin) = origins.next() {
            if origins.next().is_some() || !self.origins.iter().any(|allowed| allowed.as_bytes() == origin.as_bytes()) {
                return Err(Status::permission_denied("Origin 不在本地宿主允许清单"));
            }
        }
        Ok(())
    }
}
#[tonic::async_trait]
impl pb::board_service_server::BoardService for RpcApp {
    async fn get_board(&self, request: Request<pb::GetBoardRequest>) -> Result<Response<pb::GetBoardResponse>, Status> {
        self.origin(&request)?;
        let board = request.into_inner().board_id;
        let snapshot = self.hub(&board)?.snapshot().await.map_err(error)?;
        let response = pb::GetBoardResponse { board_id: board, cursor: Some(to_cursor(&snapshot.cursor)),
            tasks: snapshot.cards.values().map(to_card).collect() };
        if response.encoded_len() > 4 * 1024 * 1024 {
            return Err(error(LiveError::Budget("GetBoard 超过 4 MiB，请使用 WatchBoard 分块快照".into())));
        }
        Ok(Response::new(response))
    }
    type WatchBoardStream = Pin<Box<dyn Stream<Item = Result<pb::BoardFrame, Status>> + Send + 'static>>;
    async fn watch_board(&self, request: Request<pb::WatchBoardRequest>) -> Result<Response<Self::WatchBoardStream>, Status> {
        self.origin(&request)?;
        let input = request.into_inner();
        if input.protocol_version != PROTOCOL_VERSION { return Err(Status::failed_precondition("不支持的订阅协议版本")); }
        if input.resume.as_ref().is_some_and(|r| r.epoch.len() > 128 || r.scope.len() > 256) {
            return Err(Status::invalid_argument("cursor 字段过长"));
        }
        let hub = self.hub(&input.board_id)?;
        let permit = self.slots.clone().try_acquire_owned().map_err(|_| Status::resource_exhausted("最多 16 个活跃订阅"))?;
        let mut changes = hub.subscribe(); // 在任何快照读取之前订阅。
        hub.snapshot().await.map_err(error)?; // header 提交前完成可用性检查。
        let mut cursor = input.resume.map(|c| Resume { epoch: c.epoch, scope: c.scope, revision: c.revision });
        let output = async_stream::try_stream! {
            let _permit = permit;
            loop {
                changes.borrow_and_update();
                match hub.next(cursor.as_ref()).await.map_err(error)? {
                    ReadNext::Reset { reason, snapshot } => {
                        let c = &snapshot.cursor;
                        if reason != "initial" {
                            yield envelope(hub.board_id(), c, pb::board_frame::Body::Reset(pb::ResetRequired { reason: reason.into() }))?;
                        }
                        let count = u32::try_from(snapshot.cards.len()).map_err(|_| Status::resource_exhausted("快照过大"))?;
                        yield envelope(hub.board_id(), c, pb::board_frame::Body::SnapshotBegin(pb::SnapshotBegin { revision: c.revision, count }))?;
                        let refs: Vec<_> = snapshot.cards.values().collect();
                        let mut chunks = 0u32;
                        for chunk in refs.chunks(64) {
                            yield envelope(hub.board_id(), c, pb::board_frame::Body::SnapshotChunk(pb::SnapshotChunk {
                                index: chunks, tasks: chunk.iter().map(|card| to_card(card)).collect(),
                            }))?;
                            chunks += 1;
                        }
                        yield envelope(hub.board_id(), c, pb::board_frame::Body::SnapshotCommit(pb::SnapshotCommit {
                            revision: c.revision, count, chunks,
                        }))?;
                        cursor = Some(snapshot.cursor.clone());
                    }
                    ReadNext::Delta(delta) => {
                        let c = cursor.as_mut().ok_or_else(|| Status::internal("delta 缺少 cursor"))?;
                        yield envelope(hub.board_id(), c, pb::board_frame::Body::Delta(pb::BoardDelta {
                            base_revision: delta.base_revision, revision: delta.revision,
                            upserts: delta.upserts.iter().map(to_card).collect(), removed_ids: delta.removed.clone(),
                        }))?;
                        c.revision = delta.revision;
                    }
                    ReadNext::Idle => {
                        tokio::select! {
                            result = changes.changed() => if result.is_err() { break; },
                            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                                let snapshot = hub.snapshot().await.map_err(error)?;
                                yield envelope(hub.board_id(), &snapshot.cursor, pb::board_frame::Body::Heartbeat(pb::Heartbeat {
                                    server_revision: snapshot.cursor.revision,
                                }))?;
                                // 心跳不推进客户端或本地流的应用 cursor。
                            }
                        }
                    }
                }
            }
        };
        Ok(Response::new(Box::pin(output)))
    }
}
#[tonic::async_trait]
impl pb::task_service_server::TaskService for RpcApp {
    async fn update_task_title(&self, request: Request<pb::UpdateTaskTitleRequest>) -> Result<Response<pb::UpdateTaskTitleResponse>, Status> {
        self.origin(&request)?;
        let input = request.into_inner();
        let expected = input.expected.ok_or_else(|| Status::invalid_argument("必须提供 expected version"))?;
        self.hub(&input.board_id)?; // 只允许已配置的 board。
        if input.actor.trim().is_empty() || input.actor.len() > 128 || input.actor.chars().any(char::is_control)
            || input.title.trim().is_empty() || input.title.len() > 4096 {
            return Err(Status::invalid_argument("actor/title 无效"));
        }
        let result = tokio::time::timeout(Duration::from_secs(10), self.commands.update_title(UpdateTitle {
            board_id: input.board_id, task_id: input.task_id, title: input.title, actor: input.actor,
            expected_version: expected.value,
        })).await.map_err(|_| Status::deadline_exceeded("命令超时，结果可能已提交；请查询确认"))?.map_err(error)?;
        Ok(Response::new(pb::UpdateTaskTitleResponse { task: Some(to_card(&result)) }))
    }
}
