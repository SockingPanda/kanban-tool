mod common;

use common::{TempDb, kanban, kanban_without_db_in_dir_str_envs};

#[test]
fn init_help_documents_force_as_deprecated_noop() -> anyhow::Result<()> {
    let temp = TempDb::new("init_help_documents_force_as_deprecated_noop")?;
    let help = kanban(&temp.path, &["init", "--help"])?.success_stdout()?;

    assert!(help.contains("--force"), "{help}");
    assert!(help.contains("Deprecated compatibility no-op"), "{help}");
    assert!(help.contains("idempotent"), "{help}");
    assert!(help.contains("never resets data"), "{help}");

    Ok(())
}

#[test]
fn init_is_idempotent_without_force() -> anyhow::Result<()> {
    let temp = TempDb::new("init_is_idempotent_without_force")?;

    let first = kanban(&temp.path, &["--json", "init"])?.success_json()?;
    assert_eq!(first["data"]["board_slug"], "default");
    let board_id = first["data"]["board_id"]
        .as_str()
        .expect("init should return board_id")
        .to_owned();

    let second = kanban(&temp.path, &["--json", "init"])?.success_json()?;
    assert_eq!(second["data"]["board_slug"], "default");
    assert_eq!(second["data"]["board_id"], board_id);

    Ok(())
}

#[test]
fn init_force_is_compatible_noop_for_initialized_database() -> anyhow::Result<()> {
    let temp = TempDb::new("init_force_is_compatible_noop_for_initialized_database")?;

    let first = kanban(&temp.path, &["--json", "init"])?.success_json()?;
    let board_id = first["data"]["board_id"]
        .as_str()
        .expect("init should return board_id")
        .to_owned();

    let forced = kanban(&temp.path, &["--json", "init", "--force"])?.success_json()?;
    assert_eq!(forced["data"]["board_slug"], "default");
    assert_eq!(forced["data"]["board_id"], board_id);

    let boards = kanban(&temp.path, &["--json", "board", "list"])?.success_json()?;
    assert_eq!(boards["data"].as_array().expect("boards array").len(), 1);

    Ok(())
}

#[test]
fn db_path_priority_is_flag_then_env_then_project_config_then_user_config_then_xdg_default()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "db_path_priority_is_flag_then_env_then_project_config_then_user_config_then_xdg_default",
    )?;
    let workspace = temp.dir.join("workspace");
    std::fs::create_dir_all(workspace.join(".kb"))?;
    let xdg_config = temp.dir.join("xdg-config");
    std::fs::create_dir_all(xdg_config.join("kanban"))?;

    let flag_db = temp.dir.join("flag.db");
    let env_db = temp.dir.join("env.db");
    let compat_env_db = temp.dir.join("compat-env.db");
    let project_db = temp.dir.join("project.db");
    let user_db = temp.dir.join("user.db");
    std::fs::write(
        workspace.join(".kb/config.toml"),
        format!("db = \"{}\"\n", project_db.display()),
    )?;
    std::fs::write(
        xdg_config.join("kanban/config.toml"),
        format!("db = \"{}\"\n", user_db.display()),
    )?;

    let from_flag = kanban_without_db_in_dir_str_envs(
        &["--db", flag_db.to_str().unwrap(), "--json", "init"],
        &workspace,
        &[
            ("KANBAN_DB", env_db.to_str().unwrap()),
            ("XDG_CONFIG_HOME", xdg_config.to_str().unwrap()),
        ],
    )?
    .success_json()?;
    assert_eq!(from_flag["data"]["db_path"], flag_db.display().to_string());
    assert!(flag_db.exists());
    assert!(!env_db.exists());

    let from_env = kanban_without_db_in_dir_str_envs(
        &["--json", "init"],
        &workspace,
        &[
            ("KANBAN_DB", env_db.to_str().unwrap()),
            ("XDG_CONFIG_HOME", xdg_config.to_str().unwrap()),
        ],
    )?
    .success_json()?;
    assert_eq!(from_env["data"]["db_path"], env_db.display().to_string());
    assert!(env_db.exists());
    assert!(!project_db.exists());

    let from_compat_env = kanban_without_db_in_dir_str_envs(
        &["--json", "init"],
        &workspace,
        &[
            ("KB_DB", compat_env_db.to_str().unwrap()),
            ("XDG_CONFIG_HOME", xdg_config.to_str().unwrap()),
        ],
    )?
    .success_json()?;
    assert_eq!(
        from_compat_env["data"]["db_path"],
        compat_env_db.display().to_string()
    );
    assert!(compat_env_db.exists());

    let from_project = kanban_without_db_in_dir_str_envs(
        &["--json", "init"],
        &workspace,
        &[("XDG_CONFIG_HOME", xdg_config.to_str().unwrap())],
    )?
    .success_json()?;
    assert_eq!(
        from_project["data"]["db_path"],
        project_db.display().to_string()
    );
    assert!(project_db.exists());
    assert!(!user_db.exists());

    std::fs::remove_file(workspace.join(".kb/config.toml"))?;
    let from_user = kanban_without_db_in_dir_str_envs(
        &["--json", "init"],
        &workspace,
        &[("XDG_CONFIG_HOME", xdg_config.to_str().unwrap())],
    )?
    .success_json()?;
    assert_eq!(from_user["data"]["db_path"], user_db.display().to_string());
    assert!(user_db.exists());

    let xdg_data = temp.dir.join("xdg-data");
    let from_default = kanban_without_db_in_dir_str_envs(
        &["--json", "init"],
        &workspace,
        &[("XDG_DATA_HOME", xdg_data.to_str().unwrap())],
    )?
    .success_json()?;
    let expected_default = xdg_data.join("kb/kb.db");
    assert_eq!(
        from_default["data"]["db_path"],
        expected_default.display().to_string()
    );
    assert!(expected_default.exists());

    Ok(())
}

#[test]
fn empty_db_env_values_are_ignored() -> anyhow::Result<()> {
    let temp = TempDb::new("empty_db_env_values_are_ignored")?;
    let workspace = temp.dir.join("workspace");
    std::fs::create_dir_all(workspace.join(".kb"))?;
    let project_db = temp.dir.join("project.db");
    std::fs::write(
        workspace.join(".kb/config.toml"),
        format!("db = \"{}\"\n", project_db.display()),
    )?;

    let from_project = kanban_without_db_in_dir_str_envs(
        &["--json", "init"],
        &workspace,
        &[("KANBAN_DB", " "), ("KB_DB", "")],
    )?
    .success_json()?;
    assert_eq!(
        from_project["data"]["db_path"],
        project_db.display().to_string()
    );

    Ok(())
}
