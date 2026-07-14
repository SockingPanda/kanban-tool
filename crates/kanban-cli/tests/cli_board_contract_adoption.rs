mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir};
use kanban_contract::{
    ArchiveBoardResponse, CliActiveBoardOutput, CreateBoardResponse, GetBoardResponse,
    ListBoardsResponse,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn fixture(path: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(
        root.join(path),
    )?)?)
}

fn setup(name: &str) -> anyhow::Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &["init"])?.success()?;
    Ok(temp)
}

fn create_fixture_board(temp: &TempDb) -> anyhow::Result<Value> {
    kanban(
        &temp.path,
        &["--json", "board", "create", "fixture", "--name", "Fixture"],
    )?
    .success_json()
}

fn normalize_board(board: &mut Value, archived: bool) -> anyhow::Result<()> {
    let object = board.as_object_mut().context("board object")?;
    let id = object.get_mut("id").context("board.id")?;
    anyhow::ensure!(
        id.as_str().is_some_and(|value| !value.is_empty()),
        "board.id must be a non-empty string"
    );
    *id = json!("b_fixture");
    let created_at = object.get_mut("created_at").context("board.created_at")?;
    anyhow::ensure!(created_at.is_i64(), "board.created_at must be an integer");
    *created_at = json!(101);
    let updated_at = object.get_mut("updated_at").context("board.updated_at")?;
    anyhow::ensure!(updated_at.is_i64(), "board.updated_at must be an integer");
    *updated_at = json!(102);
    let archived_at = object.get_mut("archived_at").context("board.archived_at")?;
    if archived {
        anyhow::ensure!(archived_at.is_i64(), "archived board must have archived_at");
        *archived_at = json!(103);
    } else {
        anyhow::ensure!(
            archived_at.is_null(),
            "active board archived_at must be null"
        );
    }
    Ok(())
}

fn consume<T: DeserializeOwned>(path: &str) -> anyhow::Result<()> {
    serde_json::from_value::<T>(fixture(path)?).context("valid CLI output fixture")?;
    Ok(())
}

#[test]
fn board_list_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("board_list_output_fixture_is_produced_by_real_cli")?;
    create_fixture_board(&temp)?;
    let mut output = kanban(
        &temp.path,
        &["--json", "board", "list", "--include-archived"],
    )?
    .success_json()?;
    serde_json::from_value::<ListBoardsResponse>(output.clone())
        .context("real board list output must satisfy its contract root")?;
    let boards = output["data"].as_array_mut().context("board list")?;
    assert_eq!(boards.len(), 2);
    let fixture_board = boards
        .iter_mut()
        .find(|board| board["slug"] == "fixture")
        .context("fixture board")?;
    normalize_board(fixture_board, false)?;
    let fixture_board = fixture_board.clone();
    output["data"] = json!([fixture_board]);
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/board-list-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn board_list_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<ListBoardsResponse>("schemas/fixtures/cli/board-list-output.v1.valid.json")
}

#[test]
fn board_create_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("board_create_output_fixture_is_produced_by_real_cli")?;
    let mut output = create_fixture_board(&temp)?;
    serde_json::from_value::<CreateBoardResponse>(output.clone())
        .context("real board create output must satisfy its contract root")?;
    normalize_board(&mut output["data"], false)?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/board-create-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn board_create_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CreateBoardResponse>("schemas/fixtures/cli/board-create-output.v1.valid.json")
}

#[test]
fn board_show_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("board_show_output_fixture_is_produced_by_real_cli")?;
    create_fixture_board(&temp)?;
    let mut output = kanban(&temp.path, &["--json", "board", "show", "fixture"])?.success_json()?;
    serde_json::from_value::<GetBoardResponse>(output.clone())
        .context("real board show output must satisfy its contract root")?;
    normalize_board(&mut output["data"], false)?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/board-show-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn board_show_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<GetBoardResponse>("schemas/fixtures/cli/board-show-output.v1.valid.json")
}

#[test]
fn board_use_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("board_use_output_fixture_is_produced_by_real_cli")?;
    create_fixture_board(&temp)?;
    let workspace = temp.dir.join("workspace");
    std::fs::create_dir_all(&workspace)?;
    let mut output = kanban_in_dir(
        &temp.path,
        &["--json", "board", "use", "fixture"],
        &workspace,
    )?
    .success_json()?;
    serde_json::from_value::<CliActiveBoardOutput>(output.clone())
        .context("real board use output must satisfy its contract root")?;
    normalize_board(&mut output["data"]["board"], false)?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/board-use-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn board_use_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliActiveBoardOutput>("schemas/fixtures/cli/board-use-output.v1.valid.json")
}

#[test]
fn board_current_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("board_current_output_fixture_is_produced_by_real_cli")?;
    create_fixture_board(&temp)?;
    let mut output = kanban(
        &temp.path,
        &["--json", "--board", "fixture", "board", "current"],
    )?
    .success_json()?;
    serde_json::from_value::<CliActiveBoardOutput>(output.clone())
        .context("real board current output must satisfy its contract root")?;
    normalize_board(&mut output["data"]["board"], false)?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/board-current-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn board_current_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<CliActiveBoardOutput>("schemas/fixtures/cli/board-current-output.v1.valid.json")
}

#[test]
fn board_archive_output_fixture_is_produced_by_real_cli() -> anyhow::Result<()> {
    let temp = setup("board_archive_output_fixture_is_produced_by_real_cli")?;
    create_fixture_board(&temp)?;
    let mut output =
        kanban(&temp.path, &["--json", "board", "archive", "fixture"])?.success_json()?;
    serde_json::from_value::<ArchiveBoardResponse>(output.clone())
        .context("real board archive output must satisfy its contract root")?;
    normalize_board(&mut output["data"], true)?;
    assert_eq!(
        output,
        fixture("schemas/fixtures/cli/board-archive-output.v1.valid.json")?
    );
    Ok(())
}

#[test]
fn board_archive_output_fixture_is_consumed_by_contract_root() -> anyhow::Result<()> {
    consume::<ArchiveBoardResponse>("schemas/fixtures/cli/board-archive-output.v1.valid.json")
}
