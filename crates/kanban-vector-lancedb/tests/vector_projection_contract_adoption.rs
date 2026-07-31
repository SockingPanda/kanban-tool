use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use kanban_contract::{
    VectorProjectionDescriptorRequest, VectorProjectionHelperRequest,
    VectorProjectionHelperResponse,
};
use kanban_vector::decode_vector_projection_response;

fn fixture(relative: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("schemas/fixtures/helper")
            .join(relative),
    )
    .unwrap()
    .trim()
    .to_owned()
}

fn descriptor_request() -> VectorProjectionHelperRequest {
    VectorProjectionHelperRequest::Descriptor(VectorProjectionDescriptorRequest {
        request_id: "req_fixture_descriptor".to_owned(),
    })
}

#[test]
fn vector_projection_request_fixture_is_produced_by_contract_dto() {
    assert_eq!(
        serde_json::to_string(&descriptor_request()).unwrap(),
        fixture("vector-projection-request.v1.valid.json")
    );
}

#[test]
fn vector_projection_request_fixture_is_consumed_by_real_projection_handler() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_kanban-vector-lancedb"))
        .args([
            "projection",
            "--db",
            "/tmp/kanban-vector-projection-contract-consumer.db",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(fixture("vector-projection-request.v1.valid.json").as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response =
        serde_json::from_slice::<VectorProjectionHelperResponse>(&output.stdout).unwrap();
    assert!(matches!(
        response,
        VectorProjectionHelperResponse::Descriptor(_)
    ));
}

#[test]
fn configured_projection_cli_uses_the_generation_backend_descriptor() {
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
         VALUES (1,'db_cli_configured',2);",
    )
    .unwrap();
    drop(conn);
    let config = temp.path().join("config.toml");
    std::fs::write(
        &config,
        "[vector]\nprovider = \"ollama\"\nendpoint = \"http://127.0.0.1:11434\"\nmodel = \"configured-model\"\ndimensions = 7\n",
    )
    .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_kanban-vector-lancedb"))
        .args([
            "projection",
            "--db",
            db.to_str().unwrap(),
            "--vector-config",
            config.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&descriptor_request()).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let descriptor =
        match serde_json::from_slice::<VectorProjectionHelperResponse>(&output.stdout).unwrap() {
            VectorProjectionHelperResponse::Descriptor(descriptor) => descriptor,
            response => panic!("unexpected configured projection response: {response:?}"),
        };
    assert_eq!(descriptor.supported_stores.len(), 2);
    for store in descriptor.supported_stores {
        assert_eq!(store.provider, "ollama");
        let corpus = store.corpus.expect("configured corpus binding");
        assert_eq!(corpus.embedding_model, "configured-model");
        assert_eq!(corpus.embedding_dimensions, 7);
    }
}

#[test]
fn vector_projection_response_fixture_is_produced_by_real_projection_handler() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_kanban-vector-lancedb"))
        .args([
            "projection",
            "--db",
            "/tmp/kanban-vector-projection-contract-producer.db",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = serde_json::to_vec(&descriptor_request()).unwrap();
    child.stdin.take().unwrap().write_all(&request).unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        fixture("vector-projection-response.v1.valid.json")
    );
}

#[test]
fn vector_projection_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    let response = decode_vector_projection_response(
        &descriptor_request(),
        fixture("vector-projection-response.v1.valid.json").as_bytes(),
    )
    .unwrap();

    assert!(matches!(
        response,
        VectorProjectionHelperResponse::Descriptor(_)
    ));
}
