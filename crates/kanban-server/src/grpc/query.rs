//! 唯一 Host 的完整查询 multiplex、共享 source 与有界发送；不持有 persistence。
mod frames;
mod runtime;
mod source;
#[cfg(test)]
mod tests;

use super::context::{call_context, invalid_request};
use frames::{Transfer, from_cursor, to_cursor};
use futures_util::{Stream, stream};
use kanban_live_core::{Resume, query::QueryNext};
use kanban_protocol::rpc::query::{
    MAX_QUERIES_PER_STREAM as MAX_QUERIES, MAX_QUERY_RESULT_BYTES as MAX_SNAPSHOT_BYTES,
    PROJECTION_VERSION, PROTOCOL_VERSION,
};
use kanban_protocol::rpc::v1::{self as pb, query_frame::Body, query_service_server::QueryService};
use runtime::Lease;
#[cfg(test)]
pub(crate) use runtime::QueryProbe;
pub use runtime::QueryRuntime;
use std::{
    collections::BTreeSet,
    pin::Pin,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, watch},
    task::AbortHandle,
    time::Instant,
};
use tonic::{Request, Response, Status};

const MAX_CONNECTIONS: usize = 16;
const MAX_HUBS: usize = 256;
// WebKit 2.52 的 Fetch pull 缺少 stash drain（WebKit #322545）；后续网络帧才能唤醒滞留字节。
// 连接级心跳不读取 application、不推进 cursor；每 Host 最多 16 流，保持原有下游背压。
const IDLE_HEARTBEAT: Duration = Duration::from_millis(250);

struct Subscription {
    id: String,
    lease: Lease,
    changes: watch::Receiver<u64>,
    cursor: Option<Resume>,
    ticket: u64,
    ready: bool,
    initial: bool,
}

struct Multiplex {
    delivery: Arc<Mutex<Option<Delivery>>>,
    watchdog: Option<AbortHandle>,
    stop: watch::Receiver<bool>,
    deadline: Option<Instant>,
    terminal: bool,
}

/// deadline watcher 可以撤销所有投影资源，不依赖 HTTP/2 下游继续 poll 响应体。
struct Delivery {
    _permit: OwnedSemaphorePermit,
    subscriptions: Vec<Subscription>,
    pending: Option<(usize, Transfer)>,
    next_query: usize,
}

enum Progress {
    Frame(pb::QueryFrame),
    Wait(Vec<watch::Receiver<u64>>),
}

#[tonic::async_trait]
impl QueryService for QueryRuntime {
    type WatchQueriesStream = Pin<Box<dyn Stream<Item = Result<pb::QueryFrame, Status>> + Send>>;

    async fn watch_queries(
        &self,
        request: Request<pb::WatchQueriesRequest>,
    ) -> Result<Response<Self::WatchQueriesStream>, Status> {
        call_context(request.metadata())?;
        let deadline = parse_deadline(request.metadata())?;
        let prepare = self.prepare(request.into_inner(), deadline);
        let state = match deadline {
            Some(deadline) => tokio::time::timeout_at(deadline, prepare)
                .await
                .map_err(|_| Status::deadline_exceeded("查询订阅初始化超时"))??,
            None => prepare.await?,
        };
        Ok(Response::new(Box::pin(stream::unfold(
            state,
            |mut state| async move { state.next().await.map(|frame| (frame, state)) },
        ))))
    }
}

impl QueryRuntime {
    async fn prepare(
        &self,
        request: pb::WatchQueriesRequest,
        deadline: Option<Instant>,
    ) -> Result<Multiplex, Status> {
        if request.protocol_version != PROTOCOL_VERSION
            || request.queries.is_empty()
            || request.queries.len() > MAX_QUERIES
        {
            return Err(invalid_request(
                "protocol_version 必须为 1，查询数为 1..=64",
            ));
        }
        let permit = self
            .0
            .connections
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("最多 16 个完整查询连接"))?;
        let mut ids = BTreeSet::new();
        for definition in &request.queries {
            if definition.client_query_id.is_empty()
                || definition.client_query_id.len() > 128
                || definition.client_query_id.chars().any(char::is_control)
                || !ids.insert(definition.client_query_id.clone())
                || definition.projection_version != PROJECTION_VERSION
                || definition.query.is_none()
            {
                return Err(invalid_request(
                    "查询 ID 必须非空、唯一且至多 128 字节；需要 query 与 projection_version=1",
                ));
            }
            if definition
                .resume
                .as_ref()
                .is_some_and(|cursor| cursor.epoch.len() > 128 || cursor.scope.len() > 256)
            {
                return Err(invalid_request("查询 cursor 超过长度预算"));
            }
        }
        let mut normalized = Vec::with_capacity(request.queries.len());
        for mut definition in request.queries {
            let query = self
                .0
                .state
                .application()
                .with_realtime_read(source::normalize(
                    &self.0.state,
                    definition.query.take().unwrap(),
                ))
                .await?;
            normalized.push((definition, query));
        }
        let mut subscriptions = Vec::with_capacity(normalized.len());
        for (definition, query) in normalized {
            let lease = self.attach(query)?;
            let changes = lease.shared.hub.subscribe();
            let ticket = if definition.refresh {
                let mut ticket = None;
                lease.shared.refresh.send_modify(|current| {
                    if let Some(next) = current.checked_add(1) {
                        *current = next;
                        ticket = Some(next);
                    }
                });
                ticket.ok_or_else(|| Status::resource_exhausted("查询刷新序号已用尽"))?
            } else {
                0
            };
            subscriptions.push(Subscription {
                id: definition.client_query_id,
                lease,
                changes,
                ticket,
                ready: false,
                initial: true,
                cursor: definition.resume.map(from_cursor),
            });
        }
        let delivery = Arc::new(Mutex::new(Some(Delivery {
            _permit: permit,
            subscriptions,
            pending: None,
            next_query: 0,
        })));
        let watchdog = deadline
            .map(|deadline| self.watch_deadline(Arc::downgrade(&delivery), deadline))
            .transpose()?;
        Ok(Multiplex {
            delivery,
            watchdog,
            stop: self.0.stop.subscribe(),
            deadline,
            terminal: false,
        })
    }
}

impl Multiplex {
    async fn next(&mut self) -> Option<Result<pb::QueryFrame, Status>> {
        loop {
            if self.terminal || *self.stop.borrow() {
                return None;
            }
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                self.terminal = true;
                self.delivery.lock().unwrap().take();
                return Some(Err(Status::deadline_exceeded("查询订阅 deadline 已到期")));
            }
            let progress = self.delivery.lock().unwrap().as_mut().map(Delivery::next);
            let receivers = match progress {
                Some(Progress::Frame(frame)) => return Some(Ok(frame)),
                Some(Progress::Wait(receivers)) => receivers,
                None => {
                    self.terminal = true;
                    return Some(Err(Status::deadline_exceeded("查询订阅 deadline 已到期")));
                }
            };
            // 等待使用临时 receiver，保留原 receiver 的未读位供下一轮扫描。
            let changed = receivers
                .into_iter()
                .map(|mut receiver| Box::pin(async move { receiver.changed().await }));
            let deadline = self.deadline;
            tokio::select! {
                biased;
                _ = wait_stop(&mut self.stop) => return None,
                _ = wait_deadline(deadline) => {},
                _ = futures_util::future::select_all(changed) => {},
                _ = tokio::time::sleep(IDLE_HEARTBEAT) => {
                    return Some(Ok(pb::QueryFrame { client_query_id: String::new(), body: Some(Body::Heartbeat(pb::QueryHeartbeat {})) }));
                }
            }
        }
    }
}

impl Drop for Multiplex {
    fn drop(&mut self) {
        if let Some(watchdog) = &self.watchdog {
            watchdog.abort();
        }
        self.delivery.lock().unwrap().take();
    }
}

impl Delivery {
    fn next(&mut self) -> Progress {
        loop {
            if let Some((index, transfer)) = &mut self.pending {
                let index = *index;
                let body = transfer.next();
                if let Body::End(end) = &body {
                    self.subscriptions[index].cursor = end.cursor.clone().map(from_cursor);
                    self.subscriptions[index].initial = true;
                    self.pending = None;
                }
                return Progress::Frame(pb::QueryFrame {
                    client_query_id: self.subscriptions[index].id.clone(),
                    body: Some(body),
                });
            }
            for _ in 0..self.subscriptions.len() {
                let index = self.next_query;
                self.next_query = (index + 1) % self.subscriptions.len();
                let sub = &mut self.subscriptions[index];
                let changed = sub.initial || sub.changes.has_changed().unwrap_or(true);
                if !changed && sub.ready {
                    continue;
                }
                if sub.lease.shared.completed.load(Ordering::Acquire) < sub.ticket {
                    sub.changes.borrow_and_update();
                    continue;
                }
                sub.initial = false;
                sub.changes.borrow_and_update();
                match sub.lease.shared.hub.next(sub.cursor.as_ref()) {
                    Ok(QueryNext::Idle) => {
                        if !sub.ready {
                            sub.ready = true;
                            return Progress::Frame(pb::QueryFrame {
                                client_query_id: sub.id.clone(),
                                body: Some(Body::Ready(pb::QueryReady {
                                    cursor: sub.cursor.clone().map(to_cursor),
                                })),
                            });
                        }
                    }
                    Ok(next) => {
                        self.pending = Transfer::new(next).map(|transfer| (index, transfer));
                        break;
                    }
                    Err(_) => {
                        if changed {
                            let failure = sub.lease.shared.failure.lock().unwrap().clone();
                            if let Some(failure) = failure {
                                sub.ready = false;
                                return Progress::Frame(pb::QueryFrame {
                                    client_query_id: sub.id.clone(),
                                    body: Some(frames::failure(failure)),
                                });
                            }
                        }
                    }
                }
            }
            if self.pending.is_some() {
                continue;
            }
            return Progress::Wait(
                self.subscriptions
                    .iter()
                    .map(|sub| sub.changes.clone())
                    .collect(),
            );
        }
    }
}

async fn wait_stop(stop: &mut watch::Receiver<bool>) {
    loop {
        if *stop.borrow() {
            return;
        }
        if stop.changed().await.is_err() {
            return;
        }
    }
}

async fn wait_deadline(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

fn parse_deadline(metadata: &tonic::metadata::MetadataMap) -> Result<Option<Instant>, Status> {
    let mut values = metadata.get_all("grpc-timeout").iter();
    let Some(raw) = values.next() else {
        return Ok(None);
    };
    let invalid = || invalid_request("grpc-timeout 必须是至多八位数字和 H/M/S/m/u/n 单位");
    if values.next().is_some() {
        return Err(invalid());
    }
    let raw = raw.to_str().map_err(|_| invalid())?;
    if !(2..=9).contains(&raw.len()) {
        return Err(invalid());
    }
    let (number, unit) = raw.split_at(raw.len() - 1);
    if !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    let number = number.parse::<u64>().map_err(|_| invalid())?;
    let duration = match unit {
        "H" => Duration::from_secs(number * 3600),
        "M" => Duration::from_secs(number * 60),
        "S" => Duration::from_secs(number),
        "m" => Duration::from_millis(number),
        "u" => Duration::from_micros(number),
        "n" => Duration::from_nanos(number),
        _ => return Err(invalid()),
    };
    Instant::now()
        .checked_add(duration)
        .map(Some)
        .ok_or_else(invalid)
}
