use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type Result<T> = std::result::Result<T, LiveError>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum LiveError {
    #[error("输入无效: {0}")]
    Invalid(String),
    #[error("不存在: {0}")]
    NotFound(String),
    #[error("版本冲突: {0}")]
    Conflict(String),
    #[error("投影暂不可用")]
    Unavailable,
    #[error("超过投影预算: {0}")]
    Budget(String),
    #[error("宿主已停止")]
    Stopped,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status { Triage, Todo, Scheduled, Ready, Running, Blocked, Review, Done, Archived }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub status: Status,
    pub priority: u32,
    pub position: i64,
    pub seq: u64,
    pub lock_version: u64,
}
impl Card {
    /// 估算的是数据预算，非 Rust allocator 的精确 heap 用量。
    pub fn weight(&self) -> usize { self.id.len() + self.title.len() + 128 }
    pub fn validate(&self) -> Result<()> {
        if !self.id.starts_with("t_") || self.id.len() <= 2 || self.id.len() > 128
            || self.id.chars().any(char::is_control) || self.title.trim().is_empty()
            || self.title.len() > 4096 || self.priority > 3 {
            return Err(LiveError::Invalid("TaskCard 字段或大小无效".into()));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume { pub epoch: String, pub scope: String, pub revision: u64 }
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub cursor: Resume,
    pub cards: Arc<BTreeMap<String, Card>>,
}
#[derive(Debug, Clone)]
pub struct Delta {
    pub base_revision: u64,
    pub revision: u64,
    pub upserts: Vec<Card>,
    pub removed: Vec<String>,
}
impl Delta {
    pub fn weight(&self) -> usize {
        128 + self.upserts.iter().map(Card::weight).sum::<usize>()
            + self.removed.iter().map(|id| id.len() + 32).sum::<usize>()
    }
}
#[derive(Debug, Clone)]
pub struct Limits {
    pub max_cards: usize,
    pub snapshot_bytes: usize,
    pub history_batches: usize,
    pub history_bytes: usize,
    pub delta_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self { max_cards: 50_000, snapshot_bytes: 16 * 1024 * 1024,
            history_batches: 256, history_bytes: 8 * 1024 * 1024, delta_bytes: 256 * 1024 }
    }
}
#[derive(Debug, Clone)]
pub struct UpdateTitle {
    pub board_id: String, pub task_id: String, pub title: String,
    pub actor: String, pub expected_version: u64,
}
/// 后续业务 handler 依同样方式调用 application service，不回调 HTTP。
pub trait TaskCommands: Send + Sync + 'static {
    fn update_title(&self, input: UpdateTitle) -> BoxFuture<'_, Result<Card>>;
}
pub fn validate_board(id: &str) -> Result<()> {
    if !id.starts_with("b_") || id.len() <= 2 || id.len() > 128 || id.chars().any(char::is_control) {
        return Err(LiveError::Invalid("必须提供 canonical b_... ID".into()));
    }
    Ok(())
}
