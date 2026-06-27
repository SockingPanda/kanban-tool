use thiserror::Error;

pub type Result<T> = std::result::Result<T, KanbanError>;

#[derive(Debug, Error)]
pub enum KanbanError {
    #[error("invalid status: {0}")]
    InvalidStatus(String),
    #[error("invalid transition: {0}")]
    InvalidTransition(String),
    #[error("execution_plan_required: {0}")]
    ExecutionPlanRequired(String),
    #[error("steps_incomplete: {0}")]
    StepsIncomplete(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("storage error: {0}")]
    Storage(String),
}
