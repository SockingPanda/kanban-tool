use crate::common::*;

#[tokio::test]
async fn default_router_does_not_enable_browser_cors_for_mutations() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (_status, headers) =
        options_raw(app, "/api/v1/boards/default/tasks", "http://127.0.0.1:1420").await?;

    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    Ok(())
}

#[tokio::test]
async fn desktop_router_allows_only_local_desktop_origins() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.desktop_router();

    let (status, headers) = options_raw(
        app.clone(),
        "/api/v1/boards/default/tasks",
        "http://127.0.0.1:1420",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&HeaderValue::from_static("http://127.0.0.1:1420"))
    );

    let (_status, headers) =
        options_raw(app, "/api/v1/boards/default/tasks", "http://example.com").await?;
    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    Ok(())
}
