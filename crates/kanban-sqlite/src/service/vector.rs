use super::*;

#[cfg(feature = "vector-lancedb")]
pub fn vector_store_status(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    let path_ref = path.as_ref();
    let conn = connect_file(path_ref)?;
    let board_id = board_id(&conn, board)?;
    vector_store_status_without_provider(&conn, &board_id)
}

#[cfg(not(feature = "vector-lancedb"))]
pub fn vector_store_status(_path: impl AsRef<Path>, _board: &str) -> Result<VectorStoreStatus> {
    Ok(kanban_vector::DisabledVectorStore.status())
}

#[cfg(feature = "vector-lancedb")]
pub fn rebuild_vector_store(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    let path_ref = path.as_ref();
    let store = LanceDbStore::connect(LanceDbConfig::degraded(vector_store_path(path_ref)))
        .map_err(vector_storage)?;
    rebuild_vector_store_with(path_ref, board, &store)
}

#[cfg(not(feature = "vector-lancedb"))]
pub fn rebuild_vector_store(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    vector_store_status(path, board)
}

#[cfg(feature = "vector-lancedb")]
pub fn sync_vector_store(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    let path_ref = path.as_ref();
    let store = LanceDbStore::connect(LanceDbConfig::degraded(vector_store_path(path_ref)))
        .map_err(vector_storage)?;
    sync_vector_store_with(path_ref, board, &store)
}

#[cfg(not(feature = "vector-lancedb"))]
pub fn sync_vector_store(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    vector_store_status(path, board)
}

pub fn rebuild_vector_store_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl VectorStore,
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let chunks = vector_chunks_for_board(&conn, &board_id, store.chunk_embedding_model())?;
    match store
        .delete_board(&board_id)
        .and_then(|()| store.upsert(&chunks))
    {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_derived_store_success(
                &conn,
                LANCEDB_CHUNKS_STORE,
                &board_id,
                last_event_id,
                true,
                now,
            )?;
            let derived = derived_status_by_name(&conn, LANCEDB_CHUNKS_STORE)?;
            let mut status = store.status();
            status.message = format!(
                "{}; rebuilt {} chunk(s); derived_dirty={} last_event_id={}",
                status.message,
                chunks.len(),
                derived.dirty,
                derived.last_event_id
            );
            Ok(status)
        }
        Err(error) => {
            mark_derived_store_failure(
                &conn,
                LANCEDB_CHUNKS_STORE,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(vector_storage(error))
        }
    }
}

pub fn sync_vector_store_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl VectorStore,
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let last_event_id = current_last_event_id(&conn, &board_id)?;
    let state = derived_status_by_name(&conn, LANCEDB_CHUNKS_STORE)?;
    if !has_pending_vector_outbox_for_board(&conn, &board_id, last_event_id)? {
        return vector_store_status_with(&conn, &board_id, store);
    }
    let jobs = pending_vector_outbox_for_board(&conn, &board_id, last_event_id)?;
    let full_rebuild = state.last_event_id == 0 || jobs.iter().any(|job| job.action == "rebuild");
    let chunks = if state.last_event_id == 0 || jobs.iter().any(|job| job.action == "rebuild") {
        vector_chunks_for_board(&conn, &board_id, store.chunk_embedding_model())?
    } else {
        let entity_uris = jobs
            .iter()
            .map(|job| job.entity_uri.clone())
            .collect::<Vec<_>>();
        vector_chunks_for_entity_uris(
            &conn,
            &board_id,
            &entity_uris,
            store.chunk_embedding_model(),
        )?
    };
    let entity_uris = if full_rebuild {
        Vec::new()
    } else {
        let mut entity_uris = jobs
            .iter()
            .map(|job| job.entity_uri.clone())
            .collect::<Vec<_>>();
        entity_uris.sort();
        entity_uris.dedup();
        entity_uris
    };
    let write_result = if full_rebuild {
        store.delete_board(&board_id)
    } else {
        store.delete_entities(&entity_uris)
    }
    .and_then(|()| store.upsert(&chunks));
    match write_result {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_derived_store_success(
                &conn,
                LANCEDB_CHUNKS_STORE,
                &board_id,
                last_event_id,
                false,
                now,
            )?;
            let derived = derived_status_by_name(&conn, LANCEDB_CHUNKS_STORE)?;
            let mut status = store.status();
            status.message = format!(
                "{}; synced {} chunk(s) from {} job(s); derived_dirty={} last_event_id={}",
                status.message,
                chunks.len(),
                jobs.len(),
                derived.dirty,
                derived.last_event_id
            );
            Ok(status)
        }
        Err(error) => {
            mark_derived_store_failure(
                &conn,
                LANCEDB_CHUNKS_STORE,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(vector_storage(error))
        }
    }
}

pub(crate) fn vector_store_status_with(
    conn: &Connection,
    board_id: &str,
    store: &(impl VectorStore + ?Sized),
) -> Result<VectorStoreStatus> {
    vector_store_status_from_base(conn, board_id, store.status())
}

#[cfg(feature = "vector-lancedb")]
fn vector_store_status_without_provider(
    conn: &Connection,
    board_id: &str,
) -> Result<VectorStoreStatus> {
    vector_store_status_from_base(
        conn,
        board_id,
        VectorStoreStatus {
            backend: "lancedb".to_owned(),
            enabled: false,
            message: "LanceDB configured without an embedding provider; vector retrieval degraded"
                .to_owned(),
        },
    )
}

fn vector_store_status_from_base(
    conn: &Connection,
    board_id: &str,
    mut status: VectorStoreStatus,
) -> Result<VectorStoreStatus> {
    let state = derived_status_by_name(conn, LANCEDB_CHUNKS_STORE)?;
    let current_last_event_id = current_last_event_id(conn, board_id)?;
    let board_has_pending =
        has_pending_vector_outbox_for_board(conn, board_id, current_last_event_id)?;
    let lag = if board_has_pending {
        search_lag(current_last_event_id, Some(state.last_event_id))
    } else {
        0
    };
    status.message = format!(
        "{}; dirty={} last_event_id={} lag={} last_error={}",
        status.message,
        state.dirty,
        state.last_event_id,
        lag,
        state.last_error.as_deref().unwrap_or("none")
    );
    Ok(status)
}

#[cfg(any(feature = "graph-oxigraph", feature = "vector-lancedb"))]
pub(crate) fn has_pending_outbox_for_target(
    conn: &Connection,
    target: OutboxTarget,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<bool> {
    let target = target.as_str();
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN (?1, 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?2 \
               AND e.id <= ?3 \
         )",
        params![target, board_id, last_event_id.unwrap_or(i64::MAX)],
        |row| row.get::<_, bool>(0),
    )
    .map_err(storage)
}

fn pending_vector_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<Vec<IndexOutboxRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT o.id,o.source_event_id,o.target,o.entity_uri,o.action,o.payload_json,o.status,o.attempts,o.last_error,o.created_at,o.updated_at \
             FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN ('lancedb', 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?1 \
               AND e.id <= ?2 \
             ORDER BY o.id ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(
            params![board_id, last_event_id.unwrap_or(i64::MAX)],
            outbox_from_row,
        )
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn has_pending_vector_outbox_for_board(
    conn: &Connection,
    board_id: &str,
    last_event_id: Option<i64>,
) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS( \
             SELECT 1 FROM index_outbox o \
             JOIN task_events e ON e.id=o.source_event_id \
             WHERE o.target IN ('lancedb', 'all') \
               AND o.status IN ('pending', 'running', 'failed') \
               AND e.board_id=?1 \
               AND e.id <= ?2 \
         )",
        params![board_id, last_event_id.unwrap_or(i64::MAX)],
        |row| row.get::<_, bool>(0),
    )
    .map_err(storage)
}

fn vector_chunks_for_board(
    conn: &Connection,
    board_id: &str,
    embedding_model: &str,
) -> Result<Vec<kanban_vector::EmbeddingChunk>> {
    let mut stmt = conn
        .prepare(
            "SELECT 'kb://task/' || t.id,t.board_id,t.id,t.title,t.description,\
                    (SELECT MAX(e.id) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id),\
                    t.created_at,t.updated_at \
             FROM tasks t WHERE t.board_id=?1 AND t.archived_at IS NULL ORDER BY t.seq ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], task_chunk_source_from_row)
        .map_err(storage)?;
    let sources = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    build_vector_chunks(&sources, embedding_model)
}

fn vector_chunks_for_entity_uris(
    conn: &Connection,
    board_id: &str,
    entity_uris: &[String],
    embedding_model: &str,
) -> Result<Vec<kanban_vector::EmbeddingChunk>> {
    let mut task_ids = entity_uris
        .iter()
        .filter_map(|uri| uri.strip_prefix("kb://task/").map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    task_ids.sort();
    task_ids.dedup();
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", task_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT 'kb://task/' || t.id,t.board_id,t.id,t.title,t.description,\
                (SELECT MAX(e.id) FROM task_events e WHERE e.board_id=t.board_id AND e.task_id=t.id),\
                t.created_at,t.updated_at \
         FROM tasks t WHERE t.board_id=? AND t.archived_at IS NULL AND t.id IN ({placeholders}) ORDER BY t.seq ASC"
    );
    let mut params = vec![Value::Text(board_id.to_owned())];
    params.extend(task_ids.into_iter().map(Value::Text));
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter()), task_chunk_source_from_row)
        .map_err(storage)?;
    let sources = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    build_vector_chunks(&sources, embedding_model)
}

fn task_chunk_source_from_row(row: &Row<'_>) -> rusqlite::Result<TaskChunkSource> {
    Ok(TaskChunkSource {
        task_uri: row.get(0)?,
        project_id: None,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        source_event_id: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn build_vector_chunks(
    sources: &[TaskChunkSource],
    embedding_model: &str,
) -> Result<Vec<kanban_vector::EmbeddingChunk>> {
    let builder = ChunkBuilder::new(embedding_model);
    let mut chunks = Vec::new();
    for source in sources {
        chunks.extend(builder.build_task_chunks(source).map_err(vector_storage)?);
    }
    Ok(chunks)
}

#[cfg(feature = "vector-lancedb")]
pub(crate) fn vector_store_path(db_path: &Path) -> PathBuf {
    kanban_local::vector_store_path(db_path.to_path_buf())
}

pub(crate) fn vector_storage(error: impl std::fmt::Display) -> KanbanError {
    KanbanError::Storage(error.to_string())
}
