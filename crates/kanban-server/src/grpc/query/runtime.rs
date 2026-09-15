use super::{MAX_HUBS, MAX_SNAPSHOT_BYTES, source, wait_stop};
use crate::AppState;
use kanban_live_core::query::{ByteBudget, QueryHub, QueryLimits, new_query_epoch};
use kanban_protocol::rpc::v1::{self as pb, query_definition::Query};
use prost::Message;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Semaphore, watch},
    task::{AbortHandle, JoinSet},
};
use tonic::Status;

#[derive(Clone)]
pub struct QueryRuntime(pub(super) Arc<Registry>);

pub(super) struct Registry {
    pub state: AppState,
    pub stop: watch::Sender<bool>,
    pub connections: Arc<Semaphore>,
    pub budget: Arc<ByteBudget>,
    epoch: String,
    active: Mutex<Active>,
}

struct Active {
    entries: BTreeMap<Vec<u8>, Entry>,
    pumps: JoinSet<()>,
}

struct Entry {
    users: usize,
    shared: Arc<Shared>,
    abort: AbortHandle,
}

pub(super) struct Shared {
    pub hub: QueryHub,
    pub failure: Mutex<Option<Status>>,
    pub refresh: watch::Sender<u64>,
    pub completed: AtomicU64,
    #[cfg(test)]
    pub reads: AtomicU64,
}

pub(super) struct Lease {
    registry: Arc<Registry>,
    key: Vec<u8>,
    pub shared: Arc<Shared>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut active = self.registry.active.lock().unwrap();
        if let Some(entry) = active.entries.get_mut(&self.key) {
            entry.users -= 1;
            if entry.users == 0 {
                let entry = active.entries.remove(&self.key).unwrap();
                entry.shared.hub.stop();
                entry.abort.abort();
            }
        }
    }
}

impl QueryRuntime {
    pub fn new(state: AppState) -> Self {
        let runtime = Self(Arc::new(Registry {
            stop: state.stream_shutdown_sender(),
            state,
            connections: Arc::new(Semaphore::new(super::MAX_CONNECTIONS)),
            budget: ByteBudget::new(256 * 1024 * 1024),
            epoch: new_query_epoch(),
            active: Mutex::new(Active {
                entries: BTreeMap::new(),
                pumps: JoinSet::new(),
            }),
        }));
        #[cfg(test)]
        runtime
            .0
            .state
            .grpc_probe
            .attach_query_runtime(QueryProbe(Arc::downgrade(&runtime.0)));
        runtime
    }

    pub fn begin_shutdown(&self) {
        self.0.stop.send_replace(true);
        self.0.connections.close();
        let mut active = self.0.active.lock().unwrap();
        for entry in active.entries.values() {
            entry.shared.hub.stop();
        }
        active.entries.clear();
        active.pumps.abort_all();
    }

    pub async fn stop(&self) {
        self.begin_shutdown();
        let mut pumps = {
            let mut active = self.0.active.lock().unwrap();
            std::mem::take(&mut active.pumps)
        };
        while pumps.join_next().await.is_some() {}
    }

    pub(super) fn attach(&self, query: Query) -> Result<Lease, Status> {
        if *self.0.stop.borrow() {
            return Err(Status::unavailable("Host 正在退出"));
        }
        let identity = pb::QueryDefinition {
            client_query_id: String::new(),
            projection_version: 1,
            resume: None,
            refresh: false,
            query: Some(query.clone()),
        }
        .encode_to_vec();
        if identity.len() > 16 * 1024 {
            return Err(Status::resource_exhausted("单查询定义超过 16 KiB"));
        }
        let mut active = self.0.active.lock().unwrap();
        while active.pumps.try_join_next().is_some() {}
        if !active.entries.contains_key(&identity) {
            if active.entries.len() >= MAX_HUBS || active.pumps.len() >= MAX_HUBS * 2 {
                return Err(Status::resource_exhausted("Host 完整查询 Hub 额度已满"));
            }
            // 已回收 Hub 不保留无界 tombstone；新 incarnation 阻止 revision 重置碰撞旧 cursor。
            let scope = format!(
                "query:v1:{:x}:{}",
                Sha256::digest(&identity),
                new_query_epoch()
            );
            let (refresh, receiver) = watch::channel(0);
            let shared = Arc::new(Shared {
                hub: QueryHub::new(
                    self.0.epoch.clone(),
                    scope,
                    QueryLimits::default(),
                    self.0.budget.clone(),
                ),
                failure: Mutex::new(None),
                refresh,
                completed: AtomicU64::new(0),
                #[cfg(test)]
                reads: AtomicU64::new(0),
            });
            let abort = active.pumps.spawn(pump(
                self.0.state.clone(),
                query,
                shared.clone(),
                receiver,
                self.0.stop.subscribe(),
            ));
            active.entries.insert(
                identity.clone(),
                Entry {
                    users: 0,
                    shared,
                    abort,
                },
            );
        }
        let entry = active.entries.get_mut(&identity).unwrap();
        entry.users += 1;
        Ok(Lease {
            registry: self.0.clone(),
            key: identity,
            shared: entry.shared.clone(),
        })
    }

    pub(super) fn watch_deadline(
        &self,
        delivery: Weak<Mutex<Option<super::Delivery>>>,
        deadline: tokio::time::Instant,
    ) -> Result<AbortHandle, Status> {
        let mut active = self.0.active.lock().unwrap();
        while active.pumps.try_join_next().is_some() {}
        if active.pumps.len() >= MAX_HUBS * 2 + super::MAX_CONNECTIONS * 2 {
            return Err(Status::resource_exhausted("Host 查询任务额度已满"));
        }
        // watchdog 由同一 JoinSet 持有，只留弱引用；stream Drop 和 Host stop 均会取消它。
        Ok(active.pumps.spawn(async move {
            tokio::time::sleep_until(deadline).await;
            if let Some(delivery) = delivery.upgrade() {
                delivery.lock().unwrap().take();
            }
        }))
    }

    #[cfg(test)]
    pub(super) fn counts(&self) -> (usize, usize, usize) {
        let active = self.0.active.lock().unwrap();
        (
            active.entries.len(),
            active.pumps.len(),
            self.0.budget.used(),
        )
    }
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct QueryProbe(Weak<Registry>);

#[cfg(test)]
impl QueryProbe {
    pub(crate) fn resources(&self) -> Option<(usize, usize, usize)> {
        self.0.upgrade().map(|runtime| {
            let active = runtime.active.lock().unwrap();
            (
                active.entries.len(),
                runtime.connections.available_permits(),
                runtime.budget.used(),
            )
        })
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        // JoinSet 的 Drop 会取消其所有任务；没有独立任务能永久持有 Registry。
        self.active.get_mut().unwrap().pumps.abort_all();
    }
}

async fn pump(
    state: AppState,
    query: Query,
    shared: Arc<Shared>,
    mut refresh: watch::Receiver<u64>,
    mut stop: watch::Receiver<bool>,
) {
    let mut hints = state.application().subscribe_realtime_changes();
    let probes_file = matches!(query, Query::GetRunLog(_));
    let probes_clock = matches!(query, Query::GetStats(_));
    let mut last_semantic = None;
    loop {
        if *stop.borrow() {
            break;
        }
        hints.borrow_and_update();
        let ticket = *refresh.borrow_and_update();
        #[cfg(test)]
        {
            shared.reads.fetch_add(1, Ordering::Relaxed);
            state.grpc_probe.query(shared.hub.scope());
        }
        let loaded = tokio::select! {
            biased;
            _ = wait_stop(&mut stop) => break,
            value = tokio::time::timeout(Duration::from_secs(10), state.application().with_realtime_read(source::sample(state.clone(), query.clone()))) => {
                value.unwrap_or_else(|_| Err(Status::deadline_exceeded("完整查询读取超时")))
            }
        };
        let result = loaded.and_then(|sample| {
            if last_semantic == Some(sample.semantic_hash) {
                return shared
                    .hub
                    .reaffirm()
                    .map_err(|error| Status::unavailable(error.to_string()));
            }
            let bytes = sample.bytes;
            if bytes.len() > MAX_SNAPSHOT_BYTES {
                return Err(Status::resource_exhausted("完整查询快照超过 64 MiB"));
            }
            let hash = Sha256::digest(&bytes).into();
            let published = shared
                .hub
                .publish(bytes, hash)
                .map_err(|error| Status::resource_exhausted(error.to_string()))?;
            last_semantic = Some(sample.semantic_hash);
            Ok(published)
        });
        let failed = result.is_err();
        *shared.failure.lock().unwrap() = result.err();
        shared.completed.store(ticket, Ordering::Release);
        if failed {
            shared.hub.unavailable();
        } else {
            shared.hub.wake();
        }
        let retry = if failed {
            Duration::from_secs(2)
        } else if probes_file {
            Duration::from_millis(500)
        } else if probes_clock {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(86400)
        };
        tokio::select! {
            biased;
            _ = wait_stop(&mut stop) => break,
            change = hints.changed() => if change.is_err() { break; },
            change = refresh.changed() => if change.is_err() { break; },
            _ = tokio::time::sleep(retry), if failed || probes_file || probes_clock => {},
        }
        tokio::select! {
            _ = wait_stop(&mut stop) => break,
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        }
    }
    shared.hub.stop();
}
