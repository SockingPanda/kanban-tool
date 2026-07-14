use crate::common::*;
use quote::ToTokens;
use std::{fs, path::PathBuf};

fn handler_signature(name: &str) -> String {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers/tasks.rs"),
    )
    .unwrap();
    let file = syn::parse_file(&source).unwrap();
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == name => {
                Some(function.sig.to_token_stream().to_string())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing handler {name}"))
}

#[test]
fn task_core_handlers_use_contract_owned_boundary_types() {
    let get = handler_signature("get_task");
    for required in ["GetTaskPath", "GetTaskQuery", "GetTaskResponse"] {
        assert!(get.contains(required), "get_task lacks {required}: {get}");
    }
    assert!(!get.contains("TaskGetQuery"), "{get}");
    assert!(!get.contains("kanban_sqlite"), "{get}");

    let update = handler_signature("update_task");
    for required in ["UpdateTaskPath", "UpdateTaskRequest", "UpdateTaskResponse"] {
        assert!(
            update.contains(required),
            "update_task lacks {required}: {update}"
        );
    }
    assert!(!update.contains("serde_json :: Value"), "{update}");
}

#[tokio::test]
async fn get_task_query_rejects_unknown_transport_fields() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "fixture", "query guard")?;
    let (status, response) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{}?unknown=true", task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert_eq!(response["error"]["code"], "invalid_input");
    Ok(())
}

#[tokio::test]
async fn update_task_contract_preserves_missing_vs_null_and_service_guard() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "fixture", "before")?;
    let uri = format!("/api/v1/tasks/{}", task.id);

    let (status, updated) = patch_json(
        test.router(),
        &uri,
        json!({
            "title": "after",
            "description": null,
            "expected_lock_version": task.lock_version
        }),
        Some("header-actor"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["data"]["title"], "after");
    assert_eq!(updated["data"]["description"], Value::Null);
    assert_eq!(updated["data"]["created_by"], "fixture");

    let (status, response) = patch_json(
        test.router(),
        &uri,
        json!({
            "title": "stale",
            "expected_lock_version": task.lock_version
        }),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert_eq!(response["error"]["code"], "invalid_input");
    Ok(())
}

#[tokio::test]
async fn update_task_contract_rejects_private_and_unknown_fields() -> anyhow::Result<()> {
    for body in [
        json!({"status":"done"}),
        json!({"claim_token":"private"}),
        json!({"unknown":true}),
    ] {
        let (status, response) = patch_json(
            TestApp::new()?.router(),
            "/api/v1/tasks/t_missing",
            body,
            None,
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
        assert_eq!(response["error"]["code"], "invalid_input");
    }
    Ok(())
}

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

fn normalize_task_response(mut value: Value) -> Value {
    let data = value["data"].as_object_mut().unwrap();
    data.insert("id".into(), json!("t_fixture"));
    data.insert("board_id".into(), json!("b_project"));
    data.insert("ref".into(), json!("project#2"));
    data.insert("created_at".into(), json!(1));
    data.insert("updated_at".into(), json!(1));
    for label in data["labels"].as_array_mut().unwrap() {
        let label = label.as_object_mut().unwrap();
        label.insert("id".into(), json!("l_core"));
        label.insert("board_id".into(), json!("b_project"));
        label.insert("created_at".into(), json!(1));
        label.insert("updated_at".into(), json!(1));
    }
    value
}

async fn create_fixture_task() -> anyhow::Result<(TestApp, String)> {
    let test = TestApp::new()?;
    let db = test.db_path().to_path_buf();
    kanban_sqlite::api::create_board(
        &db,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )?;
    kanban_sqlite::api::create_label(
        &db,
        "project",
        kanban_sqlite::api::CreateLabel {
            name: "core".into(),
            color: None,
        },
    )?;
    kanban_sqlite::api::create_task(
        &db,
        "project",
        "seed",
        kanban_sqlite::api::CreateTask::ready("parent"),
    )?;
    let (status, response) = post_json(
        test.router(),
        "/api/v1/boards/project/tasks",
        fixture("create-task-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    Ok((test, response["data"]["id"].as_str().unwrap().to_owned()))
}

#[test]
fn get_task_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::GetTaskPath {
            task_id: "t_fixture".into()
        })
        .unwrap(),
        fixture("get-task-path.v1.valid.json")
    );
}

#[tokio::test]
async fn get_task_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let path: kanban_contract::GetTaskPath =
        serde_json::from_value(fixture("get-task-path.v1.valid.json"))?;
    let (status, response) = get_json(
        TestApp::new()?.router(),
        &format!("/api/v1/tasks/{}", path.task_id),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{response}");
    Ok(())
}

#[test]
fn get_task_query_dto_serializes_to_committed_fixture() {
    let query: kanban_contract::GetTaskQuery =
        serde_json::from_value(fixture("get-task-query.v1.valid.json")).unwrap();
    assert_eq!(
        serde_json::to_value(query).unwrap(),
        fixture("get-task-query.v1.valid.json")
    );
}

#[tokio::test]
async fn get_task_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, task_id) = create_fixture_task().await?;
    let query: kanban_contract::GetTaskQuery =
        serde_json::from_value(fixture("get-task-query.v1.valid.json"))?;
    let (status, response) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{task_id}?include={}", query.include.unwrap()),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert!(response.get("meta").is_some(), "{response}");
    Ok(())
}

#[tokio::test]
async fn get_task_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let (test, task_id) = create_fixture_task().await?;
    let (status, response) = get_json(test.router(), &format!("/api/v1/tasks/{task_id}")).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        normalize_task_response(response),
        fixture("get-task-response.v1.valid.json")
    );
    Ok(())
}

#[test]
fn get_task_response_fixture_is_consumed_by_contract_root() {
    let valid = fixture("get-task-response.v1.valid.json");
    let response: kanban_contract::GetTaskResponse = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), valid);
    assert!(
        serde_json::from_value::<kanban_contract::GetTaskResponse>(fixture(
            "get-task-response.v1.invalid.json"
        ))
        .is_err()
    );
}

#[test]
fn update_task_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::UpdateTaskPath {
            task_id: "t_fixture".into()
        })
        .unwrap(),
        fixture("update-task-path.v1.valid.json")
    );
}

#[tokio::test]
async fn update_task_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let path: kanban_contract::UpdateTaskPath =
        serde_json::from_value(fixture("update-task-path.v1.valid.json"))?;
    let (status, response) = patch_json(
        TestApp::new()?.router(),
        &format!("/api/v1/tasks/{}", path.task_id),
        fixture("update-task-request.v1.valid.json"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{response}");
    Ok(())
}

#[test]
fn update_task_request_dto_serializes_to_committed_fixture() {
    let valid = fixture("update-task-request.v1.valid.json");
    let request: kanban_contract::UpdateTaskRequest =
        serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), valid);
}

#[tokio::test]
async fn update_task_rejects_explicit_null_for_non_nullable_patch_fields() -> anyhow::Result<()> {
    for field in [
        "title",
        "priority",
        "metadata_json",
        "expected_lock_version",
    ] {
        let (test, task_id) = create_fixture_task().await?;
        let (status, response) = patch_json(
            test.router(),
            &format!("/api/v1/tasks/{task_id}"),
            json!({field: null}),
            None,
        )
        .await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{field}: {response}");
        assert_eq!(response["error"]["code"], "invalid_input", "{field}");
    }

    let (test, task_id) = create_fixture_task().await?;
    let (status, response) = patch_json(
        test.router(),
        &format!("/api/v1/tasks/{task_id}"),
        json!({
            "metadata": {"source": "must-not-apply"},
            "metadata_json": null
        }),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{response}");
    assert_eq!(response["error"]["code"], "invalid_input");
    Ok(())
}

#[tokio::test]
async fn update_task_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, task_id) = create_fixture_task().await?;
    let (status, response) = patch_json(
        test.router(),
        &format!("/api/v1/tasks/{task_id}"),
        fixture("update-task-request.v1.valid.json"),
        Some("ignored-header-actor"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["title"], "Updated contract task");
    assert_eq!(response["data"]["description"], Value::Null);
    assert_eq!(response["data"]["metadata"], json!({"source":"fixture"}));
    Ok(())
}

#[tokio::test]
async fn update_task_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let (test, task_id) = create_fixture_task().await?;
    let (status, response) = patch_json(
        test.router(),
        &format!("/api/v1/tasks/{task_id}"),
        fixture("update-task-request.v1.valid.json"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        normalize_task_response(response),
        fixture("update-task-response.v1.valid.json")
    );
    Ok(())
}

#[test]
fn update_task_response_fixture_is_consumed_by_contract_root() {
    let valid = fixture("update-task-response.v1.valid.json");
    let response: kanban_contract::UpdateTaskResponse =
        serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), valid);
    assert!(
        serde_json::from_value::<kanban_contract::UpdateTaskResponse>(fixture(
            "update-task-response.v1.invalid.json"
        ))
        .is_err()
    );
}
