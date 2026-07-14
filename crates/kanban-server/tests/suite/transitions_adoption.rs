use crate::common::*;
use serde::{Serialize, de::DeserializeOwned};
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

fn normalize_response(mut value: Value) -> Value {
    let task = value["data"].as_object_mut().expect("task response data");
    for (field, replacement) in [
        ("id", json!("t_fixture")),
        ("board_id", json!("b_fixture")),
        ("created_at", json!(1)),
        ("updated_at", json!(2)),
        ("position", json!(1024)),
    ] {
        task.insert(field.to_owned(), replacement);
    }
    for field in [
        "started_at",
        "completed_at",
        "archived_at",
        "claim_expires_at",
        "last_heartbeat_at",
    ] {
        if !task[field].is_null() {
            task.insert(field.to_owned(), json!(3));
        }
    }
    if !task["current_run_id"].is_null() {
        task.insert("current_run_id".to_owned(), json!("r_fixture"));
    }
    value
}

fn assert_deterministic_fields_match_fixture(actual: &Value, expected: &Value) {
    let mut actual = actual.clone();
    let mut expected = expected.clone();
    for document in [&mut actual, &mut expected] {
        let task = document["data"]
            .as_object_mut()
            .expect("task response data");
        for field in [
            "id",
            "board_id",
            "created_at",
            "updated_at",
            "position",
            "started_at",
            "completed_at",
            "archived_at",
            "claim_expires_at",
            "last_heartbeat_at",
            "current_run_id",
        ] {
            task.remove(field);
        }
    }
    assert_eq!(
        actual, expected,
        "deterministic transition semantics drifted"
    );
}

fn assert_path_producer<T: Serialize>(dto: T, name: &str) {
    assert_eq!(serde_json::to_value(dto).unwrap(), fixture(name));
}

async fn assert_path_consumer<T: DeserializeOwned + Serialize>(
    name: &str,
    transition: &str,
) -> anyhow::Result<()> {
    let path: T = serde_json::from_value(fixture(name))?;
    let task_id = serde_json::to_value(path)?["task_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let request = match transition {
        "specify" => json!({"description":"fixture specification"}),
        "reopen" => json!({"reason":"fixture reopen"}),
        "heartbeat" => json!({"claim_token":"ct_fixture"}),
        "block" => json!({"reason":"fixture block"}),
        _ => json!({}),
    };
    let (status, body) = post_json(
        TestApp::new()?.router(),
        &format!("/api/v1/tasks/{task_id}/transitions/{transition}"),
        request,
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    Ok(())
}

macro_rules! path_witnesses {
    ($producer:ident, $consumer:ident, $ty:ty, $dto:expr, $fixture:literal, $transition:literal) => {
        #[test]
        fn $producer() {
            assert_path_producer($dto, $fixture);
        }

        #[tokio::test]
        async fn $consumer() -> anyhow::Result<()> {
            assert_path_consumer::<$ty>($fixture, $transition).await
        }
    };
}

path_witnesses!(
    claim_task_path_dto_serializes_to_committed_fixture,
    claim_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::ClaimTaskPath,
    kanban_contract::ClaimTaskPath {
        task_id: "t_fixture".into()
    },
    "claim-task-path.v1.valid.json",
    "claim"
);
path_witnesses!(
    reclaim_task_path_dto_serializes_to_committed_fixture,
    reclaim_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::ReclaimTaskPath,
    kanban_contract::ReclaimTaskPath {
        task_id: "t_fixture".into()
    },
    "reclaim-task-path.v1.valid.json",
    "reclaim"
);
path_witnesses!(
    heartbeat_task_path_dto_serializes_to_committed_fixture,
    heartbeat_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::HeartbeatTaskPath,
    kanban_contract::HeartbeatTaskPath {
        task_id: "t_fixture".into()
    },
    "heartbeat-task-path.v1.valid.json",
    "heartbeat"
);
path_witnesses!(
    complete_task_path_dto_serializes_to_committed_fixture,
    complete_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::CompleteTaskPath,
    kanban_contract::CompleteTaskPath {
        task_id: "t_fixture".into()
    },
    "complete-task-path.v1.valid.json",
    "complete"
);
path_witnesses!(
    submit_review_task_path_dto_serializes_to_committed_fixture,
    submit_review_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::SubmitReviewTaskPath,
    kanban_contract::SubmitReviewTaskPath {
        task_id: "t_fixture".into()
    },
    "submit-review-task-path.v1.valid.json",
    "submit-review"
);
path_witnesses!(
    block_task_path_dto_serializes_to_committed_fixture,
    block_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::BlockTaskPath,
    kanban_contract::BlockTaskPath {
        task_id: "t_fixture".into()
    },
    "block-task-path.v1.valid.json",
    "block"
);
path_witnesses!(
    specify_task_path_dto_serializes_to_committed_fixture,
    specify_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::SpecifyTaskPath,
    kanban_contract::SpecifyTaskPath {
        task_id: "t_fixture".into()
    },
    "specify-task-path.v1.valid.json",
    "specify"
);
path_witnesses!(
    promote_task_path_dto_serializes_to_committed_fixture,
    promote_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::PromoteTaskPath,
    kanban_contract::PromoteTaskPath {
        task_id: "t_fixture".into()
    },
    "promote-task-path.v1.valid.json",
    "promote"
);
path_witnesses!(
    reopen_task_path_dto_serializes_to_committed_fixture,
    reopen_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::ReopenTaskPath,
    kanban_contract::ReopenTaskPath {
        task_id: "t_fixture".into()
    },
    "reopen-task-path.v1.valid.json",
    "reopen"
);
path_witnesses!(
    unblock_task_path_dto_serializes_to_committed_fixture,
    unblock_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::UnblockTaskPath,
    kanban_contract::UnblockTaskPath {
        task_id: "t_fixture".into()
    },
    "unblock-task-path.v1.valid.json",
    "unblock"
);
path_witnesses!(
    archive_task_path_dto_serializes_to_committed_fixture,
    archive_task_path_fixture_is_consumed_by_real_router,
    kanban_contract::ArchiveTaskPath,
    kanban_contract::ArchiveTaskPath {
        task_id: "t_fixture".into()
    },
    "archive-task-path.v1.valid.json",
    "archive"
);

async fn produced(transition: &str) -> anyhow::Result<Value> {
    let test = TestApp::new()?;
    let db = test.db_path().to_path_buf();
    const BOARD: &str = "transitions-project";
    kanban_sqlite::api::create_board(
        &db,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: BOARD.into(),
            name: "Transitions Contract Project".into(),
            description: Some("non-default adoption board".into()),
        },
    )?;
    let fixture_task = || kanban_sqlite::api::CreateTask {
        title: "transition fixture".into(),
        description: Some("fixture specification".into()),
        status: None,
        assignee: Some("fixture-owner".into()),
        priority: 3,
        scheduled_at: None,
        due_at: Some(4_102_444_800_000),
        max_retries: Some(4),
        metadata_json: r#"{"cohort":"B3-C1","rank":7}"#.into(),
    };
    let task = match transition {
        "specify" => kanban_sqlite::api::create_task(
            &db,
            BOARD,
            "seed",
            kanban_sqlite::api::CreateTask {
                description: None,
                ..fixture_task()
            },
        )?,
        "promote" => {
            let task = kanban_sqlite::api::create_task(&db, BOARD, "seed", fixture_task())?;
            let parent = create_ready_task_for_test(&db, BOARD, "seed", "fixture parent")?;
            kanban_sqlite::api::add_dependency(&db, BOARD, "seed", &parent.id, &task.id)?;
            mark_plan_not_required_for_test(&db, BOARD, "seed", &task.id)?;
            let claim =
                kanban_sqlite::api::claim_task(&db, BOARD, "fixture-parent", &parent.id, 60_000)?;
            kanban_sqlite::api::complete_task(
                &db,
                BOARD,
                "fixture-parent",
                &parent.id,
                Some(&claim.claim_token),
                false,
            )?;
            task
        }
        _ => {
            let task = kanban_sqlite::api::create_task(&db, BOARD, "seed", fixture_task())?;
            mark_plan_not_required_for_test(&db, BOARD, "seed", &task.id)?;
            task
        }
    };
    if transition == "specify" {
        mark_plan_not_required_for_test(&db, BOARD, "seed", &task.id)?;
    }
    if transition == "reopen" {
        let claim = kanban_sqlite::api::claim_task(&db, BOARD, "fixture-worker", &task.id, 60_000)?;
        kanban_sqlite::api::complete_task_with_summary_and_result(
            &db,
            BOARD,
            "fixture-worker",
            &task.id,
            Some(&claim.claim_token),
            false,
            Some("fixture done"),
            None,
        )?;
    }
    let app = test.router();
    match transition {
        "unblock" => {
            let (status, _) = post_json(
                app.clone(),
                &format!("/api/v1/tasks/{}/transitions/block", task.id),
                json!({"actor":"fixture-blocker","reason":"fixture block"}),
            )
            .await?;
            assert_eq!(status, StatusCode::OK);
        }
        "reopen" => {}
        _ => {}
    }
    let request = match transition {
        "specify" => {
            json!({"actor":"fixture-actor","description":"fixture specification","scheduled_at":null})
        }
        "promote" => json!({"actor":"fixture-actor"}),
        "reopen" => json!({"actor":"fixture-actor","reason":"fixture reopen"}),
        "unblock" => json!({"actor":"fixture-actor"}),
        "archive" => json!({"actor":"fixture-actor","force":false}),
        _ => unreachable!(),
    };
    let (status, body) = post_json(
        app,
        &format!("/api/v1/tasks/{}/transitions/{transition}", task.id),
        request,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let expected = fixture(&format!("{transition}-task-response.v1.valid.json"));
    assert_deterministic_fields_match_fixture(&body, &expected);
    let data = body["data"].as_object().expect("transition task response");
    assert_eq!(data["board_slug"], BOARD);
    assert_eq!(data["assignee"], "fixture-owner");
    assert_eq!(data["priority"], 3);
    assert_eq!(data["due_at"], 4_102_444_800_000_i64);
    assert_eq!(data["max_retries"], 4);
    assert_eq!(data["metadata"], json!({"cohort":"B3-C1","rank":7}));
    Ok(normalize_response(body))
}

macro_rules! response_witnesses {
    ($producer:ident, $consumer:ident, $ty:ty, $fixture:literal, $transition:literal) => {
        #[tokio::test]
        async fn $producer() -> anyhow::Result<()> {
            assert_eq!(produced($transition).await?, fixture($fixture));
            Ok(())
        }

        #[test]
        fn $consumer() {
            let raw = fixture($fixture);
            let parsed: $ty = serde_json::from_value(raw.clone()).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), raw);
            let mut hostile = raw;
            hostile
                .as_object_mut()
                .unwrap()
                .insert("extra".into(), json!(true));
            assert!(serde_json::from_value::<$ty>(hostile).is_err());
            for (field, drift) in [
                ("assignee", json!("drift-owner")),
                ("priority", json!(1)),
                ("due_at", json!(99)),
                ("metadata", json!({})),
            ] {
                let mut drifted = fixture($fixture);
                drifted["data"][field] = drift;
                let parsed: $ty = serde_json::from_value(drifted.clone()).unwrap();
                assert_ne!(serde_json::to_value(parsed).unwrap(), fixture($fixture));
            }
        }
    };
}

response_witnesses!(
    specify_task_response_fixture_is_produced_by_real_router,
    specify_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::SpecifyTaskResponse,
    "specify-task-response.v1.valid.json",
    "specify"
);

async fn produced_lifecycle(transition: &str) -> anyhow::Result<Value> {
    let test = TestApp::new()?;
    let db = test.db_path().to_path_buf();
    const BOARD: &str = "lifecycle-project";
    kanban_sqlite::api::create_board(
        &db,
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: BOARD.into(),
            name: "Lifecycle Contract Project".into(),
            description: Some("B3-C2 non-default adoption board".into()),
        },
    )?;
    let task = kanban_sqlite::api::create_task(
        &db,
        BOARD,
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "lifecycle fixture".into(),
            description: Some("B3-C2 fixture specification".into()),
            status: None,
            assignee: Some("fixture-owner".into()),
            priority: 2,
            scheduled_at: None,
            due_at: Some(4_102_444_800_000),
            max_retries: Some(5),
            metadata_json: r#"{"cohort":"B3-C2","rank":8}"#.into(),
        },
    )?;
    mark_plan_not_required_for_test(&db, BOARD, "seed", &task.id)?;
    let mut claim_token = None;
    if transition != "block" {
        claim_token = Some(
            kanban_sqlite::api::claim_task(&db, BOARD, "fixture-worker", &task.id, 300_000)?
                .claim_token,
        );
    }
    let request = match transition {
        "reclaim" => {
            json!({"actor":"fixture-actor","force":true,"to_status":"ready","reason":"fixture reclaim"})
        }
        "heartbeat" => {
            json!({"actor":"fixture-actor","claim_token":claim_token,"ttl_ms":300000,"note":"fixture heartbeat"})
        }
        "complete" => {
            json!({"actor":"fixture-actor","claim_token":claim_token,"force":false,"summary":"fixture complete","result":{"ok":true}})
        }
        "submit-review" => {
            json!({"actor":"fixture-actor","claim_token":claim_token,"force":false,"summary":"fixture review"})
        }
        "block" => json!({"actor":"fixture-actor","reason":"fixture block","force":false}),
        _ => unreachable!(),
    };
    let (status, body) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/{transition}", task.id),
        request,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["data"].get("claim_token").is_none(),
        "private token leaked"
    );
    let expected = fixture(&format!("{transition}-task-response.v1.valid.json"));
    assert_deterministic_fields_match_fixture(&body, &expected);
    Ok(normalize_response(body))
}

macro_rules! lifecycle_response_witnesses {
    ($producer:ident, $consumer:ident, $ty:ty, $fixture:literal, $transition:literal) => {
        #[tokio::test]
        async fn $producer() -> anyhow::Result<()> {
            assert_eq!(produced_lifecycle($transition).await?, fixture($fixture));
            Ok(())
        }

        #[test]
        fn $consumer() {
            let raw = fixture($fixture);
            let parsed: $ty = serde_json::from_value(raw.clone()).unwrap();
            assert_eq!(serde_json::to_value(parsed).unwrap(), raw);
            let mut hostile = raw;
            hostile["data"]["claim_token"] = json!("must-not-be-public");
            assert!(serde_json::from_value::<$ty>(hostile).is_err());
        }
    };
}

lifecycle_response_witnesses!(
    reclaim_task_response_fixture_is_produced_by_real_router,
    reclaim_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::ReclaimTaskResponse,
    "reclaim-task-response.v1.valid.json",
    "reclaim"
);
lifecycle_response_witnesses!(
    heartbeat_task_response_fixture_is_produced_by_real_router,
    heartbeat_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::HeartbeatTaskResponse,
    "heartbeat-task-response.v1.valid.json",
    "heartbeat"
);
lifecycle_response_witnesses!(
    complete_task_response_fixture_is_produced_by_real_router,
    complete_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::CompleteTaskResponse,
    "complete-task-response.v1.valid.json",
    "complete"
);
lifecycle_response_witnesses!(
    submit_review_task_response_fixture_is_produced_by_real_router,
    submit_review_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::SubmitReviewTaskResponse,
    "submit-review-task-response.v1.valid.json",
    "submit-review"
);
lifecycle_response_witnesses!(
    block_task_response_fixture_is_produced_by_real_router,
    block_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::BlockTaskResponse,
    "block-task-response.v1.valid.json",
    "block"
);

#[tokio::test]
async fn b3_c2_accept_language_is_honored_per_endpoint() -> anyhow::Result<()> {
    for (transition, body) in [
        ("claim", json!({})),
        ("reclaim", json!({})),
        ("heartbeat", json!({"claim_token":"ct_missing"})),
        ("complete", json!({})),
        ("submit-review", json!({})),
        ("block", json!({"reason":"fixture block"})),
    ] {
        let uri = format!("/api/v1/tasks/t_missing/transitions/{transition}");
        let (en_status, en) = request_json_with_accept_language(
            TestApp::new()?.router(),
            "POST",
            &uri,
            Some(body.clone()),
            "en",
        )
        .await?;
        let (zh_status, zh) = request_json_with_accept_language(
            TestApp::new()?.router(),
            "POST",
            &uri,
            Some(body),
            "zh-CN",
        )
        .await?;
        assert_eq!(en_status, StatusCode::NOT_FOUND, "{transition}: {en}");
        assert_eq!(zh_status, StatusCode::NOT_FOUND, "{transition}: {zh}");
        assert_eq!(en["error"]["code"], "not_found");
        assert_eq!(zh["error"]["code"], "not_found");
        assert_ne!(
            en["error"]["message"], zh["error"]["message"],
            "{transition}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn b3_c2_actor_header_is_forwarded_per_endpoint() -> anyhow::Result<()> {
    for transition in [
        "claim",
        "reclaim",
        "heartbeat",
        "complete",
        "submit-review",
        "block",
    ] {
        let test = TestApp::new()?;
        let db = test.db_path();
        let task =
            create_ready_task_for_test(db, "default", "seed", &format!("{transition} actor"))?;
        let mut token = None;
        if transition != "claim" && transition != "block" {
            token = Some(
                kanban_sqlite::api::claim_task(db, "default", "setup", &task.id, 60_000)?
                    .claim_token,
            );
        }
        let body = match transition {
            "claim" => json!({}),
            "reclaim" => json!({"force":true}),
            "heartbeat" => json!({"claim_token":token}),
            "complete" => json!({"claim_token":token}),
            "submit-review" => json!({"claim_token":token}),
            "block" => json!({"reason":"header actor block"}),
            _ => unreachable!(),
        };
        let (status, response) = request_json(
            test.router(),
            "POST",
            &format!("/api/v1/tasks/{}/transitions/{transition}", task.id),
            Some(body),
            Some("b3-c2-header-actor"),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{transition}: {response}");
        let events = kanban_sqlite::api::list_events(db, "default", Some(&task.id))?;
        assert_eq!(
            events.last().expect("transition event").actor.as_deref(),
            Some("b3-c2-header-actor"),
            "{transition}"
        );
    }
    Ok(())
}
response_witnesses!(
    promote_task_response_fixture_is_produced_by_real_router,
    promote_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::PromoteTaskResponse,
    "promote-task-response.v1.valid.json",
    "promote"
);
response_witnesses!(
    reopen_task_response_fixture_is_produced_by_real_router,
    reopen_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::ReopenTaskResponse,
    "reopen-task-response.v1.valid.json",
    "reopen"
);
response_witnesses!(
    unblock_task_response_fixture_is_produced_by_real_router,
    unblock_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::UnblockTaskResponse,
    "unblock-task-response.v1.valid.json",
    "unblock"
);
response_witnesses!(
    archive_task_response_fixture_is_produced_by_real_router,
    archive_task_response_fixture_is_consumed_by_contract_root,
    kanban_contract::ArchiveTaskResponse,
    "archive-task-response.v1.valid.json",
    "archive"
);

async fn assert_guard_rejects_without_side_effect(
    test: &TestApp,
    task_id: &str,
    transition: &str,
    body: Value,
) -> anyhow::Result<()> {
    let db = test.db_path();
    let before_task =
        serde_json::to_value(kanban_sqlite::api::get_task_by_id_global(db, task_id)?)?;
    let before_events = kanban_sqlite::api::list_events(db, "default", Some(task_id))?;
    let before_runs = kanban_sqlite::api::list_runs(db, "default", Some(task_id))?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{task_id}/transitions/{transition}"),
        body,
    )
    .await?;
    assert!(
        status.is_client_error(),
        "{transition}: {status} {response}"
    );
    assert_eq!(
        serde_json::to_value(kanban_sqlite::api::get_task_by_id_global(db, task_id)?)?,
        before_task,
        "{transition}: task changed after rejected guard"
    );
    assert_eq!(
        kanban_sqlite::api::list_events(db, "default", Some(task_id))?,
        before_events,
        "{transition}: events changed after rejected guard"
    );
    assert_eq!(
        kanban_sqlite::api::list_runs(db, "default", Some(task_id))?,
        before_runs,
        "{transition}: runs changed after rejected guard"
    );
    Ok(())
}

#[tokio::test]
async fn transition_business_guard_and_no_side_effect_matrix() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db = test.db_path();

    let wrong_source = create_ready_task_for_test(db, "default", "seed", "wrong specify source")?;
    assert_guard_rejects_without_side_effect(
        &test,
        &wrong_source.id,
        "specify",
        json!({"description":"cannot specify ready"}),
    )
    .await?;

    let parent = kanban_sqlite::api::create_task(
        db,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "unfinished dependency".into(),
            description: None,
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let deps_plan = kanban_sqlite::api::create_task_with_dependencies(
        db,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask {
            title: "deps and plan guard".into(),
            description: Some("specified but no plan".into()),
            status: None,
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
        std::slice::from_ref(&parent.id),
    )?;
    assert_guard_rejects_without_side_effect(&test, &deps_plan.id, "promote", json!({})).await?;

    let reopen = create_ready_task_for_test(db, "default", "seed", "reopen reason guard")?;
    assert_guard_rejects_without_side_effect(&test, &reopen.id, "reopen", json!({"reason":"   "}))
        .await?;

    let running = create_ready_task_for_test(db, "default", "seed", "archive running guard")?;
    kanban_sqlite::api::claim_task(db, "default", "worker", &running.id, 60_000)?;
    assert_guard_rejects_without_side_effect(&test, &running.id, "archive", json!({"force":false}))
        .await?;

    let not_blocked = create_ready_task_for_test(db, "default", "seed", "unblock source guard")?;
    assert_guard_rejects_without_side_effect(&test, &not_blocked.id, "unblock", json!({})).await?;

    let dependency = kanban_sqlite::api::create_task(
        db,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("unblock dependency"),
    )?;
    let blocked = kanban_sqlite::api::create_task_with_dependencies(
        db,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("unblock recompute"),
        std::slice::from_ref(&dependency.id),
    )?;
    let (status, _) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/block", blocked.id),
        json!({"reason":"explicit block"}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let runs_before = kanban_sqlite::api::list_runs(db, "default", Some(&blocked.id))?;
    let (status, response) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/unblock", blocked.id),
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"]["status"], "todo");
    assert_eq!(
        kanban_sqlite::api::list_runs(db, "default", Some(&blocked.id))?,
        runs_before
    );
    Ok(())
}

#[tokio::test]
async fn claim_task_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "seed", "claim fixture")?;
    let (status, mut actual) = post_json(
        test.router(),
        &format!("/api/v1/tasks/{}/transitions/claim", task.id),
        json!({"actor":"fixture-worker","ttl_ms":300000}),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(actual["data"]["claim_token"].is_string());
    assert!(actual["data"]["task"]["claim_token"].is_null());

    let mut expected = fixture("claim-task-response.v1.valid.json");
    for document in [&mut actual, &mut expected] {
        let data = document["data"]
            .as_object_mut()
            .expect("claim response data");
        data.insert("claim_token".to_owned(), json!("ct_fixture"));
        data.insert("claim_expires_at".to_owned(), json!(4102444800000_i64));
        let task = data["task"].as_object_mut().expect("claim task");
        for (field, value) in [
            ("id", json!("t_fixture")),
            ("board_id", json!("b_fixture")),
            ("created_at", json!(1)),
            ("updated_at", json!(2)),
            ("started_at", json!(1)),
            ("claim_expires_at", json!(4102444800000_i64)),
            ("current_run_id", json!("r_fixture")),
            ("last_heartbeat_at", Value::Null),
            ("ref", json!("transitions-project#1")),
        ] {
            task.insert(field.to_owned(), value);
        }
        let run = data["run"].as_object_mut().expect("claim run");
        for (field, value) in [
            ("id", json!("r_fixture")),
            ("task_id", json!("t_fixture")),
            ("started_at", json!(1)),
        ] {
            run.insert(field.to_owned(), value);
        }
    }
    assert_eq!(actual, expected, "claim response drifted from fixture");
    Ok(())
}

#[test]
fn claim_task_response_fixture_is_consumed_by_contract_root() {
    let raw = fixture("claim-task-response.v1.valid.json");
    let parsed: kanban_contract::ClaimTaskResponse = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), raw);
    let mut hostile = raw;
    hostile["data"]["claim_token"] = json!(null);
    assert!(serde_json::from_value::<kanban_contract::ClaimTaskResponse>(hostile).is_err());
}
