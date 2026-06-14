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
