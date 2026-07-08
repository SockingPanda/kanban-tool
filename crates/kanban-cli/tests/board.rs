mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir, kanban_in_dir_env, kanban_without_db_in_dir_str_envs};

#[test]
fn board_create_list_show_archive_and_current_use_round_trip() -> anyhow::Result<()> {
    let temp = TempDb::new("board_create_list_show_archive_and_current_use_round_trip")?;
    kanban(&temp.path, &["init"])?.success()?;

    let created = kanban(
        &temp.path,
        &[
            "--json",
            "board",
            "create",
            "project",
            "--name",
            "Project Board",
            "--description",
            "Local project",
        ],
    )?
    .success_json()?;
    assert_eq!(created["data"]["slug"], "project");

    let list = kanban(&temp.path, &["--json", "board", "list"])?.success_json()?;
    assert_eq!(
        list["data"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        2
    );

    let shown = kanban(&temp.path, &["--json", "board", "show", "project"])?.success_json()?;
    assert_eq!(shown["data"]["name"], "Project Board");

    let project_dir = temp.dir.join("workspace").join("nested");
    std::fs::create_dir_all(&project_dir)?;
    kanban_in_dir(&temp.path, &["board", "use", "project"], &project_dir)?.success()?;
    let config = std::fs::read_to_string(project_dir.join(".kb/config.toml"))?;
    assert_eq!(config.trim(), "board = \"project\"");

    let current =
        kanban_in_dir(&temp.path, &["--json", "board", "current"], &project_dir)?.success_json()?;
    assert_eq!(current["data"]["board"]["slug"], "project");

    kanban(&temp.path, &["board", "archive", "project"])?.success()?;
    let list = kanban(&temp.path, &["--json", "board", "list"])?.success_json()?;
    assert_eq!(
        list["data"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        1
    );
    let all = kanban(
        &temp.path,
        &["--json", "board", "list", "--include-archived"],
    )?
    .success_json()?;
    assert_eq!(
        all["data"].as_array().context("expected JSON array")?.len(),
        2
    );
    Ok(())
}

#[test]
fn active_board_priority_is_flag_then_env_then_nearest_config_then_default() -> anyhow::Result<()> {
    let temp =
        TempDb::new("active_board_priority_is_flag_then_env_then_nearest_config_then_default")?;
    kanban(&temp.path, &["init"])?.success()?;
    for slug in ["envboard", "configboard", "flagboard"] {
        kanban(
            &temp.path,
            &["board", "create", slug, "--name", &format!("{slug} board")],
        )?
        .success()?;
    }

    let workspace = temp.dir.join("workspace");
    let nested = workspace.join("a/b");
    std::fs::create_dir_all(workspace.join(".kb"))?;
    std::fs::create_dir_all(&nested)?;
    std::fs::write(
        workspace.join(".kb/config.toml"),
        "board = \"configboard\"\n",
    )?;

    let from_config =
        kanban_in_dir(&temp.path, &["--json", "board", "current"], &nested)?.success_json()?;
    assert_eq!(from_config["data"]["board"]["slug"], "configboard");

    let from_env = kanban_in_dir_env(
        &temp.path,
        &["--json", "board", "current"],
        &nested,
        Some("envboard"),
    )?
    .success_json()?;
    assert_eq!(from_env["data"]["board"]["slug"], "envboard");

    let from_flag = kanban_in_dir_env(
        &temp.path,
        &["--board", "flagboard", "--json", "board", "current"],
        &nested,
        Some("envboard"),
    )?
    .success_json()?;
    assert_eq!(from_flag["data"]["board"]["slug"], "flagboard");
    Ok(())
}

#[test]
fn db_path_priority_is_flag_then_env_then_project_config_then_user_config() -> anyhow::Result<()> {
    let temp =
        TempDb::new("db_path_priority_is_flag_then_env_then_project_config_then_user_config")?;
    let flag_db = temp.dir.join("flag.sqlite");
    let kanban_db = temp.dir.join("kanban-env.sqlite");
    let kb_db = temp.dir.join("kb-env.sqlite");
    let project_db = temp.dir.join("project.sqlite");
    let user_db = temp.dir.join("user.sqlite");

    for (db, slug) in [
        (&flag_db, "flag-db"),
        (&kanban_db, "kanban-env-db"),
        (&kb_db, "kb-env-db"),
        (&project_db, "project-db"),
        (&user_db, "user-db"),
    ] {
        kanban(db, &["init"])?.success()?;
        kanban(db, &["board", "create", slug, "--name", slug])?.success()?;
    }

    let workspace = temp.dir.join("workspace");
    let nested = workspace.join("a/b");
    let xdg_config = temp.dir.join("xdg-config");
    std::fs::create_dir_all(workspace.join(".kb"))?;
    std::fs::create_dir_all(&nested)?;
    std::fs::create_dir_all(xdg_config.join("kanban"))?;
    std::fs::write(
        workspace.join(".kb/config.toml"),
        format!("db = {:?}\n", project_db.display().to_string()),
    )?;
    std::fs::write(
        xdg_config.join("kanban/config.toml"),
        format!("db = {:?}\n", user_db.display().to_string()),
    )?;

    let xdg_config = xdg_config.to_str().context("xdg path")?;
    let flag_db = flag_db.to_str().context("flag db")?;
    let kanban_db = kanban_db.to_str().context("kanban db")?;
    let kb_db = kb_db.to_str().context("kb db")?;

    kanban_without_db_in_dir_str_envs(
        &["--json", "board", "show", "user-db"],
        &temp.dir,
        &[("XDG_CONFIG_HOME", xdg_config)],
    )?
    .success_json()?;

    kanban_without_db_in_dir_str_envs(
        &["--json", "board", "show", "project-db"],
        &nested,
        &[("XDG_CONFIG_HOME", xdg_config)],
    )?
    .success_json()?;

    kanban_without_db_in_dir_str_envs(
        &["--json", "board", "show", "kb-env-db"],
        &nested,
        &[("XDG_CONFIG_HOME", xdg_config), ("KB_DB", kb_db)],
    )?
    .success_json()?;

    kanban_without_db_in_dir_str_envs(
        &["--json", "board", "show", "kanban-env-db"],
        &nested,
        &[
            ("XDG_CONFIG_HOME", xdg_config),
            ("KB_DB", kb_db),
            ("KANBAN_DB", kanban_db),
        ],
    )?
    .success_json()?;

    kanban_without_db_in_dir_str_envs(
        &["--db", flag_db, "--json", "board", "show", "flag-db"],
        &nested,
        &[("XDG_CONFIG_HOME", xdg_config), ("KANBAN_DB", kanban_db)],
    )?
    .success_json()?;

    Ok(())
}

#[test]
fn task_output_and_refs_use_board_slug_seq_format() -> anyhow::Result<()> {
    let temp = TempDb::new("task_output_and_refs_use_board_slug_seq_format")?;
    kanban(&temp.path, &["init"])?.success()?;
    kanban(
        &temp.path,
        &["board", "create", "project", "--name", "Project"],
    )?
    .success()?;

    let human = kanban(
        &temp.path,
        &[
            "--board",
            "project",
            "task",
            "create",
            "project task",
            "--description",
            "ready spec",
        ],
    )?
    .success_stdout()?;
    assert!(human.contains("project#1"), "{human}");
    assert!(!human.contains("t_"), "{human}");

    let json = kanban(
        &temp.path,
        &["--board", "project", "--json", "task", "show", "1"],
    )?
    .success_json()?;
    assert_eq!(json["data"]["board_slug"], "project");
    assert_eq!(json["data"]["ref"], "project#1");
    let task_id = json["data"]["id"]
        .as_str()
        .context("expected JSON string")?
        .to_owned();

    let by_project_seq =
        kanban(&temp.path, &["--json", "task", "show", "project#1"])?.success_json()?;
    assert_eq!(by_project_seq["data"]["id"], task_id);

    let by_project_slash =
        kanban(&temp.path, &["--json", "task", "show", "project/#1"])?.success_json()?;
    assert_eq!(by_project_slash["data"]["id"], task_id);
    Ok(())
}
