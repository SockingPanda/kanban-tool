mod common;

use anyhow::{Context, Result};
use common::{TempDb, kanban};
use kanban_contract::{CliEntityListOutput, CliEntityShowOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::fs;

fn fixture(name: &str) -> Result<Value> {
    let path = format!(
        "{}/../../schemas/fixtures/cli/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&fs::read_to_string(&path).with_context(|| format!("read {path}"))?)
        .with_context(|| format!("parse {path}"))
}

fn consume<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).context("consume exact entity output contract")
}

fn setup(name: &str) -> Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

fn create_task(temp: &TempDb, title: &str) -> Result<Value> {
    kanban(&temp.path, &["--json", "task", "create", title])?.success_json()
}

fn normalize_entity(entity: &mut Value) {
    entity["uri"] = json!("kb://task/t_FIXTURE");
    entity["source_id"] = json!("t_FIXTURE");
    entity["task_id"] = json!("t_FIXTURE");
    entity["board_id"] = json!("b_BOARD");
    entity["created_at"] = json!(100);
    entity["updated_at"] = json!(100);
}

fn entity_handler_ownership_violations(source: &str) -> Vec<&'static str> {
    let mut violations = Vec::new();
    if !source.contains("let output = CliEntityListOutput::new(") {
        violations.push("entity list must construct CliEntityListOutput");
    }
    if !source.contains("let output = CliEntityShowOutput::new(") {
        violations.push("entity show must construct CliEntityShowOutput");
    }
    if source
        .matches("print_contract_or_human(json, &output, || human)?;")
        .count()
        != 2
    {
        violations.push("entity list/show must render the contract-owned output exactly twice");
    }
    if source.contains("print_or_json(") {
        violations.push("entity handlers must not serialize private SQLite records");
    }
    violations
}

#[test]
fn entity_handlers_have_fail_closed_contract_output_ownership() {
    let source = include_str!("../src/commands/substrate.rs");
    let start = source
        .find("pub(crate) fn handle_entity")
        .expect("handle_entity source");
    let end = source
        .find("pub(crate) fn handle_outbox")
        .expect("handle_outbox source");
    let handler = &source[start..end];
    assert!(
        entity_handler_ownership_violations(handler).is_empty(),
        "production entity ownership violations: {:#?}",
        entity_handler_ownership_violations(handler)
    );

    let private_record_regression = handler.replace(
        "print_contract_or_human(json, &output, || human)?;",
        "print_or_json(json, &entities, || human)?;",
    );
    assert!(
        !entity_handler_ownership_violations(&private_record_regression).is_empty(),
        "ownership gate must reject private-record serialization regression"
    );
}

#[test]
fn producer_entity_list_matches_exact_fixture_and_honors_kind_and_limit() -> Result<()> {
    let temp = setup("producer_entity_list")?;
    let _first = create_task(&temp, "first entity")?;
    let second = create_task(&temp, "entity fixture")?;
    kanban(
        &temp.path,
        &["board", "create", "decoy", "--name", "newer board decoy"],
    )?
    .success()?;

    let unfiltered =
        kanban(&temp.path, &["--json", "entity", "list", "--limit", "10"])?.success_json()?;
    let kinds: Vec<_> = unfiltered["data"]
        .as_array()
        .context("entity list data must be an array")?
        .iter()
        .filter_map(|entity| entity["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"board"));
    assert!(kinds.iter().filter(|kind| **kind == "task").count() >= 2);

    let actual = kanban(
        &temp.path,
        &["--json", "entity", "list", "--kind", "task", "--limit", "1"],
    )?
    .success_json()?;
    let typed: CliEntityListOutput = consume(actual.clone())?;
    assert_eq!(typed.data.len(), 1);
    assert_eq!(typed.data[0].kind, "task");
    assert_eq!(typed.data[0].title.as_deref(), Some("entity fixture"));
    assert_eq!(typed.data[0].source_id, second["data"]["id"]);
    assert_eq!(
        typed.data[0].task_id.as_deref(),
        second["data"]["id"].as_str()
    );

    let mut normalized = actual;
    normalize_entity(&mut normalized["data"][0]);
    assert_eq!(normalized, fixture("entity-list-output.v1.valid.json")?);
    Ok(())
}

#[test]
fn producer_entity_show_matches_exact_fixture() -> Result<()> {
    let temp = setup("producer_entity_show")?;
    let task = create_task(&temp, "entity fixture")?;
    let task_id = task["data"]["id"].as_str().context("task id")?;
    let uri = format!("kb://task/{task_id}");

    let actual = kanban(&temp.path, &["--json", "entity", "show", &uri])?.success_json()?;
    let typed: CliEntityShowOutput = consume(actual.clone())?;
    assert_eq!(typed.data.uri, uri);
    assert_eq!(typed.data.source_id, task_id);
    assert_eq!(typed.data.task_id.as_deref(), Some(task_id));
    assert_eq!(typed.data.title.as_deref(), Some("entity fixture"));

    let mut normalized = actual;
    normalize_entity(&mut normalized["data"]);
    assert_eq!(normalized, fixture("entity-show-output.v1.valid.json")?);
    Ok(())
}

#[test]
fn entity_list_output_fixture_is_consumed_by_public_contract() -> Result<()> {
    let list: CliEntityListOutput = consume(fixture("entity-list-output.v1.valid.json")?)?;
    assert_eq!(list.data.len(), 1);
    Ok(())
}

#[test]
fn entity_show_output_fixture_is_consumed_by_public_contract() -> Result<()> {
    let show: CliEntityShowOutput = consume(fixture("entity-show-output.v1.valid.json")?)?;
    assert_eq!(show.data.uri, "kb://task/t_FIXTURE");
    Ok(())
}

#[test]
fn entity_contract_rejects_missing_nullable_and_unknown_fields() -> Result<()> {
    let list_fixture = fixture("entity-list-output.v1.valid.json")?;
    let show_fixture = fixture("entity-show-output.v1.valid.json")?;
    for key in [
        "board_id",
        "task_id",
        "title",
        "summary",
        "content_hash",
        "archived_at",
    ] {
        let mut list = list_fixture.clone();
        list["data"][0]
            .as_object_mut()
            .context("list entity object")?
            .remove(key);
        assert!(serde_json::from_value::<CliEntityListOutput>(list).is_err());

        let mut show = show_fixture.clone();
        show["data"]
            .as_object_mut()
            .context("show entity object")?
            .remove(key);
        assert!(serde_json::from_value::<CliEntityShowOutput>(show).is_err());
    }

    let mut nullable = show_fixture;
    for key in [
        "board_id",
        "task_id",
        "title",
        "summary",
        "content_hash",
        "archived_at",
    ] {
        nullable["data"][key] = Value::Null;
    }
    consume::<CliEntityShowOutput>(nullable)?;

    let invalid = fixture("entity-show-output.v1.invalid.json")?;
    assert!(serde_json::from_value::<CliEntityShowOutput>(invalid).is_err());
    Ok(())
}

#[test]
fn entity_show_preserves_not_found_failure() -> Result<()> {
    let temp = setup("entity_show_not_found")?;
    let result = kanban(
        &temp.path,
        &["--json", "entity", "show", "kb://task/t_missing"],
    )?;
    assert_eq!(result.output.status.code(), Some(3));
    assert!(result.output.stderr.is_empty());
    let json: Value = serde_json::from_slice(&result.output.stdout)?;
    assert_eq!(json["error"]["code"], "not_found");
    assert_eq!(json["error"]["exit_code"], 3);
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("entity kb://task/t_missing")
    );
    Ok(())
}
