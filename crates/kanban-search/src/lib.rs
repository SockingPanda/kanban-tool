//! Task search 的共享领域 query/result 类型。
//!
//! 搜索索引由 host 内的 `kanban-service` 持有；本 crate 不包含任何
//! 外部全文检索 provider。Turso 的 FTS projection 是可重建的派生物，canonical
//! `tasks`、`task_comments`、`task_runs` 和 `task_events` 仍然是搜索事实来源。

use kanban_core::TaskStatus;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

pub const MAX_SEARCH_LIMIT: usize = 1_000;
pub const SEARCH_INDEX_VERSION: &str = "turso-fts-task-v1";
pub const SEARCH_PROVIDER: &str = "turso_fts";
pub const SEARCH_PROVIDER_FINGERPRINT: &str = "turso-fts-task-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchQuery {
    pub board: String,
    pub q: Option<String>,
    pub statuses: Vec<TaskStatus>,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    pub include_archived: bool,
    pub limit: usize,
    pub offset: usize,
}

impl SearchQuery {
    pub fn validate(&self) -> Result<(), SearchBackendError> {
        if self.board.trim().is_empty() {
            return Err(SearchBackendError::invalid("board is required"));
        }
        if self.limit > MAX_SEARCH_LIMIT {
            return Err(SearchBackendError::invalid(format!(
                "limit must be <= {MAX_SEARCH_LIMIT}"
            )));
        }
        if i64::try_from(self.offset).is_err() {
            return Err(SearchBackendError::invalid("offset is too large"));
        }
        if self
            .q
            .as_deref()
            .is_some_and(|value| value.chars().count() > 1_024)
        {
            return Err(SearchBackendError::invalid(
                "query exceeds 1024 characters",
            ));
        }
        if self
            .assignee
            .as_deref()
            .is_some_and(|value| value.chars().count() > 128)
        {
            return Err(SearchBackendError::invalid(
                "assignee exceeds 128 characters",
            ));
        }
        Ok(())
    }

    pub fn normalized(mut self) -> Self {
        self.board = self.board.trim().to_owned();
        self.q = self.q.and_then(trimmed);
        self.assignee = self.assignee.and_then(trimmed);
        self.labels = self
            .labels
            .into_iter()
            .filter_map(|value| trimmed(value))
            .collect();
        self
    }
}

fn trimmed(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub task_id: String,
    pub seq: i64,
    pub score: f64,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchMeta {
    pub backend: String,
    pub stale: bool,
    pub database_instance_id: Option<String>,
    pub protocol_version: Option<i64>,
    pub generation: Option<String>,
    pub resolved_board_id: String,
    pub fallback_reason: Option<String>,
    pub index_version: Option<String>,
    pub last_event_id: Option<i64>,
    pub index_lag_events: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    pub meta: SearchMeta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchIndexStatus {
    pub backend: String,
    pub derived_index: bool,
    pub stale: bool,
    pub database_instance_id: Option<String>,
    pub protocol_version: Option<i64>,
    pub generation: Option<String>,
    pub resolved_board_id: String,
    pub fallback_reason: Option<String>,
    pub index_version: Option<String>,
    pub last_event_id: Option<i64>,
    pub index_lag_events: Option<i64>,
    pub message: String,
}

/// FTS projection 使用的 canonical task 快照形状。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSearchDocument {
    pub board_id: String,
    pub task_id: String,
    pub seq: i64,
    pub status: TaskStatus,
    pub assignee: Option<String>,
    pub priority: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub due_at: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub comments: String,
    pub run_text: String,
    pub event_text: String,
}

impl TaskSearchDocument {
    pub fn content(&self) -> String {
        [
            self.title.as_str(),
            self.description.as_deref().unwrap_or_default(),
            self.comments.as_str(),
            self.run_text.as_str(),
            self.event_text.as_str(),
        ]
        .join("\n")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchBackendErrorKind {
    InvalidInput,
    Unavailable,
    Corrupt,
    Query,
    Storage,
}

#[derive(Debug, Clone)]
pub struct SearchBackendError {
    kind: SearchBackendErrorKind,
    message: String,
}

impl SearchBackendError {
    pub fn new(kind: SearchBackendErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(SearchBackendErrorKind::InvalidInput, message)
    }

    pub fn kind(&self) -> SearchBackendErrorKind {
        self.kind
    }

    pub fn is_fallback_eligible(&self) -> bool {
        matches!(
            self.kind,
            SearchBackendErrorKind::Unavailable
                | SearchBackendErrorKind::Corrupt
                | SearchBackendErrorKind::Query
        )
    }
}

impl fmt::Display for SearchBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SearchBackendError {}
