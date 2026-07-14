mod common;

use anyhow::Context;
use common::{TempDb, kanban};
use kanban_contract::{CliBackupOutput, CliCheckpointOutput, CliVacuumOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(operation: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(format!(
        "schemas/fixtures/cli/{operation}-output.v1.valid.json"
    ));
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
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
fn backup_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("backup_output_fixture_is_produced_by_real_cli")?;
    let backup = temp.dir.join("fixture-backup.sqlite");
    let backup_arg = backup.to_str().context("UTF-8 backup path")?;
    let mut output = kanban(
        &temp.path,
        &[
            "--json", "--actor", "fixture", "backup", "--out", backup_arg,
        ],
    )?
    .success_json()?;
    serde_json::from_value::<CliBackupOutput>(output.clone())?;
    anyhow::ensure!(backup.is_file(), "backup file was not created");
    anyhow::ensure!(output["data"]["out_path"] == backup_arg);
    kanban(&backup, &["board", "list"])?.success()?;
    output["data"]["out_path"] = json!("/fixture/backup.sqlite");
    assert_eq!(output, fixture("backup")?);
    Ok(())
}

#[test]
fn checkpoint_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("checkpoint_output_fixture_is_produced_by_real_cli")?;
    kanban(
        &temp.path,
        &[
            "--actor",
            "fixture",
            "task",
            "create",
            "Checkpoint write",
            "--description",
            "exercise the WAL",
        ],
    )?
    .success()?;
    let mut output =
        kanban(&temp.path, &["--json", "--actor", "fixture", "checkpoint"])?.success_json()?;
    serde_json::from_value::<CliCheckpointOutput>(output.clone())?;
    let data = output["data"].as_object_mut().context("checkpoint data")?;
    let busy = data["busy"].as_i64().context("busy integer")?;
    let log_frames = data["log_frames"].as_i64().context("log_frames integer")?;
    let checkpointed_frames = data["checkpointed_frames"]
        .as_i64()
        .context("checkpointed_frames integer")?;
    anyhow::ensure!((0..=1).contains(&busy), "busy must be 0 or 1");
    anyhow::ensure!(log_frames >= 0, "log_frames must be non-negative");
    anyhow::ensure!(
        (0..=log_frames).contains(&checkpointed_frames),
        "checkpointed_frames must be within the WAL frame count"
    );
    for (key, sentinel) in [
        ("busy", 0_i64),
        ("log_frames", 17_i64),
        ("checkpointed_frames", 17_i64),
    ] {
        data.insert(key.to_owned(), json!(sentinel));
    }
    assert_eq!(output, fixture("checkpoint")?);
    Ok(())
}

#[test]
fn vacuum_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("vacuum_output_fixture_is_produced_by_real_cli")?;
    let output = kanban(&temp.path, &["--json", "--actor", "fixture", "vacuum"])?.success_json()?;
    serde_json::from_value::<CliVacuumOutput>(output.clone())?;
    anyhow::ensure!(output["data"]["ok"] == true);
    kanban(&temp.path, &["board", "list"])?.success()?;
    assert_eq!(output, fixture("vacuum")?);
    Ok(())
}

#[test]
fn backup_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliBackupOutput>("backup")
}

#[test]
fn checkpoint_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliCheckpointOutput>("checkpoint")
}

#[test]
fn vacuum_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliVacuumOutput>("vacuum")
}
