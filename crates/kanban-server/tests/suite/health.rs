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
    assert_eq!(
        json["data"]["db_path"],
        test.db_path().display().to_string()
    );
    assert!(
        json["data"]["db_fingerprint"]
            .as_str()
            .context("db fingerprint")?
            .starts_with("sqlite:")
    );
    assert!(json.get("error").is_none());
    Ok(())
}

#[tokio::test]
async fn health_rejects_deleted_database_instead_of_recreating_it() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    std::fs::remove_file(&db_path).context("remove database")?;
    let app = test.router();

    let (status, json) = get_json(app, "/health").await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("database file is missing")
    );
    assert!(
        !db_path.exists(),
        "health must not recreate a deleted database"
    );
    Ok(())
}

#[tokio::test]
async fn api_rejects_deleted_database_before_task_or_board_handlers_recreate_it()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    std::fs::remove_file(&db_path).context("remove database")?;
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/boards").await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("database file is missing")
    );
    assert!(
        !db_path.exists(),
        "API guard must not recreate the database"
    );
    Ok(())
}

#[tokio::test]
async fn health_rejects_empty_database_file_without_reporting_ok() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    std::fs::remove_file(&db_path).context("remove database")?;
    std::fs::File::create(&db_path).context("create empty database file")?;
    let app = test.router();

    let (status, json) = get_json(app, "/health").await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("database failed health check")
    );
    Ok(())
}

#[tokio::test]
async fn health_response_fixture_is_produced_by_real_router() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let (status, json) = get_json(test.router(), "/health").await?;
    assert_eq!(status, StatusCode::OK);
    let typed: kanban_contract::HealthResponse = serde_json::from_value(json.clone())?;
    assert_eq!(typed.data.db_path, test.db_path().display().to_string());
    assert!(typed.data.db_fingerprint.starts_with("sqlite:"));
    assert_eq!(serde_json::to_value(&typed)?, json);
    let mut typed_json = serde_json::to_value(typed.clone())?;
    typed_json["data"]["db_path"] = serde_json::json!("/tmp/kanban.db");
    typed_json["data"]["db_fingerprint"] = serde_json::json!("sqlite:4096:1");
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/fixtures/api/health-response.v1.valid.json");
    if std::env::var("KANBAN_UPDATE_SCHEMA_FIXTURES").as_deref() == Ok("1") {
        std::fs::write(
            &fixture_path,
            serde_json::to_string_pretty(&typed_json)? + "\n",
        )?;
    }
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fixture_path)?)?;
    assert_eq!(typed_json, fixture);
    Ok(())
}

#[test]
fn health_response_contract_consumes_producer_fixture() -> anyhow::Result<()> {
    let fixture: kanban_contract::HealthResponse = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/health-response.v1.valid.json"
    ))?;
    assert_eq!(
        serde_json::from_str::<kanban_contract::HealthResponse>(&serde_json::to_string(&fixture)?)?,
        fixture
    );
    Ok(())
}

#[test]
fn api_error_response_contract_consumes_fixture() -> anyhow::Result<()> {
    let fixture: kanban_contract::ErrorEnvelope = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/error-response.v1.valid.json"
    ))?;
    assert_eq!(
        serde_json::to_value(&fixture)?,
        serde_json::json!({"error":{"code":"invalid_input","message":"输入无效：limit 必须小于等于 1000"}})
    );
    Ok(())
}
