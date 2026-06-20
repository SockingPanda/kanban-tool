pub use std::{
    path::Path,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

pub use anyhow::Context;
pub use kanban_core::{KanbanError, TaskStatus, new_run_id};
pub use kanban_sqlite::{
    BoardListOptions, BootstrapTaskLabel, CreateBoard, CreateComment, CreateLabel, CreateTask,
    DispatchOptions, FinishPolicy, LabelOntologyActionInput, LabelOntologyActionType,
    LabelOntologyActor, LabelOntologyAtomApplyInput, LabelOntologyCandidateAtomInput,
    LabelOntologyProposedAction, LabelOntologyQualityOptions, LabelOntologyRecordInput,
    LabelOntologyRetargetOptions, LabelOntologyRevertInput, LabelOntologyReviewGroupBy,
    LabelOntologyReviewOptions, LabelOntologySignalInput, LabelOntologySignalKind,
    LabelOntologySignalListOptions, LabelOntologySignalStatus, LabelOntologyStructurePlanInput,
    LabelOntologySuggestState, LabelOntologyValidationEffectiveOutcome,
    LabelOntologyValidationInput, LabelOntologyValidationStatus, LabelProposalCandidate,
    LabelProposalCreateOptions, LabelProposalDecisionOptions, LabelProposalListOptions,
    LabelProposalProposeOptions, LabelProposalStatus, ManualLabelProposalProvider, TaskPatch,
    TaskRecord, UpsertLabelSemantics, accept_label_proposal, accept_label_proposal_with_options,
    add_dependency, apply_label_ontology_atom, apply_label_ontology_atom_with_options,
    archive_board, archive_task, begin_database_replace, begin_database_runtime, block_task,
    bootstrap_task_label, build_context_pack, claim_task, clear_label_semantics_with_options,
    complete_task, connect_file, create_board, create_comment, create_comment_with_options,
    create_label, create_label_ontology_action, create_task, delete_label, derived_store_statuses,
    dispatch_once, doctor_database, explain_label_atom, export_jsonl, get_board,
    get_label_ontology_signal, get_label_proposal, get_label_semantics, get_run_by_id_global,
    get_task, import_jsonl, init_database, label_ontology_quality_report, list_board_columns,
    list_boards, list_comments, list_dependencies, list_events, list_label_atoms,
    list_label_ontology_signals, list_label_proposals, list_labels, list_outbox, list_runs,
    list_tasks, plan_label_ontology_structure_change, promote_task, propose_task_label,
    propose_task_label_with, propose_task_label_with_store,
    propose_task_label_with_store_and_create_options, record_label_ontology_observation,
    reject_label_proposal, revert_label_ontology_mutation, review_label_ontology, search_tasks,
    set_task_retry_policy_by_id, specify_task, submit_review_task, task_ontology_summary,
    unblock_task, update_task, upsert_label_semantics, validate_label_ontology_action,
};
#[cfg(feature = "vector-lancedb")]
pub use kanban_sqlite::{
    LabelOntologyTrustedValidationInput, LabelSuggestionOptions, label_atom_index_status_with,
    query_label_atom_index_by_vector_with, query_label_atom_index_with,
    rebuild_label_atom_index_with, rebuild_vector_store_with, sync_vector_store_with,
    validate_label_ontology_action_with_trusted_suggestions,
};
#[cfg(feature = "vector-lancedb")]
pub use kanban_vector::{
    ChunkVectorStore, EmbeddingChunk, LabelAtomHit, LabelAtomQuery, LabelAtomVector,
    LabelAtomVectorHit, LabelAtomVectorQuery, LabelAtomVectorStore, QueryEmbeddingProvider,
    VectorError, VectorHit, VectorQuery, VectorStoreBackend, VectorStoreStatus,
};
pub use rusqlite::{Connection, params};

pub fn test_error(message: impl Into<String>) -> anyhow::Error {
    anyhow::anyhow!(message.into())
}

pub fn join_thread<T>(handle: thread::JoinHandle<T>) -> anyhow::Result<T> {
    handle
        .join()
        .map_err(|panic| test_error(format!("test thread panicked: {panic:?}")))
}

pub fn result_err<T, E>(result: Result<T, E>) -> anyhow::Result<E>
where
    T: std::fmt::Debug,
{
    match result {
        Ok(value) => Err(test_error(format!("expected error, got Ok({value:?})"))),
        Err(error) => Ok(error),
    }
}

pub struct TempDb {
    _temp_dir: tempfile::TempDir,
    pub dir: std::path::PathBuf,
    pub path: std::path::PathBuf,
}

impl TempDb {
    pub fn new(name: &str) -> anyhow::Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix(&format!("kb-sqlite-all-{name}-"))
            .tempdir()?;
        let path = dir.path().join("kb.db");
        let dir_path = dir.path().to_path_buf();
        Ok(Self {
            _temp_dir: dir,
            dir: dir_path,
            path,
        })
    }
}

impl AsRef<Path> for TempDb {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

#[cfg(any(
    feature = "graph-oxigraph",
    feature = "tantivy-backend",
    feature = "vector-lancedb"
))]
pub fn insert_board(path: &Path, slug: &str, id: &str) -> anyhow::Result<()> {
    connect_file(path)?
        .execute(
            "INSERT INTO boards(id, slug, name, description, created_at, updated_at, archived_at) VALUES (?1, ?2, ?3, NULL, 1, 1, NULL)",
            params![id, slug, slug],
        )?;
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[derive(Default)]
pub struct RecordingVectorStore {
    embedding_model: Option<String>,
    live_chunks: std::sync::Mutex<Vec<EmbeddingChunk>>,
    live_label_atoms: std::sync::Mutex<Vec<LabelAtomVector>>,
    upserted: std::sync::Mutex<Vec<String>>,
    upserted_label_atoms: std::sync::Mutex<Vec<LabelAtomVector>>,
    label_atom_vector_queries: std::sync::Mutex<Vec<LabelAtomVectorQuery>>,
    upserted_models: std::sync::Mutex<Vec<String>>,
    deleted: std::sync::Mutex<Vec<String>>,
    deleted_boards: std::sync::Mutex<Vec<String>>,
}

#[cfg(feature = "vector-lancedb")]
impl RecordingVectorStore {
    pub fn with_embedding_model(embedding_model: &str) -> Self {
        Self {
            embedding_model: Some(embedding_model.to_owned()),
            ..Self::default()
        }
    }

    pub fn expected_model(&self) -> &str {
        self.embedding_model
            .as_deref()
            .unwrap_or(kanban_vector::DEFAULT_EMBEDDING_MODEL)
    }

    pub fn upserted_texts(&self) -> anyhow::Result<Vec<String>> {
        Ok(self
            .upserted
            .lock()
            .map_err(|err| test_error(format!("upserted mutex poisoned: {err}")))?
            .clone())
    }

    pub fn upserted_models(&self) -> anyhow::Result<Vec<String>> {
        Ok(self
            .upserted_models
            .lock()
            .map_err(|err| test_error(format!("upserted_models mutex poisoned: {err}")))?
            .clone())
    }

    pub fn deleted_entity_uris(&self) -> anyhow::Result<Vec<String>> {
        Ok(self
            .deleted
            .lock()
            .map_err(|err| test_error(format!("deleted mutex poisoned: {err}")))?
            .clone())
    }

    pub fn deleted_board_ids(&self) -> anyhow::Result<Vec<String>> {
        Ok(self
            .deleted_boards
            .lock()
            .map_err(|err| test_error(format!("deleted_boards mutex poisoned: {err}")))?
            .clone())
    }

    pub fn live_texts(&self) -> anyhow::Result<Vec<String>> {
        Ok(self
            .live_chunks
            .lock()
            .map_err(|err| test_error(format!("live_chunks mutex poisoned: {err}")))?
            .iter()
            .map(|chunk| chunk.text.clone())
            .collect())
    }

    pub fn upserted_label_atoms(&self) -> anyhow::Result<Vec<LabelAtomVector>> {
        Ok(self
            .upserted_label_atoms
            .lock()
            .map_err(|err| test_error(format!("upserted_label_atoms mutex poisoned: {err}")))?
            .clone())
    }

    pub fn label_atom_vector_queries(&self) -> anyhow::Result<Vec<LabelAtomVectorQuery>> {
        Ok(self
            .label_atom_vector_queries
            .lock()
            .map_err(|err| test_error(format!("label_atom_vector_queries mutex poisoned: {err}")))?
            .clone())
    }
}

#[cfg(feature = "vector-lancedb")]
impl VectorStoreBackend for RecordingVectorStore {
    fn embedding_model(&self) -> &str {
        self.expected_model()
    }

    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus::new("test-vector", true, "test vector store")
    }
}

#[cfg(feature = "vector-lancedb")]
impl QueryEmbeddingProvider for RecordingVectorStore {
    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
        Ok(vec![1.0, 1.0, 1.0])
    }
}

#[cfg(feature = "vector-lancedb")]
impl ChunkVectorStore for RecordingVectorStore {
    fn upsert(&self, chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        if let Some(chunk) = chunks
            .iter()
            .find(|chunk| chunk.embedding_model != self.expected_model())
        {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: self.expected_model().to_owned(),
                actual: chunk.embedding_model.clone(),
            });
        }
        let mut upserted = self
            .upserted
            .lock()
            .map_err(|err| VectorError::Store(format!("upserted mutex poisoned: {err}")))?;
        upserted.extend(chunks.iter().map(|chunk| chunk.text.clone()));
        let mut upserted_models = self
            .upserted_models
            .lock()
            .map_err(|err| VectorError::Store(format!("upserted_models mutex poisoned: {err}")))?;
        upserted_models.extend(chunks.iter().map(|chunk| chunk.embedding_model.clone()));
        let mut live_chunks = self
            .live_chunks
            .lock()
            .map_err(|err| VectorError::Store(format!("live_chunks mutex poisoned: {err}")))?;
        for chunk in chunks {
            live_chunks.retain(|live| live.chunk_key() != chunk.chunk_key());
            live_chunks.push(chunk.clone());
        }
        Ok(())
    }

    fn delete_board(&self, board_id: &str) -> Result<(), VectorError> {
        let mut deleted_boards = self
            .deleted_boards
            .lock()
            .map_err(|err| VectorError::Store(format!("deleted_boards mutex poisoned: {err}")))?;
        deleted_boards.push(board_id.to_owned());
        self.live_chunks
            .lock()
            .map_err(|err| VectorError::Store(format!("live_chunks mutex poisoned: {err}")))?
            .retain(|chunk| chunk.board_id.as_deref() != Some(board_id));
        Ok(())
    }

    fn delete_entities(&self, entity_uris: &[String]) -> Result<(), VectorError> {
        let mut deleted = self
            .deleted
            .lock()
            .map_err(|err| VectorError::Store(format!("deleted mutex poisoned: {err}")))?;
        deleted.extend(entity_uris.iter().cloned());
        self.live_chunks
            .lock()
            .map_err(|err| VectorError::Store(format!("live_chunks mutex poisoned: {err}")))?
            .retain(|chunk| {
                !entity_uris
                    .iter()
                    .any(|entity_uri| entity_uri == chunk.chunk.entity_uri.as_str())
            });
        Ok(())
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

#[cfg(feature = "vector-lancedb")]
impl LabelAtomVectorStore for RecordingVectorStore {
    fn delete_label_atoms_for_board(&self, board_id: &str) -> Result<(), VectorError> {
        self.live_label_atoms
            .lock()
            .map_err(|err| VectorError::Store(format!("live_label_atoms mutex poisoned: {err}")))?
            .retain(|atom| atom.board_id != board_id);
        Ok(())
    }

    fn upsert_label_atoms(&self, atoms: &[LabelAtomVector]) -> Result<(), VectorError> {
        if let Some(atom) = atoms
            .iter()
            .find(|atom| atom.embedding_model != self.expected_model())
        {
            return Err(VectorError::EmbeddingModelMismatch {
                expected: self.expected_model().to_owned(),
                actual: atom.embedding_model.clone(),
            });
        }
        self.upserted_label_atoms
            .lock()
            .map_err(|err| {
                VectorError::Store(format!("upserted_label_atoms mutex poisoned: {err}"))
            })?
            .extend(atoms.iter().cloned());
        let mut live = self
            .live_label_atoms
            .lock()
            .map_err(|err| VectorError::Store(format!("live_label_atoms mutex poisoned: {err}")))?;
        for atom in atoms {
            live.retain(|existing| existing.atom_key() != atom.atom_key());
            live.push(atom.clone());
        }
        Ok(())
    }

    fn query_label_atoms(&self, query: &LabelAtomQuery) -> Result<Vec<LabelAtomHit>, VectorError> {
        let atoms = self
            .live_label_atoms
            .lock()
            .map_err(|err| VectorError::Store(format!("live_label_atoms mutex poisoned: {err}")))?
            .clone();
        Ok(atoms
            .into_iter()
            .filter(|atom| {
                query
                    .board_id
                    .as_ref()
                    .is_none_or(|board_id| &atom.board_id == board_id)
                    && query
                        .embedding_model
                        .as_ref()
                        .is_none_or(|model| &atom.embedding_model == model)
                    && query
                        .polarity
                        .as_ref()
                        .is_none_or(|polarity| &atom.polarity == polarity)
            })
            .take(query.limit)
            .map(|atom| LabelAtomHit {
                atom_id: atom.atom_id,
                label_id: atom.label_id,
                label_name: atom.label_name,
                board_id: atom.board_id,
                polarity: atom.polarity,
                kind: atom.kind,
                text: atom.text,
                ordinal: atom.ordinal,
                content_hash: atom.content_hash,
                embedding_model: atom.embedding_model,
                distance: 1.0,
            })
            .collect())
    }

    fn query_label_atoms_by_vector(
        &self,
        query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        self.label_atom_vector_queries
            .lock()
            .map_err(|err| {
                VectorError::Store(format!("label_atom_vector_queries mutex poisoned: {err}"))
            })?
            .push(query.clone());
        let atoms = self
            .live_label_atoms
            .lock()
            .map_err(|err| VectorError::Store(format!("live_label_atoms mutex poisoned: {err}")))?
            .clone();
        Ok(atoms
            .into_iter()
            .filter(|atom| {
                query
                    .board_id
                    .as_ref()
                    .is_none_or(|board_id| &atom.board_id == board_id)
                    && query
                        .embedding_model
                        .as_ref()
                        .is_none_or(|model| &atom.embedding_model == model)
                    && query
                        .polarity
                        .as_ref()
                        .is_none_or(|polarity| &atom.polarity == polarity)
            })
            .take(query.limit)
            .map(|atom| {
                let vector = query.include_vector.then(|| vector_for_label_atom(&atom));
                LabelAtomVectorHit {
                    hit: LabelAtomHit {
                        atom_id: atom.atom_id,
                        label_id: atom.label_id,
                        label_name: atom.label_name,
                        board_id: atom.board_id,
                        polarity: atom.polarity,
                        kind: atom.kind,
                        text: atom.text,
                        ordinal: atom.ordinal,
                        content_hash: atom.content_hash,
                        embedding_model: atom.embedding_model,
                        distance: 1.0,
                    },
                    vector,
                }
            })
            .collect())
    }
}

#[cfg(feature = "vector-lancedb")]
pub struct FailingVectorStore;

#[cfg(feature = "vector-lancedb")]
impl VectorStoreBackend for FailingVectorStore {
    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus::new("test-vector", true, "test vector store")
    }
}

#[cfg(feature = "vector-lancedb")]
impl QueryEmbeddingProvider for FailingVectorStore {}

#[cfg(feature = "vector-lancedb")]
impl ChunkVectorStore for FailingVectorStore {
    fn upsert(&self, _chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        Err(VectorError::DimensionMismatch {
            expected: 3,
            actual: 2,
        })
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), VectorError> {
        Ok(())
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

#[cfg(feature = "vector-lancedb")]
impl LabelAtomVectorStore for FailingVectorStore {
    fn delete_label_atoms_for_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Ok(())
    }

    fn upsert_label_atoms(&self, _atoms: &[LabelAtomVector]) -> Result<(), VectorError> {
        Err(VectorError::DimensionMismatch {
            expected: 3,
            actual: 2,
        })
    }
}

#[cfg(feature = "vector-lancedb")]
pub struct QueryFailingVectorStore;

#[cfg(feature = "vector-lancedb")]
impl VectorStoreBackend for QueryFailingVectorStore {
    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus::new("test-vector", true, "test vector store")
    }
}

#[cfg(feature = "vector-lancedb")]
impl QueryEmbeddingProvider for QueryFailingVectorStore {
    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
        Ok(vec![1.0, 0.0, 0.0])
    }
}

#[cfg(feature = "vector-lancedb")]
impl ChunkVectorStore for QueryFailingVectorStore {
    fn upsert(&self, _chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        Ok(())
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), VectorError> {
        Ok(())
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        Err(VectorError::Store("query exploded".to_owned()))
    }
}

#[cfg(feature = "vector-lancedb")]
impl LabelAtomVectorStore for QueryFailingVectorStore {
    fn query_label_atoms_by_vector(
        &self,
        _query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        Err(VectorError::Store("query exploded".to_owned()))
    }
}

#[cfg(feature = "vector-lancedb")]
fn vector_for_label_atom(atom: &LabelAtomVector) -> Vec<f32> {
    let _ = atom;
    vec![1.0, 0.0, 0.0]
}

#[cfg(feature = "tantivy-backend")]
pub fn tantivy_outbox_statuses_for_board(
    path: &Path,
    board_slug: &str,
) -> anyhow::Result<Vec<String>> {
    let conn = connect_file(path)?;
    let mut stmt = conn
        .prepare(
            "SELECT io.status              FROM index_outbox io              JOIN task_events e ON e.id=io.source_event_id              JOIN boards b ON b.id=e.board_id              WHERE b.slug=?1 AND io.target='tantivy'              ORDER BY io.id ASC",
        )
        ?;
    Ok(stmt
        .query_map([board_slug], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

#[cfg(feature = "graph-oxigraph")]
pub fn graph_outbox_statuses_for_board(
    path: &Path,
    board_slug: &str,
) -> anyhow::Result<Vec<String>> {
    let conn = connect_file(path)?;
    let mut stmt = conn
        .prepare(
            "SELECT io.status              FROM index_outbox io              JOIN task_events e ON e.id=io.source_event_id              JOIN boards b ON b.id=e.board_id              WHERE b.slug=?1 AND io.target='oxigraph'              ORDER BY io.id ASC",
        )
        ?;
    Ok(stmt
        .query_map([board_slug], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

#[cfg(feature = "vector-lancedb")]
pub fn lancedb_outbox_statuses_for_board(
    path: &Path,
    board_slug: &str,
) -> anyhow::Result<Vec<String>> {
    let conn = connect_file(path)?;
    let mut stmt = conn
        .prepare(
            "SELECT io.status              FROM index_outbox io              JOIN task_events e ON e.id=io.source_event_id              JOIN boards b ON b.id=e.board_id              WHERE b.slug=?1 AND io.target='lancedb'              ORDER BY io.id ASC",
        )
        ?;
    Ok(stmt
        .query_map([board_slug], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn now_ms() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(error) => panic!("system clock is before unix epoch: {error}"),
    }
}

pub fn set_retry_policy(path: &Path, task_id: &str, max_retries: i64) -> anyhow::Result<()> {
    connect_file(path)?.execute(
        "UPDATE tasks SET max_retries=?1 WHERE id=?2",
        rusqlite::params![max_retries, task_id],
    )?;
    Ok(())
}

pub fn mark_task_running_in_current_tx(conn: &Connection, task_id: &str) -> anyhow::Result<()> {
    let run_id = new_run_id();
    let now = now_ms();
    let claim_token = format!("token-{task_id}");
    let (board_id, claim_owner): (String, String) = conn.query_row(
        "SELECT board_id, 'worker' FROM tasks WHERE id=?1",
        [task_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    conn.execute(
        "UPDATE tasks SET status='running', claim_token=?1, claim_owner=?2, claim_expires_at=?3, last_heartbeat_at=?4, started_at=COALESCE(started_at, ?4), current_run_id=?5, updated_at=?4, lock_version=lock_version+1 WHERE id=?6",
        params![claim_token, claim_owner, now + 300_000, now, run_id, task_id],
    )?;
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, worker_profile, claim_token, claim_owner, claim_expires_at, started_at, last_heartbeat_at, metadata_json) VALUES (?1, ?2, ?3, 'running', 'test', ?4, ?5, ?6, ?7, ?7, '{}')",
        params![run_id, board_id, task_id, claim_token, claim_owner, now + 300_000, now],
    )?;
    Ok(())
}
