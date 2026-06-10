use crate::common::*;

#[tokio::test]
async fn health_reports_ok_database_and_version() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, json) = get_json(app, "/health").await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["ok"], true);
    assert_eq!(json["data"]["db"], "ok");
    assert_eq!(json["data"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(json.get("error").is_none());
    Ok(())
}
