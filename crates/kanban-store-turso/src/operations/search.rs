//! Turso-native task search and host-local FTS projection maintenance.

use kanban_core::TaskStatus;
use sha2::{Digest, Sha256};
use turso::{Connection, Row, Value, transaction::TransactionBehavior};

use crate::{
    db::TursoStore,
    error::StoreError,
    shared::{first_row, integer_value, now_ms, optional_text_value, text_value},
};

const TASK_SOURCE_KIND: &str = "task";
const SEARCH_INDEX_VERSION: &str = "turso-fts-task-v1";
const SEARCH_PROVIDER: &str = "turso_fts";
const SEARCH_PROVIDER_FINGERPRINT: &str = "turso-fts-task-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSearchQuery {
    pub board: String,
    pub q: Option<String>,
    pub statuses: Vec<TaskStatus>,
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    pub include_archived: bool,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoreSearchHit {
    pub task_id: String,
    pub seq: i64,
    pub score: f64,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSearchMeta {
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

#[derive(Debug, Clone, PartialEq)]
pub struct StoreSearchResults {
    pub hits: Vec<StoreSearchHit>,
    pub meta: StoreSearchMeta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreSearchIndexStatus {
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

type SearchQuery = StoreSearchQuery;
type SearchHit = StoreSearchHit;
type SearchMeta = StoreSearchMeta;
type SearchResults = StoreSearchResults;
type SearchIndexStatus = StoreSearchIndexStatus;

#[derive(Debug, Clone)]
struct StoreTaskSearchDocument {
    board_id: String,
    task_id: String,
    created_at: i64,
    updated_at: i64,
    title: String,
    description: Option<String>,
    comments: String,
    run_text: String,
    event_text: String,
}

impl StoreTaskSearchDocument {
    fn content(&self) -> String {
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

type TaskSearchDocument = StoreTaskSearchDocument;

#[derive(Debug, Clone)]
struct BoardRef {
    id: String,
    slug: String,
}

#[derive(Debug, Clone)]
struct ProjectionState {
    lifecycle_status: String,
    active_generation: Option<String>,
    provider: Option<String>,
    provider_fingerprint: Option<String>,
    last_event_id: i64,
    dirty: bool,
    last_error: Option<String>,
}

impl TursoStore {
    /// 查询任务。FTS 未 ready、FTS provider 失败或 query 不被 provider 接受时，
    /// 退回 canonical SQL，并在 meta 中明确标记 degraded/fallback 原因。
    pub async fn search_tasks(&self, query: SearchQuery) -> Result<SearchResults, StoreError> {
        let connection = self.connection().await?;
        let board = resolve_board(&connection, &query.board).await?;
        let state = projection_state(&connection).await?;
        let current_event = board_last_event_id(&connection, &board.id).await?;

        if should_use_fts(&query, &board, state.as_ref(), current_event) {
            match fts_search(&connection, &board, &query).await {
                Ok(hits) => {
                    return Ok(SearchResults {
                        hits,
                        meta: fts_meta(&board, state.as_ref(), current_event),
                    });
                }
                Err(error) => {
                    let hits = canonical_search(&connection, &board, &query).await?;
                    return Ok(SearchResults {
                        hits,
                        meta: canonical_meta(
                            &board,
                            state.as_ref(),
                            current_event,
                            true,
                            Some(if error.to_string().contains("fts") {
                                "fts_query_invalid".to_owned()
                            } else {
                                "fts_unavailable".to_owned()
                            }),
                        ),
                    });
                }
            }
        }

        let hits = canonical_search(&connection, &board, &query).await?;
        let exact = query
            .q
            .as_deref()
            .is_some_and(|value| is_exact_reference(value, &board));
        Ok(SearchResults {
            hits,
            meta: canonical_meta(
                &board,
                state.as_ref(),
                current_event,
                state
                    .as_ref()
                    .is_some_and(|value| value.dirty || current_event > value.last_event_id),
                exact.then(|| "exact_task_reference".to_owned()),
            ),
        })
    }

    pub async fn search_index_status(
        &self,
        board_selector: &str,
    ) -> Result<SearchIndexStatus, StoreError> {
        let connection = self.connection().await?;
        let board = resolve_board(&connection, board_selector).await?;
        let current_event = board_last_event_id(&connection, &board.id).await?;
        let state = projection_state(&connection).await?;
        let capability = fts_capability(&connection).await?;
        let ready = capability
            && state.as_ref().is_some_and(|value| {
                value.lifecycle_status == "ready"
                    && !value.dirty
                    && value.provider.as_deref() == Some(SEARCH_PROVIDER)
                    && value.provider_fingerprint.as_deref() == Some(SEARCH_PROVIDER_FINGERPRINT)
                    && value.last_event_id == current_event
            });
        if ready {
            return Ok(SearchIndexStatus {
                backend: SEARCH_PROVIDER.to_owned(),
                derived_index: true,
                stale: false,
                database_instance_id: None,
                protocol_version: Some(2),
                generation: state
                    .as_ref()
                    .and_then(|value| value.active_generation.clone()),
                resolved_board_id: board.id,
                fallback_reason: None,
                index_version: Some(SEARCH_INDEX_VERSION.to_owned()),
                last_event_id: Some(current_event),
                index_lag_events: Some(0),
                message: "Turso FTS task projection is ready".to_owned(),
            });
        }
        let reason = if !capability {
            "fts_unavailable"
        } else if state
            .as_ref()
            .and_then(|value| value.last_error.as_deref())
            .is_some()
        {
            "projection_failed"
        } else {
            "projection_not_ready"
        };
        Ok(SearchIndexStatus {
            backend: "canonical".to_owned(),
            derived_index: false,
            stale: true,
            database_instance_id: None,
            protocol_version: Some(2),
            generation: state
                .as_ref()
                .and_then(|value| value.active_generation.clone()),
            resolved_board_id: board.id,
            fallback_reason: Some(reason.to_owned()),
            index_version: None,
            last_event_id: Some(current_event),
            index_lag_events: state
                .as_ref()
                .map(|value| current_event.saturating_sub(value.last_event_id)),
            message: format!(
                "Turso FTS task projection unavailable ({reason}); canonical fallback search is active"
            ),
        })
    }

    pub async fn rebuild_search_index(
        &self,
        board_selector: &str,
    ) -> Result<SearchIndexStatus, StoreError> {
        let mut connection = self.connection().await?;
        let board = resolve_board(&connection, board_selector).await?;
        if !fts_capability(&connection).await? {
            mark_projection_error(&connection, "fts_unavailable").await?;
            return self.search_index_status(board_selector).await;
        }
        let documents = task_search_documents(&connection, &board.id).await?;
        let now = now_ms();
        let generation = format!("fts-{now}");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        set_projection_building(&transaction, &generation, now).await?;
        replace_board_documents(&transaction, &board.id, &documents, now).await?;
        let last_event_id = board_last_event_id_tx(&transaction, &board.id).await?;
        transaction
            .execute(
                "UPDATE projection_state SET lifecycle_status='ready', active_generation=:generation, active_fingerprint=:fingerprint, previous_generation=NULL, previous_fingerprint=NULL, building_generation=NULL, building_fingerprint=NULL, provider=:provider, provider_fingerprint=:provider_fingerprint, corpus_schema='task-search-v1', corpus_fingerprint=:corpus_fingerprint, last_event_id=:last_event_id, dirty=0, last_success_at=:now, last_error=NULL, updated_at=:now WHERE projection='fts'",
                vec![
                    (":generation".to_owned(), Value::Text(generation.clone())),
                    (":fingerprint".to_owned(), Value::Text(SEARCH_PROVIDER_FINGERPRINT.to_owned())),
                    (":provider".to_owned(), Value::Text(SEARCH_PROVIDER.to_owned())),
                    (":provider_fingerprint".to_owned(), Value::Text(SEARCH_PROVIDER_FINGERPRINT.to_owned())),
                    (":corpus_fingerprint".to_owned(), Value::Text(SEARCH_PROVIDER_FINGERPRINT.to_owned())),
                    (":last_event_id".to_owned(), Value::Integer(last_event_id)),
                    (":now".to_owned(), Value::Integer(now)),
                ],
            )
            .await?;
        transaction
            .execute(
                "UPDATE projection_jobs SET status='done', lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, last_error=NULL, updated_at=:now WHERE target='fts' AND board_id=:board_id AND status IN ('pending','failed')",
                vec![
                    (":now".to_owned(), Value::Integer(now)),
                    (":board_id".to_owned(), Value::Text(board.id.clone())),
                ],
            )
            .await?;
        transaction.commit().await?;
        self.search_index_status(board_selector).await
    }

    pub async fn sync_search_index(
        &self,
        board_selector: &str,
    ) -> Result<SearchIndexStatus, StoreError> {
        let connection = self.connection().await?;
        let board = resolve_board(&connection, board_selector).await?;
        let state = projection_state(&connection).await?;
        let pending = pending_job_count(&connection, &board.id).await?;
        let current_event = board_last_event_id(&connection, &board.id).await?;
        if pending == 0
            && state.as_ref().is_some_and(|value| {
                value.lifecycle_status == "ready"
                    && !value.dirty
                    && value.last_event_id == current_event
            })
        {
            return self.search_index_status(board_selector).await;
        }
        drop(connection);
        self.rebuild_search_index(board_selector).await
    }
}

fn should_use_fts(
    query: &SearchQuery,
    board: &BoardRef,
    state: Option<&ProjectionState>,
    current_event: i64,
) -> bool {
    state.is_some_and(|state| {
        state.lifecycle_status == "ready"
            && !state.dirty
            && state.last_event_id == current_event
            && state.provider.as_deref() == Some(SEARCH_PROVIDER)
            && state.provider_fingerprint.as_deref() == Some(SEARCH_PROVIDER_FINGERPRINT)
    }) && query
        .q
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty() && !is_exact_reference(value, board))
}

async fn resolve_board(connection: &Connection, selector: &str) -> Result<BoardRef, StoreError> {
    let selector = selector.trim();
    let row = first_row(
        connection
            .query(
                "SELECT id, slug FROM boards WHERE id=:selector OR slug=:selector LIMIT 1",
                [(":selector", selector)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    Ok(BoardRef {
        id: text_value(row.get_value(0)?, "boards.id")?,
        slug: text_value(row.get_value(1)?, "boards.slug")?,
    })
}

async fn projection_state(connection: &Connection) -> Result<Option<ProjectionState>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT lifecycle_status, active_generation, provider, provider_fingerprint, last_event_id, dirty, last_error FROM projection_state WHERE projection='fts'",
            (),
        )
        .await?;
    let Some(row) = rows.next().await? else {
        return Ok(None);
    };
    Ok(Some(ProjectionState {
        lifecycle_status: text_value(row.get_value(0)?, "projection_state.lifecycle_status")?,
        active_generation: optional_text_value(
            row.get_value(1)?,
            "projection_state.active_generation",
        )?,
        provider: optional_text_value(row.get_value(2)?, "projection_state.provider")?,
        provider_fingerprint: optional_text_value(
            row.get_value(3)?,
            "projection_state.provider_fingerprint",
        )?,
        last_event_id: integer_value(row.get_value(4)?, "projection_state.last_event_id")?,
        dirty: integer_value(row.get_value(5)?, "projection_state.dirty")? != 0,
        last_error: optional_text_value(row.get_value(6)?, "projection_state.last_error")?,
    }))
}

async fn fts_capability(connection: &Connection) -> Result<bool, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT available FROM schema_capabilities WHERE capability='fts'",
                (),
            )
            .await?,
    )
    .await;
    match row {
        Ok(row) => Ok(integer_value(row.get_value(0)?, "schema_capabilities.available")? != 0),
        Err(turso::Error::QueryReturnedNoRows) => Ok(false),
        Err(error) => Err(StoreError::Turso(error)),
    }
}

async fn board_last_event_id(connection: &Connection, board_id: &str) -> Result<i64, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT COALESCE(MAX(id),0) FROM task_events WHERE board_id=:board_id",
                [(":board_id", board_id)],
            )
            .await?,
    )
    .await?;
    integer_value(row.get_value(0)?, "task_events.last_event_id")
}

async fn board_last_event_id_tx(
    connection: &turso::transaction::Transaction<'_>,
    board_id: &str,
) -> Result<i64, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT COALESCE(MAX(id),0) FROM task_events WHERE board_id=:board_id",
                [(":board_id", board_id)],
            )
            .await?,
    )
    .await?;
    integer_value(row.get_value(0)?, "task_events.last_event_id")
}

async fn pending_job_count(connection: &Connection, board_id: &str) -> Result<i64, StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT COUNT(*) FROM projection_jobs WHERE target='fts' AND board_id=:board_id AND status IN ('pending','failed')",
                [(":board_id", board_id)],
            )
            .await?,
    )
    .await?;
    integer_value(row.get_value(0)?, "projection_jobs.pending_count")
}

async fn task_search_documents(
    connection: &Connection,
    board_id: &str,
) -> Result<Vec<TaskSearchDocument>, StoreError> {
    let mut rows = connection
        .query(
            "SELECT t.id,t.board_id,t.seq,t.status,t.assignee,t.priority,t.created_at,t.updated_at,t.due_at,t.title,t.description, COALESCE((SELECT group_concat(c.body, char(10)) FROM task_comments c WHERE c.board_id=t.board_id AND c.task_id=t.id ORDER BY c.created_at ASC, c.id ASC), ''), COALESCE((SELECT group_concat(COALESCE(r.summary,'') || ' ' || COALESCE(r.error,''), char(10)) FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id ORDER BY r.started_at ASC, r.id ASC), ''), COALESCE((SELECT group_concat(e.kind || ' ' || e.payload_json, char(10)) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id ORDER BY e.id ASC), '') FROM tasks t WHERE t.board_id=:board_id ORDER BY t.seq ASC",
            [(":board_id", board_id)],
        )
        .await?;
    let mut documents = Vec::new();
    while let Some(row) = rows.next().await? {
        documents.push(document_from_row(row)?);
    }
    Ok(documents)
}

fn document_from_row(row: Row) -> Result<TaskSearchDocument, StoreError> {
    Ok(TaskSearchDocument {
        task_id: text_value(row.get_value(0)?, "tasks.id")?,
        board_id: text_value(row.get_value(1)?, "tasks.board_id")?,
        created_at: integer_value(row.get_value(6)?, "tasks.created_at")?,
        updated_at: integer_value(row.get_value(7)?, "tasks.updated_at")?,
        title: text_value(row.get_value(9)?, "tasks.title")?,
        description: optional_text_value(row.get_value(10)?, "tasks.description")?,
        comments: text_value(row.get_value(11)?, "task_comments.body")?,
        run_text: text_value(row.get_value(12)?, "task_runs.summary")?,
        event_text: text_value(row.get_value(13)?, "task_events.payload_json")?,
    })
}

async fn replace_board_documents(
    transaction: &turso::transaction::Transaction<'_>,
    board_id: &str,
    documents: &[TaskSearchDocument],
    now: i64,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "DELETE FROM retrieval_documents WHERE board_id=:board_id AND source_kind=:source_kind",
            [(":board_id", board_id), (":source_kind", TASK_SOURCE_KIND)],
        )
        .await?;
    for document in documents {
        upsert_document(transaction, document, now).await?;
    }
    Ok(())
}

async fn upsert_document(
    transaction: &turso::transaction::Transaction<'_>,
    document: &TaskSearchDocument,
    now: i64,
) -> Result<(), StoreError> {
    let content = document.content();
    let content_hash = format!("sha256:{:x}", Sha256::digest(content.as_bytes()));
    let id = task_document_id(&document.task_id);
    transaction
        .execute(
            "INSERT INTO retrieval_documents(id, board_id, entity_uri, source_kind, content, content_hash, created_at, updated_at) VALUES (:id,:board_id,NULL,:source_kind,:content,:content_hash,:created_at,:updated_at) ON CONFLICT(id) DO UPDATE SET board_id=excluded.board_id, source_kind=excluded.source_kind, content=excluded.content, content_hash=excluded.content_hash, updated_at=excluded.updated_at",
            vec![
                (":id".to_owned(), Value::Text(id)),
                (":board_id".to_owned(), Value::Text(document.board_id.clone())),
                (":source_kind".to_owned(), Value::Text(TASK_SOURCE_KIND.to_owned())),
                (":content".to_owned(), Value::Text(content)),
                (":content_hash".to_owned(), Value::Text(content_hash)),
                (":created_at".to_owned(), Value::Integer(document.created_at)),
                (":updated_at".to_owned(), Value::Integer(now.max(document.updated_at))),
            ],
        )
        .await?;
    Ok(())
}

fn task_document_id(task_id: &str) -> String {
    format!("doc_task_{task_id}")
}

async fn fts_search(
    connection: &Connection,
    board: &BoardRef,
    query: &SearchQuery,
) -> Result<Vec<SearchHit>, StoreError> {
    let needle = query.q.as_deref().unwrap_or_default();
    let (where_sql, mut params) = search_filters(board, query, true);
    let sql = format!(
        "SELECT t.id,t.seq,fts_score(d.content,:fts_query),fts_highlight(d.content,'<mark>','</mark>',:fts_query),t.updated_at FROM retrieval_documents d JOIN tasks t ON d.id=:doc_prefix || t.id AND d.board_id=t.board_id WHERE d.source_kind=:source_kind AND {where_sql} AND fts_match(d.content,:fts_query) ORDER BY fts_score(d.content,:fts_query) DESC,t.updated_at DESC,t.seq ASC LIMIT :limit OFFSET :offset"
    );
    params.push((":fts_query".to_owned(), Value::Text(needle.to_owned())));
    params.push((
        ":doc_prefix".to_owned(),
        Value::Text("doc_task_".to_owned()),
    ));
    params.push((
        ":source_kind".to_owned(),
        Value::Text(TASK_SOURCE_KIND.to_owned()),
    ));
    params.push((":limit".to_owned(), Value::Integer(query.limit as i64)));
    params.push((":offset".to_owned(), Value::Integer(query.offset as i64)));
    let mut rows = connection.query(&sql, params).await?;
    let mut hits = Vec::new();
    while let Some(row) = rows.next().await? {
        let score = match row.get_value(2)? {
            Value::Real(value) => value,
            Value::Integer(value) => value as f64,
            _ => {
                return Err(StoreError::InvalidStoredValue { field: "fts_score" });
            }
        };
        hits.push(SearchHit {
            task_id: text_value(row.get_value(0)?, "tasks.id")?,
            seq: integer_value(row.get_value(1)?, "tasks.seq")?,
            score,
            snippet: optional_text_value(row.get_value(3)?, "fts_highlight")?,
        });
    }
    Ok(hits)
}

async fn canonical_search(
    connection: &Connection,
    board: &BoardRef,
    query: &SearchQuery,
) -> Result<Vec<SearchHit>, StoreError> {
    let (where_sql, mut params) = search_filters(board, query, false);
    let (score_sql, snippet_sql, outer_filter) = if query
        .q
        .as_deref()
        .and_then(|value| exact_reference_clause(value, board))
        .is_some()
    {
        ("120.0", "t.id", "WHERE score > 0.0")
    } else if let Some(q) = query.q.as_deref() {
        let needle = format!("%{}%", like_literal(&q.to_lowercase()));
        for key in [
            ":title_score",
            ":description_score",
            ":comment_score",
            ":run_summary_score",
            ":run_error_score",
            ":event_kind_score",
            ":event_payload_score",
            ":title_snippet",
            ":description_snippet",
            ":comment_snippet",
            ":run_summary_snippet",
            ":run_error_snippet",
            ":event_kind_snippet",
            ":event_payload_snippet",
        ] {
            params.push((key.to_owned(), Value::Text(needle.clone())));
        }
        (
            "CASE WHEN lower(t.title) LIKE :title_score ESCAPE '\\' THEN 100.0 ELSE 0.0 END + CASE WHEN lower(COALESCE(t.description,'')) LIKE :description_score ESCAPE '\\' THEN 60.0 ELSE 0.0 END + CASE WHEN EXISTS (SELECT 1 FROM task_comments c WHERE c.board_id=t.board_id AND c.task_id=t.id AND lower(c.body) LIKE :comment_score ESCAPE '\\') THEN 40.0 ELSE 0.0 END + CASE WHEN EXISTS (SELECT 1 FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id AND lower(COALESCE(r.summary,'')) LIKE :run_summary_score ESCAPE '\\') THEN 30.0 ELSE 0.0 END + CASE WHEN EXISTS (SELECT 1 FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id AND lower(COALESCE(r.error,'')) LIKE :run_error_score ESCAPE '\\') THEN 30.0 ELSE 0.0 END + CASE WHEN EXISTS (SELECT 1 FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id AND lower(e.kind) LIKE :event_kind_score ESCAPE '\\') THEN 20.0 ELSE 0.0 END + CASE WHEN EXISTS (SELECT 1 FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id AND lower(e.payload_json) LIKE :event_payload_score ESCAPE '\\') THEN 20.0 ELSE 0.0 END",
            "COALESCE(CASE WHEN lower(t.title) LIKE :title_snippet ESCAPE '\\' THEN t.title END, CASE WHEN lower(COALESCE(t.description,'')) LIKE :description_snippet ESCAPE '\\' THEN t.description END, (SELECT c.body FROM task_comments c WHERE c.board_id=t.board_id AND c.task_id=t.id AND lower(c.body) LIKE :comment_snippet ESCAPE '\\' ORDER BY c.created_at ASC,c.id ASC LIMIT 1), (SELECT r.summary FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id AND lower(COALESCE(r.summary,'')) LIKE :run_summary_snippet ESCAPE '\\' ORDER BY r.started_at DESC,r.id ASC LIMIT 1), (SELECT r.error FROM task_runs r WHERE r.board_id=t.board_id AND r.task_id=t.id AND lower(COALESCE(r.error,'')) LIKE :run_error_snippet ESCAPE '\\' ORDER BY r.started_at DESC,r.id ASC LIMIT 1), (SELECT e.kind || ' ' || e.payload_json FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id AND (lower(e.kind) LIKE :event_kind_snippet ESCAPE '\\' OR lower(e.payload_json) LIKE :event_payload_snippet ESCAPE '\\') ORDER BY e.id ASC LIMIT 1))",
            "WHERE score > 0.0",
        )
    } else {
        ("0.0", "NULL", "")
    };
    params.push((":limit".to_owned(), Value::Integer(query.limit as i64)));
    params.push((":offset".to_owned(), Value::Integer(query.offset as i64)));
    let sql = format!(
        "SELECT task_id,seq,score,snippet FROM (SELECT t.id AS task_id,t.seq AS seq,({score_sql}) AS score,{snippet_sql} AS snippet,t.updated_at AS updated_at FROM tasks t WHERE {where_sql}) {outer_filter} ORDER BY score DESC,updated_at DESC,seq ASC LIMIT :limit OFFSET :offset"
    );
    let mut rows = connection.query(&sql, params).await?;
    let mut hits = Vec::new();
    while let Some(row) = rows.next().await? {
        hits.push(SearchHit {
            task_id: text_value(row.get_value(0)?, "tasks.id")?,
            seq: integer_value(row.get_value(1)?, "tasks.seq")?,
            score: match row.get_value(2)? {
                Value::Real(value) => value,
                Value::Integer(value) => value as f64,
                _ => 0.0,
            },
            snippet: optional_text_value(row.get_value(3)?, "search.snippet")?,
        });
    }
    Ok(hits)
}

fn search_filters(
    board: &BoardRef,
    query: &SearchQuery,
    fts: bool,
) -> (String, Vec<(String, Value)>) {
    let mut clauses = vec!["t.board_id=:board_id".to_owned()];
    let mut params = vec![(":board_id".to_owned(), Value::Text(board.id.clone()))];
    if fts {
        clauses.push("d.board_id=:board_id".to_owned());
    }
    if !query.include_archived {
        clauses.push("t.status != 'archived'".to_owned());
    }
    if !query.statuses.is_empty() {
        let names = query
            .statuses
            .iter()
            .enumerate()
            .map(|(index, _)| format!(":status_{index}"))
            .collect::<Vec<_>>();
        clauses.push(format!("t.status IN ({})", names.join(",")));
        params.extend(query.statuses.iter().enumerate().map(|(index, status)| {
            (
                format!(":status_{index}"),
                Value::Text(status.as_str().to_owned()),
            )
        }));
    }
    for (index, label) in query.labels.iter().enumerate() {
        let name = format!(":label_{index}");
        clauses.push(format!("EXISTS (SELECT 1 FROM task_labels tl JOIN labels l ON l.id=tl.label_id AND l.board_id=t.board_id WHERE tl.board_id=t.board_id AND tl.task_id=t.id AND (l.name={name} OR l.id={name}))"));
        params.push((name, Value::Text(label.clone())));
    }
    if let Some(assignee) = query.assignee.as_deref() {
        clauses.push("t.assignee=:assignee".to_owned());
        params.push((":assignee".to_owned(), Value::Text(assignee.to_owned())));
    }
    if !fts {
        if let Some((name, value)) = query
            .q
            .as_deref()
            .and_then(|value| exact_reference_clause(value, board))
        {
            if name == ":exact_seq" {
                clauses.push("t.seq=:exact_seq".to_owned());
            } else {
                clauses.push("t.id=:exact_task_id".to_owned());
            }
            params.push((name, value));
        }
    }
    (clauses.join(" AND "), params)
}

fn exact_reference_clause(value: &str, board: &BoardRef) -> Option<(String, Value)> {
    let value = value.trim();
    if value.starts_with("t_") && value.len() > 2 {
        return Some((":exact_task_id".to_owned(), Value::Text(value.to_owned())));
    }
    let (prefix, sequence) = value
        .split_once('#')
        .map_or((None, value), |(prefix, sequence)| (Some(prefix), sequence));
    let sequence = if let Some(prefix) = prefix {
        (prefix == board.id || prefix == board.slug).then_some(sequence)?
    } else {
        sequence.strip_prefix('#').unwrap_or(sequence)
    };
    sequence
        .parse::<i64>()
        .ok()
        .map(|value| (":exact_seq".to_owned(), Value::Integer(value)))
}

fn is_exact_reference(value: &str, board: &BoardRef) -> bool {
    exact_reference_clause(value, board).is_some()
}

fn like_literal(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| {
            if matches!(character, '%' | '_' | '\\') {
                vec!['\\', character]
            } else {
                vec![character]
            }
        })
        .collect()
}

fn fts_meta(board: &BoardRef, state: Option<&ProjectionState>, current_event: i64) -> SearchMeta {
    SearchMeta {
        backend: SEARCH_PROVIDER.to_owned(),
        stale: false,
        database_instance_id: None,
        protocol_version: Some(2),
        generation: state.and_then(|value| value.active_generation.clone()),
        resolved_board_id: board.id.clone(),
        fallback_reason: None,
        index_version: Some(SEARCH_INDEX_VERSION.to_owned()),
        last_event_id: Some(current_event),
        index_lag_events: Some(0),
    }
}

fn canonical_meta(
    board: &BoardRef,
    state: Option<&ProjectionState>,
    current_event: i64,
    stale: bool,
    fallback_reason: Option<String>,
) -> SearchMeta {
    let indexed = state.map(|value| value.last_event_id);
    SearchMeta {
        backend: "canonical".to_owned(),
        stale,
        database_instance_id: None,
        protocol_version: Some(2),
        generation: state.and_then(|value| value.active_generation.clone()),
        resolved_board_id: board.id.clone(),
        fallback_reason,
        index_version: None,
        last_event_id: Some(current_event),
        index_lag_events: indexed.map(|value| current_event.saturating_sub(value)),
    }
}

async fn set_projection_building(
    transaction: &turso::transaction::Transaction<'_>,
    generation: &str,
    now: i64,
) -> Result<(), StoreError> {
    transaction
        .execute(
            "UPDATE projection_state SET lifecycle_status='rebuilding', building_generation=:generation, building_fingerprint=:fingerprint, dirty=1, last_error=NULL, updated_at=:now WHERE projection='fts'",
            vec![
                (":generation".to_owned(), Value::Text(generation.to_owned())),
                (":fingerprint".to_owned(), Value::Text(SEARCH_PROVIDER_FINGERPRINT.to_owned())),
                (":now".to_owned(), Value::Integer(now)),
            ],
        )
        .await?;
    Ok(())
}

async fn mark_projection_error(connection: &Connection, message: &str) -> Result<(), StoreError> {
    connection
        .execute(
            "UPDATE projection_state SET lifecycle_status='degraded', dirty=1, last_error=:error, updated_at=:now WHERE projection='fts'",
            vec![
                (":error".to_owned(), Value::Text(message.to_owned())),
                (":now".to_owned(), Value::Integer(now_ms())),
            ],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{create_input, integer_value, store};

    #[tokio::test]
    async fn rebuilds_turso_fts_and_falls_back_for_exact_references() {
        let (_directory, store, _path) = store("search-projection").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_search_one", None, "Fix FTS indexing"),
            )
            .await
            .expect("create searchable task");
        store
            .create_task(
                "default",
                create_input("t_search_two", None, "Unrelated task"),
            )
            .await
            .expect("create second task");

        let before = store
            .search_index_status("default")
            .await
            .expect("status before rebuild");
        assert!(before.stale);

        let ready = store
            .rebuild_search_index("default")
            .await
            .expect("rebuild search index");
        assert_eq!(ready.backend, SEARCH_PROVIDER);
        assert!(!ready.stale);

        let result = store
            .search_tasks(SearchQuery {
                board: "default".to_owned(),
                q: Some("fts".to_owned()),
                statuses: Vec::new(),
                labels: Vec::new(),
                assignee: None,
                include_archived: false,
                limit: 20,
                offset: 0,
            })
            .await
            .expect("fts search");
        assert_eq!(result.meta.backend, SEARCH_PROVIDER);
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].task_id, "t_search_one");
        assert!(
            result.hits[0]
                .snippet
                .as_deref()
                .is_some_and(|value| { value.contains("<mark>") && value.contains("</mark>") })
        );

        let exact = store
            .search_tasks(SearchQuery {
                board: "default".to_owned(),
                q: Some("default#1".to_owned()),
                statuses: Vec::new(),
                labels: Vec::new(),
                assignee: None,
                include_archived: false,
                limit: 20,
                offset: 0,
            })
            .await
            .expect("exact reference search");
        assert_eq!(exact.meta.backend, "canonical");
        assert_eq!(
            exact.meta.fallback_reason.as_deref(),
            Some("exact_task_reference")
        );
        assert_eq!(exact.hits[0].task_id, "t_search_one");

        store
            .create_task(
                "default",
                create_input("t_search_three", None, "New FTS task"),
            )
            .await
            .expect("create task after rebuild");
        let stale = store
            .search_tasks(SearchQuery {
                board: "default".to_owned(),
                q: Some("new".to_owned()),
                statuses: Vec::new(),
                labels: Vec::new(),
                assignee: None,
                include_archived: false,
                limit: 20,
                offset: 0,
            })
            .await
            .expect("canonical fallback search");
        assert_eq!(stale.meta.backend, "canonical");
        assert!(stale.meta.stale);
        assert_eq!(stale.hits[0].task_id, "t_search_three");

        let resynced = store
            .sync_search_index("default")
            .await
            .expect("sync pending projection");
        assert_eq!(resynced.backend, SEARCH_PROVIDER);
        assert!(!resynced.stale);

        let connection = store.connection().await.expect("connection");
        let mut rows = connection
            .query(
                "SELECT COUNT(*) FROM projection_jobs WHERE target='fts' AND status='done'",
                (),
            )
            .await
            .expect("projection jobs query");
        let row = rows.next().await.expect("projection row").expect("row");
        assert!(integer_value(row.get_value(0).expect("count"), "count").expect("count") >= 2);
    }
}
