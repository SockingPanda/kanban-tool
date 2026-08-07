use std::{path::Path, process::Command};

use serde_json::Value;

#[test]
fn committed_schema_tree_and_fixtures_match_registry() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate must live below workspace root");

    xtask::check_contract(repo_root, false)
        .expect("committed schema contract should match fresh generation");
}

#[test]
fn generated_artifact_set_is_byte_deterministic() {
    let first = xtask::expected_artifacts().expect("first generation should succeed");
    let second = xtask::expected_artifacts().expect("second generation should succeed");

    assert_eq!(first, second);
    assert!(first.contains_key("manifest.json"));
    assert!(first.contains_key("operations.json"));
    assert!(first.contains_key("surface-operations.json"));
}

#[test]
fn every_root_is_self_contained_and_has_a_complete_fixture_pair() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate must live below workspace root");

    for root in kanban_protocol::schema::schema_registry() {
        assert!(root.valid_fixture.starts_with("schemas/fixtures/"));
        assert!(root.invalid_fixture.starts_with("schemas/fixtures/"));
        assert_ne!(root.valid_fixture, root.invalid_fixture);
        assert!(
            repo_root.join(root.valid_fixture).is_file(),
            "missing positive fixture for {}",
            root.id
        );
        assert!(
            repo_root.join(root.invalid_fixture).is_file(),
            "missing negative fixture for {}",
            root.id
        );

        let artifact_path = repo_root
            .join(xtask::ARTIFACT_DIRECTORY)
            .join(root.artifact_path);
        let schema: Value =
            serde_json::from_slice(&std::fs::read(&artifact_path).unwrap_or_else(|error| {
                panic!("cannot read {}: {error}", artifact_path.display())
            }))
            .unwrap_or_else(|error| panic!("{} is not JSON: {error}", artifact_path.display()));
        assert_eq!(schema.get("$id"), Some(&Value::String(root.id.to_owned())));
        assert_eq!(
            schema.get("$schema"),
            Some(&Value::String(
                kanban_protocol::schema::DRAFT_2020_12.to_owned()
            ))
        );
        assert_local_references(&schema, root.id);
    }
}

fn assert_local_references(value: &Value, root_id: &str) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref") {
                let reference = reference
                    .as_str()
                    .unwrap_or_else(|| panic!("{root_id} contains a non-string $ref"));
                assert!(
                    reference.starts_with("#/$defs/"),
                    "{root_id} contains non-local $ref {reference}"
                );
            }
            for child in object.values() {
                assert_local_references(child, root_id);
            }
        }
        Value::Array(array) => {
            for child in array {
                assert_local_references(child, root_id);
            }
        }
        _ => {}
    }
}

#[test]
fn binary_help_preserves_public_cli_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("--help")
        .output()
        .expect("schema tool binary should execute");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("help output must be UTF-8"),
        "用法：xtask <affected plan|json|run|self-test|docs check|schema generate|check|audit|witnesses|deps check|agents check|tooling check|package cli> [--base REF] [--root PATH] [--require-closed]\n"
    );
}

#[test]
fn witnesses_json_matches_canonical_inventory_order_and_format() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["schema", "witnesses"])
        .output()
        .expect("schema tool binary should execute");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let adopted = kanban_protocol::operation_inventory()
        .iter()
        .filter(|operation| operation.migration == kanban_protocol::MigrationState::Adopted)
        .collect::<Vec<_>>();
    let expected = format!("{}\n", serde_json::to_string_pretty(&adopted).unwrap());
    assert_eq!(
        String::from_utf8(output.stdout).expect("witness output must be UTF-8"),
        expected
    );
}

#[test]
fn closure_audit_accepts_closed_inventory() {
    xtask::audit_inventory(true).expect("contract train must be closed");
    assert_eq!(xtask::unfinished_contract_count(), 0);
}

#[test]
fn decision_schema_accepts_missing_but_rejects_explicit_null() {
    let root = kanban_protocol::schema_registry()
        .iter()
        .find(|root| root.contract_id == "metadata.decision.input")
        .expect("decision root must exist");
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&kanban_protocol::schema::schema_document(root))
        .expect("decision schema must compile");
    let missing = serde_json::json!({
        "options": [{
            "slug": "typed-open",
            "title": "Typed open contract",
            "detail": "Known fields stay typed."
        }],
        "selected": "typed-open",
        "reason": "missing is valid"
    });
    assert!(validator.is_valid(&missing));

    for field in ["risk", "verification"] {
        let mut explicit_null = missing.clone();
        explicit_null
            .as_object_mut()
            .expect("fixture is an object")
            .insert(field.to_owned(), serde_json::Value::Null);
        assert!(
            !validator.is_valid(&explicit_null),
            "{field}=null 必须被 JSON Schema 拒绝"
        );
    }
}

#[test]
fn data_envelope_meta_rejects_untyped_value() {
    let payload = serde_json::json!({"data": {"ok": true}, "meta": {"arbitrary": true}});
    assert!(
        serde_json::from_value::<kanban_protocol::DataEnvelope<serde_json::Value>>(payload)
            .is_err()
    );
}
