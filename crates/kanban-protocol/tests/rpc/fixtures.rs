use kanban_protocol::rpc::v1;
use prost::Message;
#[test]
fn get_health_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetHealthRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetHealthRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_health_response_fixture_roundtrip() {
    let dto: kanban_protocol::HealthResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/health-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetHealthResponse::try_from(dto).unwrap();
    let wire = v1::GetHealthResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::HealthResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_boards_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::ListBoardsQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-boards-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListBoardsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListBoardsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_boards_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListBoardsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-boards-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListBoardsResponse::try_from(dto).unwrap();
    let wire = v1::ListBoardsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListBoardsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_board_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::CreateBoardRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-board-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CreateBoardRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CreateBoardRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_board_response_fixture_roundtrip() {
    let dto: kanban_protocol::CreateBoardResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-board-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CreateBoardResponse::try_from(dto).unwrap();
    let wire = v1::CreateBoardResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CreateBoardResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_board_request_fixture_roundtrip() {
    let path: kanban_protocol::GetBoardPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-board-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetBoardRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetBoardRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_board_response_fixture_roundtrip() {
    let dto: kanban_protocol::GetBoardResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-board-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetBoardResponse::try_from(dto).unwrap();
    let wire = v1::GetBoardResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GetBoardResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn archive_board_request_fixture_roundtrip() {
    let path: kanban_protocol::ArchiveBoardPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/archive-board-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ArchiveBoardRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/archive-board-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ArchiveBoardRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ArchiveBoardRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn archive_board_response_fixture_roundtrip() {
    let dto: kanban_protocol::ArchiveBoardResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/archive-board-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ArchiveBoardResponse::try_from(dto).unwrap();
    let wire = v1::ArchiveBoardResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ArchiveBoardResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_board_columns_request_fixture_roundtrip() {
    let path: kanban_protocol::ListBoardColumnsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-board-columns-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListBoardColumnsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListBoardColumnsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_board_columns_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListBoardColumnsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-board-columns-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListBoardColumnsResponse::try_from(dto).unwrap();
    let wire = v1::ListBoardColumnsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListBoardColumnsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_tasks_request_fixture_roundtrip() {
    let path: kanban_protocol::ListTasksPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-tasks-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::ListTasksQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-tasks-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListTasksRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListTasksRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_tasks_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListTasksResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-tasks-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListTasksResponse::try_from(dto).unwrap();
    let wire = v1::ListTasksResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListTasksResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_tasks_by_status_request_fixture_roundtrip() {
    let path: kanban_protocol::ListTasksByStatusPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-tasks-by-status-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::ListTasksByStatusQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-tasks-by-status-query.v1.valid.json"
        )))
        .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListTasksByStatusRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListTasksByStatusRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_tasks_by_status_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListTasksByStatusResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-tasks-by-status-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListTasksByStatusResponse::try_from(dto).unwrap();
    let wire = v1::ListTasksByStatusResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListTasksByStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_task_request_fixture_roundtrip() {
    let path: kanban_protocol::CreateTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::CreateTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CreateTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CreateTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::CreateTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CreateTaskResponse::try_from(dto).unwrap();
    let wire = v1::CreateTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CreateTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_task_request_fixture_roundtrip() {
    let path: kanban_protocol::GetTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-task-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::GetTaskQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-task-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::GetTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetTaskResponse::try_from(dto).unwrap();
    let wire = v1::GetTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GetTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn update_task_request_fixture_roundtrip() {
    let path: kanban_protocol::UpdateTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/update-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::UpdateTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/update-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::UpdateTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::UpdateTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn update_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::UpdateTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/update-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::UpdateTaskResponse::try_from(dto).unwrap();
    let wire = v1::UpdateTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::UpdateTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn specify_task_request_fixture_roundtrip() {
    let path: kanban_protocol::SpecifyTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/specify-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::SpecifyTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/specify-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SpecifyTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SpecifyTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn specify_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::SpecifyTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/specify-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SpecifyTaskResponse::try_from(dto).unwrap();
    let wire = v1::SpecifyTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SpecifyTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn promote_task_request_fixture_roundtrip() {
    let path: kanban_protocol::PromoteTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/promote-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::PromoteTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/promote-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::PromoteTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::PromoteTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn promote_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::PromoteTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/promote-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::PromoteTaskResponse::try_from(dto).unwrap();
    let wire = v1::PromoteTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::PromoteTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn claim_task_request_fixture_roundtrip() {
    let path: kanban_protocol::ClaimTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/claim-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ClaimTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/claim-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ClaimTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ClaimTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn claim_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::ClaimTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/claim-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ClaimTaskResponse::try_from(dto).unwrap();
    let wire = v1::ClaimTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ClaimTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reopen_task_request_fixture_roundtrip() {
    let path: kanban_protocol::ReopenTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reopen-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ReopenTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reopen-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ReopenTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ReopenTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reopen_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::ReopenTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reopen-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ReopenTaskResponse::try_from(dto).unwrap();
    let wire = v1::ReopenTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ReopenTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reclaim_task_request_fixture_roundtrip() {
    let path: kanban_protocol::ReclaimTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reclaim-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ReclaimTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reclaim-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ReclaimTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ReclaimTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reclaim_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::ReclaimTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reclaim-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ReclaimTaskResponse::try_from(dto).unwrap();
    let wire = v1::ReclaimTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ReclaimTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn heartbeat_task_request_fixture_roundtrip() {
    let path: kanban_protocol::HeartbeatTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/heartbeat-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::HeartbeatTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/heartbeat-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::HeartbeatTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::HeartbeatTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn heartbeat_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::HeartbeatTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/heartbeat-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::HeartbeatTaskResponse::try_from(dto).unwrap();
    let wire = v1::HeartbeatTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::HeartbeatTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn release_task_request_fixture_roundtrip() {
    let path: kanban_protocol::ReleaseTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/release-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ReleaseTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/release-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ReleaseTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ReleaseTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn release_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::ReleaseTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/release-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ReleaseTaskResponse::try_from(dto).unwrap();
    let wire = v1::ReleaseTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ReleaseTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn complete_task_request_fixture_roundtrip() {
    let path: kanban_protocol::CompleteTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/complete-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::CompleteTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/complete-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CompleteTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CompleteTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn complete_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::CompleteTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/complete-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CompleteTaskResponse::try_from(dto).unwrap();
    let wire = v1::CompleteTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CompleteTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn submit_review_task_request_fixture_roundtrip() {
    let path: kanban_protocol::SubmitReviewTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/submit-review-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::SubmitReviewTaskRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/submit-review-task-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SubmitReviewTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SubmitReviewTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn submit_review_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::SubmitReviewTaskResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/submit-review-task-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SubmitReviewTaskResponse::try_from(dto).unwrap();
    let wire = v1::SubmitReviewTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SubmitReviewTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn block_task_request_fixture_roundtrip() {
    let path: kanban_protocol::BlockTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/block-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::BlockTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/block-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::BlockTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::BlockTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn block_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::BlockTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/block-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::BlockTaskResponse::try_from(dto).unwrap();
    let wire = v1::BlockTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::BlockTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn unblock_task_request_fixture_roundtrip() {
    let path: kanban_protocol::UnblockTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/unblock-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::UnblockTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/unblock-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::UnblockTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::UnblockTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn unblock_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::UnblockTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/unblock-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::UnblockTaskResponse::try_from(dto).unwrap();
    let wire = v1::UnblockTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::UnblockTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn archive_task_request_fixture_roundtrip() {
    let path: kanban_protocol::ArchiveTaskPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/archive-task-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ArchiveTaskRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/archive-task-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ArchiveTaskRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ArchiveTaskRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn archive_task_response_fixture_roundtrip() {
    let dto: kanban_protocol::ArchiveTaskResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/archive-task-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ArchiveTaskResponse::try_from(dto).unwrap();
    let wire = v1::ArchiveTaskResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ArchiveTaskResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_steps_request_fixture_roundtrip() {
    let path: kanban_protocol::ListStepsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-steps-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListStepsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListStepsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_steps_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListStepsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-steps-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListStepsResponse::try_from(dto).unwrap();
    let wire = v1::ListStepsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListStepsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_step_request_fixture_roundtrip() {
    let path: kanban_protocol::CreateStepPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-step-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::CreateStepRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-step-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CreateStepRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CreateStepRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_step_response_fixture_roundtrip() {
    let dto: kanban_protocol::CreateStepResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-step-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CreateStepResponse::try_from(dto).unwrap();
    let wire = v1::CreateStepResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CreateStepResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn update_step_request_fixture_roundtrip() {
    let path: kanban_protocol::UpdateStepPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/update-step-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::UpdateStepRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/update-step-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::UpdateStepRequest::from_parts(path, query, input).unwrap();
    let wire = v1::UpdateStepRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn update_step_response_fixture_roundtrip() {
    let dto: kanban_protocol::UpdateStepResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/update-step-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::UpdateStepResponse::try_from(dto).unwrap();
    let wire = v1::UpdateStepResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::UpdateStepResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn remove_step_request_fixture_roundtrip() {
    let path: kanban_protocol::RemoveStepPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/remove-step-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RemoveStepRequest::from_parts(path, query, input).unwrap();
    let wire = v1::RemoveStepRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn remove_step_response_fixture_roundtrip() {
    let dto: kanban_protocol::RemoveStepResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/remove-step-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RemoveStepResponse::try_from(dto).unwrap();
    let wire = v1::RemoveStepResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::RemoveStepResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn complete_step_request_fixture_roundtrip() {
    let path: kanban_protocol::CompleteStepPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/complete-step-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::CompleteStepRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/complete-step-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CompleteStepRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CompleteStepRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn complete_step_response_fixture_roundtrip() {
    let dto: kanban_protocol::CompleteStepResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/complete-step-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CompleteStepResponse::try_from(dto).unwrap();
    let wire = v1::CompleteStepResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CompleteStepResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn skip_step_request_fixture_roundtrip() {
    let path: kanban_protocol::SkipStepPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/skip-step-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::SkipStepRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/skip-step-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SkipStepRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SkipStepRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn skip_step_response_fixture_roundtrip() {
    let dto: kanban_protocol::SkipStepResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/skip-step-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SkipStepResponse::try_from(dto).unwrap();
    let wire = v1::SkipStepResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SkipStepResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reopen_step_request_fixture_roundtrip() {
    let path: kanban_protocol::ReopenStepPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reopen-step-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ReopenStepRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reopen-step-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ReopenStepRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ReopenStepRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reopen_step_response_fixture_roundtrip() {
    let dto: kanban_protocol::ReopenStepResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reopen-step-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ReopenStepResponse::try_from(dto).unwrap();
    let wire = v1::ReopenStepResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ReopenStepResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn mark_execution_plan_not_required_request_fixture_roundtrip() {
    let path: kanban_protocol::MarkExecutionPlanNotRequiredPath =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/mark-execution-plan-not-required-path.v1.valid.json"
        )))
        .unwrap();
    let query = ();
    let input: kanban_protocol::MarkExecutionPlanNotRequiredRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/mark-execution-plan-not-required-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MarkExecutionPlanNotRequiredRequest::from_parts(path, query, input).unwrap();
    let wire =
        v1::MarkExecutionPlanNotRequiredRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn mark_execution_plan_not_required_response_fixture_roundtrip() {
    let dto: kanban_protocol::MarkExecutionPlanNotRequiredResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/mark-execution-plan-not-required-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MarkExecutionPlanNotRequiredResponse::try_from(dto).unwrap();
    let wire =
        v1::MarkExecutionPlanNotRequiredResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::MarkExecutionPlanNotRequiredResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_dependencies_request_fixture_roundtrip() {
    let path: kanban_protocol::ListDependenciesPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-dependencies-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListDependenciesRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListDependenciesRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_dependencies_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListDependenciesResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-dependencies-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListDependenciesResponse::try_from(dto).unwrap();
    let wire = v1::ListDependenciesResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListDependenciesResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn add_dependency_request_fixture_roundtrip() {
    let path: kanban_protocol::AddDependencyPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/add-dependency-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::AddDependencyRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/add-dependency-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::AddDependencyRequest::from_parts(path, query, input).unwrap();
    let wire = v1::AddDependencyRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn add_dependency_response_fixture_roundtrip() {
    let dto: kanban_protocol::AddDependencyResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/add-dependency-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::AddDependencyResponse::try_from(dto).unwrap();
    let wire = v1::AddDependencyResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::AddDependencyResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn remove_dependency_request_fixture_roundtrip() {
    let path: kanban_protocol::RemoveDependencyPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/remove-dependency-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RemoveDependencyRequest::from_parts(path, query, input).unwrap();
    let wire = v1::RemoveDependencyRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn remove_dependency_response_fixture_roundtrip() {
    let dto: kanban_protocol::RemoveDependencyResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/remove-dependency-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RemoveDependencyResponse::try_from(dto).unwrap();
    let wire = v1::RemoveDependencyResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::RemoveDependencyResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_runs_request_fixture_roundtrip() {
    let path: kanban_protocol::ListRunsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-runs-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListRunsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListRunsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_runs_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListRunsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-runs-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListRunsResponse::try_from(dto).unwrap();
    let wire = v1::ListRunsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListRunsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_run_request_fixture_roundtrip() {
    let path: kanban_protocol::GetRunPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-run-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetRunRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetRunRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_run_response_fixture_roundtrip() {
    let dto: kanban_protocol::GetRunResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-run-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetRunResponse::try_from(dto).unwrap();
    let wire = v1::GetRunResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GetRunResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_run_log_request_fixture_roundtrip() {
    let path: kanban_protocol::GetRunLogPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-run-log-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetRunLogRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetRunLogRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_run_log_response_fixture_roundtrip() {
    let dto: kanban_protocol::GetRunLogResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-run-log-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetRunLogResponse::try_from(dto).unwrap();
    let wire = v1::GetRunLogResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GetRunLogResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_comments_request_fixture_roundtrip() {
    let path: kanban_protocol::ListCommentsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-comments-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListCommentsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListCommentsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_comments_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListCommentsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-comments-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListCommentsResponse::try_from(dto).unwrap();
    let wire = v1::ListCommentsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListCommentsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_comment_request_fixture_roundtrip() {
    let path: kanban_protocol::CreateCommentPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-comment-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::CreateCommentRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-comment-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CreateCommentRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CreateCommentRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_comment_response_fixture_roundtrip() {
    let dto: kanban_protocol::CreateCommentResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-comment-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CreateCommentResponse::try_from(dto).unwrap();
    let wire = v1::CreateCommentResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CreateCommentResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_attachments_request_fixture_roundtrip() {
    let path: kanban_protocol::ListAttachmentsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-attachments-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListAttachmentsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListAttachmentsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_attachments_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListAttachmentsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-attachments-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListAttachmentsResponse::try_from(dto).unwrap();
    let wire = v1::ListAttachmentsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListAttachmentsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_attachment_request_fixture_roundtrip() {
    let path: kanban_protocol::CreateAttachmentPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-attachment-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::CreateAttachmentRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/create-attachment-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CreateAttachmentRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CreateAttachmentRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_attachment_response_fixture_roundtrip() {
    let dto: kanban_protocol::CreateAttachmentResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/create-attachment-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CreateAttachmentResponse::try_from(dto).unwrap();
    let wire = v1::CreateAttachmentResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CreateAttachmentResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn download_attachment_request_fixture_roundtrip() {
    let path: kanban_protocol::GetAttachmentPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/download-attachment-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::DownloadAttachmentRequest::from_parts(path, query, input).unwrap();
    let wire = v1::DownloadAttachmentRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn delete_attachment_request_fixture_roundtrip() {
    let path: kanban_protocol::DeleteAttachmentPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/delete-attachment-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::DeleteAttachmentRequest::from_parts(path, query, input).unwrap();
    let wire = v1::DeleteAttachmentRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn delete_attachment_response_fixture_roundtrip() {
    let dto: kanban_protocol::DeleteAttachmentResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/delete-attachment-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::DeleteAttachmentResponse::try_from(dto).unwrap();
    let wire = v1::DeleteAttachmentResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::DeleteAttachmentResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_events_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::ListEventsQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-events-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListEventsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListEventsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_events_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListEventsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-events-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListEventsResponse::try_from(dto).unwrap();
    let wire = v1::ListEventsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListEventsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_task_labels_request_fixture_roundtrip() {
    let path: kanban_protocol::ListTaskLabelsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-task-labels-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListTaskLabelsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListTaskLabelsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_task_labels_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListTaskLabelsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-task-labels-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListTaskLabelsResponse::try_from(dto).unwrap();
    let wire = v1::ListTaskLabelsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListTaskLabelsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn add_task_label_request_fixture_roundtrip() {
    let path: kanban_protocol::AddTaskLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/add-task-label-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::AddTaskLabelRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/add-task-label-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::AddTaskLabelRequest::from_parts(path, query, input).unwrap();
    let wire = v1::AddTaskLabelRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn add_task_label_response_fixture_roundtrip() {
    let dto: kanban_protocol::AddTaskLabelResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/add-task-label-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::AddTaskLabelResponse::try_from(dto).unwrap();
    let wire = v1::AddTaskLabelResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::AddTaskLabelResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn bootstrap_task_label_request_fixture_roundtrip() {
    let path: kanban_protocol::TaskLabelSurfacePath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/bootstrap-task-label-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::BootstrapTaskLabelRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/bootstrap-task-label-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::BootstrapTaskLabelRequest::from_parts(path, query, input).unwrap();
    let wire = v1::BootstrapTaskLabelRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn bootstrap_task_label_response_fixture_roundtrip() {
    let dto: kanban_protocol::BootstrapTaskLabelResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/bootstrap-task-label-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::BootstrapTaskLabelResponse::try_from(dto).unwrap();
    let wire = v1::BootstrapTaskLabelResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::BootstrapTaskLabelResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn remove_task_label_request_fixture_roundtrip() {
    let path: kanban_protocol::RemoveTaskLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/remove-task-label-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RemoveTaskLabelRequest::from_parts(path, query, input).unwrap();
    let wire = v1::RemoveTaskLabelRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn remove_task_label_response_fixture_roundtrip() {
    let dto: kanban_protocol::RemoveTaskLabelResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/remove-task-label-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RemoveTaskLabelResponse::try_from(dto).unwrap();
    let wire = v1::RemoveTaskLabelResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::RemoveTaskLabelResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_board_labels_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-board-labels-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListBoardLabelsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListBoardLabelsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_board_labels_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListBoardLabelsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-board-labels-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListBoardLabelsResponse::try_from(dto).unwrap();
    let wire = v1::ListBoardLabelsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListBoardLabelsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_board_label_proposals_request_fixture_roundtrip() {
    let path: kanban_protocol::ListBoardLabelProposalsPath =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-board-label-proposals-path.v1.valid.json"
        )))
        .unwrap();
    let query: kanban_protocol::ListBoardLabelProposalsQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-board-label-proposals-query.v1.valid.json"
        )))
        .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListBoardLabelProposalsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListBoardLabelProposalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_board_label_proposals_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListBoardLabelProposalsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-board-label-proposals-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListBoardLabelProposalsResponse::try_from(dto).unwrap();
    let wire =
        v1::ListBoardLabelProposalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListBoardLabelProposalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_board_label_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-board-label-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::CreateBoardLabelRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/create-board-label-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CreateBoardLabelRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CreateBoardLabelRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_board_label_response_fixture_roundtrip() {
    let dto: kanban_protocol::CreateBoardLabelResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/create-board-label-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CreateBoardLabelResponse::try_from(dto).unwrap();
    let wire = v1::CreateBoardLabelResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CreateBoardLabelResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn delete_board_label_request_fixture_roundtrip() {
    let path: kanban_protocol::DeleteBoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/delete-board-label-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::DeleteBoardLabelQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/delete-board-label-query.v1.valid.json"
        )))
        .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::DeleteBoardLabelRequest::from_parts(path, query, input).unwrap();
    let wire = v1::DeleteBoardLabelRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn delete_board_label_response_fixture_roundtrip() {
    let dto: kanban_protocol::DeleteBoardLabelResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/delete-board-label-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::DeleteBoardLabelResponse::try_from(dto).unwrap();
    let wire = v1::DeleteBoardLabelResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::DeleteBoardLabelResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_label_semantics_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-label-semantics-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListLabelSemanticsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListLabelSemanticsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_label_semantics_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListLabelSemanticsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-label-semantics-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListLabelSemanticsResponse::try_from(dto).unwrap();
    let wire = v1::ListLabelSemanticsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListLabelSemanticsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_label_semantics_request_fixture_roundtrip() {
    let path: kanban_protocol::LabelSemanticsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-label-semantics-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetLabelSemanticsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetLabelSemanticsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_label_semantics_response_fixture_roundtrip() {
    let dto: kanban_protocol::GetLabelSemanticsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/get-label-semantics-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetLabelSemanticsResponse::try_from(dto).unwrap();
    let wire = v1::GetLabelSemanticsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GetLabelSemanticsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn upsert_label_semantics_request_fixture_roundtrip() {
    let path: kanban_protocol::LabelSemanticsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/upsert-label-semantics-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::UpsertLabelSemanticsRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/upsert-label-semantics-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::UpsertLabelSemanticsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::UpsertLabelSemanticsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn upsert_label_semantics_response_fixture_roundtrip() {
    let dto: kanban_protocol::UpsertLabelSemanticsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/upsert-label-semantics-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::UpsertLabelSemanticsResponse::try_from(dto).unwrap();
    let wire = v1::UpsertLabelSemanticsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::UpsertLabelSemanticsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn delete_label_semantics_request_fixture_roundtrip() {
    let path: kanban_protocol::LabelSemanticsPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/delete-label-semantics-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::DeleteLabelSemanticsQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/delete-label-semantics-query.v1.valid.json"
        )))
        .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::DeleteLabelSemanticsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::DeleteLabelSemanticsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn delete_label_semantics_response_fixture_roundtrip() {
    let dto: kanban_protocol::DeleteResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/delete-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::DeleteLabelSemanticsResponse::try_from(dto).unwrap();
    let wire = v1::DeleteLabelSemanticsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::DeleteResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_label_atoms_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-label-atoms-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListLabelAtomsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListLabelAtomsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_label_atoms_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListLabelAtomsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-label-atoms-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListLabelAtomsResponse::try_from(dto).unwrap();
    let wire = v1::ListLabelAtomsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListLabelAtomsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn explain_label_atom_request_fixture_roundtrip() {
    let path: kanban_protocol::LabelAtomPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/label-atom-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ExplainLabelAtomRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ExplainLabelAtomRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn explain_label_atom_response_fixture_roundtrip() {
    let dto: kanban_protocol::ExplainLabelAtomResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/explain-label-atom-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ExplainLabelAtomResponse::try_from(dto).unwrap();
    let wire = v1::ExplainLabelAtomResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ExplainLabelAtomResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn label_atom_index_status_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/label-atom-index-status-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::LabelAtomIndexStatusRequest::from_parts(path, query, input).unwrap();
    let wire = v1::LabelAtomIndexStatusRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn label_atom_index_status_response_fixture_roundtrip() {
    let dto: kanban_protocol::LabelAtomIndexStatusResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/label-atom-index-status-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::LabelAtomIndexStatusResponse::try_from(dto).unwrap();
    let wire = v1::LabelAtomIndexStatusResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LabelAtomIndexStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn rebuild_label_atom_index_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/rebuild-label-atom-index-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RebuildLabelAtomIndexRequest::from_parts(path, query, input).unwrap();
    let wire = v1::RebuildLabelAtomIndexRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn rebuild_label_atom_index_response_fixture_roundtrip() {
    let dto: kanban_protocol::RebuildLabelAtomIndexResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/rebuild-label-atom-index-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RebuildLabelAtomIndexResponse::try_from(dto).unwrap();
    let wire = v1::RebuildLabelAtomIndexResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::RebuildLabelAtomIndexResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn query_label_atom_index_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/query-label-atom-index-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::LabelAtomIndexQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/query-label-atom-index-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::QueryLabelAtomIndexRequest::from_parts(path, query, input).unwrap();
    let wire = v1::QueryLabelAtomIndexRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_signals_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-signals-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::SignalQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-signals-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListSignalsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListSignalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_signals_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListSignalsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-signals-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListSignalsResponse::try_from(dto).unwrap();
    let wire = v1::ListSignalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListSignalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn review_signals_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/review-signals-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::SignalQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/review-signals-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ReviewSignalsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ReviewSignalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn review_signals_response_fixture_roundtrip() {
    let dto: kanban_protocol::ReviewSignalsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/review-signals-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ReviewSignalsResponse::try_from(dto).unwrap();
    let wire = v1::ReviewSignalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ReviewSignalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_signal_request_fixture_roundtrip() {
    let path: kanban_protocol::SignalPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-signal-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetSignalRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetSignalRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_signal_response_fixture_roundtrip() {
    let dto: kanban_protocol::GetSignalResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-signal-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetSignalResponse::try_from(dto).unwrap();
    let wire = v1::GetSignalResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GetSignalResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn record_signal_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/record-signal-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::RecordSignalRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/record-signal-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RecordSignalRequest::from_parts(path, query, input).unwrap();
    let wire = v1::RecordSignalRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn record_signal_response_fixture_roundtrip() {
    let dto: kanban_protocol::RecordSignalResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/record-signal-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RecordSignalResponse::try_from(dto).unwrap();
    let wire = v1::RecordSignalResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::RecordSignalResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn confirm_signals_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/confirm-signals-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ReviewSignalsRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/review-signals-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ConfirmSignalsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ConfirmSignalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn confirm_signals_response_fixture_roundtrip() {
    let dto: kanban_protocol::ConfirmSignalsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/confirm-signals-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ConfirmSignalsResponse::try_from(dto).unwrap();
    let wire = v1::ConfirmSignalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ConfirmSignalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reject_signals_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reject-signals-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ReviewSignalsRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reject-signals-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RejectSignalsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::RejectSignalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reject_signals_response_fixture_roundtrip() {
    let dto: kanban_protocol::RejectSignalsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reject-signals-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RejectSignalsResponse::try_from(dto).unwrap();
    let wire = v1::RejectSignalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::RejectSignalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn resolve_signals_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/resolve-signals-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ReviewSignalsRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/resolve-signals-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ResolveSignalsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ResolveSignalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn resolve_signals_response_fixture_roundtrip() {
    let dto: kanban_protocol::ResolveSignalsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/resolve-signals-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ResolveSignalsResponse::try_from(dto).unwrap();
    let wire = v1::ResolveSignalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ResolveSignalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn supersede_signals_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/supersede-signals-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ReviewSignalsRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/supersede-signals-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SupersedeSignalsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SupersedeSignalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn supersede_signals_response_fixture_roundtrip() {
    let dto: kanban_protocol::SupersedeSignalsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/supersede-signals-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SupersedeSignalsResponse::try_from(dto).unwrap();
    let wire = v1::SupersedeSignalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SupersedeSignalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn suggest_task_labels_request_fixture_roundtrip() {
    let path: kanban_protocol::TaskLabelSurfacePath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/suggest-task-labels-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::rpc::dto::TaskLabelSuggestionQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/label-suggestion-query.v1.valid.json"
        )))
        .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SuggestTaskLabelsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SuggestTaskLabelsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn suggest_task_labels_response_fixture_roundtrip() {
    let dto: kanban_protocol::SuggestTaskLabelsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/suggest-task-labels-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SuggestTaskLabelsResponse::try_from(dto).unwrap();
    let wire = v1::SuggestTaskLabelsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SuggestTaskLabelsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_task_label_proposals_request_fixture_roundtrip() {
    let path: kanban_protocol::TaskLabelSurfacePath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-task-label-proposals-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::rpc::dto::TaskLabelProposalQuery =
        serde_json::from_str("{}").unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListTaskLabelProposalsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListTaskLabelProposalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_task_label_proposals_response_fixture_roundtrip() {
    let dto: kanban_protocol::ListTaskLabelProposalsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-task-label-proposals-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListTaskLabelProposalsResponse::try_from(dto).unwrap();
    let wire = v1::ListTaskLabelProposalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ListTaskLabelProposalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn propose_task_label_request_fixture_roundtrip() {
    let path: kanban_protocol::TaskLabelSurfacePath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/propose-task-label-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::rpc::dto::TaskLabelSuggestionQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/propose-task-label-query.v1.valid.json"
        )))
        .unwrap();
    let input: kanban_protocol::ProposeTaskLabelRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/propose-task-label-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ProposeTaskLabelRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ProposeTaskLabelRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn propose_task_label_response_fixture_roundtrip() {
    let dto: kanban_protocol::ProposeTaskLabelResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/propose-task-label-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ProposeTaskLabelResponse::try_from(dto).unwrap();
    let wire = v1::ProposeTaskLabelResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ProposeTaskLabelResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn record_label_ontology_observation_request_fixture_roundtrip() {
    let path: kanban_protocol::TaskLabelSurfacePath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/record-label-ontology-observation-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::rpc::dto::TaskLabelBoardQuery = serde_json::from_str("{}").unwrap();
    let input: kanban_protocol::RecordLabelOntologyObservationRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/record-label-ontology-observation-body.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RecordLabelOntologyObservationRequest::from_parts(path, query, input).unwrap();
    let wire =
        v1::RecordLabelOntologyObservationRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn record_label_ontology_observation_response_fixture_roundtrip() {
    let dto: kanban_protocol::RecordLabelOntologyObservationResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/record-label-ontology-observation-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RecordLabelOntologyObservationResponse::try_from(dto).unwrap();
    let wire = v1::RecordLabelOntologyObservationResponse::decode(wire.encode_to_vec().as_slice())
        .unwrap();
    let actual: kanban_protocol::RecordLabelOntologyObservationResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_label_ontology_signals_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/list-label-ontology-signals-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::LabelOntologySignalQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/label-ontology-signal-query.v1.valid.json"
        )))
        .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListLabelOntologySignalsRequest::from_parts(path, query, input).unwrap();
    let wire =
        v1::ListLabelOntologySignalsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_label_ontology_signals_response_fixture_roundtrip() {
    let dto: kanban_protocol::LabelOntologySignalsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/list-label-ontology-signals-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListLabelOntologySignalsResponse::try_from(dto).unwrap();
    let wire =
        v1::ListLabelOntologySignalsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LabelOntologySignalsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn review_label_ontology_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/review-label-ontology-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::LabelOntologyReviewQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/label-ontology-review-query.v1.valid.json"
        )))
        .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ReviewLabelOntologyRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ReviewLabelOntologyRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn review_label_ontology_response_fixture_roundtrip() {
    let dto: kanban_protocol::ReviewLabelOntologyResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/review-label-ontology-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ReviewLabelOntologyResponse::try_from(dto).unwrap();
    let wire = v1::ReviewLabelOntologyResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ReviewLabelOntologyResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_label_ontology_action_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/create-label-ontology-action-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::LabelOntologyActionRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/create-label-ontology-action-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CreateLabelOntologyActionRequest::from_parts(path, query, input).unwrap();
    let wire =
        v1::CreateLabelOntologyActionRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn create_label_ontology_action_response_fixture_roundtrip() {
    let dto: kanban_protocol::LabelOntologyActionResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/create-label-ontology-action-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CreateLabelOntologyActionResponse::try_from(dto).unwrap();
    let wire =
        v1::CreateLabelOntologyActionResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LabelOntologyActionResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn apply_label_ontology_atom_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/apply-label-ontology-atom-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ApplyLabelOntologyAtomRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/apply-label-ontology-atom-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ApplyLabelOntologyAtomRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ApplyLabelOntologyAtomRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn apply_label_ontology_atom_response_fixture_roundtrip() {
    let dto: kanban_protocol::LabelOntologyActionResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/apply-label-ontology-atom-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ApplyLabelOntologyAtomResponse::try_from(dto).unwrap();
    let wire = v1::ApplyLabelOntologyAtomResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LabelOntologyActionResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn revert_label_ontology_mutation_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/revert-label-ontology-mutation-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::RevertLabelOntologyMutationRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/revert-label-ontology-mutation-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RevertLabelOntologyMutationRequest::from_parts(path, query, input).unwrap();
    let wire =
        v1::RevertLabelOntologyMutationRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn revert_label_ontology_mutation_response_fixture_roundtrip() {
    let dto: kanban_protocol::LabelOntologyActionResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/revert-label-ontology-mutation-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RevertLabelOntologyMutationResponse::try_from(dto).unwrap();
    let wire =
        v1::RevertLabelOntologyMutationResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LabelOntologyActionResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn validate_label_ontology_action_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardLabelPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/validate-label-ontology-action-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::ValidateLabelOntologyActionRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/validate-label-ontology-action-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ValidateLabelOntologyActionRequest::from_parts(path, query, input).unwrap();
    let wire =
        v1::ValidateLabelOntologyActionRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn validate_label_ontology_action_response_fixture_roundtrip() {
    let dto: kanban_protocol::LabelOntologyActionResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/validate-label-ontology-action-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ValidateLabelOntologyActionResponse::try_from(dto).unwrap();
    let wire =
        v1::ValidateLabelOntologyActionResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LabelOntologyActionResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_label_ontology_signal_request_fixture_roundtrip() {
    let path: kanban_protocol::SignalPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-label-ontology-signal-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetLabelOntologySignalRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetLabelOntologySignalRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_label_ontology_signal_response_fixture_roundtrip() {
    let dto: kanban_protocol::GetLabelOntologySignalResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/get-label-ontology-signal-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetLabelOntologySignalResponse::try_from(dto).unwrap();
    let wire = v1::GetLabelOntologySignalResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GetLabelOntologySignalResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_label_proposal_request_fixture_roundtrip() {
    let path: kanban_protocol::ProposalPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-label-proposal-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetLabelProposalRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetLabelProposalRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_label_proposal_response_fixture_roundtrip() {
    let dto: kanban_protocol::GetLabelProposalResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/get-label-proposal-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetLabelProposalResponse::try_from(dto).unwrap();
    let wire = v1::GetLabelProposalResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GetLabelProposalResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn accept_label_proposal_request_fixture_roundtrip() {
    let path: kanban_protocol::ProposalPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/accept-label-proposal-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::LabelProposalDecisionRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/accept-label-proposal-body.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::AcceptLabelProposalRequest::from_parts(path, query, input).unwrap();
    let wire = v1::AcceptLabelProposalRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn accept_label_proposal_response_fixture_roundtrip() {
    let dto: kanban_protocol::LabelProposalDecisionResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/accept-label-proposal-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::AcceptLabelProposalResponse::try_from(dto).unwrap();
    let wire = v1::AcceptLabelProposalResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LabelProposalDecisionResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reject_label_proposal_request_fixture_roundtrip() {
    let path: kanban_protocol::ProposalPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/reject-label-proposal-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input: kanban_protocol::LabelProposalDecisionRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/reject-label-proposal-body.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RejectLabelProposalRequest::from_parts(path, query, input).unwrap();
    let wire = v1::RejectLabelProposalRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn reject_label_proposal_response_fixture_roundtrip() {
    let dto: kanban_protocol::LabelProposalDecisionResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/reject-label-proposal-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RejectLabelProposalResponse::try_from(dto).unwrap();
    let wire = v1::RejectLabelProposalResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LabelProposalDecisionResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn board_task_map_request_fixture_roundtrip() {
    let path: kanban_protocol::BoardTaskMapPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/board-task-map-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::BoardTaskMapQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/board-task-map-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::BoardTaskMapRequest::from_parts(path, query, input).unwrap();
    let wire = v1::BoardTaskMapRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn board_task_map_response_fixture_roundtrip() {
    let dto: kanban_protocol::BoardTaskMapResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/board-task-map-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::BoardTaskMapResponse::try_from(dto).unwrap();
    let wire = v1::BoardTaskMapResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::BoardTaskMapResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn task_neighborhood_request_fixture_roundtrip() {
    let path: kanban_protocol::TaskNeighborhoodPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/task-neighborhood-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::TaskNeighborhoodQuery =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/task-neighborhood-query.v1.valid.json"
        )))
        .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::TaskNeighborhoodRequest::from_parts(path, query, input).unwrap();
    let wire = v1::TaskNeighborhoodRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn task_neighborhood_response_fixture_roundtrip() {
    let dto: kanban_protocol::TaskNeighborhoodResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/task-neighborhood-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::TaskNeighborhoodResponse::try_from(dto).unwrap();
    let wire = v1::TaskNeighborhoodResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::TaskNeighborhoodResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn search_tasks_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::SearchTasksQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-tasks-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SearchTasksRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SearchTasksRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn search_tasks_response_fixture_roundtrip() {
    let dto: kanban_protocol::SearchTasksResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-tasks-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SearchTasksResponse::try_from(dto).unwrap();
    let wire = v1::SearchTasksResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SearchTasksResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn search_tasks_by_status_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::SearchTasksQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-tasks-by-status-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SearchTasksByStatusRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SearchTasksByStatusRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn search_tasks_by_status_response_fixture_roundtrip() {
    let dto: kanban_protocol::SearchTasksByStatusResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/search-tasks-by-status-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SearchTasksByStatusResponse::try_from(dto).unwrap();
    let wire = v1::SearchTasksByStatusResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SearchTasksByStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn search_status_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::BoardQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-status-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SearchStatusRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SearchStatusRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn search_status_response_fixture_roundtrip() {
    let dto: kanban_protocol::SearchStatusResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-status-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SearchStatusResponse::try_from(dto).unwrap();
    let wire = v1::SearchStatusResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SearchStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn rebuild_search_index_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::BoardQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-status-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::RebuildSearchIndexRequest::from_parts(path, query, input).unwrap();
    let wire = v1::RebuildSearchIndexRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn rebuild_search_index_response_fixture_roundtrip() {
    let dto: kanban_protocol::SearchStatusResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-status-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::RebuildSearchIndexResponse::try_from(dto).unwrap();
    let wire = v1::RebuildSearchIndexResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SearchStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn sync_search_index_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::BoardQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-status-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::SyncSearchIndexRequest::from_parts(path, query, input).unwrap();
    let wire = v1::SyncSearchIndexRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn sync_search_index_response_fixture_roundtrip() {
    let dto: kanban_protocol::SearchStatusResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/search-status-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::SyncSearchIndexResponse::try_from(dto).unwrap();
    let wire = v1::SyncSearchIndexResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::SearchStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn build_context_request_fixture_roundtrip() {
    let path: kanban_protocol::BuildContextPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/build-context-path.v1.valid.json"
    )))
    .unwrap();
    let query: kanban_protocol::BuildContextQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/build-context-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::BuildContextRequest::from_parts(path, query, input).unwrap();
    let wire = v1::BuildContextRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn build_context_response_fixture_roundtrip() {
    let dto: kanban_protocol::BuildContextResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/build-context-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::BuildContextResponse::try_from(dto).unwrap();
    let wire = v1::BuildContextResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::BuildContextResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_status_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::BoardQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/graph-status-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GraphStatusRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GraphStatusRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_status_response_fixture_roundtrip() {
    let dto: kanban_protocol::GraphStatusResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/graph-status-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GraphStatusResponse::try_from(dto).unwrap();
    let wire = v1::GraphStatusResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GraphStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_neighbors_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::GraphNeighborsQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/graph-neighbors-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GraphNeighborsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GraphNeighborsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_neighbors_response_fixture_roundtrip() {
    let dto: kanban_protocol::GraphNeighborsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/graph-neighbors-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GraphNeighborsResponse::try_from(dto).unwrap();
    let wire = v1::GraphNeighborsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GraphNeighborsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_query_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::GraphQueryQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/graph-query-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GraphQueryRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GraphQueryRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_query_response_fixture_roundtrip() {
    let dto: kanban_protocol::cli_helpers::CliGraphQueryOutput =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/graph-query-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GraphQueryResponse::try_from(dto).unwrap();
    let wire = v1::GraphQueryResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::cli_helpers::CliGraphQueryOutput = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_rebuild_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::BoardQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/graph-rebuild-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GraphRebuildRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GraphRebuildRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_rebuild_response_fixture_roundtrip() {
    let dto: kanban_protocol::GraphMaintenanceResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/graph-rebuild-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GraphRebuildResponse::try_from(dto).unwrap();
    let wire = v1::GraphRebuildResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GraphMaintenanceResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_sync_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::BoardQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/graph-sync-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GraphSyncRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GraphSyncRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn graph_sync_response_fixture_roundtrip() {
    let dto: kanban_protocol::GraphMaintenanceResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/graph-sync-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GraphSyncResponse::try_from(dto).unwrap();
    let wire = v1::GraphSyncResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::GraphMaintenanceResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_entities_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::EntityListQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/entity-list-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::ListEntitiesRequest::from_parts(path, query, input).unwrap();
    let wire = v1::ListEntitiesRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn list_entities_response_fixture_roundtrip() {
    let dto: kanban_protocol::EntityListResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/entity-list-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::ListEntitiesResponse::try_from(dto).unwrap();
    let wire = v1::ListEntitiesResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::EntityListResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn upsert_entity_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::EntityUpsertRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/entity-upsert-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::UpsertEntityRequest::from_parts(path, query, input).unwrap();
    let wire = v1::UpsertEntityRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn upsert_entity_response_fixture_roundtrip() {
    let dto: kanban_protocol::EntityResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/entity-upsert-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::UpsertEntityResponse::try_from(dto).unwrap();
    let wire = v1::UpsertEntityResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::EntityResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_entity_request_fixture_roundtrip() {
    let path: kanban_protocol::EntityPath = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/entity-path.v1.valid.json"
    )))
    .unwrap();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetEntityRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetEntityRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_entity_response_fixture_roundtrip() {
    let dto: kanban_protocol::EntityResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/entity-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetEntityResponse::try_from(dto).unwrap();
    let wire = v1::GetEntityResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::EntityResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_status_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::VectorStatusQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/vector-status-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::VectorStatusRequest::from_parts(path, query, input).unwrap();
    let wire = v1::VectorStatusRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_status_response_fixture_roundtrip() {
    let dto: kanban_protocol::VectorStatusResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/vector-status-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::VectorStatusResponse::try_from(dto).unwrap();
    let wire = v1::VectorStatusResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::VectorStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_configure_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::VectorConfigureRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/vector-configure-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::VectorConfigureRequest::from_parts(path, query, input).unwrap();
    let wire = v1::VectorConfigureRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_configure_response_fixture_roundtrip() {
    let dto: kanban_protocol::VectorConfigureResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/vector-configure-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::VectorConfigureResponse::try_from(dto).unwrap();
    let wire = v1::VectorConfigureResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::VectorConfigureResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_rebuild_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::VectorProjectionRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/vector-rebuild-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::VectorRebuildRequest::from_parts(path, query, input).unwrap();
    let wire = v1::VectorRebuildRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_rebuild_response_fixture_roundtrip() {
    let dto: kanban_protocol::VectorProjectionResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/vector-rebuild-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::VectorRebuildResponse::try_from(dto).unwrap();
    let wire = v1::VectorRebuildResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::VectorProjectionResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_sync_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::VectorProjectionRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/vector-sync-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::VectorSyncRequest::from_parts(path, query, input).unwrap();
    let wire = v1::VectorSyncRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_sync_response_fixture_roundtrip() {
    let dto: kanban_protocol::VectorProjectionResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/vector-sync-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::VectorSyncResponse::try_from(dto).unwrap();
    let wire = v1::VectorSyncResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::VectorProjectionResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_query_chunks_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::VectorQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/vector-query-chunks-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::VectorQueryChunksRequest::from_parts(path, query, input).unwrap();
    let wire = v1::VectorQueryChunksRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_query_chunks_response_fixture_roundtrip() {
    let dto: kanban_protocol::VectorQueryChunksResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/vector-query-chunks-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::VectorQueryChunksResponse::try_from(dto).unwrap();
    let wire = v1::VectorQueryChunksResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::VectorQueryChunksResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_query_label_atoms_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::VectorQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/vector-query-label-atoms-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::VectorQueryLabelAtomsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::VectorQueryLabelAtomsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn vector_query_label_atoms_response_fixture_roundtrip() {
    let dto: kanban_protocol::VectorQueryLabelAtomsResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/vector-query-label-atoms-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::VectorQueryLabelAtomsResponse::try_from(dto).unwrap();
    let wire = v1::VectorQueryLabelAtomsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::VectorQueryLabelAtomsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_stats_request_fixture_roundtrip() {
    let path = ();
    let query: kanban_protocol::BoardQuery = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-stats-query.v1.valid.json"
    )))
    .unwrap();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::GetStatsRequest::from_parts(path, query, input).unwrap();
    let wire = v1::GetStatsRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn get_stats_response_fixture_roundtrip() {
    let dto: kanban_protocol::StatsResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/get-stats-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::GetStatsResponse::try_from(dto).unwrap();
    let wire = v1::GetStatsResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::StatsResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn doctor_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::DoctorRequest::from_parts(path, query, input).unwrap();
    let wire = v1::DoctorRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn doctor_response_fixture_roundtrip() {
    let dto: kanban_protocol::DoctorResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/doctor-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::DoctorResponse::try_from(dto).unwrap();
    let wire = v1::DoctorResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::DoctorResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn checkpoint_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::CheckpointRequest::from_parts(path, query, input).unwrap();
    let wire = v1::CheckpointRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn checkpoint_response_fixture_roundtrip() {
    let dto: kanban_protocol::CheckpointResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/checkpoint-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::CheckpointResponse::try_from(dto).unwrap();
    let wire = v1::CheckpointResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::CheckpointResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_backup_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::MaintenancePathRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/maintenance-backup-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceBackupRequest::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceBackupRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_backup_response_fixture_roundtrip() {
    let dto: kanban_protocol::BackupResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-backup-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceBackupResponse::try_from(dto).unwrap();
    let wire = v1::MaintenanceBackupResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::BackupResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_export_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::MaintenancePathRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/maintenance-export-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceExportRequest::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceExportRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_export_response_fixture_roundtrip() {
    let dto: kanban_protocol::ExportResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-export-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceExportResponse::try_from(dto).unwrap();
    let wire = v1::MaintenanceExportResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ExportResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_import_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::MaintenanceImportRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/maintenance-import-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceImportRequest::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceImportRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_import_response_fixture_roundtrip() {
    let dto: kanban_protocol::ImportResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-import-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceImportResponse::try_from(dto).unwrap();
    let wire = v1::MaintenanceImportResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::ImportResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_vacuum_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceVacuumRequest::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceVacuumRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_vacuum_response_fixture_roundtrip() {
    let dto: kanban_protocol::VacuumResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-vacuum-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceVacuumResponse::try_from(dto).unwrap();
    let wire = v1::MaintenanceVacuumResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::VacuumResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_status_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input = ();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceStatusRequest::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceStatusRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_status_response_fixture_roundtrip() {
    let dto: kanban_protocol::MaintenanceStatusResponse =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/maintenance-status-response.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceStatusResponse::try_from(dto).unwrap();
    let wire = v1::MaintenanceStatusResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::MaintenanceStatusResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_run_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::MaintenanceRunRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/maintenance-run-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceRunRequest::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceRunRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_run_response_fixture_roundtrip() {
    let dto: kanban_protocol::MaintenanceRunResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-run-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceRunResponse::try_from(dto).unwrap();
    let wire = v1::MaintenanceRunResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::MaintenanceRunResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_rebuild_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::MaintenanceRunRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/maintenance-rebuild-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceRebuildRequest::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceRebuildRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_rebuild_response_fixture_roundtrip() {
    let dto: kanban_protocol::MaintenanceRunResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-rebuild-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceRebuildResponse::try_from(dto).unwrap();
    let wire = v1::MaintenanceRebuildResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::MaintenanceRunResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_cleanup_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::MaintenanceRunRequest =
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/fixtures/api/maintenance-cleanup-request.v1.valid.json"
        )))
        .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceCleanupRequest::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceCleanupRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_cleanup_response_fixture_roundtrip() {
    let dto: kanban_protocol::MaintenanceRunResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-cleanup-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceCleanupResponse::try_from(dto).unwrap();
    let wire = v1::MaintenanceCleanupResponse::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::MaintenanceRunResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_import_v30_request_fixture_roundtrip() {
    let path = ();
    let query = ();
    let input: kanban_protocol::LegacyImportRequest = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-import-v30-request.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value((&path, &query, &input)).unwrap();
    let wire = v1::MaintenanceImportV30Request::from_parts(path, query, input).unwrap();
    let wire = v1::MaintenanceImportV30Request::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual = wire.decode_parts().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
#[test]
fn maintenance_import_v30_response_fixture_roundtrip() {
    let dto: kanban_protocol::LegacyImportResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/maintenance-import-v30-response.v1.valid.json"
    )))
    .unwrap();
    let expected = serde_json::to_value(&dto).unwrap();
    let wire = v1::MaintenanceImportV30Response::try_from(dto).unwrap();
    let wire = v1::MaintenanceImportV30Response::decode(wire.encode_to_vec().as_slice()).unwrap();
    let actual: kanban_protocol::LegacyImportResponse = wire.try_into().unwrap();
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
}
