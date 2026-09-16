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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    pub epoch: String,
    pub scope: String,
    pub revision: u64,
}
