mod labels_adoption {
    use kanban_protocol::{
        AddTaskLabelPath, AddTaskLabelRequest, AddTaskLabelResponse, BoardLabelPath,
        CreateBoardLabelRequest, CreateBoardLabelResponse, DeleteBoardLabelPath,
        DeleteBoardLabelQuery, DeleteBoardLabelResponse, ListBoardLabelsResponse,
        ListTaskLabelsPath, ListTaskLabelsResponse, RemoveTaskLabelPath, RemoveTaskLabelResponse,
    };
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json::Value;

    // 格式契约保留独立字段预期；不把静态 fixture 反序列化称为真实 router 证据。
    fn check_fixture<T>(
        name: &str,
        raw: &str,
        expected_fields: fn(&T) -> bool,
        expectation: &str,
    ) -> Result<(), String>
    where
        T: DeserializeOwned + Serialize,
    {
        let expected: Value =
            serde_json::from_str(raw).map_err(|error| format!("{name}: fixture JSON: {error}"))?;
        let value: T =
            serde_json::from_str(raw).map_err(|error| format!("{name}: DTO 解码: {error}"))?;
        if !expected_fields(&value) {
            return Err(format!("{name}: 字段预期不成立: {expectation}"));
        }
        let actual =
            serde_json::to_value(value).map_err(|error| format!("{name}: DTO 编码: {error}"))?;
        if actual != expected {
            return Err(format!(
                "{name}: 往返不一致\nexpected={expected}\nactual={actual}"
            ));
        }
        Ok(())
    }

    #[test]
    fn label_fixture_contracts() {
        macro_rules! check {
            ($ty:ty, $name:literal, $expected:expr) => {
                check_fixture::<$ty>(
                    $name,
                    include_str!(concat!(
                        "../../../schemas/fixtures/api/",
                        $name,
                        ".v1.valid.json"
                    )),
                    $expected,
                    stringify!($expected),
                )
            };
        }

        let results = [
            check!(BoardLabelPath, "list-board-labels-path", |value| {
                value.board == "fixture"
            }),
            check!(
                ListBoardLabelsResponse,
                "list-board-labels-response",
                |value| { value.data.is_empty() }
            ),
            check!(BoardLabelPath, "create-board-label-path", |value| {
                value.board == "fixture"
            }),
            check!(
                CreateBoardLabelRequest,
                "create-board-label-request",
                |value| { value.name == "fixture" }
            ),
            check!(
                CreateBoardLabelResponse,
                "create-board-label-response",
                |value| { value.data.name == "fixture" }
            ),
            check!(DeleteBoardLabelPath, "delete-board-label-path", |value| {
                value.board == "fixture" && value.label_id == "l_fixture"
            }),
            check!(DeleteBoardLabelQuery, "delete-board-label-query", |value| {
                !value.force
            }),
            check!(
                DeleteBoardLabelResponse,
                "delete-board-label-response",
                |value| {
                    value.data.label.name == "fixture"
                        && value.data.forced
                        && value.data.removed_task_bindings == 1
                }
            ),
            check!(ListTaskLabelsPath, "list-task-labels-path", |value| {
                value.task_id == "t_fixture"
            }),
            check!(
                ListTaskLabelsResponse,
                "list-task-labels-response",
                |value| {
                    value
                        .data
                        .first()
                        .is_some_and(|label| label.name == "后端-api")
                }
            ),
            check!(AddTaskLabelPath, "add-task-label-path", |value| {
                value.task_id == "t_fixture"
            }),
            check!(AddTaskLabelRequest, "add-task-label-request", |value| {
                value
                    .label_names()
                    .is_ok_and(|names| names == vec!["后端-api"])
            }),
            check!(AddTaskLabelResponse, "add-task-label-response", |value| {
                value.data.labels.len() == 1
                    && value
                        .meta
                        .as_ref()
                        .is_some_and(|meta| meta.created_labels.len() == 1)
            }),
            check!(RemoveTaskLabelPath, "remove-task-label-path", |value| {
                value.label_id == "l_fixture"
            }),
            check!(
                RemoveTaskLabelResponse,
                "remove-task-label-response",
                |value| { value.data.labels.is_empty() }
            ),
        ];
        let failures = results
            .into_iter()
            .filter_map(Result::err)
            .collect::<Vec<_>>();
        assert!(
            failures.is_empty(),
            "label 格式契约失败:\n{}",
            failures.join("\n")
        );
    }

    #[tokio::test]
    async fn delete_board_label_router_returns_removal_report() {
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
    use tower::ServiceExt;

    use crate::{AppState, build_router};

    // 只验证 RPC 边界的真实往返；静态格式契约不再触发此流程。
    #[tokio::test]
    async fn maintenance_rpc_exercises_real_router() {
        run_rpc_flow().await.expect("maintenance RPC flow");
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

    // 格式契约没有数据库前置条件。每个失败都携带 fixture 名称。
    fn fixture_roundtrip<T>(name: &str, raw: &str) -> Result<(), String>
    where
        T: DeserializeOwned + Serialize,
    {
        let expected: Value =
            serde_json::from_str(raw).map_err(|error| format!("{name}: fixture JSON: {error}"))?;
        let value: T =
            serde_json::from_str(raw).map_err(|error| format!("{name}: DTO 解码: {error}"))?;
        let actual =
            serde_json::to_value(value).map_err(|error| format!("{name}: DTO 编码: {error}"))?;
        if !actual.is_object() {
            return Err(format!("{name}: 编码结果不是对象"));
        }
        if actual != expected {
            return Err(format!(
                "{name}: 往返不一致\nexpected={expected}\nactual={actual}"
            ));
        }
        Ok(())
    }

    #[test]
    fn maintenance_fixture_contracts() {
        macro_rules! check {
            ($ty:ty, $name:literal) => {
                fixture_roundtrip::<$ty>(
                    $name,
                    include_str!(concat!(
                        "../../../schemas/fixtures/api/",
                        $name,
                        ".v1.valid.json"
                    )),
                )
            };
        }

        // 收集所有格式失败，避免合并后只能看到第一个不兼容的 fixture。
        let results = [
            check!(MaintenancePathRequest, "maintenance-path-request"),
            check!(MaintenanceImportRequest, "maintenance-import-request"),
            check!(MaintenancePathRequest, "maintenance-backup-request"),
            check!(MaintenancePathRequest, "maintenance-export-request"),
            check!(MaintenanceRunRequest, "maintenance-run-request"),
            check!(MaintenanceRunRequest, "maintenance-rebuild-request"),
            check!(MaintenanceRunRequest, "maintenance-cleanup-request"),
            check!(LegacyImportRequest, "maintenance-import-v30-request"),
            check!(BackupResponse, "maintenance-backup-response"),
            check!(ExportResponse, "maintenance-export-response"),
            check!(ImportResponse, "maintenance-import-response"),
            check!(VacuumResponse, "maintenance-vacuum-response"),
            check!(MaintenanceStatusResponse, "maintenance-status-response"),
            check!(MaintenanceRunResponse, "maintenance-run-response"),
            check!(MaintenanceRebuildResponse, "maintenance-rebuild-response"),
            check!(MaintenanceRunResponse, "maintenance-cleanup-response"),
            check!(LegacyImportResponse, "maintenance-import-v30-response"),
            check!(CheckpointResponse, "checkpoint-response"),
            check!(DoctorResponse, "doctor-response"),
        ];
        let failures = results
            .into_iter()
            .filter_map(Result::err)
            .collect::<Vec<_>>();
        assert!(
            failures.is_empty(),
            "maintenance 格式契约失败:\n{}",
            failures.join("\n")
        );

        // 保留原有 fixture 语义断言；不将静态示例称作真实 WAL 或 doctor 证据。
        let checkpoint: CheckpointResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/checkpoint-response.v1.valid.json"
        ))
        .expect("checkpoint response fixture");
        assert_eq!(
            checkpoint.data.log_frames,
            checkpoint.data.checkpointed_frames
        );
        assert!(checkpoint.data.busy >= 0);
        assert!(checkpoint.data.checkpointed_frames <= checkpoint.data.log_frames);

        let doctor: DoctorResponse = serde_json::from_str(include_str!(
            "../../../schemas/fixtures/api/doctor-response.v1.valid.json"
        ))
        .expect("doctor response fixture");
        assert!(doctor.data.ok);
        assert_eq!(doctor.data.derived_stores.len(), 1);
        assert_eq!(doctor.data.integrity_check, "ok");
        assert_eq!(doctor.data.user_version, 1);
    }
}

mod legacy_adoption;
