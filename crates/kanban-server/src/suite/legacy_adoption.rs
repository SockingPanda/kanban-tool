#[cfg(feature = "legacy-sqlite-import")]
use std::fs;

use crate::test_support::{parts, rpc_request, wire_response};
use kanban_protocol::rpc::v1 as pb;
#[cfg(feature = "legacy-sqlite-import")]
use serde_json::Value;
use std::collections::BTreeMap;
use tokio::sync::OnceCell;
use tower::ServiceExt;

use crate::{AppState, build_router};

static LEGACY_RPC_FLOW: OnceCell<()> = OnceCell::const_new();

pub(crate) async fn ensure_legacy_rpc_flow() {
    LEGACY_RPC_FLOW
        .get_or_init(|| async {
            run_legacy_rpc_flow()
                .await
                .expect("legacy SQLite v30 RPC flow");
        })
        .await;
}

async fn run_legacy_rpc_flow() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let target_path = directory.path().join("legacy-target.db");
    let target = AppState::open(&target_path, "legacy-adoption")
        .await
        .map_err(|error| error.to_string())?;
    let router = build_router(target.clone());

    #[cfg(feature = "legacy-sqlite-import")]
    {
        let source_path =
            kanban_service::adoption_test_support::make_legacy_source(directory.path())?;
        let attachment_root = directory.path().join("canonical-attachments");
        let response = router
            .oneshot(rpc_request(
                "MaintenanceImportV30",
                pb::MaintenanceImportV30Request::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({
                        "path": source_path,
                        "canonical_attachment_root": attachment_root,
                    })),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        let report: kanban_protocol::LegacyImportResponse =
            wire_response::<pb::MaintenanceImportV30Response>(response)
                .await
                .unwrap()
                .try_into()
                .unwrap();
        let body = serde_json::to_value(report).unwrap();
        assert_eq!(body["data"]["phase"], "completed");
        assert_eq!(body["data"]["resumed"], false);
        assert_eq!(body["data"]["attachment_count"], 1);
        assert_eq!(table_count(&body, "boards"), 1);
        assert_eq!(table_count(&body, "tasks"), 2);
        assert_eq!(table_count(&body, "task_dependencies"), 1);
        assert_eq!(table_count(&body, "task_attachments"), 1);
        let published = attachment_root.join("attachments/legacy.txt");
        assert_eq!(
            fs::read(&published).map_err(|error| error.to_string())?,
            b"legacy\n"
        );
        assert_target_facts(&target).await?;
    }

    #[cfg(not(feature = "legacy-sqlite-import"))]
    {
        let response = router
            .oneshot(rpc_request(
                "MaintenanceImportV30",
                pb::MaintenanceImportV30Request::from_parts(
                    (),
                    (),
                    parts(serde_json::json!({"path":"/tmp/legacy-v30.sqlite"})),
                )
                .unwrap(),
                &BTreeMap::new(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(
            wire_response::<pb::MaintenanceImportV30Response>(response)
                .await
                .unwrap_err()
                .code(),
            tonic::Code::Unimplemented
        );
    }
    Ok(())
}

#[cfg(feature = "legacy-sqlite-import")]
fn table_count(body: &Value, table: &str) -> u64 {
    body["data"]["table_counts"]
        .as_array()
        .expect("legacy table counts")
        .iter()
        .find(|count| count["table"] == table)
        .and_then(|count| count["source_rows"].as_u64())
        .expect("legacy table count entry")
}

#[cfg(feature = "legacy-sqlite-import")]
async fn assert_target_facts(target: &AppState) -> Result<(), String> {
    let export_path = target
        .db_path()
        .parent()
        .ok_or("target database has no parent")?
        .join("legacy-target.jsonl");
    target
        .application()
        .export(
            export_path
                .to_str()
                .ok_or("target export path is not UTF-8")?,
        )
        .await
        .map_err(|error| error.to_string())?;
    kanban_service::adoption_test_support::assert_legacy_target_facts(&export_path)
}
