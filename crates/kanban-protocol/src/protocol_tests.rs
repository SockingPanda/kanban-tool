//! Schema and fixture tests for configuration contracts.

use crate::{ProjectConfigInput, WorkerProfileInput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::path::Path;

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
