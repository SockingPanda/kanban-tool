//! SQLite JSON-text to natural structured metadata adapters.

use serde_json::Value;

use kanban_core::{KanbanError, Result};

const NATURAL_FIELDS: &[(&str, &str)] = &[
    ("evidence_json", "evidence"),
    ("related_labels_json", "related_labels"),
    ("proposal_json", "proposal"),
    ("change_json", "change"),
    ("validation_json", "validation"),
    ("task_snapshot_json", "task_snapshot"),
    ("agent_candidates_json", "agent_candidates"),
    ("suggestion_snapshot_json", "suggestion_snapshot"),
    ("final_decision_json", "final_decision"),
    ("diagnostics_json", "diagnostics"),
];

/// Converts canonical SQLite JSON-text columns into public natural JSON fields.
pub fn naturalize_structured_metadata(value: &mut Value) -> Result<()> {
    match value {
        Value::Array(values) => {
            for value in values {
                naturalize_structured_metadata(value)?;
            }
        }
        Value::Object(object) => {
            for (stored, natural) in NATURAL_FIELDS {
                let Some(raw) = object.remove(*stored) else {
                    continue;
                };
                if object.contains_key(*natural) {
                    return Err(KanbanError::Storage(format!(
                        "structured metadata contains both {stored} and {natural}"
                    )));
                }
                let Value::String(raw) = raw else {
                    return Err(KanbanError::Storage(format!(
                        "stored {stored} must be JSON text"
                    )));
                };
                let parsed = serde_json::from_str(&raw).map_err(|error| {
                    KanbanError::Storage(format!("stored {stored} is invalid JSON: {error}"))
                })?;
                object.insert((*natural).to_owned(), parsed);
            }
            for key in [
                "data",
                "signal",
                "observation",
                "signals",
                "action",
                "actions",
                "provenance_actions",
                "supporting_signals",
                "validation_history",
            ] {
                if let Some(value) = object.get_mut(key) {
                    naturalize_structured_metadata(value)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn naturalizer_removes_double_encoded_fields_recursively() {
        let mut value = json!({
            "observation": {"evidence_json": "{\"source\":\"cli\"}"},
            "signals": [{"related_labels_json": "[]", "proposal_json": "{}"}],
            "actions": [{"change_json": "{\"kind\":\"add\"}", "validation_json": "{}"}]
        });
        naturalize_structured_metadata(&mut value).unwrap();
        assert_eq!(value["observation"]["evidence"]["source"], "cli");
        assert_eq!(value["signals"][0]["related_labels"], json!([]));
        assert_eq!(value["signals"][0]["proposal"], json!({}));
        assert_eq!(value["actions"][0]["change"]["kind"], "add");
        assert!(value.to_string().find("_json").is_none());
    }

    #[test]
    fn naturalizer_fails_closed_on_invalid_stored_json() {
        let mut value = json!({"evidence_json": "{invalid"});
        assert!(naturalize_structured_metadata(&mut value).is_err());
    }

    #[test]
    fn naturalizer_preserves_same_named_keys_inside_user_json() {
        let mut value = json!({
            "data": {
                "observation": {
                    "evidence_json": "{\"change_json\":\"user-owned\",\"nested\":{\"proposal_json\":\"also-user-owned\"}}"
                },
                "extension": {"validation_json": "not-a-storage-column"}
            }
        });
        naturalize_structured_metadata(&mut value).unwrap();
        assert_eq!(
            value["data"]["observation"]["evidence"]["change_json"],
            "user-owned"
        );
        assert_eq!(
            value["data"]["observation"]["evidence"]["nested"]["proposal_json"],
            "also-user-owned"
        );
        assert_eq!(
            value["data"]["extension"]["validation_json"],
            "not-a-storage-column"
        );
    }
}
