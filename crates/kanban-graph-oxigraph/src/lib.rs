mod oxigraph_backend;

use kanban_entity::Relation;
use kanban_graph::{GraphQueryRow, GraphStoreStatus};
use kanban_protocol::{
    GraphHelperErrorResponse, GraphHelperHandshakeResponse, GraphHelperNeighborsResponse,
    GraphHelperQueryBinding, GraphHelperQueryResponse, GraphHelperQueryRow, GraphHelperRelation,
    GraphHelperRelationProvenance, GraphHelperStatusResponse,
};

pub use oxigraph_backend::OxigraphStore;

pub const GRAPH_HELPER_BUILD_IDENTITY: &str = match option_env!("KANBAN_BUILD_ID") {
    Some(build_id) => build_id,
    None => concat!(
        "dev:",
        env!("CARGO_PKG_NAME"),
        "@",
        env!("CARGO_PKG_VERSION")
    ),
};

pub const fn graph_helper_build_identity() -> &'static str {
    GRAPH_HELPER_BUILD_IDENTITY
}

pub fn graph_helper_handshake_response(version: &str) -> GraphHelperHandshakeResponse {
    GraphHelperHandshakeResponse {
        helper: "kanban-graph-oxigraph".to_owned(),
        protocol: kanban_helper_protocol::HelperEnvelope::PROTOCOL.to_owned(),
        version: version.to_owned(),
    }
}

pub fn graph_helper_error_response(message: impl Into<String>) -> GraphHelperErrorResponse {
    GraphHelperErrorResponse {
        code: "helper_error".to_owned(),
        message: message.into(),
    }
}

pub fn graph_helper_status_response(status: GraphStoreStatus) -> GraphHelperStatusResponse {
    GraphHelperStatusResponse {
        backend: status.backend,
        enabled: status.enabled,
        message: status.message,
    }
}

pub fn graph_helper_neighbors_response(
    relations: Vec<Relation>,
) -> Result<GraphHelperNeighborsResponse, serde_json::Error> {
    relations
        .into_iter()
        .map(|relation| {
            Ok(GraphHelperRelation {
                subject_uri: relation.subject_uri.to_string(),
                predicate: relation.predicate.to_string(),
                object_uri: relation.object_uri.to_string(),
                graph_uri: relation.graph_uri.to_string(),
                provenance: GraphHelperRelationProvenance {
                    source_table: relation.provenance.source_table,
                    source_id: relation.provenance.source_id,
                    source_event_id: relation.provenance.source_event_id,
                    authoritative_store: relation.provenance.authoritative_store,
                },
                metadata: serde_json::from_str(&relation.metadata_json)?,
                created_at: relation.created_at,
                updated_at: relation.updated_at,
            })
        })
        .collect()
}

pub fn graph_helper_query_response(rows: Vec<GraphQueryRow>) -> GraphHelperQueryResponse {
    rows.into_iter()
        .map(|row| GraphHelperQueryRow {
            bindings: row
                .bindings
                .into_iter()
                .map(|binding| GraphHelperQueryBinding {
                    name: binding.name,
                    value: binding.value,
                })
                .collect(),
        })
        .collect()
}
