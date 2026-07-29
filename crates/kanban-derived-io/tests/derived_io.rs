use std::cell::RefCell;

use kanban_derived_io::{
    board_id, connect_file, current_last_event_id, derived_status_by_name,
    graph_relation_snapshot_for_board, has_pending_vector_outbox_for_board, maintenance_lock_path,
    rebuild_lancedb_chunks_with_store, sync_oxigraph_with_store, vector_chunks_for_board,
};
use kanban_entity::{EntityUri, Predicate, Relation};
use kanban_graph::{GraphError, GraphQueryRow, GraphStoreStatus, RelationGraph};
use kanban_indexer::{LANCEDB_CHUNKS_STORE, OXIGRAPH_RELATIONS_STORE};
use kanban_vector::{
    ChunkVectorStore, EmbeddingChunk, LabelAtomVectorStore, QueryEmbeddingProvider, VectorError,
    VectorQuery, VectorStoreBackend, VectorStoreStatus,
};
use tempfile::NamedTempFile;

#[test]
fn db_status_and_vector_rebuild_use_narrow_sqlite_io() {
    let db = TestDb::new();
    let conn = connect_file(db.path()).unwrap();

    assert_eq!(board_id(&conn, "default").unwrap(), "b_test");
    assert_eq!(current_last_event_id(&conn, "b_test").unwrap(), Some(2));
    assert!(has_pending_vector_outbox_for_board(&conn, "b_test", Some(2)).unwrap());

    let chunks = vector_chunks_for_board(&conn, "b_test", "test-model").unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].task_id.as_deref(), Some("t_one"));
    assert_eq!(chunks[0].embedding_model, "test-model");

    drop(conn);
    let store = MockVectorStore::default();
    let status = rebuild_lancedb_chunks_with_store(db.path(), "default", &store).unwrap();
    assert!(status.message.contains("rebuilt 1 chunk"));
    assert_eq!(store.deleted_boards.borrow().as_slice(), ["b_test"]);
    assert_eq!(store.upserted.borrow().len(), 1);

    let conn = connect_file(db.path()).unwrap();
    let derived = derived_status_by_name(&conn, LANCEDB_CHUNKS_STORE).unwrap();
    assert!(!derived.dirty);
    assert_eq!(derived.last_event_id, 2);
    let pending: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM index_outbox WHERE target='lancedb' AND status!='done'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(pending, 0);
}

#[test]
fn connect_file_rejects_active_maintenance_lock() {
    let file = NamedTempFile::new().unwrap();
    let lock_path = maintenance_lock_path(file.path());
    std::fs::write(&lock_path, "pid=\n").unwrap();

    let error = connect_file(file.path()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("database is locked for maintenance")
    );
    assert!(lock_path.exists());
}

#[test]
fn connect_file_removes_stale_maintenance_lock_and_opens() {
    let file = NamedTempFile::new().unwrap();
    let lock_path = maintenance_lock_path(file.path());
    std::fs::write(&lock_path, "pid=999999999\n").unwrap();

    let _conn = connect_file(file.path()).unwrap();
    assert!(!lock_path.exists());
}

#[test]
fn graph_snapshot_and_sync_use_narrow_sqlite_io() {
    let db = TestDb::new();
    let conn = connect_file(db.path()).unwrap();
    let relations = graph_relation_snapshot_for_board(&conn, "b_test").unwrap();
    assert_eq!(relations.len(), 1);
    assert_eq!(relations[0].predicate, Predicate::BelongsToBoard);
    drop(conn);

    let graph = MockGraph::default();
    let status = sync_oxigraph_with_store(db.path(), "default", &graph).unwrap();
    assert!(status.message.contains("1 job"));
    assert_eq!(graph.upserted.borrow().len(), 1);

    let conn = connect_file(db.path()).unwrap();
    let derived = derived_status_by_name(&conn, OXIGRAPH_RELATIONS_STORE).unwrap();
    assert_eq!(derived.last_event_id, 2);
    let pending: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM index_outbox WHERE target='oxigraph' AND status!='done'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(pending, 0);
}

#[test]
fn legacy_graph_writer_is_rejected_after_projection_v2_takes_control() {
    let db = TestDb::new();
    let conn = connect_file(db.path()).unwrap();
    conn.execute_batch(
        "CREATE TABLE projection_store_state(
           store_name TEXT PRIMARY KEY,
           control_plane TEXT NOT NULL,
           building_generation TEXT
         );
         INSERT INTO projection_store_state(store_name,control_plane)
         VALUES('oxigraph_relations','v2');",
    )
    .unwrap();
    drop(conn);

    let graph = MockGraph::default();
    let error = sync_oxigraph_with_store(db.path(), "default", &graph).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("managed by projection maintenance v2")
    );
    assert!(graph.upserted.borrow().is_empty());

    let conn = connect_file(db.path()).unwrap();
    let pending: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM index_outbox
             WHERE target='oxigraph' AND status='pending'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(pending, 1);
}

#[test]
fn legacy_label_atom_writer_is_rejected_after_projection_v2_takes_control() {
    let db = TestDb::new();
    let conn = connect_file(db.path()).unwrap();
    conn.execute_batch(
        "CREATE TABLE projection_store_state(
           store_name TEXT PRIMARY KEY,
           control_plane TEXT NOT NULL,
           building_generation TEXT
         );
         INSERT INTO projection_store_state(store_name,control_plane)
         VALUES('lancedb_label_atoms','v2');",
    )
    .unwrap();
    drop(conn);

    let store = MockVectorStore::default();
    let error =
        kanban_derived_io::sync_lancedb_label_atoms_with_store(db.path(), "default", &store)
            .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("managed by projection maintenance v2")
    );
}

struct TestDb {
    file: NamedTempFile,
}

impl TestDb {
    fn new() -> Self {
        let file = NamedTempFile::new().unwrap();
        let db = Self { file };
        db.seed();
        db
    }

    fn path(&self) -> &std::path::Path {
        self.file.path()
    }

    fn seed(&self) {
        let conn = connect_file(self.path()).unwrap();
        conn.execute_batch(
            "CREATE TABLE boards(id TEXT PRIMARY KEY, slug TEXT NOT NULL, archived_at INTEGER);
             CREATE TABLE tasks(id TEXT PRIMARY KEY, board_id TEXT NOT NULL, seq INTEGER NOT NULL, title TEXT NOT NULL, description TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, archived_at INTEGER);
             CREATE TABLE task_events(id INTEGER PRIMARY KEY AUTOINCREMENT, event_id TEXT NOT NULL, board_id TEXT NOT NULL, task_id TEXT, created_at INTEGER NOT NULL);
             CREATE TABLE entities(uri TEXT PRIMARY KEY, board_id TEXT, title TEXT);
             CREATE TABLE entity_relations(subject_uri TEXT NOT NULL, predicate TEXT NOT NULL, object_uri TEXT NOT NULL, graph_uri TEXT NOT NULL, authoritative_store TEXT NOT NULL, source_table TEXT, source_id TEXT, source_event_id INTEGER, metadata_json TEXT NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE index_outbox(id INTEGER PRIMARY KEY AUTOINCREMENT, source_event_id INTEGER, target TEXT NOT NULL, entity_uri TEXT NOT NULL, action TEXT NOT NULL, payload_json TEXT NOT NULL, status TEXT NOT NULL, attempts INTEGER NOT NULL, last_error TEXT, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE derived_store_state(store_name TEXT PRIMARY KEY, schema_version INTEGER NOT NULL, last_event_id INTEGER NOT NULL, dirty INTEGER NOT NULL, last_rebuild_at INTEGER, last_sync_at INTEGER, last_error TEXT, updated_at INTEGER NOT NULL);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO boards(id, slug, archived_at) VALUES ('b_test', 'default', NULL)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO tasks(id, board_id, seq, title, description, created_at, updated_at, archived_at) VALUES ('t_one', 'b_test', 1, 'Task one', 'Body', 1000, 2000, NULL)", []).unwrap();
        conn.execute("INSERT INTO task_events(event_id, board_id, task_id, created_at) VALUES ('e_one', 'b_test', 't_one', 1000)", []).unwrap();
        conn.execute("INSERT INTO task_events(event_id, board_id, task_id, created_at) VALUES ('e_two', 'b_test', 't_one', 2000)", []).unwrap();
        conn.execute("INSERT INTO entities(uri, board_id, title) VALUES ('kb://task/t_one', 'b_test', 'Task one')", []).unwrap();
        conn.execute("INSERT INTO entities(uri, board_id, title) VALUES ('kb://board/b_test', 'b_test', 'Default')", []).unwrap();
        conn.execute("INSERT INTO entity_relations(subject_uri,predicate,object_uri,graph_uri,authoritative_store,source_table,source_id,source_event_id,metadata_json,created_at,updated_at) VALUES ('kb://task/t_one','belongs_to_board','kb://board/b_test','kb://graph/indexed','sqlite','tasks','t_one',2,'{}',1000,2000)", []).unwrap();
        conn.execute("INSERT INTO derived_store_state(store_name,schema_version,last_event_id,dirty,last_rebuild_at,last_sync_at,last_error,updated_at) VALUES (?1,1,0,1,NULL,NULL,NULL,1000)", [LANCEDB_CHUNKS_STORE]).unwrap();
        conn.execute("INSERT INTO derived_store_state(store_name,schema_version,last_event_id,dirty,last_rebuild_at,last_sync_at,last_error,updated_at) VALUES (?1,1,1,1,NULL,NULL,NULL,1000)", [OXIGRAPH_RELATIONS_STORE]).unwrap();
        conn.execute("INSERT INTO index_outbox(source_event_id,target,entity_uri,action,payload_json,status,attempts,last_error,created_at,updated_at) VALUES (2,'lancedb','kb://task/t_one','upsert','{}','pending',0,NULL,2000,2000)", []).unwrap();
        conn.execute("INSERT INTO index_outbox(source_event_id,target,entity_uri,action,payload_json,status,attempts,last_error,created_at,updated_at) VALUES (2,'oxigraph','kb://task/t_one','upsert','{}','pending',0,NULL,2000,2000)", []).unwrap();
        conn.execute("INSERT INTO index_outbox(source_event_id,target,entity_uri,action,payload_json,status,attempts,last_error,created_at,updated_at) VALUES (2,'tantivy','kb://task/t_one','upsert','{}','pending',0,NULL,2000,2000)", []).unwrap();
    }
}

#[derive(Default)]
struct MockVectorStore {
    deleted_boards: RefCell<Vec<String>>,
    upserted: RefCell<Vec<EmbeddingChunk>>,
}

impl VectorStoreBackend for MockVectorStore {
    fn embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> VectorStoreStatus {
        VectorStoreStatus::new("mock", true, "mock vector")
    }
}

impl ChunkVectorStore for MockVectorStore {
    fn delete_board(&self, board_id: &str) -> Result<(), VectorError> {
        self.deleted_boards.borrow_mut().push(board_id.to_owned());
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), VectorError> {
        Ok(())
    }

    fn upsert(&self, chunks: &[EmbeddingChunk]) -> Result<(), VectorError> {
        self.upserted.borrow_mut().extend_from_slice(chunks);
        Ok(())
    }

    fn query(&self, _query: &VectorQuery) -> Result<Vec<kanban_vector::VectorHit>, VectorError> {
        Ok(Vec::new())
    }
}

impl QueryEmbeddingProvider for MockVectorStore {}

impl LabelAtomVectorStore for MockVectorStore {}

#[derive(Default)]
struct MockGraph {
    upserted: RefCell<Vec<Relation>>,
    deleted: RefCell<Vec<EntityUri>>,
}

impl RelationGraph for MockGraph {
    fn status(&self) -> GraphStoreStatus {
        GraphStoreStatus {
            backend: "mock".to_owned(),
            enabled: true,
            message: "mock graph".to_owned(),
        }
    }

    fn init(&self) -> Result<(), GraphError> {
        Ok(())
    }

    fn upsert(&self, relations: &[Relation]) -> Result<(), GraphError> {
        self.upserted.borrow_mut().extend_from_slice(relations);
        Ok(())
    }

    fn delete(&self, entity_uri: &EntityUri) -> Result<(), GraphError> {
        self.deleted.borrow_mut().push(entity_uri.clone());
        Ok(())
    }

    fn rebuild(&self, relations: &[Relation]) -> Result<(), GraphError> {
        self.upsert(relations)
    }

    fn neighbors(
        &self,
        _entity_uri: &EntityUri,
        _predicate: Option<Predicate>,
        _limit: usize,
    ) -> Result<Vec<Relation>, GraphError> {
        Ok(Vec::new())
    }

    fn query(&self, _sparql: &str, _limit: usize) -> Result<Vec<GraphQueryRow>, GraphError> {
        Ok(Vec::new())
    }
}
