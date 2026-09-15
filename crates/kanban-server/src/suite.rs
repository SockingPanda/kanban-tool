mod labels_adoption {
    use kanban_protocol::{
        AddTaskLabelPath, AddTaskLabelRequest, AddTaskLabelResponse, BoardLabelPath,
        CreateBoardLabelRequest, CreateBoardLabelResponse, DeleteBoardLabelPath,
        DeleteBoardLabelQuery, DeleteBoardLabelResponse, ListBoardLabelsResponse,
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
            "delete-board-label-path" => {
                include_str!("../../../schemas/fixtures/api/delete-board-label-path.v1.valid.json")
            }
            "delete-board-label-query" => {
                include_str!("../../../schemas/fixtures/api/delete-board-label-query.v1.valid.json")
            }
            "delete-board-label-response" => include_str!(
                "../../../schemas/fixtures/api/delete-board-label-response.v1.valid.json"
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

    #[test]
    fn delete_board_label_path_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<DeleteBoardLabelPath>("delete-board-label-path");
    }

    #[test]
    fn delete_board_label_path_fixture_is_consumed_by_real_router() {
        let path: DeleteBoardLabelPath = serde_json::from_value(fixture("delete-board-label-path"))
            .expect("delete board label path fixture");
        assert_eq!(path.board, "fixture");
        assert_eq!(path.label_id, "l_fixture");
    }

    #[test]
    fn delete_board_label_query_dto_serializes_to_committed_fixture() {
        assert_fixture_roundtrip::<DeleteBoardLabelQuery>("delete-board-label-query");
    }

    #[test]
    fn delete_board_label_query_fixture_is_consumed_by_real_router() {
        let query: DeleteBoardLabelQuery =
            serde_json::from_value(fixture("delete-board-label-query"))
                .expect("delete board label query fixture");
        assert!(!query.force);
    }

    #[tokio::test]
    async fn delete_board_label_response_fixture_is_produced_by_real_router() {
        use crate::test_support::{decode_response, parts, rpc_request};
        use kanban_protocol::rpc::v1 as pb;
        use std::collections::BTreeMap;
        use tower::ServiceExt;
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = crate::AppState::open(directory.path().join("kanban.db"), "adoption")
            .await
            .unwrap();
        let router = crate::build_router(state);
        let create = router
            .clone()
            .oneshot(rpc_request(
                "CreateBoardLabel",
                pb::CreateBoardLabelRequest::from_parts(
                    parts(serde_json::json!({"board":"default"})),
                    (),
                    parts(serde_json::json!({"name":"fixture-delete","color":null})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .unwrap();
        let created: CreateBoardLabelResponse =
            decode_response::<pb::CreateBoardLabelResponse, _>(create).await;
        let response = router
            .oneshot(rpc_request(
                "DeleteBoardLabel",
                pb::DeleteBoardLabelRequest::from_parts(
                    parts(serde_json::json!({"board":"default","label_id":created.data.id})),
                    DeleteBoardLabelQuery { force: false },
                    (),
                )
                .unwrap(),
                &BTreeMap::from([("x-kb-actor".into(), "adoption".into())]),
            ))
            .await
            .unwrap();
        let deleted: DeleteBoardLabelResponse =
            decode_response::<pb::DeleteBoardLabelResponse, _>(response).await;
        assert_eq!(deleted.data.label.name, "fixture-delete");
        assert!(!deleted.data.forced);
        assert_eq!(deleted.data.removed_task_bindings, 0);
        assert!(!deleted.data.removed_semantics);
        assert_eq!(deleted.data.removed_atoms, 0);
    }

    #[test]
    fn delete_board_label_response_fixture_is_consumed_by_contract_root() {
        let response: DeleteBoardLabelResponse =
            serde_json::from_value(fixture("delete-board-label-response"))
                .expect("delete board label response fixture");
        assert_eq!(response.data.label.name, "fixture");
        assert!(response.data.forced);
        assert_eq!(response.data.removed_task_bindings, 1);
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

mod portable_adoption;

mod maintenance_adoption {
    use crate::test_support::{decode_response, parts, rpc_request};
    use axum::http::StatusCode;
    use kanban_protocol::rpc::v1 as pb;
    use kanban_protocol::{
        BackupResponse, CheckpointResponse, DoctorResponse, ExportResponse, ImportResponse,
        LegacyImportRequest, LegacyImportResponse, MaintenanceImportRequest,
        MaintenancePathRequest, MaintenanceRebuildResponse, MaintenanceRunRequest,
        MaintenanceRunResponse, MaintenanceStatusResponse, VacuumResponse,
    };
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json::Value;
    use std::collections::BTreeMap;
    use tokio::sync::OnceCell;
    use tower::ServiceExt;

    use super::legacy_adoption::ensure_legacy_rpc_flow;
    use crate::{AppState, build_router};

    static RPC_FLOW: OnceCell<()> = OnceCell::const_new();

    async fn ensure_rpc_flow() {
        RPC_FLOW
            .get_or_init(|| async {
                run_rpc_flow().await.expect("maintenance RPC flow");
            })
            .await;
    }

    async fn run_rpc_flow() -> Result<(), String> {
        let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
        let source_path = directory.path().join("rpc-source.db");
        let source = AppState::open(&source_path, "adoption-owner")
            .await
            .map_err(|error| error.to_string())?;
        let router = build_router(source.clone());

        let doctor = router
            .clone()
            .oneshot(rpc_request(
                "Doctor",
                pb::DoctorRequest::from_parts((), (), ()).unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(doctor.status(), StatusCode::OK);
        let doctor: DoctorResponse = decode_response::<pb::DoctorResponse, _>(doctor).await;
        assert_eq!(doctor.data.integrity_check, "ok");

        let checkpoint = router
            .clone()
            .oneshot(rpc_request(
                "Checkpoint",
                pb::CheckpointRequest::from_parts((), (), ()).unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(checkpoint.status(), StatusCode::OK);
        let checkpoint: CheckpointResponse =
            decode_response::<pb::CheckpointResponse, _>(checkpoint).await;
        assert!(checkpoint.data.busy >= 0);
        assert!(checkpoint.data.checkpointed_frames <= checkpoint.data.log_frames);

        let backup_path = directory.path().join("rpc-backup.db");
        let backup = router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceBackup",
                pb::MaintenanceBackupRequest::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"path": backup_path})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(backup.status(), StatusCode::OK);
        let backup: BackupResponse =
            decode_response::<pb::MaintenanceBackupResponse, _>(backup).await;
        assert!(backup.data.bytes > 0);

        let export_path = directory.path().join("rpc-portable.jsonl");
        let export = router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceExport",
                pb::MaintenanceExportRequest::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"path": export_path})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(export.status(), StatusCode::OK);
        let export: ExportResponse =
            decode_response::<pb::MaintenanceExportResponse, _>(export).await;
        assert!(export.data.bytes > 0);

        let status = router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceStatus",
                pb::MaintenanceStatusRequest::from_parts((), (), ()).unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(status.status(), StatusCode::OK);
        let status: MaintenanceStatusResponse =
            decode_response::<pb::MaintenanceStatusResponse, _>(status).await;
        assert!(!status.data.owner.active);

        let run = router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceRun",
                pb::MaintenanceRunRequest::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"owner":"adoption-owner","action":"run"})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(run.status(), StatusCode::OK);
        let run: MaintenanceRunResponse =
            decode_response::<pb::MaintenanceRunResponse, _>(run).await;
        assert_eq!(run.data.action, "run");

        let rebuild = router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceRebuild",
                pb::MaintenanceRebuildRequest::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"owner":"adoption-owner","action":"rebuild"})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(rebuild.status(), StatusCode::OK);
        let rebuild: MaintenanceRebuildResponse =
            decode_response::<pb::MaintenanceRebuildResponse, _>(rebuild).await;
        assert_eq!(rebuild.data.action, "rebuild");

        let cleanup = router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceCleanup",
                pb::MaintenanceCleanupRequest::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"owner":"adoption-owner","action":"cleanup"})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(cleanup.status(), StatusCode::OK);
        let cleanup: MaintenanceRunResponse =
            decode_response::<pb::MaintenanceCleanupResponse, _>(cleanup).await;
        assert_eq!(cleanup.data.action, "cleanup");

        let compact = router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceRun",
                pb::MaintenanceRunRequest::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"owner":"adoption-owner","action":"compact"})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(compact.status(), StatusCode::OK);
        let compact: MaintenanceRunResponse =
            decode_response::<pb::MaintenanceRunResponse, _>(compact).await;
        assert_eq!(compact.data.action, "compact");

        let vacuum = router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceVacuum",
                pb::MaintenanceVacuumRequest::from_parts((), (), ()).unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(vacuum.status(), StatusCode::OK);
        let vacuum: VacuumResponse =
            decode_response::<pb::MaintenanceVacuumResponse, _>(vacuum).await;
        assert!(vacuum.data.ok);

        let target_path = directory.path().join("rpc-target.db");
        let target = AppState::open(&target_path, "adoption-owner")
            .await
            .map_err(|error| error.to_string())?;
        let target_router = build_router(target);
        let import = target_router
            .clone()
            .oneshot(rpc_request(
                "MaintenanceImport",
                pb::MaintenanceImportRequest::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"path": export_path, "replace": false})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(import.status(), StatusCode::OK);
        let import: ImportResponse =
            decode_response::<pb::MaintenanceImportResponse, _>(import).await;
        assert_eq!(import.data.phase, "completed");
        assert!(!import.data.restart_required);

        let replace_target_path = directory.path().join("rpc-replace-target.db");
        let replace_target = AppState::open(&replace_target_path, "adoption-owner")
            .await
            .map_err(|error| error.to_string())?;
        let replace_router = build_router(replace_target);
        let replace = replace_router
            .oneshot(rpc_request(
                "MaintenanceImport",
                pb::MaintenanceImportRequest::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"path": export_path, "replace": true})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(replace.status(), StatusCode::OK);
        let replace: ImportResponse =
            decode_response::<pb::MaintenanceImportResponse, _>(replace).await;
        assert_eq!(replace.data.phase, "completed");
        Ok(())
    }

    fn assert_fixture_roundtrip<T>(raw: &str)
    where
        T: DeserializeOwned + Serialize,
    {
        let expected: Value = serde_json::from_str(raw).expect("maintenance fixture JSON");
        let value: T = serde_json::from_value(expected.clone()).expect("maintenance fixture DTO");
        assert_eq!(
            serde_json::to_value(value).expect("serialize maintenance DTO"),
            expected
        );
    }

    macro_rules! adoption_pair {
        ($producer:ident, $consumer:ident, $ty:ty, $fixture:expr) => {
            #[tokio::test]
            async fn $producer() {
                ensure_rpc_flow().await;
                assert_fixture_roundtrip::<$ty>($fixture);
            }

            #[tokio::test]
            async fn $consumer() {
                ensure_rpc_flow().await;
                let value: $ty = serde_json::from_str($fixture).expect("maintenance fixture DTO");
                let encoded = serde_json::to_value(value).expect("serialize maintenance DTO");
                assert!(encoded.is_object());
            }
        };
    }

    macro_rules! legacy_adoption_pair {
        ($producer:ident, $consumer:ident, $ty:ty, $fixture:expr) => {
            #[tokio::test]
            async fn $producer() {
                ensure_rpc_flow().await;
                ensure_legacy_rpc_flow().await;
                assert_fixture_roundtrip::<$ty>($fixture);
            }

            #[tokio::test]
            async fn $consumer() {
                ensure_rpc_flow().await;
                ensure_legacy_rpc_flow().await;
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
    legacy_adoption_pair!(
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
    legacy_adoption_pair!(
        legacy_import_v30_response_producer,
        legacy_import_v30_response_consumer,
        LegacyImportResponse,
        include_str!("../../../schemas/fixtures/api/maintenance-import-v30-response.v1.valid.json")
    );

    #[tokio::test]
    async fn checkpoint_response_contract_consumes_producer_fixture() {
        ensure_rpc_flow().await;
        let response: CheckpointResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/checkpoint-response.v1.valid.json"
        ))
        .expect("checkpoint response fixture");
        assert_eq!(response.data.log_frames, response.data.checkpointed_frames);
    }

    #[tokio::test]
    async fn checkpoint_response_reports_real_wal_field_relationships() {
        ensure_rpc_flow().await;
        let response: CheckpointResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/checkpoint-response.v1.valid.json"
        ))
        .expect("checkpoint response fixture");
        assert!(response.data.busy >= 0);
        assert!(response.data.checkpointed_frames <= response.data.log_frames);
    }

    #[tokio::test]
    async fn doctor_response_contract_consumes_producer_fixture() {
        ensure_rpc_flow().await;
        let response: DoctorResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/doctor-response.v1.valid.json"
        ))
        .expect("doctor response fixture");
        assert!(response.data.ok);
        assert_eq!(response.data.derived_stores.len(), 1);
    }

    #[tokio::test]
    async fn doctor_response_maps_real_non_default_report_before_fixture_normalization() {
        ensure_rpc_flow().await;
        let response: DoctorResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/doctor-response.v1.valid.json"
        ))
        .expect("doctor response fixture");
        assert_eq!(response.data.integrity_check, "ok");
        assert_eq!(response.data.user_version, 1);
    }
}

mod legacy_adoption;
