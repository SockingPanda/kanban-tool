//! Schema and fixture tests for config/helper protocol contracts.

use crate::{
    ProjectConfigInput, ProjectionArtifactManifest, ProjectionBatchReceipt, ProjectionDelivery,
    ProjectionDeliveryAction, ProjectionPublishReceipt, VectorHelperCheckProviderResponse,
    VectorHelperEmbedQueryResponse, VectorHelperErrorResponse, VectorHelperHandshakeResponse,
    VectorHelperLabelAtomsStatusResponse, VectorHelperQueryChunksResponse,
    VectorHelperQueryLabelAtomsResponse, VectorHelperRebuildLabelAtomsResponse,
    VectorHelperRebuildResponse, VectorHelperStatusResponse, VectorHelperSyncLabelAtomsResponse,
    VectorHelperSyncResponse, VectorProjectionHelperRequest, VectorProjectionHelperResponse,
    WorkerProfileInput,
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
    assert_exact::<WorkerProfileInput>("config/selected-worker-profile-input.v1.valid.json");
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

#[test]
fn vector_projection_helper_protocol_fixtures_are_exact() {
    assert_exact::<VectorProjectionHelperRequest>("helper/vector-projection-request.v2.valid.json");
    assert_exact::<VectorProjectionHelperResponse>(
        "helper/vector-projection-response.v1.valid.json",
    );
}

#[test]
fn vector_projection_helper_protocol_rejects_wrong_variants() {
    assert!(
        serde_json::from_value::<VectorProjectionHelperRequest>(json!({
            "operation": "descriptor",
            "payload": {
                "request_id": "req_fixture_descriptor",
                "batch": {}
            }
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<VectorProjectionHelperResponse>(json!({
            "operation": "inspect_active",
            "payload": {
                "request_id": "req_fixture_inspect",
                "receipt": {}
            }
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<VectorProjectionHelperResponse>(json!({
            "operation": "error",
            "payload": {
                "kind": "unknown",
                "code": "unknown",
                "provider": null,
                "backend": null,
                "retryable": false,
                "message": "unknown kind",
                "request_id": null,
                "delivery_digest": null,
                "projection_store": null,
                "generation_id": null
            }
        }))
        .is_err()
    );
}

#[test]
fn vector_projection_destructive_authority_is_required() {
    let context = json!({
        "request_id": "req_destructive_authority_required",
        "projection_store": "lancedb_chunks",
        "generation_id": "gen_fixture_active",
        "delivery_digest": "sha256:fixture-delivery-digest"
    });
    let requests = [
        json!({
            "operation": "quarantine",
            "payload": {
                "context": context
            }
        }),
        json!({
            "operation": "abort",
            "payload": {
                "context": context
            }
        }),
        json!({
            "operation": "cleanup",
            "payload": {
                "context": context,
                "dry_run": false,
                "protection": {
                    "active_generation": "gen_fixture_active",
                    "previous_generation": "gen_fixture_previous",
                    "building_generation": null,
                    "additional_generations": []
                }
            }
        }),
    ];

    for request in requests {
        assert!(
            serde_json::from_value::<VectorProjectionHelperRequest>(request).is_err(),
            "destructive request without authority must be rejected"
        );
    }
}

#[test]
fn vector_projection_destructive_authority_debug_redacts_lease_token() {
    let mut manifest = projection_manifest_json();
    manifest["generation"] = json!("gen_fixture_active");
    manifest["fingerprint"] = json!("sha256:fixture-generation-fingerprint");
    let request = serde_json::from_value::<VectorProjectionHelperRequest>(json!({
        "operation": "quarantine",
        "payload": {
            "context": {
                "request_id": "req_destructive_authority_debug",
                "projection_store": "lancedb_chunks",
                "generation_id": "gen_fixture_active",
                "delivery_digest": "sha256:delivery"
            },
            "authority": {
                "owner": "fixture-maintenance-owner",
                "lease_token": "fixture-destructive-lease-token-secret",
                "fence_epoch": 11,
                "role": "active",
                "generation": "gen_fixture_active",
                "expected_manifest": manifest,
                "expected_binding": {
                    "generation": "gen_fixture_active",
                    "fingerprint": "sha256:fixture-generation-fingerprint",
                    "fence_epoch": 7,
                    "snapshot_cursor": 11,
                    "provider": "ollama",
                    "provider_fingerprint": "sha256:provider",
                    "canonical_count": 1,
                    "canonical_digest": "sha256:canonical",
                    "delivery_count": 1,
                    "delivery_digest": "sha256:delivery",
                    "corpus": {
                        "corpus_schema": "task-chunks-v2",
                        "corpus_fingerprint": "sha256:corpus",
                        "embedding_model": "fixture-model",
                        "embedding_dimensions": 3
                    }
                },
                "building_phase": null
            }
        }
    }))
    .expect("destructive request with authority must decode");

    let rendered = format!("{request:?}");
    assert!(!rendered.contains("fixture-destructive-lease-token-secret"));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn vector_projection_orphaned_authority_accepts_explicit_null_evidence() {
    let request = json!({
        "operation": "quarantine",
        "payload": {
            "context": {
                "request_id": "req_orphaned_authority",
                "projection_store": "lancedb_chunks",
                "generation_id": "orphaned-entry",
                "delivery_digest": "sha256:orphaned-correlation"
            },
            "authority": {
                "owner": "fixture-maintenance-owner",
                "lease_token": "fixture-orphaned-lease-token",
                "fence_epoch": 11,
                "role": "orphaned",
                "generation": "orphaned-entry",
                "expected_manifest": null,
                "expected_binding": null,
                "building_phase": null
            }
        }
    });
    serde_json::from_value::<VectorProjectionHelperRequest>(request.clone())
        .expect("orphaned authority must represent absent physical evidence explicitly");

    let mut missing_binding = request;
    missing_binding["payload"]["authority"]
        .as_object_mut()
        .unwrap()
        .remove("expected_binding");
    assert!(
        serde_json::from_value::<VectorProjectionHelperRequest>(missing_binding).is_err(),
        "required-nullable expected_binding must not be omitted"
    );
}

fn projection_manifest_json() -> Value {
    json!({
        "store_name": "lancedb_chunks",
        "database_instance_id": "db_fixture",
        "protocol_version": 2,
        "schema_version": 2,
        "generation": "gen_fixture",
        "fence_epoch": 7,
        "snapshot_cursor": 11,
        "provider": "ollama",
        "provider_fingerprint": "sha256:provider",
        "corpus": {
            "corpus_schema": "task-chunks-v2",
            "corpus_fingerprint": "sha256:corpus",
            "embedding_model": "fixture-model",
            "embedding_dimensions": 3
        },
        "canonical_item_count": 1,
        "canonical_digest": "sha256:canonical",
        "delivery_item_count": 1,
        "delivery_digest": "sha256:delivery",
        "fingerprint": null
    })
}

#[test]
fn vector_projection_nullable_evidence_fields_are_required_but_accept_null() {
    let manifest = projection_manifest_json();
    serde_json::from_value::<ProjectionArtifactManifest>(manifest.clone()).unwrap();
    for field in ["corpus", "fingerprint"] {
        let mut missing = manifest.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<ProjectionArtifactManifest>(missing).is_err(),
            "{field}"
        );
    }

    let delivery = json!({
        "id": 1,
        "outbox_id": 1,
        "store_name": "lancedb_chunks",
        "generation_id": "gen_fixture",
        "board_id": "b_fixture",
        "source_event_id": null,
        "cursor": 1,
        "action": "delete",
        "entity_uri": "kb://task/t_fixture",
        "payload_json": "{}",
        "attempts": 1
    });
    serde_json::from_value::<ProjectionDelivery>(delivery.clone()).unwrap();
    let mut missing_source_event = delivery;
    missing_source_event
        .as_object_mut()
        .unwrap()
        .remove("source_event_id");
    assert!(serde_json::from_value::<ProjectionDelivery>(missing_source_event).is_err());
    let mut unknown_action = json!({
        "id": 1,
        "outbox_id": 1,
        "store_name": "lancedb_chunks",
        "generation_id": "gen_fixture",
        "board_id": "b_fixture",
        "source_event_id": null,
        "cursor": 1,
        "action": "reindex",
        "entity_uri": "kb://task/t_fixture",
        "payload_json": "{}",
        "attempts": 1
    });
    assert!(serde_json::from_value::<ProjectionDelivery>(unknown_action.clone()).is_err());
    unknown_action["action"] = json!("rebuild");
    let delivery = serde_json::from_value::<ProjectionDelivery>(unknown_action).unwrap();
    assert_eq!(delivery.action, ProjectionDeliveryAction::Rebuild);

    let receipt = json!({
        "active": {
            "manifest": manifest,
            "fingerprint": "sha256:generation"
        },
        "retained_previous": null
    });
    serde_json::from_value::<ProjectionPublishReceipt>(receipt.clone()).unwrap();
    let mut missing_previous = receipt;
    missing_previous
        .as_object_mut()
        .unwrap()
        .remove("retained_previous");
    assert!(serde_json::from_value::<ProjectionPublishReceipt>(missing_previous).is_err());
}

#[test]
fn vector_projection_debug_redacts_capability_tokens() {
    let request = serde_json::from_value::<VectorProjectionHelperRequest>(json!({
        "operation": "apply_batch",
        "payload": {
            "context": {
                "request_id": "req_fixture_apply_batch",
                "projection_store": "lancedb_chunks",
                "generation_id": "gen_fixture_chunks",
                "delivery_digest": "sha256:fixture-delivery-digest"
            },
            "authority": {
                "owner": "fixture-maintenance-owner",
                "lease_token": "fixture-authority-lease-token-not-a-secret",
                "fence_epoch": 7,
                "role": "building",
                "generation": "gen_fixture_chunks",
                "expected_manifest": null,
                "expected_binding": {
                    "generation": "gen_fixture_chunks",
                    "fingerprint": null,
                    "fence_epoch": 7,
                    "snapshot_cursor": null,
                    "provider": "ollama",
                    "provider_fingerprint": "sha256:fixture-provider-fingerprint",
                    "canonical_count": 0,
                    "canonical_digest": "sha256:fixture-canonical-digest",
                    "delivery_count": 0,
                    "delivery_digest": "sha256:fixture-delivery-digest",
                    "corpus": {
                        "corpus_schema": "task-chunks-v2",
                        "corpus_fingerprint": "sha256:fixture-corpus-fingerprint",
                        "embedding_model": "fixture-model",
                        "embedding_dimensions": 3
                    }
                },
                "building_phase": "snapshotting"
            },
            "batch": {
                "store_name": "lancedb_chunks",
                "database_instance_id": "dbi_fixture_projection",
                "protocol_version": 2,
                "schema_version": 2,
                "provider": "ollama",
                "provider_fingerprint": "sha256:fixture-provider-fingerprint",
                "owner": "fixture-maintenance-owner",
                "lease_token": "fixture-lease-token-not-a-secret",
                "fence_epoch": 7,
                "target_generation": "gen_fixture_chunks",
                "claim_token": "fixture-claim-token-not-a-secret",
                "claim_expires_at": 4102444800000_i64,
                "items": [{
                    "id": 41,
                    "outbox_id": 17,
                    "store_name": "lancedb_chunks",
                    "generation_id": "gen_fixture_chunks",
                    "board_id": "b_fixture_board",
                    "source_event_id": 101,
                    "cursor": 101,
                    "action": "upsert",
                    "entity_uri": "kb://task/t_fixture",
                    "payload_json": "{\"projection_store\":\"lancedb_chunks\"}",
                    "attempts": 0
                }]
            }
        }
    }))
    .unwrap();
    let rendered = format!("{request:?}");
    assert!(!rendered.contains("fixture-lease-token-not-a-secret"));
    assert!(!rendered.contains("fixture-claim-token-not-a-secret"));
    assert!(rendered.contains("[REDACTED]"));

    let VectorProjectionHelperRequest::ApplyBatch(request) = request else {
        panic!("fixture must be an apply_batch request");
    };
    let receipt = ProjectionBatchReceipt {
        store_name: request.batch.store_name,
        database_instance_id: request.batch.database_instance_id,
        protocol_version: request.batch.protocol_version,
        schema_version: request.batch.schema_version,
        provider: request.batch.provider,
        provider_fingerprint: request.batch.provider_fingerprint,
        target_generation: request.batch.target_generation,
        lease_token: request.batch.lease_token,
        fence_epoch: request.batch.fence_epoch,
        claim_token: request.batch.claim_token,
        applied_item_count: request.batch.items.len(),
    };
    let rendered = format!("{receipt:?}");
    assert!(!rendered.contains("fixture-lease-token-not-a-secret"));
    assert!(!rendered.contains("fixture-claim-token-not-a-secret"));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn vector_projection_helper_stdout_never_contains_capability_fields() {
    let schema =
        serde_json::to_string(&schemars::schema_for!(VectorProjectionHelperResponse)).unwrap();
    let fixture = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("schemas/fixtures/helper/vector-projection-response.v1.valid.json"),
    )
    .unwrap();
    for forbidden in ["lease_token", "claim_token"] {
        assert!(!schema.contains(forbidden), "{forbidden}: {schema}");
        assert!(!fixture.contains(forbidden), "{forbidden}: {fixture}");
    }
}

#[test]
fn vector_projection_cleanup_requires_an_explicit_dry_run_decision() {
    let authority =
        fixture("helper/vector-projection-request.v2.valid.json")["payload"]["authority"].clone();
    let missing_dry_run = json!({
        "operation": "cleanup",
        "payload": {
            "context": {
                "request_id": "req_fixture_cleanup_default",
                "projection_store": "lancedb_chunks",
                "generation_id": "gen_fixture_active",
                "delivery_digest": "sha256:fixture-delivery-digest"
            },
            "authority": authority,
            "protection": {
                "active_generation": "gen_fixture_active",
                "previous_generation": "gen_fixture_previous",
                "building_generation": null,
                "additional_generations": []
            }
        }
    });
    assert!(
        serde_json::from_value::<VectorProjectionHelperRequest>(missing_dry_run.clone()).is_err()
    );
    let mut explicit_dry_run = missing_dry_run;
    explicit_dry_run["payload"]["dry_run"] = json!(true);
    let request =
        serde_json::from_value::<VectorProjectionHelperRequest>(explicit_dry_run).unwrap();

    assert!(matches!(
        request,
        VectorProjectionHelperRequest::Cleanup(crate::VectorProjectionCleanupRequest {
            dry_run: true,
            ..
        })
    ));
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
