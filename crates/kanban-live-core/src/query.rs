//! 完整 typed 投影的不可变编码、精确 splice 和有界历史；不解释业务字段。
use crate::{LiveError, Result, Resume};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::watch;

pub fn new_query_epoch() -> String {
    ulid::Ulid::new().to_string()
}

/// 所有 Hub 和正在发送的帧共享同一预算；历史淘汰不会漏算消费者仍持有的字节。
#[derive(Debug)]
pub struct ByteBudget {
    maximum: usize,
    used: AtomicUsize,
}

impl ByteBudget {
    pub fn new(maximum: usize) -> Arc<Self> {
        Arc::new(Self {
            maximum,
            used: AtomicUsize::new(0),
        })
    }

    pub fn used(&self) -> usize {
        self.used.load(Ordering::Relaxed)
    }

    fn retain(self: &Arc<Self>, bytes: Vec<u8>) -> Result<Arc<QueryBytes>> {
        let size = bytes.len();
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |used| {
                used.checked_add(size).filter(|next| *next <= self.maximum)
            })
            .map_err(|_| LiveError::Budget("Host 完整查询字节".into()))?;
        Ok(Arc::new(QueryBytes {
            bytes: bytes.into_boxed_slice(),
            budget: self.clone(),
        }))
    }
}

#[derive(Debug)]
pub struct QueryBytes {
    bytes: Box<[u8]>,
    budget: Arc<ByteBudget>,
}

impl AsRef<[u8]> for QueryBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for QueryBytes {
    fn drop(&mut self) {
        self.budget
            .used
            .fetch_sub(self.bytes.len(), Ordering::AcqRel);
    }
}

#[derive(Clone, Debug)]
pub struct QueryLimits {
    pub snapshot_bytes: usize,
    pub history_batches: usize,
    pub history_bytes: usize,
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self {
            snapshot_bytes: 64 * 1024 * 1024,
            history_batches: 64,
            history_bytes: 4 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct QuerySnapshot {
    pub cursor: Resume,
    pub bytes: Arc<QueryBytes>,
    pub sha256: [u8; 32],
}

#[derive(Debug)]
pub struct QueryDelta {
    pub cursor: Resume,
    pub base: Resume,
    pub offset: usize,
    pub delete_length: usize,
    pub patch: Arc<QueryBytes>,
    pub result_size: usize,
    pub result_sha256: [u8; 32],
}

impl QueryDelta {
    fn weight(&self) -> usize {
        self.patch.as_ref().as_ref().len() + 128
    }
}

#[derive(Debug)]
pub enum QueryNext {
    Snapshot(QuerySnapshot),
    Delta(Arc<QueryDelta>),
    Idle,
}

pub struct QueryHub {
    epoch: String,
    scope: String,
    limits: QueryLimits,
    budget: Arc<ByteBudget>,
    changed: watch::Sender<u64>,
    inner: Mutex<QueryState>,
}

struct QueryState {
    snapshot: Option<QuerySnapshot>,
    history: VecDeque<Arc<QueryDelta>>,
    history_bytes: usize,
    ready: bool,
    stopped: bool,
}

impl QueryHub {
    pub fn new(epoch: String, scope: String, limits: QueryLimits, budget: Arc<ByteBudget>) -> Self {
        let (changed, _) = watch::channel(0);
        Self {
            epoch,
            scope,
            limits,
            budget,
            changed,
            inner: Mutex::new(QueryState {
                snapshot: None,
                history: VecDeque::new(),
                history_bytes: 0,
                ready: false,
                stopped: false,
            }),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    fn notify(&self) {
        self.changed
            .send_modify(|sequence| *sequence = sequence.wrapping_add(1));
    }

    /// 读取确认等控制状态可以唤醒消费者；此通知不是投影 revision。
    pub fn wake(&self) {
        self.notify();
    }

    /// 只允许唯一 source 在一致读取并确定性编码后发布；相同编码不增加 revision。
    pub fn publish(&self, bytes: Vec<u8>, sha256: [u8; 32]) -> Result<bool> {
        if bytes.len() > self.limits.snapshot_bytes {
            return Err(LiveError::Budget(format!(
                "完整查询快照超过 {} 字节",
                self.limits.snapshot_bytes
            )));
        }
        let mut state = self.inner.lock().unwrap();
        if state.stopped {
            return Err(LiveError::Stopped);
        }
        if state
            .snapshot
            .as_ref()
            .is_some_and(|old| old.bytes.as_ref().as_ref() == bytes)
        {
            if !state.ready {
                state.ready = true;
                self.notify();
            }
            return Ok(false);
        }
        let revision = state
            .snapshot
            .as_ref()
            .map_or(Some(1), |old| old.cursor.revision.checked_add(1))
            .ok_or_else(|| LiveError::Budget("查询 revision 溢出".into()))?;
        let cursor = Resume {
            epoch: self.epoch.clone(),
            scope: self.scope.clone(),
            revision,
        };
        let retained = self.budget.retain(bytes)?;
        let delta = state.snapshot.as_ref().and_then(|old| {
            let (offset, delete_length, patch) =
                splice(old.bytes.as_ref().as_ref(), retained.as_ref().as_ref());
            if patch.len().saturating_add(128) > self.limits.history_bytes {
                return None;
            }
            let patch = self.budget.retain(patch.to_vec()).ok()?;
            Some(Arc::new(QueryDelta {
                cursor: cursor.clone(),
                base: old.cursor.clone(),
                offset,
                delete_length,
                patch,
                result_size: retained.as_ref().as_ref().len(),
                result_sha256: sha256,
            }))
        });
        if let Some(delta) = delta {
            state.history_bytes += delta.weight();
            state.history.push_back(delta);
            while state.history.len() > self.limits.history_batches
                || state.history_bytes > self.limits.history_bytes
            {
                if let Some(old) = state.history.pop_front() {
                    state.history_bytes -= old.weight();
                }
            }
        } else {
            state.history.clear();
            state.history_bytes = 0;
        }
        state.snapshot = Some(QuerySnapshot {
            cursor,
            bytes: retained,
            sha256,
        });
        state.ready = true;
        self.notify();
        Ok(true)
    }

    /// epoch/scope/base 必须精确匹配；未知、过期和 future cursor 一律重发完整快照。
    pub fn next(&self, resume: Option<&Resume>) -> Result<QueryNext> {
        let state = self.inner.lock().unwrap();
        if state.stopped {
            return Err(LiveError::Stopped);
        }
        if !state.ready {
            return Err(LiveError::Unavailable);
        }
        let snapshot = state.snapshot.as_ref().ok_or(LiveError::Unavailable)?;
        if let Some(resume) = resume {
            if resume == &snapshot.cursor {
                return Ok(QueryNext::Idle);
            }
            if let Some(delta) = state.history.iter().find(|delta| &delta.base == resume) {
                return Ok(QueryNext::Delta(delta.clone()));
            }
        }
        Ok(QueryNext::Snapshot(snapshot.clone()))
    }

    pub fn unavailable(&self) {
        self.inner.lock().unwrap().ready = false;
        self.notify();
    }

    /// source 已重新核对业务内容；保留最近一次真实样本及其 cursor，不重新编码旧结果。
    pub fn reaffirm(&self) -> Result<bool> {
        let mut state = self.inner.lock().unwrap();
        if state.stopped {
            return Err(LiveError::Stopped);
        }
        if state.snapshot.is_none() {
            return Err(LiveError::Unavailable);
        }
        if !state.ready {
            state.ready = true;
            self.notify();
        }
        Ok(false)
    }

    pub fn stop(&self) {
        let mut state = self.inner.lock().unwrap();
        state.stopped = true;
        state.snapshot = None;
        state.history.clear();
        state.history_bytes = 0;
        self.notify();
    }
}

fn splice<'a>(old: &[u8], next: &'a [u8]) -> (usize, usize, &'a [u8]) {
    let prefix = old
        .iter()
        .zip(next)
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(next[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    (
        prefix,
        old.len() - prefix - suffix,
        &next[prefix..next.len() - suffix],
    )
}

#[cfg(test)]
mod tests;
