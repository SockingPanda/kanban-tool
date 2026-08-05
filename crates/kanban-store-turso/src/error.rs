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
    RunNotFound(String),
    StepNotFound(String),
    DependencyCycle(String),
    TaskConflict(String),
    IdempotencyConflict {
        board_id: String,
        key: String,
        existing_task_id: String,
    },
    SchemaMismatch(String),
    BackupRequired(String),
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
            Self::RunNotFound(run_id) => write!(formatter, "run not found: {run_id}"),
            Self::StepNotFound(step_id) => write!(formatter, "step not found: {step_id}"),
            Self::DependencyCycle(message) => write!(formatter, "dependency cycle: {message}"),
            Self::TaskConflict(task_id) => write!(formatter, "task id already exists: {task_id}"),
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
        }
    }
}

impl Error for StoreError {}

impl From<turso::Error> for StoreError {
    fn from(error: turso::Error) -> Self {
        Self::Turso(error)
    }
}
