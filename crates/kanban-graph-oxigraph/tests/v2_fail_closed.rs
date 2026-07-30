use std::process::Command;

use rusqlite::Connection;

#[test]
fn helper_status_rejects_sqlite_active_without_published_marker() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let db = temp.path().join("kanban.db");
    let conn = Connection::open(&db)?;
    conn.execute_batch(
        "CREATE TABLE boards(
           id TEXT PRIMARY KEY,slug TEXT NOT NULL UNIQUE,archived_at INTEGER
         );
         INSERT INTO boards VALUES('b_default','default',NULL);
         CREATE TABLE projection_store_state(
           store_name TEXT PRIMARY KEY,control_plane TEXT,database_instance_id TEXT,
           protocol_version INTEGER,schema_version INTEGER,active_generation TEXT,
           active_fingerprint TEXT,active_fence_epoch INTEGER,
           active_snapshot_cursor INTEGER,active_provider TEXT,
           active_provider_fingerprint TEXT,active_canonical_count INTEGER,
           active_canonical_digest TEXT,active_delivery_count INTEGER,
           active_delivery_digest TEXT,building_generation TEXT,last_error TEXT
         );
         CREATE TABLE projection_deliveries(store_name TEXT,status TEXT);
         CREATE TABLE entities(uri TEXT PRIMARY KEY,board_id TEXT);
         CREATE TABLE entity_relations(
           subject_uri TEXT,predicate TEXT,object_uri TEXT,graph_uri TEXT,
           authoritative_store TEXT,source_table TEXT,source_id TEXT,
           source_event_id INTEGER,metadata_json TEXT,created_at INTEGER,updated_at INTEGER
         );
         INSERT INTO projection_store_state VALUES(
           'oxigraph_relations','v2','db_test',2,1,'pgen_test','fp_test',7,11,
           'oxigraph','oxigraph-relations-v2',0,'canonical',0,'delivery',NULL,NULL
         );",
    )?;
    drop(conn);

    let root = kanban_local::projection_store_root_path(&db, "oxigraph_relations")?;
    let generation = root.join("generations").join("pgen_test");
    std::fs::create_dir_all(&generation)?;
    std::fs::write(generation.join("relations.json"), b"[]")?;
    let content_fingerprint = fnv_fingerprint(b"[]");
    std::fs::write(
        generation.join("kb-projection-meta.json"),
        serde_json::to_vec(&serde_json::json!({
            "manifest": {
                "store_name": "oxigraph_relations",
                "database_instance_id": "db_test",
                "protocol_version": 2,
                "schema_version": 1,
                "generation": "pgen_test",
                "fence_epoch": 7,
                "snapshot_cursor": 11,
                "provider": "oxigraph",
                "provider_fingerprint": "oxigraph-relations-v2",
                "canonical_item_count": 0,
                "canonical_digest": "canonical",
                "delivery_item_count": 0,
                "delivery_digest": "delivery",
                "fingerprint": "fp_test"
            },
            "fingerprint": "fp_test",
            "content_fingerprint": content_fingerprint
        }))?,
    )?;

    let output = Command::new(env!("CARGO_BIN_EXE_kanban-graph-oxigraph"))
        .args([
            "status",
            "--db",
            db.to_str().expect("UTF-8 DB path"),
            "--board",
            "default",
        ])
        .output()?;
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("active generation is not published"),
        "{stdout}"
    );
    Ok(())
}

fn fnv_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in (bytes.len() as u64).to_le_bytes().iter().chain(bytes) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{hash:016x}")
}
