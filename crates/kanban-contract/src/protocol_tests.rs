//! Schema and fixture tests for config/helper protocol contracts.

use crate::{
    GraphHelperErrorResponse, GraphHelperHandshakeResponse, GraphHelperNeighborsResponse,
    GraphHelperQueryResponse, GraphHelperRebuildResponse, GraphHelperStatusResponse,
    GraphHelperSyncResponse, ProjectConfigInput, VectorHelperCheckProviderResponse,
    VectorHelperEmbedQueryResponse, VectorHelperErrorResponse, VectorHelperHandshakeResponse,
    VectorHelperLabelAtomsStatusResponse, VectorHelperQueryChunksResponse,
    VectorHelperQueryLabelAtomsResponse, VectorHelperRebuildLabelAtomsResponse,
    VectorHelperRebuildResponse, VectorHelperStatusResponse, VectorHelperSyncLabelAtomsResponse,
    VectorHelperSyncResponse, WorkerProfilesInput,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};

fn fixture(relative: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    serde_json::from_str(
        &std::fs::read_to_string(root.join("schemas/fixtures").join(relative)).unwrap(),
    )
    .unwrap()
}

fn assert_exact<T: DeserializeOwned>(relative: &str) {
    let value = fixture(relative);
    serde_json::from_value::<T>(value.clone()).unwrap();

    let mut hostile = value;
    match &mut hostile {
        Value::Object(object) => {
            object.insert("unexpected".to_owned(), json!(true));
        }
        Value::Array(items) => {
            if let Some(object) = items[0].as_object_mut() {
                object.insert("unexpected".to_owned(), json!(true));
            } else {
                items.push(json!({"unexpected": true}));
            }
        }
        _ => panic!("fixture root must be an object or non-empty object array"),
    }
    assert!(serde_json::from_value::<T>(hostile).is_err(), "{relative}");
}

#[test]
fn config_protocol_fixtures_are_exact() {
    assert_exact::<ProjectConfigInput>("config/project-input.v1.valid.json");
    assert_exact::<WorkerProfilesInput>("config/worker-profiles-input.v1.valid.json");
}

#[test]
fn graph_helper_protocol_fixtures_are_exact() {
    assert_exact::<GraphHelperHandshakeResponse>("helper/graph-handshake-response.v1.valid.json");
    assert_exact::<GraphHelperErrorResponse>("helper/graph-error-response.v1.valid.json");
    assert_exact::<GraphHelperStatusResponse>("helper/graph-status-response.v1.valid.json");
    assert_exact::<GraphHelperRebuildResponse>("helper/graph-rebuild-response.v1.valid.json");
    assert_exact::<GraphHelperSyncResponse>("helper/graph-sync-response.v1.valid.json");
    assert_exact::<GraphHelperNeighborsResponse>("helper/graph-neighbors-response.v1.valid.json");
    assert_exact::<GraphHelperQueryResponse>("helper/graph-query-response.v1.valid.json");
}

#[test]
fn vector_helper_protocol_fixtures_are_exact() {
    assert_exact::<VectorHelperHandshakeResponse>("helper/vector-handshake-response.v1.valid.json");
    assert_exact::<VectorHelperErrorResponse>("helper/vector-error-response.v1.valid.json");
    assert_exact::<VectorHelperCheckProviderResponse>(
        "helper/vector-check-provider-response.v1.valid.json",
    );
    assert_exact::<VectorHelperStatusResponse>("helper/vector-status-response.v1.valid.json");
    assert_exact::<VectorHelperRebuildResponse>("helper/vector-rebuild-response.v1.valid.json");
    assert_exact::<VectorHelperSyncResponse>("helper/vector-sync-response.v1.valid.json");
    assert_exact::<VectorHelperLabelAtomsStatusResponse>(
        "helper/vector-label-atoms-status-response.v1.valid.json",
    );
    assert_exact::<VectorHelperRebuildLabelAtomsResponse>(
        "helper/vector-rebuild-label-atoms-response.v1.valid.json",
    );
    assert_exact::<VectorHelperSyncLabelAtomsResponse>(
        "helper/vector-sync-label-atoms-response.v1.valid.json",
    );
    assert_exact::<VectorHelperQueryChunksResponse>(
        "helper/vector-query-chunks-response.v1.valid.json",
    );
    assert_exact::<VectorHelperQueryLabelAtomsResponse>(
        "helper/vector-query-label-atoms-response.v1.valid.json",
    );
    assert_exact::<VectorHelperEmbedQueryResponse>(
        "helper/vector-embed-query-response.v1.valid.json",
    );
}

fn collect_schema_types(root: &Value, schema: &Value, output: &mut BTreeSet<String>) {
    if let Some(reference) = schema["$ref"].as_str()
        && let Some(pointer) = reference.strip_prefix('#')
    {
        collect_schema_types(
            root,
            root.pointer(pointer).expect("local schema ref"),
            output,
        );
    }
    match &schema["type"] {
        Value::String(kind) => {
            output.insert(kind.clone());
        }
        Value::Array(kinds) => {
            output.extend(kinds.iter().filter_map(Value::as_str).map(str::to_owned))
        }
        _ => {}
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(branches) = schema[keyword].as_array() {
            for branch in branches {
                collect_schema_types(root, branch, output);
            }
        }
    }
}

fn assert_required_nullable(root: &Value, schema: &Value, field: &str) {
    let required = schema["required"]
        .as_array()
        .expect("object schema must publish required keys");
    assert!(required.iter().any(|key| key == field), "{field}");

    let property = &schema["properties"][field];
    let mut types = BTreeSet::new();
    collect_schema_types(root, property, &mut types);
    assert!(types.contains("null"), "{field}: {types:?}");
    assert!(
        types.iter().any(|kind| kind != "null"),
        "{field}: {types:?}"
    );
}

#[test]
fn helper_required_nullable_schemas_accept_null_and_reject_missing() {
    let provenance =
        serde_json::to_value(schemars::schema_for!(crate::GraphHelperRelationProvenance)).unwrap();
    for field in ["source_table", "source_id", "source_event_id"] {
        assert_required_nullable(&provenance, &provenance, field);
    }

    let status = serde_json::to_value(schemars::schema_for!(VectorHelperStatusResponse)).unwrap();
    for field in ["dirty", "board_dirty"] {
        assert_required_nullable(&status, &status, field);
    }

    let chunk = serde_json::to_value(schemars::schema_for!(crate::VectorHelperChunkHit)).unwrap();
    for field in ["text", "summary"] {
        assert_required_nullable(&chunk, &chunk, field);
    }
    let chunk_ref = &chunk["$defs"]["VectorHelperChunkRef"];
    assert_required_nullable(&chunk, chunk_ref, "content_hash");

    let vector_hit =
        serde_json::to_value(schemars::schema_for!(crate::VectorHelperLabelAtomVectorHit)).unwrap();
    assert_required_nullable(&vector_hit, &vector_hit, "vector");

    assert!(
        serde_json::from_value::<VectorHelperStatusResponse>(json!({
            "backend": "lancedb",
            "enabled": true,
            "message": "ready",
            "diagnostics": []
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<VectorHelperStatusResponse>(json!({
            "backend": "lancedb",
            "enabled": true,
            "message": "ready",
            "diagnostics": [],
            "dirty": null,
            "board_dirty": null
        }))
        .is_ok()
    );
}
