use serde::Serialize;

use crate::common::*;

const LIST_TASKS_BOARD: &str = "fixture-list-board";
const LIST_TASKS_BY_STATUS_BOARD: &str = "fixture-status-board";

fn assert_request_dto_matches_fixture<T: Serialize>(
    dto: T,
    raw_fixture: &str,
) -> anyhow::Result<()> {
    let actual = serde_json::to_value(dto)?;
    let expected: Value = serde_json::from_str(raw_fixture)?;
    assert_eq!(actual, expected);
    Ok(())
}

macro_rules! request_producer_witness {
    ($name:ident, $ty:ty, $dto:expr, $fixture:literal) => {
        #[test]
        fn $name() -> anyhow::Result<()> {
            let dto: $ty = $dto;
            assert_request_dto_matches_fixture(dto, include_str!($fixture))
        }
    };
}

request_producer_witness!(
    list_tasks_path_dto_serializes_to_committed_fixture,
    kanban_contract::ListTasksPath,
    kanban_contract::ListTasksPath {
        board: LIST_TASKS_BOARD.to_owned(),
    },
    "../../../../schemas/fixtures/api/list-tasks-path.v1.valid.json"
);

request_producer_witness!(
    list_tasks_query_dto_serializes_to_committed_fixture,
    kanban_contract::ListTasksQuery,
    kanban_contract::ListTasksQuery {
        status: vec![
            kanban_contract::ApiTaskStatus::Ready,
            kanban_contract::ApiTaskStatus::Todo,
        ],
        priority: vec![
            kanban_contract::ApiTaskPriority::new(0).expect("P0"),
            kanban_contract::ApiTaskPriority::new(2).expect("P2"),
        ],
        label: vec![
            kanban_contract::TaskReadLabel::new("\u{3000}后端\u{3000}API\u{2003}")
                .expect("normalized bounded label"),
            kanban_contract::TaskReadLabel::new("api=客户端+v1").expect("bounded label"),
        ],
        plan_filter: vec![
            kanban_contract::TaskReadPlanFilter::PlanNeeded,
            kanban_contract::TaskReadPlanFilter::IncompleteRequiredSteps,
        ],
        assignee: Some("fixture 工作者+一".to_owned()),
        q: Some("schema & contract=值 + 空格".to_owned()),
        include_archived: false,
        limit: 25,
        offset: 0,
        sort: kanban_contract::TaskReadSort::UpdatedAtDesc,
    },
    "../../../../schemas/fixtures/api/list-tasks-query.v1.valid.json"
);

request_producer_witness!(
    list_tasks_by_status_path_dto_serializes_to_committed_fixture,
    kanban_contract::ListTasksByStatusPath,
    kanban_contract::ListTasksByStatusPath {
        board: LIST_TASKS_BY_STATUS_BOARD.to_owned(),
    },
    "../../../../schemas/fixtures/api/list-tasks-by-status-path.v1.valid.json"
);

request_producer_witness!(
    list_tasks_by_status_query_dto_serializes_to_committed_fixture,
    kanban_contract::ListTasksByStatusQuery,
    kanban_contract::ListTasksByStatusQuery {
        status: vec![
            kanban_contract::ApiTaskStatus::Todo,
            kanban_contract::ApiTaskStatus::Ready,
        ],
        priority: Vec::new(),
        label: vec![
            kanban_contract::TaskReadLabel::new("\u{3000}状态\u{3000}窗口\u{2003}")
                .expect("normalized bounded label"),
        ],
        plan_filter: Vec::new(),
        assignee: None,
        q: None,
        include_archived: false,
        limit: 50,
        offset: 0,
        sort: kanban_contract::TaskReadSort::Position,
    },
    "../../../../schemas/fixtures/api/list-tasks-by-status-query.v1.valid.json"
);

fn encode_task_read_uri(path: &str, pairs: Vec<(&str, String)>) -> String {
    let query = serde_urlencoded::to_string(pairs).expect("form-encode task-read fixture");
    format!("{path}?{query}")
}

fn list_tasks_query_uri(board: &str, query: &kanban_contract::ListTasksQuery) -> String {
    let mut pairs = Vec::new();
    for status in &query.status {
        pairs.push(("status", status.as_str().to_owned()));
    }
    for priority in &query.priority {
        pairs.push(("priority", priority.get().to_string()));
    }
    for label in &query.label {
        pairs.push(("label", label.as_str().to_owned()));
    }
    for filter in &query.plan_filter {
        pairs.push(("plan_filter", filter.as_str().to_owned()));
    }
    if let Some(assignee) = &query.assignee {
        pairs.push(("assignee", assignee.clone()));
    }
    if let Some(q) = &query.q {
        pairs.push(("q", q.clone()));
    }
    pairs.push(("include_archived", query.include_archived.to_string()));
    pairs.push(("limit", query.limit.to_string()));
    pairs.push(("offset", query.offset.to_string()));
    pairs.push(("sort", query.sort.as_str().to_owned()));
    encode_task_read_uri(&format!("/api/v1/boards/{board}/tasks"), pairs)
}

fn list_tasks_by_status_query_uri(
    board: &str,
    query: &kanban_contract::ListTasksByStatusQuery,
) -> String {
    let mut pairs = Vec::new();
    for status in &query.status {
        pairs.push(("status", status.as_str().to_owned()));
    }
    for priority in &query.priority {
        pairs.push(("priority", priority.get().to_string()));
    }
    for label in &query.label {
        pairs.push(("label", label.as_str().to_owned()));
    }
    for filter in &query.plan_filter {
        pairs.push(("plan_filter", filter.as_str().to_owned()));
    }
    if let Some(assignee) = &query.assignee {
        pairs.push(("assignee", assignee.clone()));
    }
    if let Some(q) = &query.q {
        pairs.push(("q", q.clone()));
    }
    pairs.push(("include_archived", query.include_archived.to_string()));
    pairs.push(("limit", query.limit.to_string()));
    pairs.push(("offset", query.offset.to_string()));
    pairs.push(("sort", query.sort.as_str().to_owned()));
    encode_task_read_uri(&format!("/api/v1/boards/{board}/tasks/by-status"), pairs)
}

fn create_fixture_board(test: &TestApp, slug: &str) -> anyhow::Result<()> {
    kanban_sqlite::api::create_board(
        test.db_path(),
        "fixture-seed",
        kanban_sqlite::api::CreateBoard {
            slug: slug.to_owned(),
            name: format!("Fixture {slug}"),
            description: None,
        },
    )?;
    Ok(())
}

#[tokio::test]
async fn list_tasks_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let path: kanban_contract::ListTasksPath = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/list-tasks-path.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    create_fixture_board(&test, &path.board)?;
    let sentinel = create_ready_task_for_test(
        test.db_path(),
        &path.board,
        "fixture-seed",
        "list path sentinel",
    )?;
    let (status, response) = get_json(
        test.router(),
        &format!("/api/v1/boards/{}/tasks", path.board),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["data"][0]["id"], sentinel.id);
    Ok(())
}

#[tokio::test]
async fn list_tasks_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let query: kanban_contract::ListTasksQuery = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/list-tasks-query.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    create_fixture_board(&test, LIST_TASKS_BOARD)?;
    let uri = list_tasks_query_uri(LIST_TASKS_BOARD, &query);
    let (status, response) = get_json(test.router(), &uri).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["meta"]["limit"], 25);
    assert_eq!(response["meta"]["offset"], 0);
    Ok(())
}

#[tokio::test]
async fn list_tasks_by_status_path_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let path: kanban_contract::ListTasksByStatusPath = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/list-tasks-by-status-path.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    create_fixture_board(&test, &path.board)?;
    let sentinel = create_ready_task_for_test(
        test.db_path(),
        &path.board,
        "fixture-seed",
        "by-status path sentinel",
    )?;
    let (status, response) = get_json(
        test.router(),
        &format!("/api/v1/boards/{}/tasks/by-status?status=ready", path.board),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response["data"]["statuses"][0]["tasks"][0]["id"],
        sentinel.id
    );
    Ok(())
}

#[tokio::test]
async fn list_tasks_by_status_query_fixture_is_consumed_by_real_router() -> anyhow::Result<()> {
    let query: kanban_contract::ListTasksByStatusQuery = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/list-tasks-by-status-query.v1.valid.json"
    ))?;
    let test = TestApp::new()?;
    create_fixture_board(&test, LIST_TASKS_BY_STATUS_BOARD)?;
    let uri = list_tasks_by_status_query_uri(LIST_TASKS_BY_STATUS_BOARD, &query);
    let (status, response) = get_json(test.router(), &uri).await?;
    assert_eq!(status, StatusCode::OK, "{response}");
    let statuses = response["data"]["statuses"]
        .as_array()
        .context("status windows")?;
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0]["status"], "todo");
    assert_eq!(statuses[1]["status"], "ready");
    Ok(())
}
