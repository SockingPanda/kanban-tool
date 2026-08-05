use std::{
    error::Error,
    fmt::{Display, Formatter},
};

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
            Self::Turso(error) => write!(formatter, "turso error: {error}"),
            Self::InvalidPath => write!(formatter, "database path must be valid non-empty UTF-8"),
            Self::InvalidInput(message) => write!(formatter, "invalid task input: {message}"),
            Self::InvalidTransition(message) => {
                write!(formatter, "invalid task transition: {message}")
            }
            Self::StepsIncomplete(message) => write!(formatter, "steps incomplete: {message}"),
            Self::ClaimConflict(message) => write!(formatter, "claim conflict: {message}"),
            Self::ClaimTokenMismatch => write!(formatter, "claim token mismatch"),
            Self::InvalidStoredValue { field } => {
                write!(formatter, "invalid stored value for {field}")
            }
            Self::BoardNotFound(selector) => write!(formatter, "board not found: {selector}"),
            Self::TaskNotFound(task_id) => write!(formatter, "task not found: {task_id}"),
            Self::LabelNotFound(label) => write!(formatter, "label not found: {label}"),
            Self::RunNotFound(run_id) => write!(formatter, "run not found: {run_id}"),
            Self::StepNotFound(step_id) => write!(formatter, "step not found: {step_id}"),
            Self::AttachmentNotFound(attachment_id) => {
                write!(formatter, "attachment not found: {attachment_id}")
            }
            Self::AttachmentFileMissing(path) => {
                write!(formatter, "attachment file missing: {path}")
            }
            Self::AttachmentConflict(message) => {
                write!(formatter, "attachment conflict: {message}")
            }
            Self::AttachmentIntegrity(message) => {
                write!(formatter, "attachment integrity failure: {message}")
            }
            Self::AttachmentIo(message) => write!(formatter, "attachment I/O error: {message}"),
            Self::SignalNotFound(signal_id) => write!(formatter, "signal not found: {signal_id}"),
            Self::EntityNotFound(uri) => write!(formatter, "entity not found: {uri}"),
            Self::PredicateNotFound(name) => {
                write!(formatter, "relation predicate not found: {name}")
            }
            Self::RelationNotFound(id) => write!(formatter, "relation not found: {id}"),
            Self::EntityConflict(message) => write!(formatter, "entity conflict: {message}"),
            Self::RelationConflict(message) => write!(formatter, "relation conflict: {message}"),
            Self::DependencyCycle(message) => write!(formatter, "dependency cycle: {message}"),
            Self::TaskConflict(task_id) => write!(formatter, "task id already exists: {task_id}"),
            Self::SignalConflict(message) => write!(formatter, "signal conflict: {message}"),
            Self::SignalIdempotencyConflict {
                board_id,
                key,
                existing_signal_id,
            } => write!(
                formatter,
                "signal idempotency conflict for board {board_id}, key {key}, existing signal {existing_signal_id}"
            ),
            Self::IdempotencyConflict {
                board_id,
                key,
                existing_task_id,
            } => write!(
                formatter,
                "idempotency conflict for board {board_id}, key {key}, existing task {existing_task_id}"
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
