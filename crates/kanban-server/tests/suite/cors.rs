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
async fn generic_serve_router_does_not_enable_desktop_cors() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.serve_router();

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

#[tokio::test]
async fn desktop_router_allows_semantics_put_preflight_and_request() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.desktop_router();
    let allowed_origin = "http://127.0.0.1:1420";

    let (status, json) = post_json(
        app.clone(),
        "/api/v1/boards/default/labels",
        json!({"name": "cors-put"}),
    )
    .await?;
    assert_eq!(status, StatusCode::CREATED);
    let label_id = json["data"]["id"].as_str().context("label id")?.to_owned();
    let semantics_path = format!("/api/v1/boards/default/labels/{label_id}/semantics");

    let (status, headers) =
        options_raw_for_method(app.clone(), &semantics_path, allowed_origin, "PUT").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&HeaderValue::from_static("http://127.0.0.1:1420"))
    );
    let allow_methods = headers
        .get(header::ACCESS_CONTROL_ALLOW_METHODS)
        .context("access-control-allow-methods")?
        .to_str()
        .context("access-control-allow-methods utf8")?;
    assert!(
        allow_methods
            .split(',')
            .any(|method| method.trim().eq_ignore_ascii_case("PUT")),
        "allow methods must include PUT, got {allow_methods}"
    );

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&semantics_path)
                .header(header::ORIGIN, allowed_origin)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "description": "CORS PUT semantics",
                        "applies_when": ["updates label semantics from desktop"]
                    })
                    .to_string(),
                ))
                .context("put request")?,
        )
        .await
        .context("put response")?;
    let headers = response.headers().clone();
    let (status, json) = response_json(response).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["description"], "CORS PUT semantics");
    assert_eq!(
        headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        Some(&HeaderValue::from_static("http://127.0.0.1:1420"))
    );

    let (_status, headers) =
        options_raw_for_method(app, &semantics_path, "http://example.com", "PUT").await?;
    assert!(headers.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());

    Ok(())
}
