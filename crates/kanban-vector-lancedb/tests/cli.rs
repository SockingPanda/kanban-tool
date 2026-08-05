use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use kanban_helper_protocol::HelperEnvelope;
use kanban_protocol::VectorHelperStatusResponse;
use rusqlite::{Connection, params};
use serde_json::Value;

fn helper_output(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kanban-vector-lancedb"))
        .args(args)
        .output()
        .unwrap()
}

fn create_control_plane_db(root: &Path) -> PathBuf {
    let db = root.join("kanban.db");
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE projection_database(
           singleton INTEGER PRIMARY KEY,
           database_instance_id TEXT NOT NULL,
           protocol_version INTEGER NOT NULL
         );
         INSERT INTO projection_database VALUES(1,'db_cli',2);
         CREATE TABLE boards(
           id TEXT PRIMARY KEY,slug TEXT NOT NULL UNIQUE,archived_at INTEGER
         );
         INSERT INTO boards VALUES('b_default','default',NULL);
         CREATE TABLE projection_store_state(
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
           snapshot_cursor INTEGER NOT NULL,
           fence_epoch INTEGER NOT NULL,
           lease_owner TEXT,
           lease_token TEXT,
           lease_expires_at INTEGER
         );",
    )
    .unwrap();
    for store in ["lancedb_chunks", "lancedb_label_atoms"] {
        conn.execute(
            "INSERT INTO projection_store_state(
               store_name,database_instance_id,protocol_version,schema_version,
               control_plane,snapshot_cursor,fence_epoch
             ) VALUES(?1,'db_cli',2,1,'v2',0,0)",
            params![store],
        )
        .unwrap();
    }
    db
}

fn write_vector_config(root: &Path) -> PathBuf {
    let config = root.join("vector.toml");
    std::fs::write(
        &config,
        "[vector]\nprovider = \"ollama\"\nendpoint = \"http://127.0.0.1:11434\"\nmodel = \"test-model\"\ndimensions = 3\n",
    )
    .unwrap();
    config
}

fn write_unconfigured_vector_config(root: &Path) -> PathBuf {
    let config = root.join("vector-empty.toml");
    std::fs::write(&config, "").unwrap();
    config
}

fn status_payload(output: Output) -> Value {
    assert!(output.status.success(), "status failed: {:?}", output);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let envelope = HelperEnvelope::from_json(&stdout)
        .unwrap_or_else(|error| panic!("invalid helper envelope: {error}: {stdout}"));
    let status: VectorHelperStatusResponse = envelope
        .decode()
        .unwrap_or_else(|error| panic!("invalid helper status payload: {error}: {stdout}"));
    serde_json::to_value(status).unwrap()
}

#[test]
fn label_atom_status_without_provider_succeeds_with_degraded_payload() {
    let temp = tempfile::tempdir().unwrap();
    let db = create_control_plane_db(temp.path());
    let config = write_unconfigured_vector_config(temp.path());

    let payload = status_payload(helper_output(&[
        "label-atoms-status",
        "--db",
        db.to_str().unwrap(),
        "--board",
        "default",
        "--vector-config",
        config.to_str().unwrap(),
    ]));
    assert_eq!(payload["backend"], "lancedb-label-atoms");
    assert_eq!(payload["enabled"], false);
    assert!(payload["message"].as_str().unwrap().contains("degraded"));
}

#[test]
fn vector_status_without_provider_succeeds_with_disabled_payload() {
    let temp = tempfile::tempdir().unwrap();
    let db = create_control_plane_db(temp.path());
    let config = write_unconfigured_vector_config(temp.path());

    let payload = status_payload(helper_output(&[
        "status",
        "--db",
        db.to_str().unwrap(),
        "--board",
        "default",
        "--vector-config",
        config.to_str().unwrap(),
    ]));
    assert_eq!(payload["backend"], "lancedb");
    assert_eq!(payload["enabled"], false);
    assert!(payload["message"].as_str().unwrap().contains("degraded"));
}

#[test]
fn vector_json_error_includes_invalid_element_path() {
    let output = helper_output(&[
        "query-label-atoms",
        "--db",
        "/tmp/kanban-vector-lancedb-path-diagnostics.db",
        "--board",
        "kanban-tool",
        "--vector-json",
        r#"[1.0,"bad"]"#,
    ]);

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("invalid --vector-json payload"), "{stdout}");
    assert!(stdout.contains("[1]"), "{stdout}");
}

#[test]
fn vector_json_rejects_trailing_characters_with_context() {
    let output = helper_output(&[
        "query-label-atoms",
        "--db",
        "/tmp/kanban-vector-lancedb-path-diagnostics.db",
        "--board",
        "kanban-tool",
        "--vector-json",
        "[1.0] trailing",
    ]);

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("invalid --vector-json payload"), "{stdout}");
    assert!(stdout.contains("trailing"), "{stdout}");
}

#[test]
fn legacy_mutations_reject_v2_before_creating_the_v1_store() {
    for command in ["rebuild", "sync", "rebuild-label-atoms", "sync-label-atoms"] {
        let temp = tempfile::tempdir().unwrap();
        let db = create_control_plane_db(temp.path());
        let config = write_vector_config(temp.path());
        let v1_store = kanban_local::vector_store_path(&db);

        let output = helper_output(&[
            command,
            "--db",
            db.to_str().unwrap(),
            "--board",
            "default",
            "--vector-config",
            config.to_str().unwrap(),
        ]);

        assert!(!output.status.success(), "{command} unexpectedly succeeded");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("managed by projection maintenance v2"),
            "{command}: {stdout}"
        );
        assert!(
            !v1_store.exists(),
            "{command} touched the legacy vector store before rejecting v2"
        );
    }
}

#[test]
fn normal_reads_reject_legacy_control_without_opening_or_creating_v1() {
    let commands: &[&[&str]] = &[
        &["query-chunks", "--text", "needle"],
        &["query-label-atoms", "--vector-json", "[0.0,0.0,0.0]"],
        &["status"],
        &["label-atoms-status"],
    ];
    for command in commands {
        let temp = tempfile::tempdir().unwrap();
        let db = create_control_plane_db(temp.path());
        let config = write_vector_config(temp.path());
        Connection::open(&db)
            .unwrap()
            .execute(
                "UPDATE projection_store_state SET control_plane='legacy'",
                [],
            )
            .unwrap();
        let mut args = command.to_vec();
        args.extend([
            "--db",
            db.to_str().unwrap(),
            "--board",
            "default",
            "--vector-config",
            config.to_str().unwrap(),
        ]);

        let output = helper_output(&args);

        assert!(
            !output.status.success(),
            "{command:?} unexpectedly succeeded"
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("Projection v2 read authority rejected"),
            "{command:?}: {stdout}"
        );
        assert!(
            !kanban_local::vector_store_path(&db).exists(),
            "{command:?} opened or created the legacy v1 vector store"
        );
    }
}

#[test]
fn chunk_query_resolves_board_when_explicit_board_id_is_omitted() {
    let temp = tempfile::tempdir().unwrap();
    let db = create_control_plane_db(temp.path());

    let output = helper_output(&[
        "query-chunks",
        "--db",
        db.to_str().unwrap(),
        "--board",
        "default",
        "--text",
        "needle",
    ]);

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("requires a configured embedding provider"),
        "query did not pass clap and board resolution: {stdout}"
    );
    assert!(!stdout.contains("--board-id"), "{stdout}");
}

#[test]
fn label_query_rejects_an_explicit_board_mismatch_before_opening_a_store() {
    let temp = tempfile::tempdir().unwrap();
    let db = create_control_plane_db(temp.path());

    let output = helper_output(&[
        "query-label-atoms",
        "--db",
        db.to_str().unwrap(),
        "--board",
        "default",
        "--board-id",
        "b_other",
        "--vector-json",
        "[0.0,0.0,0.0]",
    ]);

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("query label atom board mismatch"),
        "{stdout}"
    );
    assert!(!kanban_local::vector_store_path(&db).exists());
}
