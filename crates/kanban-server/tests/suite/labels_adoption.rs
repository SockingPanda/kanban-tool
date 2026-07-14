use crate::common::*;
use std::{fs, path::PathBuf};

fn fx(name: &str) -> serde_json::Value {
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

fn normalize_label(value: &mut serde_json::Value) {
    value["id"] = json!("l_fixture");
    value["board_id"] = json!("b_fixture");
    value["created_at"] = json!(1);
    value["updated_at"] = json!(2);
}
fn normalize_task(value: &mut serde_json::Value) {
    value["id"] = json!("task-fixture");
    value["board_id"] = json!("board-fixture");
    value["ref"] = json!("default#1");
    value["created_at"] = json!(1);
    value["updated_at"] = json!(2);
    for label in value["labels"].as_array_mut().unwrap() {
        normalize_label(label);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct LabelMutationSnapshot {
    labels: i64,
    task_labels: i64,
    events: i64,
    task_version: (i64, i64),
}

fn label_mutation_snapshot(
    path: &std::path::Path,
    task_id: &str,
) -> anyhow::Result<LabelMutationSnapshot> {
    let conn = kanban_test_support::connect_file(path)?;
    let count = |table: &str| -> anyhow::Result<i64> {
        Ok(
            conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })?,
        )
    };
    let task_version = conn.query_row(
        "SELECT updated_at, lock_version FROM tasks WHERE id = ?1",
        [task_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(LabelMutationSnapshot {
        labels: count("labels")?,
        task_labels: count("task_labels")?,
        events: count("task_events")?,
        task_version,
    })
}

async fn assert_rejected_without_label_side_effects(
    app: axum::Router,
    db: &std::path::Path,
    task_id: &str,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> anyhow::Result<()> {
    let before = label_mutation_snapshot(db, task_id)?;
    let (status, _) = request_json(app, method, uri, body, None).await?;
    assert!(
        status.is_client_error(),
        "unexpected status {status} for {method} {uri}"
    );
    assert_eq!(
        label_mutation_snapshot(db, task_id)?,
        before,
        "side effect for {method} {uri}"
    );
    Ok(())
}

async fn produced() -> anyhow::Result<(serde_json::Value, serde_json::Value, serde_json::Value)> {
    let test = TestApp::new()?;
    let db = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db,
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("label fixture"),
    )?;
    let app = test.router();
    let (status, mut add) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/labels", task.id),
        fx("add-task-label-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(add["data"]["labels"][0]["name"], "后端-api");
    let label_id = add["data"]["labels"][0]["id"].as_str().unwrap().to_owned();
    normalize_task(&mut add["data"]);
    for item in add["meta"]["created_labels"].as_array_mut().unwrap() {
        normalize_label(item);
    }
    let (status, mut list) =
        get_json(app.clone(), &format!("/api/v1/tasks/{}/labels", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    for item in list["data"].as_array_mut().unwrap() {
        normalize_label(item);
    }
    let (status, mut remove) = delete_json(
        app,
        &format!("/api/v1/tasks/{}/labels/{}", task.id, label_id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    normalize_task(&mut remove["data"]);
    Ok((list, add, remove))
}

macro_rules! dto_path_test {
    ($name:ident,$ty:ty,$fixture:literal,$value:expr) => {
        #[test]
        fn $name() {
            assert_eq!(serde_json::to_value($value).unwrap(), fx($fixture));
        }
    };
}
dto_path_test!(
    list_task_labels_path_dto_serializes_to_committed_fixture,
    kanban_contract::ListTaskLabelsPath,
    "list-task-labels-path.v1.valid.json",
    kanban_contract::ListTaskLabelsPath {
        task_id: "t_fixture".into()
    }
);
dto_path_test!(
    add_task_label_path_dto_serializes_to_committed_fixture,
    kanban_contract::AddTaskLabelPath,
    "add-task-label-path.v1.valid.json",
    kanban_contract::AddTaskLabelPath {
        task_id: "t_fixture".into()
    }
);
dto_path_test!(
    remove_task_label_path_dto_serializes_to_committed_fixture,
    kanban_contract::RemoveTaskLabelPath,
    "remove-task-label-path.v1.valid.json",
    kanban_contract::RemoveTaskLabelPath {
        task_id: "t_fixture".into(),
        label_id: "l_fixture".into()
    }
);

#[test]
fn add_task_label_request_dto_serializes_to_committed_fixture() {
    let dto = kanban_contract::AddTaskLabelRequest {
        name: None,
        names: Some(vec!["后端-api".into()]),
        create_missing: true,
        actor: Some("fixture-agent".into()),
    };
    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        fx("add-task-label-request.v1.valid.json")
    );
}

#[tokio::test]
async fn list_task_labels_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::ListTaskLabelsPath =
        serde_json::from_value(fx("list-task-labels-path.v1.valid.json"))?;
    let (s, b) = get_json(
        TestApp::new()?.router(),
        &format!("/api/v1/tasks/{}/labels", p.task_id),
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(b["error"]["code"], "not_found");
    Ok(())
}
#[tokio::test]
async fn add_task_label_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::AddTaskLabelPath =
        serde_json::from_value(fx("add-task-label-path.v1.valid.json"))?;
    let (s, b) = post_json(
        TestApp::new()?.router(),
        &format!("/api/v1/tasks/{}/labels", p.task_id),
        json!({"name":"x"}),
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(b["error"]["code"], "not_found");
    Ok(())
}
#[tokio::test]
async fn remove_task_label_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::RemoveTaskLabelPath =
        serde_json::from_value(fx("remove-task-label-path.v1.valid.json"))?;
    let (s, b) = delete_json(
        TestApp::new()?.router(),
        &format!("/api/v1/tasks/{}/labels/{}", p.task_id, p.label_id),
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(b["error"]["code"], "not_found");
    Ok(())
}
#[tokio::test]
async fn add_task_label_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let body: kanban_contract::AddTaskLabelRequest =
        serde_json::from_value(fx("add-task-label-request.v1.valid.json"))?;
    let (s, b) = post_json(
        TestApp::new()?.router(),
        "/api/v1/tasks/t_fixture/labels",
        serde_json::to_value(body)?,
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(b["error"]["code"], "not_found");
    Ok(())
}

#[tokio::test]
async fn list_task_labels_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        produced().await?.0,
        fx("list-task-labels-response.v1.valid.json")
    );
    Ok(())
}
#[tokio::test]
async fn add_task_label_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        produced().await?.1,
        fx("add-task-label-response.v1.valid.json")
    );
    Ok(())
}
#[tokio::test]
async fn remove_task_label_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        produced().await?.2,
        fx("remove-task-label-response.v1.valid.json")
    );
    Ok(())
}
#[test]
fn list_task_labels_response_fixture_is_consumed_by_contract_root() {
    let v = fx("list-task-labels-response.v1.valid.json");
    let d: kanban_contract::ListTaskLabelsResponse = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(d).unwrap(), v)
}
#[test]
fn add_task_label_response_fixture_is_consumed_by_contract_root() {
    let v = fx("add-task-label-response.v1.valid.json");
    let d: kanban_contract::AddTaskLabelResponse = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(d).unwrap(), v)
}
#[test]
fn remove_task_label_response_fixture_is_consumed_by_contract_root() {
    let v = fx("remove-task-label-response.v1.valid.json");
    let d: kanban_contract::RemoveTaskLabelResponse = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(d).unwrap(), v)
}

#[tokio::test]
async fn business_invalid_label_mutations_leave_database_and_task_version_unchanged()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db = test.db_path().to_path_buf();
    let task = kanban_sqlite::api::create_task(
        &db,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("invalid label target"),
    )?;
    let app = test.router();
    let uri = format!("/api/v1/tasks/{}/labels", task.id);
    for body in [
        json!({}),
        json!({"name":"one","names":["two"]}),
        json!({"names":[]}),
        json!({"name":"missing","create_missing":false}),
    ] {
        assert_rejected_without_label_side_effects(
            app.clone(),
            &db,
            &task.id,
            "POST",
            &uri,
            Some(body),
        )
        .await?;
    }
    assert_rejected_without_label_side_effects(
        app.clone(),
        &db,
        &task.id,
        "DELETE",
        &format!("{uri}/l_missing"),
        None,
    )
    .await?;

    kanban_sqlite::api::archive_task(&db, "default", "seed", &task.id, false)?;
    assert_rejected_without_label_side_effects(
        app.clone(),
        &db,
        &task.id,
        "POST",
        &uri,
        Some(json!({"name":"x","create_missing":true})),
    )
    .await?;
    assert_rejected_without_label_side_effects(
        app.clone(),
        &db,
        &task.id,
        "DELETE",
        &format!("{uri}/l_missing"),
        None,
    )
    .await?;

    let board_task = kanban_sqlite::api::create_task(
        &db,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("archived board target"),
    )?;
    kanban_sqlite::api::archive_board(&db, "default", "seed")?;
    let board_uri = format!("/api/v1/tasks/{}/labels", board_task.id);
    assert_rejected_without_label_side_effects(
        app.clone(),
        &db,
        &board_task.id,
        "POST",
        &board_uri,
        Some(json!({"name":"x","create_missing":true})),
    )
    .await?;
    assert_rejected_without_label_side_effects(
        app,
        &db,
        &board_task.id,
        "DELETE",
        &format!("{board_uri}/l_missing"),
        None,
    )
    .await?;
    Ok(())
}
