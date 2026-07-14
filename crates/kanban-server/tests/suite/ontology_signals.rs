use crate::common::*;

#[tokio::test]
async fn list_label_ontology_signals_response_fixture_is_produced_by_real_router()
-> anyhow::Result<()> {
    let app = TestApp::new()?;
    let (status, value) = get_json(
        app.router(),
        "/api/v1/boards/default/label-ontology/signals?limit=7",
    )
    .await?;
    anyhow::ensure!(status == axum::http::StatusCode::OK);
    let response: kanban_contract::LabelOntologySignalsResponse = serde_json::from_value(value)?;
    anyhow::ensure!(response.meta.limit == 7);
    Ok(())
}

#[test]
fn list_label_ontology_signals_response_fixture_is_consumed_by_contract_root() -> anyhow::Result<()>
{
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../schemas/fixtures/api/list-label-ontology-signals-response.v1.valid.json"
    ))?;
    let _: kanban_contract::LabelOntologySignalsResponse = serde_json::from_value(value)?;
    Ok(())
}

#[tokio::test]
async fn list_label_ontology_signals_rejects_limit_overflow() -> anyhow::Result<()> {
    let app = TestApp::new()?;
    let (status, _) = get_json(
        app.router(),
        "/api/v1/boards/default/label-ontology/signals?limit=1001",
    )
    .await?;
    anyhow::ensure!(status == axum::http::StatusCode::BAD_REQUEST);
    Ok(())
}
