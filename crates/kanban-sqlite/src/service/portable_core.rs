//! Core portable JSONL row adapters.

use kanban_contract::jsonl_core::{
    AttachmentJsonlData, BoardJsonlData, ColumnJsonlData, CommentJsonlData, DependencyJsonlData,
    EventJsonlData, RunJsonlData, TaskJsonlData, TaskLabelJsonlData,
};
use kanban_core::{KanbanError, Result};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

const DISCRIMINATORS: &[&str] = &[
    "board",
    "column",
    "task",
    "dependency",
    "run",
    "comment",
    "event",
    "attachment",
    "task_label",
];

pub(crate) fn encode_record(
    discriminator: &str,
    mut data: Map<String, Value>,
) -> Result<Map<String, Value>> {
    ensure_owned(discriminator)?;
    // The central catalog's ownership probe intentionally routes an empty map
    // through every lane. Real export rows are never empty.
    if data.is_empty() {
        return Ok(data);
    }
    match discriminator {
        "task" => {
            json_text_to_wire(&mut data, "result_json", "result", true, "task")?;
            json_text_to_wire(&mut data, "metadata_json", "metadata", false, "task")?;
            strict_output::<TaskJsonlData>(data, "task")
        }
        "run" => {
            json_text_to_wire(&mut data, "metadata_json", "metadata", false, "run")?;
            strict_output::<RunJsonlData>(data, "run")
        }
        "comment" => {
            json_text_to_wire(&mut data, "metadata_json", "metadata", false, "comment")?;
            strict_output::<CommentJsonlData>(data, "comment")
        }
        "event" => {
            json_text_to_wire(&mut data, "payload_json", "payload", false, "event")?;
            strict_output::<EventJsonlData>(data, "event")
        }
        "column" => {
            let hidden = data.get_mut("hidden").ok_or_else(|| {
                KanbanError::Storage("column export row is missing hidden".into())
            })?;
            *hidden = match hidden {
                Value::Number(number) if number.as_i64() == Some(0) => Value::Bool(false),
                Value::Number(number) if number.as_i64() == Some(1) => Value::Bool(true),
                Value::Bool(value) => Value::Bool(*value),
                _ => {
                    return Err(KanbanError::Storage(
                        "column hidden must be SQLite 0 or 1".into(),
                    ));
                }
            };
            strict_output::<ColumnJsonlData>(data, "column")
        }
        "board" => strict_output::<BoardJsonlData>(data, "board"),
        "dependency" => strict_output::<DependencyJsonlData>(data, "dependency"),
        "attachment" => strict_output::<AttachmentJsonlData>(data, "attachment"),
        "task_label" => strict_output::<TaskLabelJsonlData>(data, "task_label"),
        _ => unreachable!("core discriminator was checked above"),
    }
}

pub(crate) fn decode_record(
    discriminator: &str,
    data: Map<String, Value>,
) -> Result<Map<String, Value>> {
    ensure_owned(discriminator)?;
    // Keep the catalog ownership probe separate from real import validation;
    // insert_jsonl_record rejects empty data before touching SQLite.
    if data.is_empty() {
        return Ok(data);
    }
    let mut data = match discriminator {
        "board" => strict_input::<BoardJsonlData>(data, "board")?,
        "column" => strict_input::<ColumnJsonlData>(data, "column")?,
        "task" => strict_input::<TaskJsonlData>(data, "task")?,
        "dependency" => strict_input::<DependencyJsonlData>(data, "dependency")?,
        "run" => strict_input::<RunJsonlData>(data, "run")?,
        "comment" => strict_input::<CommentJsonlData>(data, "comment")?,
        "event" => strict_input::<EventJsonlData>(data, "event")?,
        "attachment" => strict_input::<AttachmentJsonlData>(data, "attachment")?,
        "task_label" => strict_input::<TaskLabelJsonlData>(data, "task_label")?,
        _ => unreachable!("core discriminator was checked above"),
    };
    match discriminator {
        "task" => {
            wire_to_json_text(&mut data, "result", "result_json", true, "task")?;
            wire_to_json_text(&mut data, "metadata", "metadata_json", false, "task")?;
        }
        "run" => wire_to_json_text(&mut data, "metadata", "metadata_json", false, "run")?,
        "comment" => wire_to_json_text(&mut data, "metadata", "metadata_json", false, "comment")?,
        "event" => wire_to_json_text(&mut data, "payload", "payload_json", false, "event")?,
        _ => {}
    }
    Ok(data)
}

fn strict_output<T>(data: Map<String, Value>, discriminator: &str) -> Result<Map<String, Value>>
where
    T: DeserializeOwned + Serialize,
{
    let typed = serde_json::from_value::<T>(Value::Object(data)).map_err(|error| {
        KanbanError::Storage(format!(
            "{discriminator} export row violates its contract: {error}"
        ))
    })?;
    serialize_map(&typed, discriminator, "export")
}

fn strict_input<T>(data: Map<String, Value>, discriminator: &str) -> Result<Map<String, Value>>
where
    T: DeserializeOwned + Serialize,
{
    let typed = serde_json::from_value::<T>(Value::Object(data)).map_err(|error| {
        KanbanError::InvalidInput(format!(
            "{discriminator} import row violates its contract: {error}"
        ))
    })?;
    serialize_map(&typed, discriminator, "import")
}

fn serialize_map(
    value: &impl Serialize,
    discriminator: &str,
    direction: &str,
) -> Result<Map<String, Value>> {
    serde_json::to_value(value)
        .map_err(|error| {
            KanbanError::Storage(format!(
                "failed to serialize {discriminator} {direction} contract: {error}"
            ))
        })?
        .as_object()
        .cloned()
        .ok_or_else(|| KanbanError::Storage("portable contract data must be an object".into()))
}

fn json_text_to_wire(
    data: &mut Map<String, Value>,
    db_field: &str,
    wire_field: &str,
    sql_nullable: bool,
    discriminator: &str,
) -> Result<()> {
    let stored = data.remove(db_field).ok_or_else(|| {
        KanbanError::Storage(format!("{discriminator} export row is missing {db_field}"))
    })?;
    let natural = match stored {
        Value::Null if sql_nullable => Value::Null,
        Value::String(text) => serde_json::from_str(&text).map_err(|error| {
            KanbanError::Storage(format!(
                "{discriminator}.{db_field} contains invalid JSON: {error}"
            ))
        })?,
        _ => {
            return Err(KanbanError::Storage(format!(
                "{discriminator}.{db_field} must be JSON text{}",
                if sql_nullable { " or null" } else { "" }
            )));
        }
    };
    data.insert(wire_field.to_owned(), natural);
    Ok(())
}

fn wire_to_json_text(
    data: &mut Map<String, Value>,
    wire_field: &str,
    db_field: &str,
    sql_nullable: bool,
    discriminator: &str,
) -> Result<()> {
    let natural = data.remove(wire_field).ok_or_else(|| {
        KanbanError::InvalidInput(format!(
            "{discriminator} import row is missing {wire_field}"
        ))
    })?;
    let stored = if sql_nullable && natural.is_null() {
        Value::Null
    } else {
        Value::String(serde_json::to_string(&natural).map_err(|error| {
            KanbanError::InvalidInput(format!(
                "failed to encode {discriminator}.{wire_field}: {error}"
            ))
        })?)
    };
    data.insert(db_field.to_owned(), stored);
    Ok(())
}

fn ensure_owned(discriminator: &str) -> Result<()> {
    if DISCRIMINATORS.contains(&discriminator) {
        Ok(())
    } else {
        Err(KanbanError::Storage(format!(
            "core portable adapter does not own discriminator: {discriminator}"
        )))
    }
}
