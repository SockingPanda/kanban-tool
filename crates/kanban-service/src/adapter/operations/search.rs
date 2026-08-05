use crate::{SearchHit, SearchIndexStatus, SearchMeta, SearchQuery, SearchResults, SearchTasks};
use crate::{StoreSearchIndexStatus, StoreSearchMeta, StoreSearchQuery, StoreSearchResults};
use kanban_core::Result;

use crate::adapter::{TursoApplicationStore, store_error};

impl SearchTasks for TursoApplicationStore {
    async fn search_tasks(&self, query: SearchQuery) -> Result<SearchResults> {
        let query = StoreSearchQuery {
            board: query.board,
            q: query.q,
            statuses: query.statuses,
            labels: query.labels,
            assignee: query.assignee,
            include_archived: query.include_archived,
            limit: query.limit,
            offset: query.offset,
        };
        let results = self.store.search_tasks(query).await.map_err(store_error)?;
        Ok(map_results(results))
    }

    async fn search_index_status(&self, board: &str) -> Result<SearchIndexStatus> {
        self.store
            .search_index_status(board)
            .await
            .map(map_status)
            .map_err(store_error)
    }

    async fn rebuild_search_index(&self, board: &str) -> Result<SearchIndexStatus> {
        self.store
            .rebuild_search_index(board)
            .await
            .map(map_status)
            .map_err(store_error)
    }

    async fn sync_search_index(&self, board: &str) -> Result<SearchIndexStatus> {
        self.store
            .sync_search_index(board)
            .await
            .map(map_status)
            .map_err(store_error)
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
