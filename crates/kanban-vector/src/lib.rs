use kanban_contract::{
    ProjectionArtifactManifest, ProjectionCorpusMetadata, ProjectionStoreDescriptor,
    VectorHelperEmbedQueryResponse, VectorHelperErrorResponse, VectorHelperLabelAtomHit,
    VectorHelperQueryChunksResponse, VectorHelperQueryLabelAtomsItem,
    VectorHelperQueryLabelAtomsResponse, VectorHelperStatusResponse,
    VectorProjectionApplyBatchRequest, VectorProjectionBuildingPhase,
    VectorProjectionDestructiveAuthority, VectorProjectionGenerationBinding,
    VectorProjectionGenerationRole, VectorProjectionHelperDescriptor, VectorProjectionHelperError,
    VectorProjectionHelperErrorKind, VectorProjectionHelperRequest, VectorProjectionHelperResponse,
    VectorProjectionMutationAck, VectorProjectionMutationContext, VectorProjectionProtectionReason,
};
use kanban_entity::{ChunkRef, EntityUri};
use kanban_helper_protocol::HelperEnvelope;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

pub const DEFAULT_EMBEDDING_MODEL: &str = "kb-local-default";
pub const TASK_CHUNKS_CORPUS_SCHEMA: &str = "task-chunks-v2";
pub const LABEL_ATOMS_CORPUS_SCHEMA: &str = "label-atoms-v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorStoreStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub dirty: Option<bool>,
    #[serde(default)]
    pub board_dirty: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<i64>,
}

impl VectorStoreStatus {
    pub fn new(backend: impl Into<String>, enabled: bool, message: impl Into<String>) -> Self {
        Self {
            backend: backend.into(),
            enabled,
            message: message.into(),
            diagnostics: Vec::new(),
            dirty: None,
            board_dirty: None,
            generation: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingChunk {
    pub chunk: ChunkRef,
    pub kind: String,
    pub project_id: Option<String>,
    pub board_id: Option<String>,
    pub task_id: Option<String>,
    pub source_table: String,
    pub source_id: String,
    pub text: String,
    pub summary: Option<String>,
    pub embedding_model: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub source_event_id: Option<i64>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelAtomVector {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub board_id: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub embedding_model: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl LabelAtomVector {
    pub fn atom_key(&self) -> String {
        atom_key(&self.atom_id, &self.embedding_model)
    }
}

pub fn atom_key(atom_id: &str, embedding_model: &str) -> String {
    format!("{atom_id}#{embedding_model}")
}

impl EmbeddingChunk {
    pub fn chunk_key(&self) -> String {
        chunk_key(self.chunk.uri.as_str(), &self.embedding_model)
    }
}

pub fn chunk_key(chunk_uri: &str, embedding_model: &str) -> String {
    format!("{chunk_uri}#{embedding_model}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskChunkSource {
    pub task_uri: String,
    pub project_id: Option<String>,
    pub board_id: Option<String>,
    pub task_id: String,
    pub title: String,
    pub description: Option<String>,
    pub comments: String,
    pub run_text: String,
    pub event_text: String,
    pub source_event_id: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkBuilder {
    embedding_model: String,
}

impl ChunkBuilder {
    pub fn new(embedding_model: impl Into<String>) -> Self {
        Self {
            embedding_model: embedding_model.into(),
        }
    }

    pub fn build_task_chunks(
        &self,
        task: &TaskChunkSource,
    ) -> Result<Vec<EmbeddingChunk>, VectorError> {
        let task_uri = kanban_entity::EntityUri::new(task.task_uri.clone())
            .map_err(|err| VectorError::Chunk(err.to_string()))?;
        let task_text = match task.description.as_deref().map(str::trim) {
            Some(description) if !description.is_empty() => {
                format!("{}\n\n{}", task.title.trim(), description)
            }
            _ => task.title.trim().to_owned(),
        };
        let sections = [
            (0_i64, "task", "tasks", task_text),
            (
                1_i64,
                "task_comments",
                "task_comments",
                task.comments.trim().to_owned(),
            ),
            (
                2_i64,
                "task_runs",
                "task_runs",
                task.run_text.trim().to_owned(),
            ),
            (
                3_i64,
                "task_events",
                "task_events",
                task.event_text.trim().to_owned(),
            ),
        ];
        let mut chunks = Vec::with_capacity(sections.len());
        for (ordinal, kind, source_table, text) in sections {
            if text.is_empty() {
                continue;
            }
            let chunk_uri = kanban_entity::EntityUri::new(format!(
                "kb://chunk/task/{}/{ordinal}",
                task.task_id
            ))
            .map_err(|err| VectorError::Chunk(err.to_string()))?;
            chunks.push(EmbeddingChunk {
                chunk: ChunkRef {
                    uri: chunk_uri,
                    entity_uri: task_uri.clone(),
                    ordinal,
                    content_hash: Some(semantic_content_hash(&text)),
                },
                kind: kind.to_owned(),
                project_id: task.project_id.clone(),
                board_id: task.board_id.clone(),
                task_id: Some(task.task_id.clone()),
                source_table: source_table.to_owned(),
                source_id: task.task_id.clone(),
                text,
                summary: Some(task.title.trim().to_owned()),
                embedding_model: self.embedding_model.clone(),
                created_at: task.created_at,
                updated_at: task.updated_at,
                source_event_id: task.source_event_id,
                metadata_json: serde_json::json!({
                    "section": kind,
                    "updated_at": task.updated_at
                })
                .to_string(),
            });
        }
        Ok(chunks)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorQuery {
    pub text: String,
    pub limit: usize,
    pub board_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomQuery {
    pub text: String,
    pub limit: usize,
    pub board_id: Option<String>,
    pub embedding_model: Option<String>,
    pub polarity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomVectorQuery {
    pub vector: Vec<f32>,
    pub limit: usize,
    pub board_id: Option<String>,
    pub embedding_model: Option<String>,
    pub polarity: Option<String>,
    pub include_vector: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorHit {
    pub chunk: ChunkRef,
    pub score: f32,
    pub text: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomHit {
    pub atom_id: String,
    pub label_id: String,
    pub label_name: String,
    pub board_id: String,
    pub polarity: String,
    pub kind: String,
    pub text: String,
    pub ordinal: i64,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelAtomVectorHit {
    pub hit: LabelAtomHit,
    pub vector: Option<Vec<f32>>,
}

pub trait EmbeddingProvider {
    fn provider_name(&self) -> &str {
        "custom"
    }

    fn embedding_model(&self) -> &str;
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError>;

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, VectorError> {
        texts.iter().map(|text| self.embed(text)).collect()
    }

    fn provider_fingerprint(&self) -> String {
        embedding_provider_fingerprint(
            self.provider_name(),
            self.embedding_model(),
            self.dimensions(),
        )
    }
}

pub trait VectorStoreBackend {
    fn embedding_model(&self) -> &str {
        DEFAULT_EMBEDDING_MODEL
    }
    fn status(&self) -> VectorStoreStatus;
}

pub trait QueryEmbeddingProvider: VectorStoreBackend {
    fn embed_query_text(&self, _text: &str) -> Result<Vec<f32>, VectorError> {
        Err(VectorError::Disabled)
    }
}

pub trait ChunkVectorStore: VectorStoreBackend {
    fn chunk_embedding_model(&self) -> &str {
        self.embedding_model()
    }
    fn delete_board(&self, board_id: &str) -> Result<(), VectorError>;
    fn delete_entities(&self, entity_uris: &[String]) -> Result<(), VectorError>;
    fn upsert(&self, chunks: &[EmbeddingChunk]) -> Result<(), VectorError>;
    fn query(&self, query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError>;
}

pub trait LabelAtomVectorStore: QueryEmbeddingProvider {
    fn delete_label_atoms_for_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }
    fn upsert_label_atoms(&self, _atoms: &[LabelAtomVector]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }
    fn query_label_atoms(&self, _query: &LabelAtomQuery) -> Result<Vec<LabelAtomHit>, VectorError> {
        Ok(Vec::new())
    }
    fn query_label_atoms_by_vector(
        &self,
        _query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

pub trait VectorStore: ChunkVectorStore + LabelAtomVectorStore {}

impl<T> VectorStore for T where T: ChunkVectorStore + LabelAtomVectorStore + ?Sized {}

#[derive(Debug, Clone, Default)]
pub struct DisabledVectorStore;

impl VectorStoreBackend for DisabledVectorStore {
    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus::new(
            "disabled",
            false,
            "Vector store is disabled; context retrieval uses lexical fallback",
        )
    }
}

impl QueryEmbeddingProvider for DisabledVectorStore {}

impl ChunkVectorStore for DisabledVectorStore {
    fn upsert(&self, _chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

impl LabelAtomVectorStore for DisabledVectorStore {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VectorStatusScope {
    #[default]
    Chunks,
    LabelAtoms,
}

#[derive(Debug, Clone)]
pub struct SubprocessVectorStore {
    helper_path: PathBuf,
    db_path: PathBuf,
    board: String,
    vector_config_path: Option<PathBuf>,
    embedding_model: Option<String>,
    status_scope: VectorStatusScope,
}

impl SubprocessVectorStore {
    pub fn new(
        helper_path: impl Into<PathBuf>,
        db_path: impl Into<PathBuf>,
        board: impl Into<String>,
        vector_config_path: Option<PathBuf>,
    ) -> Self {
        Self {
            helper_path: helper_path.into(),
            db_path: db_path.into(),
            board: board.into(),
            vector_config_path,
            embedding_model: None,
            status_scope: VectorStatusScope::Chunks,
        }
    }

    pub fn with_embedding_model(mut self, embedding_model: impl Into<String>) -> Self {
        self.embedding_model = Some(embedding_model.into());
        self
    }

    pub fn with_status_scope(mut self, status_scope: VectorStatusScope) -> Self {
        self.status_scope = status_scope;
        self
    }

    fn status_command(&self) -> &'static str {
        match self.status_scope {
            VectorStatusScope::Chunks => "status",
            VectorStatusScope::LabelAtoms => "label-atoms-status",
        }
    }

    fn helper_args(&self, command_args: &[String]) -> Vec<String> {
        let mut args = command_args.to_vec();
        args.push("--db".to_owned());
        args.push(self.db_path.display().to_string());
        args.push("--board".to_owned());
        args.push(self.board.clone());
        if let Some(path) = &self.vector_config_path {
            args.push("--vector-config".to_owned());
            args.push(path.display().to_string());
        }
        args
    }

    fn run_helper<T>(&self, command_args: &[String]) -> Result<T, VectorError>
    where
        T: DeserializeOwned,
    {
        let args = self.helper_args(command_args);
        let output = Command::new(&self.helper_path)
            .args(&args)
            .output()
            .map_err(|error| {
                VectorError::Store(format!(
                    "vector helper unavailable: failed to run {}: {}",
                    self.helper_path.display(),
                    error
                ))
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() {
            if let Ok(envelope) = HelperEnvelope::from_json(stdout.trim())
                && let Ok(error) = envelope.decode::<VectorHelperErrorResponse>()
            {
                return Err(VectorError::Store(format!(
                    "vector helper failed: {} ({})",
                    error.message, error.code
                )));
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VectorError::Store(format!(
                "vector helper {} exited with status {:?}: {}",
                self.helper_path.display(),
                output.status.code(),
                bounded_helper_message(stderr.trim())
            )));
        }
        let envelope = HelperEnvelope::from_json(stdout.trim()).map_err(|error| {
            VectorError::Store(format!(
                "vector helper {} returned invalid JSON envelope: {}",
                self.helper_path.display(),
                error
            ))
        })?;
        envelope.decode::<T>().map_err(|error| {
            VectorError::Store(format!(
                "vector helper {} returned an invalid payload: {}",
                self.helper_path.display(),
                error
            ))
        })
    }

    pub fn label_atom_status(&self) -> VectorStoreStatus {
        match self
            .run_helper::<VectorHelperStatusResponse>(&["label-atoms-status".to_owned()])
            .map(vector_store_status)
        {
            Ok(status) => status,
            Err(error) => {
                let mut status = VectorStoreStatus::new(
                    "helper-missing",
                    false,
                    format!("vector helper unavailable: {error}"),
                );
                status.diagnostics.push("helper_missing".to_owned());
                status
                    .diagnostics
                    .push("label_atom_helper_missing".to_owned());
                status
            }
        }
    }

    pub fn rebuild_label_atoms(&self) -> Result<VectorStoreStatus, VectorError> {
        self.run_helper::<VectorHelperStatusResponse>(&["rebuild-label-atoms".to_owned()])
            .map(vector_store_status)
    }

    pub fn sync_label_atoms(&self) -> Result<VectorStoreStatus, VectorError> {
        self.run_helper::<VectorHelperStatusResponse>(&["sync-label-atoms".to_owned()])
            .map(vector_store_status)
    }
}

pub const DEFAULT_VECTOR_PROJECTION_HELPER_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VectorProjectionClientLimits {
    pub max_stdin_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for VectorProjectionClientLimits {
    fn default() -> Self {
        Self {
            max_stdin_bytes: 32 * 1024 * 1024,
            max_stdout_bytes: 2 * 1024 * 1024,
            max_stderr_bytes: 256 * 1024,
        }
    }
}

#[derive(Clone)]
pub struct SubprocessVectorProjectionClient {
    helper_path: PathBuf,
    db_path: PathBuf,
    vector_config_path: Option<PathBuf>,
    timeout: Duration,
    limits: VectorProjectionClientLimits,
}

impl fmt::Debug for SubprocessVectorProjectionClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SubprocessVectorProjectionClient")
            .field("helper_path", &self.helper_path)
            .field("has_vector_config", &self.vector_config_path.is_some())
            .field("timeout", &self.timeout)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl SubprocessVectorProjectionClient {
    pub fn new(
        helper_path: impl Into<PathBuf>,
        db_path: impl Into<PathBuf>,
        vector_config_path: Option<PathBuf>,
    ) -> Self {
        Self {
            helper_path: helper_path.into(),
            db_path: db_path.into(),
            vector_config_path,
            timeout: DEFAULT_VECTOR_PROJECTION_HELPER_TIMEOUT,
            limits: VectorProjectionClientLimits::default(),
        }
    }

    pub fn helper_path(&self) -> &Path {
        &self.helper_path
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_limits(mut self, limits: VectorProjectionClientLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn execute(
        &self,
        request: &VectorProjectionHelperRequest,
    ) -> Result<VectorProjectionHelperResponse, VectorError> {
        let correlation = projection_request_correlation(request);
        if !valid_projection_request_correlation(request) {
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Protocol,
                "invalid_request",
                false,
                "projection request correlation is internally inconsistent",
                &correlation,
            ));
        }
        let request_json = serde_json::to_vec(request).map_err(|error| {
            projection_client_error(
                VectorProjectionHelperErrorKind::Protocol,
                "request_encoding_failed",
                false,
                bounded_projection_message(&error.to_string(), request, 240),
                &correlation,
            )
        })?;
        if request_json.len() > self.limits.max_stdin_bytes {
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Protocol,
                "request_too_large",
                false,
                format!(
                    "projection request exceeds the configured {} byte stdin limit",
                    self.limits.max_stdin_bytes
                ),
                &correlation,
            ));
        }

        let mut command = Command::new(&self.helper_path);
        command.arg("projection").arg("--db").arg(&self.db_path);
        if let Some(path) = &self.vector_config_path {
            command.arg("--vector-config").arg(path);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "spawn_failed",
                true,
                format!(
                    "vector projection helper could not be started: {}",
                    bounded_projection_message(&error.to_string(), request, 240)
                ),
                &correlation,
            )
        })?;

        let Some(stdin) = child.stdin.take() else {
            terminate_projection_child(&mut child);
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "stdin_unavailable",
                true,
                "vector projection helper stdin pipe is unavailable",
                &correlation,
            ));
        };
        let Some(stdout) = child.stdout.take() else {
            terminate_projection_child(&mut child);
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "stdout_unavailable",
                true,
                "vector projection helper stdout pipe is unavailable",
                &correlation,
            ));
        };
        let Some(stderr) = child.stderr.take() else {
            terminate_projection_child(&mut child);
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "stderr_unavailable",
                true,
                "vector projection helper stderr pipe is unavailable",
                &correlation,
            ));
        };

        let stdout_overflow = Arc::new(AtomicBool::new(false));
        let stderr_overflow = Arc::new(AtomicBool::new(false));
        let stdin_thread = thread::spawn(move || {
            let mut stdin = stdin;
            let result = stdin.write_all(&request_json).map_err(|error| error.kind());
            drop(stdin);
            result
        });
        let stdout_thread = spawn_bounded_projection_reader(
            stdout,
            self.limits.max_stdout_bytes,
            Arc::clone(&stdout_overflow),
        );
        let stderr_thread = spawn_bounded_projection_reader(
            stderr,
            self.limits.max_stderr_bytes,
            Arc::clone(&stderr_overflow),
        );

        let mut outcome =
            wait_for_projection_child(&mut child, self.timeout, &stdout_overflow, &stderr_overflow);
        let stdin_result = stdin_thread.join();
        let stdout_result = stdout_thread.join();
        let stderr_result = stderr_thread.join();

        if stdout_overflow.load(Ordering::Acquire) {
            terminate_projection_child(&mut child);
            outcome = ProjectionChildOutcome::StdoutTooLarge;
        } else if stderr_overflow.load(Ordering::Acquire) {
            terminate_projection_child(&mut child);
            outcome = ProjectionChildOutcome::StderrTooLarge;
        }

        if matches!(outcome, ProjectionChildOutcome::Timeout) {
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "timeout",
                true,
                "vector projection helper exceeded its execution timeout",
                &correlation,
            ));
        }
        if matches!(outcome, ProjectionChildOutcome::StdoutTooLarge) {
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "stdout_too_large",
                false,
                format!(
                    "vector projection helper stdout exceeds the configured {} byte limit",
                    self.limits.max_stdout_bytes
                ),
                &correlation,
            ));
        }
        if matches!(outcome, ProjectionChildOutcome::StderrTooLarge) {
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "stderr_too_large",
                false,
                format!(
                    "vector projection helper stderr exceeds the configured {} byte limit",
                    self.limits.max_stderr_bytes
                ),
                &correlation,
            ));
        }
        let ProjectionChildOutcome::Exited(status) = outcome else {
            let message = match outcome {
                ProjectionChildOutcome::WaitFailed(message) => message,
                _ => "vector projection helper process failed".to_owned(),
            };
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "wait_failed",
                true,
                bounded_projection_message(&message, request, 240),
                &correlation,
            ));
        };

        match stdin_result {
            Ok(Ok(())) => {}
            Ok(Err(_)) | Err(_) => {
                return Err(projection_client_error(
                    VectorProjectionHelperErrorKind::Backend,
                    "stdin_write_failed",
                    true,
                    "vector projection helper closed stdin before accepting the request",
                    &correlation,
                ));
            }
        }
        let stdout = match stdout_result {
            Ok(BoundedProjectionRead {
                bytes,
                read_error: None,
            }) => bytes,
            Ok(BoundedProjectionRead {
                read_error: Some(_),
                ..
            })
            | Err(_) => {
                return Err(projection_client_error(
                    VectorProjectionHelperErrorKind::Backend,
                    "stdout_read_failed",
                    true,
                    "vector projection helper stdout could not be read",
                    &correlation,
                ));
            }
        };
        let stderr = match stderr_result {
            Ok(BoundedProjectionRead {
                bytes,
                read_error: None,
            }) => bytes,
            Ok(BoundedProjectionRead {
                read_error: Some(_),
                ..
            })
            | Err(_) => {
                return Err(projection_client_error(
                    VectorProjectionHelperErrorKind::Backend,
                    "stderr_read_failed",
                    true,
                    "vector projection helper stderr could not be read",
                    &correlation,
                ));
            }
        };

        if !status.success() {
            if matches!(
                serde_json::from_slice::<VectorProjectionHelperResponse>(&stdout),
                Ok(VectorProjectionHelperResponse::Error(_))
            ) {
                return decode_vector_projection_response(request, &stdout);
            }
            let stderr =
                bounded_projection_message(&String::from_utf8_lossy(&stderr), request, 240);
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Backend,
                "helper_exit",
                false,
                format!(
                    "vector projection helper exited with status {}: {}",
                    projection_exit_label(status),
                    if stderr.is_empty() {
                        "no bounded diagnostic".to_owned()
                    } else {
                        stderr
                    }
                ),
                &correlation,
            ));
        }
        decode_vector_projection_response(request, &stdout)
    }
}

pub fn decode_vector_projection_response(
    request: &VectorProjectionHelperRequest,
    stdout: &[u8],
) -> Result<VectorProjectionHelperResponse, VectorError> {
    let correlation = projection_request_correlation(request);
    let response =
        serde_json::from_slice::<VectorProjectionHelperResponse>(stdout).map_err(|error| {
            projection_client_error(
                VectorProjectionHelperErrorKind::Protocol,
                "invalid_response",
                false,
                format!(
                    "vector projection helper returned invalid JSON: {}",
                    bounded_projection_message(&error.to_string(), request, 160)
                ),
                &correlation,
            )
        })?;
    if let VectorProjectionHelperResponse::Error(error) = response {
        if !valid_projection_error_correlation(&error, &correlation) {
            return Err(projection_client_error(
                VectorProjectionHelperErrorKind::Protocol,
                "correlation_mismatch",
                false,
                "vector projection helper error correlation does not match the request",
                &correlation,
            ));
        }
        return Err(projection_helper_error(error, request, &correlation));
    }
    validate_projection_response(request, &response).map_err(|validation| {
        projection_client_error(
            VectorProjectionHelperErrorKind::Protocol,
            validation.code(),
            false,
            validation.message(),
            &correlation,
        )
    })?;
    Ok(response)
}

struct BoundedProjectionRead {
    bytes: Vec<u8>,
    read_error: Option<std::io::ErrorKind>,
}

fn spawn_bounded_projection_reader(
    mut reader: impl Read + Send + 'static,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<BoundedProjectionRead> {
    thread::spawn(move || {
        let mut bytes = Vec::with_capacity(limit.min(16 * 1024));
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    return BoundedProjectionRead {
                        bytes,
                        read_error: None,
                    };
                }
                Ok(read) => {
                    let retained = limit.saturating_sub(bytes.len()).min(read);
                    bytes.extend_from_slice(&buffer[..retained]);
                    if retained < read {
                        overflow.store(true, Ordering::Release);
                    }
                }
                Err(error) => {
                    return BoundedProjectionRead {
                        bytes,
                        read_error: Some(error.kind()),
                    };
                }
            }
        }
    })
}

enum ProjectionChildOutcome {
    Exited(ExitStatus),
    Timeout,
    StdoutTooLarge,
    StderrTooLarge,
    WaitFailed(String),
}

fn wait_for_projection_child(
    child: &mut Child,
    timeout: Duration,
    stdout_overflow: &AtomicBool,
    stderr_overflow: &AtomicBool,
) -> ProjectionChildOutcome {
    let started = Instant::now();
    loop {
        if stdout_overflow.load(Ordering::Acquire) {
            terminate_projection_child(child);
            return ProjectionChildOutcome::StdoutTooLarge;
        }
        if stderr_overflow.load(Ordering::Acquire) {
            terminate_projection_child(child);
            return ProjectionChildOutcome::StderrTooLarge;
        }
        match child.try_wait() {
            Ok(Some(status)) => return ProjectionChildOutcome::Exited(status),
            Ok(None) => {}
            Err(error) => {
                terminate_projection_child(child);
                return ProjectionChildOutcome::WaitFailed(error.to_string());
            }
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            terminate_projection_child(child);
            return ProjectionChildOutcome::Timeout;
        }
        thread::sleep((timeout - elapsed).min(Duration::from_millis(5)));
    }
}

fn terminate_projection_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn projection_exit_label(status: ExitStatus) -> String {
    status.code().map_or_else(
        || "terminated by signal".to_owned(),
        |code| code.to_string(),
    )
}

struct ProjectionRequestCorrelation<'request> {
    request_id: &'request str,
    projection_store: Option<&'request str>,
    generation_id: Option<&'request str>,
    delivery_digest: Option<&'request str>,
}

fn projection_request_correlation(
    request: &VectorProjectionHelperRequest,
) -> ProjectionRequestCorrelation<'_> {
    match request {
        VectorProjectionHelperRequest::Descriptor(request) => ProjectionRequestCorrelation {
            request_id: &request.request_id,
            projection_store: None,
            generation_id: None,
            delivery_digest: None,
        },
        VectorProjectionHelperRequest::PrepareSnapshot(request) => {
            mutation_correlation(&request.context)
        }
        VectorProjectionHelperRequest::ApplyBatch(request) => {
            mutation_correlation(&request.context)
        }
        VectorProjectionHelperRequest::Publish(request) => mutation_correlation(&request.context),
        VectorProjectionHelperRequest::InspectActive(request) => ProjectionRequestCorrelation {
            request_id: &request.request_id,
            projection_store: Some(&request.projection_store),
            generation_id: None,
            delivery_digest: None,
        },
        VectorProjectionHelperRequest::InspectGeneration(request) => ProjectionRequestCorrelation {
            request_id: &request.request_id,
            projection_store: Some(&request.projection_store),
            generation_id: Some(&request.generation_id),
            delivery_digest: None,
        },
        VectorProjectionHelperRequest::ValidateGenerationPublication(request) => {
            ProjectionRequestCorrelation {
                request_id: &request.request_id,
                projection_store: Some(&request.projection_store),
                generation_id: Some(&request.expected.manifest.generation),
                delivery_digest: Some(&request.expected.manifest.delivery_digest),
            }
        }
        VectorProjectionHelperRequest::ValidateActiveContents(request) => {
            ProjectionRequestCorrelation {
                request_id: &request.request_id,
                projection_store: Some(&request.projection_store),
                generation_id: Some(&request.active.manifest.generation),
                delivery_digest: Some(&request.active.manifest.delivery_digest),
            }
        }
        VectorProjectionHelperRequest::RepairPublication(request) => {
            mutation_correlation(&request.context)
        }
        VectorProjectionHelperRequest::Quarantine(request)
        | VectorProjectionHelperRequest::Abort(request) => mutation_correlation(&request.context),
        VectorProjectionHelperRequest::Inventory(request) => ProjectionRequestCorrelation {
            request_id: &request.request_id,
            projection_store: Some(&request.projection_store),
            generation_id: None,
            delivery_digest: None,
        },
        VectorProjectionHelperRequest::Cleanup(request) => mutation_correlation(&request.context),
    }
}

fn mutation_correlation(
    context: &VectorProjectionMutationContext,
) -> ProjectionRequestCorrelation<'_> {
    ProjectionRequestCorrelation {
        request_id: &context.request_id,
        projection_store: Some(&context.projection_store),
        generation_id: Some(&context.generation_id),
        delivery_digest: Some(&context.delivery_digest),
    }
}

fn valid_projection_request_correlation(request: &VectorProjectionHelperRequest) -> bool {
    match request {
        VectorProjectionHelperRequest::PrepareSnapshot(request) => {
            context_matches_manifest(&request.context, &request.snapshot.manifest)
                && request.snapshot.manifest.corpus.as_ref() == Some(&request.metadata)
                && valid_projection_destructive_authority(&request.context, &request.authority)
        }
        VectorProjectionHelperRequest::ApplyBatch(VectorProjectionApplyBatchRequest {
            context,
            authority,
            batch,
        }) => {
            context.projection_store == batch.store_name
                && context.generation_id == batch.target_generation
                && valid_projection_destructive_authority(context, authority)
                && authority.owner == batch.owner
                && authority.lease_token == batch.lease_token
                && authority.fence_epoch == batch.fence_epoch
                && authority.generation == batch.target_generation
                && batch.items.iter().all(|item| {
                    item.store_name == batch.store_name
                        && item.generation_id == batch.target_generation
                })
        }
        VectorProjectionHelperRequest::Publish(request) => {
            context_matches_manifest(&request.context, &request.prepared.manifest)
                && valid_projection_destructive_authority(&request.context, &request.authority)
                && request.expected_active.as_ref().is_none_or(|expected| {
                    expected.manifest.store_name == request.context.projection_store
                })
        }
        VectorProjectionHelperRequest::ValidateGenerationPublication(request) => {
            request.projection_store == request.expected.manifest.store_name
        }
        VectorProjectionHelperRequest::ValidateActiveContents(request) => {
            request.projection_store == request.active.manifest.store_name
        }
        VectorProjectionHelperRequest::RepairPublication(request) => {
            context_matches_manifest(&request.context, &request.expected.manifest)
                && valid_projection_destructive_authority(&request.context, &request.authority)
        }
        VectorProjectionHelperRequest::Quarantine(request)
        | VectorProjectionHelperRequest::Abort(request) => {
            valid_projection_destructive_authority(&request.context, &request.authority)
        }
        VectorProjectionHelperRequest::Cleanup(request) => {
            valid_cleanup_protection(&request.protection)
                && valid_projection_destructive_authority(&request.context, &request.authority)
        }
        _ => true,
    }
}

fn valid_projection_destructive_authority(
    context: &VectorProjectionMutationContext,
    authority: &VectorProjectionDestructiveAuthority,
) -> bool {
    if context.request_id.trim().is_empty()
        || context.projection_store.trim().is_empty()
        || context.generation_id.trim().is_empty()
        || context.delivery_digest.trim().is_empty()
        || authority.owner.trim().is_empty()
        || authority.lease_token.trim().is_empty()
        || authority.fence_epoch < 0
        || authority.generation != context.generation_id
    {
        return false;
    }

    if authority.role == VectorProjectionGenerationRole::Orphaned {
        return authority.expected_manifest.is_none()
            && authority.expected_binding.is_none()
            && authority.building_phase.is_none();
    }

    let Some(binding) = authority.expected_binding.as_ref() else {
        return false;
    };
    if authority.generation != binding.generation
        || context.delivery_digest != binding.delivery_digest
        || binding.fence_epoch > authority.fence_epoch
        || !valid_projection_generation_binding(binding)
    {
        return false;
    }

    let has_complete_artifact_binding = binding
        .fingerprint
        .as_ref()
        .is_some_and(|fingerprint| !fingerprint.trim().is_empty())
        && binding.snapshot_cursor.is_some();
    let manifest_matches = authority
        .expected_manifest
        .as_ref()
        .is_some_and(|manifest| {
            context_matches_manifest(context, manifest)
                && valid_projection_manifest(manifest)
                && projection_manifest_matches_generation_binding(manifest, binding)
        });

    match authority.role {
        VectorProjectionGenerationRole::Active | VectorProjectionGenerationRole::Previous => {
            authority.building_phase.is_none() && has_complete_artifact_binding && manifest_matches
        }
        VectorProjectionGenerationRole::Building => match authority.building_phase {
            Some(VectorProjectionBuildingPhase::Snapshotting) => {
                authority.expected_manifest.is_none()
                    && binding.fingerprint.is_none()
                    && binding.snapshot_cursor.is_none()
            }
            Some(
                VectorProjectionBuildingPhase::Prepared
                | VectorProjectionBuildingPhase::StorePublished,
            ) => has_complete_artifact_binding && manifest_matches,
            None => false,
        },
        VectorProjectionGenerationRole::Orphaned => {
            unreachable!("orphaned authority returned before binding validation")
        }
    }
}

fn valid_projection_generation_binding(binding: &VectorProjectionGenerationBinding) -> bool {
    !binding.generation.trim().is_empty()
        && binding
            .fingerprint
            .as_ref()
            .is_none_or(|fingerprint| !fingerprint.trim().is_empty())
        && binding.fence_epoch >= 0
        && binding.snapshot_cursor.is_none_or(|cursor| cursor >= 0)
        && !binding.provider.trim().is_empty()
        && !binding.provider_fingerprint.trim().is_empty()
        && binding.canonical_count >= 0
        && !binding.canonical_digest.trim().is_empty()
        && binding.delivery_count >= 0
        && !binding.delivery_digest.trim().is_empty()
        && binding.corpus.as_ref().is_none_or(valid_projection_corpus)
}

fn valid_projection_manifest(manifest: &ProjectionArtifactManifest) -> bool {
    !manifest.store_name.trim().is_empty()
        && manifest.database_instance_id.starts_with("db_")
        && manifest.protocol_version == kanban_contract::VECTOR_PROJECTION_PROTOCOL_VERSION
        && manifest.schema_version > 0
        && !manifest.generation.trim().is_empty()
        && manifest.fence_epoch >= 0
        && manifest.snapshot_cursor >= 0
        && !manifest.provider.trim().is_empty()
        && !manifest.provider_fingerprint.trim().is_empty()
        && manifest.canonical_item_count >= 0
        && !manifest.canonical_digest.trim().is_empty()
        && manifest.delivery_item_count >= 0
        && !manifest.delivery_digest.trim().is_empty()
        && manifest
            .fingerprint
            .as_ref()
            .is_some_and(|fingerprint| !fingerprint.trim().is_empty())
        && manifest.corpus.as_ref().is_none_or(valid_projection_corpus)
}

fn valid_projection_corpus(corpus: &ProjectionCorpusMetadata) -> bool {
    !corpus.corpus_schema.trim().is_empty()
        && !corpus.corpus_fingerprint.trim().is_empty()
        && !corpus.embedding_model.trim().is_empty()
        && corpus.embedding_dimensions > 0
}

fn projection_manifest_matches_generation_binding(
    manifest: &ProjectionArtifactManifest,
    binding: &VectorProjectionGenerationBinding,
) -> bool {
    manifest.generation == binding.generation
        && manifest.fingerprint == binding.fingerprint
        && manifest.fence_epoch == binding.fence_epoch
        && Some(manifest.snapshot_cursor) == binding.snapshot_cursor
        && manifest.provider == binding.provider
        && manifest.provider_fingerprint == binding.provider_fingerprint
        && manifest.canonical_item_count == binding.canonical_count
        && manifest.canonical_digest == binding.canonical_digest
        && manifest.delivery_item_count == binding.delivery_count
        && manifest.delivery_digest == binding.delivery_digest
        && manifest.corpus == binding.corpus
}

fn valid_cleanup_protection(
    protection: &kanban_contract::VectorProjectionCleanupProtection,
) -> bool {
    let mut generations = BTreeSet::new();
    [
        protection.active_generation.as_deref(),
        protection.previous_generation.as_deref(),
        protection.building_generation.as_deref(),
    ]
    .into_iter()
    .flatten()
    .chain(protection.additional_generations.iter().map(String::as_str))
    .all(|generation| !generation.trim().is_empty() && generations.insert(generation))
}

pub fn validate_projection_request_against_descriptor(
    request: &VectorProjectionHelperRequest,
    descriptor: &VectorProjectionHelperDescriptor,
) -> Result<(), VectorError> {
    let correlation = projection_request_correlation(request);
    if !valid_projection_request_correlation(request) {
        return Err(projection_client_error(
            VectorProjectionHelperErrorKind::Protocol,
            "invalid_request",
            false,
            "projection request correlation is internally inconsistent",
            &correlation,
        ));
    }
    if !valid_projection_descriptor(descriptor) {
        return Err(projection_client_error(
            VectorProjectionHelperErrorKind::Protocol,
            "invalid_descriptor",
            false,
            "vector projection helper descriptor is internally inconsistent",
            &correlation,
        ));
    }
    if !descriptor
        .supported_operations
        .contains(&request.operation())
    {
        return Err(projection_client_error(
            VectorProjectionHelperErrorKind::Protocol,
            "unsupported_operation",
            false,
            "vector projection helper does not advertise the requested operation",
            &correlation,
        ));
    }
    let Some(binding) = projection_request_store_binding(request) else {
        return Ok(());
    };
    if binding
        .protocol_version
        .is_some_and(|protocol_version| protocol_version != descriptor.protocol_version)
    {
        return Err(projection_client_error(
            VectorProjectionHelperErrorKind::Protocol,
            "protocol_version_mismatch",
            false,
            "vector projection helper protocol version does not match the request",
            &correlation,
        ));
    }
    let Some(store) = descriptor
        .supported_stores
        .iter()
        .find(|store| store.store_name == binding.store_name)
    else {
        return Err(projection_client_error(
            VectorProjectionHelperErrorKind::Protocol,
            "unsupported_store",
            false,
            "vector projection helper does not advertise the requested store",
            &correlation,
        ));
    };
    if binding
        .schema_version
        .is_some_and(|schema_version| schema_version != store.schema_version)
    {
        return Err(projection_client_error(
            VectorProjectionHelperErrorKind::Protocol,
            "schema_version_mismatch",
            false,
            "vector projection helper store schema does not match the request",
            &correlation,
        ));
    }
    if binding
        .provider
        .is_some_and(|provider| provider != store.provider)
        || binding
            .provider_fingerprint
            .is_some_and(|fingerprint| fingerprint != store.provider_fingerprint)
        || binding
            .corpus
            .is_some_and(|corpus| store.corpus.as_ref() != Some(corpus))
    {
        return Err(projection_client_error(
            VectorProjectionHelperErrorKind::Protocol,
            "provider_binding_mismatch",
            false,
            "vector projection helper store provider binding does not match the request",
            &correlation,
        ));
    }
    Ok(())
}

fn valid_projection_descriptor(descriptor: &VectorProjectionHelperDescriptor) -> bool {
    if descriptor.protocol_version != kanban_contract::VECTOR_PROJECTION_PROTOCOL_VERSION
        || descriptor.build_identity.trim().is_empty()
        || descriptor.supported_operations.is_empty()
        || !descriptor
            .supported_operations
            .contains(&kanban_contract::VectorProjectionHelperOperation::Descriptor)
        || !strictly_increasing(&descriptor.supported_operations)
        || !descriptor
            .supported_stores
            .windows(2)
            .all(|stores| stores[0].store_name < stores[1].store_name)
    {
        return false;
    }
    let has_store_operation = descriptor.supported_operations.iter().any(|operation| {
        *operation != kanban_contract::VectorProjectionHelperOperation::Descriptor
    });
    if has_store_operation && descriptor.supported_stores.is_empty() {
        return false;
    }
    descriptor
        .supported_stores
        .iter()
        .all(valid_store_descriptor)
}

fn strictly_increasing<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_store_descriptor(descriptor: &ProjectionStoreDescriptor) -> bool {
    !descriptor.store_name.trim().is_empty()
        && descriptor.schema_version > 0
        && !descriptor.provider.trim().is_empty()
        && !descriptor.provider_fingerprint.trim().is_empty()
        && descriptor.corpus.as_ref().is_none_or(|corpus| {
            !corpus.corpus_schema.trim().is_empty()
                && !corpus.corpus_fingerprint.trim().is_empty()
                && !corpus.embedding_model.trim().is_empty()
                && corpus.embedding_dimensions > 0
        })
}

struct ProjectionRequestStoreBinding<'request> {
    store_name: &'request str,
    protocol_version: Option<i64>,
    schema_version: Option<i64>,
    provider: Option<&'request str>,
    provider_fingerprint: Option<&'request str>,
    corpus: Option<&'request ProjectionCorpusMetadata>,
}

fn projection_request_store_binding(
    request: &VectorProjectionHelperRequest,
) -> Option<ProjectionRequestStoreBinding<'_>> {
    match request {
        VectorProjectionHelperRequest::Descriptor(_) => None,
        VectorProjectionHelperRequest::PrepareSnapshot(request) => {
            Some(manifest_store_binding(&request.snapshot.manifest))
        }
        VectorProjectionHelperRequest::ApplyBatch(request) => Some(ProjectionRequestStoreBinding {
            store_name: &request.batch.store_name,
            protocol_version: Some(request.batch.protocol_version),
            schema_version: Some(request.batch.schema_version),
            provider: Some(&request.batch.provider),
            provider_fingerprint: Some(&request.batch.provider_fingerprint),
            corpus: None,
        }),
        VectorProjectionHelperRequest::Publish(request) => {
            Some(manifest_store_binding(&request.prepared.manifest))
        }
        VectorProjectionHelperRequest::InspectActive(request) => {
            Some(store_only_binding(&request.projection_store))
        }
        VectorProjectionHelperRequest::InspectGeneration(request) => {
            Some(store_only_binding(&request.projection_store))
        }
        VectorProjectionHelperRequest::ValidateGenerationPublication(request) => {
            Some(manifest_store_binding(&request.expected.manifest))
        }
        VectorProjectionHelperRequest::ValidateActiveContents(request) => {
            Some(manifest_store_binding(&request.active.manifest))
        }
        VectorProjectionHelperRequest::RepairPublication(request) => {
            Some(manifest_store_binding(&request.expected.manifest))
        }
        VectorProjectionHelperRequest::Quarantine(request)
        | VectorProjectionHelperRequest::Abort(request) => {
            Some(store_only_binding(&request.context.projection_store))
        }
        VectorProjectionHelperRequest::Inventory(request) => {
            Some(store_only_binding(&request.projection_store))
        }
        VectorProjectionHelperRequest::Cleanup(request) => {
            Some(store_only_binding(&request.context.projection_store))
        }
    }
}

fn manifest_store_binding(
    manifest: &ProjectionArtifactManifest,
) -> ProjectionRequestStoreBinding<'_> {
    ProjectionRequestStoreBinding {
        store_name: &manifest.store_name,
        protocol_version: Some(manifest.protocol_version),
        schema_version: Some(manifest.schema_version),
        provider: Some(&manifest.provider),
        provider_fingerprint: Some(&manifest.provider_fingerprint),
        corpus: manifest.corpus.as_ref(),
    }
}

fn store_only_binding(store_name: &str) -> ProjectionRequestStoreBinding<'_> {
    ProjectionRequestStoreBinding {
        store_name,
        protocol_version: None,
        schema_version: None,
        provider: None,
        provider_fingerprint: None,
        corpus: None,
    }
}

fn context_matches_manifest(
    context: &VectorProjectionMutationContext,
    manifest: &kanban_contract::ProjectionArtifactManifest,
) -> bool {
    context.projection_store == manifest.store_name
        && context.generation_id == manifest.generation
        && context.delivery_digest == manifest.delivery_digest
}

fn valid_projection_error_correlation(
    error: &VectorProjectionHelperError,
    expected: &ProjectionRequestCorrelation<'_>,
) -> bool {
    optional_error_field_matches(error.request_id.as_deref(), Some(expected.request_id))
        && optional_error_field_matches(
            error.projection_store.as_deref(),
            expected.projection_store,
        )
        && optional_error_field_matches(error.generation_id.as_deref(), expected.generation_id)
        && optional_error_field_matches(error.delivery_digest.as_deref(), expected.delivery_digest)
}

fn optional_error_field_matches(provided: Option<&str>, expected: Option<&str>) -> bool {
    match provided {
        Some(provided) => Some(provided) == expected,
        None => true,
    }
}

#[derive(Debug)]
enum ProjectionResponseValidation {
    Operation,
    Correlation,
    Receipt,
}

impl ProjectionResponseValidation {
    fn code(&self) -> &'static str {
        match self {
            Self::Operation => "operation_mismatch",
            Self::Correlation => "correlation_mismatch",
            Self::Receipt => "receipt_mismatch",
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::Operation => {
                "vector projection helper response operation does not match the request"
            }
            Self::Correlation => {
                "vector projection helper response correlation does not match the request"
            }
            Self::Receipt => "vector projection helper response receipt does not match the request",
        }
    }
}

fn validate_projection_response(
    request: &VectorProjectionHelperRequest,
    response: &VectorProjectionHelperResponse,
) -> Result<(), ProjectionResponseValidation> {
    match (request, response) {
        (
            VectorProjectionHelperRequest::Descriptor(request),
            VectorProjectionHelperResponse::Descriptor(response),
        ) => {
            correlation_check(request.request_id == response.request_id)?;
            receipt_check(valid_projection_descriptor(response))
        }
        (
            VectorProjectionHelperRequest::PrepareSnapshot(request),
            VectorProjectionHelperResponse::PrepareSnapshot(response),
        ) => {
            correlation_check(ack_matches(&request.context, &response.ack))?;
            receipt_check(response.evidence.manifest == request.snapshot.manifest)
        }
        (
            VectorProjectionHelperRequest::ApplyBatch(request),
            VectorProjectionHelperResponse::ApplyBatch(response),
        ) => {
            correlation_check(ack_matches(&request.context, &response.ack))?;
            receipt_check(
                response.receipt.store_name == request.batch.store_name
                    && response.receipt.database_instance_id == request.batch.database_instance_id
                    && response.receipt.protocol_version == request.batch.protocol_version
                    && response.receipt.schema_version == request.batch.schema_version
                    && response.receipt.provider == request.batch.provider
                    && response.receipt.provider_fingerprint == request.batch.provider_fingerprint
                    && response.receipt.target_generation == request.batch.target_generation
                    && response.receipt.fence_epoch == request.batch.fence_epoch
                    && response.receipt.applied_item_count == request.batch.items.len(),
            )
        }
        (
            VectorProjectionHelperRequest::Publish(request),
            VectorProjectionHelperResponse::Publish(response),
        ) => {
            correlation_check(ack_matches(&request.context, &response.ack))?;
            receipt_check(
                response.receipt.active == request.prepared
                    && response.receipt.retained_previous == request.expected_active,
            )
        }
        (
            VectorProjectionHelperRequest::InspectActive(request),
            VectorProjectionHelperResponse::InspectActive(response),
        ) => {
            correlation_check(
                request.request_id == response.request_id
                    && request.projection_store == response.projection_store,
            )?;
            receipt_check(
                response
                    .active
                    .as_ref()
                    .is_none_or(|active| active.manifest.store_name == request.projection_store),
            )
        }
        (
            VectorProjectionHelperRequest::InspectGeneration(request),
            VectorProjectionHelperResponse::InspectGeneration(response),
        ) => {
            correlation_check(
                request.request_id == response.request_id
                    && request.projection_store == response.projection_store
                    && request.generation_id == response.generation_id,
            )?;
            receipt_check(response.evidence.as_ref().is_none_or(|evidence| {
                evidence.manifest.store_name == request.projection_store
                    && evidence.manifest.generation == request.generation_id
            }))
        }
        (
            VectorProjectionHelperRequest::ValidateGenerationPublication(request),
            VectorProjectionHelperResponse::ValidateGenerationPublication(response),
        ) => correlation_check(
            request.request_id == response.request_id
                && request.projection_store == response.projection_store,
        ),
        (
            VectorProjectionHelperRequest::ValidateActiveContents(request),
            VectorProjectionHelperResponse::ValidateActiveContents(response),
        ) => correlation_check(
            request.request_id == response.request_id
                && request.projection_store == response.projection_store,
        ),
        (
            VectorProjectionHelperRequest::RepairPublication(request),
            VectorProjectionHelperResponse::RepairPublication(response),
        ) => correlation_check(ack_matches(&request.context, response)),
        (
            VectorProjectionHelperRequest::Quarantine(request),
            VectorProjectionHelperResponse::Quarantine(response),
        ) => correlation_check(ack_matches(&request.context, response)),
        (
            VectorProjectionHelperRequest::Abort(request),
            VectorProjectionHelperResponse::Abort(response),
        ) => correlation_check(ack_matches(&request.context, response)),
        (
            VectorProjectionHelperRequest::Inventory(request),
            VectorProjectionHelperResponse::Inventory(response),
        ) => {
            correlation_check(
                request.request_id == response.request_id
                    && request.projection_store == response.projection_store,
            )?;
            receipt_check(response.generations.iter().all(|generation| {
                generation.evidence.as_ref().is_none_or(|evidence| {
                    evidence.manifest.store_name == request.projection_store
                        && evidence.manifest.generation == generation.generation_id
                })
            }))
        }
        (
            VectorProjectionHelperRequest::Cleanup(request),
            VectorProjectionHelperResponse::Cleanup(response),
        ) => {
            correlation_check(ack_matches(&request.context, &response.ack))?;
            receipt_check(cleanup_response_matches(request, response))
        }
        (_, VectorProjectionHelperResponse::Error(_)) => {
            unreachable!("error responses are handled before response validation")
        }
        _ => Err(ProjectionResponseValidation::Operation),
    }
}

fn cleanup_response_matches(
    request: &kanban_contract::VectorProjectionCleanupRequest,
    response: &kanban_contract::VectorProjectionCleanupResponse,
) -> bool {
    if response.dry_run != request.dry_run
        || (request.dry_run && !response.removed_generations.is_empty())
    {
        return false;
    }
    let Some(expected_protected) = cleanup_protected_generations(&request.protection) else {
        return false;
    };
    let mut actual_protected = BTreeMap::new();
    for protected in &response.protected_generations {
        if actual_protected
            .insert(protected.generation_id.as_str(), protected.reason)
            .is_some()
        {
            return false;
        }
    }
    if actual_protected != expected_protected {
        return false;
    }
    let removed = response
        .removed_generations
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if removed.len() != response.removed_generations.len()
        || removed
            .iter()
            .any(|generation| expected_protected.contains_key(generation))
    {
        return false;
    }
    let skipped = response
        .skipped_generations
        .iter()
        .map(|entry| entry.generation_id.as_str())
        .collect::<BTreeSet<_>>();
    skipped.len() == response.skipped_generations.len()
        && removed.is_disjoint(&skipped)
        && skipped
            .iter()
            .all(|generation| !expected_protected.contains_key(generation))
}

fn cleanup_protected_generations(
    protection: &kanban_contract::VectorProjectionCleanupProtection,
) -> Option<BTreeMap<&str, VectorProjectionProtectionReason>> {
    let mut protected = BTreeMap::new();
    for (generation, reason) in [
        (
            protection.active_generation.as_deref(),
            VectorProjectionProtectionReason::Active,
        ),
        (
            protection.previous_generation.as_deref(),
            VectorProjectionProtectionReason::Previous,
        ),
        (
            protection.building_generation.as_deref(),
            VectorProjectionProtectionReason::Building,
        ),
    ] {
        if let Some(generation) = generation
            && (generation.trim().is_empty() || protected.insert(generation, reason).is_some())
        {
            return None;
        }
    }
    for generation in &protection.additional_generations {
        if generation.trim().is_empty()
            || protected
                .insert(generation, VectorProjectionProtectionReason::Explicit)
                .is_some()
        {
            return None;
        }
    }
    Some(protected)
}

fn ack_matches(
    context: &VectorProjectionMutationContext,
    ack: &VectorProjectionMutationAck,
) -> bool {
    context.request_id == ack.request_id
        && context.projection_store == ack.projection_store
        && context.generation_id == ack.generation_id
        && context.delivery_digest == ack.delivery_digest
}

fn correlation_check(valid: bool) -> Result<(), ProjectionResponseValidation> {
    valid
        .then_some(())
        .ok_or(ProjectionResponseValidation::Correlation)
}

fn receipt_check(valid: bool) -> Result<(), ProjectionResponseValidation> {
    valid
        .then_some(())
        .ok_or(ProjectionResponseValidation::Receipt)
}

fn projection_helper_error(
    error: VectorProjectionHelperError,
    request: &VectorProjectionHelperRequest,
    correlation: &ProjectionRequestCorrelation<'_>,
) -> VectorError {
    VectorError::ProjectionHelper(Box::new(VectorProjectionClientError {
        kind: error.kind,
        code: bounded_projection_message(&error.code, request, 128),
        provider: error
            .provider
            .map(|value| bounded_projection_message(&value, request, 128)),
        backend: error
            .backend
            .map(|value| bounded_projection_message(&value, request, 128)),
        retryable: error.retryable,
        message: bounded_projection_message(&error.message, request, 512),
        request_id: Some(correlation.request_id.to_owned()),
        delivery_digest: correlation.delivery_digest.map(str::to_owned),
        projection_store: correlation.projection_store.map(str::to_owned),
        generation_id: correlation.generation_id.map(str::to_owned),
    }))
}

fn projection_client_error(
    kind: VectorProjectionHelperErrorKind,
    code: impl Into<String>,
    retryable: bool,
    message: impl Into<String>,
    correlation: &ProjectionRequestCorrelation<'_>,
) -> VectorError {
    VectorError::ProjectionHelper(Box::new(VectorProjectionClientError {
        kind,
        code: code.into(),
        provider: None,
        backend: None,
        retryable,
        message: message.into(),
        request_id: Some(correlation.request_id.to_owned()),
        delivery_digest: correlation.delivery_digest.map(str::to_owned),
        projection_store: correlation.projection_store.map(str::to_owned),
        generation_id: correlation.generation_id.map(str::to_owned),
    }))
}

fn bounded_projection_message(
    value: &str,
    request: &VectorProjectionHelperRequest,
    max_bytes: usize,
) -> String {
    let mut value = value.replace(['\r', '\n'], " ");
    match request {
        VectorProjectionHelperRequest::PrepareSnapshot(request) => {
            redact_projection_capability(&mut value, &request.authority.lease_token);
        }
        VectorProjectionHelperRequest::Publish(request) => {
            redact_projection_capability(&mut value, &request.authority.lease_token);
        }
        VectorProjectionHelperRequest::RepairPublication(request) => {
            redact_projection_capability(&mut value, &request.authority.lease_token);
        }
        VectorProjectionHelperRequest::ApplyBatch(request) => {
            for capability in [&request.batch.lease_token, &request.batch.claim_token] {
                redact_projection_capability(&mut value, capability);
            }
            redact_projection_capability(&mut value, &request.authority.lease_token);
        }
        VectorProjectionHelperRequest::Quarantine(request)
        | VectorProjectionHelperRequest::Abort(request) => {
            redact_projection_capability(&mut value, &request.authority.lease_token);
        }
        VectorProjectionHelperRequest::Cleanup(request) => {
            redact_projection_capability(&mut value, &request.authority.lease_token);
        }
        _ => {}
    }
    truncate_projection_message(value, max_bytes)
}

fn redact_projection_capability(value: &mut String, capability: &str) {
    if !capability.is_empty() {
        *value = value.replace(capability, "[REDACTED]");
    }
}

fn truncate_projection_message(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let boundary = value
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_bytes)
        .last()
        .unwrap_or(0);
    value.truncate(boundary);
    value.push_str("...");
    value
}

fn bounded_helper_message(value: &str) -> String {
    const MAX: usize = 240;
    let mut value = value.replace(['\r', '\n'], " ");
    if value.len() > MAX {
        value.truncate(MAX);
        value.push_str("...");
    }
    value
}

impl VectorStoreBackend for SubprocessVectorStore {
    fn embedding_model(&self) -> &str {
        self.embedding_model
            .as_deref()
            .unwrap_or(DEFAULT_EMBEDDING_MODEL)
    }

    fn status(&self) -> VectorStoreStatus {
        let command = self.status_command().to_owned();
        match self
            .run_helper::<VectorHelperStatusResponse>(&[command])
            .map(vector_store_status)
        {
            Ok(status) => status,
            Err(error) => {
                let mut status = VectorStoreStatus::new(
                    "helper-missing",
                    false,
                    format!("vector helper unavailable: {error}"),
                );
                status.diagnostics.push("helper_missing".to_owned());
                status
            }
        }
    }
}

impl QueryEmbeddingProvider for SubprocessVectorStore {
    fn embed_query_text(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        self.run_helper::<VectorHelperEmbedQueryResponse>(&[
            "embed-query".to_owned(),
            "--text".to_owned(),
            text.to_owned(),
        ])
    }
}

impl ChunkVectorStore for SubprocessVectorStore {
    fn delete_board(&self, _board_id: &str) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn upsert(&self, _chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        Err(VectorError::Disabled)
    }

    fn query(&self, query: &VectorQuery) -> Result<Vec<VectorHit>, VectorError> {
        self.run_helper::<VectorHelperQueryChunksResponse>(&[
            "query-chunks".to_owned(),
            "--text".to_owned(),
            query.text.clone(),
            "--limit".to_owned(),
            query.limit.to_string(),
            "--board-id".to_owned(),
            query.board_id.clone(),
        ])?
        .into_iter()
        .map(vector_hit)
        .collect()
    }
}

impl LabelAtomVectorStore for SubprocessVectorStore {
    fn query_label_atoms(&self, query: &LabelAtomQuery) -> Result<Vec<LabelAtomHit>, VectorError> {
        let mut args = vec![
            "query-label-atoms".to_owned(),
            "--text".to_owned(),
            query.text.clone(),
            "--limit".to_owned(),
            query.limit.to_string(),
        ];
        push_label_atom_filters(
            &mut args,
            query.board_id.as_deref(),
            query.embedding_model.as_deref(),
            query.polarity.as_deref(),
        );
        self.run_helper::<VectorHelperQueryLabelAtomsResponse>(&args)?
            .into_iter()
            .map(|item| match item {
                VectorHelperQueryLabelAtomsItem::Hit(hit) => label_atom_hit(hit),
                VectorHelperQueryLabelAtomsItem::WithVector(hit) => label_atom_hit(hit.hit),
            })
            .collect()
    }

    fn query_label_atoms_by_vector(
        &self,
        query: &LabelAtomVectorQuery,
    ) -> Result<Vec<LabelAtomVectorHit>, VectorError> {
        let vector_json = serde_json::to_string(&query.vector)
            .map_err(|error| VectorError::Store(error.to_string()))?;
        let mut args = vec![
            "query-label-atoms".to_owned(),
            "--vector-json".to_owned(),
            vector_json,
            "--limit".to_owned(),
            query.limit.to_string(),
        ];
        push_label_atom_filters(
            &mut args,
            query.board_id.as_deref(),
            query.embedding_model.as_deref(),
            query.polarity.as_deref(),
        );
        if query.include_vector {
            args.push("--include-vector".to_owned());
        }
        self.run_helper::<VectorHelperQueryLabelAtomsResponse>(&args)?
            .into_iter()
            .map(|item| match item {
                VectorHelperQueryLabelAtomsItem::Hit(hit) => Ok(LabelAtomVectorHit {
                    hit: label_atom_hit(hit)?,
                    vector: None,
                }),
                VectorHelperQueryLabelAtomsItem::WithVector(hit) => Ok(LabelAtomVectorHit {
                    hit: label_atom_hit(hit.hit)?,
                    vector: hit.vector,
                }),
            })
            .collect()
    }
}

fn vector_store_status(status: VectorHelperStatusResponse) -> VectorStoreStatus {
    VectorStoreStatus {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
        diagnostics: status.diagnostics,
        dirty: status.dirty,
        board_dirty: status.board_dirty,
        generation: status.generation,
    }
}

fn vector_hit(hit: kanban_contract::VectorHelperChunkHit) -> Result<VectorHit, VectorError> {
    Ok(VectorHit {
        chunk: ChunkRef {
            uri: EntityUri::new(hit.chunk.uri)
                .map_err(|error| VectorError::Store(error.to_string()))?,
            entity_uri: EntityUri::new(hit.chunk.entity_uri)
                .map_err(|error| VectorError::Store(error.to_string()))?,
            ordinal: hit.chunk.ordinal,
            content_hash: hit.chunk.content_hash,
        },
        score: hit.score,
        text: hit.text,
        summary: hit.summary,
    })
}

fn label_atom_hit(hit: VectorHelperLabelAtomHit) -> Result<LabelAtomHit, VectorError> {
    Ok(LabelAtomHit {
        atom_id: hit.atom_id,
        label_id: hit.label_id,
        label_name: hit.label_name,
        board_id: hit.board_id,
        polarity: hit.polarity,
        kind: hit.kind,
        text: hit.text,
        ordinal: hit.ordinal,
        content_hash: hit.content_hash,
        embedding_model: hit.embedding_model,
        distance: hit.distance,
    })
}

fn push_label_atom_filters(
    args: &mut Vec<String>,
    board_id: Option<&str>,
    embedding_model: Option<&str>,
    polarity: Option<&str>,
) {
    if let Some(board_id) = board_id {
        args.push("--board-id".to_owned());
        args.push(board_id.to_owned());
    }
    if let Some(embedding_model) = embedding_model {
        args.push("--embedding-model".to_owned());
        args.push(embedding_model.to_owned());
    }
    if let Some(polarity) = polarity {
        args.push("--polarity".to_owned());
        args.push(polarity.to_owned());
    }
}
pub fn ensure_dimensions(vector: &[f32], expected: usize) -> Result<(), VectorError> {
    if vector.len() == expected {
        Ok(())
    } else {
        Err(VectorError::DimensionMismatch {
            expected,
            actual: vector.len(),
        })
    }
}

pub fn normalize_semantic_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn semantic_content_hash(text: &str) -> String {
    let normalized = normalize_semantic_text(text);
    let digest = Sha256::digest(normalized.as_bytes());
    format!("sha256:{digest:x}")
}

pub fn embedding_provider_fingerprint(provider: &str, model: &str, dimensions: usize) -> String {
    sha256_fingerprint(&[
        b"kanban.embedding-provider.v1",
        provider.as_bytes(),
        model.as_bytes(),
        dimensions.to_string().as_bytes(),
    ])
}

pub fn corpus_provider_fingerprint(corpus_schema: &str, provider_fingerprint: &str) -> String {
    sha256_fingerprint(&[
        b"kanban.embedding-corpus.v1",
        corpus_schema.as_bytes(),
        provider_fingerprint.as_bytes(),
    ])
}

/// Build the canonical identity for the label-atom corpus.
///
/// Label atoms intentionally use a corpus namespace separate from task chunks;
/// the provider/model/dimension tuple is included in the fingerprint so a
/// projection cannot be reused after any embedding configuration change.
pub fn label_atoms_corpus_metadata(
    provider: &str,
    model: &str,
    dimensions: usize,
) -> Result<ProjectionCorpusMetadata, VectorError> {
    if provider.trim().is_empty() {
        return Err(VectorError::Provider {
            message:
                "invalid label corpus identity field provider: embedding provider must not be blank"
                    .to_owned(),
            retryable: false,
        });
    }
    if model.trim().is_empty() {
        return Err(VectorError::Provider {
            message: "invalid label corpus identity field model: embedding model must not be blank"
                .to_owned(),
            retryable: false,
        });
    }
    if dimensions == 0 {
        return Err(VectorError::Provider {
            message: "invalid label corpus identity field dimensions: embedding dimensions must be greater than zero"
                .to_owned(),
            retryable: false,
        });
    }
    let provider_fingerprint = embedding_provider_fingerprint(provider, model, dimensions);
    Ok(ProjectionCorpusMetadata {
        corpus_schema: LABEL_ATOMS_CORPUS_SCHEMA.to_owned(),
        corpus_fingerprint: corpus_provider_fingerprint(
            LABEL_ATOMS_CORPUS_SCHEMA,
            &provider_fingerprint,
        ),
        embedding_model: model.to_owned(),
        embedding_dimensions: dimensions,
    })
}

fn sha256_fingerprint(parts: &[&[u8]]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part);
    }
    format!("sha256:{:x}", digest.finalize())
}

#[derive(Debug, Clone, Error)]
#[error("{kind:?}/{code}: {message}")]
pub struct VectorProjectionClientError {
    pub kind: VectorProjectionHelperErrorKind,
    pub code: String,
    pub provider: Option<String>,
    pub backend: Option<String>,
    pub retryable: bool,
    pub message: String,
    pub request_id: Option<String>,
    pub delivery_digest: Option<String>,
    pub projection_store: Option<String>,
    pub generation_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum VectorError {
    #[error("vector store is disabled")]
    Disabled,
    #[error("embedding provider is not configured")]
    MissingEmbeddingProvider,
    #[error("embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("embedding model mismatch: expected {expected}, got {actual}")]
    EmbeddingModelMismatch { expected: String, actual: String },
    #[error("embedding provider error: {message}")]
    Provider { message: String, retryable: bool },
    #[error("vector projection helper {0}")]
    ProjectionHelper(Box<VectorProjectionClientError>),
    #[error("chunk build error: {0}")]
    Chunk(String),
    #[error("vector store error: {0}")]
    Store(String),
}

impl VectorError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Provider { retryable, .. } => *retryable,
            Self::ProjectionHelper(error) => error.retryable,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChunkBuilder, ChunkVectorStore, DisabledVectorStore, LABEL_ATOMS_CORPUS_SCHEMA,
        LabelAtomVectorStore, SubprocessVectorProjectionClient, TASK_CHUNKS_CORPUS_SCHEMA,
        TaskChunkSource, VectorError, VectorProjectionClientLimits, VectorStore,
        VectorStoreBackend, corpus_provider_fingerprint, embedding_provider_fingerprint,
        ensure_dimensions, label_atoms_corpus_metadata,
    };
    use kanban_contract::{
        ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch,
        ProjectionCorpusMetadata, ProjectionDelivery, ProjectionDeliveryAction, ProjectionSnapshot,
        ProjectionStoreDescriptor, VectorProjectionApplyBatchRequest,
        VectorProjectionApplyBatchResponse, VectorProjectionBatchApplicationReceipt,
        VectorProjectionBuildingPhase, VectorProjectionCleanupProtection,
        VectorProjectionCleanupRequest, VectorProjectionCleanupResponse,
        VectorProjectionDestructiveAuthority, VectorProjectionGenerationBinding,
        VectorProjectionGenerationMutationRequest, VectorProjectionGenerationRole,
        VectorProjectionHelperDescriptor, VectorProjectionHelperError,
        VectorProjectionHelperErrorKind, VectorProjectionHelperOperation,
        VectorProjectionHelperRequest, VectorProjectionHelperResponse, VectorProjectionMutationAck,
        VectorProjectionMutationContext, VectorProjectionPrepareSnapshotRequest,
        VectorProjectionProtectedGeneration, VectorProjectionProtectionReason,
        VectorProjectionPublishRequest, VectorProjectionRepairPublicationRequest,
    };
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    fn assert_chunk_store<T: ChunkVectorStore>(_store: &T) {}
    fn assert_label_atom_store<T: LabelAtomVectorStore>(_store: &T) {}
    fn assert_vector_store<T: VectorStore>(_store: &T) {}

    fn projection_apply_request() -> VectorProjectionHelperRequest {
        let context = VectorProjectionMutationContext {
            request_id: "request-1".to_owned(),
            projection_store: "lancedb_chunks".to_owned(),
            generation_id: "generation-7".to_owned(),
            delivery_digest: "sha256:delivery-7".to_owned(),
        };
        let mut authority = projection_destructive_authority(&context);
        authority.fence_epoch = 8;
        authority.lease_token = "lease-token-secret".to_owned();
        VectorProjectionHelperRequest::ApplyBatch(VectorProjectionApplyBatchRequest {
            context,
            authority,
            batch: ProjectionBatch {
                store_name: "lancedb_chunks".to_owned(),
                database_instance_id: "database-1".to_owned(),
                protocol_version: 2,
                schema_version: 29,
                provider: "lancedb".to_owned(),
                provider_fingerprint: "sha256:provider".to_owned(),
                owner: "maintenance-owner".to_owned(),
                lease_token: "lease-token-secret".to_owned(),
                fence_epoch: 8,
                target_generation: "generation-7".to_owned(),
                claim_token: "claim-token-secret".to_owned(),
                claim_expires_at: 1_900_000_000_000,
                items: vec![ProjectionDelivery {
                    id: 11,
                    outbox_id: 7,
                    store_name: "lancedb_chunks".to_owned(),
                    generation_id: "generation-7".to_owned(),
                    board_id: "b_1".to_owned(),
                    source_event_id: Some(9),
                    cursor: 7,
                    action: ProjectionDeliveryAction::Upsert,
                    entity_uri: "kb://task/t_1".to_owned(),
                    payload_json: "{}".to_owned(),
                    attempts: 0,
                }],
            },
        })
    }

    fn projection_apply_response() -> VectorProjectionHelperResponse {
        VectorProjectionHelperResponse::ApplyBatch(VectorProjectionApplyBatchResponse {
            ack: VectorProjectionMutationAck {
                request_id: "request-1".to_owned(),
                projection_store: "lancedb_chunks".to_owned(),
                generation_id: "generation-7".to_owned(),
                delivery_digest: "sha256:delivery-7".to_owned(),
            },
            receipt: VectorProjectionBatchApplicationReceipt {
                store_name: "lancedb_chunks".to_owned(),
                database_instance_id: "database-1".to_owned(),
                protocol_version: 2,
                schema_version: 29,
                provider: "lancedb".to_owned(),
                provider_fingerprint: "sha256:provider".to_owned(),
                target_generation: "generation-7".to_owned(),
                fence_epoch: 8,
                applied_item_count: 1,
            },
        })
    }

    fn projection_destructive_context() -> VectorProjectionMutationContext {
        VectorProjectionMutationContext {
            request_id: "destructive-request-1".to_owned(),
            projection_store: "lancedb_chunks".to_owned(),
            generation_id: "generation-active".to_owned(),
            delivery_digest: "sha256:delivery-active".to_owned(),
        }
    }

    fn projection_destructive_authority(
        context: &VectorProjectionMutationContext,
    ) -> VectorProjectionDestructiveAuthority {
        let corpus = Some(ProjectionCorpusMetadata {
            corpus_schema: TASK_CHUNKS_CORPUS_SCHEMA.to_owned(),
            corpus_fingerprint: "sha256:corpus-active".to_owned(),
            embedding_model: "model-1".to_owned(),
            embedding_dimensions: 3,
        });
        let manifest = ProjectionArtifactManifest {
            store_name: context.projection_store.clone(),
            database_instance_id: "db_projection_1".to_owned(),
            protocol_version: 2,
            schema_version: 29,
            generation: context.generation_id.clone(),
            fence_epoch: 7,
            snapshot_cursor: 41,
            provider: "lancedb".to_owned(),
            provider_fingerprint: "sha256:provider".to_owned(),
            corpus: corpus.clone(),
            canonical_item_count: 2,
            canonical_digest: "sha256:canonical-active".to_owned(),
            delivery_item_count: 1,
            delivery_digest: context.delivery_digest.clone(),
            fingerprint: Some("sha256:generation-active".to_owned()),
        };
        VectorProjectionDestructiveAuthority {
            owner: "maintenance-owner".to_owned(),
            lease_token: "destructive-lease-token-secret".to_owned(),
            fence_epoch: 11,
            role: VectorProjectionGenerationRole::Active,
            generation: manifest.generation.clone(),
            expected_binding: Some(VectorProjectionGenerationBinding {
                generation: manifest.generation.clone(),
                fingerprint: manifest.fingerprint.clone(),
                fence_epoch: manifest.fence_epoch,
                snapshot_cursor: Some(manifest.snapshot_cursor),
                provider: manifest.provider.clone(),
                provider_fingerprint: manifest.provider_fingerprint.clone(),
                canonical_count: manifest.canonical_item_count,
                canonical_digest: manifest.canonical_digest.clone(),
                delivery_count: manifest.delivery_item_count,
                delivery_digest: manifest.delivery_digest.clone(),
                corpus,
            }),
            expected_manifest: Some(manifest),
            building_phase: None,
        }
    }

    fn projection_quarantine_request() -> VectorProjectionHelperRequest {
        let context = projection_destructive_context();
        VectorProjectionHelperRequest::Quarantine(VectorProjectionGenerationMutationRequest {
            authority: projection_destructive_authority(&context),
            context,
        })
    }

    fn projection_helper_descriptor() -> VectorProjectionHelperDescriptor {
        VectorProjectionHelperDescriptor {
            request_id: "descriptor-1".to_owned(),
            protocol_version: 2,
            build_identity: "build-1".to_owned(),
            supported_stores: vec![ProjectionStoreDescriptor {
                store_name: "lancedb_chunks".to_owned(),
                schema_version: 29,
                provider: "lancedb".to_owned(),
                provider_fingerprint: "sha256:provider".to_owned(),
                corpus: Some(ProjectionCorpusMetadata {
                    corpus_schema: "task-chunks-v2".to_owned(),
                    corpus_fingerprint: "sha256:chunks".to_owned(),
                    embedding_model: "model-1".to_owned(),
                    embedding_dimensions: 3,
                }),
            }],
            supported_operations: vec![
                VectorProjectionHelperOperation::Descriptor,
                VectorProjectionHelperOperation::ApplyBatch,
            ],
        }
    }

    #[test]
    fn projection_apply_items_are_bound_to_the_batch_store_and_generation() {
        let request = projection_apply_request();
        assert!(super::valid_projection_request_correlation(&request));

        let mut wrong_store = request.clone();
        let VectorProjectionHelperRequest::ApplyBatch(wrong_store_request) = &mut wrong_store
        else {
            unreachable!()
        };
        wrong_store_request.batch.items[0].store_name = "lancedb_label_atoms".to_owned();
        assert!(!super::valid_projection_request_correlation(&wrong_store));

        let mut wrong_generation = request;
        let VectorProjectionHelperRequest::ApplyBatch(wrong_generation_request) =
            &mut wrong_generation
        else {
            unreachable!()
        };
        wrong_generation_request.batch.items[0].generation_id = "generation-other".to_owned();
        assert!(!super::valid_projection_request_correlation(
            &wrong_generation
        ));
    }

    #[test]
    fn projection_destructive_authority_rejects_empty_capability_or_invalid_fence() {
        let request = projection_quarantine_request();
        assert!(super::valid_projection_request_correlation(&request));

        let mutations: [fn(&mut VectorProjectionDestructiveAuthority); 5] = [
            |authority: &mut VectorProjectionDestructiveAuthority| authority.owner.clear(),
            |authority: &mut VectorProjectionDestructiveAuthority| {
                authority.lease_token = " \t".to_owned();
            },
            |authority: &mut VectorProjectionDestructiveAuthority| authority.fence_epoch = -1,
            |authority: &mut VectorProjectionDestructiveAuthority| {
                authority.expected_binding.as_mut().unwrap().fence_epoch = -1;
            },
            |authority: &mut VectorProjectionDestructiveAuthority| {
                authority.expected_binding.as_mut().unwrap().fence_epoch =
                    authority.fence_epoch + 1;
            },
        ];
        for mutate in mutations {
            let mut invalid = request.clone();
            let VectorProjectionHelperRequest::Quarantine(request) = &mut invalid else {
                unreachable!()
            };
            mutate(&mut request.authority);
            assert!(!super::valid_projection_request_correlation(&invalid));
        }
    }

    #[test]
    fn projection_destructive_authority_rejects_role_binding_inconsistency() {
        let request = projection_quarantine_request();

        let mut missing_canonical_manifest = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut missing_canonical_manifest
        else {
            unreachable!()
        };
        mutation.authority.expected_manifest = None;
        assert!(!super::valid_projection_request_correlation(
            &missing_canonical_manifest
        ));

        let mut missing_canonical_binding = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut missing_canonical_binding
        else {
            unreachable!()
        };
        mutation.authority.expected_binding = None;
        assert!(!super::valid_projection_request_correlation(
            &missing_canonical_binding
        ));

        let mut active_with_building_phase = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut active_with_building_phase
        else {
            unreachable!()
        };
        mutation.authority.building_phase = Some(VectorProjectionBuildingPhase::Prepared);
        assert!(!super::valid_projection_request_correlation(
            &active_with_building_phase
        ));

        let mut building_without_phase = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut building_without_phase
        else {
            unreachable!()
        };
        mutation.authority.role = VectorProjectionGenerationRole::Building;
        assert!(!super::valid_projection_request_correlation(
            &building_without_phase
        ));

        let mut snapshotting_with_manifest = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut snapshotting_with_manifest
        else {
            unreachable!()
        };
        mutation.authority.role = VectorProjectionGenerationRole::Building;
        mutation.authority.building_phase = Some(VectorProjectionBuildingPhase::Snapshotting);
        assert!(!super::valid_projection_request_correlation(
            &snapshotting_with_manifest
        ));

        let mut prepared_without_manifest = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut prepared_without_manifest
        else {
            unreachable!()
        };
        mutation.authority.role = VectorProjectionGenerationRole::Building;
        mutation.authority.building_phase = Some(VectorProjectionBuildingPhase::Prepared);
        mutation.authority.expected_manifest = None;
        assert!(!super::valid_projection_request_correlation(
            &prepared_without_manifest
        ));

        let mut orphan_with_canonical_evidence = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) =
            &mut orphan_with_canonical_evidence
        else {
            unreachable!()
        };
        mutation.authority.role = VectorProjectionGenerationRole::Orphaned;
        assert!(!super::valid_projection_request_correlation(
            &orphan_with_canonical_evidence
        ));

        let mut orphan_with_building_phase = request;
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut orphan_with_building_phase
        else {
            unreachable!()
        };
        mutation.authority.role = VectorProjectionGenerationRole::Orphaned;
        mutation.authority.expected_manifest = None;
        mutation.authority.expected_binding = None;
        mutation.authority.building_phase = Some(VectorProjectionBuildingPhase::Snapshotting);
        assert!(!super::valid_projection_request_correlation(
            &orphan_with_building_phase
        ));
    }

    #[test]
    fn projection_destructive_authority_accepts_supported_role_binding_shapes() {
        let request = projection_quarantine_request();

        for role in [
            VectorProjectionGenerationRole::Active,
            VectorProjectionGenerationRole::Previous,
        ] {
            let mut candidate = request.clone();
            let VectorProjectionHelperRequest::Quarantine(mutation) = &mut candidate else {
                unreachable!()
            };
            mutation.authority.role = role;
            assert!(super::valid_projection_request_correlation(&candidate));
        }

        for phase in [
            VectorProjectionBuildingPhase::Prepared,
            VectorProjectionBuildingPhase::StorePublished,
        ] {
            let mut candidate = request.clone();
            let VectorProjectionHelperRequest::Quarantine(mutation) = &mut candidate else {
                unreachable!()
            };
            mutation.authority.role = VectorProjectionGenerationRole::Building;
            mutation.authority.building_phase = Some(phase);
            assert!(super::valid_projection_request_correlation(&candidate));
        }

        let mut snapshotting = request;
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut snapshotting else {
            unreachable!()
        };
        mutation.authority.role = VectorProjectionGenerationRole::Building;
        mutation.authority.building_phase = Some(VectorProjectionBuildingPhase::Snapshotting);
        mutation.authority.expected_manifest = None;
        let binding = mutation.authority.expected_binding.as_mut().unwrap();
        binding.fingerprint = None;
        binding.snapshot_cursor = None;
        assert!(super::valid_projection_request_correlation(&snapshotting));

        let mut orphaned = projection_quarantine_request();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut orphaned else {
            unreachable!()
        };
        mutation.context.generation_id = "orphaned-entry".to_owned();
        mutation.authority.role = VectorProjectionGenerationRole::Orphaned;
        mutation.authority.generation = "orphaned-entry".to_owned();
        mutation.authority.expected_manifest = None;
        mutation.authority.expected_binding = None;
        assert!(super::valid_projection_request_correlation(&orphaned));
    }

    #[test]
    fn projection_destructive_authority_rejects_context_and_exact_binding_drift() {
        let request = projection_quarantine_request();

        let mut generation_drift = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut generation_drift else {
            unreachable!()
        };
        mutation.authority.generation = "generation-other".to_owned();
        assert!(!super::valid_projection_request_correlation(
            &generation_drift
        ));

        let mut delivery_drift = request.clone();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut delivery_drift else {
            unreachable!()
        };
        mutation
            .authority
            .expected_binding
            .as_mut()
            .unwrap()
            .delivery_digest = "sha256:delivery-other".to_owned();
        assert!(!super::valid_projection_request_correlation(
            &delivery_drift
        ));

        let mut manifest_binding_drift = request;
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut manifest_binding_drift
        else {
            unreachable!()
        };
        mutation
            .authority
            .expected_binding
            .as_mut()
            .unwrap()
            .canonical_count += 1;
        assert!(!super::valid_projection_request_correlation(
            &manifest_binding_drift
        ));
    }

    #[test]
    fn projection_destructive_authority_redacts_token_from_bounded_diagnostics() {
        let quarantine = projection_quarantine_request();
        let VectorProjectionHelperRequest::Quarantine(quarantine_request) = &quarantine else {
            unreachable!()
        };
        let token = quarantine_request.authority.lease_token.clone();
        let abort = VectorProjectionHelperRequest::Abort(quarantine_request.clone());
        let cleanup = VectorProjectionHelperRequest::Cleanup(VectorProjectionCleanupRequest {
            context: quarantine_request.context.clone(),
            authority: quarantine_request.authority.clone(),
            dry_run: false,
            protection: VectorProjectionCleanupProtection {
                active_generation: Some("generation-active".to_owned()),
                previous_generation: None,
                building_generation: None,
                additional_generations: Vec::new(),
            },
        });

        for request in [quarantine, abort, cleanup] {
            let message =
                super::bounded_projection_message(&format!("helper echoed {token}"), &request, 240);
            assert!(!message.contains(&token));
            assert!(message.contains("[REDACTED]"));
        }
    }

    #[test]
    fn every_mutating_projection_request_redacts_its_authority_capability() {
        let context = projection_destructive_context();
        let authority = projection_destructive_authority(&context);
        let manifest = authority.expected_manifest.clone().unwrap();
        let evidence = ProjectionArtifactEvidence {
            fingerprint: manifest.fingerprint.clone().unwrap(),
            manifest: manifest.clone(),
        };
        let prepare = VectorProjectionHelperRequest::PrepareSnapshot(
            VectorProjectionPrepareSnapshotRequest {
                context: context.clone(),
                authority: authority.clone(),
                snapshot: ProjectionSnapshot {
                    manifest: manifest.clone(),
                    records: Vec::new(),
                },
                metadata: manifest.corpus.clone().unwrap(),
            },
        );
        let publish =
            VectorProjectionHelperRequest::Publish(Box::new(VectorProjectionPublishRequest {
                context: context.clone(),
                authority: authority.clone(),
                expected_active: None,
                prepared: evidence.clone(),
            }));
        let repair = VectorProjectionHelperRequest::RepairPublication(
            VectorProjectionRepairPublicationRequest {
                context: context.clone(),
                authority: authority.clone(),
                expected: evidence,
            },
        );
        let apply = projection_apply_request();
        let quarantine = projection_quarantine_request();
        let VectorProjectionHelperRequest::Quarantine(quarantine_request) = &quarantine else {
            unreachable!()
        };
        let abort = VectorProjectionHelperRequest::Abort(quarantine_request.clone());
        let cleanup = VectorProjectionHelperRequest::Cleanup(VectorProjectionCleanupRequest {
            context: quarantine_request.context.clone(),
            authority: quarantine_request.authority.clone(),
            dry_run: false,
            protection: VectorProjectionCleanupProtection {
                active_generation: Some("generation-active".to_owned()),
                previous_generation: None,
                building_generation: None,
                additional_generations: Vec::new(),
            },
        });

        for (request, capabilities) in [
            (prepare, vec!["destructive-lease-token-secret"]),
            (
                apply,
                vec!["lease-token-secret", "claim-token-secret"],
            ),
            (publish, vec!["destructive-lease-token-secret"]),
            (repair, vec!["destructive-lease-token-secret"]),
            (quarantine, vec!["destructive-lease-token-secret"]),
            (abort, vec!["destructive-lease-token-secret"]),
            (cleanup, vec!["destructive-lease-token-secret"]),
        ] {
            let message = super::bounded_projection_message(
                &format!(
                    "helper echoed {}",
                    capabilities
                        .iter()
                        .map(|capability| capability.to_string())
                        .collect::<Vec<_>>()
                        .join(" and ")
                ),
                &request,
                240,
            );
            for capability in capabilities {
                assert!(!message.contains(capability), "{message}");
            }
            assert!(message.contains("[REDACTED]"), "{message}");
        }
    }

    #[test]
    fn projection_descriptor_fences_operation_protocol_schema_and_store() {
        let request = projection_apply_request();
        let descriptor = projection_helper_descriptor();
        super::validate_projection_request_against_descriptor(&request, &descriptor).unwrap();

        let mut missing_operation = descriptor.clone();
        missing_operation
            .supported_operations
            .retain(|operation| *operation != VectorProjectionHelperOperation::ApplyBatch);
        assert!(
            super::validate_projection_request_against_descriptor(&request, &missing_operation)
                .is_err()
        );

        let mut wrong_protocol = descriptor.clone();
        wrong_protocol.protocol_version = 3;
        assert!(
            super::validate_projection_request_against_descriptor(&request, &wrong_protocol)
                .is_err()
        );

        let mut wrong_schema = descriptor.clone();
        wrong_schema.supported_stores[0].schema_version = 30;
        assert!(
            super::validate_projection_request_against_descriptor(&request, &wrong_schema).is_err()
        );

        let mut wrong_store = descriptor;
        wrong_store.supported_stores[0].store_name = "lancedb_label_atoms".to_owned();
        assert!(
            super::validate_projection_request_against_descriptor(&request, &wrong_store).is_err()
        );
    }

    #[test]
    fn projection_destructive_descriptor_uses_store_identity_not_historical_provider() {
        let mut request = projection_quarantine_request();
        let VectorProjectionHelperRequest::Quarantine(mutation) = &mut request else {
            unreachable!()
        };
        let historical_corpus = Some(ProjectionCorpusMetadata {
            corpus_schema: TASK_CHUNKS_CORPUS_SCHEMA.to_owned(),
            corpus_fingerprint: "sha256:historical-corpus".to_owned(),
            embedding_model: "historical-model".to_owned(),
            embedding_dimensions: 7,
        });
        let binding = mutation.authority.expected_binding.as_mut().unwrap();
        binding.provider = "historical-provider".to_owned();
        binding.provider_fingerprint = "sha256:historical-provider".to_owned();
        binding.corpus = historical_corpus.clone();
        let manifest = mutation.authority.expected_manifest.as_mut().unwrap();
        manifest.provider = "historical-provider".to_owned();
        manifest.provider_fingerprint = "sha256:historical-provider".to_owned();
        manifest.corpus = historical_corpus;

        let mut descriptor = projection_helper_descriptor();
        descriptor.supported_operations = vec![
            VectorProjectionHelperOperation::Descriptor,
            VectorProjectionHelperOperation::Quarantine,
        ];
        super::validate_projection_request_against_descriptor(&request, &descriptor).unwrap();
    }

    #[test]
    fn projection_cleanup_response_cannot_remove_protected_or_dry_run_generations() {
        let context = VectorProjectionMutationContext {
            request_id: "cleanup-1".to_owned(),
            projection_store: "lancedb_chunks".to_owned(),
            generation_id: "generation-active".to_owned(),
            delivery_digest: "sha256:cleanup".to_owned(),
        };
        let request = VectorProjectionHelperRequest::Cleanup(VectorProjectionCleanupRequest {
            context: context.clone(),
            authority: projection_destructive_authority(&context),
            dry_run: true,
            protection: VectorProjectionCleanupProtection {
                active_generation: Some("generation-active".to_owned()),
                previous_generation: Some("generation-previous".to_owned()),
                building_generation: None,
                additional_generations: vec!["generation-pinned".to_owned()],
            },
        });
        let response = VectorProjectionHelperResponse::Cleanup(VectorProjectionCleanupResponse {
            ack: VectorProjectionMutationAck {
                request_id: context.request_id,
                projection_store: context.projection_store,
                generation_id: context.generation_id,
                delivery_digest: context.delivery_digest,
            },
            dry_run: true,
            removed_generations: Vec::new(),
            protected_generations: vec![
                VectorProjectionProtectedGeneration {
                    generation_id: "generation-active".to_owned(),
                    reason: VectorProjectionProtectionReason::Active,
                },
                VectorProjectionProtectedGeneration {
                    generation_id: "generation-previous".to_owned(),
                    reason: VectorProjectionProtectionReason::Previous,
                },
                VectorProjectionProtectedGeneration {
                    generation_id: "generation-pinned".to_owned(),
                    reason: VectorProjectionProtectionReason::Explicit,
                },
            ],
            skipped_generations: Vec::new(),
        });
        super::validate_projection_response(&request, &response).unwrap();

        let mut removed_during_dry_run = response.clone();
        let VectorProjectionHelperResponse::Cleanup(dry_run_response) = &mut removed_during_dry_run
        else {
            unreachable!()
        };
        dry_run_response
            .removed_generations
            .push("generation-orphan".to_owned());
        assert!(super::validate_projection_response(&request, &removed_during_dry_run).is_err());

        let mut removed_protected = response;
        let VectorProjectionHelperResponse::Cleanup(protected_response) = &mut removed_protected
        else {
            unreachable!()
        };
        protected_response.dry_run = false;
        protected_response
            .removed_generations
            .push("generation-active".to_owned());
        let VectorProjectionHelperRequest::Cleanup(mut live_request) = request else {
            unreachable!()
        };
        live_request.dry_run = false;
        assert!(
            super::validate_projection_response(
                &VectorProjectionHelperRequest::Cleanup(live_request),
                &removed_protected,
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    struct FakeProjectionHelper {
        root: PathBuf,
        helper_path: PathBuf,
        args_path: PathBuf,
        stdin_path: PathBuf,
    }

    #[cfg(unix)]
    impl FakeProjectionHelper {
        fn new(body: &str) -> Self {
            static COUNTER: AtomicU64 = AtomicU64::new(0);

            let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "kanban-vector-projection-client-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            let helper_path = root.join("fake-vector-helper");
            let args_path = root.join("args");
            let stdin_path = root.join("stdin");
            let script = format!(
                "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$@\" > '{}'\ncat > '{}'\n{body}\n",
                args_path.display(),
                stdin_path.display()
            );
            fs::write(&helper_path, script).unwrap();
            let mut permissions = fs::metadata(&helper_path).unwrap().permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&helper_path, permissions).unwrap();
            Self {
                root,
                helper_path,
                args_path,
                stdin_path,
            }
        }

        fn responding(response: &VectorProjectionHelperResponse) -> Self {
            let json = serde_json::to_string(response).unwrap();
            Self::new(&format!("printf '%s' '{json}'"))
        }

        fn client(&self) -> SubprocessVectorProjectionClient {
            SubprocessVectorProjectionClient::new(
                &self.helper_path,
                self.root.join("db.sqlite"),
                Some(self.root.join("vector.toml")),
            )
            .with_timeout(Duration::from_secs(10))
            .with_limits(VectorProjectionClientLimits {
                max_stdin_bytes: 16 * 1024,
                max_stdout_bytes: 16 * 1024,
                max_stderr_bytes: 16 * 1024,
            })
        }

        fn args(&self) -> String {
            fs::read_to_string(&self.args_path).unwrap()
        }

        fn stdin(&self) -> String {
            fs::read_to_string(&self.stdin_path).unwrap()
        }
    }

    #[cfg(unix)]
    impl Drop for FakeProjectionHelper {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    fn assert_projection_error(
        error: VectorError,
        expected_kind: VectorProjectionHelperErrorKind,
        expected_code: &str,
        retryable: bool,
    ) -> VectorError {
        match &error {
            VectorError::ProjectionHelper(failure) => {
                assert_eq!(failure.kind, expected_kind, "{error:?}");
                assert_eq!(failure.code, expected_code, "{error:?}");
                assert_eq!(failure.retryable, retryable, "{error:?}");
            }
            other => panic!("unexpected error: {other:?}"),
        }
        error
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_uses_stdin_and_validates_capability_free_apply_response() {
        let request = projection_apply_request();
        let response = projection_apply_response();
        let helper = FakeProjectionHelper::responding(&response);
        let client = helper.client();

        assert_eq!(client.helper_path(), helper.helper_path.as_path());
        assert_eq!(client.execute(&request).unwrap(), response);

        let args = helper.args();
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            [
                "projection",
                "--db",
                helper.root.join("db.sqlite").to_str().unwrap(),
                "--vector-config",
                helper.root.join("vector.toml").to_str().unwrap(),
            ]
        );
        for capability in [
            "lease-token-secret",
            "claim-token-secret",
            "request-1",
            "sha256:delivery-7",
        ] {
            assert!(!args.contains(capability), "{args}");
        }

        let stdin = helper.stdin();
        assert!(stdin.contains("lease-token-secret"), "{stdin}");
        assert!(stdin.contains("claim-token-secret"), "{stdin}");
        let stdout_contract = serde_json::to_string(&response).unwrap();
        assert!(
            !stdout_contract.contains("lease_token"),
            "{stdout_contract}"
        );
        assert!(
            !stdout_contract.contains("claim_token"),
            "{stdout_contract}"
        );
        assert!(!format!("{client:?}").contains("token-secret"));
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_rejects_response_operation_mismatch() {
        let response =
            VectorProjectionHelperResponse::Descriptor(VectorProjectionHelperDescriptor {
                request_id: "request-1".to_owned(),
                protocol_version: 2,
                build_identity: "build-1".to_owned(),
                supported_stores: Vec::new(),
                supported_operations: Vec::new(),
            });
        let helper = FakeProjectionHelper::responding(&response);

        let error = helper
            .client()
            .execute(&projection_apply_request())
            .unwrap_err();
        assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Protocol,
            "operation_mismatch",
            false,
        );
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_rejects_response_correlation_mismatch() {
        let mut cases = Vec::new();
        for field in [
            "request_id",
            "projection_store",
            "generation_id",
            "delivery_digest",
        ] {
            let mut response = projection_apply_response();
            let VectorProjectionHelperResponse::ApplyBatch(apply_response) = &mut response else {
                unreachable!()
            };
            match field {
                "request_id" => apply_response.ack.request_id = "request-other".to_owned(),
                "projection_store" => {
                    apply_response.ack.projection_store = "lancedb_label_atoms".to_owned()
                }
                "generation_id" => apply_response.ack.generation_id = "generation-other".to_owned(),
                "delivery_digest" => apply_response.ack.delivery_digest = "sha256:other".to_owned(),
                _ => unreachable!(),
            }
            cases.push((field, response));
        }

        for (field, response) in cases {
            let helper = FakeProjectionHelper::responding(&response);
            let error = helper
                .client()
                .execute(&projection_apply_request())
                .unwrap_err();
            let error = assert_projection_error(
                error,
                VectorProjectionHelperErrorKind::Protocol,
                "correlation_mismatch",
                false,
            );
            let VectorError::ProjectionHelper(failure) = error else {
                unreachable!()
            };
            assert_eq!(failure.request_id.as_deref(), Some("request-1"), "{field}");
            assert_eq!(
                failure.projection_store.as_deref(),
                Some("lancedb_chunks"),
                "{field}"
            );
            assert_eq!(
                failure.generation_id.as_deref(),
                Some("generation-7"),
                "{field}"
            );
            assert_eq!(
                failure.delivery_digest.as_deref(),
                Some("sha256:delivery-7"),
                "{field}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_preserves_structured_error_and_redacts_capabilities() {
        let response = VectorProjectionHelperResponse::Error(VectorProjectionHelperError {
            kind: VectorProjectionHelperErrorKind::Provider,
            code: "provider_rate_limited".to_owned(),
            provider: Some("ollama".to_owned()),
            backend: Some("lancedb".to_owned()),
            retryable: true,
            message: "retry lease-token-secret and claim-token-secret".to_owned(),
            request_id: Some("request-1".to_owned()),
            delivery_digest: Some("sha256:delivery-7".to_owned()),
            projection_store: Some("lancedb_chunks".to_owned()),
            generation_id: Some("generation-7".to_owned()),
        });
        let helper = FakeProjectionHelper::responding(&response);

        let error = helper
            .client()
            .execute(&projection_apply_request())
            .unwrap_err();
        assert!(error.is_retryable());
        let rendered = error.to_string();
        assert!(!rendered.contains("lease-token-secret"), "{rendered}");
        assert!(!rendered.contains("claim-token-secret"), "{rendered}");
        let error = assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Provider,
            "provider_rate_limited",
            true,
        );
        let VectorError::ProjectionHelper(failure) = error else {
            unreachable!()
        };
        assert_eq!(failure.provider.as_deref(), Some("ollama"));
        assert_eq!(failure.backend.as_deref(), Some("lancedb"));
        assert_eq!(failure.message, "retry [REDACTED] and [REDACTED]");
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_rejects_mismatched_structured_error_correlation() {
        let response = VectorProjectionHelperResponse::Error(VectorProjectionHelperError {
            kind: VectorProjectionHelperErrorKind::Delivery,
            code: "delivery_rejected".to_owned(),
            provider: None,
            backend: Some("lancedb".to_owned()),
            retryable: false,
            message: "rejected".to_owned(),
            request_id: Some("request-other".to_owned()),
            delivery_digest: Some("sha256:delivery-7".to_owned()),
            projection_store: Some("lancedb_chunks".to_owned()),
            generation_id: Some("generation-7".to_owned()),
        });
        let helper = FakeProjectionHelper::responding(&response);

        let error = helper
            .client()
            .execute(&projection_apply_request())
            .unwrap_err();
        assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Protocol,
            "correlation_mismatch",
            false,
        );
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_rejects_oversize_stdin_before_spawn() {
        let helper = FakeProjectionHelper::responding(&projection_apply_response());
        let client = helper.client().with_limits(VectorProjectionClientLimits {
            max_stdin_bytes: 32,
            max_stdout_bytes: 16 * 1024,
            max_stderr_bytes: 16 * 1024,
        });

        let error = client.execute(&projection_apply_request()).unwrap_err();
        assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Protocol,
            "request_too_large",
            false,
        );
        assert!(!helper.args_path.exists());
        assert!(!helper.stdin_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_times_out_then_kills_and_waits_for_child() {
        let helper = FakeProjectionHelper::new("while :; do :; done");
        let client = helper.client().with_timeout(Duration::from_millis(100));
        let started = Instant::now();

        let error = client.execute(&projection_apply_request()).unwrap_err();
        assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Backend,
            "timeout",
            true,
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_kills_child_when_stdout_exceeds_limit() {
        let helper = FakeProjectionHelper::new("while :; do printf '0123456789abcdef'; done");
        let client = helper.client().with_limits(VectorProjectionClientLimits {
            max_stdin_bytes: 16 * 1024,
            max_stdout_bytes: 64,
            max_stderr_bytes: 16 * 1024,
        });

        let error = client.execute(&projection_apply_request()).unwrap_err();
        assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Backend,
            "stdout_too_large",
            false,
        );
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_kills_child_when_stderr_exceeds_limit() {
        let helper = FakeProjectionHelper::new("while :; do printf '0123456789abcdef' >&2; done");
        let client = helper.client().with_limits(VectorProjectionClientLimits {
            max_stdin_bytes: 16 * 1024,
            max_stdout_bytes: 16 * 1024,
            max_stderr_bytes: 64,
        });

        let error = client.execute(&projection_apply_request()).unwrap_err();
        assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Backend,
            "stderr_too_large",
            false,
        );
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_requires_exactly_one_stdout_envelope() {
        let response = serde_json::to_string(&projection_apply_response()).unwrap();
        let helper = FakeProjectionHelper::new(&format!("printf '%s' '{response}{response}'"));

        let error = helper
            .client()
            .execute(&projection_apply_request())
            .unwrap_err();
        assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Protocol,
            "invalid_response",
            false,
        );
    }

    #[cfg(unix)]
    #[test]
    fn projection_client_bounds_spawn_error_without_echoing_argv() {
        let helper = FakeProjectionHelper::responding(&projection_apply_response());
        let missing = helper.root.join("x".repeat(1_024));
        let client = SubprocessVectorProjectionClient::new(
            missing,
            helper.root.join("db.sqlite"),
            Some(helper.root.join("vector.toml")),
        );

        let error = client.execute(&projection_apply_request()).unwrap_err();
        let rendered = error.to_string();
        assert_projection_error(
            error,
            VectorProjectionHelperErrorKind::Backend,
            "spawn_failed",
            true,
        );
        assert!(rendered.len() <= 512, "{}", rendered.len());
        assert!(!rendered.contains("lease-token-secret"), "{rendered}");
        assert!(!rendered.contains("claim-token-secret"), "{rendered}");
    }

    #[test]
    fn disabled_store_implements_split_vector_traits() {
        let store = DisabledVectorStore;

        assert_chunk_store(&store);
        assert_label_atom_store(&store);
        assert_vector_store(&store);
    }

    #[test]
    fn disabled_vector_store_rejects_writes() {
        let store = DisabledVectorStore;

        assert!(matches!(store.upsert(&[]), Err(VectorError::Disabled)));
        assert!(matches!(
            store.delete_entities(&["kb://task/t_1".to_owned()]),
            Err(VectorError::Disabled)
        ));
        assert_eq!(
            store
                .query(&super::VectorQuery {
                    text: "x".into(),
                    limit: 1,
                    board_id: "b_1".into(),
                })
                .unwrap(),
            Vec::new()
        );
        assert!(!store.status().enabled);
    }

    #[test]
    fn subprocess_status_scope_selects_independent_store_commands() {
        let chunks = crate::SubprocessVectorStore::new(
            "/tmp/vector-helper",
            "/tmp/kanban.db",
            "default",
            None,
        );
        let labels = chunks
            .clone()
            .with_status_scope(crate::VectorStatusScope::LabelAtoms);

        assert_eq!(chunks.status_command(), "status");
        assert_eq!(labels.status_command(), "label-atoms-status");
    }

    #[test]
    fn chunk_builder_uses_chunk_uri_and_model_as_stable_key() {
        let builder = ChunkBuilder::new("test-model");
        let chunks = builder
            .build_task_chunks(&TaskChunkSource {
                task_uri: "kb://task/t_1".to_owned(),
                project_id: Some("project-a".to_owned()),
                board_id: Some("b_1".to_owned()),
                task_id: "t_1".to_owned(),
                title: "Title".to_owned(),
                description: Some("Spec body".to_owned()),
                comments: String::new(),
                run_text: String::new(),
                event_text: String::new(),
                source_event_id: Some(7),
                created_at: 41,
                updated_at: 42,
            })
            .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk.uri.as_str(), "kb://chunk/task/t_1/0");
        assert_eq!(chunks[0].chunk.entity_uri.as_str(), "kb://task/t_1");
        assert_eq!(chunks[0].chunk_key(), "kb://chunk/task/t_1/0#test-model");
        assert_eq!(chunks[0].text, "Title\n\nSpec body");
        assert_eq!(chunks[0].source_table, "tasks");
        assert_eq!(chunks[0].source_id, "t_1");
        assert_eq!(chunks[0].source_event_id, Some(7));
    }

    #[test]
    fn chunk_content_hash_is_sha256_of_normalized_semantic_text() {
        let builder = ChunkBuilder::new("test-model");
        let source = TaskChunkSource {
            task_uri: "kb://task/t_1".to_owned(),
            project_id: Some("project-a".to_owned()),
            board_id: Some("b_1".to_owned()),
            task_id: "t_1".to_owned(),
            title: "Title".to_owned(),
            description: Some("Spec body".to_owned()),
            comments: String::new(),
            run_text: String::new(),
            event_text: String::new(),
            source_event_id: Some(7),
            created_at: 41,
            updated_at: 42,
        };
        let mut whitespace_variant = source.clone();
        whitespace_variant.title = "  Title  ".to_owned();
        whitespace_variant.description = Some("Spec   body\n".to_owned());

        let first = builder.build_task_chunks(&source).unwrap();
        let second = builder.build_task_chunks(&whitespace_variant).unwrap();
        let first_hash = first[0].chunk.content_hash.as_deref().unwrap();

        assert!(first_hash.starts_with("sha256:"), "{first_hash}");
        assert_eq!(first_hash.len(), "sha256:".len() + 64);
        assert_eq!(first_hash, second[0].chunk.content_hash.as_deref().unwrap());
        assert_eq!(first[0].chunk.uri, second[0].chunk.uri);
    }

    #[test]
    fn chunk_builder_uses_stable_section_identities_for_task_corpus() {
        let builder = ChunkBuilder::new("test-model");
        let chunks = builder
            .build_task_chunks(&TaskChunkSource {
                task_uri: "kb://task/t_1".to_owned(),
                project_id: None,
                board_id: Some("b_1".to_owned()),
                task_id: "t_1".to_owned(),
                title: "Title".to_owned(),
                description: Some("Spec body".to_owned()),
                comments: "first comment\nsecond comment".to_owned(),
                run_text: "run completed".to_owned(),
                event_text: "task.created".to_owned(),
                source_event_id: Some(7),
                created_at: 41,
                updated_at: 42,
            })
            .unwrap();

        assert_eq!(chunks.len(), 4);
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.chunk.uri.as_str())
                .collect::<Vec<_>>(),
            [
                "kb://chunk/task/t_1/0",
                "kb://chunk/task/t_1/1",
                "kb://chunk/task/t_1/2",
                "kb://chunk/task/t_1/3",
            ]
        );
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.source_table.as_str())
                .collect::<Vec<_>>(),
            ["tasks", "task_comments", "task_runs", "task_events"]
        );
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.chunk.ordinal)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
    }

    #[test]
    fn dimension_mismatch_is_explicit_error() {
        let err = ensure_dimensions(&[1.0, 2.0], 3).unwrap_err();
        assert!(matches!(
            err,
            VectorError::DimensionMismatch {
                expected: 3,
                actual: 2
            }
        ));
    }

    #[test]
    fn provider_fingerprint_binds_provider_model_and_dimensions() {
        let baseline = embedding_provider_fingerprint("ollama", "model-a", 768);

        assert!(baseline.starts_with("sha256:"), "{baseline}");
        assert_ne!(
            baseline,
            embedding_provider_fingerprint("other-provider", "model-a", 768)
        );
        assert_ne!(
            baseline,
            embedding_provider_fingerprint("ollama", "model-b", 768)
        );
        assert_ne!(
            baseline,
            embedding_provider_fingerprint("ollama", "model-a", 1024)
        );
    }

    #[test]
    fn corpus_fingerprint_is_independent_for_chunks_and_label_atoms() {
        let provider = embedding_provider_fingerprint("ollama", "model-a", 768);
        let chunks = corpus_provider_fingerprint(TASK_CHUNKS_CORPUS_SCHEMA, &provider);
        let label_atoms = corpus_provider_fingerprint(LABEL_ATOMS_CORPUS_SCHEMA, &provider);

        assert!(chunks.starts_with("sha256:"), "{chunks}");
        assert_ne!(chunks, label_atoms);
    }

    #[test]
    fn label_atoms_corpus_metadata_is_canonical_and_provider_bound() {
        let metadata = label_atoms_corpus_metadata("ollama", "model-a", 768).unwrap();
        assert_eq!(metadata.corpus_schema, LABEL_ATOMS_CORPUS_SCHEMA);
        assert_eq!(metadata.embedding_model, "model-a");
        assert_eq!(metadata.embedding_dimensions, 768);
        assert_eq!(
            metadata.corpus_fingerprint,
            corpus_provider_fingerprint(
                LABEL_ATOMS_CORPUS_SCHEMA,
                &embedding_provider_fingerprint("ollama", "model-a", 768),
            )
        );
        assert_ne!(
            metadata.corpus_fingerprint,
            label_atoms_corpus_metadata("other", "model-a", 768)
                .unwrap()
                .corpus_fingerprint
        );
    }

    #[test]
    fn label_atoms_corpus_metadata_rejects_invalid_identity_inputs() {
        for (provider, model, dimensions, field) in [
            ("", "model-a", 768, "provider"),
            (" \t\n", "model-a", 768, "provider"),
            ("ollama", "", 768, "model"),
            ("ollama", " \t\n", 768, "model"),
            ("ollama", "model-a", 0, "dimensions"),
        ] {
            let error = label_atoms_corpus_metadata(provider, model, dimensions).unwrap_err();
            let VectorError::Provider { message, retryable } = error else {
                panic!("unexpected validation error for {field}: {error}");
            };
            assert!(!retryable);
            assert!(message.contains(&format!("field {field}:")), "{message}");
        }
    }
}
