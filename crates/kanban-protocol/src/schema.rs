#![doc = include_str!("../docs/schema.md")]

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::{ContractDirection, ContractStrictness};

pub const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

#[derive(Debug, Clone, Copy)]
pub struct SchemaRoot {
    pub id: &'static str,
    pub artifact_path: &'static str,
    pub title: &'static str,
    pub contract_id: &'static str,
    pub direction: ContractDirection,
    pub strictness: ContractStrictness,
    pub valid_fixture: &'static str,
    pub invalid_fixture: &'static str,
    pub(crate) generate: fn(ContractDirection) -> Value,
}

pub fn generated_schema_ids() -> Vec<&'static str> {
    schema_registry().iter().map(|root| root.id).collect()
}

pub fn schema_registry() -> &'static [SchemaRoot] {
    static REGISTRY: std::sync::OnceLock<Vec<SchemaRoot>> = std::sync::OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            let mut registry = crate::CatalogProjection::new(
                crate::operation_catalog::operation_catalog(),
            )
            .schemas();
            let source_ids = registry
                .iter()
                .map(|root| root.contract_id)
                .collect::<std::collections::BTreeSet<_>>();
            for root in crate::metadata_config_catalog::schema_roots() {
                if !source_ids.contains(root.contract_id) {
                    registry.push(root);
                }
            }
            registry.extend(crate::admin_catalog::template_schema_roots());
            reorder_schema_roots(
                &mut registry,
                crate::board_catalog::HISTORICAL_SCHEMA_ORDER,
            );
            registry
        })
        .as_slice()
}

fn reorder_schema_roots(registry: &mut Vec<SchemaRoot>, order: &[&str]) {
    let mut ordered = Vec::with_capacity(registry.len());
    for contract_id in order {
        if let Some(index) = registry
            .iter()
            .position(|root| root.contract_id == *contract_id)
        {
            ordered.push(registry.remove(index));
        }
    }
    ordered.append(registry);
    *registry = ordered;
}

pub fn generated_artifacts() -> BTreeMap<String, Vec<u8>> {
    schema_registry()
        .iter()
        .map(|root| (root.artifact_path.to_owned(), schema_document_bytes(root)))
        .collect()
}

pub fn schema_document(root: &SchemaRoot) -> Value {
    let mut schema = (root.generate)(root.direction);
    let object = schema
        .as_object_mut()
        .expect("schemars root schema must be a JSON object");
    object.insert("$id".to_owned(), Value::String(root.id.to_owned()));
    object.insert("title".to_owned(), Value::String(root.title.to_owned()));
    canonicalize(schema)
}

pub fn schema_document_bytes(root: &SchemaRoot) -> Vec<u8> {
    let mut bytes =
        serde_json::to_vec_pretty(&schema_document(root)).expect("JSON Schema must serialize");
    bytes.push(b'\n');
    bytes
}

pub fn canonicalize(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.into_iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            let mut canonical = Map::new();
            for (key, value) in entries {
                canonical.insert(key, canonicalize(value));
            }
            Value::Object(canonical)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        scalar => scalar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_root_uses_explicit_draft_and_offline_id() {
        for root in schema_registry() {
            let schema = schema_document(root);
            assert_eq!(schema["$schema"], DRAFT_2020_12, "{}", root.id);
            assert_eq!(schema["$id"], root.id, "{}", root.id);
            assert!(root.id.starts_with("urn:kanban-tool:schema:"));
        }
    }

    #[test]
    fn generated_documents_only_contain_local_refs() {
        for root in schema_registry() {
            assert_local_refs(&schema_document(root), root.id);
        }
    }

    #[test]
    fn every_portable_input_key_is_required_and_fixture_nulls_remain_nullable() {
        let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

        for descriptor in crate::portable_contract_catalog() {
            let root = schema_registry()
                .iter()
                .find(|root| root.id == descriptor.input.schema_id)
                .unwrap_or_else(|| panic!("missing input root {}", descriptor.input.schema_id));
            let schema = schema_document(root);
            let data_schema = resolve_local_ref(&schema, &schema["properties"]["data"]);
            let fixture: Value = serde_json::from_slice(
                &std::fs::read(repository_root.join(root.valid_fixture))
                    .unwrap_or_else(|error| panic!("read {}: {error}", root.valid_fixture)),
            )
            .unwrap_or_else(|error| panic!("parse {}: {error}", root.valid_fixture));
            let data = fixture["data"]
                .as_object()
                .unwrap_or_else(|| panic!("{} must contain object data", root.valid_fixture));
            let required = data_schema["required"]
                .as_array()
                .unwrap_or_else(|| panic!("{} data schema must declare required", root.id))
                .iter()
                .map(|key| key.as_str().expect("required key must be a string"))
                .collect::<std::collections::BTreeSet<_>>();
            let fixture_keys = data
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                required, fixture_keys,
                "{} must require every data key",
                root.id
            );

            for (key, value) in data {
                if value.is_null() {
                    let property = resolve_local_ref(&schema, &data_schema["properties"][key]);
                    assert!(
                        schema_allows_null(&schema, property),
                        "{} data.{key} must accept explicit null: {property}",
                        root.id
                    );
                }
            }
        }
    }

    fn resolve_local_ref<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
        schema
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|reference| reference.strip_prefix('#'))
            .and_then(|pointer| root.pointer(pointer))
            .unwrap_or(schema)
    }

    fn schema_allows_null(root: &Value, schema: &Value) -> bool {
        let schema = resolve_local_ref(root, schema);
        schema.get("type").is_some_and(|types| {
            types == "null"
                || types
                    .as_array()
                    .is_some_and(|types| types.iter().any(|schema_type| schema_type == "null"))
        }) || ["anyOf", "oneOf"]
            .into_iter()
            .filter_map(|keyword| schema.get(keyword).and_then(Value::as_array))
            .flatten()
            .any(|branch| schema_allows_null(root, branch))
    }

    fn assert_local_refs(value: &Value, root_id: &str) {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                    assert!(
                        reference.starts_with("#/"),
                        "{root_id}: external ref {reference}"
                    );
                }
                for value in object.values() {
                    assert_local_refs(value, root_id);
                }
            }
            Value::Array(values) => {
                for value in values {
                    assert_local_refs(value, root_id);
                }
            }
            _ => {}
        }
    }
}
