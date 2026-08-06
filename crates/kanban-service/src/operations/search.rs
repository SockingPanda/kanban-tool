use kanban_core::{Clock, KanbanError, Result, TaskStatus};
use serde::{Deserialize, Serialize};

use crate::KanbanService;
use crate::store_operations::search::{
    StoreSearchIndexStatus, StoreSearchMeta, StoreSearchQuery, StoreSearchResults,
};

pub const MAX_SEARCH_LIMIT: usize = 1_000;

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
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.board.trim().is_empty() {
            return Err("board is required".to_owned());
        }
        if self.limit > MAX_SEARCH_LIMIT {
            return Err(format!("limit must be <= {MAX_SEARCH_LIMIT}"));
        }
        if i64::try_from(self.offset).is_err() {
            return Err("offset is too large".to_owned());
        }
        if self
            .q
            .as_deref()
            .is_some_and(|value| value.chars().count() > 1_024)
        {
            return Err("query exceeds 1024 characters".to_owned());
        }
        if self
            .assignee
            .as_deref()
            .is_some_and(|value| value.chars().count() > 128)
        {
            return Err("assignee exceeds 128 characters".to_owned());
        }
        Ok(())
    }

    pub fn normalized(mut self) -> Self {
        self.board = self.board.trim().to_owned();
        self.q = self.q.and_then(trimmed);
        self.assignee = self.assignee.and_then(trimmed);
        self.labels = self.labels.into_iter().filter_map(trimmed).collect();
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

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn search_tasks(&self, query: SearchQuery) -> Result<SearchResults> {
        let query = query.normalized();
        query
            .validate()
            .map_err(|error| KanbanError::InvalidInput(error.to_string()))?;
        self.store
            .search_tasks(StoreSearchQuery {
                board: query.board,
                q: query.q,
                statuses: query.statuses,
                labels: query.labels,
                assignee: query.assignee,
                include_archived: query.include_archived,
                limit: query.limit,
                offset: query.offset,
            })
            .await
            .map_err(crate::error::store_error)
            .map(map_results)
    }

    pub async fn search_index_status(&self, board: &str) -> Result<SearchIndexStatus> {
        let board = normalize_board(board)?;
        self.store
            .search_index_status(&board)
            .await
            .map_err(crate::error::store_error)
            .map(map_status)
    }

    pub async fn rebuild_search_index(&self, board: &str) -> Result<SearchIndexStatus> {
        let board = normalize_board(board)?;
        self.store
            .rebuild_search_index(&board)
            .await
            .map_err(crate::error::store_error)
            .map(map_status)
    }

    pub async fn sync_search_index(&self, board: &str) -> Result<SearchIndexStatus> {
        let board = normalize_board(board)?;
        self.store
            .sync_search_index(&board)
            .await
            .map_err(crate::error::store_error)
            .map(map_status)
    }
}

fn map_results(value: StoreSearchResults) -> SearchResults {
    let StoreSearchResults { hits, meta } = value;
    SearchResults {
        hits: hits
            .into_iter()
            .map(|hit| SearchHit {
                task_id: hit.task_id,
                seq: hit.seq,
                score: hit.score,
                snippet: hit.snippet,
            })
            .collect(),
        meta: map_meta(meta),
    }
}

fn map_meta(value: StoreSearchMeta) -> SearchMeta {
    SearchMeta {
        backend: value.backend,
        stale: value.stale,
        database_instance_id: value.database_instance_id,
        protocol_version: value.protocol_version,
        generation: value.generation,
        resolved_board_id: value.resolved_board_id,
        fallback_reason: value.fallback_reason,
        index_version: value.index_version,
        last_event_id: value.last_event_id,
        index_lag_events: value.index_lag_events,
    }
}

fn map_status(value: StoreSearchIndexStatus) -> SearchIndexStatus {
    SearchIndexStatus {
        backend: value.backend,
        derived_index: value.derived_index,
        stale: value.stale,
        database_instance_id: value.database_instance_id,
        protocol_version: value.protocol_version,
        generation: value.generation,
        resolved_board_id: value.resolved_board_id,
        fallback_reason: value.fallback_reason,
        index_version: value.index_version,
        last_event_id: value.last_event_id,
        index_lag_events: value.index_lag_events,
        message: value.message,
    }
}

fn normalize_board(board: &str) -> Result<String> {
    let board = board.trim();
    if board.is_empty() {
        return Err(KanbanError::InvalidInput("board is required".to_owned()));
    }
    Ok(board.to_owned())
}
