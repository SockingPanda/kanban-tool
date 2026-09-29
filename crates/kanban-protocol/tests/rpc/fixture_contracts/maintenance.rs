//! DTO 格式契约与真实数据库、Host 和维护流程无关。
use kanban_protocol::{
    BackupResponse, CheckpointResponse, DoctorResponse, ExportResponse, ImportResponse,
    LegacyImportRequest, LegacyImportResponse, MaintenanceImportRequest, MaintenancePathRequest,
    MaintenanceRebuildResponse, MaintenanceRunRequest, MaintenanceRunResponse,
    MaintenanceStatusResponse, VacuumResponse,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

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
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../schemas/fixtures/api/",
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
    let checkpoint: CheckpointResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/checkpoint-response.v1.valid.json"
    )))
    .expect("checkpoint response fixture");
    assert_eq!(
        checkpoint.data.log_frames,
        checkpoint.data.checkpointed_frames
    );
    assert!(checkpoint.data.busy >= 0);
    assert!(checkpoint.data.checkpointed_frames <= checkpoint.data.log_frames);

    let doctor: DoctorResponse = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/api/doctor-response.v1.valid.json"
    )))
    .expect("doctor response fixture");
    assert!(doctor.data.ok);
    assert_eq!(doctor.data.derived_stores.len(), 1);
    assert_eq!(doctor.data.integrity_check, "ok");
    assert_eq!(doctor.data.user_version, 1);
}
