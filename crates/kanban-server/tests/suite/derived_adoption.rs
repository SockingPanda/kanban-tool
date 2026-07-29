use anyhow::Context;
use axum::http::StatusCode;
use kanban_contract::{
    ApiTaskStatus, BoardQuery, BuildContextPath, BuildContextQuery, BuildContextResponse,
    GraphNeighborsQuery, GraphNeighborsResponse, GraphStatusResponse, ListEventsQuery,
    SearchStatusResponse, SearchTasksByStatusResponse, SearchTasksQuery, SearchTasksResponse,
    StatsResponse, VectorStatusResponse,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::common::{AppState, TestApp, build_router, create_ready_task_for_test, get_json};

#[cfg(unix)]
fn graph_fixture_helper(dir: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("graph-contract-helper");
    std::fs::write(
        &path,
        r#"#!/usr/bin/env python3
import json
payload = [{"subject_uri":"kb://task/t_fixture","predicate":"depends_on","object_uri":"kb://task/t_dependency","graph_uri":"kb://board/default","provenance":{"source_table":"task_dependencies","source_id":"dep_fixture","source_event_id":17,"authoritative_store":"sqlite"},"metadata":{},"created_at":101,"updated_at":102}]
print(json.dumps({"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}))
"#,
    )?;
    let mut permissions = std::fs::metadata(&path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions)?;
    Ok(path)
}

fn fixture(path: &str) -> anyhow::Result<Value> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Ok(serde_json::from_str(&std::fs::read_to_string(
        root.join(path),
    )?)?)
}

fn consume<T: DeserializeOwned>(path: &str) -> anyhow::Result<()> {
    let value = fixture(path)?;
    serde_json::from_value::<T>(value.clone()).context("valid fixture")?;
    let mut hostile = value;
    hostile
        .as_object_mut()
        .context("object fixture")?
        .insert("unexpected".into(), json!(true));
    assert!(serde_json::from_value::<T>(hostile).is_err());
    Ok(())
}

macro_rules! consumer_test {
    ($name:ident, $ty:ty, $path:literal) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            consume::<$ty>($path)
        }
    };
}

fn produce<T: Serialize>(value: T, path: &str) -> anyhow::Result<()> {
    assert_eq!(serde_json::to_value(value)?, fixture(path)?);
    Ok(())
}

macro_rules! producer_test {
    ($name:ident, $value:expr, $path:literal) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            produce($value, $path)
        }
    };
}

fn fixture_search_query() -> SearchTasksQuery {
    SearchTasksQuery {
        board: "default".into(),
        q: Some("needle".into()),
        status: vec![ApiTaskStatus::Ready, ApiTaskStatus::Todo],
        label: vec!["alpha".into(), "beta".into()],
        include_archived: false,
        limit: 1,
        offset: 1,
        assignee: Some("agent".into()),
    }
}

producer_test!(
    get_stats_query_dto_serializes_to_committed_fixture,
    BoardQuery {
        board: "default".into()
    },
    "schemas/fixtures/api/get-stats-query.v1.valid.json"
);
producer_test!(
    search_tasks_query_dto_serializes_to_committed_fixture,
    fixture_search_query(),
    "schemas/fixtures/api/search-tasks-query.v1.valid.json"
);
producer_test!(
    search_tasks_by_status_query_dto_serializes_to_committed_fixture,
    fixture_search_query(),
    "schemas/fixtures/api/search-tasks-by-status-query.v1.valid.json"
);
producer_test!(
    search_status_query_dto_serializes_to_committed_fixture,
    BoardQuery {
        board: "default".into()
    },
    "schemas/fixtures/api/search-status-query.v1.valid.json"
);
producer_test!(
    build_context_path_dto_serializes_to_committed_fixture,
    BuildContextPath {
        task_id: "t_fixture".into()
    },
    "schemas/fixtures/api/build-context-path.v1.valid.json"
);
producer_test!(
    build_context_query_dto_serializes_to_committed_fixture,
    BuildContextQuery {
        board: "default".into(),
        lexical_limit: 5,
        graph_limit: 10,
        vector_limit: 5,
        max_items: 20,
    },
    "schemas/fixtures/api/build-context-query.v1.valid.json"
);
producer_test!(
    graph_status_query_dto_serializes_to_committed_fixture,
    BoardQuery {
        board: "default".into()
    },
    "schemas/fixtures/api/graph-status-query.v1.valid.json"
);
producer_test!(
    graph_neighbors_query_dto_serializes_to_committed_fixture,
    GraphNeighborsQuery {
        board: "default".into(),
        entity_uri: "kb://task/t_fixture".into(),
        predicate: Some("depends_on".into()),
        limit: 2,
    },
    "schemas/fixtures/api/graph-neighbors-query.v1.valid.json"
);
producer_test!(
    vector_status_query_dto_serializes_to_committed_fixture,
    BoardQuery {
        board: "default".into()
    },
    "schemas/fixtures/api/vector-status-query.v1.valid.json"
);
producer_test!(
    list_events_query_dto_serializes_to_committed_fixture,
    ListEventsQuery {
        board: "default".into(),
        task_id: None,
        after: 0,
        limit: 100
    },
    "schemas/fixtures/api/list-events-query.v1.valid.json"
);

consumer_test!(
    get_stats_query_fixture_is_consumed_by_contract_root,
    BoardQuery,
    "schemas/fixtures/api/get-stats-query.v1.valid.json"
);
consumer_test!(
    get_stats_response_fixture_is_consumed_by_contract_root,
    StatsResponse,
    "schemas/fixtures/api/get-stats-response.v1.valid.json"
);
consumer_test!(
    search_tasks_query_fixture_is_consumed_by_contract_root,
    SearchTasksQuery,
    "schemas/fixtures/api/search-tasks-query.v1.valid.json"
);
consumer_test!(
    search_tasks_response_fixture_is_consumed_by_contract_root,
    SearchTasksResponse,
    "schemas/fixtures/api/search-tasks-response.v1.valid.json"
);
consumer_test!(
    search_tasks_by_status_query_fixture_is_consumed_by_contract_root,
    SearchTasksQuery,
    "schemas/fixtures/api/search-tasks-by-status-query.v1.valid.json"
);
consumer_test!(
    search_tasks_by_status_response_fixture_is_consumed_by_contract_root,
    SearchTasksByStatusResponse,
    "schemas/fixtures/api/search-tasks-by-status-response.v1.valid.json"
);
consumer_test!(
    search_status_query_fixture_is_consumed_by_contract_root,
    BoardQuery,
    "schemas/fixtures/api/search-status-query.v1.valid.json"
);
consumer_test!(
    search_status_response_fixture_is_consumed_by_contract_root,
    SearchStatusResponse,
    "schemas/fixtures/api/search-status-response.v1.valid.json"
);
consumer_test!(
    build_context_path_fixture_is_consumed_by_contract_root,
    BuildContextPath,
    "schemas/fixtures/api/build-context-path.v1.valid.json"
);
consumer_test!(
    build_context_query_fixture_is_consumed_by_contract_root,
    BuildContextQuery,
    "schemas/fixtures/api/build-context-query.v1.valid.json"
);
consumer_test!(
    build_context_response_fixture_is_consumed_by_contract_root,
    BuildContextResponse,
    "schemas/fixtures/api/build-context-response.v1.valid.json"
);
consumer_test!(
    graph_status_query_fixture_is_consumed_by_contract_root,
    BoardQuery,
    "schemas/fixtures/api/graph-status-query.v1.valid.json"
);
consumer_test!(
    graph_status_response_fixture_is_consumed_by_contract_root,
    GraphStatusResponse,
    "schemas/fixtures/api/graph-status-response.v1.valid.json"
);
consumer_test!(
    graph_neighbors_response_fixture_is_consumed_by_contract_root,
    GraphNeighborsResponse,
    "schemas/fixtures/api/graph-neighbors-response.v1.valid.json"
);
consumer_test!(
    vector_status_query_fixture_is_consumed_by_contract_root,
    BoardQuery,
    "schemas/fixtures/api/vector-status-query.v1.valid.json"
);
consumer_test!(
    vector_status_response_fixture_is_consumed_by_contract_root,
    VectorStatusResponse,
    "schemas/fixtures/api/vector-status-response.v1.valid.json"
);
consumer_test!(
    list_events_query_fixture_is_consumed_by_contract_root,
    ListEventsQuery,
    "schemas/fixtures/api/list-events-query.v1.valid.json"
);

async fn assert_ok(uri: &str) -> anyhow::Result<Value> {
    let (status, value) = get_json(TestApp::new()?.router(), uri).await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    Ok(value)
}

#[tokio::test]
async fn get_stats_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    create_ready_task_for_test(test.db_path(), "default", "fixture", "stats sentinel")?;
    let (status, value) = get_json(test.router(), "/api/v1/stats?board=default").await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert!(
        value["data"]["board_id"]
            .as_str()
            .context("board id")?
            .starts_with("b_")
    );
    assert!(
        value["data"]["status_counts"]
            .as_array()
            .context("counts")?
            .iter()
            .any(|entry| entry == &json!({"status":"ready","count":1}))
    );
    Ok(())
}

fn seed_search_fixture(test: &TestApp) -> anyhow::Result<()> {
    for name in ["alpha", "beta"] {
        kanban_sqlite::api::create_label(
            test.db_path(),
            "default",
            kanban_sqlite::api::CreateLabel {
                name: name.into(),
                color: None,
            },
        )?;
    }
    kanban_sqlite::api::create_task_with_labels(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask {
            title: "needle contract sentinel".into(),
            description: Some("derived query witness".into()),
            status: Some(kanban_core::TaskStatus::Ready),
            assignee: Some("agent".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
        &["alpha".into(), "beta".into()],
    )?;
    Ok(())
}

fn seed_search_query_decoys(test: &TestApp) -> anyhow::Result<()> {
    for name in ["alpha", "beta"] {
        kanban_sqlite::api::create_label(
            test.db_path(),
            "default",
            kanban_sqlite::api::CreateLabel {
                name: name.into(),
                color: None,
            },
        )?;
    }
    for (title, assignee, labels, archived) in [
        ("A needle exact", "agent", vec!["alpha", "beta"], false),
        ("B needle exact", "agent", vec!["alpha", "beta"], false),
        ("wrong query", "agent", vec!["alpha", "beta"], false),
        (
            "needle wrong assignee",
            "other",
            vec!["alpha", "beta"],
            false,
        ),
        ("needle missing label", "agent", vec!["alpha"], false),
        ("needle archived", "agent", vec!["alpha", "beta"], true),
    ] {
        let task = kanban_sqlite::api::create_task_with_labels(
            test.db_path(),
            "default",
            "fixture",
            kanban_sqlite::api::CreateTask {
                title: title.into(),
                description: Some("derived filter witness".into()),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: Some(assignee.into()),
                priority: 0,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".into(),
            },
            &labels.into_iter().map(str::to_owned).collect::<Vec<_>>(),
        )?;
        if archived {
            kanban_sqlite::api::archive_task(test.db_path(), "default", "fixture", &task.id, true)?;
        }
    }
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture",
        kanban_sqlite::api::CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;
    for name in ["alpha", "beta"] {
        kanban_sqlite::api::create_label(
            test.db_path(),
            "other",
            kanban_sqlite::api::CreateLabel {
                name: name.into(),
                color: None,
            },
        )?;
    }
    kanban_sqlite::api::create_task_with_labels(
        test.db_path(),
        "other",
        "fixture",
        kanban_sqlite::api::CreateTask {
            title: "other board needle".into(),
            description: Some("board witness".into()),
            status: Some(kanban_core::TaskStatus::Ready),
            assignee: Some("agent".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
        &["alpha".into(), "beta".into()],
    )?;
    Ok(())
}

#[tokio::test]
async fn search_tasks_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_search_query_decoys(&test)?;
    let (status, value) = get_json(test.router(), "/api/v1/search/tasks?board=default&q=needle&status=ready&status=todo&label=alpha&label=beta&include_archived=false&limit=1&offset=1&assignee=agent").await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["data"]["hits"].as_array().context("hits")?.len(), 1);
    assert_eq!(value["data"]["hits"][0]["task"]["title"], "A needle exact");
    assert_eq!(value["meta"], json!({"limit":1,"offset":1}));

    let app = test.router();
    let (_, archived_hidden) = get_json(app.clone(), "/api/v1/search/tasks?board=default&q=needle&status=archived&label=alpha&label=beta&assignee=agent&include_archived=false").await?;
    assert!(
        archived_hidden["data"]["hits"]
            .as_array()
            .context("hidden archived")?
            .is_empty()
    );
    let (_, archived_visible) = get_json(app.clone(), "/api/v1/search/tasks?board=default&q=needle&status=archived&label=alpha&label=beta&assignee=agent&include_archived=true").await?;
    assert_eq!(
        archived_visible["data"]["hits"][0]["task"]["title"],
        "needle archived"
    );
    let (_, other_board) = get_json(app, "/api/v1/search/tasks?board=other&q=needle&status=ready&status=todo&label=alpha&label=beta&assignee=agent").await?;
    assert_eq!(
        other_board["data"]["hits"]
            .as_array()
            .context("other hits")?
            .len(),
        1
    );
    assert_eq!(
        other_board["data"]["hits"][0]["task"]["title"],
        "other board needle"
    );
    Ok(())
}
#[tokio::test]
async fn search_tasks_by_status_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_search_query_decoys(&test)?;
    let (status, value) = get_json(test.router(), "/api/v1/search/tasks/by-status?board=default&q=needle&status=ready&status=todo&label=alpha&label=beta&include_archived=false&limit=1&offset=1&assignee=agent").await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    let windows = value["data"]["statuses"].as_array().context("windows")?;
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0]["status"], "ready");
    assert!(
        windows[0]["tasks"]
            .as_array()
            .context("ready tasks")?
            .is_empty()
    );
    assert_eq!(windows[1]["status"], "todo");
    assert_eq!(
        windows[1]["tasks"].as_array().context("todo tasks")?.len(),
        1
    );
    assert_eq!(windows[1]["tasks"][0]["title"], "A needle exact");

    let app = test.router();
    let (_, archived_hidden) = get_json(app.clone(), "/api/v1/search/tasks/by-status?board=default&q=needle&status=archived&label=alpha&label=beta&assignee=agent&include_archived=false").await?;
    assert!(
        archived_hidden["data"]["statuses"][0]["tasks"]
            .as_array()
            .context("hidden archived")?
            .is_empty()
    );
    let (_, archived_visible) = get_json(app.clone(), "/api/v1/search/tasks/by-status?board=default&q=needle&status=archived&label=alpha&label=beta&assignee=agent&include_archived=true").await?;
    assert_eq!(
        archived_visible["data"]["statuses"][0]["tasks"][0]["title"],
        "needle archived"
    );
    let (_, other_board) = get_json(app, "/api/v1/search/tasks/by-status?board=other&q=needle&status=ready&status=todo&label=alpha&label=beta&assignee=agent").await?;
    let other_windows = other_board["data"]["statuses"]
        .as_array()
        .context("other windows")?;
    assert_eq!(other_windows[1]["tasks"][0]["title"], "other board needle");
    Ok(())
}
#[tokio::test]
async fn search_status_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    assert_ok("/api/v1/search/status?board=default").await?;
    Ok(())
}
#[tokio::test]
async fn graph_status_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    assert_ok("/api/v1/graph/status?board=default").await?;
    Ok(())
}
#[cfg(unix)]
#[tokio::test]
async fn graph_neighbors_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let helper = graph_fixture_helper(test.dir_path())?;
    let app = build_router(AppState::new(test.db_path(), "fixture").with_graph_helper_path(helper));
    let (status, value) = get_json(
        app,
        "/api/v1/graph/neighbors?board=default&entity_uri=kb%3A%2F%2Ftask%2Ft_fixture&predicate=depends_on&limit=2",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["meta"]["limit"], 2);
    assert_eq!(value["data"][0]["predicate"], "depends_on");
    assert_eq!(value["data"][0]["subject_uri"], "kb://task/t_fixture");
    Ok(())
}
#[tokio::test]
async fn vector_status_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    assert_ok("/api/v1/vector/status?board=default").await?;
    Ok(())
}
#[tokio::test]
async fn list_events_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    create_ready_task_for_test(test.db_path(), "default", "fixture", "event sentinel")?;
    let (status, value) = get_json(
        test.router(),
        "/api/v1/events?board=default&after=0&limit=100",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert!(!value["data"].as_array().context("events")?.is_empty());
    assert!(value["meta"]["next_after"].as_i64().context("next_after")? > 0);
    Ok(())
}

#[tokio::test]
async fn build_context_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "fixture", "context")?;
    let (status, _) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{}/context?board=default", task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn build_context_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "fixture", "context")?;
    let uri = format!(
        "/api/v1/tasks/{}/context?board=default&lexical_limit=5&graph_limit=10&vector_limit=5&max_items=20",
        task.id
    );
    let (status, value) = get_json(test.router(), &uri).await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(
        value["data"]["policy"],
        json!({"lexical_limit":5,"graph_limit":10,"vector_limit":5,"max_items":20})
    );
    Ok(())
}

#[tokio::test]
async fn get_stats_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let stale = kanban_sqlite::api::create_task(
        test.db_path(),
        "default",
        "fixture",
        kanban_sqlite::api::CreateTask::ready("stale claim"),
    )?;
    kanban_sqlite::api::mark_execution_plan_not_required(
        test.db_path(),
        "default",
        "fixture",
        &stale.id,
        "stats witness",
    )?;
    kanban_sqlite::api::claim_task(test.db_path(), "default", "worker", &stale.id, 60_000)?;
    let conn = kanban_test_support::connect_file(test.db_path())?;
    conn.execute(
        "UPDATE tasks SET claim_expires_at=0 WHERE id=?1",
        (&stale.id,),
    )?;
    conn.execute(
        "UPDATE task_runs SET claim_expires_at=0 WHERE task_id=?1 AND status='running'",
        (&stale.id,),
    )?;
    for title in ["blocked a", "blocked b"] {
        let task = kanban_sqlite::api::create_task(
            test.db_path(),
            "default",
            "fixture",
            kanban_sqlite::api::CreateTask::ready(title),
        )?;
        kanban_sqlite::api::block_task(
            test.db_path(),
            "default",
            "fixture",
            &task.id,
            "waiting on operator",
            None,
            true,
        )?;
    }
    let (status, mut value) = get_json(test.router(), "/api/v1/stats?board=default").await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    value["data"]["board_id"] = json!("b_fixture");
    value["data"]["generated_at"] = json!(0);
    value["data"]["stale_claims"][0]["task_id"] = json!("t_fixture");
    value["data"]["stale_claims"][0]["last_heartbeat_at"] = json!(0);
    value["data"]["stale_claims"][0]["current_run_id"] = json!("r_fixture");
    assert_eq!(
        value,
        fixture("schemas/fixtures/api/get-stats-response.v1.valid.json")?
    );
    Ok(())
}

fn normalize_search_task(task: &mut Value) {
    task["id"] = json!("t_fixture");
    task["board_id"] = json!("b_fixture");
    task["ref"] = json!("default#1");
    task["seq"] = json!(1);
    task["created_at"] = json!(0);
    task["updated_at"] = json!(0);
    for label in task["labels"].as_array_mut().expect("labels") {
        label["id"] = json!(format!("l_{}", label["name"].as_str().expect("name")));
        label["board_id"] = json!("b_fixture");
        label["created_at"] = json!(0);
        label["updated_at"] = json!(0);
    }
}

#[tokio::test]
async fn search_tasks_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    seed_search_fixture(&test)?;
    let (status, mut value) = get_json(test.router(), "/api/v1/search/tasks?board=default&q=needle&status=ready&status=todo&label=alpha&label=beta&assignee=agent&limit=20&offset=0").await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    normalize_search_task(&mut value["data"]["hits"][0]["task"]);
    value["data"]["hits"][0]["task_id"] = json!("t_fixture");
    value["data"]["meta"]["last_event_id"] = json!(0);
    value["data"]["meta"]["resolved_board_id"] = json!("b_fixture");
    assert_eq!(
        value,
        fixture("schemas/fixtures/api/search-tasks-response.v1.valid.json")?
    );
    Ok(())
}

#[tokio::test]
async fn search_tasks_by_status_response_fixture_is_produced_by_real_router() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    seed_search_fixture(&test)?;
    let (status, mut value) = get_json(test.router(), "/api/v1/search/tasks/by-status?board=default&q=needle&status=ready&status=todo&label=alpha&label=beta&assignee=agent&limit=20&offset=0").await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    normalize_search_task(&mut value["data"]["statuses"][1]["tasks"][0]);
    for window in value["data"]["statuses"]
        .as_array_mut()
        .context("windows")?
    {
        window["search_meta"]["last_event_id"] = json!(0);
        window["search_meta"]["resolved_board_id"] = json!("b_fixture");
    }
    assert!(!serde_json::to_string(&value)?.contains("claim_token"));
    assert_eq!(
        value,
        fixture("schemas/fixtures/api/search-tasks-by-status-response.v1.valid.json")?
    );
    Ok(())
}

#[tokio::test]
async fn search_status_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let mut value = assert_ok("/api/v1/search/status?board=default").await?;
    value["data"]["message"] = json!("fixture");
    value["data"]["resolved_board_id"] = json!("b_fixture");
    assert_eq!(
        value,
        fixture("schemas/fixtures/api/search-status-response.v1.valid.json")?
    );
    Ok(())
}

#[tokio::test]
async fn build_context_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "fixture", "context")?;
    let (status, mut value) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{}/context?board=default", task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    value["data"]["subject"] = json!("kanban://task/t_fixture");
    for item in value["data"]["items"]
        .as_array_mut()
        .context("context items")?
    {
        if item["entity_uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with(&task.id))
        {
            item["entity_uri"] = json!("kanban://task/t_fixture");
        }
    }
    assert_eq!(
        value,
        fixture("schemas/fixtures/api/build-context-response.v1.valid.json")?
    );
    Ok(())
}

#[tokio::test]
async fn graph_status_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = build_router(
        AppState::new(test.db_path(), "fixture")
            .with_graph_helper_path(test.dir_path().join("missing-graph-helper")),
    );
    let (status, mut value) = get_json(app, "/api/v1/graph/status?board=default").await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    value["data"]["message"] = json!("fixture");
    assert_eq!(
        value,
        fixture("schemas/fixtures/api/graph-status-response.v1.valid.json")?
    );
    Ok(())
}
#[cfg(unix)]
#[tokio::test]
async fn graph_neighbors_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let helper = graph_fixture_helper(test.dir_path())?;
    let app = build_router(AppState::new(test.db_path(), "fixture").with_graph_helper_path(helper));
    let (status, value) = get_json(
        app,
        "/api/v1/graph/neighbors?board=default&entity_uri=kb%3A%2F%2Ftask%2Ft_fixture&predicate=depends_on&limit=2",
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(
        value,
        fixture("schemas/fixtures/api/graph-neighbors-response.v1.valid.json")?
    );
    Ok(())
}

#[tokio::test]
async fn vector_status_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = build_router(
        AppState::new(test.db_path(), "fixture")
            .with_vector_helper_path(test.dir_path().join("missing-vector-helper")),
    );
    let (status, mut value) = get_json(app, "/api/v1/vector/status?board=default").await?;
    assert_eq!(status, StatusCode::OK, "{value}");
    value["data"]["message"] = json!("fixture");
    assert_eq!(
        value,
        fixture("schemas/fixtures/api/vector-status-response.v1.valid.json")?
    );
    Ok(())
}

#[tokio::test]
async fn derived_query_contracts_reject_unknown_fields_at_real_router() -> anyhow::Result<()> {
    for uri in [
        "/api/v1/stats?unexpected=1",
        "/api/v1/search/tasks?unexpected=1",
        "/api/v1/search/tasks/by-status?unexpected=1",
        "/api/v1/search/status?unexpected=1",
        "/api/v1/graph/status?unexpected=1",
        "/api/v1/vector/status?unexpected=1",
        "/api/v1/events?unexpected=1",
    ] {
        let (status, body) = get_json(TestApp::new()?.router(), uri).await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        assert_eq!(body["error"]["code"], "invalid_input");
    }
    let test = TestApp::new()?;
    let task = create_ready_task_for_test(test.db_path(), "default", "fixture", "context")?;
    let (status, body) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{}/context?unexpected=1", task.id),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    Ok(())
}

#[tokio::test]
async fn search_contract_never_exposes_claim_token() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    create_ready_task_for_test(test.db_path(), "default", "fixture", "privacy")?;
    let (status, body) = get_json(
        test.router(),
        "/api/v1/search/tasks?board=default&q=privacy",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(!serde_json::to_string(&body)?.contains("claim_token"));
    Ok(())
}

fn assert_contract_owned_handlers(
    search: &str,
    context: &str,
    maintenance: &str,
    graph: &str,
    vector: &str,
    events: &str,
) {
    for source in [search, context, maintenance, graph, vector, events] {
        syn::parse_file(source).expect("handler source must parse");
    }
    for alias in [
        "SearchTasksResponse",
        "SearchTasksByStatusResponse",
        "SearchStatusResponse",
    ] {
        assert!(search.contains(alias), "missing {alias}");
    }
    assert!(!search.contains("DataEnvelope<kanban_search::"));
    assert!(context.contains("BuildContextResponse"));
    assert!(!context.contains("DataEnvelope<kanban_context::ContextPack>"));
    assert!(maintenance.contains("StatsResponse"));
    assert!(!maintenance.contains("DataEnvelope<kanban_sqlite::api::QueueStats>"));
    assert!(graph.contains("GraphStatusResponse"));
    assert!(!graph.contains("DataEnvelope<kanban_graph::GraphStoreStatus>"));
    assert!(vector.contains("VectorStatusResponse"));
    assert!(!vector.contains("DataEnvelope<kanban_vector::VectorStoreStatus>"));
    assert!(events.contains("Query<ListEventsQuery>"));
}

#[test]
fn derived_handlers_use_contract_owned_exact_roots_and_preserve_provider_seams() {
    let search = include_str!("../../src/handlers/search.rs");
    let context = include_str!("../../src/handlers/context.rs");
    let maintenance = include_str!("../../src/handlers/maintenance.rs");
    let graph = include_str!("../../src/handlers/graph.rs");
    let vector = include_str!("../../src/handlers/vector.rs");
    let events = include_str!("../../src/handlers/events.rs");
    assert_contract_owned_handlers(search, context, maintenance, graph, vector, events);
    assert!(context.contains("provider::build_context_pack_with_vector_store"));
    assert!(graph.contains("run_helper_json::<GraphHelperStatusResponse>"));
    assert!(graph.contains("run_helper_json::<GraphHelperNeighborsResponse>"));
    assert!(vector.contains("run_helper_json::<VectorHelperStatusResponse>"));
    assert!(maintenance.contains("kanban_sqlite::api::queue_stats"));
}

#[test]
fn derived_handler_ownership_gate_rejects_foreign_response_regressions() {
    let search = include_str!("../../src/handlers/search.rs").replace(
        "SearchTasksResponse",
        "DataEnvelope<kanban_search::SearchResults>",
    );
    let result = std::panic::catch_unwind(|| {
        assert_contract_owned_handlers(
            &search,
            include_str!("../../src/handlers/context.rs"),
            include_str!("../../src/handlers/maintenance.rs"),
            include_str!("../../src/handlers/graph.rs"),
            include_str!("../../src/handlers/vector.rs"),
            include_str!("../../src/handlers/events.rs"),
        )
    });
    assert!(result.is_err());
}
