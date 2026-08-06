#[cfg(feature = "legacy-sqlite-import")]
use std::fs;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tokio::sync::OnceCell;
use tower::ServiceExt;

use crate::{AppState, build_router};

static LEGACY_HTTP_FLOW: OnceCell<()> = OnceCell::const_new();

pub(crate) async fn ensure_legacy_http_flow() {
    LEGACY_HTTP_FLOW
        .get_or_init(|| async {
            run_legacy_http_flow()
                .await
                .expect("legacy SQLite v30 HTTP flow");
        })
        .await;
}

async fn run_legacy_http_flow() -> Result<(), String> {
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
            .oneshot(post_json(
                "/api/v1/maintenance/import-v30",
                serde_json::json!({
                    "path": source_path,
                    "canonical_attachment_root": attachment_root,
                }),
            ))
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let body = decode_json(response).await?;
        assert_eq!(status, StatusCode::OK, "legacy import response: {body}");
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
            .oneshot(post_json(
                "/api/v1/maintenance/import-v30",
                serde_json::json!({"path":"/tmp/legacy-v30.sqlite"}),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
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

fn post_json(uri: &str, value: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&value).expect("legacy request JSON"),
        ))
        .expect("legacy POST request")
}

#[cfg(feature = "legacy-sqlite-import")]
async fn decode_json(response: axum::response::Response) -> Result<Value, String> {
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .map_err(|error| error.to_string())?
        .to_bytes();
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}
