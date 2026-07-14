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

fn normalize_list(mut value: serde_json::Value) -> serde_json::Value {
    let runs = value["data"].as_array_mut().unwrap();
    for run in runs.iter_mut() {
        let running = run["status"] == "running";
        run["id"] = json!(if running { "r_active" } else { "r_finished" });
        run["task_id"] = json!("t_fixture");
        run["started_at"] = json!(if running { 2 } else { 1 });
        if !running {
            run["finished_at"] = json!(1);
        }
    }
    runs.sort_by_key(|run| run["status"] != "running");
    value
}

fn normalize_get(mut value: serde_json::Value) -> serde_json::Value {
    let run = value["data"].as_object_mut().unwrap();
    run.insert("id".into(), json!("r_finished"));
    run.insert("task_id".into(), json!("t_fixture"));
    run.insert("started_at".into(), json!(1));
    run.insert("finished_at".into(), json!(1));
    value
}

async fn produced() -> anyhow::Result<(serde_json::Value, serde_json::Value)> {
    let test = TestApp::new()?;
    let db = test.db_path().to_path_buf();
    kanban_sqlite::api::create_board(
        &db,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "runs-project".into(),
            name: "Runs Project".into(),
            description: None,
        },
    )?;
    let task = create_ready_task_for_test(&db, "runs-project", "seed", "runs fixture")?;
    kanban_sqlite::api::mark_execution_plan_not_required(
        &db,
        "runs-project",
        "seed",
        &task.id,
        "fixture",
    )?;
    let dispatched = kanban_sqlite::api::dispatch_once(
        &db,
        "runs-project",
        kanban_sqlite::api::DispatchOptions {
            actor: "runner".into(),
            command: "printf fixture-log".into(),
            worker_profile: "manual".into(),
            claim_ttl_ms: 300_000,
            heartbeat_interval_ms: 10,
            on_success: kanban_sqlite::api::FinishPolicy::Done,
            on_failure: kanban_sqlite::api::FinishPolicy::Blocked,
            log_dir: test.dir_path().join("logs"),
        },
    )?;
    let finished_run_id = dispatched.run_id.context("dispatch run id")?;
    kanban_sqlite::api::reopen_task(&db, "runs-project", "runner", &task.id, "retry")?;
    let _second = kanban_sqlite::api::claim_task_with_profile_and_metadata(
        &db,
        "runs-project",
        "runner",
        &task.id,
        300_000,
        "manual",
        "{\"attempt\":2}",
    )?;
    let app = test.router();
    let (status, list) = get_json(app.clone(), &format!("/api/v1/tasks/{}/runs", task.id)).await?;
    assert_eq!(status, StatusCode::OK);
    let (status, get) = get_json(app, &format!("/api/v1/runs/{finished_run_id}")).await?;
    assert_eq!(status, StatusCode::OK);
    let listed = list["data"]
        .as_array()
        .context("list runs data")?
        .iter()
        .find(|run| run["id"] == finished_run_id)
        .context("finished run in list response")?;
    assert_eq!(listed, &get["data"]);
    assert_eq!(listed["has_log"], true);
    assert_eq!(get["data"]["has_log"], true);
    for value in [listed, &get["data"]] {
        assert!(value.get("log_path").is_none());
        assert!(value.get("claim_token").is_none());
    }
    let list = normalize_list(list);
    let get = normalize_get(get);
    assert_eq!(list["data"][1]["id"], get["data"]["id"]);
    assert_eq!(list["data"][1], get["data"]);
    Ok((list, get))
}

#[test]
fn list_runs_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::ListRunsPath {
            task_id: "t_fixture".into(),
        })
        .unwrap(),
        fx("list-runs-path.v1.valid.json")
    );
}

#[tokio::test]
async fn list_runs_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let path: kanban_contract::ListRunsPath =
        serde_json::from_value(fx("list-runs-path.v1.valid.json"))?;
    let (status, body) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{}/runs", path.task_id),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    Ok(())
}

#[test]
fn get_run_path_dto_serializes_to_committed_fixture() {
    assert_eq!(
        serde_json::to_value(kanban_contract::GetRunPath {
            run_id: "r_fixture".into(),
        })
        .unwrap(),
        fx("get-run-path.v1.valid.json")
    );
}

#[tokio::test]
async fn get_run_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let path: kanban_contract::GetRunPath =
        serde_json::from_value(fx("get-run-path.v1.valid.json"))?;
    let (status, body) = get_json(test.router(), &format!("/api/v1/runs/{}", path.run_id)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    Ok(())
}

#[tokio::test]
async fn list_runs_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(produced().await?.0, fx("list-runs-response.v1.valid.json"));
    Ok(())
}

#[test]
fn list_runs_response_fixture_is_consumed_by_contract_root() {
    let value = fx("list-runs-response.v1.valid.json");
    let dto: kanban_contract::ListRunsResponse = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(dto).unwrap(), value);
}

#[tokio::test]
async fn get_run_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    assert_eq!(produced().await?.1, fx("get-run-response.v1.valid.json"));
    Ok(())
}

#[test]
fn get_run_response_fixture_is_consumed_by_contract_root() {
    let value = fx("get-run-response.v1.valid.json");
    let dto: kanban_contract::GetRunResponse = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(dto).unwrap(), value);
}
