use crate::common::*;
use std::{collections::BTreeMap, fs, path::PathBuf};

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

fn normalized(mut value: serde_json::Value) -> serde_json::Value {
    fn walk(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if map.contains_key("execution_plan") {
                    map.insert("task_id".into(), json!("t_project_parent"));
                }
                if map.contains_key("board_id") && map.contains_key("state") {
                    map.insert("board_id".into(), json!("b_project"));
                    map.insert("task_id".into(), json!("t_project_parent"));
                }
                if map.contains_key("parent_task_id") && map.contains_key("resolution_note") {
                    map.insert("id".into(), json!("step_fixture"));
                    map.insert("parent_task_id".into(), json!("t_project_parent"));
                    map.insert("created_at".into(), json!(1));
                    map.insert("updated_at".into(), json!(1));
                    if !map["resolved_at"].is_null() {
                        map.insert("resolved_at".into(), json!(2));
                    }
                }
                for child in map.values_mut() {
                    walk(child);
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    walk(child);
                }
            }
            _ => {}
        }
    }
    walk(&mut value);
    value
}

fn assert_raw_identity(
    response: &serde_json::Value,
    parent: &kanban_sqlite::api::TaskRecord,
    step_id: Option<&str>,
) {
    assert_eq!(response["data"]["task_id"], parent.id);
    assert_eq!(response["data"]["execution_plan"]["task_id"], parent.id);
    assert_eq!(
        response["data"]["execution_plan"]["board_id"],
        parent.board_id
    );
    let steps = response["data"]["steps"].as_array().expect("steps array");
    match step_id {
        Some(step_id) => {
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0]["id"], step_id);
            assert_eq!(steps[0]["parent_task_id"], parent.id);
        }
        None => assert!(steps.is_empty()),
    }
}

async fn seeded() -> anyhow::Result<(TestApp, kanban_sqlite::api::TaskRecord)> {
    let test = TestApp::new()?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "project".into(),
            name: "Project".into(),
            description: None,
        },
    )?;
    let parent = create_ready_task_for_test(test.db_path(), "project", "seed", "parent")?;
    Ok((test, parent))
}

async fn produce() -> anyhow::Result<BTreeMap<&'static str, serde_json::Value>> {
    let (test, parent) = seeded().await?;
    let app = test.router();
    let (status, created) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps", parent.id),
        fx("create-step-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let step_id = created["data"]["steps"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, listed) = get_json(app.clone(), &format!("/api/v1/tasks/{}/steps", parent.id)).await?;
    let (_, updated) = patch_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{step_id}", parent.id),
        fx("update-step-request.v1.valid.json"),
        None,
    )
    .await?;
    let (_, done) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{step_id}/done", parent.id),
        fx("complete-step-request.v1.valid.json"),
    )
    .await?;
    let (_, reopened) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{step_id}/reopen", parent.id),
        fx("reopen-step-request.v1.valid.json"),
    )
    .await?;
    let (_, skipped) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/steps/{step_id}/skip", parent.id),
        fx("skip-step-request.v1.valid.json"),
    )
    .await?;
    let (_, removed) =
        delete_json(app, &format!("/api/v1/tasks/{}/steps/{step_id}", parent.id)).await?;
    for response in [&created, &listed, &updated, &done, &reopened, &skipped] {
        assert_raw_identity(response, &parent, Some(&step_id));
    }
    assert_raw_identity(&removed, &parent, None);
    Ok(BTreeMap::from([
        ("list", normalized(listed)),
        ("create", normalized(created)),
        ("update", normalized(updated)),
        ("complete", normalized(done)),
        ("skip", normalized(skipped)),
        ("reopen", normalized(reopened)),
        ("remove", normalized(removed)),
    ]))
}

macro_rules! path_tests {
    ($producer:ident,$consumer:ident,$ty:ty,$fixture:literal,$method:literal,$suffix:literal) => {
        #[test]
        fn $producer() {
            let value = serde_json::to_value(<$ty>::from_fixture()).unwrap();
            assert_eq!(value, fx($fixture));
        }
    };
}

trait FixturePath {
    fn from_fixture() -> Self;
}
macro_rules! task_path_impl {
    ($ty:ty) => {
        impl FixturePath for $ty {
            fn from_fixture() -> Self {
                Self {
                    task_id: "t_project_parent".into(),
                }
            }
        }
    };
}
macro_rules! item_path_impl {
    ($ty:ty) => {
        impl FixturePath for $ty {
            fn from_fixture() -> Self {
                Self {
                    task_id: "t_project_parent".into(),
                    step_id: "st_fixture".into(),
                }
            }
        }
    };
}
task_path_impl!(kanban_contract::ListStepsPath);
task_path_impl!(kanban_contract::CreateStepPath);
item_path_impl!(kanban_contract::UpdateStepPath);
item_path_impl!(kanban_contract::RemoveStepPath);
item_path_impl!(kanban_contract::CompleteStepPath);
item_path_impl!(kanban_contract::SkipStepPath);
item_path_impl!(kanban_contract::ReopenStepPath);

path_tests!(
    list_steps_path_dto_serializes_to_committed_fixture,
    list_steps_path_fixture_is_consumed_by_real_router,
    kanban_contract::ListStepsPath,
    "list-steps-path.v1.valid.json",
    "GET",
    "steps"
);
path_tests!(
    create_step_path_dto_serializes_to_committed_fixture,
    create_step_path_fixture_is_consumed_by_real_router,
    kanban_contract::CreateStepPath,
    "create-step-path.v1.valid.json",
    "POST",
    "steps"
);
path_tests!(
    update_step_path_dto_serializes_to_committed_fixture,
    update_step_path_fixture_is_consumed_by_real_router,
    kanban_contract::UpdateStepPath,
    "update-step-path.v1.valid.json",
    "PATCH",
    "steps/st_fixture"
);
path_tests!(
    remove_step_path_dto_serializes_to_committed_fixture,
    remove_step_path_fixture_is_consumed_by_real_router,
    kanban_contract::RemoveStepPath,
    "remove-step-path.v1.valid.json",
    "DELETE",
    "steps/st_fixture"
);
path_tests!(
    complete_step_path_dto_serializes_to_committed_fixture,
    complete_step_path_fixture_is_consumed_by_real_router,
    kanban_contract::CompleteStepPath,
    "complete-step-path.v1.valid.json",
    "POST",
    "steps/st_fixture/done"
);
path_tests!(
    skip_step_path_dto_serializes_to_committed_fixture,
    skip_step_path_fixture_is_consumed_by_real_router,
    kanban_contract::SkipStepPath,
    "skip-step-path.v1.valid.json",
    "POST",
    "steps/st_fixture/skip"
);
path_tests!(
    reopen_step_path_dto_serializes_to_committed_fixture,
    reopen_step_path_fixture_is_consumed_by_real_router,
    kanban_contract::ReopenStepPath,
    "reopen-step-path.v1.valid.json",
    "POST",
    "steps/st_fixture/reopen"
);

async fn seeded_step() -> anyhow::Result<(TestApp, kanban_sqlite::api::TaskRecord, String)> {
    let (test, parent) = seeded().await?;
    let (_, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/steps", parent.id),
        fx("create-step-request.v1.valid.json"),
    )
    .await?;
    let step_id = response["data"]["steps"][0]["id"]
        .as_str()
        .expect("seeded step id")
        .to_owned();
    Ok((test, parent, step_id))
}

#[tokio::test]
async fn list_steps_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, parent) = seeded().await?;
    let mut path: kanban_contract::ListStepsPath =
        serde_json::from_value(fx("list-steps-path.v1.valid.json"))?;
    path.task_id = parent.id.clone();
    let (status, response) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{}/steps", path.task_id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_raw_identity(&response, &parent, None);
    Ok(())
}

#[tokio::test]
async fn create_step_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, parent) = seeded().await?;
    let mut path: kanban_contract::CreateStepPath =
        serde_json::from_value(fx("create-step-path.v1.valid.json"))?;
    path.task_id = parent.id.clone();
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/steps", path.task_id),
        fx("create-step-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let step_id = response["data"]["steps"][0]["id"].as_str().unwrap();
    assert_raw_identity(&response, &parent, Some(step_id));
    Ok(())
}

#[tokio::test]
async fn update_step_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, parent, step_id) = seeded_step().await?;
    let mut path: kanban_contract::UpdateStepPath =
        serde_json::from_value(fx("update-step-path.v1.valid.json"))?;
    path.task_id = parent.id.clone();
    path.step_id = step_id.clone();
    let (status, response) = patch_json(
        test.router(),
        &format!("/api/v1/tasks/{}/steps/{}", path.task_id, path.step_id),
        fx("update-step-request.v1.valid.json"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_raw_identity(&response, &parent, Some(&step_id));
    Ok(())
}

#[tokio::test]
async fn remove_step_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, parent, step_id) = seeded_step().await?;
    let mut path: kanban_contract::RemoveStepPath =
        serde_json::from_value(fx("remove-step-path.v1.valid.json"))?;
    path.task_id = parent.id.clone();
    path.step_id = step_id;
    let (status, response) = delete_json(
        test.router(),
        &format!("/api/v1/tasks/{}/steps/{}", path.task_id, path.step_id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_raw_identity(&response, &parent, None);
    Ok(())
}

#[tokio::test]
async fn complete_step_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, parent, step_id) = seeded_step().await?;
    let mut path: kanban_contract::CompleteStepPath =
        serde_json::from_value(fx("complete-step-path.v1.valid.json"))?;
    path.task_id = parent.id.clone();
    path.step_id = step_id.clone();
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/steps/{}/done", path.task_id, path.step_id),
        fx("complete-step-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["data"]["steps"][0]["status"], "done");
    assert_raw_identity(&response, &parent, Some(&step_id));
    Ok(())
}

#[tokio::test]
async fn skip_step_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, parent, step_id) = seeded_step().await?;
    let mut path: kanban_contract::SkipStepPath =
        serde_json::from_value(fx("skip-step-path.v1.valid.json"))?;
    path.task_id = parent.id.clone();
    path.step_id = step_id.clone();
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/steps/{}/skip", path.task_id, path.step_id),
        fx("skip-step-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["data"]["steps"][0]["status"], "skipped");
    assert_raw_identity(&response, &parent, Some(&step_id));
    Ok(())
}

#[tokio::test]
async fn reopen_step_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (test, parent, step_id) = seeded_step().await?;
    let app = test.router();
    let mut path: kanban_contract::ReopenStepPath =
        serde_json::from_value(fx("reopen-step-path.v1.valid.json"))?;
    path.task_id = parent.id.clone();
    path.step_id = step_id.clone();
    assert_eq!(
        post_json(
            app.clone(),
            &format!("/api/v1/tasks/{}/steps/{}/done", path.task_id, path.step_id),
            fx("complete-step-request.v1.valid.json")
        )
        .await?
        .0,
        StatusCode::OK
    );
    let (status, response) = post_json(
        app,
        &format!(
            "/api/v1/tasks/{}/steps/{}/reopen",
            path.task_id, path.step_id
        ),
        fx("reopen-step-request.v1.valid.json"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["data"]["steps"][0]["status"], "todo");
    assert_raw_identity(&response, &parent, Some(&step_id));
    Ok(())
}

#[test]
fn create_step_request_dto_serializes_to_committed_fixture() {
    let dto = kanban_contract::CreateStepRequest {
        idempotency_key: Some("step.retry:fixed".into()),
        title: "Draft checks".into(),
        body: Some("Cover plan guards".into()),
        linked_task_ref: None,
        position: Some(2048),
        required: true,
        actor: Some("codex".into()),
    };
    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        fx("create-step-request.v1.valid.json")
    );
}
#[test]
fn update_step_request_dto_serializes_to_committed_fixture() {
    let dto = kanban_contract::UpdateStepRequest {
        title: Some("Verify checks".into()),
        body: None,
        linked_task_ref: None,
        unlink_task: false,
        position: Some(4096),
        required: Some(false),
        actor: Some("reviewer".into()),
    };
    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        fx("update-step-request.v1.valid.json")
    );
}
#[test]
fn complete_step_request_dto_serializes_to_committed_fixture() {
    let dto = kanban_contract::CompleteStepRequest {
        note: "verified".into(),
        actor: Some("verifier".into()),
    };
    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        fx("complete-step-request.v1.valid.json")
    );
}
#[test]
fn skip_step_request_dto_serializes_to_committed_fixture() {
    let dto = kanban_contract::SkipStepRequest {
        reason: "not needed".into(),
        actor: Some("reviewer".into()),
    };
    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        fx("skip-step-request.v1.valid.json")
    );
}
#[test]
fn reopen_step_request_dto_serializes_to_committed_fixture() {
    let dto = kanban_contract::ReopenStepRequest {
        reason: "redo".into(),
        actor: Some("reviewer".into()),
    };
    assert_eq!(
        serde_json::to_value(dto).unwrap(),
        fx("reopen-step-request.v1.valid.json")
    );
}

#[test]
fn step_request_defaults_are_contract_owned_and_legacy_aliases_fail_closed() {
    let create: kanban_contract::CreateStepRequest =
        serde_json::from_value(json!({"title":"x"})).unwrap();
    assert!(create.required);
    assert!(create.body.is_none());
    assert!(create.linked_task_ref.is_none());
    let update: kanban_contract::UpdateStepRequest = serde_json::from_value(json!({})).unwrap();
    assert!(!update.unlink_task);
    assert!(update.title.is_none());
    assert!(update.required.is_none());
    assert!(
        serde_json::from_value::<kanban_contract::CreateStepRequest>(
            json!({"title":"x","linked_task_id":"t_link"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<kanban_contract::UpdateStepRequest>(
            json!({"linked_task_id":"t_link"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<kanban_contract::CreateStepRequest>(
            json!({"title":"x","unknown":1})
        )
        .is_err()
    );
}

#[tokio::test]
async fn create_step_request_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let (t, p) = seeded().await?;
    assert_eq!(
        post_json(
            t.router(),
            &format!("/api/v1/tasks/{}/steps", p.id),
            fx("create-step-request.v1.valid.json")
        )
        .await?
        .0,
        StatusCode::CREATED
    );
    Ok(())
}
macro_rules! mutation_consumer {
    ($name:ident,$fixture:literal,$suffix:literal,$patch:expr) => {
        #[tokio::test]
        async fn $name() -> anyhow::Result<()> {
            let (t, p) = seeded().await?;
            let app = t.router();
            let (_, v) = post_json(
                app.clone(),
                &format!("/api/v1/tasks/{}/steps", p.id),
                fx("create-step-request.v1.valid.json"),
            )
            .await?;
            let id = v["data"]["steps"][0]["id"].as_str().unwrap();
            let url = if $suffix.is_empty() {
                format!("/api/v1/tasks/{}/steps/{}", p.id, id)
            } else {
                format!("/api/v1/tasks/{}/steps/{}/{}", p.id, id, $suffix)
            };
            let status = if $patch {
                patch_json(app, &url, fx($fixture), None).await?.0
            } else {
                post_json(app, &url, fx($fixture)).await?.0
            };
            assert_eq!(status, StatusCode::OK);
            Ok(())
        }
    };
}
mutation_consumer!(
    update_step_request_fixture_is_consumed_by_real_router,
    "update-step-request.v1.valid.json",
    "",
    true
);
mutation_consumer!(
    complete_step_request_fixture_is_consumed_by_real_router,
    "complete-step-request.v1.valid.json",
    "done",
    false
);
mutation_consumer!(
    skip_step_request_fixture_is_consumed_by_real_router,
    "skip-step-request.v1.valid.json",
    "skip",
    false
);
mutation_consumer!(
    reopen_step_request_fixture_is_consumed_by_real_router,
    "reopen-step-request.v1.valid.json",
    "reopen",
    false
);

macro_rules! response_tests {
    ($producer:ident,$consumer:ident,$key:literal,$ty:ty,$fixture:literal) => {
        #[tokio::test]
        async fn $producer() -> anyhow::Result<()> {
            assert_eq!(produce().await?[$key], fx($fixture));
            Ok(())
        }
        #[test]
        fn $consumer() {
            let value = fx($fixture);
            let dto: $ty = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(dto).unwrap(), value)
        }
    };
}
response_tests!(
    list_steps_response_fixture_is_produced_by_real_router,
    list_steps_response_fixture_is_consumed_by_contract_root,
    "list",
    kanban_contract::ListStepsResponse,
    "list-steps-response.v1.valid.json"
);
response_tests!(
    create_step_response_fixture_is_produced_by_real_router,
    create_step_response_fixture_is_consumed_by_contract_root,
    "create",
    kanban_contract::CreateStepResponse,
    "create-step-response.v1.valid.json"
);
response_tests!(
    update_step_response_fixture_is_produced_by_real_router,
    update_step_response_fixture_is_consumed_by_contract_root,
    "update",
    kanban_contract::UpdateStepResponse,
    "update-step-response.v1.valid.json"
);
response_tests!(
    remove_step_response_fixture_is_produced_by_real_router,
    remove_step_response_fixture_is_consumed_by_contract_root,
    "remove",
    kanban_contract::RemoveStepResponse,
    "remove-step-response.v1.valid.json"
);
response_tests!(
    complete_step_response_fixture_is_produced_by_real_router,
    complete_step_response_fixture_is_consumed_by_contract_root,
    "complete",
    kanban_contract::CompleteStepResponse,
    "complete-step-response.v1.valid.json"
);
response_tests!(
    skip_step_response_fixture_is_produced_by_real_router,
    skip_step_response_fixture_is_consumed_by_contract_root,
    "skip",
    kanban_contract::SkipStepResponse,
    "skip-step-response.v1.valid.json"
);
response_tests!(
    reopen_step_response_fixture_is_produced_by_real_router,
    reopen_step_response_fixture_is_consumed_by_contract_root,
    "reopen",
    kanban_contract::ReopenStepResponse,
    "reopen-step-response.v1.valid.json"
);
