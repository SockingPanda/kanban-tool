use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use kanban_contract::{
    ProjectionArtifactEvidence, ProjectionArtifactManifest, ProjectionBatch, ProjectionDelivery,
    ProjectionDeliveryAction, ProjectionSnapshot, ProjectionSnapshotRecord,
    VECTOR_PROJECTION_PROTOCOL_VERSION, VectorProjectionApplyBatchRequest,
    VectorProjectionBuildingPhase, VectorProjectionCleanupProtection,
    VectorProjectionCleanupRequest, VectorProjectionDestructiveAuthority,
    VectorProjectionGenerationBinding, VectorProjectionGenerationMutationRequest,
    VectorProjectionGenerationRole, VectorProjectionGenerationState,
    VectorProjectionHelperErrorKind, VectorProjectionHelperOperation,
    VectorProjectionHelperRequest, VectorProjectionHelperResponse,
    VectorProjectionInspectActiveRequest, VectorProjectionInspectGenerationRequest,
    VectorProjectionInventoryRequest, VectorProjectionMutationContext,
    VectorProjectionPrepareSnapshotRequest, VectorProjectionPublishRequest,
    VectorProjectionRepairPublicationRequest, VectorProjectionValidateActiveRequest,
    VectorProjectionValidateGenerationRequest,
};
use kanban_indexer::{LANCEDB_CHUNKS_STORE, LANCEDB_LABEL_ATOMS_STORE};
use kanban_local::{DatabaseLifecycleExclusiveGuard, DerivedStoreWriteGuard};
use kanban_vector::{
    ChunkVectorStore, EmbeddingProvider, LABEL_ATOMS_CORPUS_SCHEMA, LabelAtomQuery,
    LabelAtomVectorStore, TASK_CHUNKS_CORPUS_SCHEMA, VectorError, VectorQuery,
};
use kanban_vector_lancedb::{
    ActiveLanceProjectionReader, EmbeddingExecutionPolicy, LanceDbConfig, LanceDbStore,
    VectorProjectionBackend, VectorProjectionBackendError,
};
use tempfile::TempDir;

struct StaticProvider;

impl EmbeddingProvider for StaticProvider {
    fn provider_name(&self) -> &str {
        "fixture"
    }

    fn embedding_model(&self) -> &str {
        "fixture-model"
    }

    fn dimensions(&self) -> usize {
        2
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        Ok(vec![text.len() as f32, 1.0])
    }
}

struct AlternateProvider;

impl EmbeddingProvider for AlternateProvider {
    fn provider_name(&self) -> &str {
        "alternate-fixture"
    }

    fn embedding_model(&self) -> &str {
        "alternate-fixture-model"
    }

    fn dimensions(&self) -> usize {
        3
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        Ok(vec![text.len() as f32, 2.0, 3.0])
    }
}

struct SnapshotMutationProvider {
    db_path: PathBuf,
    model_calls: AtomicUsize,
    mutated: AtomicBool,
}

impl SnapshotMutationProvider {
    fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            model_calls: AtomicUsize::new(0),
            mutated: AtomicBool::new(false),
        }
    }
}

impl EmbeddingProvider for SnapshotMutationProvider {
    fn provider_name(&self) -> &str {
        "fixture"
    }

    fn embedding_model(&self) -> &str {
        if self.model_calls.fetch_add(1, Ordering::SeqCst) == 4 {
            let conn = rusqlite::Connection::open(&self.db_path).unwrap();
            conn.busy_timeout(Duration::from_secs(5)).unwrap();
            conn.execute(
                "INSERT INTO tasks(
                     id,board_id,seq,title,description,status,archived_at,created_at,updated_at
                 ) VALUES (
                     't_snapshot_later','b_snapshot',2,'later task',
                     NULL,'todo',NULL,2,2
                 )",
                [],
            )
            .unwrap();
            self.mutated.store(true, Ordering::SeqCst);
        }
        "fixture-model"
    }

    fn dimensions(&self) -> usize {
        2
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        Ok(vec![text.len() as f32, 1.0])
    }
}

#[derive(Default)]
struct CountingProvider {
    batch_calls: AtomicUsize,
    successful_texts: Mutex<Vec<String>>,
    fail_once_on: Mutex<Option<String>>,
}

impl CountingProvider {
    fn fail_once_on(text: &str) -> Self {
        Self {
            fail_once_on: Mutex::new(Some(text.to_owned())),
            ..Self::default()
        }
    }

    fn successful_count(&self, text: &str) -> usize {
        self.successful_texts
            .lock()
            .unwrap()
            .iter()
            .filter(|item| item.as_str() == text)
            .count()
    }
}

impl EmbeddingProvider for CountingProvider {
    fn provider_name(&self) -> &str {
        "fixture"
    }

    fn embedding_model(&self) -> &str {
        "fixture-model"
    }

    fn dimensions(&self) -> usize {
        2
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, VectorError> {
        self.embed_batch(&[text.to_owned()])?
            .into_iter()
            .next()
            .ok_or_else(|| VectorError::Store("fixture embedding was missing".to_owned()))
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, VectorError> {
        self.batch_calls.fetch_add(1, Ordering::SeqCst);
        let mut fail_once_on = self.fail_once_on.lock().unwrap();
        if fail_once_on
            .as_ref()
            .is_some_and(|target| texts.iter().any(|text| text == target))
        {
            fail_once_on.take();
            return Err(VectorError::Provider {
                message: "fixture provider interruption".to_owned(),
                retryable: true,
            });
        }
        drop(fail_once_on);
        self.successful_texts
            .lock()
            .unwrap()
            .extend(texts.iter().cloned());
        Ok(texts
            .iter()
            .map(|text| vec![text.len() as f32, 1.0])
            .collect())
    }
}

#[test]
fn backend_constructor_is_database_io_free() {
    let temp = tempfile::tempdir().unwrap();
    let missing_database = temp.path().join("missing.sqlite");

    let backend =
        VectorProjectionBackend::new(&missing_database, Arc::new(StaticProvider)).unwrap();

    assert!(!missing_database.exists());
    assert_eq!(
        backend
            .descriptor("req_database_io_free")
            .supported_stores
            .len(),
        2
    );
}

#[test]
fn configured_descriptor_advertises_both_independent_corpora_and_all_operations() {
    let (_temp, _db, backend) = backend();
    let descriptor = backend.descriptor("req_descriptor");

    assert_eq!(descriptor.supported_stores.len(), 2);
    assert_eq!(
        descriptor
            .supported_stores
            .iter()
            .map(|store| (
                store.store_name.as_str(),
                store.corpus.as_ref().unwrap().corpus_schema.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![
            (LANCEDB_CHUNKS_STORE, TASK_CHUNKS_CORPUS_SCHEMA),
            (LANCEDB_LABEL_ATOMS_STORE, LABEL_ATOMS_CORPUS_SCHEMA),
        ]
    );
    assert_eq!(
        descriptor.supported_operations,
        vec![
            VectorProjectionHelperOperation::Descriptor,
            VectorProjectionHelperOperation::PrepareSnapshot,
            VectorProjectionHelperOperation::ApplyBatch,
            VectorProjectionHelperOperation::Publish,
            VectorProjectionHelperOperation::InspectActive,
            VectorProjectionHelperOperation::InspectGeneration,
            VectorProjectionHelperOperation::ValidateGenerationPublication,
            VectorProjectionHelperOperation::ValidateActiveContents,
            VectorProjectionHelperOperation::RepairPublication,
            VectorProjectionHelperOperation::Quarantine,
            VectorProjectionHelperOperation::Abort,
            VectorProjectionHelperOperation::Inventory,
            VectorProjectionHelperOperation::Cleanup,
        ]
    );
    assert_ne!(
        descriptor.supported_stores[0]
            .corpus
            .as_ref()
            .unwrap()
            .corpus_fingerprint,
        descriptor.supported_stores[1]
            .corpus
            .as_ref()
            .unwrap()
            .corpus_fingerprint
    );
}

#[test]
fn active_reader_uses_only_the_sqlite_bound_v2_generation_and_ignores_v1() {
    let (temp, db, backend) = backend();
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_active_reader", 1);
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_reader','b_one',1,'active reader',NULL,'todo',NULL,1,1)",
        [],
    )
    .unwrap();
    drop(conn);
    let mut task_delivery = delivery(&evidence, 901, 2, ProjectionDeliveryAction::Upsert);
    task_delivery.entity_uri = "kb://task/t_reader".to_owned();
    apply(&backend, &db, &evidence, task_delivery);
    publish(&backend, &db, None, &evidence);

    let legacy = kanban_local::vector_store_path(&db);
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("v1-sentinel"), b"must not be opened").unwrap();
    let before = filesystem_digest(temp.path());

    let reader =
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
            .unwrap();
    assert_eq!(reader.generation(), "gen_active_reader");
    let hits = reader
        .query_chunks(&VectorQuery {
            text: "active reader".to_owned(),
            limit: 10,
            board_id: "b_one".to_owned(),
        })
        .unwrap();

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk.entity_uri.as_str(), "kb://task/t_reader");
    assert_eq!(
        filesystem_digest(temp.path()),
        before,
        "opening and querying an active Projection v2 generation must be read-only"
    );
}

#[test]
fn active_reader_holds_physical_read_authority_until_drop() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_reader_authority",
        1,
    );
    publish(&backend, &db, None, &evidence);

    let reader =
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
            .unwrap();

    let writer_error =
        DerivedStoreWriteGuard::acquire(&db, &format!("{LANCEDB_CHUNKS_STORE}-projection-helper"))
            .expect_err("an active reader must retain the helper's physical read authority");
    assert_eq!(writer_error.kind(), std::io::ErrorKind::WouldBlock);
    let replace_error = DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db)
        .expect_err("an active reader must retain the database lifecycle authority");
    assert_eq!(replace_error.kind(), std::io::ErrorKind::WouldBlock);

    drop(reader);
    drop(
        DerivedStoreWriteGuard::acquire(&db, &format!("{LANCEDB_CHUNKS_STORE}-projection-helper"))
            .unwrap(),
    );
    drop(DatabaseLifecycleExclusiveGuard::acquire_existing_for_replace(&db).unwrap());
}

#[test]
fn active_reader_validation_uses_one_sqlite_wal_snapshot() {
    let (_temp, db, backend) = backend();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
    conn.execute(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES (
             't_snapshot_initial','b_snapshot',1,'initial task',
             NULL,'todo',NULL,1,1
         )",
        [],
    )
    .unwrap();
    drop(conn);
    let evidence = prepare_with_records(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_snapshot_consistency",
        1,
        vec![task_record(
            "b_snapshot",
            "t_snapshot_initial",
            "initial task",
        )],
    );
    publish(&backend, &db, None, &evidence);

    let provider = Arc::new(SnapshotMutationProvider::new(db.clone()));
    let reader = ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, provider.clone())
        .expect("one read session must not mix a later canonical mutation into its snapshot");
    assert!(provider.mutated.load(Ordering::SeqCst));
    assert_eq!(reader.generation(), "gen_snapshot_consistency");
    drop(reader);

    let error =
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
            .expect_err("a new session must observe that the active generation is now stale");
    assert!(
        error
            .to_string()
            .contains("physical row set does not match canonical SQLite truth"),
        "{error}"
    );
}

#[test]
fn board_scoped_reader_resolves_and_revalidates_board_inside_the_read_session() {
    let (_temp, db, backend) = backend();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE boards(
             id TEXT PRIMARY KEY,
             slug TEXT NOT NULL UNIQUE,
             archived_at INTEGER
         );
         INSERT INTO boards(id,slug,archived_at)
         VALUES ('b_scoped','scoped-board',NULL);
         INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES (
             't_scoped','b_scoped',1,'scoped task',NULL,'todo',NULL,1,1
         );",
    )
    .unwrap();
    drop(conn);
    let evidence = prepare_with_records(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_board_session",
        1,
        vec![task_record("b_scoped", "t_scoped", "scoped task")],
    );
    publish(&backend, &db, None, &evidence);

    let reader = ActiveLanceProjectionReader::open_for_board(
        &db,
        LANCEDB_CHUNKS_STORE,
        "scoped-board",
        Some("b_scoped"),
        Arc::new(StaticProvider),
    )
    .unwrap();
    assert_eq!(reader.resolved_board_id(), Some("b_scoped"));
    drop(reader);

    let error = ActiveLanceProjectionReader::open_for_board(
        &db,
        LANCEDB_CHUNKS_STORE,
        "scoped-board",
        Some("b_other"),
        Arc::new(StaticProvider),
    )
    .unwrap_err();
    assert!(error.to_string().contains("board mismatch"), "{error}");
}

#[test]
fn board_scoped_chunk_reader_rejects_a_different_request_board_before_querying() {
    let (_temp, db, backend) = backend();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE boards(
             id TEXT PRIMARY KEY,
             slug TEXT NOT NULL UNIQUE,
             archived_at INTEGER
         );
         INSERT INTO boards(id,slug,archived_at) VALUES
             ('b_scope_one','scope-one',NULL),
             ('b_scope_two','scope-two',NULL);
         INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES
             ('t_scope_one','b_scope_one',1,'first scoped task',NULL,'todo',NULL,1,1),
             ('t_scope_two','b_scope_two',1,'second scoped task',NULL,'todo',NULL,1,1);",
    )
    .unwrap();
    drop(conn);
    let evidence = prepare_with_records(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_chunk_query_scope",
        1,
        vec![
            task_record("b_scope_one", "t_scope_one", "first scoped task"),
            task_record("b_scope_two", "t_scope_two", "second scoped task"),
        ],
    );
    publish(&backend, &db, None, &evidence);

    let provider = Arc::new(CountingProvider::default());
    let reader = ActiveLanceProjectionReader::open_for_board(
        &db,
        LANCEDB_CHUNKS_STORE,
        "scope-one",
        Some("b_scope_one"),
        provider.clone(),
    )
    .unwrap();
    let error = reader
        .query_chunks(&VectorQuery {
            text: "second scoped task".to_owned(),
            limit: 10,
            board_id: "b_scope_two".to_owned(),
        })
        .expect_err("a board-scoped reader must reject a different request board");
    assert!(
        matches!(
            error,
            VectorProjectionBackendError::Protocol(ref message)
                if message.contains("board mismatch")
                    && message.contains("b_scope_one")
                    && message.contains("b_scope_two")
        ),
        "{error}"
    );
    assert_eq!(
        provider.batch_calls.load(Ordering::SeqCst),
        0,
        "the rejected request must not reach embedding or the physical query"
    );

    let hits = reader
        .query_chunks(&VectorQuery {
            text: "first scoped task".to_owned(),
            limit: 10,
            board_id: "b_scope_one".to_owned(),
        })
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk.entity_uri.as_str(), "kb://task/t_scope_one");
}

#[test]
fn board_scoped_label_reader_rejects_text_and_vector_queries_for_a_different_board() {
    let (_temp, db, backend) = backend();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE boards(
             id TEXT PRIMARY KEY,
             slug TEXT NOT NULL UNIQUE,
             archived_at INTEGER
         );
         INSERT INTO boards(id,slug,archived_at) VALUES
             ('b_scope_one','scope-one',NULL),
             ('b_scope_two','scope-two',NULL);",
    )
    .unwrap();
    drop(conn);
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_LABEL_ATOMS_STORE,
        "gen_label_query_scope",
        1,
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO labels(id,board_id,name) VALUES
             ('l_scope_one','b_scope_one','first urgent'),
             ('l_scope_two','b_scope_two','second urgent');
         INSERT INTO label_atoms(
             id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at
         ) VALUES
             ('la_scope_one','l_scope_one','b_scope_one','positive','name',
              'first urgent',0,'atom-scope-one',1,1),
             ('la_scope_two','l_scope_two','b_scope_two','positive','name',
              'second urgent',0,'atom-scope-two',1,1);",
    )
    .unwrap();
    drop(conn);
    let mut rebuild_one = delivery(&evidence, 904, 2, ProjectionDeliveryAction::Rebuild);
    rebuild_one.board_id = "b_scope_one".to_owned();
    rebuild_one.entity_uri = "kb://board/b_scope_one".to_owned();
    apply(&backend, &db, &evidence, rebuild_one);
    let mut rebuild_two = delivery(&evidence, 905, 3, ProjectionDeliveryAction::Rebuild);
    rebuild_two.board_id = "b_scope_two".to_owned();
    rebuild_two.entity_uri = "kb://board/b_scope_two".to_owned();
    apply(&backend, &db, &evidence, rebuild_two);
    publish(&backend, &db, None, &evidence);

    let provider = Arc::new(CountingProvider::default());
    let reader = ActiveLanceProjectionReader::open_for_board(
        &db,
        LANCEDB_LABEL_ATOMS_STORE,
        "scope-one",
        Some("b_scope_one"),
        provider.clone(),
    )
    .unwrap();
    let text_error = reader
        .query_label_atoms(&LabelAtomQuery {
            text: "second urgent".to_owned(),
            limit: 10,
            board_id: Some("b_scope_two".to_owned()),
            embedding_model: None,
            polarity: None,
        })
        .expect_err("scoped label text query must reject a different board");
    assert!(
        matches!(
            text_error,
            VectorProjectionBackendError::Protocol(ref message)
                if message.contains("board mismatch")
                    && message.contains("b_scope_one")
                    && message.contains("b_scope_two")
        ),
        "{text_error}"
    );
    let vector_error = reader
        .query_label_atoms_by_vector(&kanban_vector::LabelAtomVectorQuery {
            vector: vec![13.0, 1.0],
            limit: 10,
            board_id: Some("b_scope_two".to_owned()),
            embedding_model: None,
            polarity: None,
            include_vector: true,
        })
        .expect_err("scoped label vector query must reject a different board");
    assert!(
        matches!(
            vector_error,
            VectorProjectionBackendError::Protocol(ref message)
                if message.contains("board mismatch")
                    && message.contains("b_scope_one")
                    && message.contains("b_scope_two")
        ),
        "{vector_error}"
    );
    assert_eq!(
        provider.batch_calls.load(Ordering::SeqCst),
        0,
        "the rejected text request must not reach embedding or the physical query"
    );

    drop(reader);
    let unscoped =
        ActiveLanceProjectionReader::open(&db, LANCEDB_LABEL_ATOMS_STORE, Arc::new(StaticProvider))
            .unwrap();
    assert_eq!(unscoped.resolved_board_id(), None);
    assert_eq!(
        unscoped
            .query_label_atoms(&LabelAtomQuery {
                text: "second urgent".to_owned(),
                limit: 10,
                board_id: Some("b_scope_two".to_owned()),
                embedding_model: None,
                polarity: None,
            })
            .unwrap()[0]
            .atom_id,
        "la_scope_two"
    );
    assert_eq!(
        unscoped
            .query_label_atoms_by_vector(&kanban_vector::LabelAtomVectorQuery {
                vector: vec![13.0, 1.0],
                limit: 10,
                board_id: Some("b_scope_two".to_owned()),
                embedding_model: None,
                polarity: None,
                include_vector: true,
            })
            .unwrap()[0]
            .hit
            .atom_id,
        "la_scope_two"
    );
}

#[test]
fn helper_read_operations_fail_busy_while_a_physical_writer_is_active() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_read_operation_guard",
        1,
    );
    let requests = vec![
        (
            "inspect active",
            VectorProjectionHelperRequest::InspectActive(VectorProjectionInspectActiveRequest {
                request_id: "req_guard_inspect_active".to_owned(),
                projection_store: LANCEDB_CHUNKS_STORE.to_owned(),
            }),
        ),
        (
            "inspect generation",
            VectorProjectionHelperRequest::InspectGeneration(
                VectorProjectionInspectGenerationRequest {
                    request_id: "req_guard_inspect_generation".to_owned(),
                    projection_store: LANCEDB_CHUNKS_STORE.to_owned(),
                    generation_id: evidence.manifest.generation.clone(),
                },
            ),
        ),
        (
            "validate generation",
            VectorProjectionHelperRequest::ValidateGenerationPublication(
                VectorProjectionValidateGenerationRequest {
                    request_id: "req_guard_validate_generation".to_owned(),
                    projection_store: LANCEDB_CHUNKS_STORE.to_owned(),
                    expected: evidence.clone(),
                },
            ),
        ),
        (
            "validate active",
            VectorProjectionHelperRequest::ValidateActiveContents(
                VectorProjectionValidateActiveRequest {
                    request_id: "req_guard_validate_active".to_owned(),
                    projection_store: LANCEDB_CHUNKS_STORE.to_owned(),
                    active: evidence.clone(),
                },
            ),
        ),
        (
            "inventory",
            VectorProjectionHelperRequest::Inventory(VectorProjectionInventoryRequest {
                request_id: "req_guard_inventory".to_owned(),
                projection_store: LANCEDB_CHUNKS_STORE.to_owned(),
            }),
        ),
        (
            "cleanup dry-run",
            VectorProjectionHelperRequest::Cleanup(VectorProjectionCleanupRequest {
                context: context(&evidence, "req_guard_cleanup"),
                authority: destructive_authority(
                    &evidence,
                    evidence.manifest.fence_epoch,
                    VectorProjectionGenerationRole::Building,
                    Some(VectorProjectionBuildingPhase::Prepared),
                ),
                dry_run: true,
                protection: VectorProjectionCleanupProtection {
                    active_generation: None,
                    previous_generation: None,
                    building_generation: Some(evidence.manifest.generation.clone()),
                    additional_generations: Vec::new(),
                },
            }),
        ),
    ];
    let writer =
        DerivedStoreWriteGuard::acquire(&db, &format!("{LANCEDB_CHUNKS_STORE}-projection-helper"))
            .unwrap();

    for (operation, request) in requests {
        match backend.execute(&request) {
            VectorProjectionHelperResponse::Error(error) => {
                assert_eq!(error.code, "projection_backend_busy", "{operation}");
                assert!(error.retryable, "{operation}");
            }
            response => panic!("{operation} bypassed the physical writer: {response:?}"),
        }
    }

    drop(writer);
    assert!(matches!(
        backend.execute(&VectorProjectionHelperRequest::InspectActive(
            VectorProjectionInspectActiveRequest {
                request_id: "req_guard_after_release".to_owned(),
                projection_store: LANCEDB_CHUNKS_STORE.to_owned(),
            }
        )),
        VectorProjectionHelperResponse::InspectActive(_)
    ));
}

#[test]
fn active_reader_isolates_nine_board_scopes() {
    let (_temp, db, backend) = backend();
    let conn = rusqlite::Connection::open(&db).unwrap();
    let mut records = Vec::new();
    for ordinal in 1..=9 {
        let board_id = format!("b_{ordinal:02}");
        let task_id = format!("t_{ordinal:02}");
        let title = format!("board {ordinal:02} task");
        conn.execute(
            "INSERT INTO tasks(
                 id,board_id,seq,title,description,status,archived_at,created_at,updated_at
             ) VALUES (?1,?2,1,?3,NULL,'todo',NULL,1,1)",
            rusqlite::params![task_id, board_id, title],
        )
        .unwrap();
        records.push(task_record(&board_id, &task_id, &title));
    }
    drop(conn);
    let evidence = prepare_with_records(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_nine_boards",
        1,
        records,
    );
    publish(&backend, &db, None, &evidence);
    let reader =
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
            .unwrap();

    for ordinal in 1..=9 {
        let board_id = format!("b_{ordinal:02}");
        let hits = reader
            .query_chunks(&VectorQuery {
                text: "task".to_owned(),
                limit: 20,
                board_id,
            })
            .unwrap();
        assert_eq!(hits.len(), 1, "board {ordinal:02}");
        assert_eq!(
            hits[0].chunk.entity_uri.as_str(),
            format!("kb://task/t_{ordinal:02}")
        );
    }
}

#[test]
fn active_label_reader_supports_text_and_vector_queries_with_resolved_board_scope() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_LABEL_ATOMS_STORE,
        "gen_active_labels",
        1,
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "INSERT INTO labels(id,board_id,name) VALUES ('l_reader','b_one','urgent')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO label_atoms(
             id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at
         ) VALUES (
             'la_reader','l_reader','b_one','positive','name','urgent',0,'atom-hash',1,1
         )",
        [],
    )
    .unwrap();
    drop(conn);
    let mut rebuild = delivery(&evidence, 902, 2, ProjectionDeliveryAction::Rebuild);
    rebuild.entity_uri = "kb://board/b_one".to_owned();
    apply(&backend, &db, &evidence, rebuild);
    publish(&backend, &db, None, &evidence);

    let reader =
        ActiveLanceProjectionReader::open(&db, LANCEDB_LABEL_ATOMS_STORE, Arc::new(StaticProvider))
            .unwrap();
    let text_hits = reader
        .query_label_atoms(&LabelAtomQuery {
            text: "urgent".to_owned(),
            limit: 10,
            board_id: Some("b_one".to_owned()),
            embedding_model: None,
            polarity: None,
        })
        .unwrap();
    assert_eq!(text_hits.len(), 1);
    assert_eq!(text_hits[0].atom_id, "la_reader");
    let vector_hits = reader
        .query_label_atoms_by_vector(&kanban_vector::LabelAtomVectorQuery {
            vector: vec![6.0, 1.0],
            limit: 10,
            board_id: Some("b_one".to_owned()),
            embedding_model: None,
            polarity: None,
            include_vector: true,
        })
        .unwrap();
    assert_eq!(vector_hits.len(), 1);
    assert_eq!(vector_hits[0].hit.atom_id, "la_reader");
    assert_eq!(vector_hits[0].vector.as_deref(), Some(&[6.0, 1.0][..]));
    assert!(
        reader
            .query_label_atoms(&LabelAtomQuery {
                text: "urgent".to_owned(),
                limit: 10,
                board_id: Some("b_two".to_owned()),
                embedding_model: None,
                polarity: None,
            })
            .unwrap()
            .is_empty()
    );
}

#[test]
fn active_reader_keeps_two_database_instances_physically_and_logically_isolated() {
    let (temp, first_db) = database();
    let second_db = temp.path().join("second.db");
    fs::copy(&first_db, &second_db).unwrap();
    let conn = rusqlite::Connection::open(&second_db).unwrap();
    conn.execute(
        "UPDATE projection_database SET database_instance_id='db_second'
         WHERE singleton=1",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE projection_store_state SET database_instance_id='db_second'",
        [],
    )
    .unwrap();
    drop(conn);

    let first_backend = VectorProjectionBackend::new(&first_db, Arc::new(StaticProvider)).unwrap();
    let second_backend =
        VectorProjectionBackend::new(&second_db, Arc::new(StaticProvider)).unwrap();
    for (db, task_id, title) in [
        (&first_db, "t_first_db", "first database"),
        (&second_db, "t_second_db", "second database"),
    ] {
        let conn = rusqlite::Connection::open(db).unwrap();
        conn.execute(
            "INSERT INTO tasks(
                 id,board_id,seq,title,description,status,archived_at,created_at,updated_at
             ) VALUES (?1,'b_shared',1,?2,NULL,'todo',NULL,1,1)",
            rusqlite::params![task_id, title],
        )
        .unwrap();
    }
    let first = prepare_with_records(
        &first_backend,
        &first_db,
        LANCEDB_CHUNKS_STORE,
        "gen_first_database",
        1,
        vec![task_record("b_shared", "t_first_db", "first database")],
    );
    publish(&first_backend, &first_db, None, &first);
    let second = prepare_with_records(
        &second_backend,
        &second_db,
        LANCEDB_CHUNKS_STORE,
        "gen_second_database",
        1,
        vec![task_record("b_shared", "t_second_db", "second database")],
    );
    publish(&second_backend, &second_db, None, &second);

    let first_reader = ActiveLanceProjectionReader::open(
        &first_db,
        LANCEDB_CHUNKS_STORE,
        Arc::new(StaticProvider),
    )
    .unwrap();
    let second_reader = ActiveLanceProjectionReader::open(
        &second_db,
        LANCEDB_CHUNKS_STORE,
        Arc::new(StaticProvider),
    )
    .unwrap();
    let query = VectorQuery {
        text: "database".to_owned(),
        limit: 10,
        board_id: "b_shared".to_owned(),
    };
    let first_hits = first_reader.query_chunks(&query).unwrap();
    let second_hits = second_reader.query_chunks(&query).unwrap();
    assert_eq!(first_hits.len(), 1);
    assert_eq!(
        first_hits[0].chunk.entity_uri.as_str(),
        "kb://task/t_first_db"
    );
    assert_eq!(second_hits.len(), 1);
    assert_eq!(
        second_hits[0].chunk.entity_uri.as_str(),
        "kb://task/t_second_db"
    );
}

#[test]
fn active_reader_rejects_every_sqlite_authority_or_corpus_mismatch() {
    let mutations = [
        (
            "multiple projection database identities",
            "INSERT INTO projection_database(singleton,database_instance_id,protocol_version)
             VALUES (2,'db_extra',2)",
        ),
        (
            "projection database protocol version",
            "UPDATE projection_database SET protocol_version=1 WHERE singleton=1",
        ),
        (
            "legacy control plane",
            "UPDATE projection_store_state SET control_plane='legacy'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "protocol version",
            "UPDATE projection_store_state SET protocol_version=1
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "schema version",
            "UPDATE projection_store_state SET schema_version=2
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "database identity",
            "UPDATE projection_store_state SET database_instance_id='db_wrong'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "generation",
            "UPDATE projection_store_state SET active_generation='gen_wrong'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "fingerprint",
            "UPDATE projection_store_state SET active_fingerprint='fnv64:wrong'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "fence",
            "UPDATE projection_store_state SET active_fence_epoch=99
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "provider",
            "UPDATE projection_store_state SET active_provider='wrong'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "provider fingerprint",
            "UPDATE projection_store_state SET active_provider_fingerprint='wrong'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "unbound v29 corpus",
            "UPDATE projection_store_state
             SET active_corpus_schema=NULL,active_corpus_fingerprint=NULL,
                 active_embedding_model=NULL,active_embedding_dimensions=NULL
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "corpus schema",
            "UPDATE projection_store_state SET active_corpus_schema='label-atoms-v2'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "corpus fingerprint",
            "UPDATE projection_store_state SET active_corpus_fingerprint='wrong'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "embedding model",
            "UPDATE projection_store_state SET active_embedding_model='wrong'
             WHERE store_name='lancedb_chunks'",
        ),
        (
            "embedding dimensions",
            "UPDATE projection_store_state SET active_embedding_dimensions=3
             WHERE store_name='lancedb_chunks'",
        ),
    ];

    for (name, mutation) in mutations {
        let (_temp, db, backend) = backend();
        let evidence = prepare(
            &backend,
            &db,
            LANCEDB_CHUNKS_STORE,
            "gen_authority_binding",
            1,
        );
        publish(&backend, &db, None, &evidence);
        rusqlite::Connection::open(&db)
            .unwrap()
            .execute_batch(mutation)
            .unwrap();

        assert!(
            ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
                .is_err(),
            "reader accepted {name}"
        );
    }
}

#[test]
fn active_reader_rejects_a_provider_model_or_dimension_upgrade_without_rebuild() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_original_reader_provider",
        1,
    );
    publish(&backend, &db, None, &evidence);

    assert!(
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(AlternateProvider))
            .is_err()
    );
}

#[test]
fn active_reader_rejects_missing_or_corrupt_generation_evidence_without_repair() {
    for relative in [
        "published",
        "projection-evidence.json",
        "projection-snapshot.json",
        "projection-content.json",
        "embedding-cache.json",
        "lance/kb_chunks.lance",
    ] {
        let (_temp, db, backend) = backend();
        let evidence = prepare(
            &backend,
            &db,
            LANCEDB_CHUNKS_STORE,
            "gen_physical_failure",
            1,
        );
        publish(&backend, &db, None, &evidence);
        let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
        let target = generation.join(relative);
        if target.is_dir() {
            fs::remove_dir_all(&target).unwrap();
        } else {
            fs::remove_file(&target).unwrap();
        }
        let before = filesystem_digest(temp_root(&db));

        assert!(
            ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
                .is_err(),
            "reader accepted missing {relative}"
        );
        assert_eq!(
            filesystem_digest(temp_root(&db)),
            before,
            "reader repaired or recreated missing {relative}"
        );
    }

    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_corrupt_active_marker",
        1,
    );
    publish(&backend, &db, None, &evidence);
    fs::write(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&evidence.manifest.generation)
            .join("published"),
        b"corrupt marker",
    )
    .unwrap();
    assert!(
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn active_reader_fails_closed_after_lance_table_directory_replacement() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_reader_path_identity",
        1,
    );
    publish(&backend, &db, None, &evidence);
    let reader =
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
            .unwrap();
    let table = generations(&db, LANCEDB_CHUNKS_STORE)
        .join(&evidence.manifest.generation)
        .join("lance")
        .join("kb_chunks.lance");
    let displaced = table.with_extension("displaced");
    fs::rename(&table, &displaced).unwrap();
    fs::create_dir(&table).unwrap();

    let error = reader
        .query_chunks(&VectorQuery {
            text: "replacement".to_owned(),
            limit: 10,
            board_id: "b_one".to_owned(),
        })
        .expect_err("a replaced generation path must fail closed before querying");
    assert!(
        error.to_string().contains("unsafe directory path"),
        "{error}"
    );

    drop(reader);
    fs::remove_dir(&table).unwrap();
    fs::rename(displaced, table).unwrap();
}

#[test]
fn active_reader_rejects_a_newer_published_physical_generation() {
    let (_temp, db, backend) = backend();
    let active = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_sqlite_active", 1);
    publish(&backend, &db, None, &active);
    let newer = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_physical_newer", 2);
    let response = backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
        VectorProjectionPublishRequest {
            context: context(&newer, "req_publish_without_sqlite_swap"),
            authority: destructive_authority(
                &newer,
                newer.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected_active: Some(active),
            prepared: newer,
        },
    )));
    assert!(matches!(
        response,
        VectorProjectionHelperResponse::Publish(_)
    ));

    assert!(
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
            .is_err(),
        "SQLite active must not silently lose to a newer physical publication"
    );
}

#[cfg(unix)]
#[test]
fn active_reader_rejects_a_symlinked_active_generation() {
    let (temp, db, backend) = backend();
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_active_symlink", 1);
    publish(&backend, &db, None, &evidence);
    let managed = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let external = temp.path().join("external-active-generation");
    fs::rename(&managed, &external).unwrap();
    std::os::unix::fs::symlink(&external, &managed).unwrap();

    assert!(
        ActiveLanceProjectionReader::open(&db, LANCEDB_CHUNKS_STORE, Arc::new(StaticProvider))
            .is_err()
    );
    assert!(managed.is_symlink());
    assert!(external.is_dir());
}

#[test]
fn prepare_rejects_schema_version_mismatch_before_creating_a_generation() {
    let (_temp, db, backend) = backend();
    let mut request = prepare_request(
        &backend,
        LANCEDB_CHUNKS_STORE,
        "gen_wrong_schema",
        1,
        Vec::new(),
    );
    request.snapshot.manifest.schema_version += 1;

    let response = backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join("gen_wrong_schema")
            .exists()
    );
}

#[test]
fn publish_retains_previous_generation_and_inventory_marks_both() {
    let (_temp, db, backend) = backend();
    let first = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_first", 1);
    publish(&backend, &db, None, &first);
    let second = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_second", 2);
    let receipt = publish(&backend, &db, Some(&first), &second);

    assert_eq!(receipt.active, second);
    assert_eq!(receipt.retained_previous, Some(first.clone()));
    let inventory = inventory(&backend, LANCEDB_CHUNKS_STORE);
    assert!(inventory.iter().any(|entry| {
        entry.generation_id == "gen_first"
            && entry.state == VectorProjectionGenerationState::Previous
            && entry.evidence.as_ref() == Some(&first)
    }));
    assert!(inventory.iter().any(|entry| {
        entry.generation_id == "gen_second"
            && entry.state == VectorProjectionGenerationState::Active
            && entry.evidence.as_ref() == Some(&second)
    }));
}

#[test]
fn provider_upgrade_inspects_retains_and_cleans_historical_generation_without_rebinding_it() {
    let (_temp, db) = database();
    let original =
        VectorProjectionBackend::new(&db, Arc::new(StaticProvider)).expect("original backend");
    let first = prepare(
        &original,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_original_provider",
        1,
    );
    publish(&original, &db, None, &first);

    let upgraded =
        VectorProjectionBackend::new(&db, Arc::new(AlternateProvider)).expect("upgraded backend");
    let before = inventory(&upgraded, LANCEDB_CHUNKS_STORE);
    assert!(before.iter().any(|entry| {
        entry.generation_id == first.manifest.generation
            && entry.state == VectorProjectionGenerationState::Active
            && entry.evidence.as_ref() == Some(&first)
    }));
    assert!(
        validate_generation(&upgraded, &first),
        "historical publication validation must use the generation's own provider/model/dimensions"
    );

    let rejected_apply =
        upgraded.execute(&VectorProjectionHelperRequest::ApplyBatch(apply_request(
            &first,
            delivery(&first, 100, 2, ProjectionDeliveryAction::Upsert),
        )));
    match rejected_apply {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
        }
        response => panic!("provider upgrade wrote a historical generation: {response:?}"),
    }

    let second = prepare(
        &upgraded,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_upgraded_provider",
        2,
    );
    publish(&upgraded, &db, Some(&first), &second);
    let cleanup_candidate = prepare(
        &upgraded,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_upgraded_cleanup_candidate",
        3,
    );
    clear_building_authority(&db, LANCEDB_CHUNKS_STORE);
    let cleaned = cleanup(
        &upgraded,
        &second,
        cleanup_candidate.manifest.fence_epoch,
        VectorProjectionGenerationRole::Active,
        None,
        false,
        VectorProjectionCleanupProtection {
            active_generation: Some(second.manifest.generation.clone()),
            previous_generation: Some(first.manifest.generation.clone()),
            building_generation: None,
            additional_generations: Vec::new(),
        },
    );
    assert_eq!(
        cleaned.removed_generations,
        vec![cleanup_candidate.manifest.generation.clone()]
    );

    let after = inventory(&upgraded, LANCEDB_CHUNKS_STORE);
    assert!(after.iter().any(|entry| {
        entry.generation_id == first.manifest.generation
            && entry.state == VectorProjectionGenerationState::Previous
            && entry.evidence.as_ref() == Some(&first)
    }));
    assert!(after.iter().any(|entry| {
        entry.generation_id == second.manifest.generation
            && entry.state == VectorProjectionGenerationState::Active
            && entry.evidence.as_ref() == Some(&second)
    }));
}

#[test]
fn provider_upgrade_refuses_to_publish_when_retained_previous_is_not_physically_recoverable() {
    let (_temp, db) = database();
    let original =
        VectorProjectionBackend::new(&db, Arc::new(StaticProvider)).expect("original backend");
    let first = prepare(
        &original,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_corrupt_previous",
        1,
    );
    publish(&original, &db, None, &first);
    fs::remove_file(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&first.manifest.generation)
            .join("projection-content.json"),
    )
    .unwrap();

    let upgraded =
        VectorProjectionBackend::new(&db, Arc::new(AlternateProvider)).expect("upgraded backend");
    let second = prepare(
        &upgraded,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_after_corrupt_previous",
        2,
    );
    let response = upgraded.execute(&VectorProjectionHelperRequest::Publish(Box::new(
        VectorProjectionPublishRequest {
            context: context(&second, "req_publish_after_corrupt_previous"),
            authority: destructive_authority(
                &second,
                second.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected_active: Some(first),
            prepared: second.clone(),
        },
    )));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&second.manifest.generation)
            .join("published")
            .exists(),
        "a new marker must not be created when rollback generation validation fails"
    );
}

#[test]
fn publish_retry_repairs_only_the_prepared_generations_corrupt_marker() {
    let (_temp, db, backend) = backend();
    let first = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_publish_retry_file",
        1,
    );
    let first_marker = generations(&db, LANCEDB_CHUNKS_STORE)
        .join(&first.manifest.generation)
        .join("published");
    fs::write(&first_marker, b"corrupt prepared marker").unwrap();
    publish(&backend, &db, None, &first);
    assert!(first_marker.is_file());
    assert!(validate_generation(&backend, &first));

    let second = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_publish_retry_directory",
        2,
    );
    let second_marker = generations(&db, LANCEDB_CHUNKS_STORE)
        .join(&second.manifest.generation)
        .join("published");
    fs::create_dir(&second_marker).unwrap();
    publish(&backend, &db, Some(&first), &second);
    assert!(second_marker.is_file());
    assert!(validate_generation(&backend, &second));
}

#[test]
fn publish_retry_fails_closed_on_another_generations_corrupt_marker() {
    let (_temp, db, backend) = backend();
    let active = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_publish_retry_active",
        1,
    );
    publish(&backend, &db, None, &active);
    let other = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_publish_retry_other",
        2,
    );
    fs::write(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&other.manifest.generation)
            .join("published"),
        b"unattributed corrupt marker",
    )
    .unwrap();
    let candidate = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_publish_retry_candidate",
        3,
    );

    let response = backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
        VectorProjectionPublishRequest {
            context: context(&candidate, "req_publish_other_corrupt"),
            authority: destructive_authority(
                &candidate,
                candidate.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected_active: Some(active),
            prepared: candidate.clone(),
        },
    )));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&candidate.manifest.generation)
            .join("published")
            .exists()
    );
}

#[test]
fn repair_publication_recovers_a_prepared_generation_after_marker_crash() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_repair", 3);
    let context = context(&evidence, "req_repair");
    let response = backend.execute(&VectorProjectionHelperRequest::RepairPublication(
        VectorProjectionRepairPublicationRequest {
            context,
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected: evidence.clone(),
        },
    ));
    assert!(matches!(
        response,
        VectorProjectionHelperResponse::RepairPublication(_)
    ));

    let inventory = inventory(&backend, LANCEDB_CHUNKS_STORE);
    assert!(inventory.iter().any(|entry| {
        entry.generation_id == "gen_repair"
            && entry.state == VectorProjectionGenerationState::Active
    }));
}

#[test]
fn repair_publication_replaces_only_the_expected_generations_corrupt_marker() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_repair_marker", 1);
    publish(&backend, &db, None, &evidence);
    let marker = generations(&db, LANCEDB_CHUNKS_STORE)
        .join(&evidence.manifest.generation)
        .join("published");
    fs::write(&marker, b"corrupt marker").unwrap();

    let response = backend.execute(&VectorProjectionHelperRequest::RepairPublication(
        VectorProjectionRepairPublicationRequest {
            context: context(&evidence, "req_repair_own_marker"),
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Active,
                None,
            ),
            expected: evidence.clone(),
        },
    ));
    assert!(matches!(
        response,
        VectorProjectionHelperResponse::RepairPublication(_)
    ));
    assert!(validate_generation(&backend, &evidence));
}

#[test]
fn repair_publication_quarantines_the_expected_generations_marker_directory() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_repair_marker_directory",
        1,
    );
    publish(&backend, &db, None, &evidence);
    let marker = generations(&db, LANCEDB_CHUNKS_STORE)
        .join(&evidence.manifest.generation)
        .join("published");
    fs::remove_file(&marker).unwrap();
    fs::create_dir(&marker).unwrap();

    let response = backend.execute(&VectorProjectionHelperRequest::RepairPublication(
        VectorProjectionRepairPublicationRequest {
            context: context(&evidence, "req_repair_marker_directory"),
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Active,
                None,
            ),
            expected: evidence.clone(),
        },
    ));
    assert!(matches!(
        response,
        VectorProjectionHelperResponse::RepairPublication(_)
    ));
    assert!(marker.is_file());
    assert!(validate_generation(&backend, &evidence));
}

#[test]
fn repair_publication_fails_closed_on_another_generations_corrupt_marker() {
    let (_temp, db, backend) = backend();
    let other = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_other_corrupt_marker",
        1,
    );
    publish(&backend, &db, None, &other);
    fs::write(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&other.manifest.generation)
            .join("published"),
        b"corrupt marker",
    )
    .unwrap();
    let expected = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_repair_candidate",
        2,
    );

    let response = backend.execute(&VectorProjectionHelperRequest::RepairPublication(
        VectorProjectionRepairPublicationRequest {
            context: context(&expected, "req_repair_with_other_corrupt"),
            authority: destructive_authority(
                &expected,
                expected.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected: expected.clone(),
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&expected.manifest.generation)
            .join("published")
            .exists()
    );
}

#[test]
fn repair_publication_rejects_corrupt_physical_rows_without_creating_a_marker() {
    let (_temp, db) = database();
    let backend =
        VectorProjectionBackend::new(&db, Arc::new(StaticProvider)).expect("configured backend");
    let request = prepare_request(
        &backend,
        LANCEDB_CHUNKS_STORE,
        "gen_corrupt_repair",
        1,
        vec![task_record("b_one", "t_one", "physical row")],
    );
    authorize_snapshotting(&db, &request);
    let evidence = match backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request)) {
        VectorProjectionHelperResponse::PrepareSnapshot(response) => {
            mark_prepared_authority(&db, &response.evidence);
            response.evidence
        }
        response => panic!("unexpected prepare response: {response:?}"),
    };
    generation_store(&db, &evidence)
        .delete_entities(&["kb://task/t_one".to_owned()])
        .unwrap();

    let response = backend.execute(&VectorProjectionHelperRequest::RepairPublication(
        VectorProjectionRepairPublicationRequest {
            context: context(&evidence, "req_repair_corrupt"),
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected: evidence.clone(),
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&evidence.manifest.generation)
            .join("published")
            .exists(),
        "repair must never publish a generation whose physical fingerprint drifted"
    );
}

#[test]
fn publish_requires_a_present_and_exactly_bound_embedding_cache() {
    let (_temp, db, backend) = backend();
    let missing = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_cache_missing", 1);
    fs::remove_file(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&missing.manifest.generation)
            .join("embedding-cache.json"),
    )
    .unwrap();
    let missing_response = backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
        VectorProjectionPublishRequest {
            context: context(&missing, "req_publish_cache_missing"),
            authority: destructive_authority(
                &missing,
                missing.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected_active: None,
            prepared: missing.clone(),
        },
    )));
    assert!(matches!(
        missing_response,
        VectorProjectionHelperResponse::Error(_)
    ));

    let wrong = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_cache_wrong_binding",
        2,
    );
    let cache_path = generations(&db, LANCEDB_CHUNKS_STORE)
        .join(&wrong.manifest.generation)
        .join("embedding-cache.json");
    let mut cache: serde_json::Value =
        serde_json::from_slice(&fs::read(&cache_path).unwrap()).unwrap();
    cache["provider_fingerprint"] = serde_json::json!("wrong-provider-fingerprint");
    fs::write(&cache_path, serde_json::to_vec(&cache).unwrap()).unwrap();
    let wrong_response = backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
        VectorProjectionPublishRequest {
            context: context(&wrong, "req_publish_cache_wrong_binding"),
            authority: destructive_authority(
                &wrong,
                wrong.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected_active: None,
            prepared: wrong.clone(),
        },
    )));
    assert!(matches!(
        wrong_response,
        VectorProjectionHelperResponse::Error(_)
    ));
    for evidence in [&missing, &wrong] {
        assert!(
            !generations(&db, LANCEDB_CHUNKS_STORE)
                .join(&evidence.manifest.generation)
                .join("published")
                .exists()
        );
    }
}

#[test]
fn repair_rejects_wrong_bound_delivery_state_without_rewriting_it() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_repair_delivery_state",
        1,
    );
    let corpus = evidence.manifest.corpus.as_ref().unwrap();
    let state_path = generations(&db, LANCEDB_CHUNKS_STORE)
        .join(&evidence.manifest.generation)
        .join("delivery-state.json");
    let wrong_state = serde_json::json!({
        "format_version": 1,
        "database_instance_id": evidence.manifest.database_instance_id.clone(),
        "store_name": evidence.manifest.store_name.clone(),
        "generation_id": evidence.manifest.generation.clone(),
        "provider_fingerprint": "wrong-provider-fingerprint",
        "corpus_fingerprint": corpus.corpus_fingerprint.clone(),
        "evidence_fingerprint": evidence.fingerprint.clone(),
        "applied": {},
    });
    let original = serde_json::to_vec(&wrong_state).unwrap();
    fs::write(&state_path, &original).unwrap();

    let response = backend.execute(&VectorProjectionHelperRequest::RepairPublication(
        VectorProjectionRepairPublicationRequest {
            context: context(&evidence, "req_repair_wrong_delivery_state"),
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected: evidence.clone(),
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert_eq!(fs::read(&state_path).unwrap(), original);
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&evidence.manifest.generation)
            .join("published")
            .exists()
    );
}

#[test]
fn active_health_reports_corrupt_auxiliary_state_as_invalid() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_active_corrupt_cache",
        1,
    );
    publish(&backend, &db, None, &evidence);
    fs::write(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&evidence.manifest.generation)
            .join("embedding-cache.json"),
        b"not valid cache json",
    )
    .unwrap();

    assert!(!validate_generation(&backend, &evidence));
    assert!(!validate_active(&backend, &evidence));
}

#[test]
fn publish_rejects_a_retained_previous_with_unrecoverable_auxiliary_state() {
    let (_temp, db, backend) = backend();
    let first = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_previous_cache_missing",
        1,
    );
    publish(&backend, &db, None, &first);
    fs::remove_file(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&first.manifest.generation)
            .join("embedding-cache.json"),
    )
    .unwrap();
    let second = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_after_previous_cache_missing",
        2,
    );

    let response = backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
        VectorProjectionPublishRequest {
            context: context(&second, "req_publish_previous_cache_missing"),
            authority: destructive_authority(
                &second,
                second.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected_active: Some(first),
            prepared: second.clone(),
        },
    )));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&second.manifest.generation)
            .join("published")
            .exists()
    );
}

#[test]
fn quarantine_moves_the_whole_generation_and_preserves_evidence() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_LABEL_ATOMS_STORE,
        "gen_quarantine",
        4,
    );
    let root = generations(&db, LANCEDB_LABEL_ATOMS_STORE);
    let original = root.join("gen_quarantine");
    assert!(original.join("projection-evidence.json").is_file());

    let response = backend.execute(&VectorProjectionHelperRequest::Quarantine(
        VectorProjectionGenerationMutationRequest {
            context: context(&evidence, "req_quarantine"),
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
        },
    ));
    assert!(matches!(
        response,
        VectorProjectionHelperResponse::Quarantine(_)
    ));
    assert!(!original.exists());
    let quarantined = fs::read_dir(&root)
        .unwrap()
        .map(Result::unwrap)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".gen_quarantine.quarantine.")
        })
        .expect("whole generation quarantine sibling");
    assert!(
        quarantined
            .path()
            .join("projection-evidence.json")
            .is_file()
    );
    let persisted: ProjectionArtifactEvidence = serde_json::from_slice(
        &fs::read(quarantined.path().join("projection-evidence.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(persisted, evidence);
}

#[test]
fn inspect_generation_rejects_evidence_moved_under_a_different_generation_name() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_path_original", 1);
    let root = generations(&db, LANCEDB_CHUNKS_STORE);
    fs::rename(
        root.join(&evidence.manifest.generation),
        root.join("gen_path_alias"),
    )
    .unwrap();

    let response = backend.execute(&VectorProjectionHelperRequest::InspectGeneration(
        VectorProjectionInspectGenerationRequest {
            request_id: "req_inspect_path_alias".to_owned(),
            projection_store: LANCEDB_CHUNKS_STORE.to_owned(),
            generation_id: "gen_path_alias".to_owned(),
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
}

#[test]
fn abort_requires_the_persisted_delivery_digest_and_never_removes_a_published_generation() {
    let (_temp, db, backend) = backend();
    let prepared = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_abort_guarded", 1);
    let mut wrong_context = context(&prepared, "req_abort_wrong_digest");
    wrong_context.delivery_digest = "fnv64:not-the-generation-digest".to_owned();
    let rejected = backend.execute(&VectorProjectionHelperRequest::Abort(
        VectorProjectionGenerationMutationRequest {
            context: wrong_context,
            authority: destructive_authority(
                &prepared,
                prepared.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
        },
    ));
    assert!(matches!(rejected, VectorProjectionHelperResponse::Error(_)));
    assert!(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&prepared.manifest.generation)
            .is_dir()
    );

    let published = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_abort_published",
        2,
    );
    publish(&backend, &db, None, &published);
    let rejected = backend.execute(&VectorProjectionHelperRequest::Abort(
        VectorProjectionGenerationMutationRequest {
            context: context(&published, "req_abort_published"),
            authority: destructive_authority(
                &published,
                published.manifest.fence_epoch,
                VectorProjectionGenerationRole::Active,
                None,
            ),
        },
    ));
    assert!(matches!(rejected, VectorProjectionHelperResponse::Error(_)));
    assert!(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&published.manifest.generation)
            .is_dir()
    );
}

#[test]
fn cleanup_dry_run_and_real_cleanup_never_remove_protected_generations() {
    let (_temp, db, backend) = backend();
    let previous = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_previous", 1);
    publish(&backend, &db, None, &previous);
    let active = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_active", 2);
    publish(&backend, &db, Some(&previous), &active);
    let building = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_building", 3);
    let explicit = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_explicit", 4);
    let candidate = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_candidate", 5);
    clear_building_authority(&db, LANCEDB_CHUNKS_STORE);
    let protection = VectorProjectionCleanupProtection {
        active_generation: Some(active.manifest.generation.clone()),
        previous_generation: Some(previous.manifest.generation.clone()),
        building_generation: Some(building.manifest.generation.clone()),
        additional_generations: vec![explicit.manifest.generation.clone()],
    };
    let dry_run = cleanup(
        &backend,
        &active,
        candidate.manifest.fence_epoch,
        VectorProjectionGenerationRole::Active,
        None,
        true,
        protection.clone(),
    );
    assert!(dry_run.removed_generations.is_empty());
    assert!(dry_run.skipped_generations.iter().any(|entry| {
        entry.generation_id == candidate.manifest.generation && entry.reason == "dry_run"
    }));
    for generation in [
        &previous.manifest.generation,
        &active.manifest.generation,
        &building.manifest.generation,
        &explicit.manifest.generation,
        &candidate.manifest.generation,
    ] {
        assert!(
            generations(&db, LANCEDB_CHUNKS_STORE)
                .join(generation)
                .is_dir()
        );
    }

    let cleaned = cleanup(
        &backend,
        &active,
        candidate.manifest.fence_epoch,
        VectorProjectionGenerationRole::Active,
        None,
        false,
        protection,
    );
    assert_eq!(
        cleaned.removed_generations,
        vec![candidate.manifest.generation.clone()]
    );
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&candidate.manifest.generation)
            .exists()
    );
    for generation in [
        &previous.manifest.generation,
        &active.manifest.generation,
        &building.manifest.generation,
        &explicit.manifest.generation,
    ] {
        assert!(
            generations(&db, LANCEDB_CHUNKS_STORE)
                .join(generation)
                .is_dir()
        );
    }
}

#[test]
fn cleanup_rejects_a_stale_or_wrong_context_digest_before_removing_any_generation() {
    let (_temp, db, backend) = backend();
    let authority = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_cleanup_authority",
        1,
    );
    publish(&backend, &db, None, &authority);
    let victim = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_cleanup_victim", 2);
    clear_building_authority(&db, LANCEDB_CHUNKS_STORE);
    let mut wrong_context = context(&authority, "req_cleanup_wrong_digest");
    wrong_context.delivery_digest = "fnv64:stale-cleanup-authority".to_owned();

    let response = backend.execute(&VectorProjectionHelperRequest::Cleanup(
        VectorProjectionCleanupRequest {
            context: wrong_context,
            authority: destructive_authority(
                &authority,
                victim.manifest.fence_epoch,
                VectorProjectionGenerationRole::Active,
                None,
            ),
            dry_run: false,
            protection: VectorProjectionCleanupProtection {
                active_generation: Some(authority.manifest.generation.clone()),
                previous_generation: None,
                building_generation: None,
                additional_generations: Vec::new(),
            },
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    for generation in [
        authority.manifest.generation.as_str(),
        victim.manifest.generation.as_str(),
    ] {
        assert!(
            generations(&db, LANCEDB_CHUNKS_STORE)
                .join(generation)
                .is_dir(),
            "cleanup authorization failure must be zero-delete"
        );
    }
}

#[test]
fn cleanup_rejects_a_missing_context_generation_before_removing_any_generation() {
    let (temp, db, backend) = backend();
    let authority = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_cleanup_missing_authority",
        1,
    );
    publish(&backend, &db, None, &authority);
    let victim = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_cleanup_missing_victim",
        2,
    );
    clear_building_authority(&db, LANCEDB_CHUNKS_STORE);
    let root = generations(&db, LANCEDB_CHUNKS_STORE);
    let missing_authority = root.join(&authority.manifest.generation);
    let displaced_authority = temp.path().join("missing-context-authority");
    fs::rename(&missing_authority, &displaced_authority).unwrap();

    let response = backend.execute(&VectorProjectionHelperRequest::Cleanup(
        VectorProjectionCleanupRequest {
            context: context(&authority, "req_cleanup_missing_context"),
            authority: destructive_authority(
                &authority,
                victim.manifest.fence_epoch,
                VectorProjectionGenerationRole::Active,
                None,
            ),
            dry_run: false,
            protection: VectorProjectionCleanupProtection {
                active_generation: Some(authority.manifest.generation.clone()),
                previous_generation: None,
                building_generation: None,
                additional_generations: Vec::new(),
            },
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(!missing_authority.exists());
    assert!(displaced_authority.is_dir());
    assert!(
        root.join(&victim.manifest.generation).is_dir(),
        "missing canonical context must not authorize deleting an orphan"
    );
}

#[test]
fn cleanup_rejects_a_corrupt_context_snapshot_before_removing_any_generation() {
    let (_temp, db, backend) = backend();
    let authority = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_cleanup_corrupt_authority",
        1,
    );
    publish(&backend, &db, None, &authority);
    let victim = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_cleanup_corrupt_victim",
        2,
    );
    clear_building_authority(&db, LANCEDB_CHUNKS_STORE);
    fs::write(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&authority.manifest.generation)
            .join("projection-snapshot.json"),
        b"not valid snapshot json",
    )
    .unwrap();

    let response = backend.execute(&VectorProjectionHelperRequest::Cleanup(
        VectorProjectionCleanupRequest {
            context: context(&authority, "req_cleanup_corrupt_context"),
            authority: destructive_authority(
                &authority,
                victim.manifest.fence_epoch,
                VectorProjectionGenerationRole::Active,
                None,
            ),
            dry_run: false,
            protection: VectorProjectionCleanupProtection {
                active_generation: Some(authority.manifest.generation.clone()),
                previous_generation: None,
                building_generation: None,
                additional_generations: Vec::new(),
            },
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&victim.manifest.generation)
            .is_dir()
    );
}

#[cfg(unix)]
#[test]
fn cleanup_rejects_a_context_generation_symlink_before_removing_any_generation() {
    let (temp, db, backend) = backend();
    let authority = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_cleanup_symlink_authority",
        1,
    );
    publish(&backend, &db, None, &authority);
    let victim = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_cleanup_symlink_victim",
        2,
    );
    clear_building_authority(&db, LANCEDB_CHUNKS_STORE);
    let root = generations(&db, LANCEDB_CHUNKS_STORE);
    let managed_authority = root.join(&authority.manifest.generation);
    let external_authority = temp.path().join("external-valid-generation");
    fs::rename(&managed_authority, &external_authority).unwrap();
    std::os::unix::fs::symlink(&external_authority, &managed_authority).unwrap();

    let response = backend.execute(&VectorProjectionHelperRequest::Cleanup(
        VectorProjectionCleanupRequest {
            context: context(&authority, "req_cleanup_symlink_context"),
            authority: destructive_authority(
                &authority,
                victim.manifest.fence_epoch,
                VectorProjectionGenerationRole::Active,
                None,
            ),
            dry_run: false,
            protection: VectorProjectionCleanupProtection {
                active_generation: Some(authority.manifest.generation.clone()),
                previous_generation: None,
                building_generation: None,
                additional_generations: Vec::new(),
            },
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(managed_authority.is_symlink());
    assert!(external_authority.is_dir());
    assert!(
        root.join(&victim.manifest.generation).is_dir(),
        "an external context symlink must never authorize deleting managed victims"
    );
}

#[test]
fn apply_is_restart_deduplicated_replay_idempotent_and_delete_restore_safe() {
    let (temp, db) = database();
    let provider = Arc::new(CountingProvider::default());
    let backend = VectorProjectionBackend::new(&db, provider.clone()).expect("configured backend");
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_apply", 1);
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_one','b_one',1,'same semantic text',NULL,'todo',NULL,1,1)",
        [],
    )
    .unwrap();
    drop(conn);

    apply(
        &backend,
        &db,
        &evidence,
        delivery(&evidence, 1, 2, ProjectionDeliveryAction::Upsert),
    );
    assert_eq!(provider.batch_calls.load(Ordering::SeqCst), 1);
    assert_eq!(provider.successful_count("same semantic text"), 1);
    let cache: serde_json::Value = serde_json::from_slice(
        &fs::read(
            generations(&db, LANCEDB_CHUNKS_STORE)
                .join(&evidence.manifest.generation)
                .join("embedding-cache.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let cache_key = cache["entries"]
        .as_object()
        .and_then(|entries| entries.keys().next())
        .expect("one persisted embedding cache key");
    for binding in [
        "store:",
        "lancedb_chunks",
        "provider:",
        "model:13:fixture-model",
        "dimensions:2",
        "content_hash:",
    ] {
        assert!(
            cache_key.contains(binding),
            "embedding cache key must contain binding {binding}: {cache_key}"
        );
    }
    let initial_hits = chunk_hits(&db, &evidence, "b_one");
    assert_eq!(initial_hits.len(), 1);
    assert_eq!(initial_hits[0].chunk.entity_uri.as_str(), "kb://task/t_one");
    let stable_identity = (
        initial_hits[0].chunk.uri.as_str().to_owned(),
        initial_hits[0].chunk.content_hash.clone(),
    );

    let restarted = VectorProjectionBackend::new(&db, provider.clone()).expect("restarted backend");
    apply(
        &restarted,
        &db,
        &evidence,
        delivery(&evidence, 2, 3, ProjectionDeliveryAction::Upsert),
    );
    assert_eq!(
        provider.batch_calls.load(Ordering::SeqCst),
        1,
        "same semantic content must be read from the generation cache after restart"
    );

    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE tasks SET status='archived',archived_at=10,updated_at=10 WHERE id='t_one'",
        [],
    )
    .unwrap();
    drop(conn);
    apply(
        &restarted,
        &db,
        &evidence,
        delivery(&evidence, 3, 4, ProjectionDeliveryAction::Upsert),
    );
    assert!(
        chunk_hits(&db, &evidence, "b_one").is_empty(),
        "Upsert is only an invalidation: archived canonical SQLite truth must remain absent"
    );

    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE tasks SET status='todo',archived_at=NULL,updated_at=11 WHERE id='t_one'",
        [],
    )
    .unwrap();
    drop(conn);
    let restore = delivery(&evidence, 4, 5, ProjectionDeliveryAction::Upsert);
    apply(&restarted, &db, &evidence, restore.clone());
    apply(&restarted, &db, &evidence, restore);
    assert_eq!(
        provider.batch_calls.load(Ordering::SeqCst),
        1,
        "restore and exact replay must reuse the persisted embedding"
    );
    let restored = chunk_hits(&db, &evidence, "b_one");
    assert_eq!(restored.len(), 1);
    assert_eq!(
        (
            restored[0].chunk.uri.as_str().to_owned(),
            restored[0].chunk.content_hash.clone(),
        ),
        stable_identity,
        "ChunkBuilder identity and semantic content hash must survive restart/delete/restore"
    );

    apply(
        &restarted,
        &db,
        &evidence,
        delivery(&evidence, 5, 6, ProjectionDeliveryAction::Delete),
    );
    assert_eq!(
        chunk_hits(&db, &evidence, "b_one").len(),
        1,
        "Delete is only an invalidation: live canonical SQLite truth must be rehydrated"
    );
    drop(temp);
}

#[test]
fn concurrent_helper_process_equivalents_preserve_delivery_and_embedding_cache_union() {
    let (_temp, db) = database();
    let provider = Arc::new(CountingProvider::default());
    let backend =
        Arc::new(VectorProjectionBackend::new(&db, provider.clone()).expect("configured backend"));
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_concurrent_apply",
        1,
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES
             ('t_one','b_one',1,'first concurrent text',NULL,'todo',NULL,1,1),
             ('t_two','b_one',2,'second concurrent text',NULL,'todo',NULL,1,1);",
    )
    .unwrap();
    drop(conn);

    let requests = [("t_one", 201_i64, 2_i64), ("t_two", 202_i64, 3_i64)]
        .into_iter()
        .map(|(task_id, id, cursor)| {
            let mut item = delivery(&evidence, id, cursor, ProjectionDeliveryAction::Upsert);
            item.entity_uri = format!("kb://task/{task_id}");
            let request = apply_request(&evidence, item);
            authorize_apply(&db, &request);
            request
        })
        .collect::<Vec<_>>();
    let start = Arc::new(Barrier::new(3));
    let handles = requests
        .into_iter()
        .map(|request| {
            let backend = Arc::clone(&backend);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request))
            })
        })
        .collect::<Vec<_>>();
    start.wait();
    for response in handles.into_iter().map(|handle| handle.join().unwrap()) {
        assert!(matches!(
            response,
            VectorProjectionHelperResponse::ApplyBatch(_)
        ));
    }

    assert_eq!(chunk_hits(&db, &evidence, "b_one").len(), 2);
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let delivery_state: serde_json::Value =
        serde_json::from_slice(&fs::read(generation.join("delivery-state.json")).unwrap()).unwrap();
    assert_eq!(
        delivery_state["applied"]
            .as_object()
            .map(|items| items.len()),
        Some(2)
    );
    let cache: serde_json::Value =
        serde_json::from_slice(&fs::read(generation.join("embedding-cache.json")).unwrap())
            .unwrap();
    assert_eq!(
        cache["entries"].as_object().map(|items| items.len()),
        Some(2)
    );
}

#[test]
fn queued_prepare_cannot_recreate_a_generation_after_sqlite_clears_building_authority() {
    let (_temp, db) = authority_database();
    let backend = Arc::new(
        VectorProjectionBackend::new(&db, Arc::new(StaticProvider)).expect("configured backend"),
    );
    let request = prepare_request(
        &backend,
        LANCEDB_CHUNKS_STORE,
        "gen_queued_stale",
        1,
        Vec::new(),
    );
    authorize_snapshotting(&db, &request);
    let lock_name = format!("{LANCEDB_CHUNKS_STORE}-projection-helper");
    let parent_guard =
        DerivedStoreWriteGuard::acquire(&db, &lock_name).expect("parent helper guard");
    let entered = Arc::new(Barrier::new(2));
    let worker = {
        let backend = Arc::clone(&backend);
        let entered = Arc::clone(&entered);
        std::thread::spawn(move || {
            entered.wait();
            backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request))
        })
    };
    entered.wait();

    clear_building_authority(&db, LANCEDB_CHUNKS_STORE);
    drop(parent_guard);

    match worker.join().expect("queued helper thread") {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("stale queued prepare was accepted: {response:?}"),
    }
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join("gen_queued_stale")
            .exists(),
        "a request that became stale while waiting for the helper lock must not recreate its generation"
    );
}

#[test]
fn queued_prepare_cannot_mutate_after_same_owner_fence_bump() {
    let (_temp, db) = authority_database();
    let backend = Arc::new(
        VectorProjectionBackend::new(&db, Arc::new(StaticProvider)).expect("configured backend"),
    );
    let request = prepare_request(
        &backend,
        LANCEDB_CHUNKS_STORE,
        "gen_queued_fence_prepare",
        1,
        Vec::new(),
    );
    authorize_snapshotting(&db, &request);
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&request.context.generation_id);
    assert!(!generation.exists());

    let lock_name = format!("{LANCEDB_CHUNKS_STORE}-projection-helper");
    let parent_guard =
        DerivedStoreWriteGuard::acquire(&db, &lock_name).expect("parent helper guard");
    let entered = Arc::new(Barrier::new(2));
    let worker = {
        let backend = Arc::clone(&backend);
        let entered = Arc::clone(&entered);
        std::thread::spawn(move || {
            entered.wait();
            backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request))
        })
    };
    entered.wait();

    bump_same_owner_fence(&db, LANCEDB_CHUNKS_STORE);
    drop(parent_guard);

    match worker.join().expect("queued helper thread") {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("same-owner stale queued prepare was accepted: {response:?}"),
    }
    assert!(
        !generation.exists(),
        "a queued prepare must not create data after its fence is bumped"
    );
}

#[test]
fn queued_quarantine_cannot_cross_a_sqlite_lease_rebind() {
    let (_temp, db, backend) = backend();
    let backend = Arc::new(backend);
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_queued_rebind", 1);
    let _ = publish(&backend, &db, None, &evidence);
    let request = VectorProjectionHelperRequest::Quarantine(
        kanban_contract::VectorProjectionGenerationMutationRequest {
            context: context(&evidence, "req_queued_rebind"),
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Active,
                None,
            ),
        },
    );
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    assert!(generation.join("projection-evidence.json").is_file());
    let helper_guard =
        DerivedStoreWriteGuard::acquire(&db, &format!("{LANCEDB_CHUNKS_STORE}-projection-helper"))
            .expect("parent helper guard");
    let entered = Arc::new(Barrier::new(2));
    let worker = {
        let backend = Arc::clone(&backend);
        let entered = Arc::clone(&entered);
        std::thread::spawn(move || {
            entered.wait();
            backend.execute(&request)
        })
    };
    entered.wait();

    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "UPDATE projection_store_state
             SET lease_owner='rebound-owner',lease_token='rebound-token',
                 fence_epoch=fence_epoch+1
             WHERE store_name=?1",
            [LANCEDB_CHUNKS_STORE],
        )
        .unwrap();
    drop(helper_guard);

    match worker.join().expect("queued helper thread") {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("stale queued quarantine was accepted: {response:?}"),
    }
    assert!(generation.join("projection-evidence.json").is_file());
    assert!(generation.is_dir());
}

#[test]
fn current_snapshotting_authority_still_prepares_the_same_generation() {
    let (_temp, db) = authority_database();
    let backend =
        VectorProjectionBackend::new(&db, Arc::new(StaticProvider)).expect("configured backend");
    let request = prepare_request(
        &backend,
        LANCEDB_CHUNKS_STORE,
        "gen_current_building",
        1,
        Vec::new(),
    );
    authorize_snapshotting(&db, &request);

    match backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request)) {
        VectorProjectionHelperResponse::PrepareSnapshot(response) => {
            assert_eq!(
                response.evidence.manifest.generation,
                "gen_current_building"
            );
        }
        response => panic!("current same-building prepare was rejected: {response:?}"),
    }
}

#[test]
fn publish_cannot_recreate_a_marker_after_sqlite_clears_building_authority() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_stale_publish", 1);
    clear_building_authority(&db, LANCEDB_CHUNKS_STORE);

    let response = backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
        VectorProjectionPublishRequest {
            context: context(&evidence, "req_stale_publish"),
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected_active: None,
            prepared: evidence.clone(),
        },
    )));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        !generations(&db, LANCEDB_CHUNKS_STORE)
            .join(&evidence.manifest.generation)
            .join("published")
            .exists(),
        "a publish that lost SQLite building authority must not recreate its marker"
    );
}

#[test]
fn queued_apply_cannot_mutate_after_same_owner_fence_bump() {
    let (_temp, db, backend) = backend();
    let backend = Arc::new(backend);
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_queued_fence_apply",
        1,
    );
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute(
            "INSERT INTO tasks(
                 id,board_id,seq,title,description,status,archived_at,created_at,updated_at
             ) VALUES ('t_queued_fence','b_one',1,'queued fence apply',NULL,'todo',NULL,1,1)",
            [],
        )
        .unwrap();
    let request = apply_request(
        &evidence,
        delivery(&evidence, 802, 2, ProjectionDeliveryAction::Upsert),
    );
    authorize_apply(&db, &request);
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let before = filesystem_digest(&generation);

    let lock_name = format!("{LANCEDB_CHUNKS_STORE}-projection-helper");
    let parent_guard =
        DerivedStoreWriteGuard::acquire(&db, &lock_name).expect("parent helper guard");
    let entered = Arc::new(Barrier::new(2));
    let worker = {
        let backend = Arc::clone(&backend);
        let entered = Arc::clone(&entered);
        std::thread::spawn(move || {
            entered.wait();
            backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request))
        })
    };
    entered.wait();

    bump_same_owner_fence(&db, LANCEDB_CHUNKS_STORE);
    drop(parent_guard);

    match worker.join().expect("queued helper thread") {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("same-owner stale queued apply was accepted: {response:?}"),
    }
    assert_eq!(
        filesystem_digest(&generation),
        before,
        "a queued apply must not mutate LanceDB or sidecars after its fence is bumped"
    );
}

#[test]
fn lease_rollover_reclaims_pending_apply_for_active_generation_with_older_manifest_fence() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_active_lease_rollover",
        1,
    );
    publish(&backend, &db, None, &evidence);
    insert_task_for_apply(&db, "active lease rollover");
    let mut request = apply_request(
        &evidence,
        delivery(&evidence, 803, 2, ProjectionDeliveryAction::Upsert),
    );
    authorize_apply(&db, &request);
    let successor_fence = release_and_reacquire_projection_lease(
        &db,
        LANCEDB_CHUNKS_STORE,
        "successor-owner",
        "successor-lease-capability",
    );
    bind_apply_request_to_lease(
        &mut request,
        "successor-owner",
        "successor-lease-capability",
        successor_fence,
        VectorProjectionGenerationRole::Active,
        None,
    );
    authorize_apply(&db, &request);
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let before = filesystem_digest(&generation);

    let response =
        match backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request.clone())) {
            VectorProjectionHelperResponse::ApplyBatch(response) => response,
            response => panic!("successor active apply was rejected: {response:?}"),
        };

    assert_eq!(successor_fence, evidence.manifest.fence_epoch + 1);
    assert_eq!(response.ack.request_id, request.context.request_id);
    assert_eq!(response.receipt.fence_epoch, successor_fence);
    assert_eq!(response.receipt.applied_item_count, 1);
    assert_ne!(filesystem_digest(&generation), before);
    acknowledge_apply(&db, &request);
}

#[test]
fn lease_rollover_resumes_prepared_building_apply_with_older_generation_fence() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_building_lease_rollover",
        1,
    );
    insert_task_for_apply(&db, "building lease rollover");
    let mut request = apply_request(
        &evidence,
        delivery(&evidence, 804, 2, ProjectionDeliveryAction::Upsert),
    );
    authorize_apply(&db, &request);
    let successor_fence = release_and_reacquire_projection_lease(
        &db,
        LANCEDB_CHUNKS_STORE,
        "successor-owner",
        "successor-lease-capability",
    );
    bind_apply_request_to_lease(
        &mut request,
        "successor-owner",
        "successor-lease-capability",
        successor_fence,
        VectorProjectionGenerationRole::Building,
        Some(VectorProjectionBuildingPhase::Prepared),
    );
    authorize_apply(&db, &request);
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let before = filesystem_digest(&generation);

    let response =
        match backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request.clone())) {
            VectorProjectionHelperResponse::ApplyBatch(response) => response,
            response => panic!("successor prepared-building apply was rejected: {response:?}"),
        };

    assert_eq!(successor_fence, evidence.manifest.fence_epoch + 1);
    assert_eq!(response.ack.request_id, request.context.request_id);
    assert_eq!(response.receipt.fence_epoch, successor_fence);
    assert_eq!(response.receipt.applied_item_count, 1);
    assert_ne!(filesystem_digest(&generation), before);
    acknowledge_apply(&db, &request);

    let mut authority = destructive_authority(
        &evidence,
        successor_fence,
        VectorProjectionGenerationRole::Building,
        Some(VectorProjectionBuildingPhase::Prepared),
    );
    bind_destructive_authority_to_lease(
        &mut authority,
        "successor-owner",
        "successor-lease-capability",
        successor_fence,
    );
    let publish_request = VectorProjectionPublishRequest {
        context: context(&evidence, "req_publish_building_lease_rollover"),
        authority,
        expected_active: None,
        prepared: evidence.clone(),
    };
    let receipt = match backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
        publish_request,
    ))) {
        VectorProjectionHelperResponse::Publish(response) => response.receipt,
        response => panic!("successor prepared-building publish was rejected: {response:?}"),
    };
    assert_eq!(receipt.active, evidence);
    mark_published_authority(&db, &receipt.active);
    assert_active_ready_after_rollover(&db, &receipt.active, successor_fence);
}

#[test]
fn lease_rollover_reconciles_prepared_building_publication_with_older_generation_fence() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_repair_lease_rollover",
        1,
    );
    let successor_fence = release_and_reacquire_projection_lease(
        &db,
        LANCEDB_CHUNKS_STORE,
        "successor-owner",
        "successor-lease-capability",
    );
    let mut authority = destructive_authority(
        &evidence,
        successor_fence,
        VectorProjectionGenerationRole::Building,
        Some(VectorProjectionBuildingPhase::Prepared),
    );
    bind_destructive_authority_to_lease(
        &mut authority,
        "successor-owner",
        "successor-lease-capability",
        successor_fence,
    );
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let before = filesystem_digest(&generation);
    let request = VectorProjectionRepairPublicationRequest {
        context: context(&evidence, "req_repair_building_lease_rollover"),
        authority,
        expected: evidence.clone(),
    };

    match backend.execute(&VectorProjectionHelperRequest::RepairPublication(request)) {
        VectorProjectionHelperResponse::RepairPublication(response) => {
            assert_eq!(response.generation_id, evidence.manifest.generation);
        }
        response => panic!("successor prepared-building repair was rejected: {response:?}"),
    }
    assert_ne!(filesystem_digest(&generation), before);
    mark_published_authority(&db, &evidence);
    assert_active_ready_after_rollover(&db, &evidence, successor_fence);
}

#[test]
fn lease_rollover_publish_and_repair_reject_stale_forged_and_future_authority_without_mutation() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_fenced_publish_repair_rollover",
        1,
    );
    let stale_authority = destructive_authority(
        &evidence,
        evidence.manifest.fence_epoch,
        VectorProjectionGenerationRole::Building,
        Some(VectorProjectionBuildingPhase::Prepared),
    );
    let stale_publish = VectorProjectionPublishRequest {
        context: context(&evidence, "req_stale_publish_rollover"),
        authority: stale_authority.clone(),
        expected_active: None,
        prepared: evidence.clone(),
    };
    let stale_repair = VectorProjectionRepairPublicationRequest {
        context: context(&evidence, "req_stale_repair_rollover"),
        authority: stale_authority,
        expected: evidence.clone(),
    };
    let successor_fence = release_and_reacquire_projection_lease(
        &db,
        LANCEDB_CHUNKS_STORE,
        "successor-owner",
        "successor-lease-capability",
    );
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let before = filesystem_digest(&generation);

    assert_projection_delivery_error(
        backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
            stale_publish,
        ))),
        "stale publish crossed lease rollover",
    );
    assert_eq!(filesystem_digest(&generation), before);
    assert_projection_delivery_error(
        backend.execute(&VectorProjectionHelperRequest::RepairPublication(
            stale_repair,
        )),
        "stale repair crossed lease rollover",
    );
    assert_eq!(filesystem_digest(&generation), before);

    let mut forged_authority = destructive_authority(
        &evidence,
        successor_fence,
        VectorProjectionGenerationRole::Building,
        Some(VectorProjectionBuildingPhase::Prepared),
    );
    bind_destructive_authority_to_lease(
        &mut forged_authority,
        "successor-owner",
        "successor-lease-capability",
        successor_fence,
    );
    forged_authority
        .expected_binding
        .as_mut()
        .expect("exact generation binding")
        .fence_epoch += 1;
    assert_projection_delivery_error(
        backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
            VectorProjectionPublishRequest {
                context: context(&evidence, "req_forged_publish_rollover"),
                authority: forged_authority.clone(),
                expected_active: None,
                prepared: evidence.clone(),
            },
        ))),
        "forged publish binding crossed lease rollover",
    );
    assert_eq!(filesystem_digest(&generation), before);
    assert_projection_delivery_error(
        backend.execute(&VectorProjectionHelperRequest::RepairPublication(
            VectorProjectionRepairPublicationRequest {
                context: context(&evidence, "req_forged_repair_rollover"),
                authority: forged_authority,
                expected: evidence.clone(),
            },
        )),
        "forged repair binding crossed lease rollover",
    );
    assert_eq!(filesystem_digest(&generation), before);

    let (_future_temp, future_db, future_backend) = crate::backend();
    let future_evidence = prepare(
        &future_backend,
        &future_db,
        LANCEDB_CHUNKS_STORE,
        "gen_future_fence_publish_repair",
        3,
    );
    let changed = rusqlite::Connection::open(&future_db)
        .unwrap()
        .execute(
            "UPDATE projection_store_state
             SET fence_epoch=2
             WHERE store_name=?1 AND fence_epoch=3
               AND lease_owner='fixture-owner'
               AND lease_token='fixture-lease-capability'",
            [LANCEDB_CHUNKS_STORE],
        )
        .unwrap();
    assert_eq!(changed, 1);
    let future_authority = destructive_authority(
        &future_evidence,
        2,
        VectorProjectionGenerationRole::Building,
        Some(VectorProjectionBuildingPhase::Prepared),
    );
    let future_generation =
        generations(&future_db, LANCEDB_CHUNKS_STORE).join(&future_evidence.manifest.generation);
    let future_before = filesystem_digest(&future_generation);
    assert_projection_delivery_error(
        future_backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(
            VectorProjectionPublishRequest {
                context: context(&future_evidence, "req_future_publish_rollover"),
                authority: future_authority.clone(),
                expected_active: None,
                prepared: future_evidence.clone(),
            },
        ))),
        "future generation fence was accepted for publish",
    );
    assert_eq!(filesystem_digest(&future_generation), future_before);
    assert_projection_delivery_error(
        future_backend.execute(&VectorProjectionHelperRequest::RepairPublication(
            VectorProjectionRepairPublicationRequest {
                context: context(&future_evidence, "req_future_repair_rollover"),
                authority: future_authority,
                expected: future_evidence,
            },
        )),
        "future generation fence was accepted for repair",
    );
    assert_eq!(filesystem_digest(&future_generation), future_before);
}

#[test]
fn lease_rollover_rejects_stale_owner_and_forged_binding_without_mutation() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_fenced_lease_rollover",
        1,
    );
    publish(&backend, &db, None, &evidence);
    insert_task_for_apply(&db, "fenced lease rollover");
    let stale = apply_request(
        &evidence,
        delivery(&evidence, 805, 2, ProjectionDeliveryAction::Upsert),
    );
    authorize_apply(&db, &stale);
    let successor_fence = release_and_reacquire_projection_lease(
        &db,
        LANCEDB_CHUNKS_STORE,
        "successor-owner",
        "successor-lease-capability",
    );
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let before = filesystem_digest(&generation);

    match backend.execute(&VectorProjectionHelperRequest::ApplyBatch(stale)) {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("stale owner crossed lease rollover: {response:?}"),
    }
    assert_eq!(filesystem_digest(&generation), before);

    let mut forged = apply_request(
        &evidence,
        delivery(&evidence, 806, 2, ProjectionDeliveryAction::Upsert),
    );
    bind_apply_request_to_lease(
        &mut forged,
        "successor-owner",
        "successor-lease-capability",
        successor_fence,
        VectorProjectionGenerationRole::Active,
        None,
    );
    forged
        .authority
        .expected_binding
        .as_mut()
        .expect("exact generation binding")
        .fence_epoch += 1;
    authorize_apply(&db, &forged);
    match backend.execute(&VectorProjectionHelperRequest::ApplyBatch(forged)) {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("forged generation binding crossed lease rollover: {response:?}"),
    }
    assert_eq!(filesystem_digest(&generation), before);
}

#[test]
fn apply_rejects_a_cleared_sqlite_claim_without_mutating_the_generation() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_stale_apply", 1);
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_one','b_one',1,'stale apply',NULL,'todo',NULL,1,1)",
        [],
    )
    .unwrap();
    drop(conn);
    let request = apply_request(
        &evidence,
        delivery(&evidence, 801, 2, ProjectionDeliveryAction::Upsert),
    );
    authorize_apply(&db, &request);
    let before = filesystem_digest(
        &generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation),
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE projection_deliveries
         SET status='pending',claim_owner=NULL,claim_token=NULL,
             claim_lease_token=NULL,claim_fence_epoch=NULL,
             claim_generation=NULL,claim_expires_at=NULL
         WHERE id=801",
        [],
    )
    .unwrap();
    drop(conn);

    let response = backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert_eq!(
        filesystem_digest(
            &generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation)
        ),
        before,
        "a stale claim must be rejected before LanceDB or helper sidecars change"
    );
}

#[test]
fn repair_cannot_recreate_a_marker_after_sqlite_clears_active_authority() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, "gen_stale_repair", 1);
    publish(&backend, &db, None, &evidence);
    let marker = generations(&db, LANCEDB_CHUNKS_STORE)
        .join(&evidence.manifest.generation)
        .join("published");
    fs::remove_file(&marker).unwrap();
    clear_active_authority(&db, LANCEDB_CHUNKS_STORE);

    let response = backend.execute(&VectorProjectionHelperRequest::RepairPublication(
        VectorProjectionRepairPublicationRequest {
            context: context(&evidence, "req_stale_repair"),
            authority: destructive_authority(
                &evidence,
                evidence.manifest.fence_epoch,
                VectorProjectionGenerationRole::Building,
                Some(VectorProjectionBuildingPhase::Prepared),
            ),
            expected: evidence,
        },
    ));
    assert!(matches!(response, VectorProjectionHelperResponse::Error(_)));
    assert!(
        !marker.exists(),
        "repair must not recreate a marker after SQLite clears active authority"
    );
}

#[test]
fn queued_publish_cannot_mutate_after_same_owner_fence_bump() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_queued_fence_publish",
        1,
    );
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let before = filesystem_digest(&generation);
    let request = VectorProjectionPublishRequest {
        context: context(&evidence, "req_queued_fence_publish"),
        authority: destructive_authority(
            &evidence,
            evidence.manifest.fence_epoch,
            VectorProjectionGenerationRole::Building,
            Some(VectorProjectionBuildingPhase::Prepared),
        ),
        expected_active: None,
        prepared: evidence.clone(),
    };

    let lock_name = format!("{LANCEDB_CHUNKS_STORE}-projection-helper");
    let parent_guard =
        DerivedStoreWriteGuard::acquire(&db, &lock_name).expect("parent helper guard");
    let entered = Arc::new(Barrier::new(2));
    let worker = {
        let backend = Arc::new(backend);
        let entered = Arc::clone(&entered);
        std::thread::spawn(move || {
            entered.wait();
            backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(request)))
        })
    };
    entered.wait();

    bump_same_owner_fence(&db, LANCEDB_CHUNKS_STORE);
    drop(parent_guard);

    match worker.join().expect("queued helper thread") {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("same-owner stale queued publish was accepted: {response:?}"),
    }
    assert_eq!(
        filesystem_digest(&generation),
        before,
        "a queued publish must not mutate the prepared generation after its fence is bumped"
    );
    assert!(
        !generation.join("published").exists(),
        "a queued publish must not recreate its marker after its fence is bumped"
    );
}

#[test]
fn queued_repair_cannot_mutate_after_same_owner_fence_bump() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_queued_fence_repair",
        1,
    );
    publish(&backend, &db, None, &evidence);
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let marker = generation.join("published");
    fs::remove_file(&marker).unwrap();
    let before = filesystem_digest(&generation);
    let request = VectorProjectionRepairPublicationRequest {
        context: context(&evidence, "req_queued_fence_repair"),
        authority: destructive_authority(
            &evidence,
            evidence.manifest.fence_epoch,
            VectorProjectionGenerationRole::Active,
            None,
        ),
        expected: evidence,
    };

    let lock_name = format!("{LANCEDB_CHUNKS_STORE}-projection-helper");
    let parent_guard =
        DerivedStoreWriteGuard::acquire(&db, &lock_name).expect("parent helper guard");
    let entered = Arc::new(Barrier::new(2));
    let worker = {
        let backend = Arc::new(backend);
        let entered = Arc::clone(&entered);
        std::thread::spawn(move || {
            entered.wait();
            backend.execute(&VectorProjectionHelperRequest::RepairPublication(request))
        })
    };
    entered.wait();

    bump_same_owner_fence(&db, LANCEDB_CHUNKS_STORE);
    drop(parent_guard);

    match worker.join().expect("queued helper thread") {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("same-owner stale queued repair was accepted: {response:?}"),
    }
    assert_eq!(
        filesystem_digest(&generation),
        before,
        "a queued repair must not mutate sidecars after its fence is bumped"
    );
    assert!(
        !marker.exists(),
        "a queued repair must not recreate its marker after its fence is bumped"
    );
}

#[test]
fn heartbeat_renewal_with_same_owner_token_and_fence_is_accepted() {
    let (_temp, db) = authority_database();
    let backend =
        VectorProjectionBackend::new(&db, Arc::new(StaticProvider)).expect("configured backend");
    let request = prepare_request(
        &backend,
        LANCEDB_CHUNKS_STORE,
        "gen_heartbeat_renewal",
        1,
        Vec::new(),
    );
    authorize_snapshotting(&db, &request);
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE projection_store_state
         SET lease_expires_at=?1
         WHERE store_name=?2",
        rusqlite::params![4_000_000_000_000_i64, LANCEDB_CHUNKS_STORE],
    )
    .unwrap();
    conn.execute(
        "UPDATE projection_store_state
         SET lease_expires_at=?1
         WHERE store_name=?2",
        rusqlite::params![5_000_000_000_000_i64, LANCEDB_CHUNKS_STORE],
    )
    .unwrap();
    drop(conn);

    match backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request)) {
        VectorProjectionHelperResponse::PrepareSnapshot(response) => {
            assert_eq!(
                response.evidence.manifest.generation,
                "gen_heartbeat_renewal"
            );
        }
        response => panic!("heartbeat-renewed authority was rejected: {response:?}"),
    }
}

#[test]
fn physical_row_fingerprint_is_bound_to_evidence_and_detects_row_set_drift() {
    let (_temp, db) = database();
    let provider = Arc::new(CountingProvider::default());
    let backend = VectorProjectionBackend::new(&db, provider.clone()).expect("configured backend");
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_content_fingerprint",
        1,
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_one','b_one',1,'fingerprinted task',NULL,'todo',NULL,1,1)",
        [],
    )
    .unwrap();
    drop(conn);
    apply(
        &backend,
        &db,
        &evidence,
        delivery(&evidence, 20, 2, ProjectionDeliveryAction::Upsert),
    );
    publish(&backend, &db, None, &evidence);

    let metadata: serde_json::Value = serde_json::from_slice(
        &fs::read(
            generations(&db, LANCEDB_CHUNKS_STORE)
                .join(&evidence.manifest.generation)
                .join("projection-content.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        metadata["evidence_fingerprint"].as_str(),
        Some(evidence.fingerprint.as_str())
    );
    assert_eq!(metadata["row_count"].as_u64(), Some(1));
    assert!(
        metadata["content_fingerprint"]
            .as_str()
            .is_some_and(|value| value.starts_with("fnv64:"))
    );
    assert!(validate_active(&backend, &evidence));

    generation_store(&db, &evidence)
        .delete_entities(&["kb://task/t_one".to_owned()])
        .unwrap();
    assert!(
        !validate_active(&backend, &evidence),
        "out-of-band physical row loss must invalidate the evidence-bound content fingerprint"
    );
}

#[test]
fn historical_validation_of_a_missing_table_is_strictly_read_only() {
    let (_temp, db, backend) = backend();
    let evidence = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_historical_read_only",
        1,
    );
    publish(&backend, &db, None, &evidence);
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
    let table = generation.join("lance").join("kb_chunks.lance");
    fs::remove_dir_all(&table).unwrap();
    fs::create_dir(&table).unwrap();
    let before = filesystem_digest(&generation);

    assert!(!validate_generation(&backend, &evidence));
    assert_eq!(
        filesystem_digest(&generation),
        before,
        "historical inspection must never recreate or mutate a missing Lance table"
    );
}

#[test]
fn current_generation_apply_validation_never_repairs_missing_or_corrupt_artifacts() {
    let (_temp, db, backend) = backend();
    for (generation_id, damage) in [
        ("gen_resume_missing_cache", "missing-cache"),
        ("gen_resume_missing_table", "missing-table"),
        ("gen_resume_corrupt_table", "corrupt-table"),
    ] {
        let evidence = prepare(&backend, &db, LANCEDB_CHUNKS_STORE, generation_id, 1);
        let apply_request = apply_request(
            &evidence,
            delivery(&evidence, 800, 2, ProjectionDeliveryAction::Upsert),
        );
        authorize_apply(&db, &apply_request);
        let generation = generations(&db, LANCEDB_CHUNKS_STORE).join(&evidence.manifest.generation);
        let table = generation.join("lance").join("kb_chunks.lance");
        match damage {
            "missing-cache" => fs::remove_file(generation.join("embedding-cache.json")).unwrap(),
            "missing-table" => fs::remove_dir_all(&table).unwrap(),
            "corrupt-table" => {
                fs::remove_dir_all(&table).unwrap();
                fs::create_dir(&table).unwrap();
            }
            _ => unreachable!(),
        }
        let before = filesystem_digest(&generation);

        let response = backend.execute(&VectorProjectionHelperRequest::ApplyBatch(
            apply_request.clone(),
        ));
        assert!(
            matches!(response, VectorProjectionHelperResponse::Error(_)),
            "apply validation accepted {damage}: {response:?}"
        );
        assert_eq!(
            filesystem_digest(&generation),
            before,
            "apply validation mutated {damage}"
        );

        fs::remove_dir_all(&generation).unwrap();
    }
}

#[test]
fn label_board_rebuild_is_physically_isolated_from_task_chunks() {
    let (_temp, db) = database();
    let provider = Arc::new(CountingProvider::default());
    let backend = VectorProjectionBackend::new(&db, provider.clone()).expect("configured backend");
    let chunks = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_chunks_isolated",
        1,
    );
    let labels = prepare(
        &backend,
        &db,
        LANCEDB_LABEL_ATOMS_STORE,
        "gen_labels_isolated",
        1,
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "INSERT INTO labels(id,board_id,name) VALUES ('l_one','b_one','urgent')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO label_atoms(
             id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at
         ) VALUES (
             'la_one','l_one','b_one','positive','name','urgent',0,'atom-hash',1,1
         )",
        [],
    )
    .unwrap();
    drop(conn);

    let mut rebuild = delivery(&labels, 10, 2, ProjectionDeliveryAction::Rebuild);
    rebuild.entity_uri = "kb://board/b_one".to_owned();
    apply(&backend, &db, &labels, rebuild);

    assert_eq!(label_hits(&db, &labels, "b_one").len(), 1);
    assert!(
        chunk_hits(&db, &chunks, "b_one").is_empty(),
        "the independent label corpus must not write the task chunk generation"
    );
}

#[test]
fn null_source_board_rebuild_rehydrates_canonical_tasks_and_preserves_other_boards() {
    let (_temp, db, backend) = backend();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES
             ('t_keep','b_one',1,'old canonical',NULL,'todo',NULL,1,1),
             ('t_stale','b_one',2,'stale content',NULL,'todo',NULL,1,1),
             ('t_other','b_two',1,'other board',NULL,'todo',NULL,1,1);",
    )
    .unwrap();
    drop(conn);

    let chunks = prepare_with_records(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_null_source_board_rebuild",
        1,
        vec![
            task_record("b_one", "t_keep", "old canonical"),
            task_record("b_one", "t_stale", "stale content"),
            task_record("b_two", "t_other", "other board"),
        ],
    );
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_one"),
        vec!["kb://task/t_keep", "kb://task/t_stale"]
    );
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_two"),
        vec!["kb://task/t_other"]
    );

    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "UPDATE tasks SET title='new canonical',updated_at=2 WHERE id='t_keep';
         DELETE FROM tasks WHERE id='t_stale';
         INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_new','b_one',2,'new task',NULL,'todo',NULL,2,2);",
    )
    .unwrap();
    drop(conn);

    let mut rebuild = delivery(&chunks, 304, 2, ProjectionDeliveryAction::Rebuild);
    rebuild.entity_uri = "kb://board/b_one".to_owned();
    assert!(rebuild.source_event_id.is_none());
    apply(&backend, &db, &chunks, rebuild);

    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_one"),
        vec!["kb://task/t_keep", "kb://task/t_new"]
    );
    let rebuilt = chunk_fingerprints(&db, &chunks, "b_one");
    assert_eq!(rebuilt.len(), 2);
    let mut rebuilt_texts = rebuilt
        .iter()
        .map(|(_, _, text)| text.clone())
        .collect::<Vec<_>>();
    rebuilt_texts.sort();
    assert_eq!(
        rebuilt_texts,
        vec![
            Some("new canonical".to_owned()),
            Some("new task".to_owned())
        ]
    );
    assert!(
        !rebuilt
            .iter()
            .any(|(_, _, text)| text.as_deref() == Some("old canonical"))
    );
    assert!(
        !rebuilt
            .iter()
            .any(|(_, _, text)| text.as_deref() == Some("stale content"))
    );
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_two"),
        vec!["kb://task/t_other"]
    );
}

#[test]
fn deliveries_require_exact_board_scoped_shapes_and_canonical_task_membership() {
    let (_temp, db, backend) = backend();
    let chunks = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_delivery_shapes",
        1,
    );
    let labels = prepare(
        &backend,
        &db,
        LANCEDB_LABEL_ATOMS_STORE,
        "gen_label_delivery_shapes",
        1,
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_one','b_one',1,'board-bound',NULL,'todo',NULL,1,1)",
        [],
    )
    .unwrap();
    drop(conn);

    let mut malformed_rebuild = delivery(&chunks, 301, 2, ProjectionDeliveryAction::Rebuild);
    malformed_rebuild.entity_uri = "kb://task/t_one".to_owned();
    let mut cross_board = delivery(&chunks, 302, 3, ProjectionDeliveryAction::Upsert);
    cross_board.board_id = "b_two".to_owned();
    let malformed_label = delivery(&labels, 303, 2, ProjectionDeliveryAction::Upsert);
    for (evidence, item) in [
        (&chunks, malformed_rebuild),
        (&chunks, cross_board),
        (&labels, malformed_label),
    ] {
        let request = apply_request(evidence, item);
        authorize_apply(&db, &request);
        let response = backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request));
        match response {
            VectorProjectionHelperResponse::Error(error) => {
                assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            }
            response => panic!("malformed delivery was accepted: {response:?}"),
        }
    }
}

#[test]
fn run_only_deliveries_require_exact_run_identity_and_restore_parent_task_chunks() {
    let (_temp, db, backend) = backend();
    let chunks = prepare(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_run_delivery_identity",
        1,
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_one','b_one',1,'run parent',NULL,'todo',NULL,1,1);
         INSERT INTO task_runs(id,board_id,task_id,summary,error,started_at)
         VALUES
           ('r_one','b_one','t_one',NULL,NULL,2),
           ('r_other','b_one','t_one',NULL,NULL,3);
         INSERT INTO task_events(id,board_id,task_id,run_id,kind,payload_json)
         VALUES
           (501,'b_one',NULL,'r_one','run.updated','{}'),
           (502,'b_one',NULL,'r_one','run.finished','{}');",
    )
    .unwrap();
    drop(conn);

    let mut run_upsert = delivery(&chunks, 501, 2, ProjectionDeliveryAction::Upsert);
    run_upsert.entity_uri = "kb://run/r_one".to_owned();
    run_upsert.source_event_id = Some(501);
    apply(&backend, &db, &chunks, run_upsert);
    let initial_fingerprints = chunk_fingerprints(&db, &chunks, "b_one");
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_one"),
        vec!["kb://task/t_one"],
        "a run-only source event maps its canonical parent task into the chunk corpus"
    );

    let mut forged_parent = delivery(&chunks, 502, 3, ProjectionDeliveryAction::Upsert);
    forged_parent.entity_uri = "kb://task/t_one".to_owned();
    forged_parent.source_event_id = Some(501);
    assert_apply_delivery_error(&backend, &db, &chunks, forged_parent);
    assert_eq!(
        chunk_fingerprints(&db, &chunks, "b_one"),
        initial_fingerprints,
        "a run-only event cannot mutate through its parent task URI"
    );

    let mut wrong_run = delivery(&chunks, 503, 4, ProjectionDeliveryAction::Upsert);
    wrong_run.entity_uri = "kb://run/r_other".to_owned();
    wrong_run.source_event_id = Some(501);
    assert_apply_delivery_error(&backend, &db, &chunks, wrong_run);
    assert_eq!(
        chunk_fingerprints(&db, &chunks, "b_one"),
        initial_fingerprints,
        "a run event cannot mutate through a different run URI"
    );

    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE tasks SET status='archived',archived_at=3,updated_at=3 WHERE id='t_one'",
        [],
    )
    .unwrap();
    drop(conn);
    let mut run_delete = delivery(&chunks, 504, 5, ProjectionDeliveryAction::Delete);
    run_delete.entity_uri = "kb://run/r_one".to_owned();
    run_delete.source_event_id = Some(502);
    apply(&backend, &db, &chunks, run_delete);
    assert!(chunk_entity_uris(&db, &chunks, "b_one").is_empty());

    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE tasks SET status='todo',archived_at=NULL,updated_at=4 WHERE id='t_one'",
        [],
    )
    .unwrap();
    drop(conn);
    let mut run_restore = delivery(&chunks, 505, 6, ProjectionDeliveryAction::Upsert);
    run_restore.entity_uri = "kb://run/r_one".to_owned();
    run_restore.source_event_id = Some(502);
    apply(&backend, &db, &chunks, run_restore);
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_one"),
        vec!["kb://task/t_one"],
        "a valid run delivery still supports archive/delete/restore convergence"
    );
}

#[test]
fn taskless_board_upsert_returns_bound_ack_without_mutating_rows_or_blocking_later_task() {
    let (_temp, db, backend) = backend();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_existing','b_one',1,'existing task',NULL,'todo',NULL,1,1);
         INSERT INTO task_events(id,board_id,task_id,run_id,kind,payload_json)
         VALUES (101,'b_one',NULL,NULL,'board.updated','{}');",
    )
    .unwrap();
    drop(conn);
    let chunks = prepare_with_records(
        &backend,
        &db,
        LANCEDB_CHUNKS_STORE,
        "gen_taskless_board_upsert",
        1,
        vec![task_record("b_one", "t_existing", "existing task")],
    );
    let before_noop = chunk_fingerprints(&db, &chunks, "b_one");
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_one"),
        vec!["kb://task/t_existing"]
    );

    let mut board_upsert = delivery(&chunks, 401, 2, ProjectionDeliveryAction::Upsert);
    board_upsert.entity_uri = "kb://board/b_one".to_owned();
    board_upsert.source_event_id = Some(101);
    apply_and_assert_bound_ack(&backend, &db, &chunks, board_upsert);
    assert_eq!(
        chunk_fingerprints(&db, &chunks, "b_one"),
        before_noop,
        "a taskless board upsert must not change Lance task rows"
    );

    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_later','b_one',2,'later mutation',NULL,'todo',NULL,2,2);
         INSERT INTO task_events(id,board_id,task_id,run_id,kind,payload_json)
         VALUES (102,'b_one','t_later',NULL,'task.updated','{}');",
    )
    .unwrap();
    drop(conn);
    let mut task_upsert = delivery(&chunks, 402, 3, ProjectionDeliveryAction::Upsert);
    task_upsert.entity_uri = "kb://task/t_later".to_owned();
    task_upsert.source_event_id = Some(102);
    apply_and_assert_bound_ack(&backend, &db, &chunks, task_upsert);
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_one"),
        vec!["kb://task/t_existing", "kb://task/t_later"]
    );

    generation_store(&db, &chunks)
        .delete_entities(&["kb://task/t_later".to_owned()])
        .unwrap();
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_one"),
        vec!["kb://task/t_existing"]
    );
    let mut board_rebuild = delivery(&chunks, 403, 4, ProjectionDeliveryAction::Rebuild);
    board_rebuild.entity_uri = "kb://board/b_one".to_owned();
    board_rebuild.source_event_id = Some(101);
    apply_and_assert_bound_ack(&backend, &db, &chunks, board_rebuild);
    let converged = vec![
        "kb://task/t_existing".to_owned(),
        "kb://task/t_later".to_owned(),
    ];
    assert_eq!(chunk_entity_uris(&db, &chunks, "b_one"), converged);

    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO task_runs(id,board_id,task_id,summary,error,started_at)
         VALUES ('r_existing','b_one','t_existing',NULL,NULL,3);
         INSERT INTO task_events(id,board_id,task_id,run_id,kind,payload_json) VALUES
           (103,'b_one','t_existing',NULL,'task.updated','{}'),
           (104,'b_one',NULL,'r_existing','run.updated','{}'),
           (105,'b_two',NULL,NULL,'board.updated','{}');",
    )
    .unwrap();
    drop(conn);

    let mut taskful_board = delivery(&chunks, 410, 10, ProjectionDeliveryAction::Upsert);
    taskful_board.entity_uri = "kb://board/b_one".to_owned();
    taskful_board.source_event_id = Some(103);
    let mut runful_board = delivery(&chunks, 411, 11, ProjectionDeliveryAction::Upsert);
    runful_board.entity_uri = "kb://board/b_one".to_owned();
    runful_board.source_event_id = Some(104);
    let mut cross_board = delivery(&chunks, 412, 12, ProjectionDeliveryAction::Upsert);
    cross_board.entity_uri = "kb://board/b_one".to_owned();
    cross_board.source_event_id = Some(105);
    let mut missing_source = delivery(&chunks, 413, 13, ProjectionDeliveryAction::Upsert);
    missing_source.entity_uri = "kb://board/b_one".to_owned();
    let mut missing_event = delivery(&chunks, 414, 14, ProjectionDeliveryAction::Upsert);
    missing_event.entity_uri = "kb://board/b_one".to_owned();
    missing_event.source_event_id = Some(999);
    let mut board_delete = delivery(&chunks, 415, 15, ProjectionDeliveryAction::Delete);
    board_delete.entity_uri = "kb://board/b_one".to_owned();
    board_delete.source_event_id = Some(101);
    for invalid in [
        taskful_board,
        runful_board,
        cross_board,
        missing_source,
        missing_event,
        board_delete,
    ] {
        assert_apply_delivery_error(&backend, &db, &chunks, invalid);
    }
    assert_eq!(
        chunk_entity_uris(&db, &chunks, "b_one"),
        converged,
        "rejected board deliveries must leave Lance task rows unchanged"
    );
}

#[test]
fn snapshot_and_incremental_canonical_validation_converge_with_existing_events() {
    let (_temp, db, backend) = backend();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO tasks(
             id,board_id,seq,title,description,status,archived_at,created_at,updated_at
         ) VALUES ('t_one','b_one',1,'eventful task',NULL,'todo',99,1,1);
         INSERT INTO task_events(id,board_id,task_id,run_id,kind,payload_json)
         VALUES (41,'b_one','t_one',NULL,'mutated','{\"field\":\"title\"}');",
    )
    .unwrap();
    drop(conn);
    let record = task_record_with_event(
        "b_one",
        "t_one",
        "eventful task",
        "mutated {\"field\":\"title\"}",
    );
    let request = prepare_request(
        &backend,
        LANCEDB_CHUNKS_STORE,
        "gen_existing_event",
        41,
        vec![record],
    );
    authorize_snapshotting(&db, &request);
    let evidence = match backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request)) {
        VectorProjectionHelperResponse::PrepareSnapshot(response) => {
            mark_prepared_authority(&db, &response.evidence);
            response.evidence
        }
        response => panic!("unexpected prepare response: {response:?}"),
    };
    publish(&backend, &db, None, &evidence);
    assert!(
        validate_active(&backend, &evidence),
        "delivery event ids are correlation-only and archived_at cannot override canonical status"
    );
}

#[test]
fn provider_failure_resumes_the_same_generation_from_persisted_content_cache() {
    let (_temp, db) = database();
    let provider = Arc::new(CountingProvider::fail_once_on("second task"));
    let backend = VectorProjectionBackend::new(&db, provider.clone())
        .expect("configured backend")
        .with_execution_policy(EmbeddingExecutionPolicy {
            batch_size: 1,
            min_batch_interval: Duration::ZERO,
            max_retries: 0,
            initial_retry_backoff: Duration::ZERO,
            max_retry_backoff: Duration::ZERO,
        });
    let records = vec![
        task_record("b_one", "t_first", "first task"),
        task_record("b_one", "t_second", "second task"),
    ];
    let request = prepare_request(
        &backend,
        LANCEDB_CHUNKS_STORE,
        "gen_provider_resume",
        1,
        records,
    );
    authorize_snapshotting(&db, &request);
    let first = backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(
        request.clone(),
    ));
    match first {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Provider);
            assert!(error.retryable);
        }
        response => panic!("unexpected first prepare response: {response:?}"),
    }
    let generation = generations(&db, LANCEDB_CHUNKS_STORE).join("gen_provider_resume");
    assert!(generation.join("projection-snapshot.json").is_file());
    assert!(generation.join("embedding-cache.json").is_file());
    assert!(!generation.join("projection-evidence.json").exists());

    match backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request)) {
        VectorProjectionHelperResponse::PrepareSnapshot(response) => {
            mark_prepared_authority(&db, &response.evidence);
        }
        response => panic!("unexpected resumed prepare response: {response:?}"),
    }
    assert_eq!(provider.successful_count("first task"), 1);
    assert_eq!(provider.successful_count("second task"), 1);
    assert_eq!(
        provider.batch_calls.load(Ordering::SeqCst),
        3,
        "the first successful content must not be sent again after the provider interruption"
    );
}

fn backend() -> (TempDir, PathBuf, VectorProjectionBackend) {
    let (temp, db) = database();
    let backend =
        VectorProjectionBackend::new(&db, Arc::new(StaticProvider)).expect("configured backend");
    (temp, db, backend)
}

fn database() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("kanban.db");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE projection_database (
            singleton INTEGER PRIMARY KEY,
            database_instance_id TEXT NOT NULL,
            protocol_version INTEGER NOT NULL
         );
         INSERT INTO projection_database(singleton,database_instance_id,protocol_version)
         VALUES (1,'db_fixture',2);
         CREATE TABLE tasks (
             id TEXT PRIMARY KEY,
             board_id TEXT NOT NULL,
             seq INTEGER NOT NULL,
             title TEXT NOT NULL,
             description TEXT,
             status TEXT NOT NULL,
             archived_at INTEGER,
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL
         );
         CREATE TABLE task_comments (
             id TEXT PRIMARY KEY,
             board_id TEXT NOT NULL,
             task_id TEXT NOT NULL,
             body TEXT NOT NULL,
             created_at INTEGER NOT NULL
         );
         CREATE TABLE task_runs (
             id TEXT PRIMARY KEY,
             board_id TEXT NOT NULL,
             task_id TEXT NOT NULL,
             summary TEXT,
             error TEXT,
             started_at INTEGER NOT NULL
         );
         CREATE TABLE task_events (
             id INTEGER PRIMARY KEY,
             board_id TEXT NOT NULL,
             task_id TEXT,
             run_id TEXT,
             kind TEXT NOT NULL,
             payload_json TEXT NOT NULL
         );
         CREATE TABLE labels (
             id TEXT PRIMARY KEY,
             board_id TEXT NOT NULL,
             name TEXT NOT NULL
         );
         CREATE TABLE label_atoms (
             id TEXT PRIMARY KEY,
             label_id TEXT NOT NULL,
             board_id TEXT NOT NULL,
             polarity TEXT NOT NULL,
             kind TEXT NOT NULL,
             text TEXT NOT NULL,
             ordinal INTEGER NOT NULL,
             content_hash TEXT NOT NULL,
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL
         );",
    )
    .unwrap();
    install_authority_schema(&conn);
    drop(conn);
    (temp, db)
}

fn authority_database() -> (TempDir, PathBuf) {
    database()
}

fn install_authority_schema(conn: &rusqlite::Connection) {
    conn.execute_batch(
        "CREATE TABLE projection_store_state (
             store_name TEXT PRIMARY KEY,
             database_instance_id TEXT NOT NULL,
             protocol_version INTEGER NOT NULL,
             schema_version INTEGER NOT NULL,
             control_plane TEXT NOT NULL,
             active_generation TEXT,
             active_fingerprint TEXT,
             active_fence_epoch INTEGER,
             active_snapshot_cursor INTEGER,
             active_provider TEXT,
             active_provider_fingerprint TEXT,
             active_corpus_schema TEXT,
             active_corpus_fingerprint TEXT,
             active_embedding_model TEXT,
             active_embedding_dimensions INTEGER,
             active_canonical_count INTEGER,
             active_canonical_digest TEXT,
             active_delivery_count INTEGER,
             active_delivery_digest TEXT,
             previous_generation TEXT,
             previous_fingerprint TEXT,
             previous_fence_epoch INTEGER,
             previous_snapshot_cursor INTEGER,
             previous_provider TEXT,
             previous_provider_fingerprint TEXT,
             previous_corpus_schema TEXT,
             previous_corpus_fingerprint TEXT,
             previous_embedding_model TEXT,
             previous_embedding_dimensions INTEGER,
             previous_canonical_count INTEGER,
             previous_canonical_digest TEXT,
             previous_delivery_count INTEGER,
             previous_delivery_digest TEXT,
             building_generation TEXT,
             building_fingerprint TEXT,
             building_fence_epoch INTEGER,
             building_provider TEXT,
             building_provider_fingerprint TEXT,
             building_corpus_schema TEXT,
             building_corpus_fingerprint TEXT,
             building_embedding_model TEXT,
             building_embedding_dimensions INTEGER,
             building_canonical_count INTEGER,
             building_canonical_digest TEXT,
             building_delivery_count INTEGER,
             building_delivery_digest TEXT,
             building_phase TEXT,
             lifecycle_status TEXT NOT NULL DEFAULT 'ready',
             snapshot_cursor INTEGER NOT NULL,
             fence_epoch INTEGER NOT NULL,
             lease_owner TEXT,
             lease_token TEXT,
             lease_expires_at INTEGER
         );
         CREATE TABLE projection_deliveries (
             id INTEGER PRIMARY KEY,
             outbox_id INTEGER NOT NULL,
             store_name TEXT NOT NULL,
             board_id TEXT NOT NULL,
             source_event_id INTEGER,
             cursor INTEGER NOT NULL,
             action TEXT NOT NULL,
             entity_uri TEXT NOT NULL,
             payload_json TEXT NOT NULL,
             status TEXT NOT NULL,
             published_generation TEXT,
             attempts INTEGER NOT NULL,
             claim_owner TEXT,
             claim_token TEXT,
             claim_lease_token TEXT,
             claim_fence_epoch INTEGER,
             claim_generation TEXT,
             claim_expires_at INTEGER
         );
         INSERT INTO projection_store_state(
             store_name,database_instance_id,protocol_version,schema_version,
             control_plane,snapshot_cursor,fence_epoch
         ) VALUES
             ('lancedb_chunks','db_fixture',2,1,'v2',0,0),
             ('lancedb_label_atoms','db_fixture',2,1,'v2',0,0);",
    )
    .unwrap();
}

fn authorize_snapshotting(db: &Path, request: &VectorProjectionPrepareSnapshotRequest) {
    let manifest = &request.snapshot.manifest;
    let corpus = manifest.corpus.as_ref().expect("fixture corpus binding");
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute(
        "UPDATE projection_store_state
         SET building_generation=?1,building_fingerprint=NULL,
             building_fence_epoch=?2,building_provider=?3,
             building_provider_fingerprint=?4,building_corpus_schema=?5,
             building_corpus_fingerprint=?6,building_embedding_model=?7,
             building_embedding_dimensions=?8,building_canonical_count=?9,
             building_canonical_digest=?10,building_delivery_count=?11,
             building_delivery_digest=?12,building_phase='snapshotting',
             lifecycle_status='rebuilding',snapshot_cursor=?13,fence_epoch=?2,
             lease_owner='fixture-owner',
             lease_token='fixture-lease-capability',
             lease_expires_at=?14
         WHERE store_name=?15",
        rusqlite::params![
            manifest.generation,
            manifest.fence_epoch,
            manifest.provider,
            manifest.provider_fingerprint,
            corpus.corpus_schema,
            corpus.corpus_fingerprint,
            corpus.embedding_model,
            i64::try_from(corpus.embedding_dimensions).unwrap(),
            manifest.canonical_item_count,
            manifest.canonical_digest,
            manifest.delivery_item_count,
            manifest.delivery_digest,
            manifest.snapshot_cursor,
            i64::MAX,
            manifest.store_name,
        ],
    )
    .unwrap();
}

fn clear_building_authority(db: &Path, store_name: &str) {
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute(
        "UPDATE projection_store_state
         SET building_generation=NULL,building_fingerprint=NULL,
             building_fence_epoch=NULL,building_provider=NULL,
             building_provider_fingerprint=NULL,building_corpus_schema=NULL,
             building_corpus_fingerprint=NULL,building_embedding_model=NULL,
             building_embedding_dimensions=NULL,building_canonical_count=NULL,
             building_canonical_digest=NULL,building_delivery_count=NULL,
             building_delivery_digest=NULL,building_phase=NULL
         WHERE store_name=?1",
        [store_name],
    )
    .unwrap();
}

fn clear_active_authority(db: &Path, store_name: &str) {
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute(
        "UPDATE projection_store_state
         SET active_generation=NULL,active_fingerprint=NULL,
             active_fence_epoch=NULL,active_snapshot_cursor=NULL,
             active_provider=NULL,active_provider_fingerprint=NULL,
             active_corpus_schema=NULL,active_corpus_fingerprint=NULL,
             active_embedding_model=NULL,active_embedding_dimensions=NULL,
             active_canonical_count=NULL,active_canonical_digest=NULL,
             active_delivery_count=NULL,active_delivery_digest=NULL
         WHERE store_name=?1",
        [store_name],
    )
    .unwrap();
}

fn bump_same_owner_fence(db: &Path, store_name: &str) {
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute(
        "UPDATE projection_store_state
         SET fence_epoch=fence_epoch+1
         WHERE store_name=?1",
        [store_name],
    )
    .unwrap();
}

fn release_and_reacquire_projection_lease(
    db: &Path,
    store_name: &str,
    successor_owner: &str,
    successor_token: &str,
) -> i64 {
    let conn = rusqlite::Connection::open(db).unwrap();
    conn.execute(
        "UPDATE projection_deliveries
         SET status='pending',claim_owner=NULL,claim_token=NULL,
             claim_lease_token=NULL,claim_fence_epoch=NULL,
             claim_generation=NULL,claim_expires_at=NULL
         WHERE store_name=?1 AND status='running'
           AND claim_lease_token='fixture-lease-capability'",
        [store_name],
    )
    .unwrap();
    let released = conn
        .execute(
            "UPDATE projection_store_state
             SET lease_owner=NULL,lease_token=NULL,lease_expires_at=NULL
             WHERE store_name=?1 AND lease_owner='fixture-owner'
               AND lease_token='fixture-lease-capability'",
            [store_name],
        )
        .unwrap();
    assert_eq!(released, 1);
    let acquired = conn
        .execute(
            "UPDATE projection_store_state
             SET lease_owner=?1,lease_token=?2,lease_expires_at=?3,
                 fence_epoch=fence_epoch+1
             WHERE store_name=?4 AND lease_token IS NULL",
            rusqlite::params![successor_owner, successor_token, i64::MAX, store_name],
        )
        .unwrap();
    assert_eq!(acquired, 1);
    conn.query_row(
        "SELECT fence_epoch FROM projection_store_state WHERE store_name=?1",
        [store_name],
        |row| row.get(0),
    )
    .unwrap()
}

fn mark_prepared_authority(db: &Path, evidence: &ProjectionArtifactEvidence) {
    let conn = rusqlite::Connection::open(db).unwrap();
    let changed = conn
        .execute(
            "UPDATE projection_store_state
             SET building_fingerprint=?1,building_phase='prepared'
             WHERE store_name=?2 AND building_generation=?3
               AND building_fence_epoch=?4 AND building_phase='snapshotting'",
            rusqlite::params![
                evidence.fingerprint,
                evidence.manifest.store_name,
                evidence.manifest.generation,
                evidence.manifest.fence_epoch,
            ],
        )
        .unwrap();
    assert_eq!(changed, 1);
}

fn mark_published_authority(db: &Path, evidence: &ProjectionArtifactEvidence) {
    let manifest = &evidence.manifest;
    let corpus = manifest.corpus.as_ref().expect("fixture corpus binding");
    let conn = rusqlite::Connection::open(db).unwrap();
    let changed = conn
        .execute(
            "UPDATE projection_store_state
             SET active_generation=?1,active_fingerprint=?2,
                 active_fence_epoch=?3,active_snapshot_cursor=?4,
                 active_provider=?5,active_provider_fingerprint=?6,
                 active_corpus_schema=?7,active_corpus_fingerprint=?8,
                 active_embedding_model=?9,active_embedding_dimensions=?10,
                 active_canonical_count=?11,active_canonical_digest=?12,
                 active_delivery_count=?13,active_delivery_digest=?14,
                 building_generation=NULL,building_fingerprint=NULL,
                 building_fence_epoch=NULL,building_provider=NULL,
                 building_provider_fingerprint=NULL,building_corpus_schema=NULL,
                 building_corpus_fingerprint=NULL,building_embedding_model=NULL,
                 building_embedding_dimensions=NULL,building_canonical_count=NULL,
                 building_canonical_digest=NULL,building_delivery_count=NULL,
                 building_delivery_digest=NULL,building_phase=NULL,
                 lifecycle_status='ready',snapshot_cursor=?4
             WHERE store_name=?15 AND building_generation=?1
               AND building_fingerprint=?2 AND building_fence_epoch=?3",
            rusqlite::params![
                manifest.generation,
                evidence.fingerprint,
                manifest.fence_epoch,
                manifest.snapshot_cursor,
                manifest.provider,
                manifest.provider_fingerprint,
                corpus.corpus_schema,
                corpus.corpus_fingerprint,
                corpus.embedding_model,
                i64::try_from(corpus.embedding_dimensions).unwrap(),
                manifest.canonical_item_count,
                manifest.canonical_digest,
                manifest.delivery_item_count,
                manifest.delivery_digest,
                manifest.store_name,
            ],
        )
        .unwrap();
    assert_eq!(changed, 1);
}

fn prepare(
    backend: &VectorProjectionBackend,
    db: &Path,
    store_name: &str,
    generation: &str,
    fence_epoch: i64,
) -> ProjectionArtifactEvidence {
    let request = prepare_request(backend, store_name, generation, fence_epoch, Vec::new());
    authorize_snapshotting(db, &request);
    match backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request)) {
        VectorProjectionHelperResponse::PrepareSnapshot(response) => {
            mark_prepared_authority(db, &response.evidence);
            response.evidence
        }
        response => panic!("unexpected prepare response: {response:?}"),
    }
}

fn prepare_with_records(
    backend: &VectorProjectionBackend,
    db: &Path,
    store_name: &str,
    generation: &str,
    fence_epoch: i64,
    records: Vec<ProjectionSnapshotRecord>,
) -> ProjectionArtifactEvidence {
    let mut request = prepare_request(backend, store_name, generation, fence_epoch, records);
    request.snapshot.manifest.database_instance_id = rusqlite::Connection::open(db)
        .unwrap()
        .query_row(
            "SELECT database_instance_id FROM projection_database WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    authorize_snapshotting(db, &request);
    match backend.execute(&VectorProjectionHelperRequest::PrepareSnapshot(request)) {
        VectorProjectionHelperResponse::PrepareSnapshot(response) => {
            mark_prepared_authority(db, &response.evidence);
            response.evidence
        }
        response => panic!("unexpected prepare response: {response:?}"),
    }
}

fn prepare_request(
    backend: &VectorProjectionBackend,
    store_name: &str,
    generation: &str,
    fence_epoch: i64,
    records: Vec<ProjectionSnapshotRecord>,
) -> VectorProjectionPrepareSnapshotRequest {
    let descriptor = backend
        .descriptor("req_descriptor")
        .supported_stores
        .into_iter()
        .find(|store| store.store_name == store_name)
        .unwrap();
    let delivery_digest = format!("fnv64:delivery-{generation}");
    let canonical_digest = record_coverage_digest(&records);
    let corpus = descriptor.corpus.clone();
    VectorProjectionPrepareSnapshotRequest {
        context: VectorProjectionMutationContext {
            request_id: format!("req_prepare_{generation}"),
            projection_store: store_name.to_owned(),
            generation_id: generation.to_owned(),
            delivery_digest: delivery_digest.clone(),
        },
        authority: VectorProjectionDestructiveAuthority {
            owner: "fixture-owner".to_owned(),
            lease_token: "fixture-lease-capability".to_owned(),
            fence_epoch,
            role: VectorProjectionGenerationRole::Building,
            generation: generation.to_owned(),
            expected_manifest: None,
            expected_binding: Some(VectorProjectionGenerationBinding {
                generation: generation.to_owned(),
                fingerprint: None,
                fence_epoch,
                snapshot_cursor: None,
                provider: descriptor.provider.clone(),
                provider_fingerprint: descriptor.provider_fingerprint.clone(),
                canonical_count: records.len() as i64,
                canonical_digest: canonical_digest.clone(),
                delivery_count: 0,
                delivery_digest: delivery_digest.clone(),
                corpus: corpus.clone(),
            }),
            building_phase: Some(VectorProjectionBuildingPhase::Snapshotting),
        },
        metadata: descriptor.corpus.clone().unwrap(),
        snapshot: ProjectionSnapshot {
            manifest: ProjectionArtifactManifest {
                store_name: store_name.to_owned(),
                database_instance_id: "db_fixture".to_owned(),
                protocol_version: VECTOR_PROJECTION_PROTOCOL_VERSION,
                schema_version: descriptor.schema_version,
                generation: generation.to_owned(),
                fence_epoch,
                snapshot_cursor: fence_epoch,
                provider: descriptor.provider,
                provider_fingerprint: descriptor.provider_fingerprint,
                corpus,
                canonical_item_count: records.len() as i64,
                canonical_digest,
                delivery_item_count: 0,
                delivery_digest,
                fingerprint: None,
            },
            records,
        },
    }
}

fn task_record(board_id: &str, task_id: &str, title: &str) -> ProjectionSnapshotRecord {
    task_record_with_event(board_id, task_id, title, "")
}

fn task_record_with_event(
    board_id: &str,
    task_id: &str,
    title: &str,
    event_text: &str,
) -> ProjectionSnapshotRecord {
    let payload_json = serde_json::json!({
        "board_id": board_id,
        "task_id": task_id,
        "seq": 1,
        "status": "todo",
        "assignee": null,
        "priority": 0,
        "created_at": 1,
        "updated_at": 1,
        "due_at": null,
        "title": title,
        "description": null,
        "comments": "",
        "run_text": "",
        "event_text": event_text,
    })
    .to_string();
    ProjectionSnapshotRecord {
        board_id: board_id.to_owned(),
        identity: format!("kb://task/{task_id}"),
        content_hash: stable_bytes_hash(payload_json.as_bytes()),
        payload_json,
    }
}

fn record_coverage_digest(records: &[ProjectionSnapshotRecord]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for record in records {
        hash_bytes(&mut hash, record.board_id.as_bytes());
        hash_bytes(&mut hash, record.identity.as_bytes());
        hash_bytes(&mut hash, record.payload_json.as_bytes());
        hash_bytes(&mut hash, record.content_hash.as_bytes());
    }
    format!("fnv64:{hash:016x}")
}

fn stable_bytes_hash(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    hash_bytes(&mut hash, bytes);
    format!("fnv64:{hash:016x}")
}

fn filesystem_digest(root: &Path) -> String {
    fn visit(root: &Path, path: &Path, hash: &mut u64) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let entry_path = entry.path();
            let relative = entry_path.strip_prefix(root).unwrap();
            hash_bytes(hash, relative.to_string_lossy().as_bytes());
            let metadata = fs::symlink_metadata(&entry_path).unwrap();
            if metadata.is_dir() {
                hash_bytes(hash, b"directory");
                visit(root, &entry_path, hash);
            } else if metadata.is_file() {
                hash_bytes(hash, b"file");
                hash_bytes(hash, &fs::read(entry_path).unwrap());
            } else {
                hash_bytes(hash, b"other");
            }
        }
    }

    let mut hash = 0xcbf29ce484222325_u64;
    visit(root, root, &mut hash);
    format!("fnv64:{hash:016x}")
}

fn temp_root(db: &Path) -> &Path {
    db.parent().expect("fixture database parent")
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn delivery(
    evidence: &ProjectionArtifactEvidence,
    id: i64,
    cursor: i64,
    action: ProjectionDeliveryAction,
) -> ProjectionDelivery {
    ProjectionDelivery {
        id,
        outbox_id: id,
        store_name: evidence.manifest.store_name.clone(),
        generation_id: evidence.manifest.generation.clone(),
        board_id: "b_one".to_owned(),
        source_event_id: None,
        cursor,
        action,
        entity_uri: "kb://task/t_one".to_owned(),
        payload_json: "{}".to_owned(),
        attempts: 1,
    }
}

fn apply(
    backend: &VectorProjectionBackend,
    db: &Path,
    evidence: &ProjectionArtifactEvidence,
    delivery: ProjectionDelivery,
) {
    let request = apply_request(evidence, delivery);
    authorize_apply(db, &request);
    match backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request)) {
        VectorProjectionHelperResponse::ApplyBatch(response) => {
            assert_eq!(response.receipt.applied_item_count, 1);
        }
        response => panic!("unexpected apply response: {response:?}"),
    }
}

fn apply_and_assert_bound_ack(
    backend: &VectorProjectionBackend,
    db: &Path,
    evidence: &ProjectionArtifactEvidence,
    delivery: ProjectionDelivery,
) {
    let request = apply_request(evidence, delivery);
    authorize_apply(db, &request);
    let response =
        match backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request.clone())) {
            VectorProjectionHelperResponse::ApplyBatch(response) => response,
            response => panic!("unexpected apply response: {response:?}"),
        };
    assert_eq!(response.ack.request_id, request.context.request_id);
    assert_eq!(
        response.ack.projection_store,
        request.context.projection_store
    );
    assert_eq!(response.ack.generation_id, request.context.generation_id);
    assert_eq!(
        response.ack.delivery_digest,
        request.context.delivery_digest
    );
    assert_eq!(response.receipt.store_name, request.batch.store_name);
    assert_eq!(
        response.receipt.database_instance_id,
        request.batch.database_instance_id
    );
    assert_eq!(
        response.receipt.protocol_version,
        request.batch.protocol_version
    );
    assert_eq!(
        response.receipt.schema_version,
        request.batch.schema_version
    );
    assert_eq!(response.receipt.provider, request.batch.provider);
    assert_eq!(
        response.receipt.provider_fingerprint,
        request.batch.provider_fingerprint
    );
    assert_eq!(
        response.receipt.target_generation,
        request.batch.target_generation
    );
    assert_eq!(response.receipt.fence_epoch, request.batch.fence_epoch);
    assert_eq!(
        response.receipt.applied_item_count,
        request.batch.items.len()
    );
}

fn assert_apply_delivery_error(
    backend: &VectorProjectionBackend,
    db: &Path,
    evidence: &ProjectionArtifactEvidence,
    delivery: ProjectionDelivery,
) {
    let request = apply_request(evidence, delivery);
    authorize_apply(db, &request);
    match backend.execute(&VectorProjectionHelperRequest::ApplyBatch(request)) {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
        }
        response => panic!("invalid board delivery was accepted: {response:?}"),
    }
}

fn apply_request(
    evidence: &ProjectionArtifactEvidence,
    delivery: ProjectionDelivery,
) -> VectorProjectionApplyBatchRequest {
    let batch = ProjectionBatch {
        store_name: evidence.manifest.store_name.clone(),
        database_instance_id: evidence.manifest.database_instance_id.clone(),
        protocol_version: evidence.manifest.protocol_version,
        schema_version: evidence.manifest.schema_version,
        provider: evidence.manifest.provider.clone(),
        provider_fingerprint: evidence.manifest.provider_fingerprint.clone(),
        owner: "fixture-owner".to_owned(),
        lease_token: "fixture-lease-capability".to_owned(),
        fence_epoch: evidence.manifest.fence_epoch,
        target_generation: evidence.manifest.generation.clone(),
        claim_token: format!("fixture-claim-capability-{}", delivery.id),
        claim_expires_at: i64::MAX,
        items: vec![delivery],
    };
    VectorProjectionApplyBatchRequest {
        context: VectorProjectionMutationContext {
            request_id: format!("req_apply_{}", batch.items[0].id),
            projection_store: evidence.manifest.store_name.clone(),
            generation_id: evidence.manifest.generation.clone(),
            delivery_digest: evidence.manifest.delivery_digest.clone(),
        },
        authority: destructive_authority(
            evidence,
            evidence.manifest.fence_epoch,
            VectorProjectionGenerationRole::Building,
            Some(VectorProjectionBuildingPhase::Prepared),
        ),
        batch,
    }
}

fn bind_apply_request_to_lease(
    request: &mut VectorProjectionApplyBatchRequest,
    owner: &str,
    lease_token: &str,
    fence_epoch: i64,
    role: VectorProjectionGenerationRole,
    building_phase: Option<VectorProjectionBuildingPhase>,
) {
    request.batch.owner = owner.to_owned();
    request.batch.lease_token = lease_token.to_owned();
    request.batch.fence_epoch = fence_epoch;
    request.batch.claim_token = format!("successor-claim-capability-{fence_epoch}");
    bind_destructive_authority_to_lease(&mut request.authority, owner, lease_token, fence_epoch);
    request.authority.role = role;
    request.authority.building_phase = building_phase;
}

fn bind_destructive_authority_to_lease(
    authority: &mut VectorProjectionDestructiveAuthority,
    owner: &str,
    lease_token: &str,
    fence_epoch: i64,
) {
    authority.owner = owner.to_owned();
    authority.lease_token = lease_token.to_owned();
    authority.fence_epoch = fence_epoch;
}

fn authorize_apply(db: &Path, request: &VectorProjectionApplyBatchRequest) {
    let conn = rusqlite::Connection::open(db).unwrap();
    for delivery in &request.batch.items {
        conn.execute(
            "INSERT OR REPLACE INTO projection_deliveries(
                 id,outbox_id,store_name,board_id,source_event_id,cursor,action,
                 entity_uri,payload_json,status,attempts,claim_owner,claim_token,
                 claim_lease_token,claim_fence_epoch,claim_generation,claim_expires_at
             ) VALUES (
                 ?1,?2,?3,?4,?5,?6,?7,?8,?9,'running',?10,?11,?12,?13,?14,?15,?16
             )",
            rusqlite::params![
                delivery.id,
                delivery.outbox_id,
                delivery.store_name,
                delivery.board_id,
                delivery.source_event_id,
                delivery.cursor,
                match delivery.action {
                    ProjectionDeliveryAction::Upsert => "upsert",
                    ProjectionDeliveryAction::Delete => "delete",
                    ProjectionDeliveryAction::Rebuild => "rebuild",
                },
                delivery.entity_uri,
                delivery.payload_json,
                delivery.attempts,
                request.batch.owner,
                request.batch.claim_token,
                request.batch.lease_token,
                request.batch.fence_epoch,
                request.batch.target_generation,
                request.batch.claim_expires_at,
            ],
        )
        .unwrap();
    }
}

fn acknowledge_apply(db: &Path, request: &VectorProjectionApplyBatchRequest) {
    let conn = rusqlite::Connection::open(db).unwrap();
    for delivery in &request.batch.items {
        let changed = conn
            .execute(
                "UPDATE projection_deliveries
                 SET status='done',published_generation=claim_generation,
                     claim_owner=NULL,claim_token=NULL,claim_lease_token=NULL,
                     claim_fence_epoch=NULL,claim_generation=NULL,claim_expires_at=NULL
                 WHERE id=?1 AND store_name=?2 AND status='running'
                   AND claim_owner=?3 AND claim_token=?4
                   AND claim_lease_token=?5 AND claim_fence_epoch=?6
                   AND claim_generation=?7",
                rusqlite::params![
                    delivery.id,
                    request.batch.store_name,
                    request.batch.owner,
                    request.batch.claim_token,
                    request.batch.lease_token,
                    request.batch.fence_epoch,
                    request.batch.target_generation,
                ],
            )
            .unwrap();
        assert_eq!(changed, 1);
        let acknowledged: (String, Option<String>) = conn
            .query_row(
                "SELECT status,published_generation
                 FROM projection_deliveries WHERE id=?1",
                [delivery.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            acknowledged,
            (
                "done".to_owned(),
                Some(request.batch.target_generation.clone())
            )
        );
    }
}

fn insert_task_for_apply(db: &Path, title: &str) {
    rusqlite::Connection::open(db)
        .unwrap()
        .execute(
            "INSERT INTO tasks(
                 id,board_id,seq,title,description,status,archived_at,created_at,updated_at
             ) VALUES ('t_one','b_one',1,?1,NULL,'todo',NULL,1,1)",
            [title],
        )
        .unwrap();
}

fn assert_active_ready_after_rollover(
    db: &Path,
    evidence: &ProjectionArtifactEvidence,
    current_lease_fence: i64,
) {
    let state: (Option<String>, Option<i64>, i64, String, Option<String>) =
        rusqlite::Connection::open(db)
            .unwrap()
            .query_row(
                "SELECT active_generation,active_fence_epoch,fence_epoch,
                        lifecycle_status,building_generation
                 FROM projection_store_state WHERE store_name=?1",
                [&evidence.manifest.store_name],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
    assert_eq!(
        state,
        (
            Some(evidence.manifest.generation.clone()),
            Some(evidence.manifest.fence_epoch),
            current_lease_fence,
            "ready".to_owned(),
            None,
        )
    );
}

fn assert_projection_delivery_error(
    response: VectorProjectionHelperResponse,
    accepted_message: &str,
) {
    match response {
        VectorProjectionHelperResponse::Error(error) => {
            assert_eq!(error.kind, VectorProjectionHelperErrorKind::Delivery);
            assert!(error.message.contains("SQLite"));
        }
        response => panic!("{accepted_message}: {response:?}"),
    }
}

fn chunk_hits(
    db: &Path,
    evidence: &ProjectionArtifactEvidence,
    board_id: &str,
) -> Vec<kanban_vector::VectorHit> {
    let store = generation_store(db, evidence);
    store
        .query(&VectorQuery {
            text: "probe".to_owned(),
            limit: 10,
            board_id: board_id.to_owned(),
        })
        .unwrap()
}

fn chunk_entity_uris(
    db: &Path,
    evidence: &ProjectionArtifactEvidence,
    board_id: &str,
) -> Vec<String> {
    let mut entities = chunk_hits(db, evidence, board_id)
        .into_iter()
        .map(|hit| hit.chunk.entity_uri.to_string())
        .collect::<Vec<_>>();
    entities.sort();
    entities.dedup();
    entities
}

fn chunk_fingerprints(
    db: &Path,
    evidence: &ProjectionArtifactEvidence,
    board_id: &str,
) -> Vec<(String, Option<String>, Option<String>)> {
    let mut fingerprints = chunk_hits(db, evidence, board_id)
        .into_iter()
        .map(|hit| (hit.chunk.uri.to_string(), hit.chunk.content_hash, hit.text))
        .collect::<Vec<_>>();
    fingerprints.sort();
    fingerprints
}

fn label_hits(
    db: &Path,
    evidence: &ProjectionArtifactEvidence,
    board_id: &str,
) -> Vec<kanban_vector::LabelAtomHit> {
    let store = generation_store(db, evidence);
    store
        .query_label_atoms(&LabelAtomQuery {
            text: "urgent".to_owned(),
            limit: 10,
            board_id: Some(board_id.to_owned()),
            embedding_model: None,
            polarity: None,
        })
        .unwrap()
}

fn generation_store(db: &Path, evidence: &ProjectionArtifactEvidence) -> LanceDbStore {
    let path = generations(db, &evidence.manifest.store_name)
        .join(&evidence.manifest.generation)
        .join("lance");
    LanceDbStore::connect(LanceDbConfig::new(path, Arc::new(StaticProvider))).unwrap()
}

fn publish(
    backend: &VectorProjectionBackend,
    db: &Path,
    expected_active: Option<&ProjectionArtifactEvidence>,
    prepared: &ProjectionArtifactEvidence,
) -> kanban_contract::ProjectionPublishReceipt {
    let request = VectorProjectionPublishRequest {
        context: context(prepared, "req_publish"),
        authority: destructive_authority(
            prepared,
            prepared.manifest.fence_epoch,
            VectorProjectionGenerationRole::Building,
            Some(VectorProjectionBuildingPhase::Prepared),
        ),
        expected_active: expected_active.cloned(),
        prepared: prepared.clone(),
    };
    match backend.execute(&VectorProjectionHelperRequest::Publish(Box::new(request))) {
        VectorProjectionHelperResponse::Publish(response) => {
            mark_published_authority(db, &response.receipt.active);
            response.receipt
        }
        response => panic!("unexpected publish response: {response:?}"),
    }
}

fn inventory(
    backend: &VectorProjectionBackend,
    store_name: &str,
) -> Vec<kanban_contract::VectorProjectionGenerationInventoryEntry> {
    match backend.execute(&VectorProjectionHelperRequest::Inventory(
        VectorProjectionInventoryRequest {
            request_id: "req_inventory".to_owned(),
            projection_store: store_name.to_owned(),
        },
    )) {
        VectorProjectionHelperResponse::Inventory(response) => response.generations,
        response => panic!("unexpected inventory response: {response:?}"),
    }
}

fn cleanup(
    backend: &VectorProjectionBackend,
    context_evidence: &ProjectionArtifactEvidence,
    current_lease_fence_epoch: i64,
    role: VectorProjectionGenerationRole,
    building_phase: Option<VectorProjectionBuildingPhase>,
    dry_run: bool,
    protection: VectorProjectionCleanupProtection,
) -> kanban_contract::VectorProjectionCleanupResponse {
    match backend.execute(&VectorProjectionHelperRequest::Cleanup(
        VectorProjectionCleanupRequest {
            context: context(context_evidence, "req_cleanup"),
            authority: destructive_authority(
                context_evidence,
                current_lease_fence_epoch,
                role,
                building_phase,
            ),
            dry_run,
            protection,
        },
    )) {
        VectorProjectionHelperResponse::Cleanup(response) => response,
        response => panic!("unexpected cleanup response: {response:?}"),
    }
}

fn destructive_authority(
    evidence: &ProjectionArtifactEvidence,
    current_lease_fence_epoch: i64,
    role: VectorProjectionGenerationRole,
    building_phase: Option<VectorProjectionBuildingPhase>,
) -> VectorProjectionDestructiveAuthority {
    let manifest = &evidence.manifest;
    VectorProjectionDestructiveAuthority {
        owner: "fixture-owner".to_owned(),
        lease_token: "fixture-lease-capability".to_owned(),
        fence_epoch: current_lease_fence_epoch,
        role,
        generation: manifest.generation.clone(),
        expected_manifest: Some(manifest.clone()),
        expected_binding: Some(VectorProjectionGenerationBinding {
            generation: manifest.generation.clone(),
            fingerprint: Some(evidence.fingerprint.clone()),
            fence_epoch: manifest.fence_epoch,
            snapshot_cursor: Some(manifest.snapshot_cursor),
            provider: manifest.provider.clone(),
            provider_fingerprint: manifest.provider_fingerprint.clone(),
            canonical_count: manifest.canonical_item_count,
            canonical_digest: manifest.canonical_digest.clone(),
            delivery_count: manifest.delivery_item_count,
            delivery_digest: manifest.delivery_digest.clone(),
            corpus: manifest.corpus.clone(),
        }),
        building_phase,
    }
}

fn context(
    evidence: &ProjectionArtifactEvidence,
    request_id: &str,
) -> VectorProjectionMutationContext {
    VectorProjectionMutationContext {
        request_id: request_id.to_owned(),
        projection_store: evidence.manifest.store_name.clone(),
        generation_id: evidence.manifest.generation.clone(),
        delivery_digest: evidence.manifest.delivery_digest.clone(),
    }
}

fn validate_active(
    backend: &VectorProjectionBackend,
    evidence: &ProjectionArtifactEvidence,
) -> bool {
    match backend.execute(&VectorProjectionHelperRequest::ValidateActiveContents(
        VectorProjectionValidateActiveRequest {
            request_id: "req_validate_active".to_owned(),
            projection_store: evidence.manifest.store_name.clone(),
            active: evidence.clone(),
        },
    )) {
        VectorProjectionHelperResponse::ValidateActiveContents(response) => response.valid,
        response => panic!("unexpected active validation response: {response:?}"),
    }
}

fn validate_generation(
    backend: &VectorProjectionBackend,
    evidence: &ProjectionArtifactEvidence,
) -> bool {
    match backend.execute(
        &VectorProjectionHelperRequest::ValidateGenerationPublication(
            VectorProjectionValidateGenerationRequest {
                request_id: "req_validate_generation".to_owned(),
                projection_store: evidence.manifest.store_name.clone(),
                expected: evidence.clone(),
            },
        ),
    ) {
        VectorProjectionHelperResponse::ValidateGenerationPublication(response) => response.valid,
        response => panic!("unexpected generation validation response: {response:?}"),
    }
}

fn generations(db: &Path, store_name: &str) -> PathBuf {
    let database_instance_id: String = rusqlite::Connection::open(db)
        .unwrap()
        .query_row(
            "SELECT database_instance_id FROM projection_database WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    kanban_local::checked_projection_store_generations_path(db, &database_instance_id, store_name)
        .unwrap()
}
