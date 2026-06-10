mod common;

use anyhow::Context;
use common::{TempDb, kb, kb_in_dir, kb_in_dir_env};

#[test]
fn board_create_list_show_archive_and_current_use_round_trip() -> anyhow::Result<()> {
    let temp = TempDb::new("board_create_list_show_archive_and_current_use_round_trip")?;
    kb(&temp.path, &["init"])?.success()?;

    let created = kb(
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

    let list = kb(&temp.path, &["--json", "board", "list"])?.success_json()?;
    assert_eq!(
        list["data"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        2
    );

    let shown = kb(&temp.path, &["--json", "board", "show", "project"])?.success_json()?;
    assert_eq!(shown["data"]["name"], "Project Board");

    let project_dir = temp.dir.join("workspace").join("nested");
    std::fs::create_dir_all(&project_dir)?;
    kb_in_dir(&temp.path, &["board", "use", "project"], &project_dir)?.success()?;
    let config = std::fs::read_to_string(project_dir.join(".kb/config.toml"))?;
    assert_eq!(config.trim(), "board = \"project\"");

    let current =
        kb_in_dir(&temp.path, &["--json", "board", "current"], &project_dir)?.success_json()?;
    assert_eq!(current["data"]["board"], "project");

    kb(&temp.path, &["board", "archive", "project"])?.success()?;
    let list = kb(&temp.path, &["--json", "board", "list"])?.success_json()?;
    assert_eq!(
        list["data"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        1
    );
    let all = kb(
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
    kb(&temp.path, &["init"])?.success()?;
    for slug in ["envboard", "configboard", "flagboard"] {
        kb(
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
        kb_in_dir(&temp.path, &["--json", "board", "current"], &nested)?.success_json()?;
    assert_eq!(from_config["data"]["board"], "configboard");

    let from_env = kb_in_dir_env(
        &temp.path,
        &["--json", "board", "current"],
        &nested,
        Some("envboard"),
    )?
    .success_json()?;
    assert_eq!(from_env["data"]["board"], "envboard");

    let from_flag = kb_in_dir_env(
        &temp.path,
        &["--board", "flagboard", "--json", "board", "current"],
        &nested,
        Some("envboard"),
    )?
    .success_json()?;
    assert_eq!(from_flag["data"]["board"], "flagboard");
    Ok(())
}

#[test]
fn task_output_and_refs_use_board_slug_seq_format() -> anyhow::Result<()> {
    let temp = TempDb::new("task_output_and_refs_use_board_slug_seq_format")?;
    kb(&temp.path, &["init"])?.success()?;
    kb(
        &temp.path,
        &["board", "create", "project", "--name", "Project"],
    )?
    .success()?;

    let human = kb(
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
    assert!(human.contains("t_"), "{human}");

    let json = kb(
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
        kb(&temp.path, &["--json", "task", "show", "project#1"])?.success_json()?;
    assert_eq!(by_project_seq["data"]["id"], task_id);

    let by_project_slash =
        kb(&temp.path, &["--json", "task", "show", "project/#1"])?.success_json()?;
    assert_eq!(by_project_slash["data"]["id"], task_id);
    Ok(())
}
