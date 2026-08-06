//! Vector projection 的 application service boundary。
//!
//! provider 调用、board selector 解析、projection job enqueue 和查询 embedding
//! 都在这里编排。HTTP 与 dispatcher 只依赖这些 command/result，不接触 Turso row
//! 或 `StoreError`。

use kanban_core::{Clock, KanbanError, Result};

use crate::{
    KanbanService,
    vector::{VectorConfig, VectorStatusRecord},
};

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
        let _mutation = self.mutation_gate.lock().await;
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
        let _mutation = self.mutation_gate.lock().await;
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
        let _mutation = self.mutation_gate.lock().await;
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::{
        KanbanService, VectorChunkQueryCommand, VectorConfigureCommand, VectorLabelAtomQueryCommand,
    };
    use crate::{
        BootstrapTaskLabelCommand, TursoStore,
        test_support::{count_rows, create_input},
        vector::{VectorEmbeddingInput, content_hash, stable_id},
    };

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

    fn mock_ollama() -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("mock Ollama listener");
        let port = listener.local_addr().expect("mock Ollama address").port();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("mock Ollama connection");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            let header_end = loop {
                let read = stream.read(&mut chunk).expect("read mock request");
                if read == 0 {
                    return;
                }
                request.extend_from_slice(&chunk[..read]);
                if let Some(end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                    break end + 4;
                }
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| line.strip_prefix("Content-Length: "))
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or_default();
            while request.len() < header_end + content_length {
                let read = stream.read(&mut chunk).expect("read mock body");
                if read == 0 {
                    return;
                }
                request.extend_from_slice(&chunk[..read]);
            }
            let body: serde_json::Value =
                serde_json::from_slice(&request[header_end..header_end + content_length])
                    .expect("mock Ollama JSON");
            let dimensions = body
                .get("dimensions")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(4) as usize;
            let values = body
                .get("input")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default();
            let embeddings = values
                .into_iter()
                .map(|value| {
                    let text = value.as_str().unwrap_or_default();
                    let mut vector = vec![0.0_f32; dimensions];
                    if text.contains("unrelated-bootstrap") {
                        if dimensions > 1 {
                            vector[1] = 1.0;
                        }
                        vector[0] = 0.2;
                    } else if text.contains("matching-bootstrap") {
                        vector[0] = 1.0;
                    } else if dimensions > 1 {
                        vector[1] = 1.0;
                    } else {
                        vector[0] = 1.0;
                    }
                    vector
                })
                .collect::<Vec<_>>();
            let payload = serde_json::json!({"embeddings": embeddings}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                payload.len(),
                payload
            );
            stream
                .write_all(response.as_bytes())
                .expect("write mock response");
        });
        (format!("http://127.0.0.1:{port}"), handle)
    }

    fn bootstrap_command(
        task_id: &str,
        endpoint: String,
        description: &str,
    ) -> BootstrapTaskLabelCommand {
        BootstrapTaskLabelCommand {
            task_id: task_id.to_owned(),
            name: "database".to_owned(),
            description: Some(description.to_owned()),
            applies_when: vec!["migration evidence".to_owned()],
            excludes_when: Vec::new(),
            positive_examples: vec!["new table migration".to_owned()],
            negative_examples: Vec::new(),
            actor: "tester".to_owned(),
            verify: true,
            min_verify_score: 0.50,
            vector_config: Some(VectorConfigureCommand {
                provider: "ollama".to_owned(),
                endpoint,
                model: "mock-bootstrap".to_owned(),
                dimensions: 4,
            }),
        }
    }

    async fn seed_existing_label_atom(
        store: &TursoStore,
        label_id: &str,
        label_name: &str,
        atom_id: &str,
        text: &str,
        vector: Vec<f32>,
        model: &str,
    ) {
        let connection = store.connection().await.expect("connection");
        let hash = content_hash(text);
        connection
            .execute(
                "INSERT INTO labels(id,board_id,name,created_at,updated_at) VALUES (:label,'b_default',:name,1,1)",
                turso::named_params! { ":label": label_id, ":name": label_name },
            )
            .await
            .expect("existing label");
        connection
            .execute(
                "INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at) VALUES (:atom,:label,'b_default','positive','description',:text,0,:hash,1,1)",
                turso::named_params! {
                    ":atom": atom_id,
                    ":label": label_id,
                    ":text": text,
                    ":hash": hash.as_str(),
                },
            )
            .await
            .expect("existing label atom");
        let document = store
            .vector_label_atom_document(atom_id)
            .await
            .expect("existing atom document")
            .expect("existing atom document row");
        store
            .upsert_vector_document(&document)
            .await
            .expect("existing vector document");
        store
            .upsert_vector_embedding(&VectorEmbeddingInput {
                id: stable_id("vec", &[&document.id, model]),
                board_id: document.board_id.clone(),
                entity_uri: document.entity_uri.clone(),
                document_id: document.id.clone(),
                embedding: vector,
                dimensions: 4,
                embedding_model: model.to_owned(),
                content_hash: document.content_hash.clone(),
                created_at: document.created_at,
                updated_at: document.updated_at,
            })
            .await
            .expect("existing vector embedding");
    }

    async fn bootstrap_canonical_counts(
        store: &TursoStore,
    ) -> (i64, i64, i64, i64, i64, i64, i64, Option<i64>) {
        let connection = store.connection().await.expect("connection");
        let labels = count_rows(&connection, "labels").await;
        let semantics = count_rows(&connection, "label_semantics").await;
        let atoms = count_rows(&connection, "label_atoms").await;
        let task_labels = count_rows(&connection, "task_labels").await;
        let actions = count_rows(&connection, "label_ontology_actions").await;
        let effects = count_rows(&connection, "label_ontology_action_atom_effects").await;
        let events = count_rows(&connection, "task_events").await;
        let mut rows = connection
            .query(
                "SELECT dirty FROM label_atom_index_boards WHERE store_name='vector_label_atoms' AND board_id='b_default' LIMIT 1",
                (),
            )
            .await
            .expect("index state");
        let dirty = rows.next().await.expect("index state row").map(|row| {
            crate::shared::integer_value(row.get_value(0).expect("dirty"), "dirty")
                .expect("dirty integer")
        });
        (
            labels,
            semantics,
            atoms,
            task_labels,
            actions,
            effects,
            events,
            dirty,
        )
    }

    #[tokio::test]
    async fn bootstrap_verification_is_staged_before_canonical_write() {
        let (_directory, store) = {
            let directory = tempfile::tempdir().expect("temp directory");
            let store = TursoStore::open(directory.path().join("bootstrap-verify.db"))
                .await
                .expect("open store");
            store.initialize().await.expect("initialize store");
            (directory, store)
        };
        store
            .create_task(
                "default",
                create_input(
                    "t_bootstrap_verify",
                    Some("bootstrap-verify"),
                    "matching-bootstrap target",
                ),
            )
            .await
            .expect("create task");
        let (endpoint, handle) = mock_ollama();
        let service = KanbanService::new(store.clone());
        let before = bootstrap_canonical_counts(&store).await;
        let result = service
            .bootstrap_task_label(bootstrap_command(
                "t_bootstrap_verify",
                endpoint,
                "matching-bootstrap database persistence",
            ))
            .await
            .expect("verification should pass");
        handle.join().expect("mock Ollama thread");
        let verification = result.verification.expect("verification result");
        assert!(verification.score >= 0.50);
        assert_eq!(verification.source, "selected_labels");
        assert_eq!(result.task.labels.len(), 1);
        assert_eq!(
            service
                .list_board_labels("default")
                .await
                .expect("labels")
                .len(),
            1
        );
        let after = bootstrap_canonical_counts(&store).await;
        assert_eq!(after.0, before.0 + 1);
        assert_eq!(after.1, before.1 + 1);
        assert!(after.2 > before.2);
        assert_eq!(after.3, before.3 + 1);
        assert_eq!(after.4, before.4 + 1);
        assert_eq!(after.5, before.5 + (after.2 - before.2));
        assert_eq!(after.6, before.6 + 2);
        assert_eq!(after.7, Some(1));
        let action = store
            .connection()
            .await
            .expect("connection")
            .query(
                "SELECT change_json FROM label_ontology_actions WHERE action_type='bootstrap_label' ORDER BY created_at DESC LIMIT 1",
                (),
            )
            .await
            .expect("action query")
            .next()
            .await
            .expect("action row")
            .expect("bootstrap action");
        let change_json =
            crate::shared::text_value(action.get_value(0).expect("change json"), "change_json")
                .expect("change text");
        assert!(change_json.contains("bootstrap_verification"));
        assert!(change_json.contains("selected_labels"));
    }

    #[tokio::test]
    async fn bootstrap_verification_falls_back_to_candidates_when_existing_label_selected() {
        let (_directory, store) = {
            let directory = tempfile::tempdir().expect("temporary database directory");
            let store = TursoStore::open(directory.path().join("bootstrap-verify-candidate.db"))
                .await
                .expect("open store");
            store.initialize().await.expect("initialize store");
            (directory, store)
        };
        store
            .create_task(
                "default",
                create_input(
                    "t_bootstrap_verify_candidate",
                    Some("bootstrap-verify-candidate"),
                    "matching-bootstrap target",
                ),
            )
            .await
            .expect("create task");
        seed_existing_label_atom(
            &store,
            "l_existing",
            "alpha existing",
            "la_existing",
            "existing matching evidence",
            vec![1.0, 0.0, 0.0, 0.0],
            "mock-bootstrap",
        )
        .await;
        let (endpoint, handle) = mock_ollama();
        let service = KanbanService::new(store.clone());
        let before = bootstrap_canonical_counts(&store).await;
        let result = service
            .bootstrap_task_label(bootstrap_command(
                "t_bootstrap_verify_candidate",
                endpoint,
                "matching-bootstrap database persistence",
            ))
            .await
            .expect("candidate fallback should pass");
        handle.join().expect("mock Ollama thread");
        let verification = result.verification.expect("verification result");
        assert_eq!(verification.source, "candidates");
        assert!(verification.score >= 0.50);
        let after = bootstrap_canonical_counts(&store).await;
        assert_eq!(after.0, before.0 + 1);
        assert_eq!(after.1, before.1 + 1);
        assert!(after.2 > before.2);
        assert_eq!(after.3, before.3 + 1);
        assert_eq!(after.4, before.4 + 1);
        assert_eq!(after.5, before.5 + (after.2 - before.2));
        assert_eq!(after.6, before.6 + 2);
    }

    #[tokio::test]
    async fn bootstrap_verification_negative_evidence_rejects_without_writes() {
        let (_directory, store) = {
            let directory = tempfile::tempdir().expect("temporary database directory");
            let store = TursoStore::open(directory.path().join("bootstrap-verify-negative.db"))
                .await
                .expect("open store");
            store.initialize().await.expect("initialize store");
            (directory, store)
        };
        store
            .create_task(
                "default",
                create_input(
                    "t_bootstrap_verify_negative",
                    Some("bootstrap-verify-negative"),
                    "matching-bootstrap target",
                ),
            )
            .await
            .expect("create task");
        let (endpoint, handle) = mock_ollama();
        let service = KanbanService::new(store.clone());
        let before = bootstrap_canonical_counts(&store).await;
        let mut command = bootstrap_command(
            "t_bootstrap_verify_negative",
            endpoint,
            "matching-bootstrap database persistence",
        );
        command.min_verify_score = 0.50;
        command.excludes_when = vec!["matching-bootstrap exclusion".to_owned()];
        let error = service
            .bootstrap_task_label(command)
            .await
            .expect_err("negative evidence should lower the score below threshold");
        handle.join().expect("mock Ollama thread");
        assert!(error.to_string().contains("低于 min_verify_score"));
        assert_eq!(bootstrap_canonical_counts(&store).await, before);
    }

    #[tokio::test]
    async fn bootstrap_verification_threshold_failure_leaves_no_canonical_rows() {
        let (_directory, store) = {
            let directory = tempfile::tempdir().expect("temp directory");
            let store = TursoStore::open(directory.path().join("bootstrap-verify-fail.db"))
                .await
                .expect("open store");
            store.initialize().await.expect("initialize store");
            (directory, store)
        };
        store
            .create_task(
                "default",
                create_input(
                    "t_bootstrap_verify_fail",
                    Some("bootstrap-verify-fail"),
                    "matching-bootstrap target",
                ),
            )
            .await
            .expect("create task");
        let (endpoint, handle) = mock_ollama();
        let service = KanbanService::new(store.clone());
        let before = bootstrap_canonical_counts(&store).await;
        let mut command = bootstrap_command(
            "t_bootstrap_verify_fail",
            endpoint,
            "unrelated-bootstrap database persistence",
        );
        command.applies_when = vec!["unrelated-bootstrap evidence".to_owned()];
        command.positive_examples = vec!["unrelated-bootstrap migration".to_owned()];
        let error = service
            .bootstrap_task_label(command)
            .await
            .expect_err("verification threshold should fail");
        handle.join().expect("mock Ollama thread");
        assert!(error.to_string().contains("低于 min_verify_score"));
        assert!(
            service
                .list_board_labels("default")
                .await
                .expect("labels")
                .is_empty()
        );
        assert_eq!(bootstrap_canonical_counts(&store).await, before);
        assert!(
            service
                .store
                .list_label_semantics("default")
                .await
                .expect("semantics")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn bootstrap_verification_no_target_leaves_no_canonical_rows() {
        let (_directory, store) = {
            let directory = tempfile::tempdir().expect("temp directory");
            let store = TursoStore::open(directory.path().join("bootstrap-verify-no-target.db"))
                .await
                .expect("open store");
            store.initialize().await.expect("initialize store");
            (directory, store)
        };
        store
            .create_task(
                "default",
                create_input(
                    "t_bootstrap_verify_no_target",
                    Some("bootstrap-verify-no-target"),
                    "matching-bootstrap target",
                ),
            )
            .await
            .expect("create task");
        let (endpoint, handle) = mock_ollama();
        let service = KanbanService::new(store.clone());
        let before = bootstrap_canonical_counts(&store).await;
        let mut command = bootstrap_command(
            "t_bootstrap_verify_no_target",
            endpoint,
            "no-target-bootstrap database persistence",
        );
        command.applies_when = vec!["no-target-bootstrap evidence".to_owned()];
        command.positive_examples = vec!["no-target-bootstrap migration".to_owned()];
        let error = service
            .bootstrap_task_label(command)
            .await
            .expect_err("verification without a returned target should fail");
        handle.join().expect("mock Ollama thread");
        assert!(error.to_string().contains("未被 label suggest 返回"));
        assert!(
            service
                .list_board_labels("default")
                .await
                .expect("labels")
                .is_empty()
        );
        assert_eq!(bootstrap_canonical_counts(&store).await, before);
    }

    #[tokio::test]
    async fn bootstrap_verification_provider_and_missing_task_fail_without_writes() {
        let (_directory, store) = {
            let directory = tempfile::tempdir().expect("temp directory");
            let store = TursoStore::open(directory.path().join("bootstrap-verify-provider.db"))
                .await
                .expect("open store");
            store.initialize().await.expect("initialize store");
            (directory, store)
        };
        store
            .create_task(
                "default",
                create_input(
                    "t_bootstrap_verify_provider",
                    Some("bootstrap-verify-provider"),
                    "provider failure target",
                ),
            )
            .await
            .expect("create task");
        let service = KanbanService::new(store.clone());
        let before = bootstrap_canonical_counts(&store).await;
        let provider_error = service
            .bootstrap_task_label(bootstrap_command(
                "t_bootstrap_verify_provider",
                "http://127.0.0.1:1".to_owned(),
                "provider failure semantics",
            ))
            .await
            .expect_err("provider failure should reject bootstrap");
        assert!(
            provider_error
                .to_string()
                .contains("label bootstrap verification")
        );
        assert!(provider_error.to_string().contains("degraded"));
        assert!(provider_error.to_string().contains("vector_query_error"));
        assert!(
            service
                .list_board_labels("default")
                .await
                .expect("labels")
                .is_empty()
        );
        assert_eq!(bootstrap_canonical_counts(&store).await, before);

        let missing_task = service
            .bootstrap_task_label(bootstrap_command(
                "t_bootstrap_verify_missing",
                "http://127.0.0.1:1".to_owned(),
                "missing task semantics",
            ))
            .await
            .expect_err("missing task should reject bootstrap");
        assert!(missing_task.to_string().contains("task"));
        assert!(
            service
                .list_board_labels("default")
                .await
                .expect("labels")
                .is_empty()
        );
        assert_eq!(bootstrap_canonical_counts(&store).await, before);
    }
}
