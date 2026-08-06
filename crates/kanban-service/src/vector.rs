//! Turso 原生向量投影的 canonical persistence。
//!
//! 这里不依赖任何外部向量数据库。`retrieval_documents` 和
//! `retrieval_vectors` 是可删除、可重建的 projection；配置、任务和生成状态
//! 仍然保存在同一个 Turso 数据库中。

use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use turso::{Value, transaction::TransactionBehavior};

use crate::{
    db::TursoStore,
    domain::LabelAtomRecord,
    error::StoreError,
    shared::{
        first_row, integer_value, now_ms, optional_integer_value, optional_text_value, text_value,
    },
    store_operations::{AtomBuildInput, BootstrapTaskLabelInput, build_atoms},
    suggestion_engine::{AtomKind, AtomPolarity, RetrievedAtom, SolverConfig, resolve_from_atoms},
};

pub(crate) const VECTOR_TASKS_PROJECTION: &str = "vector_tasks";
pub(crate) const VECTOR_LABEL_ATOMS_PROJECTION: &str = "vector_label_atoms";
pub(crate) const VECTOR_BACKEND: &str = "turso-vector32";
pub(crate) const MAX_VECTOR_BATCH: usize = 64;
pub(crate) const MAX_VECTOR_DIMENSIONS: usize = 16_384;
pub(crate) const MAX_VECTOR_CONTENT_BYTES: usize = 1_048_576;

const VECTOR_JOB_LEASE_MS: i64 = 30_000;

const OLLAMA_READ_TIMEOUT: Duration = Duration::from_secs(30);
const OLLAMA_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OLLAMA_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// host 侧 Ollama 配置。该配置只描述 provider，不改变 canonical 事实。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct VectorConfig {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub dimensions: usize,
}

impl VectorConfig {
    pub(crate) fn validate(&self) -> Result<(), StoreError> {
        if !self.provider.eq_ignore_ascii_case("ollama") {
            return Err(StoreError::InvalidInput(
                "vector provider 目前必须是 ollama".to_owned(),
            ));
        }
        if self.endpoint.trim().is_empty() || !self.endpoint.starts_with("http://") {
            return Err(StoreError::InvalidInput(
                "Ollama endpoint 必须是非空 http:// URL".to_owned(),
            ));
        }
        let authority = self
            .endpoint
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or_default();
        let host = authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host)
            .trim_matches(['[', ']']);
        if authority.is_empty()
            || authority.contains('@')
            || !matches!(host, "localhost" | "127.0.0.1" | "::1")
        {
            return Err(StoreError::InvalidInput(
                "Ollama endpoint 必须指向 loopback host".to_owned(),
            ));
        }
        if self.model.trim().is_empty() {
            return Err(StoreError::InvalidInput(
                "embedding model 不能为空".to_owned(),
            ));
        }
        if self.model.len() > 256 {
            return Err(StoreError::InvalidInput(
                "embedding model 长度不能超过 256".to_owned(),
            ));
        }
        if self.dimensions == 0 || self.dimensions > MAX_VECTOR_DIMENSIONS {
            return Err(StoreError::InvalidInput(format!(
                "embedding dimensions 必须在 1..={MAX_VECTOR_DIMENSIONS} 内"
            )));
        }
        Ok(())
    }

    pub(crate) fn fingerprint(&self) -> String {
        provider_fingerprint(&self.provider, &self.model, self.dimensions)
    }
}

/// projection_state 的稳定读取 DTO。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VectorStatusRecord {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    pub diagnostics: Vec<String>,
    pub dirty: Option<bool>,
    pub board_dirty: Option<bool>,
    pub generation: Option<i64>,
    pub provider: Option<String>,
    pub provider_fingerprint: Option<String>,
    pub model: Option<String>,
    pub dimensions: Option<usize>,
    pub pending_jobs: i64,
    pub running_jobs: i64,
    pub failed_jobs: i64,
}

/// 由 canonical 事实构造的文本文档。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VectorDocumentInput {
    pub id: String,
    pub board_id: String,
    pub entity_uri: Option<String>,
    pub source_kind: String,
    pub content: String,
    pub content_hash: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 一个可投影的向量值。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VectorEmbeddingInput {
    pub id: String,
    pub board_id: String,
    pub entity_uri: Option<String>,
    pub document_id: String,
    pub embedding: Vec<f32>,
    pub dimensions: usize,
    pub embedding_model: String,
    pub content_hash: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectionJobRecord {
    pub id: i64,
    pub board_id: Option<String>,
    pub source_event_id: Option<i64>,
    pub target: String,
    pub entity_uri: Option<String>,
    pub operation: String,
    pub payload_json: String,
    pub attempts: i64,
    pub max_attempts: i64,
    pub lease_owner: Option<String>,
    pub lease_token: Option<String>,
    pub fence_epoch: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VectorChunkHitRecord {
    pub id: String,
    pub board_id: Option<String>,
    pub entity_uri: Option<String>,
    pub source_kind: String,
    pub content: String,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VectorLabelAtomHitRecord {
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
    pub vector: Option<Vec<f32>>,
}

impl TursoStore {
    /// 为一个 board 的 canonical task/label-atom 快照补齐 vector projection job。
    ///
    /// `rebuild` 会重新排队全部事实；`sync` 复用 dedupe key，只补齐缺失或已完成后
    /// 又被 canonical trigger 标脏的条目。provider 不可用时 job 保留 pending，供
    /// 后续 host worker resume，不能把 projection_state 提前发布为 ready。
    pub async fn enqueue_vector_projection_jobs(
        &self,
        board_selector: &str,
        rebuild: bool,
    ) -> Result<u64, StoreError> {
        let board_id = self.vector_board_id(board_selector).await?;
        let operation = if rebuild { "rebuild" } else { "upsert" };
        let task_ids = self.vector_task_ids(&board_id).await?;
        let atom_ids = self.vector_label_atom_ids(&board_id).await?;
        let mut count = 0_u64;
        for task_id in task_ids {
            let uri = format!("kb://task/{task_id}");
            let payload = format!(r#"{{"task_id":"{task_id}"}}"#);
            self.enqueue_vector_job(
                Some(&board_id),
                None,
                VECTOR_TASKS_PROJECTION,
                &uri,
                operation,
                &payload,
            )
            .await?;
            count = count.saturating_add(1);
        }
        for atom_id in atom_ids {
            let uri = format!("kb://label-atom/{atom_id}");
            let payload = format!(r#"{{"atom_id":"{atom_id}"}}"#);
            self.enqueue_vector_job(
                Some(&board_id),
                None,
                VECTOR_LABEL_ATOMS_PROJECTION,
                &uri,
                operation,
                &payload,
            )
            .await?;
            count = count.saturating_add(1);
        }
        let provider_configured = self.vector_config().await?.is_some();
        let message = if provider_configured {
            "vector job pending，等待 host worker".to_owned()
        } else {
            "vector provider 未配置，job 保持 pending".to_owned()
        };
        if count > 0 || !provider_configured {
            let connection = self.connection().await?;
            connection
                .execute(
                    "UPDATE projection_state SET lifecycle_status='degraded', dirty=1, last_error=?1, updated_at=?2 WHERE projection IN ('vector_tasks','vector_label_atoms')",
                    (message.as_str(), crate::shared::now_ms()),
                )
                .await?;
        }
        Ok(count)
    }

    /// 设置 provider/model/dimension，并清除旧 generation 的读 authority。
    pub async fn configure_vector(
        &self,
        config: &VectorConfig,
    ) -> Result<VectorStatusRecord, StoreError> {
        config.validate()?;
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let now = now_ms();
        let fingerprint = config.fingerprint();
        let config_json = serde_json::to_string(config).map_err(|error| {
            StoreError::InvalidInput(format!("vector config 序列化失败: {error}"))
        })?;
        for projection in [VECTOR_TASKS_PROJECTION, VECTOR_LABEL_ATOMS_PROJECTION] {
            transaction
                .execute(
                    "UPDATE projection_state SET lifecycle_status='degraded', active_generation=NULL, active_fingerprint=NULL, building_generation=NULL, building_fingerprint=NULL, provider=?2, provider_fingerprint=?3, corpus_schema=?4, corpus_fingerprint=?5, embedding_model=?6, embedding_dimensions=?7, dirty=1, last_error=NULL, updated_at=?8 WHERE projection=?1",
                    (
                        projection,
                        config.provider.trim().to_ascii_lowercase(),
                        fingerprint.as_str(),
                        projection,
                        corpus_fingerprint(projection, &fingerprint),
                        config.model.as_str(),
                        i64::try_from(config.dimensions).map_err(|_| StoreError::InvalidInput("embedding dimensions 太大".to_owned()))?,
                        now,
                    ),
                )
                .await?;
        }
        transaction
            .execute(
                "INSERT INTO app_settings(key, value_json, updated_at) VALUES ('vector.config', ?1, ?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json, updated_at=excluded.updated_at",
                (config_json.as_str(), now),
            )
            .await?;
        transaction.commit().await?;
        self.vector_status(None).await
    }

    /// 读取 host worker 使用的 provider 配置；没有配置时返回 `None`。
    pub async fn vector_config(&self) -> Result<Option<VectorConfig>, StoreError> {
        let connection = self.connection().await?;
        let row = match first_row(
            connection
                .query(
                    "SELECT value_json FROM app_settings WHERE key='vector.config'",
                    (),
                )
                .await?,
        )
        .await
        {
            Ok(row) => row,
            Err(turso::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(StoreError::Turso(error)),
        };
        let payload = text_value(row.get_value(0)?, "app_settings.value_json")?;
        let config = serde_json::from_str::<VectorConfig>(&payload).map_err(|error| {
            StoreError::SchemaMismatch(format!("vector config 无法解析: {error}"))
        })?;
        config.validate()?;
        Ok(Some(config))
    }

    /// 从 canonical task 事实生成一个待 embedding 的文档；不存在 task 时返回 `None`。
    pub async fn vector_task_document(
        &self,
        task_id: &str,
    ) -> Result<Option<VectorDocumentInput>, StoreError> {
        let connection = self.connection().await?;
        let row = match first_row(
            connection
                .query(
                    "SELECT board_id, title, description, created_at, updated_at FROM tasks WHERE id=?1",
                    [task_id],
                )
                .await?,
        )
        .await
        {
            Ok(row) => row,
            Err(turso::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(StoreError::Turso(error)),
        };
        let board_id = text_value(row.get_value(0)?, "tasks.board_id")?;
        let title = text_value(row.get_value(1)?, "tasks.title")?;
        let description = optional_text_value(row.get_value(2)?, "tasks.description")?;
        let content = match description.as_deref().map(str::trim) {
            Some(description) if !description.is_empty() => {
                format!("{}\n\n{}", title.trim(), description)
            }
            _ => title.clone(),
        };
        let created_at = integer_value(row.get_value(3)?, "tasks.created_at")?;
        let updated_at = integer_value(row.get_value(4)?, "tasks.updated_at")?;
        let content_hash = content_hash(&content);
        Ok(Some(VectorDocumentInput {
            id: stable_id("doc", &["task", task_id]),
            board_id,
            entity_uri: Some(format!("kb://task/{task_id}")),
            source_kind: "task".to_owned(),
            content,
            content_hash,
            created_at,
            updated_at,
        }))
    }

    pub async fn vector_task_ids(&self, board_selector: &str) -> Result<Vec<String>, StoreError> {
        let connection = self.connection().await?;
        let mut rows = connection
            .query(
                "SELECT t.id FROM tasks t JOIN boards b ON b.id=t.board_id WHERE b.id=?1 OR b.slug=?1 ORDER BY t.seq ASC",
                [board_selector],
            )
            .await?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next().await? {
            ids.push(text_value(row.get_value(0)?, "tasks.id")?);
        }
        Ok(ids)
    }

    pub async fn vector_label_atom_document(
        &self,
        atom_id: &str,
    ) -> Result<Option<VectorDocumentInput>, StoreError> {
        let connection = self.connection().await?;
        let row = match first_row(
            connection
                .query(
                    "SELECT board_id, text, content_hash, created_at, updated_at FROM label_atoms WHERE id=?1",
                    [atom_id],
                )
                .await?,
        )
        .await
        {
            Ok(row) => row,
            Err(turso::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(StoreError::Turso(error)),
        };
        Ok(Some(VectorDocumentInput {
            id: stable_id("doc", &["label_atom", atom_id]),
            board_id: text_value(row.get_value(0)?, "label_atoms.board_id")?,
            entity_uri: None,
            source_kind: "label_atom".to_owned(),
            content: text_value(row.get_value(1)?, "label_atoms.text")?,
            content_hash: text_value(row.get_value(2)?, "label_atoms.content_hash")?,
            created_at: integer_value(row.get_value(3)?, "label_atoms.created_at")?,
            updated_at: integer_value(row.get_value(4)?, "label_atoms.updated_at")?,
        }))
    }

    pub async fn vector_label_atom_ids(
        &self,
        board_selector: &str,
    ) -> Result<Vec<String>, StoreError> {
        let connection = self.connection().await?;
        let mut rows = connection
            .query(
                "SELECT a.id FROM label_atoms a JOIN boards b ON b.id=a.board_id WHERE b.id=?1 OR b.slug=?1 ORDER BY a.label_id, a.ordinal ASC",
                [board_selector],
            )
            .await?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next().await? {
            ids.push(text_value(row.get_value(0)?, "label_atoms.id")?);
        }
        Ok(ids)
    }

    pub async fn vector_board_id(&self, board_selector: &str) -> Result<String, StoreError> {
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT id FROM boards WHERE id=?1 OR slug=?1 LIMIT 1",
                    [board_selector],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => {
                StoreError::BoardNotFound(board_selector.to_owned())
            }
            other => StoreError::Turso(other),
        })?;
        text_value(row.get_value(0)?, "boards.id")
    }

    /// 读取全局或 board-scoped vector 状态。provider 不可用时状态仍可读。
    pub async fn vector_status(
        &self,
        board_id: Option<&str>,
    ) -> Result<VectorStatusRecord, StoreError> {
        let connection = self.connection().await?;
        let projection = first_row(
            connection
                .query(
                    "SELECT lifecycle_status, active_generation, provider, provider_fingerprint, embedding_model, embedding_dimensions, dirty, last_error FROM projection_state WHERE projection='vector_tasks'",
                    (),
                )
                .await?,
        )
        .await?;
        let lifecycle = text_value(
            projection.get_value(0)?,
            "projection_state.lifecycle_status",
        )?;
        let active_generation = optional_text_value(
            projection.get_value(1)?,
            "projection_state.active_generation",
        )?;
        let provider = optional_text_value(projection.get_value(2)?, "projection_state.provider")?;
        let provider_fingerprint = optional_text_value(
            projection.get_value(3)?,
            "projection_state.provider_fingerprint",
        )?;
        let model =
            optional_text_value(projection.get_value(4)?, "projection_state.embedding_model")?;
        let dimensions = optional_integer_value(
            projection.get_value(5)?,
            "projection_state.embedding_dimensions",
        )?
        .map(|value| {
            usize::try_from(value).map_err(|_| StoreError::InvalidStoredValue {
                field: "projection_state.embedding_dimensions",
            })
        })
        .transpose()?;
        let dirty = match integer_value(projection.get_value(6)?, "projection_state.dirty")? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
        let last_error =
            optional_text_value(projection.get_value(7)?, "projection_state.last_error")?;
        let board_dirty = if let Some(board_id) = board_id {
            let row = first_row(
                connection
                    .query(
                        "SELECT EXISTS(SELECT 1 FROM projection_jobs WHERE target IN ('vector_tasks','vector_label_atoms') AND board_id=?1 AND status IN ('pending','running','failed'))",
                        [board_id],
                    )
                    .await?,
            )
            .await?;
            Some(integer_value(row.get_value(0)?, "projection_jobs.board_dirty")? != 0)
        } else {
            None
        };
        let counts = first_row(
            connection
                .query(
                    "SELECT COALESCE(SUM(status='pending'), 0), COALESCE(SUM(status='running'), 0), COALESCE(SUM(status='failed'), 0) FROM projection_jobs WHERE target IN ('vector_tasks','vector_label_atoms') AND (?1 IS NULL OR board_id=?1)",
                    [board_id],
                )
                .await?,
        )
        .await?;
        let pending_jobs = integer_value(counts.get_value(0)?, "projection_jobs.pending")?;
        let running_jobs = integer_value(counts.get_value(1)?, "projection_jobs.running")?;
        let failed_jobs = integer_value(counts.get_value(2)?, "projection_jobs.failed")?;
        let capability = first_row(
            connection
                .query(
                    "SELECT available, detail FROM schema_capabilities WHERE capability='vector32'",
                    (),
                )
                .await?,
        )
        .await
        .ok();
        let capability_available = capability
            .as_ref()
            .and_then(|row| {
                integer_value(row.get_value(0).ok()?, "schema_capabilities.available").ok()
            })
            .is_none_or(|available| available != 0);
        let enabled =
            capability_available && provider.is_some() && model.is_some() && dimensions.is_some();
        let message = if !enabled {
            "vector provider 未配置".to_owned()
        } else if lifecycle == "ready" && last_error.is_none() {
            "Turso vector32 已就绪".to_owned()
        } else if last_error.is_some() {
            "vector projection 降级，等待重试".to_owned()
        } else {
            format!("vector projection 状态：{lifecycle}")
        };
        let mut diagnostics = Vec::new();
        if let Some(error) = last_error {
            diagnostics.push(error);
        }
        if failed_jobs > 0 {
            diagnostics.push(format!("{failed_jobs} 个 vector job 最近失败"));
        }
        if !capability_available {
            diagnostics.push("Turso vector32 capability 不可用".to_owned());
        }
        Ok(VectorStatusRecord {
            backend: VECTOR_BACKEND.to_owned(),
            enabled,
            message,
            diagnostics,
            dirty,
            board_dirty,
            generation: active_generation.and_then(|value| value.parse::<i64>().ok()),
            provider,
            provider_fingerprint,
            model,
            dimensions,
            pending_jobs,
            running_jobs,
            failed_jobs,
        })
    }

    /// 将一个 canonical 事实加入 vector projection 队列。重复事件通过 dedupe key 合并。
    pub async fn enqueue_vector_job(
        &self,
        board_id: Option<&str>,
        source_event_id: Option<i64>,
        target: &str,
        entity_uri: &str,
        operation: &str,
        payload_json: &str,
    ) -> Result<i64, StoreError> {
        validate_target(target)?;
        validate_operation(operation)?;
        if !entity_uri.starts_with("kb://") {
            return Err(StoreError::InvalidInput(
                "vector job entity_uri 必须是 kb:// URI".to_owned(),
            ));
        }
        if payload_json.len() > MAX_VECTOR_CONTENT_BYTES {
            return Err(StoreError::InvalidInput(
                "vector job payload 超过大小上限".to_owned(),
            ));
        }
        let dedupe = format!("{target}:{entity_uri}:{operation}");
        let now = now_ms();
        let connection = self.connection().await?;
        connection
            .execute(
                "INSERT INTO projection_jobs(board_id, source_event_id, target, entity_uri, dedupe_key, operation, payload_json, status, attempts, max_attempts, next_attempt_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', 0, 10, ?8, ?8, ?8) ON CONFLICT(target, dedupe_key) DO UPDATE SET source_event_id=excluded.source_event_id, payload_json=excluded.payload_json, status=CASE WHEN projection_jobs.status='done' THEN 'pending' ELSE projection_jobs.status END, next_attempt_at=excluded.next_attempt_at, updated_at=excluded.updated_at",
                (board_id, source_event_id, target, entity_uri, dedupe.as_str(), operation, payload_json, now),
            )
            .await?;
        let row = first_row(
            connection
                .query(
                    "SELECT id FROM projection_jobs WHERE target=?1 AND dedupe_key=?2",
                    (target, dedupe.as_str()),
                )
                .await?,
        )
        .await?;
        integer_value(row.get_value(0)?, "projection_jobs.id")
    }

    /// 原子 claim ready vector jobs，并回收已经过期的 vector lease。
    ///
    /// 过期的 `running` job 会在同一事务内由当前 owner 重新 claim；旧 owner
    /// 携带的 token/fence 因此不能再完成或失败该 job。达到 `max_attempts` 的过期
    /// job 则转为 `failed`，避免崩溃把队列永久留在 `running`。
    pub async fn claim_vector_jobs(
        &self,
        owner: &str,
        limit: usize,
        now: i64,
    ) -> Result<Vec<ProjectionJobRecord>, StoreError> {
        if owner.trim().is_empty() {
            return Err(StoreError::InvalidInput(
                "vector worker owner 不能为空".to_owned(),
            ));
        }
        let limit = limit.clamp(1, MAX_VECTOR_BATCH);
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        // 最后一轮尝试也可能在 provider 调用前崩溃。不能让这种 job 永远保持
        // running；达到上限时保留失败语义，未达到上限的 job 由下面的 CAS 重新 claim。
        let expired_terminal_error = "vector worker lease 过期且已达到最大尝试次数";
        let terminal_expired = transaction
            .execute(
                "UPDATE projection_jobs SET status='failed', lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, next_attempt_at=NULL, last_error=COALESCE(last_error, ?1), updated_at=?2 WHERE target IN ('vector_tasks','vector_label_atoms') AND status='running' AND lease_expires_at IS NOT NULL AND lease_expires_at <= ?2 AND attempts >= max_attempts",
                (expired_terminal_error, now),
            )
            .await?;
        if terminal_expired > 0 {
            transaction
                .execute(
                    "UPDATE projection_state SET lifecycle_status='degraded', dirty=1, last_error=?1, updated_at=?2 WHERE projection IN ('vector_tasks','vector_label_atoms')",
                    (expired_terminal_error, now),
                )
                .await?;
        }
        let mut rows = transaction
            .query(
                &format!("SELECT id, board_id, source_event_id, target, entity_uri, operation, payload_json, attempts, max_attempts, fence_epoch FROM projection_jobs WHERE target IN ('vector_tasks','vector_label_atoms') AND ((status IN ('pending','failed') AND attempts < max_attempts AND (next_attempt_at IS NULL OR next_attempt_at <= ?1)) OR (status='running' AND lease_expires_at IS NOT NULL AND lease_expires_at <= ?1 AND attempts < max_attempts)) ORDER BY updated_at ASC LIMIT {limit}"),
                [now],
            )
            .await?;
        let mut jobs = Vec::new();
        while let Some(row) = rows.next().await? {
            let id = integer_value(row.get_value(0)?, "projection_jobs.id")?;
            let attempts = integer_value(row.get_value(7)?, "projection_jobs.attempts")? + 1;
            let previous_fence_epoch =
                integer_value(row.get_value(9)?, "projection_jobs.fence_epoch")?;
            let fence_epoch = previous_fence_epoch.saturating_add(1);
            let token = claim_token(owner, id, now, fence_epoch);
            let changed = transaction
                .execute(
                    "UPDATE projection_jobs SET status='running', attempts=?2, lease_owner=?3, lease_token=?4, lease_expires_at=?5, fence_epoch=?6, next_attempt_at=NULL, updated_at=?5 WHERE id=?1 AND fence_epoch=?7 AND ((status IN ('pending','failed') AND attempts < max_attempts AND (next_attempt_at IS NULL OR next_attempt_at <= ?8)) OR (status='running' AND lease_expires_at IS NOT NULL AND lease_expires_at <= ?8 AND attempts < max_attempts))",
                    (
                        id,
                        attempts,
                        owner,
                        token.as_str(),
                        now.saturating_add(VECTOR_JOB_LEASE_MS),
                        fence_epoch,
                        previous_fence_epoch,
                        now,
                    ),
                )
                .await?;
            if changed == 0 {
                continue;
            }
            jobs.push(ProjectionJobRecord {
                id,
                board_id: optional_text_value(row.get_value(1)?, "projection_jobs.board_id")?,
                source_event_id: optional_integer_value(
                    row.get_value(2)?,
                    "projection_jobs.source_event_id",
                )?,
                target: text_value(row.get_value(3)?, "projection_jobs.target")?,
                entity_uri: optional_text_value(row.get_value(4)?, "projection_jobs.entity_uri")?,
                operation: text_value(row.get_value(5)?, "projection_jobs.operation")?,
                payload_json: text_value(row.get_value(6)?, "projection_jobs.payload_json")?,
                attempts,
                max_attempts: integer_value(row.get_value(8)?, "projection_jobs.max_attempts")?,
                lease_owner: Some(owner.to_owned()),
                lease_token: Some(token),
                fence_epoch,
            });
        }
        transaction.commit().await?;
        Ok(jobs)
    }

    pub async fn complete_vector_job(&self, job: &ProjectionJobRecord) -> Result<bool, StoreError> {
        let owner = job
            .lease_owner
            .as_deref()
            .ok_or_else(|| StoreError::InvalidInput("vector job 未 claim".to_owned()))?;
        let token = job
            .lease_token
            .as_deref()
            .ok_or_else(|| StoreError::InvalidInput("vector job lease token 缺失".to_owned()))?;
        let connection = self.connection().await?;
        let changed = connection.execute(
            "UPDATE projection_jobs SET status='done', lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, next_attempt_at=NULL, last_error=NULL, updated_at=?4 WHERE id=?1 AND status='running' AND lease_owner=?2 AND lease_token=?3 AND fence_epoch=?5",
            (job.id, owner, token, now_ms(), job.fence_epoch),
        ).await?;
        Ok(changed > 0)
    }

    /// provider outage 时保留 failed job，并让下一次 worker tick 可以重试。
    pub async fn fail_vector_job(
        &self,
        job: &ProjectionJobRecord,
        error: &str,
        retryable: bool,
    ) -> Result<bool, StoreError> {
        let owner = job
            .lease_owner
            .as_deref()
            .ok_or_else(|| StoreError::InvalidInput("vector job 未 claim".to_owned()))?;
        let token = job
            .lease_token
            .as_deref()
            .ok_or_else(|| StoreError::InvalidInput("vector job lease token 缺失".to_owned()))?;
        let capped_error = error.chars().take(1024).collect::<String>();
        let next_attempt = if retryable {
            now_ms().saturating_add(retry_backoff_ms(job.attempts))
        } else {
            i64::MAX
        };
        let connection = self.connection().await?;
        let changed = connection.execute(
            "UPDATE projection_jobs SET status='failed', lease_owner=NULL, lease_token=NULL, lease_expires_at=NULL, next_attempt_at=?4, last_error=?5, updated_at=?6 WHERE id=?1 AND status='running' AND lease_owner=?2 AND lease_token=?3 AND fence_epoch=?7",
            (job.id, owner, token, next_attempt, capped_error.as_str(), now_ms(), job.fence_epoch),
        ).await?;
        if changed > 0 {
            connection.execute(
                "UPDATE projection_state SET lifecycle_status='degraded', dirty=1, last_error=?1, updated_at=?2 WHERE projection IN ('vector_tasks','vector_label_atoms')",
                (capped_error.as_str(), now_ms()),
            ).await?;
        }
        Ok(changed > 0)
    }

    pub async fn upsert_vector_document(
        &self,
        document: &VectorDocumentInput,
    ) -> Result<(), StoreError> {
        validate_document(document)?;
        let connection = self.connection().await?;
        connection.execute(
            "INSERT INTO retrieval_documents(id, board_id, entity_uri, source_kind, content, content_hash, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) ON CONFLICT(id) DO UPDATE SET board_id=excluded.board_id, entity_uri=excluded.entity_uri, source_kind=excluded.source_kind, content=excluded.content, content_hash=excluded.content_hash, updated_at=excluded.updated_at",
            (document.id.as_str(), document.board_id.as_str(), document.entity_uri.as_deref(), document.source_kind.as_str(), document.content.as_str(), document.content_hash.as_str(), document.created_at, document.updated_at),
        ).await?;
        Ok(())
    }

    pub async fn upsert_vector_embedding(
        &self,
        embedding: &VectorEmbeddingInput,
    ) -> Result<(), StoreError> {
        validate_embedding(embedding)?;
        let literal = vector_literal(&embedding.embedding)?;
        let connection = self.connection().await?;
        connection.execute(
            "INSERT INTO retrieval_vectors(id, board_id, entity_uri, document_id, embedding, dimensions, embedding_model, content_hash, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, vector32(?5), ?6, ?7, ?8, ?9, ?10) ON CONFLICT(document_id, embedding_model) DO UPDATE SET board_id=excluded.board_id, entity_uri=excluded.entity_uri, embedding=excluded.embedding, dimensions=excluded.dimensions, content_hash=excluded.content_hash, updated_at=excluded.updated_at",
            (embedding.id.as_str(), embedding.board_id.as_str(), embedding.entity_uri.as_deref(), embedding.document_id.as_str(), literal.as_str(), i64::try_from(embedding.dimensions).map_err(|_| StoreError::InvalidInput("embedding dimensions 太大".to_owned()))?, embedding.embedding_model.as_str(), embedding.content_hash.as_str(), embedding.created_at, embedding.updated_at),
        ).await?;
        Ok(())
    }

    pub async fn vector_embedding_is_current(
        &self,
        document_id: &str,
        embedding_model: &str,
        content_hash: &str,
    ) -> Result<bool, StoreError> {
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT EXISTS(SELECT 1 FROM retrieval_vectors WHERE document_id=?1 AND embedding_model=?2 AND content_hash=?3)",
                    (document_id, embedding_model, content_hash),
                )
                .await?,
        )
        .await?;
        Ok(integer_value(row.get_value(0)?, "retrieval_vectors.current")? != 0)
    }

    pub async fn query_vector_chunks(
        &self,
        board_id: &str,
        embedding: &[f32],
        model: &str,
        limit: usize,
    ) -> Result<Vec<VectorChunkHitRecord>, StoreError> {
        validate_vector(embedding)?;
        if model.trim().is_empty() {
            return Err(StoreError::InvalidInput(
                "embedding model 不能为空".to_owned(),
            ));
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let literal = vector_literal(embedding)?;
        let limit = limit.min(MAX_VECTOR_BATCH);
        let connection = self.connection().await?;
        let mut rows = connection.query(
            &format!("SELECT v.id, v.board_id, v.entity_uri, d.source_kind, d.content, d.content_hash, v.embedding_model, vector_distance_cos(v.embedding, vector32(?1)) FROM retrieval_vectors v JOIN retrieval_documents d ON d.id=v.document_id WHERE v.board_id=?2 AND v.embedding_model=?3 ORDER BY vector_distance_cos(v.embedding, vector32(?1)) ASC LIMIT {limit}"),
            (literal.as_str(), board_id, model),
        ).await?;
        let mut hits = Vec::new();
        while let Some(row) = rows.next().await? {
            let distance = real_value(row.get_value(7)?, "retrieval_vectors.distance")? as f32;
            hits.push(VectorChunkHitRecord {
                id: text_value(row.get_value(0)?, "retrieval_vectors.id")?,
                board_id: optional_text_value(row.get_value(1)?, "retrieval_vectors.board_id")?,
                entity_uri: optional_text_value(row.get_value(2)?, "retrieval_vectors.entity_uri")?,
                source_kind: text_value(row.get_value(3)?, "retrieval_documents.source_kind")?,
                content: text_value(row.get_value(4)?, "retrieval_documents.content")?,
                content_hash: text_value(row.get_value(5)?, "retrieval_documents.content_hash")?,
                embedding_model: text_value(
                    row.get_value(6)?,
                    "retrieval_vectors.embedding_model",
                )?,
                distance,
                score: 1.0 - distance,
            });
        }
        Ok(hits)
    }

    pub async fn query_vector_label_atoms(
        &self,
        board_id: Option<&str>,
        embedding: &[f32],
        model: &str,
        polarity: Option<&str>,
        limit: usize,
        include_vector: bool,
    ) -> Result<Vec<VectorLabelAtomHitRecord>, StoreError> {
        validate_vector(embedding)?;
        if model.trim().is_empty() {
            return Err(StoreError::InvalidInput(
                "embedding model 不能为空".to_owned(),
            ));
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let literal = vector_literal(embedding)?;
        let limit = limit.min(MAX_VECTOR_BATCH);
        let connection = self.connection().await?;
        let mut rows = connection.query(
            &format!("SELECT a.id, a.label_id, l.name, a.board_id, a.polarity, a.kind, a.text, a.ordinal, a.content_hash, v.embedding_model, vector_distance_cos(v.embedding, vector32(?1)), CASE WHEN ?4 = 1 THEN vector_extract(v.embedding) ELSE NULL END FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id JOIN retrieval_vectors v ON v.board_id=a.board_id AND v.content_hash=a.content_hash JOIN retrieval_documents d ON d.id=v.document_id WHERE (?2 IS NULL OR a.board_id=?2) AND v.embedding_model=?3 AND (?5 IS NULL OR a.polarity=?5) ORDER BY vector_distance_cos(v.embedding, vector32(?1)) ASC LIMIT {limit}"),
            (literal.as_str(), board_id, model, i64::from(include_vector), polarity),
        ).await?;
        let mut hits = Vec::new();
        while let Some(row) = rows.next().await? {
            let distance = real_value(row.get_value(10)?, "retrieval_vectors.distance")? as f32;
            let vector = match row.get_value(11)? {
                Value::Text(value) if include_vector => {
                    serde_json::from_str::<Vec<f32>>(&value).ok()
                }
                _ => None,
            };
            hits.push(VectorLabelAtomHitRecord {
                atom_id: text_value(row.get_value(0)?, "label_atoms.id")?,
                label_id: text_value(row.get_value(1)?, "label_atoms.label_id")?,
                label_name: text_value(row.get_value(2)?, "labels.name")?,
                board_id: text_value(row.get_value(3)?, "label_atoms.board_id")?,
                polarity: text_value(row.get_value(4)?, "label_atoms.polarity")?,
                kind: text_value(row.get_value(5)?, "label_atoms.kind")?,
                text: text_value(row.get_value(6)?, "label_atoms.text")?,
                ordinal: integer_value(row.get_value(7)?, "label_atoms.ordinal")?,
                content_hash: text_value(row.get_value(8)?, "label_atoms.content_hash")?,
                embedding_model: text_value(
                    row.get_value(9)?,
                    "retrieval_vectors.embedding_model",
                )?,
                distance,
                vector,
            });
        }
        Ok(hits)
    }

    pub async fn publish_vector_generation(
        &self,
        generation: &str,
        fingerprint: &str,
    ) -> Result<(), StoreError> {
        if generation.trim().is_empty() || fingerprint.trim().is_empty() {
            return Err(StoreError::InvalidInput(
                "vector generation/fingerprint 不能为空".to_owned(),
            ));
        }
        let now = now_ms();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        // 发布和“是否仍有待处理 job”必须在同一写事务内判断。这样 canonical
        // trigger 若先拿到写锁，会让本次 CAS 观察到 dirty/job；若发布先提交，
        // trigger 会在随后把状态重新标脏，而不会被这里无条件覆盖。
        let ready = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM projection_state WHERE projection IN ('vector_tasks','vector_label_atoms') AND provider_fingerprint=?1 AND dirty=1 AND (building_fingerprint IS NULL OR building_fingerprint=?1)",
                    [fingerprint],
                )
                .await?,
        )
        .await?;
        let ready_rows = integer_value(ready.get_value(0)?, "projection_state.publish_ready")?;
        if ready_rows != 2 {
            transaction.commit().await?;
            return Ok(());
        }
        let outstanding = first_row(
            transaction
                .query(
                    "SELECT COUNT(*) FROM projection_jobs WHERE target IN ('vector_tasks','vector_label_atoms') AND status IN ('pending','running','failed')",
                    (),
                )
                .await?,
        )
        .await?;
        if integer_value(outstanding.get_value(0)?, "projection_jobs.outstanding")? != 0 {
            transaction.commit().await?;
            return Ok(());
        }

        let changed = transaction
            .execute(
                "UPDATE projection_state SET lifecycle_status='ready', active_generation=?1, active_fingerprint=?2, dirty=0, last_success_at=?3, last_error=NULL, updated_at=?3 WHERE projection IN ('vector_tasks','vector_label_atoms') AND provider_fingerprint=?2 AND dirty=1 AND (building_fingerprint IS NULL OR building_fingerprint=?2) AND (SELECT COUNT(*) FROM projection_state WHERE projection IN ('vector_tasks','vector_label_atoms') AND provider_fingerprint=?2 AND dirty=1 AND (building_fingerprint IS NULL OR building_fingerprint=?2))=2 AND NOT EXISTS (SELECT 1 FROM projection_jobs WHERE target IN ('vector_tasks','vector_label_atoms') AND status IN ('pending','running','failed'))",
                (generation, fingerprint, now),
            )
            .await?;
        if changed == 2 {
            // board ledger 是每个 board 的派生镜像；只清除本次确实有完成记录、
            // 且没有未完成 label-atom job 的 board。没有对应 job 的 dirty board
            // 不能因为全局 generation 发布而被一并抹掉。
            transaction
                .execute(
                    "UPDATE label_atom_index_boards SET dirty=0,last_rebuild_at=?1,last_error=NULL,updated_at=?1 WHERE store_name='vector_label_atoms' AND EXISTS (SELECT 1 FROM projection_jobs WHERE target='vector_label_atoms' AND board_id=label_atom_index_boards.board_id AND status='done' AND updated_at >= label_atom_index_boards.updated_at) AND NOT EXISTS (SELECT 1 FROM projection_jobs WHERE target='vector_label_atoms' AND board_id=label_atom_index_boards.board_id AND status IN ('pending','running','failed'))",
                    [now],
                )
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

fn validate_target(target: &str) -> Result<(), StoreError> {
    if matches!(
        target,
        VECTOR_TASKS_PROJECTION | VECTOR_LABEL_ATOMS_PROJECTION
    ) {
        Ok(())
    } else {
        Err(StoreError::InvalidInput(format!(
            "未知 vector target: {target}"
        )))
    }
}

fn validate_operation(operation: &str) -> Result<(), StoreError> {
    if matches!(operation, "upsert" | "delete" | "rebuild") {
        Ok(())
    } else {
        Err(StoreError::InvalidInput(format!(
            "未知 vector operation: {operation}"
        )))
    }
}

fn validate_document(document: &VectorDocumentInput) -> Result<(), StoreError> {
    if !document.id.starts_with("doc_") || document.id.len() <= 4 {
        return Err(StoreError::InvalidInput(
            "retrieval document id 必须是 doc_...".to_owned(),
        ));
    }
    if document.board_id.trim().is_empty() || document.source_kind.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "vector document board/source_kind 不能为空".to_owned(),
        ));
    }
    if document.content.len() > MAX_VECTOR_CONTENT_BYTES {
        return Err(StoreError::InvalidInput(
            "vector document content 超过大小上限".to_owned(),
        ));
    }
    if document.content_hash.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "vector document content_hash 不能为空".to_owned(),
        ));
    }
    Ok(())
}

fn validate_embedding(embedding: &VectorEmbeddingInput) -> Result<(), StoreError> {
    if !embedding.id.starts_with("vec_") || embedding.id.len() <= 4 {
        return Err(StoreError::InvalidInput(
            "retrieval vector id 必须是 vec_...".to_owned(),
        ));
    }
    if embedding.document_id.trim().is_empty() || embedding.board_id.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "retrieval vector document/board 不能为空".to_owned(),
        ));
    }
    if embedding.embedding_model.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "embedding model 不能为空".to_owned(),
        ));
    }
    validate_vector(&embedding.embedding)?;
    if embedding.embedding.len() != embedding.dimensions {
        return Err(StoreError::InvalidInput(format!(
            "embedding 维度不匹配：期望 {}, 实际 {}",
            embedding.dimensions,
            embedding.embedding.len()
        )));
    }
    Ok(())
}

fn validate_vector(vector: &[f32]) -> Result<(), StoreError> {
    if vector.is_empty() || vector.len() > MAX_VECTOR_DIMENSIONS {
        return Err(StoreError::InvalidInput(
            "embedding 维度超出范围".to_owned(),
        ));
    }
    if vector.iter().any(|value| !value.is_finite()) {
        return Err(StoreError::InvalidInput(
            "embedding 必须全部是有限数".to_owned(),
        ));
    }
    Ok(())
}

fn vector_literal(vector: &[f32]) -> Result<String, StoreError> {
    validate_vector(vector)?;
    Ok(format!(
        "[{}]",
        vector
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn real_value(value: Value, field: &'static str) -> Result<f64, StoreError> {
    match value {
        Value::Real(value) => Ok(value),
        Value::Integer(value) => Ok(value as f64),
        _ => Err(StoreError::InvalidStoredValue { field }),
    }
}

fn provider_fingerprint(provider: &str, model: &str, dimensions: usize) -> String {
    let mut digest = Sha256::new();
    digest.update(provider.trim().to_ascii_lowercase().as_bytes());
    digest.update([0]);
    digest.update(model.trim().as_bytes());
    digest.update([0]);
    digest.update(dimensions.to_string().as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

fn corpus_fingerprint(projection: &str, provider_fingerprint: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(projection.as_bytes());
    digest.update([0]);
    digest.update(provider_fingerprint.as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

pub(crate) fn content_hash(content: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(content.as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

pub(crate) fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part.as_bytes());
        digest.update([0]);
    }
    format!(
        "{prefix}_{}",
        hex_digest(&digest.finalize())[..24].to_owned()
    )
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn claim_token(owner: &str, id: i64, now: i64, fence_epoch: i64) -> String {
    let mut digest = Sha256::new();
    digest.update(owner.as_bytes());
    digest.update(id.to_le_bytes());
    digest.update(now.to_le_bytes());
    digest.update(fence_epoch.to_le_bytes());
    format!("vjob:{:x}", digest.finalize())
}

fn retry_backoff_ms(attempts: i64) -> i64 {
    let exponent = attempts.clamp(0, 10) as u32;
    let millis = 250_i64.saturating_mul(1_i64 << exponent);
    millis.min(Duration::from_secs(30).as_millis() as i64)
}

/// 使用当前 canonical vector 配置生成查询 embedding。
pub(crate) async fn embed_query(
    store: &TursoStore,
    text: &str,
) -> Result<(VectorConfig, Vec<f32>), StoreError> {
    let config = store
        .vector_config()
        .await?
        .ok_or_else(|| vector_degraded("vector provider 未配置"))?;
    let embedding = embed_texts(config.clone(), vec![text.to_owned()])
        .await
        .map_err(|error| vector_degraded(error.message))?
        .into_iter()
        .next()
        .ok_or_else(|| vector_degraded("vector provider 返回空 embedding"))?;
    Ok((config, embedding))
}

/// 在 canonical bootstrap transaction 之外，对候选 label 做一次 provider 验证。
///
/// 候选 atoms 只存在于内存；阈值失败或 provider 失败都不会写入 labels、semantics、
/// atoms、task_labels 或 ontology ledger。provider 调用完成后，调用方再以 task/label/
/// ontology 快照进入唯一 bootstrap transaction。
pub(crate) async fn verify_bootstrap_task_label(
    store: &TursoStore,
    task_id: &str,
    input: &BootstrapTaskLabelInput,
    config: &VectorConfig,
    min_score: f32,
) -> Result<crate::BootstrapTaskLabelVerification, StoreError> {
    if !(0.0..=1.0).contains(&min_score) {
        return Err(StoreError::InvalidInput(
            "min_verify_score 必须在 0 到 1 之间".to_owned(),
        ));
    }
    config.validate()?;
    let task = store.get_task_global(task_id).await?;
    let name = input.name.trim().to_owned();
    if name.is_empty() {
        return Err(StoreError::InvalidInput("label name 不能为空".to_owned()));
    }
    let description = input
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let applies_when = normalize_bootstrap_values(&input.applies_when);
    let excludes_when = normalize_bootstrap_values(&input.excludes_when);
    let positive_examples = normalize_bootstrap_values(&input.positive_examples);
    let negative_examples = normalize_bootstrap_values(&input.negative_examples);
    if description.is_none()
        && applies_when.is_empty()
        && excludes_when.is_empty()
        && positive_examples.is_empty()
        && negative_examples.is_empty()
    {
        return Err(StoreError::InvalidInput(
            "label bootstrap 需要 description 或语义示例".to_owned(),
        ));
    }
    let atoms = build_atoms(AtomBuildInput {
        label_id: &input.label_id,
        board_id: &task.board_id,
        label_name: &name,
        description: &description,
        applies: &applies_when,
        excludes: &excludes_when,
        positive: &positive_examples,
        negative: &negative_examples,
        now: task.updated_at,
    });
    let positive_atoms = atoms
        .iter()
        .filter(|atom| atom.polarity == "positive")
        .collect::<Vec<_>>();
    if positive_atoms.is_empty() {
        return Err(StoreError::InvalidInput(
            "label bootstrap verification 需要至少一个正向 semantic atom".to_owned(),
        ));
    }
    if positive_atoms.len() >= MAX_VECTOR_BATCH {
        return Err(StoreError::InvalidInput(format!(
            "label bootstrap verification 最多接受 {} 个 semantic atom",
            MAX_VECTOR_BATCH - 1
        )));
    }
    let existing_atoms =
        stage_bootstrap_label_atom_vectors(store, &task.board_id, &input.label_id, &config.model)
            .await
            .map_err(bootstrap_verification_degraded)?;
    let task_text = bootstrap_task_query_text(&task.title, task.description.as_deref());
    let mut texts = Vec::with_capacity(atoms.len() + 1);
    texts.push(task_text);
    texts.extend(atoms.iter().map(|atom| atom.text.clone()));
    let embeddings = embed_texts(config.clone(), texts)
        .await
        .map_err(|error| bootstrap_verification_degraded(error.message))?;
    let query = embeddings
        .first()
        .ok_or_else(|| bootstrap_verification_degraded("provider 返回空 task embedding"))?;
    let candidate_vectors = embeddings.iter().skip(1).cloned().collect::<Vec<_>>();
    if candidate_vectors.len() != atoms.len() {
        return Err(bootstrap_verification_degraded(format!(
            "candidate embedding 数量不匹配：期望 {}，实际 {}",
            atoms.len(),
            candidate_vectors.len()
        )));
    }
    let mut staged_atoms = existing_atoms;
    staged_atoms.extend(
        atoms
            .iter()
            .zip(candidate_vectors)
            .map(|(atom, vector)| retrieved_bootstrap_atom(atom, vector))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let solver_config = SolverConfig {
        max_candidates: 32,
        retrieval_limit: 80,
        max_selected_labels: 4,
        min_candidate_score: 0.0,
        ..SolverConfig::default()
    };
    let solver_result = resolve_from_atoms(query, &solver_config, &staged_atoms)
        .map_err(bootstrap_verification_degraded)?;
    let target = solver_result
        .selected_labels
        .iter()
        .find(|label| label.label_id == input.label_id)
        .map(|label| (label.score, "selected_labels"))
        .or_else(|| {
            solver_result
                .candidates
                .iter()
                .find(|candidate| candidate.label_id == input.label_id)
                .map(|candidate| (candidate.score, "candidates"))
        });
    let Some((score, source)) = target else {
        return Err(StoreError::InvalidInput(format!(
            "label bootstrap verification 失败：label {name} 未被 label suggest 返回"
        )));
    };
    if score < min_score {
        return Err(StoreError::InvalidInput(format!(
            "label bootstrap verification 失败：label {name} 的 score {score:.3} 低于 min_verify_score {min_score:.3}"
        )));
    }
    Ok(crate::BootstrapTaskLabelVerification {
        label_name: name,
        score,
        source: source.to_owned(),
        min_score,
        degraded: false,
        diagnostics: Vec::new(),
    })
}

const MAX_BOOTSTRAP_STAGED_ATOMS: usize = 4096;

async fn stage_bootstrap_label_atom_vectors(
    store: &TursoStore,
    board_id: &str,
    target_label_id: &str,
    embedding_model: &str,
) -> Result<Vec<RetrievedAtom>, StoreError> {
    let connection = store.connection().await?;
    let mut rows = connection
        .query(
            &format!(
                "SELECT a.id,a.label_id,l.name,a.polarity,a.kind,a.text,a.content_hash,d.id,d.source_kind,d.content_hash,v.embedding_model,vector_extract(v.embedding) FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id JOIN retrieval_vectors v ON v.board_id=a.board_id AND v.content_hash=a.content_hash JOIN retrieval_documents d ON d.id=v.document_id AND d.board_id=a.board_id AND d.source_kind='label_atom' AND d.content_hash=a.content_hash AND d.content=a.text WHERE a.board_id=:board AND a.label_id!=:target_label AND v.embedding_model=:model ORDER BY a.label_id,a.ordinal,a.id LIMIT {MAX_BOOTSTRAP_STAGED_ATOMS}"
            ),
            [
                (":board", board_id),
                (":target_label", target_label_id),
                (":model", embedding_model),
            ],
        )
        .await?;
    let mut atoms = Vec::new();
    while let Some(row) = rows.next().await? {
        let atom_id = text_value(row.get_value(0)?, "label_atoms.id")?;
        let atom_hash = text_value(row.get_value(6)?, "label_atoms.content_hash")?;
        let document_id = text_value(row.get_value(7)?, "retrieval_documents.id")?;
        let expected_document_id = stable_id("doc", &["label_atom", &atom_id]);
        if document_id != expected_document_id {
            continue;
        }
        let source_kind = text_value(row.get_value(8)?, "retrieval_documents.source_kind")?;
        if source_kind != "label_atom" {
            continue;
        }
        let document_hash = text_value(row.get_value(9)?, "retrieval_documents.content_hash")?;
        if document_hash != atom_hash {
            return Err(StoreError::SchemaMismatch(
                "label atom vector document content_hash 不匹配".to_owned(),
            ));
        }
        let actual_model = text_value(row.get_value(10)?, "retrieval_vectors.embedding_model")?;
        if actual_model != embedding_model {
            return Err(StoreError::SchemaMismatch(
                "label atom vector embedding model 不匹配".to_owned(),
            ));
        }
        let vector = match row.get_value(11)? {
            Value::Text(value) => serde_json::from_str::<Vec<f32>>(&value).map_err(|error| {
                StoreError::SchemaMismatch(format!("label atom vector 提取失败：{error}"))
            })?,
            _ => {
                return Err(StoreError::InvalidStoredValue {
                    field: "retrieval_vectors.embedding",
                });
            }
        };
        atoms.push(RetrievedAtom {
            atom_id,
            label_id: text_value(row.get_value(1)?, "label_atoms.label_id")?,
            label_name: text_value(row.get_value(2)?, "labels.name")?,
            polarity: bootstrap_atom_polarity(&text_value(
                row.get_value(3)?,
                "label_atoms.polarity",
            )?)?,
            kind: bootstrap_atom_kind(&text_value(row.get_value(4)?, "label_atoms.kind")?)?,
            text: text_value(row.get_value(5)?, "label_atoms.text")?,
            vector,
        });
    }
    Ok(atoms)
}

fn retrieved_bootstrap_atom(
    atom: &LabelAtomRecord,
    vector: Vec<f32>,
) -> Result<RetrievedAtom, StoreError> {
    Ok(RetrievedAtom {
        atom_id: atom.id.clone(),
        label_id: atom.label_id.clone(),
        label_name: atom.label_name.clone(),
        polarity: bootstrap_atom_polarity(&atom.polarity)?,
        kind: bootstrap_atom_kind(&atom.kind)?,
        text: atom.text.clone(),
        vector,
    })
}

fn bootstrap_task_query_text(title: &str, description: Option<&str>) -> String {
    match description.map(str::trim).filter(|value| !value.is_empty()) {
        Some(description) => format!("{}\n\n{}", title.trim(), description),
        None => title.trim().to_owned(),
    }
}

fn bootstrap_atom_polarity(value: &str) -> Result<AtomPolarity, StoreError> {
    match value {
        "positive" => Ok(AtomPolarity::Positive),
        "negative" => Ok(AtomPolarity::Negative),
        _ => Err(StoreError::SchemaMismatch(format!(
            "未知 label atom polarity：{value}"
        ))),
    }
}

fn bootstrap_atom_kind(value: &str) -> Result<AtomKind, StoreError> {
    match value {
        "name" => Ok(AtomKind::Name),
        "description" => Ok(AtomKind::Description),
        "applies_when" => Ok(AtomKind::AppliesWhen),
        "positive_example" => Ok(AtomKind::PositiveExample),
        "excludes_when" => Ok(AtomKind::ExcludesWhen),
        "negative_example" => Ok(AtomKind::NegativeExample),
        _ => Err(StoreError::SchemaMismatch(format!(
            "未知 label atom kind：{value}"
        ))),
    }
}

fn bootstrap_verification_degraded(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidInput(format!(
        "label bootstrap verification 失败：label suggest degraded（vector_query_error：{error}）"
    ))
}

fn normalize_bootstrap_values(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

async fn embed_texts(
    config: VectorConfig,
    texts: Vec<String>,
) -> Result<Vec<Vec<f32>>, ProviderFailure> {
    tokio::task::spawn_blocking(move || OllamaEmbeddingProvider::new(config).embed_batch(&texts))
        .await
        .map_err(|error| {
            ProviderFailure::retryable(format!("vector provider worker failed: {error}"))
        })?
}

fn vector_degraded(message: impl Into<String>) -> StoreError {
    StoreError::InvalidInput(format!("vector degraded: {}", message.into()))
}

/// 执行一个 host 内 vector projection worker tick。
pub(crate) async fn worker_tick(store: TursoStore, owner: &str) -> Result<usize, StoreError> {
    let Some(config) = store.vector_config().await? else {
        return Ok(0);
    };
    let jobs = store.claim_vector_jobs(owner, 8, now_ms()).await?;
    let mut completed = 0;
    for job in jobs {
        match process_job(&store, &config, &job).await {
            Ok(()) => {
                if store.complete_vector_job(&job).await? {
                    completed += 1;
                }
            }
            Err(error) => {
                let _ = store
                    .fail_vector_job(&job, &error.message, error.retryable)
                    .await?;
            }
        }
    }
    // 即使本 tick 没有 claim 到 job 也要尝试发布：上一次 CAS 可能在检查后
    // 遇到 canonical trigger，后续 tick 应在队列重新安静后再次尝试。
    let generation = now_ms().to_string();
    store
        .publish_vector_generation(&generation, &config.fingerprint())
        .await?;
    Ok(completed)
}

async fn process_job(
    store: &TursoStore,
    config: &VectorConfig,
    job: &ProjectionJobRecord,
) -> Result<(), ProviderFailure> {
    if job.operation == "delete" {
        return Ok(());
    }
    let Some(entity_uri) = job.entity_uri.as_deref() else {
        return Ok(());
    };
    let document = if job.target == VECTOR_TASKS_PROJECTION {
        let Some(task_id) = entity_uri.strip_prefix("kb://task/") else {
            return Ok(());
        };
        store
            .vector_task_document(task_id)
            .await
            .map_err(store_failure)?
    } else if job.target == VECTOR_LABEL_ATOMS_PROJECTION {
        let Some(atom_id) = entity_uri.strip_prefix("kb://label-atom/") else {
            return Ok(());
        };
        store
            .vector_label_atom_document(atom_id)
            .await
            .map_err(store_failure)?
    } else {
        return Ok(());
    };
    let Some(document) = document else {
        return Ok(());
    };
    if store
        .vector_embedding_is_current(&document.id, &config.model, &document.content_hash)
        .await
        .map_err(store_failure)?
    {
        return Ok(());
    }
    let text = document.content.clone();
    let config_for_task = config.clone();
    let vectors = tokio::task::spawn_blocking(move || {
        OllamaEmbeddingProvider::new(config_for_task).embed_batch(&[text])
    })
    .await
    .map_err(|error| ProviderFailure::retryable(error.to_string()))?
    .map_err(|error| ProviderFailure {
        message: error.message,
        retryable: error.retryable,
    })?;
    let embedding = vectors
        .into_iter()
        .next()
        .ok_or_else(|| ProviderFailure::retryable("Ollama 返回空 embedding"))?;
    let vector = VectorEmbeddingInput {
        id: stable_id("vec", &[&document.id, &config.model]),
        board_id: document.board_id.clone(),
        entity_uri: document.entity_uri.clone(),
        document_id: document.id.clone(),
        dimensions: config.dimensions,
        embedding,
        embedding_model: config.model.clone(),
        content_hash: document.content_hash.clone(),
        created_at: document.created_at,
        updated_at: document.updated_at,
    };
    store
        .upsert_vector_document(&document)
        .await
        .map_err(store_failure)?;
    store
        .upsert_vector_embedding(&vector)
        .await
        .map_err(store_failure)?;
    Ok(())
}

fn store_failure(error: StoreError) -> ProviderFailure {
    ProviderFailure {
        message: error.to_string(),
        retryable: false,
    }
}

#[derive(Debug)]
struct ProviderFailure {
    message: String,
    retryable: bool,
}

impl ProviderFailure {
    fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }
}

#[derive(Debug, Clone)]
struct OllamaEmbeddingProvider {
    config: VectorConfig,
}

impl OllamaEmbeddingProvider {
    fn new(config: VectorConfig) -> Self {
        Self { config }
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ProviderFailure> {
        let body = serde_json::json!({
            "model": self.config.model,
            "input": texts,
            "dimensions": self.config.dimensions
        })
        .to_string();
        let (status, response) = http_post_json(&self.config.endpoint, "/api/embed", &body)?;
        if status >= 400 {
            return Err(ProviderFailure {
                message: format!("Ollama HTTP {status}"),
                retryable: status == 429 || status >= 500,
            });
        }
        let parsed: OllamaEmbedResponse = serde_json::from_slice(&response).map_err(|error| {
            ProviderFailure::retryable(format!("Ollama 响应 JSON 无效: {error}"))
        })?;
        if parsed.embeddings.len() != texts.len() {
            return Err(ProviderFailure::retryable(format!(
                "Ollama embedding 数量不匹配：期望 {}, 实际 {}",
                texts.len(),
                parsed.embeddings.len()
            )));
        }
        for vector in &parsed.embeddings {
            if vector.len() != self.config.dimensions {
                return Err(ProviderFailure {
                    message: format!(
                        "embedding 维度不匹配：期望 {}, 实际 {}",
                        self.config.dimensions,
                        vector.len()
                    ),
                    retryable: false,
                });
            }
            if vector.iter().any(|value| !value.is_finite()) {
                return Err(ProviderFailure {
                    message: "embedding 含非有限数".to_owned(),
                    retryable: false,
                });
            }
        }
        Ok(parsed.embeddings)
    }
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

fn http_post_json(
    endpoint: &str,
    path: &str,
    body: &str,
) -> Result<(u16, Vec<u8>), ProviderFailure> {
    let endpoint = endpoint
        .strip_prefix("http://")
        .ok_or_else(|| ProviderFailure {
            message: "Ollama endpoint 仅支持 http://".to_owned(),
            retryable: false,
        })?;
    let authority = endpoint.split('/').next().unwrap_or(endpoint);
    let (host, port) = parse_authority(authority)?;
    let address = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|error| ProviderFailure::retryable(format!("解析 Ollama endpoint 失败: {error}")))?
        .next()
        .ok_or_else(|| ProviderFailure::retryable("Ollama endpoint 无可用地址"))?;
    let mut stream = TcpStream::connect_timeout(&address, OLLAMA_CONNECT_TIMEOUT)
        .map_err(|error| ProviderFailure::retryable(format!("连接 Ollama 失败: {error}")))?;
    stream.set_read_timeout(Some(OLLAMA_READ_TIMEOUT)).ok();
    stream.set_write_timeout(Some(OLLAMA_CONNECT_TIMEOUT)).ok();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| ProviderFailure::retryable(format!("发送 Ollama 请求失败: {error}")))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| ProviderFailure::retryable(format!("读取 Ollama 响应失败: {error}")))?;
    if response.len() > OLLAMA_MAX_RESPONSE_BYTES {
        return Err(ProviderFailure {
            message: "Ollama 响应体超过上限".to_owned(),
            retryable: false,
        });
    }
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| ProviderFailure::retryable("Ollama 响应缺少 headers"))?;
    let header = String::from_utf8_lossy(&response[..split]);
    let status = header
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| ProviderFailure::retryable("Ollama 响应状态码无效"))?;
    Ok((status, response[split + 4..].to_vec()))
}

fn parse_authority(authority: &str) -> Result<(&str, u16), ProviderFailure> {
    if authority.is_empty() || authority.contains('@') {
        return Err(ProviderFailure {
            message: "Ollama endpoint authority 无效".to_owned(),
            retryable: false,
        });
    }
    if let Some(host) = authority.strip_prefix('[') {
        let Some(close) = host.find(']') else {
            return Err(ProviderFailure {
                message: "Ollama endpoint IPv6 authority 无效".to_owned(),
                retryable: false,
            });
        };
        let host_end = &host[..close];
        let suffix = &host[close + 1..];
        let port = if let Some(port) = suffix.strip_prefix(':') {
            port.parse::<u16>().map_err(|_| ProviderFailure {
                message: "Ollama endpoint 端口无效".to_owned(),
                retryable: false,
            })?
        } else if suffix.is_empty() {
            80
        } else {
            return Err(ProviderFailure {
                message: "Ollama endpoint IPv6 authority 无效".to_owned(),
                retryable: false,
            });
        };
        return Ok((host_end, port));
    }
    if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port.parse::<u16>().map_err(|_| ProviderFailure {
            message: "Ollama endpoint 端口无效".to_owned(),
            retryable: false,
        })?;
        Ok((host, port))
    } else {
        Ok((authority, 80))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::store;

    fn test_config() -> VectorConfig {
        VectorConfig {
            provider: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            model: "test-model".to_owned(),
            dimensions: 2,
        }
    }

    async fn insert_rebuild_job(
        store: &TursoStore,
        board_id: &str,
        target: &str,
        status: &str,
        attempts: i64,
        max_attempts: i64,
        updated_at: i64,
    ) {
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO projection_jobs(board_id,target,operation,payload_json,status,attempts,max_attempts,next_attempt_at,created_at,updated_at) VALUES (?1,?2,'rebuild','{}',?3,?4,?5,0,?6,?6)",
                (board_id, target, status, attempts, max_attempts, updated_at),
            )
            .await
            .expect("projection job");
    }

    async fn board_dirty(store: &TursoStore, board_id: &str) -> i64 {
        let connection = store.connection().await.expect("connection");
        let row = first_row(
            connection
                .query(
                    "SELECT dirty FROM label_atom_index_boards WHERE store_name='vector_label_atoms' AND board_id=?1",
                    [board_id],
                )
                .await
                .expect("board dirty query"),
        )
        .await
        .expect("board dirty row");
        integer_value(row.get_value(0).expect("board dirty value"), "board dirty")
            .expect("board dirty integer")
    }

    #[tokio::test]
    async fn expired_running_vector_job_is_reclaimed_and_old_fence_is_ignored() {
        let (_directory, store, _path) = store("vector-expired-reclaim").await;
        store.initialize().await.expect("initialize");
        insert_rebuild_job(
            &store,
            "b_default",
            VECTOR_TASKS_PROJECTION,
            "pending",
            0,
            3,
            1,
        )
        .await;

        let first = store
            .claim_vector_jobs("worker-a", 1, 1_000)
            .await
            .expect("first claim")
            .pop()
            .expect("first job");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE projection_jobs SET lease_expires_at=900 WHERE id=?1",
                [first.id],
            )
            .await
            .expect("expire lease");
        drop(connection);

        let second = store
            .claim_vector_jobs("worker-b", 1, 2_000)
            .await
            .expect("reclaim")
            .pop()
            .expect("reclaimed job");
        assert_eq!(second.attempts, first.attempts + 1);
        assert_eq!(second.fence_epoch, first.fence_epoch + 1);
        assert_ne!(second.lease_token, first.lease_token);
        assert!(
            !store
                .complete_vector_job(&first)
                .await
                .expect("late complete")
        );
        assert!(
            !store
                .fail_vector_job(&first, "late failure", true)
                .await
                .expect("late fail")
        );
        assert!(
            store
                .complete_vector_job(&second)
                .await
                .expect("new owner complete")
        );

        let connection = store.connection().await.expect("connection");
        let row = first_row(
            connection
                .query(
                    "SELECT status,attempts,fence_epoch FROM projection_jobs WHERE id=?1",
                    [second.id],
                )
                .await
                .expect("job query"),
        )
        .await
        .expect("job row");
        assert_eq!(
            text_value(row.get_value(0).expect("status"), "status").expect("status text"),
            "done"
        );
        assert_eq!(
            integer_value(row.get_value(1).expect("attempts"), "attempts")
                .expect("attempts integer"),
            2
        );
        assert_eq!(
            integer_value(row.get_value(2).expect("fence"), "fence").expect("fence integer"),
            2
        );
    }

    #[tokio::test]
    async fn publish_vector_generation_keeps_dirty_when_new_job_is_pending() {
        let (_directory, store, _path) = store("vector-publish-cas").await;
        store.initialize().await.expect("initialize");
        let config = test_config();
        store
            .configure_vector(&config)
            .await
            .expect("configure vector");
        insert_rebuild_job(
            &store,
            "b_default",
            VECTOR_TASKS_PROJECTION,
            "pending",
            0,
            3,
            10,
        )
        .await;

        store
            .publish_vector_generation("generation-1", &config.fingerprint())
            .await
            .expect("CAS publish");
        let blocked = store.vector_status(None).await.expect("blocked status");
        assert_eq!(blocked.dirty, Some(true));
        assert_ne!(blocked.generation, Some(1));

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE projection_jobs SET status='done',next_attempt_at=NULL,updated_at=20 WHERE target='vector_tasks'",
                (),
            )
            .await
            .expect("finish job");
        drop(connection);
        store
            .publish_vector_generation("generation-1", &config.fingerprint())
            .await
            .expect("retry publish");
        let ready = store.vector_status(None).await.expect("ready status");
        assert_eq!(ready.dirty, Some(false));
        assert_eq!(
            ready.generation, None,
            "non-numeric generation is not parsed"
        );
    }

    #[tokio::test]
    async fn publish_vector_generation_only_clears_covered_board_ledgers() {
        let (_directory, store, _path) = store("vector-board-dirty-cas").await;
        store.initialize().await.expect("initialize");
        let config = test_config();
        store
            .configure_vector(&config)
            .await
            .expect("configure vector");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id,slug,name,created_at,updated_at) VALUES ('b_other','other','Other',1,1)",
                (),
            )
            .await
            .expect("other board");
        connection
            .execute(
                "INSERT INTO label_atom_index_boards(store_name,board_id,dirty,last_rebuild_at,last_error,updated_at) VALUES ('vector_label_atoms','b_default',1,NULL,NULL,100),('vector_label_atoms','b_other',1,NULL,NULL,200)",
                (),
            )
            .await
            .expect("board ledgers");
        drop(connection);
        insert_rebuild_job(
            &store,
            "b_default",
            VECTOR_LABEL_ATOMS_PROJECTION,
            "done",
            1,
            3,
            300,
        )
        .await;
        insert_rebuild_job(
            &store,
            "b_other",
            VECTOR_LABEL_ATOMS_PROJECTION,
            "done",
            1,
            3,
            100,
        )
        .await;

        store
            .publish_vector_generation("generation-2", &config.fingerprint())
            .await
            .expect("publish");
        assert_eq!(board_dirty(&store, "b_default").await, 0);
        assert_eq!(board_dirty(&store, "b_other").await, 1);
    }

    #[tokio::test]
    async fn normal_vector_queue_eventually_publishes_ready() {
        let (_directory, store, _path) = store("vector-normal-ready").await;
        store.initialize().await.expect("initialize");
        let config = test_config();
        store
            .configure_vector(&config)
            .await
            .expect("configure vector");
        insert_rebuild_job(
            &store,
            "b_default",
            VECTOR_TASKS_PROJECTION,
            "pending",
            0,
            3,
            1,
        )
        .await;

        let completed = worker_tick(store.clone(), "vector-test-worker")
            .await
            .expect("worker tick");
        assert_eq!(completed, 1);
        let status = store.vector_status(None).await.expect("vector status");
        assert_eq!(status.dirty, Some(false));
        assert_eq!(status.message, "Turso vector32 已就绪");
        assert_eq!(status.pending_jobs, 0);
        assert_eq!(status.running_jobs, 0);
        assert_eq!(status.failed_jobs, 0);
    }
}
