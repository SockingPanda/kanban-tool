//! 知识面 HTTP 契约的真实 host adoption tests。
//!
//! 这些测试只通过 `build_router` 发请求，并用已提交的 fixture 作为请求/响应
//! DTO 的输入。时间戳、ULID 和 board id 等由真实 store 生成的字段在断言前
//! 做局部归一化；不以 `serde_json::Value` 代替 wire DTO。

use std::collections::BTreeMap;

use axum::{
    Router,
    body::Body,
    http::{Method, Request, Response, StatusCode},
};
use http_body_util::BodyExt;
use kanban_protocol::{
    AddTaskLabelRequest, AddTaskLabelResponse, ApiCreateTaskStatus, ApiTask, ArchiveBoardRequest,
    ArchiveBoardResponse, BoardLabelPath, BoardQuery, BoardTaskMapPath, BoardTaskMapQuery,
    BoardTaskMapResponse, BuildContextPath, BuildContextQuery, BuildContextResponse,
    ConfirmSignalsResponse, CreateBoardLabelRequest, CreateBoardLabelResponse, CreateBoardRequest,
    CreateBoardResponse, CreateTaskRequest, CreateTaskResponse, EntityListQuery,
    EntityListResponse, EntityPath, EntityResponse, EntityUpsertRequest, ErrorEnvelope,
    GetLabelOntologySignalResponse, GetLabelProposalResponse, GetSignalResponse,
    GraphMaintenanceResponse, GraphNeighborsQuery, GraphNeighborsResponse, GraphQueryQuery,
    GraphStatusResponse, LabelAtomIndexStatusResponse, LabelOntologyActionRequest,
    LabelOntologyActionResponse, LabelOntologyReviewQuery, LabelOntologySignalQuery,
    LabelOntologySignalWire, LabelOntologySignalsResponse, LabelProposalCandidateWire,
    LabelProposalDecisionRequest, LabelSemanticsPath, ListBoardsResponse, ListLabelAtomsResponse,
    ListLabelSemanticsResponse, ListSignalsResponse, ListTaskLabelProposalsResponse,
    MetadataEnvelope, ProposalPath, ProposeTaskLabelRequest, ProposeTaskLabelResponse,
    RecordLabelOntologyObservationRequest, RecordLabelOntologyObservationResponse,
    RecordSignalRequest, RecordSignalResponse, RemoveTaskLabelResponse, ReviewSignalsRequest,
    SearchStatusResponse, SearchTasksByStatusResponse, SearchTasksQuery, SearchTasksResponse,
    SignalFilterMeta, SignalPath, TaskNeighborhoodPath, TaskNeighborhoodQuery,
    TaskNeighborhoodResponse, UpsertLabelSemanticsRequest, UpsertLabelSemanticsResponse,
    VectorConfigureRequest, VectorConfigureResponse, VectorProjectionRequest,
    VectorProjectionResponse, VectorQuery, VectorStatusQuery, VectorStatusResponse,
};
use serde::{Serialize, de::DeserializeOwned};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::{router::build_router, state::AppState};
use kanban_service::RelationUpsertCommand;

macro_rules! fixture {
    ($ty:ty, $path:literal) => {{
        serde_json::from_str::<$ty>(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/",
            $path
        )))
        .unwrap_or_else(|error| panic!("fixture {}: {error}", $path))
    }};
}

async fn test_router() -> (TempDir, Router) {
    let (directory, _state, router) = test_router_with_state().await;
    (directory, router)
}

async fn test_router_with_state() -> (TempDir, AppState, Router) {
    let directory = tempfile::tempdir().expect("temporary database directory");
    let state = AppState::open(directory.path().join("kanban.db"), "test")
        .await
        .expect("open state");
    let router = build_router(state.clone());
    (directory, state, router)
}

async fn response_json<T: DeserializeOwned>(response: Response<Body>) -> T {
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("decode {status} response: {error}; body={bytes:?}"))
}

fn request_json<T: Serialize>(method: Method, uri: &str, body: &T) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(body).expect("request JSON")))
        .expect("JSON request")
}

fn request_json_with_headers<T: Serialize>(
    method: Method,
    uri: &str,
    body: &T,
    headers: &BTreeMap<String, String>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    builder
        .body(Body::from(serde_json::to_vec(body).expect("request JSON")))
        .expect("JSON request with headers")
}

fn request_empty_with_headers(
    method: Method,
    uri: &str,
    headers: &BTreeMap<String, String>,
) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    builder
        .body(Body::empty())
        .expect("empty request with headers")
}

fn request_empty(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .expect("empty request")
}

fn actor_headers() -> BTreeMap<String, String> {
    fixture!(BTreeMap<String, String>, "headers/locale-actor-json-headers.v1.valid.json")
}

fn header_fixture(path: &str) -> BTreeMap<String, String> {
    match path {
        "locale-headers" => fixture!(
            BTreeMap<String, String>,
            "headers/locale-headers.v1.valid.json"
        ),
        "locale-json-headers" => fixture!(
            BTreeMap<String, String>,
            "headers/locale-json-headers.v1.valid.json"
        ),
        "locale-actor-headers" => fixture!(
            BTreeMap<String, String>,
            "headers/locale-actor-headers.v1.valid.json"
        ),
        "locale-actor-json-headers" => fixture!(
            BTreeMap<String, String>,
            "headers/locale-actor-json-headers.v1.valid.json"
        ),
        "locale-actor-optional-json-headers" => fixture!(
            BTreeMap<String, String>,
            "headers/locale-actor-optional-json-headers.v1.valid.json"
        ),
        _ => panic!("unknown header fixture {path}"),
    }
}

async fn create_task(router: &Router, board: &str, task_id: &str, title: &str) -> ApiTask {
    let request = CreateTaskRequest {
        task_id: Some(task_id.to_owned()),
        idempotency_key: None,
        title: title.to_owned(),
        description: Some("fixture-backed host task".to_owned()),
        status: Some(ApiCreateTaskStatus::Todo),
        assignee: None,
        priority: 3,
        scheduled_at: None,
        due_at: None,
        max_retries: Some(2),
        metadata: None,
        labels: Vec::new(),
        depends_on: Vec::new(),
        actor: Some("fixture-test".to_owned()),
    };
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/boards/{board}/tasks"),
            &request,
        ))
        .await
        .expect("create task response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: CreateTaskResponse = response_json(response).await;
    created.data
}

async fn create_assigned_task(router: &Router, board: &str, task_id: &str, title: &str) {
    let request = CreateTaskRequest {
        task_id: Some(task_id.to_owned()),
        idempotency_key: None,
        title: title.to_owned(),
        description: Some("derived query witness".to_owned()),
        status: Some(ApiCreateTaskStatus::Todo),
        assignee: Some("agent".to_owned()),
        priority: 3,
        scheduled_at: None,
        due_at: None,
        max_retries: None,
        metadata: None,
        labels: Vec::new(),
        depends_on: Vec::new(),
        actor: Some("fixture".to_owned()),
    };
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/boards/{board}/tasks"),
            &request,
        ))
        .await
        .expect("create assigned task response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let _: CreateTaskResponse = response_json(response).await;
}

async fn create_dependent_task(router: &Router, board: &str, task_id: &str, parent_id: &str) {
    let request = CreateTaskRequest {
        task_id: Some(task_id.to_owned()),
        idempotency_key: None,
        title: "fixture graph child".to_owned(),
        description: Some("relation witness".to_owned()),
        status: Some(ApiCreateTaskStatus::Todo),
        assignee: None,
        priority: 3,
        scheduled_at: None,
        due_at: None,
        max_retries: None,
        metadata: None,
        labels: Vec::new(),
        depends_on: vec![parent_id.to_owned()],
        actor: Some("fixture".to_owned()),
    };
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/boards/{board}/tasks"),
            &request,
        ))
        .await
        .expect("create dependent task response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let _: CreateTaskResponse = response_json(response).await;
}

async fn create_board(router: &Router, slug: &str) {
    let request = CreateBoardRequest {
        slug: slug.to_owned(),
        name: slug.to_owned(),
        description: Some("fixture board".to_owned()),
        actor: Some("fixture-test".to_owned()),
    };
    let response = router
        .clone()
        .oneshot(request_json(Method::POST, "/api/v1/boards", &request))
        .await
        .expect("create board response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let _: CreateBoardResponse = response_json(response).await;
}

async fn create_label(
    router: &Router,
    board: &str,
    request: CreateBoardLabelRequest,
) -> CreateBoardLabelResponse {
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/boards/{board}/labels"),
            &request,
        ))
        .await
        .expect("create label response");
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await
}

#[tokio::test]
async fn labels_semantics_and_atoms_use_committed_fixtures_through_host() {
    let (_directory, router) = test_router().await;
    create_board(&router, "fixture").await;
    let _task = create_task(&router, "fixture", "t_fixture", "fixture semantics task").await;
    let create_request: CreateBoardLabelRequest = fixture!(
        CreateBoardLabelRequest,
        "create-board-label-request.v1.valid.json"
    );
    let label = create_label(&router, "fixture", create_request).await;

    let atom_path: BoardLabelPath = fixture!(BoardLabelPath, "list-label-atoms-path.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/boards/{}/labels/atoms", atom_path.board),
        ))
        .await
        .expect("empty list atoms response");
    assert_eq!(response.status(), StatusCode::OK);
    let actual: ListLabelAtomsResponse = response_json(response).await;
    let expected_atoms: ListLabelAtomsResponse = fixture!(
        ListLabelAtomsResponse,
        "list-label-atoms-response.v1.valid.json"
    );
    assert_eq!(actual, expected_atoms);

    let board_path: BoardLabelPath =
        fixture!(BoardLabelPath, "list-label-semantics-path.v1.valid.json");
    assert_eq!(board_path.board, "fixture");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/boards/{}/labels/semantics", board_path.board),
        ))
        .await
        .expect("list semantics response");
    assert_eq!(response.status(), StatusCode::OK);
    let actual: ListLabelSemanticsResponse = response_json(response).await;
    let expected: ListLabelSemanticsResponse = fixture!(
        ListLabelSemanticsResponse,
        "list-label-semantics-response.v1.valid.json"
    );
    assert_eq!(actual, expected);

    let mut semantics_path: LabelSemanticsPath = fixture!(
        LabelSemanticsPath,
        "upsert-label-semantics-path.v1.valid.json"
    );
    semantics_path.label_id = label.data.id.clone();
    let request: UpsertLabelSemanticsRequest = fixture!(
        UpsertLabelSemanticsRequest,
        "upsert-label-semantics-request.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_json(
            Method::PUT,
            &format!(
                "/api/v1/boards/{}/labels/{}/semantics",
                semantics_path.board, semantics_path.label_id
            ),
            &request,
        ))
        .await
        .expect("upsert semantics response");
    assert_eq!(response.status(), StatusCode::OK);
    let actual: UpsertLabelSemanticsResponse = response_json(response).await;
    let expected_upsert: UpsertLabelSemanticsResponse = fixture!(
        UpsertLabelSemanticsResponse,
        "upsert-label-semantics-response.v1.valid.json"
    );
    assert_eq!(actual.data.label_id, label.data.id);
    assert_eq!(actual.data.board_id, label.data.board_id);
    assert_eq!(actual.data.label_name, expected_upsert.data.label_name);
    assert_eq!(actual.data.applies_when, Vec::<String>::new());
    assert_eq!(actual.data.atoms.len(), 1);

    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/boards/{}/labels/atoms", atom_path.board),
        ))
        .await
        .expect("list atoms response");
    assert_eq!(response.status(), StatusCode::OK);
    let actual: ListLabelAtomsResponse = response_json(response).await;
    assert_eq!(actual.data.len(), 1);
    assert_eq!(actual.data[0].label_id, label.data.id);

    let response = router
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/boards/{}/labels/atom-index/status",
                atom_path.board
            ),
        ))
        .await
        .expect("atom index status response");
    assert_eq!(response.status(), StatusCode::OK);
    let status: LabelAtomIndexStatusResponse = response_json(response).await;
    assert!(!status.data.backend.is_empty());
}

#[tokio::test]
async fn locale_actor_json_header_fixture_is_consumed_by_real_router() {
    let (_directory, router) = test_router().await;
    create_board(&router, "fixture").await;
    let task = create_task(&router, "fixture", "t_fixture", "fixture label task").await;
    let label = create_label(
        &router,
        "fixture",
        CreateBoardLabelRequest {
            name: "fixture".to_owned(),
            color: None,
        },
    )
    .await;
    let request = AddTaskLabelRequest {
        name: Some(label.data.name.clone()),
        names: None,
        create_missing: false,
        actor: None,
    };
    let headers = actor_headers();
    assert_eq!(
        headers.get("X-KB-Actor").map(String::as_str),
        Some("schema-agent")
    );
    let response = router
        .oneshot(request_json_with_headers(
            Method::POST,
            &format!("/api/v1/tasks/{}/labels", task.id),
            &request,
            &headers,
        ))
        .await
        .expect("add label response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let added: kanban_protocol::AddTaskLabelResponse = response_json(response).await;
    assert_eq!(added.data.labels.len(), 1);
    assert_eq!(added.data.labels[0].id, label.data.id);
}

#[tokio::test]
async fn locale_header_fixture_is_consumed_by_real_router() {
    let (_directory, router) = test_router().await;
    let headers = header_fixture("locale-headers");
    let response = router
        .oneshot(request_empty_with_headers(
            Method::GET,
            "/api/v1/boards",
            &headers,
        ))
        .await
        .expect("list boards response");
    assert_eq!(response.status(), StatusCode::OK);
    let _: ListBoardsResponse = response_json(response).await;
}

#[tokio::test]
async fn locale_json_header_fixture_is_consumed_by_real_router() {
    let (_directory, router) = test_router().await;
    create_board(&router, "fixture").await;
    let headers = header_fixture("locale-json-headers");
    let request: CreateBoardLabelRequest = fixture!(
        CreateBoardLabelRequest,
        "create-board-label-request.v1.valid.json"
    );
    let response = router
        .oneshot(request_json_with_headers(
            Method::POST,
            "/api/v1/boards/fixture/labels",
            &request,
            &headers,
        ))
        .await
        .expect("create board label response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let _: CreateBoardLabelResponse = response_json(response).await;
}

#[tokio::test]
async fn locale_actor_header_fixture_is_consumed_by_real_router() {
    let (_directory, router) = test_router().await;
    create_board(&router, "fixture").await;
    let task = create_task(&router, "fixture", "t_fixture", "fixture label task").await;
    let label = create_label(
        &router,
        "fixture",
        CreateBoardLabelRequest {
            name: "fixture".to_owned(),
            color: None,
        },
    )
    .await;
    let add_request = AddTaskLabelRequest {
        name: Some(label.data.name.clone()),
        names: None,
        create_missing: false,
        actor: Some("fixture-agent".to_owned()),
    };
    let add_response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/tasks/{}/labels", task.id),
            &add_request,
        ))
        .await
        .expect("add label response");
    assert_eq!(add_response.status(), StatusCode::CREATED);
    let _: AddTaskLabelResponse = response_json(add_response).await;

    let headers = header_fixture("locale-actor-headers");
    let response = router
        .oneshot(request_empty_with_headers(
            Method::DELETE,
            &format!("/api/v1/tasks/{}/labels/{}", task.id, label.data.id),
            &headers,
        ))
        .await
        .expect("remove label response");
    assert_eq!(response.status(), StatusCode::OK);
    let _: RemoveTaskLabelResponse = response_json(response).await;
}

#[tokio::test]
async fn locale_actor_optional_json_header_fixture_is_consumed_by_real_router() {
    let (_directory, router) = test_router().await;
    create_board(&router, "fixture").await;
    let mut headers = header_fixture("locale-actor-optional-json-headers");
    // This profile intentionally makes Content-Type optional, while the archive
    // route still receives its actor-bearing JSON body through Axum's Json extractor.
    assert!(!headers.contains_key("Content-Type"));
    headers.insert("Content-Type".to_owned(), "application/json".to_owned());
    let request: ArchiveBoardRequest =
        fixture!(ArchiveBoardRequest, "archive-board-request.v1.valid.json");
    let response = router
        .oneshot(request_json_with_headers(
            Method::POST,
            "/api/v1/boards/fixture/archive",
            &request,
            &headers,
        ))
        .await
        .expect("archive board response");
    assert_eq!(response.status(), StatusCode::OK);
    let _: ArchiveBoardResponse = response_json(response).await;
}

#[tokio::test]
async fn label_proposal_routes_consume_typed_fixtures_and_persist_real_proposal() {
    let (_directory, router) = test_router().await;
    create_board(&router, "fixture").await;
    let _task = create_task(&router, "fixture", "t_fixture", "fixture proposal task").await;

    let task_path: kanban_protocol::TaskLabelSurfacePath = fixture!(
        kanban_protocol::TaskLabelSurfacePath,
        "list-task-label-proposals-path.v1.valid.json"
    );
    let empty_request: ProposeTaskLabelRequest = fixture!(
        ProposeTaskLabelRequest,
        "propose-task-label-request.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/tasks/{}/label-proposals", task_path.task_id),
            &empty_request,
        ))
        .await
        .expect("degraded proposal response");
    assert_eq!(response.status(), StatusCode::OK);
    let degraded: ProposeTaskLabelResponse = response_json(response).await;
    let mut expected_degraded: ProposeTaskLabelResponse = fixture!(
        ProposeTaskLabelResponse,
        "propose-task-label-response.v1.valid.json"
    );
    expected_degraded.data.board_id = degraded.data.board_id.clone();
    expected_degraded.data.diagnostics = degraded.data.diagnostics.clone();
    assert_eq!(degraded, expected_degraded);

    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/tasks/{}/label-proposals", task_path.task_id),
        ))
        .await
        .expect("empty proposal list response");
    assert_eq!(response.status(), StatusCode::OK);
    let listed: ListTaskLabelProposalsResponse = response_json(response).await;
    let expected_empty: ListTaskLabelProposalsResponse = fixture!(
        ListTaskLabelProposalsResponse,
        "list-task-label-proposals-response.v1.valid.json"
    );
    assert_eq!(listed, expected_empty);

    let mut request: ProposeTaskLabelRequest = fixture!(
        ProposeTaskLabelRequest,
        "propose-task-label-request.v1.valid.json"
    );
    request.actor = Some("fixture".to_owned());
    request.proposal = Some(LabelProposalCandidateWire {
        name: "fixture".to_owned(),
        description: None,
        applies_when: Vec::new(),
        excludes_when: Vec::new(),
        positive_examples: Vec::new(),
        negative_examples: Vec::new(),
    });
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/tasks/{}/label-proposals", task_path.task_id),
            &request,
        ))
        .await
        .expect("persisted proposal response");
    assert_eq!(response.status(), StatusCode::OK);
    let created: ProposeTaskLabelResponse = response_json(response).await;
    let proposal = created.data.proposal.clone().expect("created proposal");
    assert_eq!(proposal.name, "fixture");
    assert_eq!(proposal.task_id, task_path.task_id);

    let proposal_list: ListTaskLabelProposalsResponse = response_json(
        router
            .clone()
            .oneshot(request_empty(
                Method::GET,
                &format!("/api/v1/tasks/{}/label-proposals", task_path.task_id),
            ))
            .await
            .expect("proposal list response"),
    )
    .await;
    assert_eq!(proposal_list.data.len(), 1);
    assert_eq!(proposal_list.data[0].id, proposal.id);

    let proposal_path: ProposalPath =
        fixture!(ProposalPath, "get-label-proposal-path.v1.valid.json");
    let mut proposal_path = proposal_path;
    proposal_path.proposal_id = proposal.id.clone();
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/label-proposals/{}", proposal_path.proposal_id),
        ))
        .await
        .expect("proposal show response");
    assert_eq!(response.status(), StatusCode::OK);
    let shown: GetLabelProposalResponse = response_json(response).await;
    let expected_shown: GetLabelProposalResponse = fixture!(
        GetLabelProposalResponse,
        "get-label-proposal-response.v1.valid.json"
    );
    assert_eq!(shown.data.id, proposal.id);
    assert_eq!(shown.data.name, expected_shown.data.name);

    let mut reject_request = request.clone();
    reject_request
        .proposal
        .as_mut()
        .expect("reject proposal")
        .name = "reject fixture".to_owned();
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/tasks/{}/label-proposals", task_path.task_id),
            &reject_request,
        ))
        .await
        .expect("reject proposal create response");
    assert_eq!(response.status(), StatusCode::OK);
    let reject_created: ProposeTaskLabelResponse = response_json(response).await;
    let reject_proposal = reject_created.data.proposal.expect("reject proposal");
    let mut reject_path: ProposalPath =
        fixture!(ProposalPath, "reject-label-proposal-path.v1.valid.json");
    reject_path.proposal_id = reject_proposal.id.clone();
    let reject_body: LabelProposalDecisionRequest = fixture!(
        LabelProposalDecisionRequest,
        "reject-label-proposal-body.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/label-proposals/{}/reject", reject_path.proposal_id),
            &reject_body,
        ))
        .await
        .expect("proposal reject response");
    assert_eq!(response.status(), StatusCode::OK);
    let rejected: kanban_protocol::LabelProposalDecisionResponse = response_json(response).await;
    let expected_rejected: kanban_protocol::LabelProposalDecisionResponse = fixture!(
        kanban_protocol::LabelProposalDecisionResponse,
        "reject-label-proposal-response.v1.valid.json"
    );
    assert_eq!(rejected.data.id, reject_proposal.id);
    assert_eq!(rejected.data.status, expected_rejected.data.status);

    let decision: LabelProposalDecisionRequest = fixture!(
        LabelProposalDecisionRequest,
        "accept-label-proposal-body.v1.valid.json"
    );
    let mut accept_path: ProposalPath =
        fixture!(ProposalPath, "accept-label-proposal-path.v1.valid.json");
    accept_path.proposal_id = proposal.id.clone();
    let response = router
        .oneshot(request_json(
            Method::POST,
            &format!("/api/v1/label-proposals/{}/accept", accept_path.proposal_id),
            &decision,
        ))
        .await
        .expect("proposal accept response");
    assert_eq!(response.status(), StatusCode::OK);
    let accepted: kanban_protocol::LabelProposalDecisionResponse = response_json(response).await;
    let expected_accepted: kanban_protocol::LabelProposalDecisionResponse = fixture!(
        kanban_protocol::LabelProposalDecisionResponse,
        "accept-label-proposal-response.v1.valid.json"
    );
    assert_eq!(accepted.data.id, proposal.id);
    assert_eq!(accepted.data.status, expected_accepted.data.status);
    assert_eq!(
        accepted.data.status,
        kanban_protocol::LabelProposalStatusWire::Accepted
    );
}

#[tokio::test]
async fn ontology_ledger_routes_consume_observation_and_action_fixtures() {
    let (_directory, router) = test_router().await;
    create_board(&router, "fixture").await;
    let _task = create_task(&router, "fixture", "t_fixture", "fixture ontology task").await;
    let _label = create_label(
        &router,
        "fixture",
        CreateBoardLabelRequest {
            name: "fixture".to_owned(),
            color: None,
        },
    )
    .await;

    let review_path: BoardLabelPath =
        fixture!(BoardLabelPath, "review-label-ontology-path.v1.valid.json");
    let review_query: LabelOntologyReviewQuery = fixture!(
        LabelOntologyReviewQuery,
        "label-ontology-review-query.v1.valid.json"
    );
    let mut review_query = review_query;
    review_query.limit = 1;
    let group_by = match review_query.group_by {
        kanban_protocol::LabelOntologyReviewGroupByWire::Label => "label",
        kanban_protocol::LabelOntologyReviewGroupByWire::CandidateAtom => "candidate_atom",
        kanban_protocol::LabelOntologyReviewGroupByWire::ProposedLabel => "proposed_label",
        kanban_protocol::LabelOntologyReviewGroupByWire::Cluster => "cluster",
    };
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/boards/{}/label-ontology/review?group_by={group_by}&include_all={}&limit={}",
                review_path.board, review_query.include_all, review_query.limit
            ),
        ))
        .await
        .expect("ontology review response");
    assert_eq!(response.status(), StatusCode::OK);
    let review: kanban_protocol::ReviewLabelOntologyResponse = response_json(response).await;
    assert!(review.data.is_empty());
    assert_eq!(review.meta.limit, 1);
    let expected_review: kanban_protocol::ReviewLabelOntologyResponse = fixture!(
        kanban_protocol::ReviewLabelOntologyResponse,
        "review-label-ontology-response.v1.valid.json"
    );
    assert_eq!(review.meta.group_by, expected_review.meta.group_by);

    let observation: RecordLabelOntologyObservationRequest = fixture!(
        RecordLabelOntologyObservationRequest,
        "record-label-ontology-observation-body.v1.valid.json"
    );
    let observation_path: kanban_protocol::TaskLabelSurfacePath = fixture!(
        kanban_protocol::TaskLabelSurfacePath,
        "record-label-ontology-observation-path.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!(
                "/api/v1/tasks/{}/label-ontology/observations",
                observation_path.task_id
            ),
            &observation,
        ))
        .await
        .expect("ontology observation response");
    assert_eq!(response.status(), StatusCode::OK);
    let recorded: RecordLabelOntologyObservationResponse = response_json(response).await;
    assert_eq!(recorded.data.task_id, observation_path.task_id);
    assert_eq!(recorded.data.signals.len(), 1);
    let signal_id = recorded.data.signals[0].id.clone();

    let signal_path: BoardLabelPath = fixture!(
        BoardLabelPath,
        "list-label-ontology-signals-path.v1.valid.json"
    );
    let signal_query: LabelOntologySignalQuery = fixture!(
        LabelOntologySignalQuery,
        "label-ontology-signal-query.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/boards/{}/label-ontology/signals?limit={}",
                signal_path.board, signal_query.limit
            ),
        ))
        .await
        .expect("ontology signal list response");
    assert_eq!(response.status(), StatusCode::OK);
    let signals: MetadataEnvelope<Vec<LabelOntologySignalWire>, SignalFilterMeta> =
        response_json(response).await;
    let expected_signals: LabelOntologySignalsResponse = fixture!(
        LabelOntologySignalsResponse,
        "list-label-ontology-signals-response.v1.valid.json"
    );
    assert_eq!(signals.data.len(), 1);
    assert_eq!(signals.meta.limit, signal_query.limit);
    assert_eq!(signals.data[0].id, signal_id);
    assert_eq!(expected_signals.data.len(), signals.data.len());

    let mut get_path: SignalPath =
        fixture!(SignalPath, "get-label-ontology-signal-path.v1.valid.json");
    get_path.signal_id = signal_id.clone();
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/label-ontology/signals/{}", get_path.signal_id),
        ))
        .await
        .expect("ontology signal show response");
    assert_eq!(response.status(), StatusCode::OK);
    let detail: GetLabelOntologySignalResponse = response_json(response).await;
    assert_eq!(detail.data.signal.id, signal_id);

    let mut action: LabelOntologyActionRequest = fixture!(
        LabelOntologyActionRequest,
        "create-label-ontology-action-request.v1.valid.json"
    );
    action.signal_ids = vec![signal_id.clone()];
    let action_path: BoardLabelPath = fixture!(
        BoardLabelPath,
        "create-label-ontology-action-path.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            &format!(
                "/api/v1/boards/{}/label-ontology/actions",
                action_path.board
            ),
            &action,
        ))
        .await
        .expect("ontology action response");
    assert_eq!(response.status(), StatusCode::OK);
    let action_response: LabelOntologyActionResponse = response_json(response).await;
    let expected_action: LabelOntologyActionResponse = fixture!(
        LabelOntologyActionResponse,
        "create-label-ontology-action-response.v1.valid.json"
    );
    assert_eq!(action_response.data.signal_ids, vec![signal_id.clone()]);
    assert_eq!(action_response.data.reason, expected_action.data.reason);

    let mut apply: kanban_protocol::ApplyLabelOntologyAtomRequest = fixture!(
        kanban_protocol::ApplyLabelOntologyAtomRequest,
        "apply-label-ontology-atom-request.v1.valid.json"
    );
    apply.signal_ids = vec![signal_id];
    let apply_path: BoardLabelPath = fixture!(
        BoardLabelPath,
        "apply-label-ontology-atom-path.v1.valid.json"
    );
    let response = router
        .oneshot(request_json(
            Method::POST,
            &format!(
                "/api/v1/boards/{}/label-ontology/apply/atom",
                apply_path.board
            ),
            &apply,
        ))
        .await
        .expect("apply ontology atom response");
    assert_eq!(response.status(), StatusCode::OK);
    let applied: LabelOntologyActionResponse = response_json(response).await;
    let expected_applied: LabelOntologyActionResponse = fixture!(
        LabelOntologyActionResponse,
        "apply-label-ontology-atom-response.v1.valid.json"
    );
    assert_eq!(applied.data.reason, "fixture");
    assert_eq!(expected_applied.data.reason, applied.data.reason);
    assert!(applied.data.result_atom_id.is_some());
}

#[tokio::test]
async fn signal_routes_consume_record_list_show_and_review_fixtures() {
    let (_directory, router) = test_router().await;
    create_board(&router, "fixture").await;
    let _task = create_task(&router, "fixture", "t_fixture", "fixture signal task").await;

    let list_path: BoardLabelPath = fixture!(BoardLabelPath, "list-signals-path.v1.valid.json");
    let mut list_query: kanban_protocol::SignalQuery = fixture!(
        kanban_protocol::SignalQuery,
        "list-signals-query.v1.valid.json"
    );
    list_query.limit = 1;
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/boards/{}/signals?limit={}",
                list_path.board, list_query.limit
            ),
        ))
        .await
        .expect("empty signal list response");
    assert_eq!(response.status(), StatusCode::OK);
    let listed: ListSignalsResponse = response_json(response).await;
    let expected_list: ListSignalsResponse =
        fixture!(ListSignalsResponse, "list-signals-response.v1.valid.json");
    assert_eq!(listed, expected_list);

    let record_request: RecordSignalRequest =
        fixture!(RecordSignalRequest, "record-signal-request.v1.valid.json");
    let record_path: BoardLabelPath = fixture!(BoardLabelPath, "record-signal-path.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_json_with_headers(
            Method::POST,
            &format!("/api/v1/boards/{}/signals", record_path.board),
            &record_request,
            &actor_headers(),
        ))
        .await
        .expect("record signal response");
    assert_eq!(response.status(), StatusCode::CREATED);
    let recorded: RecordSignalResponse = response_json(response).await;
    let expected_record: RecordSignalResponse =
        fixture!(RecordSignalResponse, "record-signal-response.v1.valid.json");
    assert_eq!(recorded.data.signal.kind, expected_record.data.signal.kind);
    assert_eq!(
        recorded.data.signal.title,
        expected_record.data.signal.title
    );
    assert_eq!(
        recorded.data.signal.summary,
        expected_record.data.signal.summary
    );
    assert_eq!(
        recorded.data.signal.severity,
        expected_record.data.signal.severity
    );
    let signal_id = recorded.data.signal.id.clone();

    let mut show_path: SignalPath = fixture!(SignalPath, "get-signal-path.v1.valid.json");
    show_path.signal_id = signal_id.clone();
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/signals/{}", show_path.signal_id),
        ))
        .await
        .expect("signal show response");
    assert_eq!(response.status(), StatusCode::OK);
    let shown: GetSignalResponse = response_json(response).await;
    let expected_show: GetSignalResponse =
        fixture!(GetSignalResponse, "get-signal-response.v1.valid.json");
    assert_eq!(shown.data.id, signal_id);
    assert_eq!(shown.data.kind, record_request.kind);
    assert_eq!(expected_show.data.status, "open");

    let review_path: BoardLabelPath =
        fixture!(BoardLabelPath, "confirm-signals-path.v1.valid.json");
    let mut review_request: ReviewSignalsRequest =
        fixture!(ReviewSignalsRequest, "review-signals-request.v1.valid.json");
    review_request.signal_ids = vec![signal_id.clone()];
    let response = router
        .oneshot(request_json_with_headers(
            Method::POST,
            &format!("/api/v1/boards/{}/signals/confirm", review_path.board),
            &review_request,
            &actor_headers(),
        ))
        .await
        .expect("confirm signals response");
    assert_eq!(response.status(), StatusCode::OK);
    let confirmed: ConfirmSignalsResponse = response_json(response).await;
    let expected_confirmed: ConfirmSignalsResponse = fixture!(
        ConfirmSignalsResponse,
        "confirm-signals-response.v1.valid.json"
    );
    assert_eq!(confirmed.data.len(), 1);
    assert!(expected_confirmed.data.is_empty());
    assert_eq!(confirmed.data[0].id, signal_id);
    assert_eq!(confirmed.data[0].status, "confirmed");
}

fn encode_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    encoded
}

fn search_query_uri(base: &str, query: &SearchTasksQuery) -> String {
    let mut params = vec![format!("board={}", encode_component(&query.board))];
    if let Some(value) = query.q.as_deref() {
        params.push(format!("q={}", encode_component(value)));
    }
    for status in &query.status {
        let value = match status {
            kanban_protocol::ApiTaskStatus::Triage => "triage",
            kanban_protocol::ApiTaskStatus::Todo => "todo",
            kanban_protocol::ApiTaskStatus::Scheduled => "scheduled",
            kanban_protocol::ApiTaskStatus::Ready => "ready",
            kanban_protocol::ApiTaskStatus::Running => "running",
            kanban_protocol::ApiTaskStatus::Blocked => "blocked",
            kanban_protocol::ApiTaskStatus::Review => "review",
            kanban_protocol::ApiTaskStatus::Done => "done",
            kanban_protocol::ApiTaskStatus::Archived => "archived",
        };
        params.push(format!("status={value}"));
    }
    for label in &query.label {
        params.push(format!("label={}", encode_component(label)));
    }
    params.push(format!("include_archived={}", query.include_archived));
    params.push(format!("limit={}", query.limit));
    params.push(format!("offset={}", query.offset));
    if let Some(value) = query.assignee.as_deref() {
        params.push(format!("assignee={}", encode_component(value)));
    }
    format!("{base}?{}", params.join("&"))
}

#[tokio::test]
async fn entity_routes_consume_upsert_list_and_path_fixtures() {
    let (_directory, router) = test_router().await;
    let _task = create_task(&router, "default", "t_fixture", "fixture entity task").await;
    let request: EntityUpsertRequest =
        fixture!(EntityUpsertRequest, "entity-upsert-request.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_json(Method::PUT, "/api/v1/entities", &request))
        .await
        .expect("entity upsert response");
    assert_eq!(response.status(), StatusCode::OK);
    let actual: EntityResponse = response_json(response).await;
    let expected: EntityResponse = fixture!(EntityResponse, "entity-upsert-response.v1.valid.json");
    assert_eq!(actual.data.uri, expected.data.uri);
    assert_eq!(actual.data.kind, expected.data.kind);
    assert_eq!(actual.data.source_table, expected.data.source_table);
    assert_eq!(actual.data.source_id, expected.data.source_id);
    assert_eq!(actual.data.task_id, expected.data.task_id);

    let list_query: EntityListQuery = fixture!(EntityListQuery, "entity-list-query.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/entities?board={}&kind={}&limit={}",
                list_query.board.as_deref().unwrap_or("default"),
                list_query.kind.as_deref().unwrap_or("task"),
                list_query.limit
            ),
        ))
        .await
        .expect("entity list response");
    assert_eq!(response.status(), StatusCode::OK);
    let listed: EntityListResponse = response_json(response).await;
    let expected_list: EntityListResponse =
        fixture!(EntityListResponse, "entity-list-response.v1.valid.json");
    assert_eq!(listed.data.len(), expected_list.data.len());
    assert_eq!(listed.data[0].uri, request.uri);

    let path: EntityPath = fixture!(EntityPath, "entity-path.v1.valid.json");
    let response = router
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/entities/{}", encode_component(&path.uri)),
        ))
        .await
        .expect("entity show response");
    assert_eq!(response.status(), StatusCode::OK);
    let shown: EntityResponse = response_json(response).await;
    assert_eq!(shown.data.uri, path.uri);
}

#[tokio::test]
async fn search_routes_consume_query_and_status_fixtures_against_real_index() {
    let (_directory, router) = test_router().await;
    let alpha = create_label(
        &router,
        "default",
        CreateBoardLabelRequest {
            name: "alpha".to_owned(),
            color: None,
        },
    )
    .await;
    let beta = create_label(
        &router,
        "default",
        CreateBoardLabelRequest {
            name: "beta".to_owned(),
            color: None,
        },
    )
    .await;
    create_assigned_task(&router, "default", "t_fixture", "needle contract sentinel").await;
    create_assigned_task(&router, "default", "t_skip", "needle earlier").await;
    for task_id in ["t_skip", "t_fixture"] {
        let labels = AddTaskLabelRequest {
            name: None,
            names: Some(vec![alpha.data.name.clone(), beta.data.name.clone()]),
            create_missing: false,
            actor: Some("fixture".to_owned()),
        };
        let response = router
            .clone()
            .oneshot(request_json(
                Method::POST,
                &format!("/api/v1/tasks/{task_id}/labels"),
                &labels,
            ))
            .await
            .expect("search label response");
        assert_eq!(response.status(), StatusCode::CREATED);
        let _: kanban_protocol::AddTaskLabelResponse = response_json(response).await;
    }

    let query: SearchTasksQuery = fixture!(SearchTasksQuery, "search-tasks-query.v1.valid.json");
    let search_uri = search_query_uri("/api/v1/search/tasks", &query);
    let response = router
        .clone()
        .oneshot(request_empty(Method::GET, &search_uri))
        .await
        .expect("search tasks response");
    assert_eq!(response.status(), StatusCode::OK);
    let actual: SearchTasksResponse = response_json(response).await;
    let expected: SearchTasksResponse =
        fixture!(SearchTasksResponse, "search-tasks-response.v1.valid.json");
    assert_eq!(actual.meta.limit, query.limit);
    assert_eq!(actual.meta.offset, query.offset);
    assert_eq!(actual.data.hits.len(), 1);
    assert_eq!(actual.data.hits[0].task_id, "t_fixture");
    assert_eq!(
        actual.data.hits[0].task.title,
        expected.data.hits[0].task.title
    );
    assert_eq!(actual.data.hits[0].task.assignee, Some("agent".to_owned()));

    let by_status_uri = search_query_uri("/api/v1/search/tasks/by-status", &query);
    let response = router
        .clone()
        .oneshot(request_empty(Method::GET, &by_status_uri))
        .await
        .expect("search by status response");
    assert_eq!(response.status(), StatusCode::OK);
    let by_status: SearchTasksByStatusResponse = response_json(response).await;
    assert_eq!(by_status.data.statuses.len(), 2);
    assert_eq!(by_status.data.statuses[1].tasks.len(), 1);
    assert_eq!(by_status.data.statuses[1].tasks[0].id, "t_fixture");
    let expected_by_status: SearchTasksByStatusResponse = fixture!(
        SearchTasksByStatusResponse,
        "search-tasks-by-status-response.v1.valid.json"
    );
    assert_eq!(
        expected_by_status.data.statuses.len(),
        by_status.data.statuses.len()
    );

    let status_query: kanban_protocol::BoardQuery = fixture!(
        kanban_protocol::BoardQuery,
        "search-status-query.v1.valid.json"
    );
    let response = router
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/search/status?board={}", status_query.board),
        ))
        .await
        .expect("search status response");
    assert_eq!(response.status(), StatusCode::OK);
    let status: SearchStatusResponse = response_json(response).await;
    let expected_status: SearchStatusResponse =
        fixture!(SearchStatusResponse, "search-status-response.v1.valid.json");
    assert_eq!(expected_status.data.backend, "sqlite");
    assert_eq!(status.data.resolved_board_id, "b_default");
    assert!(!status.data.backend.is_empty());
}

#[tokio::test]
async fn vector_routes_consume_typed_projection_fixtures_and_real_degraded_queries() {
    let (_directory, router) = test_router().await;
    let status_query: VectorStatusQuery =
        fixture!(VectorStatusQuery, "vector-status-query.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/vector/status?board={}", status_query.board),
        ))
        .await
        .expect("vector status response");
    assert_eq!(response.status(), StatusCode::OK);
    let status: VectorStatusResponse = response_json(response).await;
    let expected_status: VectorStatusResponse =
        fixture!(VectorStatusResponse, "vector-status-response.v1.valid.json");
    assert!(!status.data.enabled);
    assert!(!status.data.backend.is_empty());
    assert!(!expected_status.data.enabled);

    let configure: VectorConfigureRequest = fixture!(
        VectorConfigureRequest,
        "vector-configure-request.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            "/api/v1/vector/configure",
            &configure,
        ))
        .await
        .expect("vector configure response");
    assert_eq!(response.status(), StatusCode::OK);
    let configured: VectorConfigureResponse = response_json(response).await;
    let expected_configured: VectorConfigureResponse = fixture!(
        VectorConfigureResponse,
        "vector-configure-response.v1.valid.json"
    );
    assert_eq!(configured, expected_configured);

    let rebuild_request: VectorProjectionRequest = fixture!(
        VectorProjectionRequest,
        "vector-rebuild-request.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            "/api/v1/vector/rebuild",
            &rebuild_request,
        ))
        .await
        .expect("vector rebuild response");
    assert_eq!(response.status(), StatusCode::OK);
    let rebuilt: VectorProjectionResponse = response_json(response).await;
    let expected_rebuilt: VectorProjectionResponse = fixture!(
        VectorProjectionResponse,
        "vector-rebuild-response.v1.valid.json"
    );
    assert!(!rebuilt.data.backend.is_empty());
    assert!(!expected_rebuilt.data.enabled);
    assert!(!rebuilt.data.message.is_empty());

    let sync_request: VectorProjectionRequest =
        fixture!(VectorProjectionRequest, "vector-sync-request.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_json(
            Method::POST,
            "/api/v1/vector/sync",
            &sync_request,
        ))
        .await
        .expect("vector sync response");
    assert_eq!(response.status(), StatusCode::OK);
    let synced: VectorProjectionResponse = response_json(response).await;
    let expected_synced: VectorProjectionResponse = fixture!(
        VectorProjectionResponse,
        "vector-sync-response.v1.valid.json"
    );
    assert!(!synced.data.backend.is_empty());
    assert!(!expected_synced.data.enabled);
    assert!(!synced.data.message.is_empty());

    let chunks_query: VectorQuery =
        fixture!(VectorQuery, "vector-query-chunks-query.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/vector/query-chunks?board={}&q={}&limit={}",
                encode_component(&chunks_query.board),
                encode_component(&chunks_query.q),
                chunks_query.limit
            ),
        ))
        .await
        .expect("vector chunk query response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: ErrorEnvelope = response_json(response).await;
    assert_eq!(
        error.error.code,
        kanban_protocol::ApiErrorCode::InvalidInput
    );
    assert!(error.error.message.contains("degraded"));

    let atoms_query: VectorQuery =
        fixture!(VectorQuery, "vector-query-label-atoms-query.v1.valid.json");
    let response = router
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/vector/query-label-atoms?board={}&q={}&limit={}&polarity={}&include_vector={}",
                encode_component(&atoms_query.board),
                encode_component(&atoms_query.q),
                atoms_query.limit,
                encode_component(atoms_query.polarity.as_deref().unwrap_or("positive")),
                atoms_query.include_vector
            ),
        ))
        .await
        .expect("vector atom query response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: ErrorEnvelope = response_json(response).await;
    assert_eq!(
        error.error.code,
        kanban_protocol::ApiErrorCode::InvalidInput
    );
}

#[tokio::test]
async fn graph_routes_consume_query_and_projection_fixtures() {
    let (_directory, state, router) = test_router_with_state().await;
    let _parent = create_task(&router, "default", "t_dependency", "fixture graph parent").await;
    create_dependent_task(&router, "default", "t_fixture", "t_dependency").await;

    let expected_neighbors: GraphNeighborsResponse = fixture!(
        GraphNeighborsResponse,
        "graph-neighbors-response.v1.valid.json"
    );
    let expected_relation = expected_neighbors
        .data
        .first()
        .expect("graph relation fixture");
    state
        .application()
        .upsert_relation(RelationUpsertCommand {
            subject_uri: expected_relation.subject_uri.clone(),
            predicate: expected_relation.predicate.clone(),
            object_uri: expected_relation.object_uri.clone(),
            graph_uri: expected_relation.graph_uri.clone(),
            board: Some("default".to_owned()),
            authoritative_store: expected_relation.provenance.authoritative_store.clone(),
            source_table: expected_relation.provenance.source_table.clone(),
            source_id: expected_relation.provenance.source_id.clone(),
            source_event_id: expected_relation.provenance.source_event_id,
            metadata_json: serde_json::to_string(&expected_relation.metadata)
                .expect("graph relation metadata JSON"),
        })
        .await
        .expect("seed graph relation through application path");

    let status_query: BoardQuery = fixture!(BoardQuery, "graph-status-query.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!("/api/v1/graph/status?board={}", status_query.board),
        ))
        .await
        .expect("graph status response");
    assert_eq!(response.status(), StatusCode::OK);
    let status: GraphStatusResponse = response_json(response).await;
    let expected_status: GraphStatusResponse =
        fixture!(GraphStatusResponse, "graph-status-response.v1.valid.json");
    assert!(!status.data.backend.is_empty());
    assert!(!expected_status.data.enabled);

    let neighbors_query: GraphNeighborsQuery =
        fixture!(GraphNeighborsQuery, "graph-neighbors-query.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/graph/neighbors?board={}&entity_uri={}&predicate={}&limit={}",
                encode_component(&neighbors_query.board),
                encode_component(&neighbors_query.entity_uri),
                encode_component(neighbors_query.predicate.as_deref().unwrap_or("depends_on")),
                neighbors_query.limit
            ),
        ))
        .await
        .expect("graph neighbors response");
    assert_eq!(response.status(), StatusCode::OK);
    let neighbors: GraphNeighborsResponse = response_json(response).await;
    assert_eq!(neighbors.meta.limit, neighbors_query.limit);
    assert_eq!(neighbors.data.len(), expected_neighbors.data.len());
    let mut normalized_neighbors = neighbors.clone();
    normalized_neighbors.data[0].created_at = expected_neighbors.data[0].created_at;
    normalized_neighbors.data[0].updated_at = expected_neighbors.data[0].updated_at;
    assert_eq!(normalized_neighbors, expected_neighbors);

    let query: GraphQueryQuery = fixture!(GraphQueryQuery, "graph-query-query.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/graph/query?board={}&query={}&limit={}",
                encode_component(&query.board),
                encode_component(&query.query),
                query.limit
            ),
        ))
        .await
        .expect("graph query response");
    assert_eq!(response.status(), StatusCode::OK);
    let rows: kanban_protocol::cli_helpers::CliGraphQueryOutput = response_json(response).await;
    let expected_rows: kanban_protocol::cli_helpers::CliGraphQueryOutput = fixture!(
        kanban_protocol::cli_helpers::CliGraphQueryOutput,
        "graph-query-response.v1.valid.json"
    );
    assert_eq!(rows, expected_rows);

    let rebuild_query: BoardQuery = fixture!(BoardQuery, "graph-rebuild-query.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::POST,
            &format!("/api/v1/graph/rebuild?board={}", rebuild_query.board),
        ))
        .await
        .expect("graph rebuild response");
    assert_eq!(response.status(), StatusCode::OK);
    let rebuilt: GraphMaintenanceResponse = response_json(response).await;
    let expected_rebuilt: GraphMaintenanceResponse = fixture!(
        GraphMaintenanceResponse,
        "graph-rebuild-response.v1.valid.json"
    );
    assert_eq!(rebuilt.data.mode, expected_rebuilt.data.mode);
    assert!(!rebuilt.data.generation.is_empty());

    let sync_query: BoardQuery = fixture!(BoardQuery, "graph-sync-query.v1.valid.json");
    let response = router
        .oneshot(request_empty(
            Method::POST,
            &format!("/api/v1/graph/sync?board={}", sync_query.board),
        ))
        .await
        .expect("graph sync response");
    assert_eq!(response.status(), StatusCode::OK);
    let synced: GraphMaintenanceResponse = response_json(response).await;
    let expected_synced: GraphMaintenanceResponse = fixture!(
        GraphMaintenanceResponse,
        "graph-sync-response.v1.valid.json"
    );
    assert_eq!(synced.data.mode, expected_synced.data.mode);
    assert!(!synced.data.fingerprint.is_empty());
}

#[tokio::test]
async fn context_neighborhood_and_task_map_routes_consume_typed_fixtures() {
    let (_directory, router) = test_router().await;
    let _context_task = create_task(&router, "default", "t_fixture", "context fixture").await;
    let _center_task = create_task(&router, "default", "t_center", "graph fixture").await;

    let context_path: BuildContextPath =
        fixture!(BuildContextPath, "build-context-path.v1.valid.json");
    let context_query: BuildContextQuery =
        fixture!(BuildContextQuery, "build-context-query.v1.valid.json");
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/tasks/{}/context?board={}&lexical_limit={}&graph_limit={}&vector_limit={}&max_items={}&task={}&depth={}&budget={}",
                context_path.task_id,
                context_query.board,
                context_query.lexical_limit,
                context_query.graph_limit,
                context_query.vector_limit,
                context_query.max_items,
                context_query.task.as_deref().unwrap_or("t_fixture"),
                context_query.depth,
                context_query.budget.unwrap_or(20)
            ),
        ))
        .await
        .expect("context response");
    assert_eq!(response.status(), StatusCode::OK);
    let context: BuildContextResponse = response_json(response).await;
    let expected_context: BuildContextResponse =
        fixture!(BuildContextResponse, "build-context-response.v1.valid.json");
    assert_eq!(context.data.subject, "kb://task/t_fixture");
    assert_eq!(context.data.policy.max_items, context_query.max_items);
    assert_eq!(
        context.data.policy.depth,
        expected_context.data.policy.depth
    );
    assert!(!context.data.degraded.is_empty());

    let neighborhood_path: TaskNeighborhoodPath =
        fixture!(TaskNeighborhoodPath, "task-neighborhood-path.v1.valid.json");
    let neighborhood_query: TaskNeighborhoodQuery = fixture!(
        TaskNeighborhoodQuery,
        "task-neighborhood-query.v1.valid.json"
    );
    let response = router
        .clone()
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/tasks/{}/neighborhood?depth={}&limit_nodes={}&include_archived_context={}",
                neighborhood_path.task_id,
                neighborhood_query.depth,
                neighborhood_query.limit_nodes,
                neighborhood_query.include_archived_context
            ),
        ))
        .await
        .expect("neighborhood response");
    assert_eq!(response.status(), StatusCode::OK);
    let neighborhood: TaskNeighborhoodResponse = response_json(response).await;
    let expected_neighborhood: TaskNeighborhoodResponse = fixture!(
        TaskNeighborhoodResponse,
        "task-neighborhood-response.v1.valid.json"
    );
    assert_eq!(neighborhood.data.center_task_id, neighborhood_path.task_id);
    assert_eq!(neighborhood.data.meta.depth, neighborhood_query.depth);
    assert_eq!(neighborhood.data.nodes.len(), 1);
    assert_eq!(
        expected_neighborhood.data.meta.limit_nodes,
        neighborhood.data.meta.limit_nodes
    );

    let map_path: BoardTaskMapPath =
        fixture!(BoardTaskMapPath, "board-task-map-path.v1.valid.json");
    let map_query: BoardTaskMapQuery =
        fixture!(BoardTaskMapQuery, "board-task-map-query.v1.valid.json");
    let response = router
        .oneshot(request_empty(
            Method::GET,
            &format!(
                "/api/v1/boards/{}/task-map?active_only={}&context_depth={}&limit_nodes={}&include_done_context={}&include_archived_context={}&hide_isolated={}",
                map_path.board,
                map_query.active_only,
                map_query.context_depth,
                map_query.limit_nodes,
                map_query.include_done_context,
                map_query.include_archived_context,
                map_query.hide_isolated
            ),
        ))
        .await
        .expect("task map response");
    assert_eq!(response.status(), StatusCode::OK);
    let map: BoardTaskMapResponse = response_json(response).await;
    let expected_map: BoardTaskMapResponse = fixture!(
        BoardTaskMapResponse,
        "board-task-map-response.v1.valid.json"
    );
    assert!(!map.data.nodes.is_empty());
    assert_eq!(map.data.meta.active_only, map_query.active_only);
    assert_eq!(
        expected_map.data.meta.limit_nodes,
        map.data.meta.limit_nodes
    );
}
