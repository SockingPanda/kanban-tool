mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliDerivedStatusOutput, CliOutboxListOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(root.join(
        format!("schemas/fixtures/cli/{operation}-output.v1.valid.json"),
    ))?)?)
}

fn consume<T: DeserializeOwned>(operation: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(operation)?)?;
    Ok(())
}

fn setup(name: &str) -> anyhow::Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

#[test]
fn outbox_list_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("outbox_list_output_fixture_is_produced_by_real_cli")?;
    let task =
        kanban(&temp.path, &["--json", "task", "create", "outbox fixture"])?.success_json()?;
    let task_id = task["data"]["id"].as_str().context("task id")?;

    let full =
        kanban(&temp.path, &["--json", "outbox", "list", "--limit", "10"])?.success_json()?;
    serde_json::from_value::<CliOutboxListOutput>(full.clone())?;
    let full_jobs = full["data"].as_array().context("full outbox data")?;
    anyhow::ensure!(
        full_jobs
            .iter()
            .map(|job| job["target"].as_str())
            .collect::<Vec<_>>()
            == vec![Some("tantivy"), Some("oxigraph"), Some("lancedb")],
        "task writes must fan out to all three derived targets"
    );

    let absent = kanban(
        &temp.path,
        &[
            "--json",
            "outbox",
            "list",
            "--status",
            "__missing__",
            "--limit",
            "10",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliOutboxListOutput>(absent.clone())?;
    anyhow::ensure!(
        absent["data"].as_array().is_some_and(Vec::is_empty),
        "status filter must exclude non-matching jobs"
    );

    let output = kanban(
        &temp.path,
        &[
            "--json", "outbox", "list", "--status", "pending", "--limit", "2",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliOutboxListOutput>(output.clone())?;
    let jobs = output["data"].as_array().context("outbox data")?;
    anyhow::ensure!(jobs.len() == 2);
    anyhow::ensure!(jobs.iter().all(|job| job["status"] == "pending"));
    anyhow::ensure!(
        jobs.iter()
            .all(|job| job["entity_uri"] == format!("kb://task/{task_id}"))
    );
    anyhow::ensure!(
        jobs[0]["id"].as_i64().context("first outbox id")?
            < jobs[1]["id"].as_i64().context("second outbox id")?
    );

    let mut normalized = output;
    for (index, job) in normalized["data"]
        .as_array_mut()
        .context("outbox data")?
        .iter_mut()
        .enumerate()
    {
        job["id"] = json!(index as i64 + 1);
        job["source_event_id"] = json!(10);
        job["entity_uri"] = json!("kb://task/t_fixture");
        job["created_at"] = json!(100);
        job["updated_at"] = json!(100);
    }
    assert_eq!(normalized, fixture("outbox-list")?);
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn derived_status_output_fixture_proves_global_dirty_watermark() -> anyhow::Result<()> {
    let temp = setup("derived_status_output_fixture_proves_global_dirty_watermark")?;
    kanban(
        &temp.path,
        &["board", "create", "second", "--name", "Second"],
    )?
    .success()?;
    let default_task =
        kanban(&temp.path, &["--json", "task", "create", "default task"])?.success_json()?;
    let second_task = kanban(
        &temp.path,
        &[
            "--json",
            "--board",
            "second",
            "task",
            "create",
            "second task",
        ],
    )?
    .success_json()?;
    let default_uri = format!(
        "kb://task/{}",
        default_task["data"]["id"]
            .as_str()
            .context("default task id")?
    );
    let second_uri = format!(
        "kb://task/{}",
        second_task["data"]["id"]
            .as_str()
            .context("second task id")?
    );
    kanban(&temp.path, &["index", "rebuild"])?.success()?;

    let pending = kanban(
        &temp.path,
        &["--json", "outbox", "list", "--status", "pending"],
    )?
    .success_json()?;
    let pending_jobs = pending["data"].as_array().context("pending outbox data")?;
    let pending_tantivy_tasks = pending_jobs
        .iter()
        .filter(|job| {
            job["target"] == "tantivy"
                && job["entity_uri"]
                    .as_str()
                    .is_some_and(|uri| uri.starts_with("kb://task/"))
        })
        .collect::<Vec<_>>();
    anyhow::ensure!(
        pending_tantivy_tasks.len() == 1 && pending_tantivy_tasks[0]["entity_uri"] == second_uri,
        "only the second board Tantivy job may remain pending after default-board rebuild"
    );

    let done = kanban(
        &temp.path,
        &["--json", "outbox", "list", "--status", "done"],
    )?
    .success_json()?;
    let done_jobs = done["data"].as_array().context("done outbox data")?;
    anyhow::ensure!(
        done_jobs.iter().any(|job| {
            job["target"] == "tantivy" && job["entity_uri"].as_str() == Some(&default_uri)
        }),
        "default-board Tantivy job must be done"
    );
    anyhow::ensure!(
        !done_jobs.iter().any(|job| {
            job["target"] == "tantivy" && job["entity_uri"].as_str() == Some(&second_uri)
        }),
        "second-board Tantivy job must not be completed by default-board rebuild"
    );

    let output = kanban(&temp.path, &["--json", "derived", "status"])?.success_json()?;
    serde_json::from_value::<CliDerivedStatusOutput>(output.clone())?;
    let stores = output["data"].as_array().context("derived status data")?;
    let tantivy = stores
        .iter()
        .find(|store| store["store_name"] == "tantivy_tasks")
        .context("tantivy status")?;
    anyhow::ensure!(tantivy["last_event_id"].as_i64().is_some_and(|id| id > 0));
    anyhow::ensure!(tantivy["dirty"] == true);
    anyhow::ensure!(tantivy["last_rebuild_at"].as_i64().is_some());
    let mut normalized = output;
    for store in normalized["data"]
        .as_array_mut()
        .context("derived status data")?
    {
        store["last_event_id"] = json!(if store["store_name"] == "tantivy_tasks" {
            10
        } else {
            0
        });
        if !store["last_rebuild_at"].is_null() {
            store["last_rebuild_at"] = json!(100);
        }
        if !store["last_sync_at"].is_null() {
            store["last_sync_at"] = json!(100);
        }
        store["updated_at"] = json!(100);
    }
    assert_eq!(normalized, fixture("derived-status")?);
    Ok(())
}

#[test]
fn outbox_list_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliOutboxListOutput>("outbox-list")
}

#[test]
fn derived_status_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliDerivedStatusOutput>("derived-status")
}

#[test]
fn substrate_contracts_require_nullable_fields_and_reject_unknown_fields() -> anyhow::Result<()> {
    for key in ["source_event_id", "last_error"] {
        let mut outbox = fixture("outbox-list")?;
        outbox["data"][0]
            .as_object_mut()
            .context("outbox item")?
            .remove(key);
        anyhow::ensure!(
            serde_json::from_value::<CliOutboxListOutput>(outbox).is_err(),
            "outbox item must require nullable field {key}"
        );
    }

    let mut nullable_outbox = fixture("outbox-list")?;
    nullable_outbox["data"][0]["source_event_id"] = Value::Null;
    serde_json::from_value::<CliOutboxListOutput>(nullable_outbox)
        .context("explicit null source_event_id must remain valid")?;

    let mut outbox = fixture("outbox-list")?;
    outbox["data"][0]["unexpected"] = json!(true);
    anyhow::ensure!(serde_json::from_value::<CliOutboxListOutput>(outbox).is_err());

    for key in ["last_rebuild_at", "last_sync_at", "last_error"] {
        let mut derived = fixture("derived-status")?;
        derived["data"][0]
            .as_object_mut()
            .context("derived store status")?
            .remove(key);
        anyhow::ensure!(
            serde_json::from_value::<CliDerivedStatusOutput>(derived).is_err(),
            "derived status must require nullable field {key}"
        );
    }
    let mut derived = fixture("derived-status")?;
    derived["data"][0]["unexpected"] = json!(true);
    anyhow::ensure!(serde_json::from_value::<CliDerivedStatusOutput>(derived).is_err());
    Ok(())
}
