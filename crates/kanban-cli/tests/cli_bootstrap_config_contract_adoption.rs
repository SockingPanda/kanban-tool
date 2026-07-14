mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_without_db_in_dir_str_envs};
use kanban_contract::{CliConfigShowOutput, CliInitOutput};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

#[cfg(unix)]
#[test]
fn init_json_rejects_non_utf8_path_before_database_mutation() -> anyhow::Result<()> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let temp = TempDb::new("init_json_rejects_non_utf8_path_before_database_mutation")?;
    let db = temp
        .dir
        .join(OsString::from_vec(b"invalid-\xff.db".to_vec()));
    let mut command = assert_cmd::Command::cargo_bin("kanban")?;
    let output = command
        .current_dir(&temp.dir)
        .arg("--db")
        .arg(&db)
        .args(["--json", "init"])
        .output()?;
    anyhow::ensure!(!output.status.success());
    anyhow::ensure!(!db.exists(), "JSON validation must precede init mutation");
    Ok(())
}

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

#[test]
fn init_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = TempDb::new("init_output_fixture_is_produced_by_real_cli")?;
    anyhow::ensure!(!temp.path.exists());
    let first = kanban(&temp.path, &["--json", "--actor", "fixture", "init"])?.success_json()?;
    anyhow::ensure!(temp.path.is_file());
    let board_id = first["data"]["board_id"]
        .as_str()
        .context("board id")?
        .to_owned();
    anyhow::ensure!(board_id.starts_with("b_"));
    kanban(
        &temp.path,
        &[
            "--json",
            "--actor",
            "fixture",
            "task",
            "create",
            "preserved task",
        ],
    )?
    .success()?;

    let mut output =
        kanban(&temp.path, &["--json", "--actor", "fixture", "init"])?.success_json()?;
    serde_json::from_value::<CliInitOutput>(output.clone())?;
    anyhow::ensure!(output["data"]["db_path"] == temp.path.to_str().context("db path")?);
    anyhow::ensure!(output["data"]["board_id"] == board_id);
    anyhow::ensure!(output["data"]["board_slug"] == "default");
    let boards = kanban(&temp.path, &["--json", "board", "list"])?.success_json()?;
    anyhow::ensure!(
        boards["data"]
            .as_array()
            .is_some_and(|boards| boards.len() == 1)
    );
    anyhow::ensure!(boards["data"][0]["id"] == board_id);
    anyhow::ensure!(boards["data"][0]["slug"] == "default");
    let tasks = kanban(&temp.path, &["--json", "task", "list"])?.success_json()?;
    anyhow::ensure!(tasks["data"][0]["title"] == "preserved task");
    output["data"]["db_path"] = json!("/fixture/kb.db");
    output["data"]["board_id"] = json!("b_fixture");
    assert_eq!(output, fixture("init")?);
    Ok(())
}

#[test]
fn config_show_output_fixture_is_produced_without_creating_database() -> anyhow::Result<()> {
    let temp = TempDb::new("config_show_output_fixture_is_produced_without_creating_database")?;
    let config = temp.dir.join(".kb/config.toml");
    std::fs::create_dir_all(config.parent().context("config parent")?)?;
    std::fs::write(&config, "db = \"project.db\"\nboard = \"project-board\"\n")?;
    let nested = temp.dir.join("nested");
    std::fs::create_dir_all(&nested)?;
    let db = temp.dir.join(".kb/project.db");
    anyhow::ensure!(!db.exists());
    let mut output = kanban_without_db_in_dir_str_envs(
        &["--locale", "en", "--json", "config", "show"],
        &nested,
        &[("XDG_CONFIG_HOME", temp.dir.to_str().context("temp dir")?)],
    )?
    .success_json()?;
    serde_json::from_value::<CliConfigShowOutput>(output.clone())?;
    anyhow::ensure!(
        !db.exists(),
        "config show must not create the resolved database"
    );
    anyhow::ensure!(output["data"]["db"]["value"] == db.to_str().context("db")?);
    anyhow::ensure!(output["data"]["db"]["source"]["kind"] == "project_config");
    anyhow::ensure!(output["data"]["db"]["source"]["path"] == config.to_str().context("config")?);
    anyhow::ensure!(output["data"]["db"]["source"]["key"] == "db");
    anyhow::ensure!(output["data"]["board"]["value"] == "project-board");
    anyhow::ensure!(output["data"]["board"]["source"]["kind"] == "project_config");
    anyhow::ensure!(
        output["data"]["board"]["source"]["path"] == config.to_str().context("board config")?
    );
    anyhow::ensure!(output["data"]["board"]["source"]["key"] == "board");
    anyhow::ensure!(output["data"]["locale"]["value"] == "en");
    anyhow::ensure!(output["data"]["locale"]["input"] == "en");
    anyhow::ensure!(output["data"]["locale"]["source"]["kind"] == "flag");
    output["data"]["db"]["value"] = json!("/fixture/.kb/project.db");
    output["data"]["db"]["source"]["path"] = json!("/fixture/.kb/config.toml");
    output["data"]["board"]["source"]["path"] = json!("/fixture/.kb/config.toml");
    assert_eq!(output, fixture("config-show")?);
    Ok(())
}

#[test]
fn init_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliInitOutput>("init")
}

#[test]
fn config_show_output_fixture_is_consumed_by_public_contract() -> anyhow::Result<()> {
    consume::<CliConfigShowOutput>("config-show")
}
