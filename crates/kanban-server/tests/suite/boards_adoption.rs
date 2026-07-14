use crate::common::*;
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> Value {
    serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/fixtures/api")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

fn seed_board(
    test: &TestApp,
    slug: &str,
    name: &str,
    description: Option<&str>,
) -> anyhow::Result<()> {
    kanban_sqlite::api::create_board(
        test.db_path(),
        "contract-test",
        kanban_sqlite::api::CreateBoard {
            slug: slug.into(),
            name: name.into(),
            description: description.map(str::to_owned),
        },
    )?;
    Ok(())
}

fn normalize_board(value: &mut Value, id: &str, created_at: i64, updated_at: i64) {
    let board = value.as_object_mut().expect("board object");
    board.insert("id".into(), json!(id));
    board.insert("created_at".into(), json!(created_at));
    board.insert("updated_at".into(), json!(updated_at));
    if board["archived_at"].is_number() {
        board.insert("archived_at".into(), json!(updated_at));
    }
}

#[test]
fn list_boards_query_dto_serializes_to_committed_fixture() {
    let request = kanban_contract::ListBoardsQuery {
        include_archived: true,
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        fixture("list-boards-query.v1.valid.json")
    );
}

#[tokio::test]
async fn list_boards_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_board(&test, "contract-archived", "Contract Archived", None)?;
    kanban_sqlite::api::archive_board(test.db_path(), "contract-archived", "contract-test")?;
    let query: kanban_contract::ListBoardsQuery =
        serde_json::from_value(fixture("list-boards-query.v1.valid.json"))?;
    let (status, response) = get_json(
        test.router(),
        &format!("/api/v1/boards?include_archived={}", query.include_archived),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        response["data"]
            .as_array()
            .context("boards")?
            .iter()
            .any(|board| board["slug"] == "contract-archived" && board["archived_at"].is_number())
    );
    Ok(())
}

#[test]
fn create_board_request_dto_serializes_to_committed_fixture() {
    let request = kanban_contract::CreateBoardRequest {
        slug: "contract-created".into(),
        name: "Contract Created".into(),
        description: Some("Created through the canonical request".into()),
        actor: Some("contract-test".into()),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        fixture("create-board-request.v1.valid.json")
    );
}

#[tokio::test]
async fn create_board_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let request = fixture("create-board-request.v1.valid.json");
    let (status, response) = post_json(test.router(), "/api/v1/boards", request.clone()).await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(response["data"]["slug"], request["slug"]);
    let events = kanban_sqlite::api::list_events(test.db_path(), "contract-created", None)?;
    assert_eq!(events[0].actor.as_deref(), Some("contract-test"));
    Ok(())
}

#[test]
fn get_board_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::GetBoardPath {
            board: "contract-get".into(),
        })
        .unwrap(),
        fixture("get-board-path.v1.valid.json")
    );
}

#[tokio::test]
async fn get_board_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_board(&test, "contract-get", "Contract Get", None)?;
    let path: kanban_contract::GetBoardPath =
        serde_json::from_value(fixture("get-board-path.v1.valid.json"))?;
    let (status, response) =
        get_json(test.router(), &format!("/api/v1/boards/{}", path.board)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["data"]["slug"], "contract-get");
    Ok(())
}

#[test]
fn archive_board_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::ArchiveBoardPath {
            board: "contract-archive".into(),
        })
        .unwrap(),
        fixture("archive-board-path.v1.valid.json")
    );
}

#[tokio::test]
async fn archive_board_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_board(&test, "contract-archive", "Contract Archive", None)?;
    let path: kanban_contract::ArchiveBoardPath =
        serde_json::from_value(fixture("archive-board-path.v1.valid.json"))?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/boards/{}/archive", path.board),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(response["data"]["archived_at"].is_number());
    Ok(())
}

#[tokio::test]
async fn list_boards_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_board(
        &test,
        "contract-list",
        "Contract List",
        Some("Listed through the canonical response"),
    )?;
    let (status, mut response) = get_json(test.router(), "/api/v1/boards").await?;
    assert_eq!(status, StatusCode::OK);
    let boards = response["data"].as_array_mut().context("boards")?;
    boards.sort_by_key(|board| board["slug"].as_str().unwrap().to_owned());
    normalize_board(&mut boards[0], "b_contract_list", 2, 2);
    normalize_board(&mut boards[1], "b_default", 1, 1);
    assert_eq!(response, fixture("list-boards-response.v1.valid.json"));
    Ok(())
}

#[test]
fn list_boards_response_fixture_is_consumed_by_contract_root() {
    let value = fixture("list-boards-response.v1.valid.json");
    let response: kanban_contract::ListBoardsResponse =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), value);
}

#[tokio::test]
async fn create_board_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let (status, mut response) = post_json(
        test.router(),
        "/api/v1/boards",
        fixture("create-board-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    normalize_board(&mut response["data"], "b_contract_created", 1, 1);
    assert_eq!(response, fixture("create-board-response.v1.valid.json"));
    Ok(())
}

#[test]
fn create_board_response_fixture_is_consumed_by_contract_root() {
    let value = fixture("create-board-response.v1.valid.json");
    let response: kanban_contract::CreateBoardResponse =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), value);
}

#[tokio::test]
async fn get_board_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_board(&test, "contract-get", "Contract Get", None)?;
    let (status, mut response) = get_json(test.router(), "/api/v1/boards/contract-get").await?;
    assert_eq!(status, StatusCode::OK);
    normalize_board(&mut response["data"], "b_contract_get", 1, 1);
    assert_eq!(response, fixture("get-board-response.v1.valid.json"));
    Ok(())
}

#[test]
fn get_board_response_fixture_is_consumed_by_contract_root() {
    let value = fixture("get-board-response.v1.valid.json");
    let response: kanban_contract::GetBoardResponse =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), value);
}

#[tokio::test]
async fn archive_board_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_board(&test, "contract-archive", "Contract Archive", None)?;
    let (status, mut response) = post_json(
        test.router(),
        "/api/v1/boards/contract-archive/archive",
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    normalize_board(&mut response["data"], "b_contract_archive", 1, 2);
    assert_eq!(response, fixture("archive-board-response.v1.valid.json"));
    Ok(())
}

#[test]
fn archive_board_response_fixture_is_consumed_by_contract_root() {
    let value = fixture("archive-board-response.v1.valid.json");
    let response: kanban_contract::ArchiveBoardResponse =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), value);
}

#[tokio::test]
async fn boards_keep_not_found_status_and_localized_message() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let (status, response) = get_json_with_accept_language(
        test.router(),
        "/api/v1/boards/missing-contract-board",
        "zh-CN",
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(response["error"]["code"], "not_found");
    assert!(
        response["error"]["message"]
            .as_str()
            .context("message")?
            .contains("未找到")
    );
    Ok(())
}

#[tokio::test]
async fn boards_invalid_request_fixtures_reach_real_extractors() -> anyhow::Result<()> {
    let list_query = fixture("list-boards-query.v1.invalid.json");
    let invalid_include_archived = list_query["include_archived"]
        .as_str()
        .context("bool fixture")?;
    let test = TestApp::new()?;
    let (status, response) = get_json(
        test.router(),
        &format!("/api/v1/boards?include_archived={invalid_include_archived}"),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["code"], "invalid_input");

    let test = TestApp::new()?;
    let (status, response) = post_json(
        test.router(),
        "/api/v1/boards",
        fixture("create-board-request.v1.invalid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["error"]["code"], "invalid_input");
    Ok(())
}
