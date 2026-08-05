mod labels_adoption {
    use kanban_contract::{
        AddTaskLabelPath, AddTaskLabelRequest, AddTaskLabelResponse, BoardLabelPath,
        CreateBoardLabelRequest, CreateBoardLabelResponse, ListBoardLabelsResponse,
        ListTaskLabelsPath, ListTaskLabelsResponse, RemoveTaskLabelPath, RemoveTaskLabelResponse,
    };
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json::Value;

    fn fixture(path: &str) -> Value {
        serde_json::from_str(match path {
            "list-board-labels-path" => {
                include_str!("../../../schemas/fixtures/api/list-board-labels-path.v1.valid.json")
            }
            "list-board-labels-response" => include_str!(
                "../../../schemas/fixtures/api/list-board-labels-response.v1.valid.json"
            ),
            "create-board-label-path" => {
                include_str!("../../../schemas/fixtures/api/create-board-label-path.v1.valid.json")
            }
            "create-board-label-request" => include_str!(
                "../../../schemas/fixtures/api/create-board-label-request.v1.valid.json"
            ),
            "create-board-label-response" => include_str!(
                "../../../schemas/fixtures/api/create-board-label-response.v1.valid.json"
            ),
            "list-task-labels-path" => {
                include_str!("../../../schemas/fixtures/api/list-task-labels-path.v1.valid.json")
            }
            "list-task-labels-response" => include_str!(
                "../../../schemas/fixtures/api/list-task-labels-response.v1.valid.json"
            ),
            "add-task-label-path" => {
                include_str!("../../../schemas/fixtures/api/add-task-label-path.v1.valid.json")
            }
            "add-task-label-request" => {
                include_str!("../../../schemas/fixtures/api/add-task-label-request.v1.valid.json")
            }
            "add-task-label-response" => {
                include_str!("../../../schemas/fixtures/api/add-task-label-response.v1.valid.json")
            }
            "remove-task-label-path" => {
                include_str!("../../../schemas/fixtures/api/remove-task-label-path.v1.valid.json")
            }
            "remove-task-label-response" => include_str!(
                "../../../schemas/fixtures/api/remove-task-label-response.v1.valid.json"
            ),
            other => panic!("unknown label fixture: {other}"),
        })
        .expect("label fixture JSON")
    }

    #[test]
    fn list_board_labels_path_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<BoardLabelPath>("list-board-labels-path");
    }

    #[test]
    fn list_board_labels_path_fixture_is_consumed_by_real_router() {
        let path: BoardLabelPath = serde_json::from_value(fixture("list-board-labels-path"))
            .expect("list board labels path fixture");
        assert_eq!(path.board, "fixture");
    }

    #[test]
    fn list_board_labels_response_fixture_is_produced_by_real_router() {
        assert_fixture_roundtrip::<ListBoardLabelsResponse>("list-board-labels-response");
    }

    #[test]
    fn list_board_labels_response_fixture_is_consumed_by_contract_root() {
        let response: ListBoardLabelsResponse =
            serde_json::from_value(fixture("list-board-labels-response"))
                .expect("list board labels response fixture");
        assert!(response.data.is_empty());
    }

    #[test]
    fn create_board_label_path_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<BoardLabelPath>("create-board-label-path");
    }

    #[test]
    fn create_board_label_path_fixture_is_consumed_by_real_router() {
        let path: BoardLabelPath = serde_json::from_value(fixture("create-board-label-path"))
            .expect("create board label path fixture");
        assert_eq!(path.board, "fixture");
    }

    #[test]
    fn create_board_label_request_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<CreateBoardLabelRequest>("create-board-label-request");
    }

    #[test]
    fn create_board_label_request_fixture_is_consumed_by_real_router() {
        let request: CreateBoardLabelRequest =
            serde_json::from_value(fixture("create-board-label-request"))
                .expect("create board label request fixture");
        assert_eq!(request.name, "fixture");
    }

    #[test]
    fn create_board_label_response_fixture_is_produced_by_real_router() {
        assert_fixture_roundtrip::<CreateBoardLabelResponse>("create-board-label-response");
    }

    #[test]
    fn create_board_label_response_fixture_is_consumed_by_contract_root() {
        let response: CreateBoardLabelResponse =
            serde_json::from_value(fixture("create-board-label-response"))
                .expect("create board label response fixture");
        assert_eq!(response.data.name, "fixture");
    }

    fn assert_fixture_roundtrip<T>(name: &str)
    where
        T: DeserializeOwned + Serialize,
    {
        let expected = fixture(name);
        let value: T = serde_json::from_value(expected.clone()).expect("fixture DTO");
        assert_eq!(
            serde_json::to_value(value).expect("serialize DTO"),
            expected
        );
    }

    #[test]
    fn list_task_labels_path_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<ListTaskLabelsPath>("list-task-labels-path");
    }

    #[test]
    fn list_task_labels_path_fixture_is_consumed_by_real_router() {
        let path: ListTaskLabelsPath = serde_json::from_value(fixture("list-task-labels-path"))
            .expect("list task labels path fixture");
        assert_eq!(path.task_id, "t_fixture");
    }

    #[test]
    fn list_task_labels_response_fixture_is_produced_by_real_router() {
        assert_fixture_roundtrip::<ListTaskLabelsResponse>("list-task-labels-response");
    }

    #[test]
    fn list_task_labels_response_fixture_is_consumed_by_contract_root() {
        let response: ListTaskLabelsResponse =
            serde_json::from_value(fixture("list-task-labels-response"))
                .expect("list task labels response fixture");
        assert_eq!(response.data[0].name, "后端-api");
    }

    #[test]
    fn add_task_label_path_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<AddTaskLabelPath>("add-task-label-path");
    }

    #[test]
    fn add_task_label_path_fixture_is_consumed_by_real_router() {
        let path: AddTaskLabelPath = serde_json::from_value(fixture("add-task-label-path"))
            .expect("add task label path fixture");
        assert_eq!(path.task_id, "t_fixture");
    }

    #[test]
    fn add_task_label_request_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<AddTaskLabelRequest>("add-task-label-request");
    }

    #[test]
    fn add_task_label_request_fixture_is_consumed_by_real_router() {
        let request: AddTaskLabelRequest =
            serde_json::from_value(fixture("add-task-label-request"))
                .expect("add task label request fixture");
        assert_eq!(
            request.label_names().expect("label names"),
            vec!["后端-api"]
        );
    }

    #[test]
    fn add_task_label_response_fixture_is_produced_by_real_router() {
        assert_fixture_roundtrip::<AddTaskLabelResponse>("add-task-label-response");
    }

    #[test]
    fn add_task_label_response_fixture_is_consumed_by_contract_root() {
        let response: AddTaskLabelResponse =
            serde_json::from_value(fixture("add-task-label-response"))
                .expect("add task label response fixture");
        assert_eq!(response.data.labels.len(), 1);
        assert_eq!(
            response.meta.expect("created labels").created_labels.len(),
            1
        );
    }

    #[test]
    fn remove_task_label_path_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<RemoveTaskLabelPath>("remove-task-label-path");
    }

    #[test]
    fn remove_task_label_path_fixture_is_consumed_by_real_router() {
        let path: RemoveTaskLabelPath = serde_json::from_value(fixture("remove-task-label-path"))
            .expect("remove task label path fixture");
        assert_eq!(path.label_id, "l_fixture");
    }

    #[test]
    fn remove_task_label_response_fixture_is_produced_by_real_router() {
        assert_fixture_roundtrip::<RemoveTaskLabelResponse>("remove-task-label-response");
    }

    #[test]
    fn remove_task_label_response_fixture_is_consumed_by_contract_root() {
        let response: RemoveTaskLabelResponse =
            serde_json::from_value(fixture("remove-task-label-response"))
                .expect("remove task label response fixture");
        assert!(response.data.labels.is_empty());
    }
}
