use crate::common::*;

#[tokio::test]
async fn api_error_limit_uses_snapshot_baseline() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/boards/default/tasks?limit=100000").await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    insta::assert_json_snapshot!(json, @r###"
    {
      "error": {
        "code": "invalid_input",
        "message": "invalid input: limit must be <= 1000"
      }
    }
    "###);
    Ok(())
}
