use std::collections::HashSet;

use kanban_core::{KanbanError, Result};

pub(crate) fn normalize_comment_metadata_json(
    kind: &str,
    metadata_json: Option<&str>,
) -> Result<String> {
    let metadata_json = metadata_json
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("{}");
    let value = serde_json::from_str::<serde_json::Value>(metadata_json)
        .map_err(|_| KanbanError::InvalidInput("metadata_json must be valid JSON".into()))?;
    validate_comment_metadata_value(kind, &value)?;
    Ok(metadata_json.to_owned())
}

pub(crate) fn normalize_imported_comment_metadata_json(
    kind: &str,
    metadata_json: Option<&serde_json::Value>,
) -> Result<String> {
    let Some(metadata_json) = metadata_json else {
        return normalize_comment_metadata_json(kind, None);
    };
    match metadata_json {
        serde_json::Value::Null => normalize_comment_metadata_json(kind, None),
        serde_json::Value::Object(_) => {
            validate_comment_metadata_value(kind, metadata_json)?;
            Ok(metadata_json.to_string())
        }
        serde_json::Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return normalize_comment_metadata_json(kind, None);
            }
            normalize_comment_metadata_json(kind, Some(trimmed))
        }
        _ => Err(KanbanError::InvalidInput(
            "metadata_json must be a JSON object".into(),
        )),
    }
}

fn validate_comment_metadata_value(kind: &str, value: &serde_json::Value) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| KanbanError::InvalidInput("metadata_json must be a JSON object".into()))?;
    if kind == "decision" {
        validate_decision_metadata(object)?;
    }
    Ok(())
}

fn validate_decision_metadata(object: &serde_json::Map<String, serde_json::Value>) -> Result<()> {
    let options = object
        .get("options")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            KanbanError::InvalidInput("decision metadata options must be a non-empty array".into())
        })?;
    if options.is_empty() {
        return Err(KanbanError::InvalidInput(
            "decision metadata options must be a non-empty array".into(),
        ));
    }

    let mut slugs = HashSet::new();
    for option in options {
        let option = option.as_object().ok_or_else(|| {
            KanbanError::InvalidInput("decision metadata options must be objects".into())
        })?;
        let slug = required_non_empty_string(option, "slug")?;
        if !is_decision_slug(slug) {
            return Err(KanbanError::InvalidInput(
                "decision metadata option slug must be a lowercase ASCII slug".into(),
            ));
        }
        if !slugs.insert(slug) {
            return Err(KanbanError::InvalidInput(
                "decision metadata option slugs must be unique".into(),
            ));
        }
        required_non_empty_string(option, "title")?;
        required_non_empty_string(option, "detail")?;
    }

    let selected = required_non_empty_string(object, "selected")?;
    if !slugs.contains(selected) {
        return Err(KanbanError::InvalidInput(
            "decision metadata selected must match an option slug".into(),
        ));
    }
    required_non_empty_string(object, "reason")?;
    optional_non_empty_string(object, "risk")?;
    optional_non_empty_string(object, "verification")?;
    Ok(())
}

fn required_non_empty_string<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<&'a str> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            KanbanError::InvalidInput(format!(
                "decision metadata {field} must be a non-empty string"
            ))
        })
}

fn optional_non_empty_string(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<()> {
    match object.get(field) {
        Some(value)
            if value
                .as_str()
                .map(str::trim)
                .is_none_or(|value| value.is_empty()) =>
        {
            Err(KanbanError::InvalidInput(format!(
                "decision metadata {field} must be a non-empty string"
            )))
        }
        _ => Ok(()),
    }
}

fn is_decision_slug(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}
