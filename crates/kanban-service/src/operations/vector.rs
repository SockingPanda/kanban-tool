//! Vector projection 的 application service boundary。
//!
//! provider 调用、board selector 解析、projection job enqueue 和查询 embedding
//! 都在这里编排。HTTP 与 dispatcher 只依赖这些 command/result，不接触 Turso row
//! 或 `StoreError`。

use kanban_core::{Clock, KanbanError, Result};

use crate::{KanbanService, VectorConfig, VectorStatusRecord};

/// 配置 host 内 vector provider 的 service command。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorConfigureCommand {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub dimensions: usize,
}

/// Vector projection 的 service result。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorStatus {
    pub backend: String,
    pub enabled: bool,
    pub message: String,
    pub diagnostics: Vec<String>,
    pub dirty: Option<bool>,
    pub board_dirty: Option<bool>,
    pub generation: Option<i64>,
    pub pending_jobs: i64,
    pub running_jobs: i64,
    pub failed_jobs: i64,
}

/// Vector chunk 查询的 service command。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorChunkQueryCommand {
    pub board: String,
    pub q: String,
    pub embedding_model: Option<String>,
    pub limit: usize,
}

/// Vector chunk 查询的 service result。
#[derive(Debug, Clone, PartialEq)]
pub struct VectorChunkResult {
    pub id: String,
    pub entity_uri: Option<String>,
    pub source_kind: String,
    pub content: String,
    pub content_hash: String,
    pub embedding_model: String,
    pub distance: f32,
    pub score: f32,
}

/// Label atom vector 查询的 service command。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorLabelAtomQueryCommand {
    pub board: String,
    pub q: String,
    pub embedding_model: Option<String>,
    pub polarity: Option<String>,
    pub limit: usize,
    pub include_vector: bool,
}

/// Label atom vector 查询的 service result。
#[derive(Debug, Clone, PartialEq)]
pub struct VectorLabelAtomResult {
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

impl<C> KanbanService<C>
where
    C: Clock,
{
    /// 读取 board-scoped vector 状态。provider 未配置时仍返回 degraded 状态。
    pub async fn vector_status(&self, board: &str) -> Result<VectorStatus> {
        let board = normalize_board(board)?;
        let board_id = self
            .store
            .vector_board_id(&board)
            .await
            .map_err(vector_store_error)?;
        self.store
            .vector_status(Some(&board_id))
            .await
            .map_err(vector_store_error)
            .map(vector_status)
    }

    /// 配置 provider，并返回清除旧 generation 后的 vector 状态。
    pub async fn configure_vector(&self, command: VectorConfigureCommand) -> Result<VectorStatus> {
        let config = VectorConfig {
            provider: command.provider,
            endpoint: command.endpoint,
            model: command.model,
            dimensions: command.dimensions,
        };
        self.store
            .configure_vector(&config)
            .await
            .map_err(vector_store_error)
            .map(vector_status)
    }

    /// 为 board 的 canonical task/label atom 事实排队重建，并返回最新状态。
    pub async fn rebuild_vector(&self, board: &str) -> Result<VectorStatus> {
        self.enqueue_vector(board, true).await
    }

    /// 为 board 的 canonical task/label atom 事实排队同步，并返回最新状态。
    pub async fn sync_vector(&self, board: &str) -> Result<VectorStatus> {
        self.enqueue_vector(board, false).await
    }

    /// 为 board 的 canonical task/label atom 事实排队，并返回入队数量。
    pub async fn enqueue_vector_jobs(&self, board: &str, rebuild: bool) -> Result<u64> {
        let board = normalize_board(board)?;
        self.store
            .enqueue_vector_projection_jobs(&board, rebuild)
            .await
            .map_err(vector_store_error)
    }

    /// 使用 service-owned provider 生成 embedding，并查询 vector chunks。
    pub async fn query_vector_chunks(
        &self,
        command: VectorChunkQueryCommand,
    ) -> Result<Vec<VectorChunkResult>> {
        validate_query(&command.board, &command.q, command.limit)?;
        let (config, embedding) = crate::vector::embed_query(&self.store, &command.q)
            .await
            .map_err(vector_store_error)?;
        validate_embedding_model(command.embedding_model.as_deref(), &config.model)?;
        let board_id = self
            .store
            .vector_board_id(command.board.trim())
            .await
            .map_err(vector_store_error)?;
        self.store
            .query_vector_chunks(&board_id, &embedding, &config.model, command.limit)
            .await
            .map_err(vector_store_error)
            .map(|hits| {
                hits.into_iter()
                    .map(|hit| VectorChunkResult {
                        id: hit.id,
                        entity_uri: hit.entity_uri,
                        source_kind: hit.source_kind,
                        content: hit.content,
                        content_hash: hit.content_hash,
                        embedding_model: hit.embedding_model,
                        distance: hit.distance,
                        score: hit.score,
                    })
                    .collect()
            })
    }

    /// 使用 service-owned provider 生成 embedding，并查询 label atom vectors。
    pub async fn query_vector_label_atoms(
        &self,
        command: VectorLabelAtomQueryCommand,
    ) -> Result<Vec<VectorLabelAtomResult>> {
        validate_query(&command.board, &command.q, command.limit)?;
        let (config, embedding) = crate::vector::embed_query(&self.store, &command.q)
            .await
            .map_err(vector_store_error)?;
        validate_embedding_model(command.embedding_model.as_deref(), &config.model)?;
        let board_id = self
            .store
            .vector_board_id(command.board.trim())
            .await
            .map_err(vector_store_error)?;
        self.store
            .query_vector_label_atoms(
                Some(&board_id),
                &embedding,
                &config.model,
                command.polarity.as_deref(),
                command.limit,
                command.include_vector,
            )
            .await
            .map_err(vector_store_error)
            .map(|hits| {
                hits.into_iter()
                    .map(|hit| VectorLabelAtomResult {
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
                        vector: hit.vector,
                    })
                    .collect()
            })
    }

    /// 执行一次 host 内 vector projection worker tick。
    pub async fn vector_worker_tick(&self, owner: &str) -> Result<usize> {
        if owner.trim().is_empty() {
            return Err(KanbanError::InvalidInput(
                "vector worker owner 不能为空".to_owned(),
            ));
        }
        crate::vector::worker_tick(self.store.clone(), owner)
            .await
            .map_err(vector_store_error)
    }

    async fn enqueue_vector(&self, board: &str, rebuild: bool) -> Result<VectorStatus> {
        let board = normalize_board(board)?;
        self.enqueue_vector_jobs(&board, rebuild).await?;
        self.vector_status(&board).await
    }
}

fn normalize_board(board: &str) -> Result<String> {
    let board = board.trim();
    if board.is_empty() {
        return Err(KanbanError::InvalidInput(
            "vector board 不能为空".to_owned(),
        ));
    }
    Ok(board.to_owned())
}

fn validate_query(board: &str, query: &str, limit: usize) -> Result<()> {
    if board.trim().is_empty() || query.trim().is_empty() {
        return Err(KanbanError::InvalidInput(
            "vector query 需要 board 和非空 q".to_owned(),
        ));
    }
    if query.len() > 64 * 1024 {
        return Err(KanbanError::InvalidInput(
            "vector query q 超过大小上限".to_owned(),
        ));
    }
    if limit == 0 || limit > 64 {
        return Err(KanbanError::InvalidInput(
            "vector query limit 必须在 1..=64 内".to_owned(),
        ));
    }
    Ok(())
}

fn validate_embedding_model(requested: Option<&str>, configured: &str) -> Result<()> {
    if let Some(model) = requested
        && model != configured
    {
        return Err(KanbanError::InvalidInput(
            "embedding model 与当前 vector 配置不一致".to_owned(),
        ));
    }
    Ok(())
}

fn vector_status(value: VectorStatusRecord) -> VectorStatus {
    VectorStatus {
        backend: value.backend,
        enabled: value.enabled,
        message: value.message,
        diagnostics: value.diagnostics,
        dirty: value.dirty,
        board_dirty: value.board_dirty,
        generation: value.generation,
        pending_jobs: value.pending_jobs,
        running_jobs: value.running_jobs,
        failed_jobs: value.failed_jobs,
    }
}

/// 保持 vector HTTP 旧有 degraded 映射：输入/provider 降级仍是 400，其余
/// storage failure 继续由 host 映射为 internal error。
fn vector_store_error(error: crate::StoreError) -> KanbanError {
    match error {
        crate::StoreError::InvalidInput(message) => KanbanError::InvalidInput(message),
        other => KanbanError::Storage(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KanbanService, VectorChunkQueryCommand, VectorConfigureCommand, VectorLabelAtomQueryCommand,
    };
    use crate::TursoStore;

    async fn service() -> (tempfile::TempDir, KanbanService) {
        let directory = tempfile::tempdir().expect("temporary database directory");
        let store = TursoStore::open(directory.path().join("kanban.db"))
            .await
            .expect("open store");
        store.initialize().await.expect("initialize store");
        (directory, KanbanService::new(store))
    }

    #[tokio::test]
    async fn vector_service_reads_degraded_status_without_provider() {
        let (_directory, service) = service().await;

        let status = service.vector_status("default").await.expect("status");

        assert_eq!(status.backend, "turso-vector32");
        assert!(!status.enabled);
        assert!(status.message.contains("未配置"));
        assert_eq!(status.board_dirty, Some(false));
    }

    #[tokio::test]
    async fn vector_service_configures_and_enqueues_through_one_boundary() {
        let (_directory, service) = service().await;
        let configured = service
            .configure_vector(VectorConfigureCommand {
                provider: "ollama".to_owned(),
                endpoint: "http://127.0.0.1:1".to_owned(),
                model: "missing-model".to_owned(),
                dimensions: 2,
            })
            .await
            .expect("configure");
        assert!(configured.enabled);

        let rebuilt = service.rebuild_vector("default").await.expect("rebuild");
        assert!(rebuilt.dirty.unwrap_or(false));
        assert!(rebuilt.pending_jobs > 0 || rebuilt.message.contains("projection"));
    }

    #[tokio::test]
    async fn vector_service_preserves_degraded_embedding_error() {
        let (_directory, service) = service().await;
        let error = service
            .query_vector_chunks(VectorChunkQueryCommand {
                board: "default".to_owned(),
                q: "query".to_owned(),
                embedding_model: None,
                limit: 5,
            })
            .await
            .expect_err("provider must be degraded");
        assert!(error.to_string().contains("degraded"));
    }

    #[tokio::test]
    async fn vector_service_rejects_invalid_query_before_provider_access() {
        let (_directory, service) = service().await;
        let error = service
            .query_vector_label_atoms(VectorLabelAtomQueryCommand {
                board: "default".to_owned(),
                q: " ".to_owned(),
                embedding_model: None,
                polarity: None,
                limit: 5,
                include_vector: false,
            })
            .await
            .expect_err("empty query");
        assert!(error.to_string().contains("非空 q"));
    }
}
