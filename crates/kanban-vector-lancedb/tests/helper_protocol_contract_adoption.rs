use kanban_entity::{ChunkRef, EntityUri};
use kanban_helper_protocol::HelperEnvelope;
use kanban_protocol::{
    VectorHelperCheckProviderResponse, VectorHelperEmbedQueryResponse, VectorHelperErrorResponse,
    VectorHelperHandshakeResponse, VectorHelperLabelAtomsStatusResponse,
    VectorHelperQueryChunksResponse, VectorHelperQueryLabelAtomsResponse,
    VectorHelperRebuildLabelAtomsResponse, VectorHelperRebuildResponse, VectorHelperStatusResponse,
    VectorHelperSyncLabelAtomsResponse, VectorHelperSyncResponse,
};
use kanban_vector::{LabelAtomHit, LabelAtomVectorHit, VectorHit, VectorStoreStatus};
use kanban_vector_lancedb::{
    vector_helper_check_provider_response, vector_helper_embed_query_response,
    vector_helper_error_response, vector_helper_handshake_response,
    vector_helper_query_chunks_response, vector_helper_query_label_atom_vectors_response,
    vector_helper_status_response,
};
use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;

fn fixture(relative: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join("schemas/fixtures/helper").join(relative))
        .unwrap()
        .trim()
        .to_owned()
}

fn assert_produced<T: Serialize>(relative: &str, actual: T) {
    assert_eq!(serde_json::to_string(&actual).unwrap(), fixture(relative));
}

fn assert_consumed<T: Serialize + DeserializeOwned>(relative: &str) {
    let expected = fixture(relative);
    let envelope = HelperEnvelope {
        protocol: HelperEnvelope::PROTOCOL.to_owned(),
        payload_json: expected.clone(),
    };
    let decoded: T = envelope.decode().unwrap();
    assert_eq!(serde_json::to_string(&decoded).unwrap(), expected);
}

fn status(
    backend: &str,
    message: &str,
    diagnostics: &[&str],
    generation: i64,
) -> VectorStoreStatus {
    VectorStoreStatus {
        backend: backend.to_owned(),
        enabled: true,
        message: message.to_owned(),
        diagnostics: diagnostics
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        dirty: Some(false),
        board_dirty: Some(false),
        generation: Some(generation),
    }
}

macro_rules! status_contract_tests {
    ($producer:ident, $consumer:ident, $ty:ty, $fixture:literal, $backend:literal, $message:literal, $diagnostics:expr, $generation:literal) => {
        #[test]
        fn $producer() {
            assert_produced(
                $fixture,
                vector_helper_status_response(status(
                    $backend,
                    $message,
                    $diagnostics,
                    $generation,
                )),
            );
        }

        #[test]
        fn $consumer() {
            assert_consumed::<$ty>($fixture);
        }
    };
}

#[test]
fn vector_helper_handshake_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "vector-handshake-response.v1.valid.json",
        vector_helper_handshake_response(env!("CARGO_PKG_VERSION")),
    );
}

#[test]
fn vector_helper_handshake_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<VectorHelperHandshakeResponse>("vector-handshake-response.v1.valid.json");
}

#[test]
fn vector_helper_error_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "vector-error-response.v1.valid.json",
        vector_helper_error_response("vector fixture failure"),
    );
}

#[test]
fn vector_helper_error_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<VectorHelperErrorResponse>("vector-error-response.v1.valid.json");
}

#[test]
fn vector_helper_check_provider_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "vector-check-provider-response.v1.valid.json",
        vector_helper_check_provider_response(),
    );
}

#[test]
fn vector_helper_check_provider_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<VectorHelperCheckProviderResponse>(
        "vector-check-provider-response.v1.valid.json",
    );
}

status_contract_tests!(
    vector_helper_status_response_fixture_is_produced_by_real_helper_adapter,
    vector_helper_status_response_fixture_is_consumed_by_runtime_protocol_decoder,
    VectorHelperStatusResponse,
    "vector-status-response.v1.valid.json",
    "lancedb",
    "vector ready",
    &[],
    7
);
status_contract_tests!(
    vector_helper_rebuild_response_fixture_is_produced_by_real_helper_adapter,
    vector_helper_rebuild_response_fixture_is_consumed_by_runtime_protocol_decoder,
    VectorHelperRebuildResponse,
    "vector-rebuild-response.v1.valid.json",
    "lancedb",
    "vector rebuilt",
    &[],
    8
);
status_contract_tests!(
    vector_helper_sync_response_fixture_is_produced_by_real_helper_adapter,
    vector_helper_sync_response_fixture_is_consumed_by_runtime_protocol_decoder,
    VectorHelperSyncResponse,
    "vector-sync-response.v1.valid.json",
    "lancedb",
    "vector synced",
    &[],
    9
);
status_contract_tests!(
    vector_helper_label_atoms_status_response_fixture_is_produced_by_real_helper_adapter,
    vector_helper_label_atoms_status_response_fixture_is_consumed_by_runtime_protocol_decoder,
    VectorHelperLabelAtomsStatusResponse,
    "vector-label-atoms-status-response.v1.valid.json",
    "lancedb-label-atoms",
    "label atoms ready",
    &["label_atom_helper"],
    10
);
status_contract_tests!(
    vector_helper_rebuild_label_atoms_response_fixture_is_produced_by_real_helper_adapter,
    vector_helper_rebuild_label_atoms_response_fixture_is_consumed_by_runtime_protocol_decoder,
    VectorHelperRebuildLabelAtomsResponse,
    "vector-rebuild-label-atoms-response.v1.valid.json",
    "lancedb-label-atoms",
    "label atoms rebuilt",
    &["label_atom_helper"],
    11
);
status_contract_tests!(
    vector_helper_sync_label_atoms_response_fixture_is_produced_by_real_helper_adapter,
    vector_helper_sync_label_atoms_response_fixture_is_consumed_by_runtime_protocol_decoder,
    VectorHelperSyncLabelAtomsResponse,
    "vector-sync-label-atoms-response.v1.valid.json",
    "lancedb-label-atoms",
    "label atoms synced",
    &["label_atom_helper"],
    12
);

#[test]
fn vector_helper_query_chunks_response_fixture_is_produced_by_real_helper_adapter() {
    let hit = VectorHit {
        chunk: ChunkRef {
            uri: EntityUri::new("kb://chunk/task/t_vector/0").unwrap(),
            entity_uri: EntityUri::new("kb://task/t_vector").unwrap(),
            ordinal: 0,
            content_hash: Some("hash-vector".to_owned()),
        },
        score: 0.91,
        text: Some("vector fixture text".to_owned()),
        summary: Some("Vector fixture".to_owned()),
    };
    assert_produced(
        "vector-query-chunks-response.v1.valid.json",
        vector_helper_query_chunks_response(vec![hit]),
    );
}

#[test]
fn vector_helper_query_chunks_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<VectorHelperQueryChunksResponse>(
        "vector-query-chunks-response.v1.valid.json",
    );
}

fn label_atom_hit() -> LabelAtomHit {
    LabelAtomHit {
        atom_id: "la_backend".to_owned(),
        label_id: "l_backend".to_owned(),
        label_name: "backend".to_owned(),
        board_id: "b_fixture".to_owned(),
        polarity: "positive".to_owned(),
        kind: "applies_when".to_owned(),
        text: "touches Rust service code".to_owned(),
        ordinal: 0,
        content_hash: "hash-atom".to_owned(),
        embedding_model: "fixture-model".to_owned(),
        distance: 0.125,
    }
}

#[test]
fn vector_helper_query_label_atoms_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "vector-query-label-atoms-response.v1.valid.json",
        vector_helper_query_label_atom_vectors_response(vec![LabelAtomVectorHit {
            hit: label_atom_hit(),
            vector: Some(vec![1.0, 0.0]),
        }]),
    );
}

#[test]
fn vector_helper_query_label_atoms_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<VectorHelperQueryLabelAtomsResponse>(
        "vector-query-label-atoms-response.v1.valid.json",
    );
}

#[test]
fn vector_helper_embed_query_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "vector-embed-query-response.v1.valid.json",
        vector_helper_embed_query_response(vec![0.25, -0.5, 0.75]),
    );
}

#[test]
fn vector_helper_embed_query_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<VectorHelperEmbedQueryResponse>("vector-embed-query-response.v1.valid.json");
}
