use crate::{Card, Delta, Limits, LiveError, Result, Resume, Snapshot, validate_board};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use tokio::sync::{Mutex, watch};

/// 每个 Hub 只表示一个完整看板卡片查询。过滤或分页扩展必须建立不同 scope。
pub struct Hub {
    board_id: String,
    scope: String,
    epoch: String,
    limits: Limits,
    inner: Mutex<State>,
    changed: watch::Sender<u64>,
}
struct State {
    revision: u64,
    ready: bool,
    stopped: bool,
    cards: Arc<BTreeMap<String, Card>>,
    history: VecDeque<Arc<Delta>>,
    history_bytes: usize,
}
#[derive(Debug)]
pub enum ReadNext {
    Reset {
        reason: &'static str,
        snapshot: Snapshot,
    },
    Delta(Arc<Delta>),
    Idle,
}
impl Hub {
    pub fn new(board_id: impl Into<String>, limits: Limits) -> Result<Self> {
        let board_id = board_id.into();
        validate_board(&board_id)?;
        if limits.max_cards == 0
            || limits.snapshot_bytes == 0
            || limits.history_batches == 0
            || limits.delta_bytes == 0
            || limits.history_bytes < limits.delta_bytes
        {
            return Err(LiveError::Invalid("订阅预算无效".into()));
        }
        let (changed, _) = watch::channel(0);
        Ok(Self {
            scope: format!("board:{board_id}:cards:v1"),
            board_id,
            epoch: ulid::Ulid::new().to_string(),
            limits,
            changed,
            inner: Mutex::new(State {
                revision: 0,
                ready: false,
                stopped: false,
                cards: Arc::new(BTreeMap::new()),
                history: VecDeque::new(),
                history_bytes: 0,
            }),
        })
    }
    pub fn board_id(&self) -> &str {
        &self.board_id
    }
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }
    fn cursor(&self, revision: u64) -> Resume {
        Resume {
            epoch: self.epoch.clone(),
            scope: self.scope.clone(),
            revision,
        }
    }
    fn snapshot_of(&self, state: &State) -> Snapshot {
        Snapshot {
            cursor: self.cursor(state.revision),
            cards: state.cards.clone(),
        }
    }
    fn check(state: &State) -> Result<()> {
        if state.stopped {
            Err(LiveError::Stopped)
        } else if !state.ready {
            Err(LiveError::Unavailable)
        } else {
            Ok(())
        }
    }
    fn notify(&self) {
        self.changed.send_modify(|v| *v = v.wrapping_add(1));
    }
    pub async fn snapshot(&self) -> Result<Snapshot> {
        let state = self.inner.lock().await;
        Self::check(&state)?;
        Ok(self.snapshot_of(&state))
    }
    /// 输入须来自 application 的一致读取。先校验、计算，最后一次性交换投影与版本。
    /// 唯一 source pump 调用本方法；命令处理器不能旁路写 Hub。
    pub async fn publish(&self, cards: Vec<Card>) -> Result<bool> {
        if cards.len() > self.limits.max_cards {
            return Err(LiveError::Budget("卡片数量".into()));
        }
        let mut next = BTreeMap::new();
        let mut bytes = 0usize;
        for card in cards {
            card.validate()?;
            bytes = bytes
                .checked_add(card.weight())
                .ok_or_else(|| LiveError::Budget("大小溢出".into()))?;
            if bytes > self.limits.snapshot_bytes {
                return Err(LiveError::Budget("快照字节".into()));
            }
            if next.insert(card.id.clone(), card).is_some() {
                return Err(LiveError::Invalid("重复 task ID".into()));
            }
        }
        let mut state = self.inner.lock().await;
        if state.stopped {
            return Err(LiveError::Stopped);
        }
        let was_ready = state.ready;
        let initial = state.revision == 0;
        if !initial && *state.cards == next {
            state.ready = true;
            if !was_ready {
                self.notify();
            }
            return Ok(false);
        }
        let revision = state
            .revision
            .checked_add(1)
            .ok_or_else(|| LiveError::Budget("revision 溢出，需重建 Hub".into()))?;
        let delta = Delta {
            base_revision: state.revision,
            revision,
            upserts: next
                .iter()
                .filter(|(id, card)| state.cards.get(*id) != Some(*card))
                .map(|(_, card)| card.clone())
                .collect(),
            removed: state
                .cards
                .keys()
                .filter(|id| !next.contains_key(*id))
                .cloned()
                .collect(),
        };
        let weight = delta.weight();
        // 过大的单次变更不拆成可见中间状态；让订阅者走分块快照。
        if initial || weight > self.limits.delta_bytes {
            state.history.clear();
            state.history_bytes = 0;
        } else {
            state.history.push_back(Arc::new(delta));
            state.history_bytes += weight;
            while state.history.len() > self.limits.history_batches
                || state.history_bytes > self.limits.history_bytes
            {
                if let Some(old) = state.history.pop_front() {
                    state.history_bytes -= old.weight();
                }
            }
        }
        state.cards = Arc::new(next);
        state.revision = revision;
        state.ready = true;
        self.notify();
        Ok(true)
    }
    /// 只从精确的 base revision 续读，不猜测或跳过缺失的 batch。
    pub async fn next(&self, resume: Option<&Resume>) -> Result<ReadNext> {
        let state = self.inner.lock().await;
        Self::check(&state)?;
        let reason = match resume {
            None => Some("initial"),
            Some(r) if r.epoch != self.epoch || r.scope != self.scope => Some("identity_changed"),
            Some(r) if r.revision > state.revision => Some("future_cursor"),
            Some(r) if r.revision == state.revision => return Ok(ReadNext::Idle),
            Some(r) => {
                if let Some(delta) = state.history.iter().find(|d| d.base_revision == r.revision) {
                    return Ok(ReadNext::Delta(delta.clone()));
                }
                Some("history_expired")
            }
        };
        Ok(ReadNext::Reset {
            reason: reason.unwrap_or("reset"),
            snapshot: self.snapshot_of(&state),
        })
    }
    /// source 失败时不继续把旧投影报告为 healthy。
    pub async fn unavailable(&self) {
        self.inner.lock().await.ready = false;
        self.notify();
    }
    pub async fn stop(&self) {
        self.inner.lock().await.stopped = true;
        self.notify();
    }
}
