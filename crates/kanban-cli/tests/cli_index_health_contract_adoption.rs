mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliIndexDoctorOutput, CliIndexStatusOutput};
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

fn reject_missing_and_unknown_fields<T: DeserializeOwned>(operation: &str) -> anyhow::Result<()> {
    let valid = fixture(operation)?;
    for key in ["index_version", "last_event_id", "index_lag_events"] {
        let mut missing = valid.clone();
        missing["data"]
            .as_object_mut()
            .context("index health data")?
            .remove(key);
        anyhow::ensure!(
            serde_json::from_value::<T>(missing).is_err(),
            "{operation} must require nullable field {key}"
        );
    }
    let mut unknown = valid;
    unknown["data"]["unexpected"] = json!(true);
    anyhow::ensure!(
        serde_json::from_value::<T>(unknown).is_err(),
        "{operation} must reject unknown data fields"
    );
    Ok(())
}

fn setup(name: &str) -> anyhow::Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

fn normalize_sqlite_status(mut output: Value) -> anyhow::Result<Value> {
    let data = output["data"]
        .as_object_mut()
        .context("index status data")?;
    anyhow::ensure!(data["backend"] == "sqlite");
    anyhow::ensure!(data["derived_index"] == false);
    anyhow::ensure!(data["stale"] == false);
    anyhow::ensure!(data["index_version"].is_null());
    anyhow::ensure!(data["last_event_id"].as_i64().is_some());
    anyhow::ensure!(data["index_lag_events"] == 0);
    anyhow::ensure!(
        data["message"]
            .as_str()
            .is_some_and(|message| message.contains("SQLite fallback"))
    );
    data.insert("last_event_id".to_owned(), json!(0));
    Ok(output)
}

#[test]
fn index_status_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("index_status_output_fixture_is_produced_by_real_cli")?;
    let output = kanban(&temp.path, &["--json", "index", "status"])?.success_json()?;
    serde_json::from_value::<CliIndexStatusOutput>(output.clone())?;
    assert_eq!(normalize_sqlite_status(output)?, fixture("index-status")?);
    Ok(())
}

#[test]
fn index_doctor_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("index_doctor_output_fixture_is_produced_by_real_cli")?;
    let status = kanban(&temp.path, &["--json", "index", "status"])?.success_json()?;
    let doctor = kanban(&temp.path, &["--json", "index", "doctor"])?.success_json()?;
    anyhow::ensure!(status == doctor, "index doctor must preserve status output");
    serde_json::from_value::<CliIndexDoctorOutput>(doctor.clone())?;
    assert_eq!(normalize_sqlite_status(doctor)?, fixture("index-doctor")?);
    Ok(())
}

#[test]
fn index_status_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliIndexStatusOutput>("index-status")
}

#[test]
fn index_doctor_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliIndexDoctorOutput>("index-doctor")
}

#[test]
fn index_health_contracts_require_nullable_fields_and_reject_unknown_fields() -> anyhow::Result<()>
{
    reject_missing_and_unknown_fields::<CliIndexStatusOutput>("index-status")?;
    reject_missing_and_unknown_fields::<CliIndexDoctorOutput>("index-doctor")
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn index_health_contracts_preserve_degraded_tantivy_fallback() -> anyhow::Result<()> {
    let temp = setup("index_health_contracts_preserve_degraded_tantivy_fallback")?;
    std::fs::create_dir_all(temp.dir.join("index/v1/tasks"))?;
    std::fs::write(
        temp.dir.join("index/v1/tasks/kb-index-meta.json"),
        b"partial tantivy meta",
    )?;

    let status = kanban(&temp.path, &["--json", "index", "status"])?.success_json()?;
    let doctor = kanban(&temp.path, &["--json", "index", "doctor"])?.success_json()?;
    serde_json::from_value::<CliIndexStatusOutput>(status.clone())?;
    serde_json::from_value::<CliIndexDoctorOutput>(doctor.clone())?;
    anyhow::ensure!(status == doctor);
    anyhow::ensure!(status["data"]["backend"] == "sqlite");
    anyhow::ensure!(status["data"]["derived_index"] == true);
    anyhow::ensure!(status["data"]["stale"] == true);
    anyhow::ensure!(
        status["data"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("degraded"))
    );
    for key in ["index_version", "last_event_id", "index_lag_events"] {
        anyhow::ensure!(
            status["data"].get(key).is_some(),
            "degraded output must retain required-nullable {key}"
        );
    }
    Ok(())
}
