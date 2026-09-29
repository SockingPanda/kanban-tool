//! Server 维护操作的真实 router 契约；故障矩阵由 service owner 持有。
use crate::test_support::{decode_response, parts, rpc_request};
use crate::{AppState, build_router};
use axum::http::StatusCode;
use kanban_protocol::rpc::v1 as pb;
use kanban_protocol::{
    BackupResponse, CheckpointResponse, DoctorResponse, ExportResponse, ImportResponse,
    MaintenanceRebuildResponse, MaintenanceRunResponse, MaintenanceStatusResponse, VacuumResponse,
};
use std::collections::BTreeMap;
use tower::ServiceExt;

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
    let backup: BackupResponse = decode_response::<pb::MaintenanceBackupResponse, _>(backup).await;
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
    let export: ExportResponse = decode_response::<pb::MaintenanceExportResponse, _>(export).await;
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
    let run: MaintenanceRunResponse = decode_response::<pb::MaintenanceRunResponse, _>(run).await;
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
    let vacuum: VacuumResponse = decode_response::<pb::MaintenanceVacuumResponse, _>(vacuum).await;
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
    let import: ImportResponse = decode_response::<pb::MaintenanceImportResponse, _>(import).await;
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
