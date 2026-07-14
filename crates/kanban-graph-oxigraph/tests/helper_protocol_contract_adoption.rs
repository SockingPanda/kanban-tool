use kanban_contract::{
    GraphHelperErrorResponse, GraphHelperHandshakeResponse, GraphHelperNeighborsResponse,
    GraphHelperQueryResponse, GraphHelperRebuildResponse, GraphHelperStatusResponse,
    GraphHelperSyncResponse,
};
use kanban_entity::{EntityUri, Predicate, Provenance, Relation};
use kanban_graph::{GraphQueryBinding, GraphQueryRow, GraphStoreStatus};
use kanban_graph_oxigraph::{
    graph_helper_error_response, graph_helper_handshake_response, graph_helper_neighbors_response,
    graph_helper_query_response, graph_helper_status_response,
};
use kanban_helper_protocol::HelperEnvelope;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::path::Path;

fn fixture(relative: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    serde_json::from_str(
        &std::fs::read_to_string(root.join("schemas/fixtures/helper").join(relative)).unwrap(),
    )
    .unwrap()
}

fn assert_produced<T: Serialize>(relative: &str, actual: T) {
    assert_eq!(serde_json::to_value(actual).unwrap(), fixture(relative));
}

fn assert_consumed<T: Serialize + DeserializeOwned>(relative: &str) {
    let envelope = HelperEnvelope::new(fixture(relative)).unwrap();
    let decoded: T = envelope.decode().unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), fixture(relative));
}

fn status(message: &str) -> GraphStoreStatus {
    GraphStoreStatus {
        backend: "oxigraph".to_owned(),
        enabled: true,
        message: message.to_owned(),
    }
}

#[test]
fn graph_helper_handshake_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "graph-handshake-response.v1.valid.json",
        graph_helper_handshake_response(env!("CARGO_PKG_VERSION")),
    );
}

#[test]
fn graph_helper_handshake_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<GraphHelperHandshakeResponse>("graph-handshake-response.v1.valid.json");
}

#[test]
fn graph_helper_error_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "graph-error-response.v1.valid.json",
        graph_helper_error_response("graph fixture failure"),
    );
}

#[test]
fn graph_helper_error_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<GraphHelperErrorResponse>("graph-error-response.v1.valid.json");
}

#[test]
fn graph_helper_status_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "graph-status-response.v1.valid.json",
        graph_helper_status_response(status("graph ready")),
    );
}

#[test]
fn graph_helper_status_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<GraphHelperStatusResponse>("graph-status-response.v1.valid.json");
}

#[test]
fn graph_helper_rebuild_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "graph-rebuild-response.v1.valid.json",
        graph_helper_status_response(status("graph rebuilt")),
    );
}

#[test]
fn graph_helper_rebuild_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<GraphHelperRebuildResponse>("graph-rebuild-response.v1.valid.json");
}

#[test]
fn graph_helper_sync_response_fixture_is_produced_by_real_helper_adapter() {
    assert_produced(
        "graph-sync-response.v1.valid.json",
        graph_helper_status_response(status("graph synced")),
    );
}

#[test]
fn graph_helper_sync_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<GraphHelperSyncResponse>("graph-sync-response.v1.valid.json");
}

#[test]
fn graph_helper_neighbors_response_fixture_is_produced_by_real_helper_adapter() {
    let relation = Relation {
        subject_uri: EntityUri::new("kb://task/t_child").unwrap(),
        predicate: Predicate::DependsOn,
        object_uri: EntityUri::new("kb://task/t_parent").unwrap(),
        graph_uri: EntityUri::new("kb://graph/relations").unwrap(),
        provenance: Provenance {
            source_table: Some("task_dependencies".to_owned()),
            source_id: Some("t_parent->t_child".to_owned()),
            source_event_id: Some(12),
            authoritative_store: "sqlite".to_owned(),
        },
        metadata_json: "{}".to_owned(),
        created_at: 100,
        updated_at: 101,
    };
    assert_produced(
        "graph-neighbors-response.v1.valid.json",
        graph_helper_neighbors_response(vec![relation]).expect("natural metadata JSON"),
    );
}

#[test]
fn graph_helper_neighbors_response_rejects_malformed_persisted_metadata() {
    let relation = Relation {
        subject_uri: EntityUri::new("kb://task/t_child").unwrap(),
        predicate: Predicate::DependsOn,
        object_uri: EntityUri::new("kb://task/t_parent").unwrap(),
        graph_uri: EntityUri::new("kb://graph/relations").unwrap(),
        provenance: Provenance {
            source_table: None,
            source_id: None,
            source_event_id: None,
            authoritative_store: "sqlite".to_owned(),
        },
        metadata_json: "{".to_owned(),
        created_at: 100,
        updated_at: 101,
    };

    assert!(graph_helper_neighbors_response(vec![relation]).is_err());
}

#[test]
fn graph_helper_neighbors_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<GraphHelperNeighborsResponse>("graph-neighbors-response.v1.valid.json");
}

#[test]
fn graph_helper_query_response_fixture_is_produced_by_real_helper_adapter() {
    let row = GraphQueryRow {
        bindings: vec![
            GraphQueryBinding {
                name: "task".to_owned(),
                value: "kb://task/t_child".to_owned(),
            },
            GraphQueryBinding {
                name: "parent".to_owned(),
                value: "kb://task/t_parent".to_owned(),
            },
        ],
    };
    assert_produced(
        "graph-query-response.v1.valid.json",
        graph_helper_query_response(vec![row]),
    );
}

#[test]
fn graph_helper_query_response_fixture_is_consumed_by_runtime_protocol_decoder() {
    assert_consumed::<GraphHelperQueryResponse>("graph-query-response.v1.valid.json");
}
