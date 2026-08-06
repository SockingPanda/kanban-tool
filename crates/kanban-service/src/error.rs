use std::{
    error::Error,
    fmt::{Display, Formatter},
};

use kanban_core::KanbanError;

#[derive(Debug)]
pub enum StoreError {
    Turso(turso::Error),
    InvalidPath,
    InvalidInput(String),
    InvalidTransition(String),
    StepsIncomplete(String),
    ClaimConflict(String),
    ClaimTokenMismatch,
    InvalidStoredValue {
        field: &'static str,
    },
    BoardNotFound(String),
    TaskNotFound(String),
    LabelNotFound(String),
    RunNotFound(String),
    StepNotFound(String),
    AttachmentNotFound(String),
    AttachmentFileMissing(String),
    AttachmentConflict(String),
    AttachmentIntegrity(String),
    AttachmentIo(String),
    SignalNotFound(String),
    EntityNotFound(String),
    PredicateNotFound(String),
    RelationNotFound(String),
    EntityConflict(String),
    RelationConflict(String),
    DependencyCycle(String),
    TaskConflict(String),
    SignalConflict(String),
    SignalIdempotencyConflict {
        board_id: String,
        key: String,
        existing_signal_id: String,
    },
    IdempotencyConflict {
        board_id: String,
        key: String,
        existing_task_id: String,
    },
    SchemaMismatch(String),
    BackupRequired(String),
    LegacyImport(String),
    MaintenanceBusy(String),
}

impl Display for StoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Turso(error) => write!(formatter, "turso 错误：{error}"),
            Self::InvalidPath => write!(formatter, "数据库路径必须是有效且非空的 UTF-8"),
            Self::InvalidInput(message) => write!(formatter, "任务输入无效：{message}"),
            Self::InvalidTransition(message) => {
                write!(formatter, "任务状态转换无效：{message}")
            }
            Self::StepsIncomplete(message) => write!(formatter, "步骤未完成：{message}"),
            Self::ClaimConflict(message) => write!(formatter, "认领冲突：{message}"),
            Self::ClaimTokenMismatch => write!(formatter, "claim token mismatch"),
            Self::InvalidStoredValue { field } => {
                write!(formatter, "存储值无效：{field}")
            }
            Self::BoardNotFound(selector) => write!(formatter, "看板不存在：{selector}"),
            Self::TaskNotFound(task_id) => write!(formatter, "任务不存在：{task_id}"),
            Self::LabelNotFound(label) => write!(formatter, "标签不存在：{label}"),
            Self::RunNotFound(run_id) => write!(formatter, "运行记录不存在：{run_id}"),
            Self::StepNotFound(step_id) => write!(formatter, "步骤不存在：{step_id}"),
            Self::AttachmentNotFound(attachment_id) => {
                write!(formatter, "附件不存在：{attachment_id}")
            }
            Self::AttachmentFileMissing(path) => {
                write!(formatter, "附件文件缺失：{path}")
            }
            Self::AttachmentConflict(message) => write!(formatter, "附件冲突：{message}"),
            Self::AttachmentIntegrity(message) => {
                write!(formatter, "附件完整性校验失败：{message}")
            }
            Self::AttachmentIo(message) => write!(formatter, "附件 I/O 错误：{message}"),
            Self::SignalNotFound(signal_id) => write!(formatter, "信号不存在：{signal_id}"),
            Self::EntityNotFound(uri) => write!(formatter, "实体不存在：{uri}"),
            Self::PredicateNotFound(name) => {
                write!(formatter, "关系谓词不存在：{name}")
            }
            Self::RelationNotFound(id) => write!(formatter, "关系不存在：{id}"),
            Self::EntityConflict(message) => write!(formatter, "实体冲突：{message}"),
            Self::RelationConflict(message) => write!(formatter, "关系冲突：{message}"),
            Self::DependencyCycle(message) => write!(formatter, "依赖环：{message}"),
            Self::TaskConflict(task_id) => write!(formatter, "任务 id 已存在：{task_id}"),
            Self::SignalConflict(message) => write!(formatter, "信号冲突：{message}"),
            Self::SignalIdempotencyConflict {
                board_id,
                key,
                existing_signal_id,
            } => write!(
                formatter,
                "信号幂等冲突：board {board_id}、key {key}、已有 signal {existing_signal_id}"
            ),
            Self::IdempotencyConflict {
                board_id,
                key,
                existing_task_id,
            } => write!(
                formatter,
                "幂等冲突：board {board_id}、key {key}、已有 task {existing_task_id}"
            ),
            Self::SchemaMismatch(message) => write!(formatter, "schema 不匹配: {message}"),
            Self::BackupRequired(message) => write!(formatter, "需要备份: {message}"),
            Self::LegacyImport(message) => write!(formatter, "旧 SQLite 导入失败: {message}"),
            Self::MaintenanceBusy(message) => write!(formatter, "维护租约忙: {message}"),
        }
    }
}

impl Error for StoreError {}

impl From<turso::Error> for StoreError {
    fn from(error: turso::Error) -> Self {
        Self::Turso(error)
    }
}

/// 将 canonical store 错误收敛到 application service 的稳定错误边界。
pub(crate) fn store_error(error: StoreError) -> KanbanError {
    match error {
        StoreError::BoardNotFound(selector) => KanbanError::NotFound(format!("看板 {selector}")),
        StoreError::TaskNotFound(task_id) => KanbanError::NotFound(format!("task {task_id}")),
        StoreError::LabelNotFound(label) => KanbanError::NotFound(format!("label {label}")),
        StoreError::RunNotFound(run_id) => KanbanError::NotFound(format!("run {run_id}")),
        StoreError::StepNotFound(step_id) => KanbanError::NotFound(format!("step {step_id}")),
        StoreError::AttachmentNotFound(attachment_id) => {
            KanbanError::NotFound(format!("attachment {attachment_id}"))
        }
        StoreError::AttachmentFileMissing(path) => {
            KanbanError::Storage(format!("attachment file missing: {path}"))
        }
        StoreError::AttachmentConflict(message) => KanbanError::Conflict(message),
        StoreError::AttachmentIntegrity(message) => KanbanError::Storage(message),
        StoreError::AttachmentIo(message) => KanbanError::Storage(message),
        StoreError::SignalNotFound(signal_id) => {
            KanbanError::NotFound(format!("signal {signal_id}"))
        }
        StoreError::EntityNotFound(uri) => KanbanError::NotFound(format!("entity {uri}")),
        StoreError::PredicateNotFound(name) => {
            KanbanError::NotFound(format!("relation predicate {name}"))
        }
        StoreError::RelationNotFound(id) => KanbanError::NotFound(format!("relation {id}")),
        StoreError::EntityConflict(message) | StoreError::RelationConflict(message) => {
            KanbanError::Conflict(message)
        }
        StoreError::DependencyCycle(message) => KanbanError::Conflict(message),
        StoreError::TaskConflict(task_id) => {
            KanbanError::Conflict(format!("task id already exists: {task_id}"))
        }
        StoreError::InvalidInput(message) if message.contains("hash mismatch") => {
            KanbanError::Conflict(message)
        }
        StoreError::SignalConflict(message) => KanbanError::Conflict(message),
        StoreError::SignalIdempotencyConflict {
            board_id,
            key,
            existing_signal_id,
        } => KanbanError::IdempotencyConflict(format!(
            "board {board_id}, key {key}, existing signal {existing_signal_id}"
        )),
        StoreError::InvalidInput(message) => KanbanError::InvalidInput(message),
        StoreError::InvalidTransition(message) => KanbanError::InvalidTransition(message),
        StoreError::ClaimConflict(message) => {
            KanbanError::InvalidTransition(format!("claim conflict: {message}"))
        }
        StoreError::ClaimTokenMismatch => {
            KanbanError::InvalidTransition("claim token mismatch".to_owned())
        }
        StoreError::StepsIncomplete(message) => KanbanError::StepsIncomplete(message),
        StoreError::IdempotencyConflict {
            board_id,
            key,
            existing_task_id,
        } => KanbanError::IdempotencyConflict(format!(
            "board {board_id}, key {key}, existing task {existing_task_id}"
        )),
        StoreError::MaintenanceBusy(message) => KanbanError::Conflict(message),
        StoreError::BackupRequired(message) => KanbanError::Conflict(message),
        StoreError::LegacyImport(message) => {
            KanbanError::Storage(format!("legacy sqlite import failed: {message}"))
        }
        other => KanbanError::Storage(other.to_string()),
    }
}
