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

fn normalized_graph(mut value: Value, fixture_name: &str) -> Value {
    let expected = fixture(fixture_name);
    value["data"]["meta"]["generated_at"] = expected["data"]["meta"]["generated_at"].clone();
    value["data"]["meta"]["active_statuses"] = expected["data"]["meta"]["active_statuses"].clone();
    if expected["data"].get("center_task_id").is_some() {
        value["data"]["center_task_id"] = expected["data"]["center_task_id"].clone();
    }
    let task = expected["data"]["nodes"]
        .as_array()
        .and_then(|n| n.first())
        .and_then(|n| n.get("task"))
        .cloned();
    if let Some(task) = task
        && let Some(nodes) = value["data"]["nodes"].as_array_mut()
    {
        for node in nodes {
            node["task"] = task.clone();
        }
    }
    value
}

#[test]
fn task_neighborhood_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::TaskNeighborhoodPath {
            task_id: "t_center".into()
        })
        .unwrap(),
        fixture("task-neighborhood-path.v1.valid.json")
    );
}

#[tokio::test]
async fn task_neighborhood_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let path: kanban_contract::TaskNeighborhoodPath =
        serde_json::from_value(fixture("task-neighborhood-path.v1.valid.json"))?;
    let (status, _) = get_json(
        TestApp::new()?.router(),
        &format!("/api/v1/tasks/{}/neighborhood", path.task_id),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[test]
fn task_neighborhood_query_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::TaskNeighborhoodQuery {
            depth: 1,
            limit_nodes: 250,
            include_archived_context: false,
        })
        .unwrap(),
        fixture("task-neighborhood-query.v1.valid.json")
    );
}

#[tokio::test]
async fn task_neighborhood_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "fixture", "graph")?;
    let query: kanban_contract::TaskNeighborhoodQuery =
        serde_json::from_value(fixture("task-neighborhood-query.v1.valid.json"))?;
    let uri = format!(
        "/api/v1/tasks/{}/neighborhood?depth={}&limit_nodes={}&include_archived_context={}",
        task.id, query.depth, query.limit_nodes, query.include_archived_context
    );
    let (status, _) = get_json(test.router(), &uri).await?;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[test]
fn board_task_map_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::BoardTaskMapPath {
            board: "default".into()
        })
        .unwrap(),
        fixture("board-task-map-path.v1.valid.json")
    );
}

#[tokio::test]
async fn board_task_map_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let path: kanban_contract::BoardTaskMapPath =
        serde_json::from_value(fixture("board-task-map-path.v1.valid.json"))?;
    let (status, _) = get_json(
        TestApp::new()?.router(),
        &format!("/api/v1/boards/{}/task-map", path.board),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[test]
fn board_task_map_query_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::BoardTaskMapQuery {
            active_only: true,
            context_depth: 1,
            limit_nodes: 250,
            include_done_context: true,
            include_archived_context: false,
            hide_isolated: false,
        })
        .unwrap(),
        fixture("board-task-map-query.v1.valid.json")
    );
}

#[tokio::test]
async fn board_task_map_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let query: kanban_contract::BoardTaskMapQuery =
        serde_json::from_value(fixture("board-task-map-query.v1.valid.json"))?;
    let uri = format!(
        "/api/v1/boards/default/task-map?active_only={}&context_depth={}&limit_nodes={}&include_done_context={}&include_archived_context={}&hide_isolated={}",
        query.active_only,
        query.context_depth,
        query.limit_nodes,
        query.include_done_context,
        query.include_archived_context,
        query.hide_isolated
    );
    let (status, _) = get_json(TestApp::new()?.router(), &uri).await?;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn task_neighborhood_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "fixture", "graph")?;
    let (status, value) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{}/neighborhood?depth=1", task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        normalized_graph(value, "task-neighborhood-response.v1.valid.json"),
        fixture("task-neighborhood-response.v1.valid.json")
    );
    Ok(())
}

#[test]
fn task_neighborhood_response_fixture_is_consumed_by_contract_root() {
    let value = fixture("task-neighborhood-response.v1.valid.json");
    let parsed: kanban_contract::TaskNeighborhoodResponse =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
}

#[tokio::test]
async fn board_task_map_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let _task = create_ready_task_for_test(test.db_path(), "default", "fixture", "graph")?;
    let (status, value) = get_json(
        test.router(),
        "/api/v1/boards/default/task-map?active_only=true&context_depth=1",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        normalized_graph(value, "board-task-map-response.v1.valid.json"),
        fixture("board-task-map-response.v1.valid.json")
    );
    Ok(())
}

#[test]
fn board_task_map_response_fixture_is_consumed_by_contract_root() {
    let value = fixture("board-task-map-response.v1.valid.json");
    let parsed: kanban_contract::BoardTaskMapResponse =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
}
