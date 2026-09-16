use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectErrorCode {
    InvalidArgument,
    NotFound,
    Conflict,
    FailedPrecondition,
    Storage,
    CommitUnknown,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectError {
    pub code: ObjectErrorCode,
    pub message: String,
}
impl ObjectError {
    pub(crate) fn invalid(m: impl Into<String>) -> Self {
        Self {
            code: ObjectErrorCode::InvalidArgument,
            message: m.into(),
        }
    }
    pub(crate) fn missing(m: impl Into<String>) -> Self {
        Self {
            code: ObjectErrorCode::NotFound,
            message: m.into(),
        }
    }
    pub(crate) fn conflict(m: impl Into<String>) -> Self {
        Self {
            code: ObjectErrorCode::Conflict,
            message: m.into(),
        }
    }
    pub(crate) fn precondition(m: impl Into<String>) -> Self {
        Self {
            code: ObjectErrorCode::FailedPrecondition,
            message: m.into(),
        }
    }
    pub(crate) fn storage(e: impl std::fmt::Display) -> Self {
        Self {
            code: ObjectErrorCode::Storage,
            message: e.to_string(),
        }
    }
}
impl std::fmt::Display for ObjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for ObjectError {}
impl From<turso::Error> for ObjectError {
    fn from(e: turso::Error) -> Self {
        Self::storage(e)
    }
}
impl From<serde_json::Error> for ObjectError {
    fn from(e: serde_json::Error) -> Self {
        Self::storage(e)
    }
}
pub type ObjectResult<T> = std::result::Result<T, ObjectError>;
