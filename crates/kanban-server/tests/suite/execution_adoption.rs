use crate::common::*;
use std::{fs, path::PathBuf};

fn fx(name: &str) -> Value {
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
fn normalize_task(task: &mut Value, id: &str, seq: i64, title: &str) {
    task["id"] = json!(id);
    task["board_id"] = json!("b_fixture");
    task["board_slug"] = json!("default");
    task["ref"] = json!(format!("default#{seq}"));
    task["seq"] = json!(seq);
    task["title"] = json!(title);
    task["position"] = json!(seq * 1024);
    task["created_at"] = json!(1);
    task["updated_at"] = json!(2);
    task["lock_version"] = json!(0);
}
fn normalize_compact(task: &mut Value, id: &str, seq: i64, title: &str) {
    task["id"] = json!(id);
    task["board_id"] = json!("b_fixture");
    task["board_slug"] = json!("default");
    task["ref"] = json!(format!("default#{seq}"));
    task["title"] = json!(title);
}
fn normalize_dependencies(value: &mut Value) {
    normalize_compact(&mut value["data"]["task"], "t_child", 2, "child fixture");
    for parent in value["data"]["parents"].as_array_mut().unwrap() {
        normalize_task(parent, "t_parent", 1, "parent fixture");
    }
    for child in value["data"]["children"].as_array_mut().unwrap() {
        normalize_task(child, "t_child", 2, "child fixture");
    }
    for edge in value["data"]["edges"].as_array_mut().unwrap() {
        normalize_compact(&mut edge["parent"], "t_parent", 1, "parent fixture");
        normalize_compact(&mut edge["child"], "t_child", 2, "child fixture");
    }
}
async fn dependency_values() -> anyhow::Result<(Value, Value, Value)> {
    let test = TestApp::new()?;
    let parent = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("parent fixture"),
    )?;
    let child = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("child fixture"),
    )?;
    let app = test.router();
    let (status, mut add) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", child.id),
        json!({"parent_task_id":parent.id,"actor":"fixture"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let (status, mut list) = get_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", child.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let (status, mut remove) = request_json(
        app,
        "DELETE",
        &format!("/api/v1/tasks/{}/dependencies/{}", child.id, parent.id),
        None,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    normalize_dependencies(&mut add);
    normalize_dependencies(&mut list);
    normalize_dependencies(&mut remove);
    Ok((list, add, remove))
}
async fn plan_value() -> anyhow::Result<Value> {
    let test = TestApp::new()?;
    let task = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("plan fixture"),
    )?;
    let (status, mut value) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/execution-plan/not-required", task.id),
        json!({"reason":"manual execution","actor":"fixture"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    value["data"]["board_id"] = json!("b_fixture");
    value["data"]["task_id"] = json!("t_plan");
    value["data"]["updated_at"] = json!(1);
    Ok(value)
}
async fn columns_value() -> anyhow::Result<Value> {
    let test = TestApp::new()?;
    let (status, mut value) = get_json(test.router(), "/api/v1/boards/default/columns").await?;
    assert_eq!(status, StatusCode::OK);
    for (i, column) in value["data"].as_array_mut().unwrap().iter_mut().enumerate() {
        column["id"] = json!(format!("col_{i}"));
        column["board_id"] = json!("b_fixture");
        column["created_at"] = json!(1);
        column["updated_at"] = json!(1);
    }
    Ok(value)
}
async fn run_log_value() -> anyhow::Result<Value> {
    let test = TestApp::new()?;
    let _task =
        create_ready_task_for_test(test.db_path(), "default", "fixture", "run log fixture")?;
    let dispatched = kanban_sqlite::api::dispatch_once(
        test.db_path(),
        "default",
        kanban_sqlite::api::DispatchOptions {
            actor: "fixture".into(),
            command: "printf fixture-log".into(),
            worker_profile: "manual".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::api::FinishPolicy::Done,
            on_failure: kanban_sqlite::api::FinishPolicy::Blocked,
            log_dir: test.dir_path().join("logs"),
        },
    )?;
    let run_id = dispatched.run_id.context("dispatch run id")?;
    let (status, mut value) =
        get_json(test.router(), &format!("/api/v1/runs/{run_id}/log")).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(value["data"].get("claim_token").is_none());
    assert!(value["data"].get("log_path").is_none());
    assert_eq!(value["data"]["content"], json!("fixture-log"));
    assert_eq!(value["data"]["truncated"], json!(false));
    value["data"]["run_id"] = json!("r_fixture");
    Ok(value)
}

macro_rules! response_consumer {
    ($name:ident,$ty:ty,$file:literal) => {
        #[test]
        fn $name() {
            let v = fx($file);
            let dto: $ty = serde_json::from_value(v.clone()).unwrap();
            assert_eq!(serde_json::to_value(dto).unwrap(), v);
        }
    };
}

#[test]
fn list_dependencies_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::ListDependenciesPath {
            task_id: "t_child".into()
        })
        .unwrap(),
        fx("list-dependencies-path.v1.valid.json")
    );
}
#[tokio::test]
async fn list_dependencies_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::ListDependenciesPath =
        serde_json::from_value(fx("list-dependencies-path.v1.valid.json"))?;
    let t = TestApp::new()?;
    let (s, _) = get_json(
        t.router(),
        &format!("/api/v1/tasks/{}/dependencies", p.task_id),
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    Ok(())
}
#[test]
fn add_dependency_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::AddDependencyPath {
            task_id: "t_child".into()
        })
        .unwrap(),
        fx("add-dependency-path.v1.valid.json")
    );
}
#[tokio::test]
async fn add_dependency_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::AddDependencyPath =
        serde_json::from_value(fx("add-dependency-path.v1.valid.json"))?;
    let t = TestApp::new()?;
    let (s, _) = post_json(
        t.router(),
        &format!("/api/v1/tasks/{}/dependencies", p.task_id),
        json!({"parent_task_id":"t_parent"}),
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    Ok(())
}
#[test]
fn remove_dependency_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::RemoveDependencyPath {
            child_task_id: "t_child".into(),
            parent_task_id: "t_parent".into()
        })
        .unwrap(),
        fx("remove-dependency-path.v1.valid.json")
    );
}
#[tokio::test]
async fn remove_dependency_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::RemoveDependencyPath =
        serde_json::from_value(fx("remove-dependency-path.v1.valid.json"))?;
    let t = TestApp::new()?;
    let (s, _) = request_json(
        t.router(),
        "DELETE",
        &format!(
            "/api/v1/tasks/{}/dependencies/{}",
            p.child_task_id, p.parent_task_id
        ),
        None,
        None,
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    Ok(())
}
#[test]
fn mark_plan_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::MarkExecutionPlanNotRequiredPath {
            task_id: "t_plan".into()
        })
        .unwrap(),
        fx("mark-execution-plan-not-required-path.v1.valid.json")
    );
}
#[tokio::test]
async fn mark_plan_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::MarkExecutionPlanNotRequiredPath =
        serde_json::from_value(fx("mark-execution-plan-not-required-path.v1.valid.json"))?;
    let t = TestApp::new()?;
    let (s, _) = post_json(
        t.router(),
        &format!("/api/v1/tasks/{}/execution-plan/not-required", p.task_id),
        fx("mark-execution-plan-not-required-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    Ok(())
}
#[test]
fn mark_plan_request_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::MarkExecutionPlanNotRequiredRequest {
            reason: "manual execution".into(),
            actor: Some("fixture".into())
        })
        .unwrap(),
        fx("mark-execution-plan-not-required-request.v1.valid.json")
    );
}
#[tokio::test]
async fn mark_plan_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        plan_value().await?,
        fx("mark-execution-plan-not-required-response.v1.valid.json")
    );
    Ok(())
}
#[test]
fn get_run_log_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::GetRunLogPath {
            run_id: "r_fixture".into()
        })
        .unwrap(),
        fx("get-run-log-path.v1.valid.json")
    );
}
#[tokio::test]
async fn get_run_log_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::GetRunLogPath =
        serde_json::from_value(fx("get-run-log-path.v1.valid.json"))?;
    let t = TestApp::new()?;
    let (s, _) = get_json(t.router(), &format!("/api/v1/runs/{}/log", p.run_id)).await?;
    assert_eq!(s, StatusCode::NOT_FOUND);
    Ok(())
}
#[test]
fn list_board_columns_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::ListBoardColumnsPath {
            board: "default".into()
        })
        .unwrap(),
        fx("list-board-columns-path.v1.valid.json")
    );
}
#[tokio::test]
async fn list_board_columns_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let p: kanban_contract::ListBoardColumnsPath =
        serde_json::from_value(fx("list-board-columns-path.v1.valid.json"))?;
    let t = TestApp::new()?;
    let (s, b) = get_json(t.router(), &format!("/api/v1/boards/{}/columns", p.board)).await?;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["data"].as_array().unwrap().len(), 9);
    Ok(())
}

#[tokio::test]
async fn list_dependencies_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        dependency_values().await?.0,
        fx("list-dependencies-response.v1.valid.json")
    );
    Ok(())
}
response_consumer!(
    list_dependencies_response_fixture_is_consumed_by_contract_root,
    kanban_contract::ListDependenciesResponse,
    "list-dependencies-response.v1.valid.json"
);
#[tokio::test]
async fn add_dependency_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        dependency_values().await?.1,
        fx("add-dependency-response.v1.valid.json")
    );
    Ok(())
}
response_consumer!(
    add_dependency_response_fixture_is_consumed_by_contract_root,
    kanban_contract::AddDependencyResponse,
    "add-dependency-response.v1.valid.json"
);
#[tokio::test]
async fn remove_dependency_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        dependency_values().await?.2,
        fx("remove-dependency-response.v1.valid.json")
    );
    Ok(())
}
response_consumer!(
    remove_dependency_response_fixture_is_consumed_by_contract_root,
    kanban_contract::RemoveDependencyResponse,
    "remove-dependency-response.v1.valid.json"
);
#[tokio::test]
async fn mark_plan_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        plan_value().await?,
        fx("mark-execution-plan-not-required-response.v1.valid.json")
    );
    Ok(())
}
response_consumer!(
    mark_plan_response_fixture_is_consumed_by_contract_root,
    kanban_contract::MarkExecutionPlanNotRequiredResponse,
    "mark-execution-plan-not-required-response.v1.valid.json"
);
#[tokio::test]
async fn get_run_log_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        run_log_value().await?,
        fx("get-run-log-response.v1.valid.json")
    );
    Ok(())
}
response_consumer!(
    get_run_log_response_fixture_is_consumed_by_contract_root,
    kanban_contract::GetRunLogResponse,
    "get-run-log-response.v1.valid.json"
);
#[tokio::test]
async fn list_board_columns_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(
        columns_value().await?,
        fx("list-board-columns-response.v1.valid.json")
    );
    Ok(())
}
response_consumer!(
    list_board_columns_response_fixture_is_consumed_by_contract_root,
    kanban_contract::ListBoardColumnsResponse,
    "list-board-columns-response.v1.valid.json"
);

#[test]
fn execution_handlers_use_exact_contract_roots_and_do_not_expose_private_records() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/handlers");
    let sources = ["dependencies.rs", "steps.rs", "runs.rs", "boards.rs"]
        .map(|p| fs::read_to_string(root.join(p)).unwrap())
        .join("\n");
    for required in [
        "ListDependenciesResponse",
        "AddDependencyResponse",
        "RemoveDependencyResponse",
        "MarkExecutionPlanNotRequiredResponse",
        "GetRunLogResponse",
        "ListBoardColumnsResponse",
    ] {
        assert!(sources.contains(required), "missing {required}");
    }
    for forbidden in [
        "RunLogDto",
        "DependenciesDto",
        "DataEnvelope<RunLogDto>",
        "BoardColumnRecord>>",
    ] {
        assert!(
            !sources.contains(forbidden),
            "private owner leaked: {forbidden}"
        );
    }
}

#[tokio::test]
async fn dependency_cycle_and_invalid_plan_body_remain_transactional() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let first = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("cycle first"),
    )?;
    let second = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("cycle second"),
    )?;
    let app = test.router();
    let (status, _) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", second.id),
        json!({"parent_task_id":first.id,"actor":"fixture"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let before = kanban_sqlite::api::list_dependencies(test.db_path(), "default", &second.id)?;
    let (status, error) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", first.id),
        json!({"parent_task_id":second.id,"actor":"fixture"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["error"]["code"], "dependency_cycle");
    assert_eq!(
        kanban_sqlite::api::list_dependencies(test.db_path(), "default", &second.id)?,
        before
    );
    let (status, error) = post_json(
        app,
        &format!("/api/v1/tasks/{}/execution-plan/not-required", second.id),
        json!({"reason":"manual","actor":"fixture","extra":true}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["error"]["code"], "invalid_input");
    assert_eq!(
        kanban_sqlite::api::execution_plan(test.db_path(), "default", &second.id)?.state,
        kanban_sqlite::api::StepPlanState::Unplanned
    );
    Ok(())
}

#[tokio::test]
async fn remove_dependency_keeps_actor_header_and_event_semantics() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let parent = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("actor parent"),
    )?;
    let child = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("actor child"),
    )?;
    kanban_sqlite::api::add_dependency(
        test.db_path(),
        "default",
        "fixture",
        &parent.id,
        &child.id,
    )?;
    let (status, _) = request_json(
        test.router(),
        "DELETE",
        &format!("/api/v1/tasks/{}/dependencies/{}", child.id, parent.id),
        None,
        Some("header-actor"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let events = kanban_sqlite::api::list_events(test.db_path(), "default", Some(&child.id))?;
    let removed = events
        .iter()
        .find(|event| event.kind == "dependency.removed")
        .context("dependency.removed")?;
    assert_eq!(removed.actor.as_deref(), Some("header-actor"));
    Ok(())
}
