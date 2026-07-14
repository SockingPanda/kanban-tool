use crate::common::*;
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> serde_json::Value {
    serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/fixtures/api")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn delete_label_semantics_response_fixture_is_produced_by_real_router() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let label = kanban_sqlite::api::create_label(
        test.db_path(),
        "default",
        kanban_sqlite::api::CreateLabel {
            name: "contract/delete".to_owned(),
            color: None,
        },
    )?;
    let (_, response) = request_json(
        test.router(),
        "PUT",
        &format!("/api/v1/boards/default/labels/{}/semantics", label.id),
        Some(serde_json::json!({"description":"delete fixture"})),
        None,
    )
    .await?;
    let hash = response["data"]["semantics_hash"]
        .as_str()
        .unwrap_or_default();
    let (status, response) = delete_json(test.router(), &format!("/api/v1/boards/default/labels/{}/semantics?expected_semantics_hash={hash}&reason=contract-delete", label.id)).await?;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(response, fixture("delete-response.v1.valid.json"));
    Ok(())
}

#[test]
fn delete_label_semantics_response_fixture_is_consumed_by_contract_root() {
    let value = fixture("delete-response.v1.valid.json");
    let response: kanban_contract::DeleteResponse = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), value);
}
