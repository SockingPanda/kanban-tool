mod common;

use common::{TempDb, kanban};

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
