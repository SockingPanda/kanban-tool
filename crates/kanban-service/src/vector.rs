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
    error::StoreError,
    shared::{
        first_row, integer_value, now_ms, optional_integer_value, optional_text_value, text_value,
    },
};

pub const VECTOR_TASKS_PROJECTION: &str = "vector_tasks";
pub const VECTOR_LABEL_ATOMS_PROJECTION: &str = "vector_label_atoms";
pub const VECTOR_BACKEND: &str = "turso-vector32";
pub const MAX_VECTOR_BATCH: usize = 64;
pub const MAX_VECTOR_DIMENSIONS: usize = 16_384;
pub const MAX_VECTOR_CONTENT_BYTES: usize = 1_048_576;

const OLLAMA_READ_TIMEOUT: Duration = Duration::from_secs(30);
const OLLAMA_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OLLAMA_MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// host 侧 Ollama 配置。该配置只描述 provider，不改变 canonical 事实。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorConfig {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub dimensions: usize,
}

impl VectorConfig {
    pub fn validate(&self) -> Result<(), StoreError> {
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

    pub fn fingerprint(&self) -> String {
        provider_fingerprint(&self.provider, &self.model, self.dimensions)
    }
}

/// projection_state 的稳定读取 DTO。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorStatusRecord {
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
pub struct VectorDocumentInput {
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
pub struct VectorEmbeddingInput {
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
pub struct ProjectionJobRecord {
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
pub struct VectorChunkHitRecord {
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
pub struct VectorLabelAtomHitRecord {
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
        Ok(integer_value(row.get_value(0)?, "projection_jobs.id")?)
    }

    /// 原子 claim ready vector jobs。只 claim vector target，不会触碰 review/task queue。
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
        let mut rows = transaction
            .query(
                &format!("SELECT id, board_id, source_event_id, target, entity_uri, operation, payload_json, attempts, max_attempts, fence_epoch FROM projection_jobs WHERE target IN ('vector_tasks','vector_label_atoms') AND status IN ('pending','failed') AND attempts < max_attempts AND (next_attempt_at IS NULL OR next_attempt_at <= ?1) ORDER BY updated_at ASC LIMIT {limit}"),
                [now],
            )
            .await?;
        let mut jobs = Vec::new();
        while let Some(row) = rows.next().await? {
            let id = integer_value(row.get_value(0)?, "projection_jobs.id")?;
            let token = claim_token(owner, id, now);
            let attempts = integer_value(row.get_value(7)?, "projection_jobs.attempts")? + 1;
            let fence_epoch = integer_value(row.get_value(9)?, "projection_jobs.fence_epoch")? + 1;
            transaction
                .execute(
                    "UPDATE projection_jobs SET status='running', attempts=?2, lease_owner=?3, lease_token=?4, lease_expires_at=?5, fence_epoch=?6, updated_at=?5 WHERE id=?1",
                    (id, attempts, owner, token.as_str(), now.saturating_add(30_000), fence_epoch),
                )
                .await?;
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
        let connection = self.connection().await?;
        connection.execute(
            "UPDATE projection_state SET lifecycle_status='ready', active_generation=?1, active_fingerprint=?2, dirty=0, last_success_at=?3, last_error=NULL, updated_at=?3 WHERE projection IN ('vector_tasks','vector_label_atoms')",
            (generation, fingerprint, now_ms()),
        ).await?;
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

pub fn content_hash(content: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(content.as_bytes());
    format!("sha256:{:x}", digest.finalize())
}

pub fn stable_id(prefix: &str, parts: &[&str]) -> String {
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

fn claim_token(owner: &str, id: i64, now: i64) -> String {
    let mut digest = Sha256::new();
    digest.update(owner.as_bytes());
    digest.update(id.to_le_bytes());
    digest.update(now.to_le_bytes());
    format!("vjob:{:x}", digest.finalize())
}

fn retry_backoff_ms(attempts: i64) -> i64 {
    let exponent = attempts.clamp(0, 10) as u32;
    let millis = 250_i64.saturating_mul(1_i64 << exponent);
    millis.min(Duration::from_secs(30).as_millis() as i64)
}

/// 使用当前 canonical vector 配置生成查询 embedding。
pub async fn embed_query(
    store: &TursoStore,
    text: &str,
) -> Result<(VectorConfig, Vec<f32>), StoreError> {
    let config = store
        .vector_config()
        .await?
        .ok_or_else(|| StoreError::InvalidInput("vector provider 未配置".to_owned()))?;
    let config_for_task = config.clone();
    let text = text.to_owned();
    let embedding = tokio::task::spawn_blocking(move || {
        OllamaEmbeddingProvider::new(config_for_task).embed_batch(&[text])
    })
    .await
    .map_err(|error| StoreError::InvalidInput(format!("vector provider worker failed: {error}")))?
    .map_err(|error| StoreError::InvalidInput(format!("vector degraded: {}", error.message)))?
    .into_iter()
    .next()
    .ok_or_else(|| StoreError::InvalidInput("vector provider 返回空 embedding".to_owned()))?;
    Ok((config, embedding))
}

/// 执行一个 host 内 vector projection worker tick。
pub async fn worker_tick(store: TursoStore, owner: &str) -> Result<usize, StoreError> {
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
                    let status = store.vector_status(None).await?;
                    if status.pending_jobs == 0
                        && status.running_jobs == 0
                        && status.failed_jobs == 0
                    {
                        let generation = now_ms().to_string();
                        store
                            .publish_vector_generation(&generation, &config.fingerprint())
                            .await?;
                    }
                }
            }
            Err(error) => {
                store
                    .fail_vector_job(&job, &error.message, error.retryable)
                    .await?;
            }
        }
    }
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
