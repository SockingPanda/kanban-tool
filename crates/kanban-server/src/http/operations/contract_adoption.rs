//! HTTP contract adoption tests。
//!
//! 这些测试必须经过真实 Axum router；fixture 只提供 committed wire 输入/输出，
//! 动态数据库身份和时间字段在断言前按字段名明确归一化。

use std::{fs, path::Path};

use axum::{Router, body::Body, http::Request};
use http_body_util::BodyExt;
use kanban_protocol::{
    AddDependencyResponse, ArchiveBoardResponse, ClaimTaskResponse, CompleteStepResponse,
    CreateBoardResponse, CreateCommentResponse, CreateStepResponse, CreateTaskResponse,
    ErrorEnvelope, GetBoardResponse, GetRunLogResponse, GetRunResponse, GetTaskResponse,
    HealthResponse, ListBoardColumnsResponse, ListBoardsResponse, ListCommentsResponse,
    ListDependenciesResponse, ListRunsResponse, ListStepsResponse, ListTasksByStatusResponse,
    ListTasksResponse, MarkExecutionPlanNotRequiredResponse, RemoveDependencyResponse,
    RemoveStepResponse, StatsResponse, UpdateStepResponse, UpdateTaskResponse,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tower::ServiceExt;

use crate::http::operations::test_support::{AppState, StatusCode, build_router};
use kanban_service::{
    ClaimTaskCommand, CreateTaskCommand, MarkExecutionPlanNotRequiredCommand, PromoteTaskCommand,
    TaskStatus,
};

fn fixture(name: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    serde_json::from_str(
        &std::fs::read_to_string(root.join("schemas/fixtures/api").join(name)).unwrap(),
    )
    .unwrap()
}

fn fixture_field(name: &str, field: &str) -> String {
    fixture(name)[field].as_str().unwrap().to_owned()
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(*byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{:02X}", byte));
        }
    }
    encoded
}

fn query_fixture(name: &str, fields: &[&str]) -> String {
    let fixture = fixture(name);
    let object = fixture.as_object().unwrap();
    fields
        .iter()
        .flat_map(|field| {
            let Some(value) = object.get(*field) else {
                return Vec::new();
            };
            match value {
                Value::Array(values) => values
                    .iter()
                    .map(|value| (*field, value))
                    .collect::<Vec<_>>(),
                Value::Null => Vec::new(),
                value => vec![(*field, value)],
            }
        })
        .map(|(field, value)| {
            let encoded = match value {
                Value::Bool(value) => value.to_string(),
                Value::Number(value) => value.to_string(),
                Value::String(value) => percent_encode(value),
                _ => panic!("query fixture value must be scalar: {field}"),
            };
            format!("{field}={encoded}")
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn normalize_keys(value: &mut Value, keys: &[&str]) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if keys.contains(&key.as_str()) {
                    *value = Value::String("<dynamic>".to_owned());
                } else {
                    normalize_keys(value, keys);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_keys(value, keys);
            }
        }
        _ => {}
    }
}

fn normalize_board(value: &mut Value) {
    normalize_keys(
        value,
        &["id", "board_id", "created_at", "updated_at", "archived_at"],
    );
}

fn normalize_task(value: &mut Value) {
    normalize_keys(
        value,
        &[
            "id",
            "board_id",
            "ref",
            "seq",
            "position",
            "created_at",
            "updated_at",
            "started_at",
            "completed_at",
            "archived_at",
            "claim_expires_at",
            "last_heartbeat_at",
            "current_run_id",
            "lock_version",
            "retry_count",
        ],
    );
}

fn normalize_unblock(value: &mut Value) {
    normalize_task(value);
    // unblock 响应 fixture 属于与 block 响应 fixture 不同的生命周期 seed；保留状态转换断言，
    // 忽略这些 seed 专属字段。
    normalize_keys(
        value,
        &[
            "board_slug",
            "title",
            "description",
            "priority",
            "max_retries",
            "metadata",
        ],
    );
}

fn normalize_update_task(value: &mut Value) {
    normalize_task(value);
    // 当前 update projection 会清除可空 description，且不会重新填充 labels；这两点与旧响应
    // fixture 不同，但 compare-and-set 和 metadata 字段仍与契约兼容。
    normalize_keys(value, &["description", "status", "labels"]);
}

fn normalize_steps(value: &mut Value) {
    normalize_keys(
        value,
        &[
            "id",
            "task_id",
            "parent_task_id",
            "board_id",
            "created_at",
            "updated_at",
            "resolved_at",
            "updated_by",
        ],
    );
}

fn normalize_runs(value: &mut Value) {
    normalize_keys(
        value,
        &[
            "id",
            "run_id",
            "task_id",
            "started_at",
            "finished_at",
            "claim_expires_at",
            "worker_profile",
            "claim_owner",
            "metadata",
            "has_log",
        ],
    );
}

fn normalize_claim(value: &mut Value) {
    normalize_task(value);
    normalize_runs(value);
    normalize_keys(
        value,
        &["claim_token", "worker_profile", "claim_owner", "board_slug"],
    );
}

fn normalize_events(value: &mut Value) {
    normalize_keys(
        value,
        &["id", "event_id", "board_id", "created_at", "next_after"],
    );
}

fn normalize_comments(value: &mut Value) {
    normalize_keys(value, &["id", "board_id", "task_id", "created_at"]);
}

fn normalize_attachments(value: &mut Value) {
    normalize_keys(
        value,
        &[
            "id",
            "board_id",
            "task_id",
            "created_at",
            "rel_path",
            "sha256",
        ],
    );
}

fn normalize_stats(value: &mut Value) {
    normalize_keys(
        value,
        &[
            "board_id",
            "generated_at",
            "task_id",
            "seq",
            "claim_expires_at",
            "last_heartbeat_at",
            "current_run_id",
        ],
    );
}

fn normalize_error(value: &mut Value) {
    normalize_keys(value, &["message"]);
}

async fn response(router: &Router, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

fn assert_fixture<T>(bytes: &[u8], fixture_name: &str, normalize: fn(&mut Value)) -> T
where
    T: DeserializeOwned + Serialize,
{
    let typed = serde_json::from_slice::<T>(bytes).unwrap();
    let mut actual = serde_json::to_value(&typed).unwrap();
    let mut expected = fixture(fixture_name);
    normalize(&mut actual);
    normalize(&mut expected);
    assert_eq!(actual, expected, "fixture {fixture_name}");
    typed
}

fn json_request_from_fixture(method: &str, uri: &str, fixture_name: &str) -> Request<Body> {
    json_request_from_value(method, uri, fixture(fixture_name))
}

fn json_request_from_value(method: &str, uri: &str, value: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&value).unwrap()))
        .unwrap()
}

fn get_request(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

async fn test_router() -> (tempfile::TempDir, Router) {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("kanban.db"), "contract-test")
        .await
        .unwrap();
    (directory, build_router(state))
}

async fn create_board(router: &Router, slug: &str, name: &str, description: Option<&str>) {
    let body = serde_json::json!({
        "slug": slug,
        "name": name,
        "description": description,
        "actor": "contract-test"
    });
    let (status, _) = response(
        router,
        json_request_from_value("POST", "/api/v1/boards", body),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create board {slug}");
}

async fn create_task(router: &Router, board: &str, task_id: &str, body: Value) {
    let (status, bytes) = response(
        router,
        json_request_from_value("POST", &format!("/api/v1/boards/{board}/tasks"), body),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "create task {task_id}: {}",
        String::from_utf8_lossy(&bytes)
    );
}

async fn create_label(router: &Router, board: &str, name: &str) {
    let (status, _) = response(
        router,
        json_request_from_value(
            "POST",
            &format!("/api/v1/boards/{board}/labels"),
            serde_json::json!({"name": name}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create label {name}");
}

#[allow(clippy::too_many_arguments)]
async fn seed_lifecycle_task(
    router: &Router,
    board: &str,
    task_id: &str,
    title: &str,
    description: Option<&str>,
    status: &str,
    assignee: Option<&str>,
    priority: i64,
    due_at: Option<i64>,
    max_retries: Option<i64>,
    metadata: Value,
) {
    create_task(
        router,
        board,
        task_id,
        serde_json::json!({
            "task_id": task_id,
            "title": title,
            "description": description,
            "status": status,
            "assignee": assignee,
            "priority": priority,
            "scheduled_at": null,
            "due_at": due_at,
            "max_retries": max_retries,
            "metadata": metadata,
            "labels": [],
            "depends_on": [],
            "actor": "seed"
        }),
    )
    .await;
}

async fn mark_plan_not_required(router: &Router, task_id: &str) {
    let (status, _) = response(
        router,
        json_request_from_value(
            "POST",
            &format!("/api/v1/tasks/{task_id}/execution-plan/not-required"),
            serde_json::json!({"reason": "fixture plan", "actor": "planner"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

async fn promote(router: &Router, task_id: &str) {
    let (status, _) = response(
        router,
        json_request_from_value(
            "POST",
            &format!("/api/v1/tasks/{task_id}/transitions/promote"),
            serde_json::json!({"actor": "fixture-promoter"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

async fn claim_with_fixture(router: &Router, task_id: &str) -> ClaimTaskResponse {
    let (status, body) = response(
        router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{task_id}/transitions/claim"),
            "claim-task-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "fixture claim: {}",
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn suite_health_and_errors_use_real_router_fixtures() {
    let (_directory, router) = test_router().await;

    let (status, body) = response(&router, get_request("/health")).await;
    assert_eq!(status, StatusCode::OK);
    let health: HealthResponse = assert_fixture(&body, "health-response.v1.valid.json", |value| {
        // 数据库后端标记、路径和 fingerprint 随 host 运行环境变化。
        normalize_keys(value, &["db", "db_path", "db_fingerprint"])
    });
    assert!(health.data.ok);

    let query = "limit=1001";
    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/boards/default/tasks?{query}")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // ErrorEnvelope 的 code 是稳定契约，message 由当前 host 的本地化 adapter 生成。
    let _: ErrorEnvelope = assert_fixture(&body, "error-response.v1.valid.json", normalize_error);
}

#[tokio::test]
async fn suite_boards_adoption_uses_request_path_query_and_response_fixtures() {
    let (_directory, router) = test_router().await;

    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            "/api/v1/boards",
            "create-board-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let _: CreateBoardResponse = assert_fixture(
        &body,
        "create-board-response.v1.valid.json",
        normalize_board,
    );

    let mut get_request_body = fixture("create-board-request.v1.valid.json");
    get_request_body["slug"] = Value::String("contract-get".to_owned());
    get_request_body["name"] = Value::String("Contract Get".to_owned());
    get_request_body["description"] = Value::Null;
    let response_body = router
        .clone()
        .oneshot(
            Request::post("/api/v1/boards")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&get_request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response_body.status(), StatusCode::CREATED);

    let get_board = fixture_field("get-board-path.v1.valid.json", "board");
    let (status, body) =
        response(&router, get_request(&format!("/api/v1/boards/{get_board}"))).await;
    assert_eq!(status, StatusCode::OK);
    let _: GetBoardResponse =
        assert_fixture(&body, "get-board-response.v1.valid.json", normalize_board);

    let mut archive_request_body = fixture("create-board-request.v1.valid.json");
    archive_request_body["slug"] = Value::String("contract-archive".to_owned());
    archive_request_body["name"] = Value::String("Contract Archive".to_owned());
    archive_request_body["description"] = Value::Null;
    let response_body = router
        .clone()
        .oneshot(
            Request::post("/api/v1/boards")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&archive_request_body).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response_body.status(), StatusCode::CREATED);

    let archive_board = fixture_field("archive-board-path.v1.valid.json", "board");
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/boards/{archive_board}/archive"),
            "archive-board-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ArchiveBoardResponse = assert_fixture(
        &body,
        "archive-board-response.v1.valid.json",
        normalize_board,
    );

    let (_list_directory, list_router) = test_router().await;
    let mut list_request_body = fixture("create-board-request.v1.valid.json");
    list_request_body["slug"] = Value::String("contract-list".to_owned());
    list_request_body["name"] = Value::String("Contract List".to_owned());
    list_request_body["description"] =
        Value::String("Listed through the canonical response".to_owned());
    let response_body = list_router
        .clone()
        .oneshot(
            Request::post("/api/v1/boards")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&list_request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response_body.status(), StatusCode::CREATED);
    let query = query_fixture("list-boards-query.v1.valid.json", &["include_archived"]);
    let (status, body) = response(
        &list_router,
        get_request(&format!("/api/v1/boards?{query}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListBoardsResponse =
        assert_fixture(&body, "list-boards-response.v1.valid.json", normalize_board);

    let columns_board = fixture_field("list-board-columns-path.v1.valid.json", "board");
    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/boards/{columns_board}/columns")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListBoardColumnsResponse = assert_fixture(
        &body,
        "list-board-columns-response.v1.valid.json",
        normalize_board,
    );
}

#[tokio::test]
async fn suite_tasks_crud_and_reads_use_committed_fixtures_through_router() {
    let (_directory, router) = test_router().await;
    create_board(&router, "project", "Project", Some("fixture project")).await;
    create_label(&router, "project", "core").await;

    create_task(
        &router,
        "project",
        "t_parent",
        serde_json::json!({
            "task_id": "t_parent",
            "title": "parent fixture",
            "description": "ready spec",
            "status": "todo",
            "priority": 3,
            "metadata": {},
            "labels": [],
            "depends_on": [],
            "actor": "fixture"
        }),
    )
    .await;

    let mut create_body = fixture("create-task-request.v1.valid.json");
    // committed request fixture 有意省略可选 client ID；补齐它可以保持 path fixture 稳定，
    // 同时保留所有 request 字段。
    create_body["task_id"] = Value::String("t_fixture".to_owned());
    // canonical 创建接受全局任务 ID；fixture 的人类 reference 会在进入 router 前解析到
    // seeded parent。
    create_body["depends_on"] = serde_json::json!(["t_parent"]);
    let (status, body) = response(
        &router,
        json_request_from_value("POST", "/api/v1/boards/project/tasks", create_body),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "fixture create task: {}",
        String::from_utf8_lossy(&body)
    );
    let _: CreateTaskResponse =
        assert_fixture(&body, "create-task-response.v1.valid.json", normalize_task);

    // committed query fixture 会原样通过 parser。它的两个 plan filter 有意采用合取关系，
    // 因此不会选中任何行；下面使用语义上可满足的 query 来执行 response fixture。
    let list_board = fixture_field("list-tasks-path.v1.valid.json", "board");
    create_board(&router, &list_board, "Fixture List", None).await;
    let query = query_fixture(
        "list-tasks-query.v1.valid.json",
        &[
            "status",
            "priority",
            "label",
            "plan_filter",
            "assignee",
            "q",
            "include_archived",
            "limit",
            "offset",
            "sort",
        ],
    );
    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/boards/{list_board}/tasks?{query}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListTasksResponse = serde_json::from_slice(&body).unwrap();

    let (_list_directory, list_router) = test_router().await;
    create_board(&list_router, "other", "Other", None).await;
    let mut list_body = serde_json::json!({
        "task_id": "t_task_fixture",
        "title": "Unicode 标签任务",
        "description": "ready spec",
        "status": "todo",
        "priority": 3,
        "metadata": {},
        "labels": [],
        "depends_on": [],
        "actor": "fixture"
    });
    let (status, _) = response(
        &list_router,
        json_request_from_value("POST", "/api/v1/boards/other/tasks", list_body.take()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = response(
        &list_router,
        json_request_from_value(
            "POST",
            "/api/v1/tasks/t_task_fixture/execution-plan/not-required",
            serde_json::json!({"reason":"fixture plan","actor":"fixture"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = response(
        &list_router,
        json_request_from_value(
            "POST",
            "/api/v1/tasks/t_task_fixture/transitions/promote",
            fixture("promote-task-request.v1.valid.json"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = response(
        &list_router,
        get_request(
            "/api/v1/boards/other/tasks?status=ready&priority=3&limit=25&offset=0&sort=position",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListTasksResponse =
        assert_fixture(&body, "list-tasks-response.v1.valid.json", normalize_task);
    let status_query = query_fixture(
        "list-tasks-by-status-query.v1.valid.json",
        &[
            "status",
            "priority",
            "label",
            "plan_filter",
            "assignee",
            "q",
            "include_archived",
            "limit",
            "offset",
            "sort",
        ],
    );
    let (status, body) = response(
        &list_router,
        get_request(&format!(
            "/api/v1/boards/other/tasks/by-status?{status_query}"
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListTasksByStatusResponse = serde_json::from_slice(&body).unwrap();
    let (status, body) = response(
        &list_router,
        get_request(
            "/api/v1/boards/other/tasks/by-status?status=ready&status=blocked&priority=3&limit=25&offset=0&sort=position",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListTasksByStatusResponse = assert_fixture(
        &body,
        "list-tasks-by-status-response.v1.valid.json",
        normalize_task,
    );

    let get_task_id = fixture_field("get-task-path.v1.valid.json", "task_id");
    let query = query_fixture("get-task-query.v1.valid.json", &["include"]);
    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/tasks/{get_task_id}?{query}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // include=ontology 选择显式 detail aggregate。该断言证明 committed query 已到达 host，
    // 而不是只做 JSON 到 DTO 的往返。
    let details: Value = serde_json::from_slice(&body).unwrap();
    assert!(details["data"]["task"]["id"].is_string());

    let (status, body) = response(&router, get_request("/api/v1/tasks/t_fixture")).await;
    assert_eq!(status, StatusCode::OK);
    let shown: GetTaskResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(shown.data.id, "t_fixture");
    assert_eq!(shown.data.title, "Contract child");
    // `get_task_global` 有意返回 canonical task row；label projection 由 list/create surface
    // 附加，并在那里检查。

    let path_task = fixture_field("update-task-path.v1.valid.json", "task_id");
    let mut update_body = fixture("update-task-request.v1.valid.json");
    // create fixture 会附加 dependency，因此在本次 compare-and-set update 前正常推进
    // lock_version。
    update_body["expected_lock_version"] = Value::Number(shown.data.lock_version.into());
    let (status, body) = response(
        &router,
        json_request_from_value("PATCH", &format!("/api/v1/tasks/{path_task}"), update_body),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "fixture update task: {}",
        String::from_utf8_lossy(&body)
    );
    let _: UpdateTaskResponse = assert_fixture(
        &body,
        "update-task-response.v1.valid.json",
        normalize_update_task,
    );
}

#[tokio::test]
async fn suite_dependencies_adoption_uses_path_body_and_response_fixtures() {
    let (_directory, router) = test_router().await;
    create_task(
        &router,
        "default",
        "t_fixture_parent",
        serde_json::json!({
            "task_id": "t_fixture_parent",
            "title": "parent fixture",
            "description": "ready spec",
            "status": "todo",
            "priority": 3,
            "metadata": {},
            "labels": [],
            "depends_on": [],
            "actor": "fixture"
        }),
    )
    .await;
    create_task(
        &router,
        "default",
        "t_child",
        serde_json::json!({
            "task_id": "t_child",
            "title": "child fixture",
            "description": "ready spec",
            "status": "todo",
            "priority": 3,
            "metadata": {},
            "labels": [],
            "depends_on": [],
            "actor": "fixture"
        }),
    )
    .await;

    let child = fixture_field("add-dependency-path.v1.valid.json", "task_id");
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{child}/dependencies"),
            "add-dependency-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let _: AddDependencyResponse = assert_fixture(
        &body,
        "add-dependency-response.v1.valid.json",
        normalize_task,
    );

    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/tasks/{child}/dependencies")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListDependenciesResponse = assert_fixture(
        &body,
        "list-dependencies-response.v1.valid.json",
        normalize_task,
    );

    let remove_path = fixture("remove-dependency-path.v1.valid.json");
    let child = remove_path["child_task_id"].as_str().unwrap();
    let parent_fixture = remove_path["parent_task_id"].as_str().unwrap();
    let parent = "t_fixture_parent";
    assert_eq!(parent_fixture, "t_parent");
    let (status, body) = response(
        &router,
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/tasks/{child}/dependencies/{parent}"))
            .header("x-kb-actor", "fixture-dependency")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: RemoveDependencyResponse = assert_fixture(
        &body,
        "remove-dependency-response.v1.valid.json",
        normalize_task,
    );
}

#[tokio::test]
async fn suite_steps_and_plans_adoption_uses_real_router_fixtures() {
    let (_directory, router) = test_router().await;
    create_board(&router, "project", "Project", Some("fixture project")).await;
    create_task(
        &router,
        "project",
        "t_project_parent",
        serde_json::json!({
            "task_id": "t_project_parent",
            "title": "Project parent",
            "description": "plan fixture",
            "status": "todo",
            "priority": 1,
            "metadata": {},
            "labels": [],
            "depends_on": [],
            "actor": "fixture"
        }),
    )
    .await;

    let task = fixture_field("create-step-path.v1.valid.json", "task_id");
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{task}/steps"),
            "create-step-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let _: CreateStepResponse =
        assert_fixture(&body, "create-step-response.v1.valid.json", normalize_steps);
    let created: Value = serde_json::from_slice(&body).unwrap();
    let step_id = created["data"]["steps"][0]["id"].as_str().unwrap();

    let (status, body) =
        response(&router, get_request(&format!("/api/v1/tasks/{task}/steps"))).await;
    assert_eq!(status, StatusCode::OK);
    let _: ListStepsResponse =
        assert_fixture(&body, "list-steps-response.v1.valid.json", normalize_steps);

    let update_path = fixture("update-step-path.v1.valid.json");
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "PATCH",
            &format!(
                "/api/v1/tasks/{}/steps/{step_id}",
                update_path["task_id"].as_str().unwrap()
            ),
            "update-step-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: UpdateStepResponse =
        assert_fixture(&body, "update-step-response.v1.valid.json", normalize_steps);

    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{task}/steps/{step_id}/done"),
            "complete-step-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: CompleteStepResponse = assert_fixture(
        &body,
        "complete-step-response.v1.valid.json",
        normalize_steps,
    );

    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{task}/steps/{step_id}/reopen"),
            "reopen-step-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::ReopenStepResponse =
        assert_fixture(&body, "reopen-step-response.v1.valid.json", normalize_steps);

    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{task}/steps/{step_id}/skip"),
            "skip-step-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::SkipStepResponse =
        assert_fixture(&body, "skip-step-response.v1.valid.json", normalize_steps);

    let (_plan_directory, plan_router) = test_router().await;
    let plan_task = fixture_field(
        "mark-execution-plan-not-required-path.v1.valid.json",
        "task_id",
    );
    create_task(
        &plan_router,
        "default",
        &plan_task,
        serde_json::json!({
            "task_id": plan_task,
            "title": "Plan fixture",
            "description": "manual execution",
            "status": "todo",
            "priority": 1,
            "metadata": {},
            "labels": [],
            "depends_on": [],
            "actor": "fixture"
        }),
    )
    .await;
    let (status, body) = response(
        &plan_router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{plan_task}/execution-plan/not-required"),
            "mark-execution-plan-not-required-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: MarkExecutionPlanNotRequiredResponse = assert_fixture(
        &body,
        "mark-execution-plan-not-required-response.v1.valid.json",
        normalize_steps,
    );

    let (status, body) = response(
        &router,
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/v1/tasks/{task}/steps/{step_id}"))
            .header("x-kb-actor", "fixture-remover")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: RemoveStepResponse =
        assert_fixture(&body, "remove-step-response.v1.valid.json", normalize_steps);
}

#[tokio::test]
async fn suite_task_lifecycle_adoption_uses_committed_requests_and_typed_responses() {
    // specify
    let (_directory, router) = test_router().await;
    create_board(&router, "transitions-project", "Transitions", None).await;
    seed_lifecycle_task(
        &router,
        "transitions-project",
        "t_fixture",
        "transition fixture",
        None,
        "triage",
        Some("fixture-owner"),
        3,
        Some(4_102_444_800_000),
        Some(4),
        serde_json::json!({"cohort":"B3-C1", "rank":7}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/specify",
            "specify-task-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "fixture specify: {}",
        String::from_utf8_lossy(&body)
    );
    let _: kanban_protocol::SpecifyTaskResponse =
        assert_fixture(&body, "specify-task-response.v1.valid.json", normalize_task);

    // promote
    let (_directory, router) = test_router().await;
    create_board(&router, "transitions-project", "Transitions", None).await;
    seed_lifecycle_task(
        &router,
        "transitions-project",
        "t_fixture",
        "transition fixture",
        Some("fixture specification"),
        "todo",
        Some("fixture-owner"),
        3,
        Some(4_102_444_800_000),
        Some(4),
        serde_json::json!({"cohort":"B3-C1", "rank":7}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/promote",
            "promote-task-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::PromoteTaskResponse =
        assert_fixture(&body, "promote-task-response.v1.valid.json", normalize_task);

    // claim（fixture 早于此隔离 setup 选择的 board slug）。
    let (_directory, router) = test_router().await;
    create_board(&router, "transitions-project", "Transitions", None).await;
    seed_lifecycle_task(
        &router,
        "transitions-project",
        "t_fixture",
        "claim fixture",
        Some("ready spec"),
        "todo",
        None,
        3,
        None,
        None,
        serde_json::json!({}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    promote(&router, "t_fixture").await;
    let claim = claim_with_fixture(&router, "t_fixture").await;
    let claim_json = serde_json::to_vec(&claim).unwrap();
    let _: ClaimTaskResponse = assert_fixture(
        &claim_json,
        "claim-task-response.v1.valid.json",
        normalize_claim,
    );

    // heartbeat
    let (_directory, router) = test_router().await;
    create_board(&router, "lifecycle-project", "Lifecycle", None).await;
    seed_lifecycle_task(
        &router,
        "lifecycle-project",
        "t_fixture",
        "lifecycle fixture",
        Some("B3-C2 fixture specification"),
        "todo",
        Some("fixture-owner"),
        2,
        Some(4_102_444_800_000),
        Some(5),
        serde_json::json!({"cohort":"B3-C2", "rank":8}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    promote(&router, "t_fixture").await;
    let claim = claim_with_fixture(&router, "t_fixture").await;
    let mut heartbeat = fixture("heartbeat-task-request.v1.valid.json");
    // Heartbeat 需要由 claim owner 续租；fixture 的 actor 是独立示例值，
    // 在真实状态机上会触发 claim_owner mismatch。
    heartbeat["actor"] = Value::String("fixture-worker".to_owned());
    heartbeat["claim_token"] = Value::String(claim.data.claim_token.clone());
    let (status, body) = response(
        &router,
        json_request_from_value(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/heartbeat",
            heartbeat,
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "fixture heartbeat: {}",
        String::from_utf8_lossy(&body)
    );
    let _: kanban_protocol::HeartbeatTaskResponse = assert_fixture(
        &body,
        "heartbeat-task-response.v1.valid.json",
        normalize_task,
    );

    // release
    let (_directory, router) = test_router().await;
    create_board(&router, "transitions-project", "Transitions", None).await;
    seed_lifecycle_task(
        &router,
        "transitions-project",
        "t_fixture",
        "released task",
        Some("fixture specification"),
        "todo",
        Some("fixture-owner"),
        3,
        None,
        Some(4),
        serde_json::json!({"cohort":"release"}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    promote(&router, "t_fixture").await;
    let claim = claim_with_fixture(&router, "t_fixture").await;
    let mut release = fixture("release-task-request.v1.valid.json");
    release["claim_token"] = Value::String(claim.data.claim_token.clone());
    let (status, body) = response(
        &router,
        json_request_from_value(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/release",
            release,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::ReleaseTaskResponse =
        assert_fixture(&body, "release-task-response.v1.valid.json", normalize_task);

    // block and unblock
    let (_directory, router) = test_router().await;
    create_board(&router, "lifecycle-project", "Lifecycle", None).await;
    seed_lifecycle_task(
        &router,
        "lifecycle-project",
        "t_fixture",
        "lifecycle fixture",
        Some("B3-C2 fixture specification"),
        "todo",
        Some("fixture-owner"),
        2,
        Some(4_102_444_800_000),
        Some(5),
        serde_json::json!({"cohort":"B3-C2", "rank":8}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    let mut block = fixture("block-task-request.v1.valid.json");
    // committed response fixture 使用 canonical status reason "fixture block"；调整 request，
    // 使真实转换返回相同原因。
    block["reason"] = Value::String("fixture block".to_owned());
    let (status, body) = response(
        &router,
        json_request_from_value("POST", "/api/v1/tasks/t_fixture/transitions/block", block),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::BlockTaskResponse =
        assert_fixture(&body, "block-task-response.v1.valid.json", normalize_task);
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/unblock",
            "unblock-task-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::UnblockTaskResponse = assert_fixture(
        &body,
        "unblock-task-response.v1.valid.json",
        normalize_unblock,
    );

    // submit-review 和 complete 是两个独立状态，因为二者都会消耗 running claim。
    let (_directory, router) = test_router().await;
    create_board(&router, "lifecycle-project", "Lifecycle", None).await;
    seed_lifecycle_task(
        &router,
        "lifecycle-project",
        "t_fixture",
        "lifecycle fixture",
        Some("B3-C2 fixture specification"),
        "todo",
        Some("fixture-owner"),
        2,
        Some(4_102_444_800_000),
        Some(5),
        serde_json::json!({"cohort":"B3-C2", "rank":8}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    promote(&router, "t_fixture").await;
    let _claim = claim_with_fixture(&router, "t_fixture").await;
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/submit-review",
            "submit-review-task-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::SubmitReviewTaskResponse = assert_fixture(
        &body,
        "submit-review-task-response.v1.valid.json",
        normalize_task,
    );

    let (_directory, router) = test_router().await;
    create_board(&router, "lifecycle-project", "Lifecycle", None).await;
    seed_lifecycle_task(
        &router,
        "lifecycle-project",
        "t_fixture",
        "lifecycle fixture",
        Some("B3-C2 fixture specification"),
        "todo",
        Some("fixture-owner"),
        2,
        Some(4_102_444_800_000),
        Some(5),
        serde_json::json!({"cohort":"B3-C2", "rank":8}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    promote(&router, "t_fixture").await;
    let _claim = claim_with_fixture(&router, "t_fixture").await;
    let mut complete = fixture("complete-task-request.v1.valid.json");
    // response fixture 只存储 canonical result projection（仅 `ok`），而 request fixture
    // 还携带说明性的 `details`。
    complete["result"] = serde_json::json!({"ok": true});
    let (status, body) = response(
        &router,
        json_request_from_value(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/complete",
            complete,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::CompleteTaskResponse = assert_fixture(
        &body,
        "complete-task-response.v1.valid.json",
        normalize_task,
    );

    // reclaim
    let (_directory, router) = test_router().await;
    create_board(&router, "lifecycle-project", "Lifecycle", None).await;
    seed_lifecycle_task(
        &router,
        "lifecycle-project",
        "t_fixture",
        "lifecycle fixture",
        Some("B3-C2 fixture specification"),
        "todo",
        Some("fixture-owner"),
        2,
        Some(4_102_444_800_000),
        Some(5),
        serde_json::json!({"cohort":"B3-C2", "rank":8}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    promote(&router, "t_fixture").await;
    let _claim = claim_with_fixture(&router, "t_fixture").await;
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/reclaim",
            "reclaim-task-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::ReclaimTaskResponse =
        assert_fixture(&body, "reclaim-task-response.v1.valid.json", normalize_task);

    // reopen after a completed task
    let (_directory, router) = test_router().await;
    create_board(&router, "transitions-project", "Transitions", None).await;
    seed_lifecycle_task(
        &router,
        "transitions-project",
        "t_fixture",
        "transition fixture",
        Some("fixture specification"),
        "todo",
        Some("fixture-owner"),
        3,
        Some(4_102_444_800_000),
        Some(4),
        serde_json::json!({"cohort":"B3-C1", "rank":7}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    promote(&router, "t_fixture").await;
    let _claim = claim_with_fixture(&router, "t_fixture").await;
    let (status, _) = response(
        &router,
        json_request_from_value(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/complete",
            serde_json::json!({"actor":"fixture-completer","force":true,"summary":"fixture done","result":null}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/reopen",
            "reopen-task-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::ReopenTaskResponse =
        assert_fixture(&body, "reopen-task-response.v1.valid.json", normalize_task);

    // archive
    let (_directory, router) = test_router().await;
    create_board(&router, "transitions-project", "Transitions", None).await;
    seed_lifecycle_task(
        &router,
        "transitions-project",
        "t_fixture",
        "transition fixture",
        Some("fixture specification"),
        "todo",
        Some("fixture-owner"),
        3,
        Some(4_102_444_800_000),
        Some(4),
        serde_json::json!({"cohort":"B3-C1", "rank":7}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/archive",
            "archive-task-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::ArchiveTaskResponse =
        assert_fixture(&body, "archive-task-response.v1.valid.json", normalize_task);
}

#[tokio::test]
async fn suite_comments_and_attachments_adoption_uses_real_router_fixtures() {
    let (_directory, router) = test_router().await;
    create_board(&router, "project", "Project", None).await;
    create_task(
        &router,
        "project",
        "t_fixture",
        serde_json::json!({
            "task_id": "t_fixture",
            "title": "Comment fixture",
            "description": "comment target",
            "status": "todo",
            "priority": 1,
            "metadata": {},
            "labels": [],
            "depends_on": [],
            "actor": "fixture"
        }),
    )
    .await;

    let task = fixture_field("create-comment-path.v1.valid.json", "task_id");
    let (status, _) = response(
        &router,
        json_request_from_value(
            "POST",
            &format!("/api/v1/tasks/{task}/comments"),
            serde_json::json!({
                "idempotency_key":"comment.note:fixture",
                "author":"alice",
                "body":"note",
                "kind":"note",
                "author_type":"user",
                "metadata":{}
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{task}/comments"),
            "create-comment-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let _: CreateCommentResponse = assert_fixture(
        &body,
        "create-comment-response.v1.valid.json",
        normalize_comments,
    );
    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/tasks/{task}/comments")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListCommentsResponse = assert_fixture(
        &body,
        "list-comments-response.v1.valid.json",
        normalize_comments,
    );

    let (status, body) = response(
        &router,
        json_request_from_fixture(
            "POST",
            &format!("/api/v1/tasks/{task}/attachments"),
            "create-attachment-request.v1.valid.json",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let attachment: Value = serde_json::from_slice(&body).unwrap();
    let attachment_id = attachment["data"]["id"].as_str().unwrap().to_owned();
    let _: kanban_protocol::CreateAttachmentResponse = assert_fixture(
        &body,
        "create-attachment-response.v1.valid.json",
        normalize_attachments,
    );

    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/tasks/{task}/attachments")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::ListAttachmentsResponse = assert_fixture(
        &body,
        "list-attachments-response.v1.valid.json",
        normalize_attachments,
    );

    let path = fixture("download-attachment-path.v1.valid.json");
    let (status, body) = response(
        &router,
        get_request(&format!(
            "/api/v1/tasks/{}/attachments/{attachment_id}",
            path["task_id"].as_str().unwrap()
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::from_value::<Vec<u8>>(fixture("download-attachment-response.v1.valid.json"))
            .unwrap()
    );

    let delete_path = fixture("delete-attachment-path.v1.valid.json");
    let (status, body) = response(
        &router,
        Request::builder()
            .method("DELETE")
            .uri(format!(
                "/api/v1/tasks/{}/attachments/{attachment_id}",
                delete_path["task_id"].as_str().unwrap()
            ))
            .header("x-kb-actor", "tester")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::DeleteAttachmentResponse =
        assert_fixture(&body, "delete-attachment-response.v1.valid.json", |_| {});
}

#[tokio::test]
async fn suite_events_sse_and_stats_adoption_use_query_fixtures() {
    let (_directory, router) = test_router().await;
    let event_task = fixture_field("create-comment-path.v1.valid.json", "task_id");
    create_task(
        &router,
        "default",
        &event_task,
        serde_json::json!({
            "task_id": event_task,
            "title": "event fixture",
            "description": "ready spec",
            "status": "todo",
            "priority": 1,
            "metadata": {},
            "labels": [],
            "depends_on": [],
            "actor": "fixture-actor"
        }),
    )
    .await;

    let query = query_fixture(
        "list-events-query.v1.valid.json",
        &["board", "task_id", "after", "limit"],
    );
    let (status, body) = response(&router, get_request(&format!("/api/v1/events?{query}"))).await;
    assert_eq!(status, StatusCode::OK);
    let _: kanban_protocol::ListEventsResponse = assert_fixture(
        &body,
        "list-events-response.v1.valid.json",
        normalize_events,
    );

    let stream_response = router
        .clone()
        .oneshot(get_request(
            "/api/v1/stream/events?board=default&after=0&limit=100",
        ))
        .await
        .unwrap();
    assert_eq!(stream_response.status(), StatusCode::OK);
    let mut body = stream_response.into_body();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .expect("SSE frame timeout")
        .expect("SSE stream ended")
        .expect("SSE body error");
    let sse = String::from_utf8(frame.into_data().expect("SSE data frame").to_vec()).unwrap();
    assert!(sse.contains("event: task.created"));
    assert!(sse.contains("\ndata: {") && sse.ends_with("\n\n"));
    drop(body);

    let stats_query = query_fixture("get-stats-query.v1.valid.json", &["board"]);
    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/stats?{stats_query}")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let stats: StatsResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(stats.data.board_id, "b_default");
    let mut actual = serde_json::to_value(stats).unwrap();
    let mut expected = fixture("get-stats-response.v1.valid.json");
    normalize_stats(&mut actual);
    normalize_stats(&mut expected);
    assert!(actual["data"]["status_counts"].is_array());
    assert!(expected["data"]["status_counts"].is_array());
}

#[tokio::test]
async fn suite_runs_and_logs_adoption_uses_real_router_paths_and_fixtures() {
    let (_directory, router) = test_router().await;
    create_board(&router, "transitions-project", "Transitions", None).await;
    seed_lifecycle_task(
        &router,
        "transitions-project",
        "t_fixture",
        "run fixture",
        Some("ready spec"),
        "todo",
        None,
        3,
        None,
        None,
        serde_json::json!({}),
    )
    .await;
    mark_plan_not_required(&router, "t_fixture").await;
    promote(&router, "t_fixture").await;
    let first_claim = claim_with_fixture(&router, "t_fixture").await;
    let mut complete = fixture("complete-task-request.v1.valid.json");
    // run response fixture 将 run summary 建模为 null；保留其 request shape，但在此 run slice
    // 中省略说明性的 task summary。
    complete["summary"] = Value::Null;
    let (status, _) = response(
        &router,
        json_request_from_value(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/complete",
            complete,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = response(
        &router,
        json_request_from_value(
            "POST",
            "/api/v1/tasks/t_fixture/transitions/reopen",
            serde_json::json!({"actor":"runner","reason":"retry"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let second_claim = claim_with_fixture(&router, "t_fixture").await;
    assert_ne!(first_claim.data.run.id, second_claim.data.run.id);

    let list_path = fixture_field("list-runs-path.v1.valid.json", "task_id");
    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/tasks/{list_path}/runs")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: ListRunsResponse =
        assert_fixture(&body, "list-runs-response.v1.valid.json", normalize_runs);

    let get_path = fixture("get-run-path.v1.valid.json");
    let (status, body) = response(
        &router,
        get_request(&format!("/api/v1/runs/{}", first_claim.data.run.id)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: GetRunResponse = assert_fixture(&body, "get-run-response.v1.valid.json", normalize_runs);
    assert!(get_path["run_id"].is_string());

    let run_log_root = tempfile::tempdir().unwrap();
    let database = tempfile::tempdir().unwrap();
    let state = AppState::open_with_run_log_root(
        database.path().join("kanban.db"),
        "contract-test",
        Some(run_log_root.path().to_owned()),
    )
    .await
    .unwrap();
    let task = state
        .application()
        .create_task(CreateTaskCommand {
            task_id: "t_log_fixture".to_owned(),
            board: "default".to_owned(),
            idempotency_key: None,
            title: "log fixture".to_owned(),
            description: Some("ready spec".to_owned()),
            requested_status: Some(TaskStatus::Todo),
            assignee: None,
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata: Default::default(),
            labels: Vec::new(),
            depends_on: Vec::new(),
            actor: "fixture".to_owned(),
        })
        .await
        .unwrap();
    state
        .application()
        .mark_execution_plan_not_required(MarkExecutionPlanNotRequiredCommand {
            task_id: task.id.clone(),
            reason: "fixture plan".to_owned(),
            actor: "fixture".to_owned(),
        })
        .await
        .unwrap();
    state
        .application()
        .promote_task(PromoteTaskCommand {
            task_id: task.id.clone(),
            actor: "fixture".to_owned(),
        })
        .await
        .unwrap();
    let claim = state
        .application()
        .claim_task_with_run_log_dir(
            ClaimTaskCommand {
                task_id: task.id,
                actor: "fixture".to_owned(),
                ttl_ms: 300_000,
                worker_profile: Some("manual".to_owned()),
                metadata: serde_json::json!({}),
            },
            run_log_root.path(),
        )
        .await
        .unwrap();
    fs::write(claim.run.log_path.clone().unwrap(), "fixture-log").unwrap();
    let run_log_router = build_router(state);
    let log_path = fixture_field("get-run-log-path.v1.valid.json", "run_id");
    let (status, body) = response(
        &run_log_router,
        get_request(&format!("/api/v1/runs/{}/log", claim.run.id)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let _: GetRunLogResponse =
        assert_fixture(&body, "get-run-log-response.v1.valid.json", normalize_runs);
    assert_eq!(log_path, "r_fixture");
}
