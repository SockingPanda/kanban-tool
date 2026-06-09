use crate::common::*;

#[tokio::test]
async fn health_reports_ok_database_and_version() {
    let (_dir, db_path) = temp_db();
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(app, "/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["ok"], true);
    assert_eq!(json["data"]["db"], "ok");
    assert_eq!(json["data"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(json.get("error").is_none());
}
