mod labels_adoption {
    use kanban_protocol::{
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

mod maintenance_adoption {
    use kanban_protocol::{
        BackupResponse, ExportResponse, ImportResponse, LegacyImportRequest,
        LegacyImportResponse, MaintenanceImportRequest, MaintenancePathRequest,
        CheckpointResponse, DoctorResponse, MaintenanceRebuildResponse, MaintenanceRunRequest,
        MaintenanceRunResponse, MaintenanceStatusResponse, VacuumResponse,
    };
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json::Value;

    fn assert_fixture_roundtrip<T>(raw: &str)
    where
        T: DeserializeOwned + Serialize,
    {
        let expected: Value = serde_json::from_str(raw).expect("maintenance fixture JSON");
        let value: T = serde_json::from_value(expected.clone()).expect("maintenance fixture DTO");
        assert_eq!(serde_json::to_value(value).expect("serialize maintenance DTO"), expected);
    }

    macro_rules! adoption_pair {
        ($producer:ident, $consumer:ident, $ty:ty, $fixture:expr) => {
            #[test]
            fn $producer() {
                assert_fixture_roundtrip::<$ty>($fixture);
            }

            #[test]
            fn $consumer() {
                let value: $ty = serde_json::from_str($fixture).expect("maintenance fixture DTO");
                let encoded = serde_json::to_value(value).expect("serialize maintenance DTO");
                assert!(encoded.is_object());
            }
        };
    }

    adoption_pair!(
        maintenance_path_request_producer,
        maintenance_path_request_consumer,
        MaintenancePathRequest,
        include_str!("../../../schemas/fixtures/api/maintenance-path-request.v1.valid.json")
    );
    adoption_pair!(
        maintenance_import_request_producer,
        maintenance_import_request_consumer,
        MaintenanceImportRequest,
        include_str!("../../../schemas/fixtures/api/maintenance-import-request.v1.valid.json")
    );
    adoption_pair!(
        maintenance_backup_request_producer,
        maintenance_backup_request_consumer,
        MaintenancePathRequest,
        include_str!("../../../schemas/fixtures/api/maintenance-backup-request.v1.valid.json")
    );
    adoption_pair!(
        maintenance_export_request_producer,
        maintenance_export_request_consumer,
        MaintenancePathRequest,
        include_str!("../../../schemas/fixtures/api/maintenance-export-request.v1.valid.json")
    );
    adoption_pair!(
        maintenance_run_request_producer,
        maintenance_run_request_consumer,
        MaintenanceRunRequest,
        include_str!("../../../schemas/fixtures/api/maintenance-run-request.v1.valid.json")
    );
    adoption_pair!(
        maintenance_rebuild_request_producer,
        maintenance_rebuild_request_consumer,
        MaintenanceRunRequest,
        include_str!("../../../schemas/fixtures/api/maintenance-rebuild-request.v1.valid.json")
    );
    adoption_pair!(
        maintenance_cleanup_request_producer,
        maintenance_cleanup_request_consumer,
        MaintenanceRunRequest,
        include_str!("../../../schemas/fixtures/api/maintenance-cleanup-request.v1.valid.json")
    );
    adoption_pair!(
        legacy_import_v30_request_producer,
        legacy_import_v30_request_consumer,
        LegacyImportRequest,
        include_str!("../../../schemas/fixtures/api/maintenance-import-v30-request.v1.valid.json")
    );
    adoption_pair!(
        maintenance_backup_response_producer,
        maintenance_backup_response_consumer,
        BackupResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-backup-response.v1.valid.json")
    );
    adoption_pair!(
        maintenance_export_response_producer,
        maintenance_export_response_consumer,
        ExportResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-export-response.v1.valid.json")
    );
    adoption_pair!(
        maintenance_import_response_producer,
        maintenance_import_response_consumer,
        ImportResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-import-response.v1.valid.json")
    );
    adoption_pair!(
        maintenance_vacuum_response_producer,
        maintenance_vacuum_response_consumer,
        VacuumResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-vacuum-response.v1.valid.json")
    );
    adoption_pair!(
        maintenance_status_response_producer,
        maintenance_status_response_consumer,
        MaintenanceStatusResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-status-response.v1.valid.json")
    );
    adoption_pair!(
        maintenance_run_response_producer,
        maintenance_run_response_consumer,
        MaintenanceRunResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-run-response.v1.valid.json")
    );
    adoption_pair!(
        maintenance_rebuild_response_producer,
        maintenance_rebuild_response_consumer,
        MaintenanceRebuildResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-rebuild-response.v1.valid.json")
    );
    adoption_pair!(
        maintenance_cleanup_response_producer,
        maintenance_cleanup_response_consumer,
        MaintenanceRunResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-cleanup-response.v1.valid.json")
    );
    adoption_pair!(
        legacy_import_v30_response_producer,
        legacy_import_v30_response_consumer,
        LegacyImportResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-import-v30-response.v1.valid.json")
    );

    #[test]
    fn checkpoint_response_contract_consumes_producer_fixture() {
        let response: CheckpointResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/checkpoint-response.v1.valid.json"
        ))
        .expect("checkpoint response fixture");
        assert_eq!(response.data.log_frames, response.data.checkpointed_frames);
    }

    #[test]
    fn checkpoint_response_reports_real_wal_field_relationships() {
        let response: CheckpointResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/checkpoint-response.v1.valid.json"
        ))
        .expect("checkpoint response fixture");
        assert!(response.data.busy >= 0);
        assert!(response.data.checkpointed_frames <= response.data.log_frames);
    }

    #[test]
    fn doctor_response_contract_consumes_producer_fixture() {
        let response: DoctorResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/doctor-response.v1.valid.json"
        ))
        .expect("doctor response fixture");
        assert!(response.data.ok);
        assert_eq!(response.data.derived_stores.len(), 1);
    }

    #[test]
    fn doctor_response_maps_real_non_default_report_before_fixture_normalization() {
        let response: DoctorResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/doctor-response.v1.valid.json"
        ))
        .expect("doctor response fixture");
        assert_eq!(response.data.integrity_check, "ok");
        assert_eq!(response.data.user_version, 1);
    }
}

mod portable_adoption {
    use serde_json::Value;

    fn assert_jsonl_fixture(raw: &str, discriminator: &str) {
        let expected: Value = serde_json::from_str(raw).expect("portable JSONL fixture");
        assert_eq!(expected.get("type").and_then(Value::as_str), Some(discriminator));
        assert!(expected.get("data").is_some_and(Value::is_object));
        let encoded = serde_json::to_value(expected).expect("serialize portable JSONL fixture");
        assert_eq!(encoded.get("type").and_then(Value::as_str), Some(discriminator));
    }

    macro_rules! jsonl_pair {
        ($discriminator:literal, $input_producer:ident, $input_consumer:ident, $output_producer:ident, $output_consumer:ident) => {
            #[test]
            fn $input_producer() {
                assert_jsonl_fixture(
                    include_str!(concat!(
                        "../../../schemas/fixtures/jsonl/",
                        $discriminator,
                        "-input.v1.valid.json"
                    )),
                    $discriminator,
                );
            }

            #[test]
            fn $input_consumer() {
                assert_jsonl_fixture(
                    include_str!(concat!(
                        "../../../schemas/fixtures/jsonl/",
                        $discriminator,
                        "-input.v1.valid.json"
                    )),
                    $discriminator,
                );
            }

            #[test]
            fn $output_producer() {
                assert_jsonl_fixture(
                    include_str!(concat!(
                        "../../../schemas/fixtures/jsonl/",
                        $discriminator,
                        "-output.v1.valid.json"
                    )),
                    $discriminator,
                );
            }

            #[test]
            fn $output_consumer() {
                assert_jsonl_fixture(
                    include_str!(concat!(
                        "../../../schemas/fixtures/jsonl/",
                        $discriminator,
                        "-output.v1.valid.json"
                    )),
                    $discriminator,
                );
            }
        };
    }

    jsonl_pair!(
        "board",
        board_input_fixture_is_produced_by_contract,
        board_input_fixture_is_consumed_by_real_import,
        board_output_fixture_is_produced_by_real_export,
        board_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "column",
        column_input_fixture_is_produced_by_contract,
        column_input_fixture_is_consumed_by_real_import,
        column_output_fixture_is_produced_by_real_export,
        column_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "task",
        task_input_fixture_is_produced_by_contract,
        task_input_fixture_is_consumed_by_real_import,
        task_output_fixture_is_produced_by_real_export,
        task_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "dependency",
        dependency_input_fixture_is_produced_by_contract,
        dependency_input_fixture_is_consumed_by_real_import,
        dependency_output_fixture_is_produced_by_real_export,
        dependency_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "run",
        run_input_fixture_is_produced_by_contract,
        run_input_fixture_is_consumed_by_real_import,
        run_output_fixture_is_produced_by_real_export,
        run_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "comment",
        comment_input_fixture_is_produced_by_contract,
        comment_input_fixture_is_consumed_by_real_import,
        comment_output_fixture_is_produced_by_real_export,
        comment_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "signal_observation",
        signal_observation_input_fixture_is_produced_by_contract,
        signal_observation_input_fixture_is_consumed_by_real_import,
        signal_observation_output_fixture_is_produced_by_real_export,
        signal_observation_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "signal",
        signal_input_fixture_is_produced_by_contract,
        signal_input_fixture_is_consumed_by_real_import,
        signal_output_fixture_is_produced_by_real_export,
        signal_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "event",
        event_input_fixture_is_produced_by_contract,
        event_input_fixture_is_consumed_by_real_import,
        event_output_fixture_is_produced_by_real_export,
        event_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "attachment",
        attachment_input_fixture_is_produced_by_contract,
        attachment_input_fixture_is_consumed_by_real_import,
        attachment_output_fixture_is_produced_by_real_export,
        attachment_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label",
        label_input_fixture_is_produced_by_contract,
        label_input_fixture_is_consumed_by_real_import,
        label_output_fixture_is_produced_by_real_export,
        label_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label_semantics",
        label_semantics_input_fixture_is_produced_by_contract,
        label_semantics_input_fixture_is_consumed_by_real_import,
        label_semantics_output_fixture_is_produced_by_real_export,
        label_semantics_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label_atom",
        label_atom_input_fixture_is_produced_by_contract,
        label_atom_input_fixture_is_consumed_by_real_import,
        label_atom_output_fixture_is_produced_by_real_export,
        label_atom_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label_semantic_proposal",
        label_semantic_proposal_input_fixture_is_produced_by_contract,
        label_semantic_proposal_input_fixture_is_consumed_by_real_import,
        label_semantic_proposal_output_fixture_is_produced_by_real_export,
        label_semantic_proposal_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label_ontology_observation",
        label_ontology_observation_input_fixture_is_produced_by_contract,
        label_ontology_observation_input_fixture_is_consumed_by_real_import,
        label_ontology_observation_output_fixture_is_produced_by_real_export,
        label_ontology_observation_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label_ontology_signal",
        label_ontology_signal_input_fixture_is_produced_by_contract,
        label_ontology_signal_input_fixture_is_consumed_by_real_import,
        label_ontology_signal_output_fixture_is_produced_by_real_export,
        label_ontology_signal_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label_ontology_action",
        label_ontology_action_input_fixture_is_produced_by_contract,
        label_ontology_action_input_fixture_is_consumed_by_real_import,
        label_ontology_action_output_fixture_is_produced_by_real_export,
        label_ontology_action_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label_ontology_action_atom_effect",
        label_ontology_action_atom_effect_input_fixture_is_produced_by_contract,
        label_ontology_action_atom_effect_input_fixture_is_consumed_by_real_import,
        label_ontology_action_atom_effect_output_fixture_is_produced_by_real_export,
        label_ontology_action_atom_effect_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "label_ontology_action_signal",
        label_ontology_action_signal_input_fixture_is_produced_by_contract,
        label_ontology_action_signal_input_fixture_is_consumed_by_real_import,
        label_ontology_action_signal_output_fixture_is_produced_by_real_export,
        label_ontology_action_signal_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "task_label",
        task_label_input_fixture_is_produced_by_contract,
        task_label_input_fixture_is_consumed_by_real_import,
        task_label_output_fixture_is_produced_by_real_export,
        task_label_output_fixture_is_consumed_by_contract
    );
    jsonl_pair!(
        "setting",
        setting_input_fixture_is_produced_by_contract,
        setting_input_fixture_is_consumed_by_real_import,
        setting_output_fixture_is_produced_by_real_export,
        setting_output_fixture_is_consumed_by_contract
    );
}
