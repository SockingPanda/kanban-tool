mod common;

use std::fs;

use anyhow::Context;
use common::{TempDb, kanban_without_db_in_dir_str_envs};

#[test]
fn config_show_reports_explicit_flag_sources_without_creating_database() -> anyhow::Result<()> {
    let temp = TempDb::new("config_show_reports_explicit_flag_sources_without_creating_database")?;
    let db_path = temp.dir.join("flag.db");
    let xdg_config = temp.dir.join("xdg-config");
    let xdg_data = temp.dir.join("xdg-data");

    let json = kanban_without_db_in_dir_str_envs(
        &[
            "--db",
            db_path.to_str().context("db path")?,
            "--board",
            "ops",
            "--locale",
            "en",
            "--json",
            "config",
            "show",
        ],
        &temp.dir,
        &[
            ("XDG_CONFIG_HOME", xdg_config.to_str().context("config")?),
            ("XDG_DATA_HOME", xdg_data.to_str().context("data")?),
        ],
    )?
    .success_json()?;

    assert_eq!(
        json["data"]["db"]["value"],
        db_path.to_string_lossy().to_string()
    );
    assert_eq!(json["data"]["db"]["source"]["kind"], "flag");
    assert_eq!(json["data"]["db"]["source"]["name"], "--db");
    assert_eq!(json["data"]["board"]["value"], "ops");
    assert_eq!(json["data"]["board"]["source"]["kind"], "flag");
    assert_eq!(json["data"]["locale"]["value"], "en");
    assert_eq!(json["data"]["locale"]["input"], "en");
    assert_eq!(json["data"]["locale"]["source"]["kind"], "flag");
    assert!(
        !db_path.exists(),
        "config show must not create the database"
    );
    Ok(())
}

#[test]
fn config_show_reports_environment_sources() -> anyhow::Result<()> {
    let temp = TempDb::new("config_show_reports_environment_sources")?;
    let db_path = temp.dir.join("env.db");
    let xdg_config = temp.dir.join("xdg-config");
    let xdg_data = temp.dir.join("xdg-data");

    let json = kanban_without_db_in_dir_str_envs(
        &["--json", "config", "show"],
        &temp.dir,
        &[
            ("KANBAN_DB", db_path.to_str().context("db")?),
            ("KB_BOARD", "env-board"),
            ("KANBAN_LOCALE", "en"),
            ("XDG_CONFIG_HOME", xdg_config.to_str().context("config")?),
            ("XDG_DATA_HOME", xdg_data.to_str().context("data")?),
        ],
    )?
    .success_json()?;

    assert_eq!(
        json["data"]["db"]["value"],
        db_path.to_string_lossy().to_string()
    );
    assert_eq!(json["data"]["db"]["source"]["kind"], "env");
    assert_eq!(json["data"]["db"]["source"]["name"], "KANBAN_DB");
    assert_eq!(json["data"]["board"]["value"], "env-board");
    assert_eq!(json["data"]["board"]["source"]["kind"], "env");
    assert_eq!(json["data"]["board"]["source"]["name"], "KB_BOARD");
    assert_eq!(json["data"]["locale"]["value"], "en");
    assert_eq!(json["data"]["locale"]["source"]["kind"], "env");
    assert_eq!(json["data"]["locale"]["source"]["name"], "KANBAN_LOCALE");
    Ok(())
}

#[test]
fn config_show_reports_legacy_kb_db_source() -> anyhow::Result<()> {
    let temp = TempDb::new("config_show_reports_legacy_kb_db_source")?;
    let db_path = temp.dir.join("legacy-env.db");
    let xdg_config = temp.dir.join("xdg-config");
    let xdg_data = temp.dir.join("xdg-data");

    let json = kanban_without_db_in_dir_str_envs(
        &["--json", "config", "show"],
        &temp.dir,
        &[
            ("KB_DB", db_path.to_str().context("db")?),
            ("XDG_CONFIG_HOME", xdg_config.to_str().context("config")?),
            ("XDG_DATA_HOME", xdg_data.to_str().context("data")?),
        ],
    )?
    .success_json()?;

    assert_eq!(
        json["data"]["db"]["value"],
        db_path.to_string_lossy().to_string()
    );
    assert_eq!(json["data"]["db"]["source"]["kind"], "env");
    assert_eq!(json["data"]["db"]["source"]["name"], "KB_DB");
    Ok(())
}

#[test]
fn config_show_empty_locale_flag_keeps_flag_precedence_over_env() -> anyhow::Result<()> {
    let temp = TempDb::new("config_show_empty_locale_flag_keeps_flag_precedence_over_env")?;
    let xdg_config = temp.dir.join("xdg-config");
    let xdg_data = temp.dir.join("xdg-data");

    let json = kanban_without_db_in_dir_str_envs(
        &["--locale", "", "--json", "config", "show"],
        &temp.dir,
        &[
            ("KANBAN_LOCALE", "en"),
            ("XDG_CONFIG_HOME", xdg_config.to_str().context("config")?),
            ("XDG_DATA_HOME", xdg_data.to_str().context("data")?),
        ],
    )?
    .success_json()?;

    assert_eq!(json["data"]["locale"]["value"], "zh-CN");
    assert_eq!(json["data"]["locale"]["input"], "");
    assert_eq!(json["data"]["locale"]["source"]["kind"], "flag");
    assert_eq!(json["data"]["locale"]["source"]["name"], "--locale");
    Ok(())
}

#[test]
fn config_show_invalid_locale_uses_runtime_json_error() -> anyhow::Result<()> {
    let temp = TempDb::new("config_show_invalid_locale_uses_runtime_json_error")?;
    let xdg_config = temp.dir.join("xdg-config");
    let xdg_data = temp.dir.join("xdg-data");

    let failed = kanban_without_db_in_dir_str_envs(
        &["--locale", "fr-FR", "--json", "config", "show"],
        &temp.dir,
        &[
            ("XDG_CONFIG_HOME", xdg_config.to_str().context("config")?),
            ("XDG_DATA_HOME", xdg_data.to_str().context("data")?),
        ],
    )?;

    assert_eq!(failed.output.status.code(), Some(2));
    assert!(failed.output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&failed.output.stdout)?;
    assert_eq!(json["error"]["code"], "invalid_input");
    assert_eq!(json["error"]["exit_code"], 2);
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("unsupported locale")
    );
    Ok(())
}

#[test]
fn config_show_reports_project_config_sources_and_resolves_relative_db() -> anyhow::Result<()> {
    let temp = TempDb::new("config_show_reports_project_config_sources_and_resolves_relative_db")?;
    let project_config = temp.dir.join(".kb").join("config.toml");
    fs::create_dir_all(project_config.parent().context("project config parent")?)?;
    fs::write(
        &project_config,
        "db = \"project.db\"\nboard = \"project-board\"\n",
    )?;
    let nested = temp.dir.join("a").join("b");
    fs::create_dir_all(&nested)?;
    let xdg_config = temp.dir.join("xdg-config");
    let xdg_data = temp.dir.join("xdg-data");

    let json = kanban_without_db_in_dir_str_envs(
        &["--json", "config", "show"],
        &nested,
        &[
            ("XDG_CONFIG_HOME", xdg_config.to_str().context("config")?),
            ("XDG_DATA_HOME", xdg_data.to_str().context("data")?),
        ],
    )?
    .success_json()?;

    let expected_db = temp.dir.join(".kb").join("project.db");
    assert_eq!(
        json["data"]["db"]["value"],
        expected_db.to_string_lossy().to_string()
    );
    assert_eq!(json["data"]["db"]["source"]["kind"], "project_config");
    assert_eq!(
        json["data"]["db"]["source"]["path"],
        project_config.to_string_lossy().to_string()
    );
    assert_eq!(json["data"]["db"]["source"]["key"], "db");
    assert_eq!(json["data"]["board"]["value"], "project-board");
    assert_eq!(json["data"]["board"]["source"]["kind"], "project_config");
    assert_eq!(json["data"]["board"]["source"]["key"], "board");
    Ok(())
}

#[test]
fn config_show_reports_global_and_default_sources() -> anyhow::Result<()> {
    let temp = TempDb::new("config_show_reports_global_and_default_sources")?;
    let xdg_config = temp.dir.join("xdg-config");
    let global_config = xdg_config.join("kanban").join("config.toml");
    fs::create_dir_all(global_config.parent().context("global config parent")?)?;
    fs::write(&global_config, "db = \"global.db\"\n")?;
    let xdg_data = temp.dir.join("xdg-data");

    let json = kanban_without_db_in_dir_str_envs(
        &["--json", "config", "show"],
        &temp.dir,
        &[
            ("XDG_CONFIG_HOME", xdg_config.to_str().context("config")?),
            ("XDG_DATA_HOME", xdg_data.to_str().context("data")?),
        ],
    )?
    .success_json()?;

    let expected_db = global_config
        .parent()
        .context("global parent")?
        .join("global.db");
    assert_eq!(
        json["data"]["db"]["value"],
        expected_db.to_string_lossy().to_string()
    );
    assert_eq!(json["data"]["db"]["source"]["kind"], "global_config");
    assert_eq!(
        json["data"]["db"]["source"]["path"],
        global_config.to_string_lossy().to_string()
    );
    assert_eq!(json["data"]["board"]["value"], "default");
    assert_eq!(json["data"]["board"]["source"]["kind"], "default");
    assert_eq!(json["data"]["locale"]["value"], "zh-CN");
    assert_eq!(json["data"]["locale"]["source"]["kind"], "default");
    Ok(())
}
