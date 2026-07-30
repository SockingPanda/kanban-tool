mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{
    CliMaintenanceRebuildOutput, CliMaintenanceRunOutput, CliMaintenanceStatusOutput,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(root.join(
        format!("schemas/fixtures/cli/{operation}-output.v1.valid.json"),
    ))?)?)
}

fn setup(name: &str) -> anyhow::Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

fn consume<T: DeserializeOwned>(operation: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(operation)?)?;
    Ok(())
}

fn normalize_status(mut output: Value) -> anyhow::Result<Value> {
    let data = output["data"]
        .as_object_mut()
        .context("maintenance status data")?;
    let database_instance_id = data["database_instance_id"]
        .as_str()
        .context("database instance id")?
        .to_owned();
    data.insert("database_instance_id".to_owned(), json!("db_fixture"));
    let stores = data["stores"]
        .as_array_mut()
        .context("maintenance stores")?;
    let mut tantivy = stores
        .iter()
        .find(|store| store["store_name"] == "tantivy_tasks")
        .cloned()
        .context("Tantivy status")?;
    anyhow::ensure!(tantivy["database_instance_id"] == database_instance_id);
    tantivy["database_instance_id"] = json!("db_fixture");
    tantivy["updated_at"] = json!(0);
    *stores = vec![tantivy];
    Ok(output)
}

fn normalize_report(mut output: Value) -> anyhow::Result<Value> {
    output["data"]["database_instance_id"] = json!("db_fixture");
    Ok(output)
}

#[test]
fn maintenance_status_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("maintenance_status_contract")?;
    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-owner",
            "--json",
            "maintenance",
            "status",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliMaintenanceStatusOutput>(output.clone())?;
    assert_eq!(normalize_status(output)?, fixture("maintenance-status")?);
    Ok(())
}

#[test]
fn maintenance_run_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("maintenance_run_contract")?;
    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-owner",
            "--json",
            "maintenance",
            "run",
            "--once",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliMaintenanceRunOutput>(output.clone())?;
    #[cfg(feature = "oxigraph-backend")]
    let expected = {
        let mut expected = fixture("maintenance-run")?;
        expected["data"]["stores"]
            .as_array_mut()
            .expect("maintenance stores fixture")
            .push(serde_json::json!({
                "store_name": "oxigraph_relations",
                "action": "generation_published",
                "processed": 0,
                "lifecycle_status": "ready",
                "fallback_reason": null
            }));
        expected
    };
    #[cfg(not(feature = "oxigraph-backend"))]
    let expected = fixture("maintenance-run")?;
    assert_eq!(normalize_report(output)?, expected);
    Ok(())
}

#[test]
fn maintenance_rebuild_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("maintenance_rebuild_contract")?;
    kanban(
        &temp.path,
        &["--actor", "fixture-owner", "maintenance", "run", "--once"],
    )?
    .success()?;
    let output = kanban(
        &temp.path,
        &[
            "--actor",
            "fixture-owner",
            "--json",
            "maintenance",
            "rebuild",
            "tantivy_tasks",
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliMaintenanceRebuildOutput>(output.clone())?;
    assert_eq!(normalize_report(output)?, fixture("maintenance-rebuild")?);
    Ok(())
}

#[test]
fn maintenance_status_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliMaintenanceStatusOutput>("maintenance-status")
}

#[test]
fn maintenance_run_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliMaintenanceRunOutput>("maintenance-run")
}

#[test]
fn maintenance_rebuild_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliMaintenanceRebuildOutput>("maintenance-rebuild")
}

#[test]
fn maintenance_contracts_reject_unknown_and_missing_nullable_fields() -> anyhow::Result<()> {
    let mut status = fixture("maintenance-status")?;
    status["data"]["stores"][0]
        .as_object_mut()
        .context("store object")?
        .remove("fallback_reason");
    anyhow::ensure!(serde_json::from_value::<CliMaintenanceStatusOutput>(status).is_err());

    let mut run = fixture("maintenance-run")?;
    run["data"]["stores"][0]["unexpected"] = json!(true);
    anyhow::ensure!(serde_json::from_value::<CliMaintenanceRunOutput>(run).is_err());
    Ok(())
}
