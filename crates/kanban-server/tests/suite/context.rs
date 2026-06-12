use crate::common::*;

#[cfg(feature = "vector-lancedb")]
struct StaticProvider {
    model: &'static str,
    dimensions: usize,
}

#[cfg(feature = "vector-lancedb")]
impl kanban_vector::EmbeddingProvider for StaticProvider {
    fn embedding_model(&self) -> &str {
        self.model
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn embed(&self, _text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
        Ok(vec![0.0; self.dimensions])
    }
}

#[tokio::test]
async fn context_graph_and_vector_apis_return_default_fallbacks() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask {
            title: "context api source".to_owned(),
            description: Some("ready spec context-api-needle".to_owned()),
            status: Some(kanban_core::TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .context("seed task")?;
    let app = test.router();

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/tasks/{}/context?board=default&lexical_limit=3",
            task.id
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["subject"], format!("kb://task/{}", task.id));
    assert_eq!(json["data"]["items"][0]["source"], "subject");
    #[cfg(not(feature = "graph-oxigraph"))]
    assert!(
        json["data"]["degraded"]
            .as_array()
            .context("value")?
            .contains(&json!("graph_disabled"))
    );
    #[cfg(feature = "graph-oxigraph")]
    assert!(
        json["data"]["degraded"]
            .as_array()
            .context("value")?
            .contains(&json!("graph_dirty"))
    );
    #[cfg(not(feature = "vector-lancedb"))]
    assert!(
        json["data"]["degraded"]
            .as_array()
            .context("value")?
            .contains(&json!("vector_disabled"))
    );
    #[cfg(feature = "vector-lancedb")]
    assert!(
        json["data"]["degraded"]
            .as_array()
            .context("value")?
            .contains(&json!("vector_disabled"))
    );

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/tasks/{}/context?board=default&max_items=0",
            task.id
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("value")?
            .contains("max_items must be >= 1")
    );

    let (status, json) = get_json(app.clone(), "/api/v1/graph/status?board=default").await?;
    assert_eq!(status, StatusCode::OK);
    let graph_enabled = json["data"]["enabled"].as_bool().context("value")?;

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/graph/neighbors?entity_uri=kb%3A%2F%2Ftask%2F{}",
            task.id
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    if !graph_enabled {
        assert_eq!(json["data"].as_array().context("value")?.len(), 0);
    }

    let (status, json) = get_json(app, "/api/v1/vector/status?board=default").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], false);
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[tokio::test]
async fn context_api_degrades_when_configured_vector_store_construction_fails() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let task = kanban_sqlite::create_task(
        &db_path,
        "default",
        "api-test",
        kanban_sqlite::CreateTask {
            title: "context api schema mismatch".to_owned(),
            description: Some("ready spec api schema mismatch needle".to_owned()),
            status: Some(kanban_core::TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".to_owned(),
        },
    )
    .context("seed task")?;

    let seed_db_path = db_path.clone();
    tokio::task::spawn_blocking(move || {
        let provider = std::sync::Arc::new(StaticProvider {
            model: "static-test",
            dimensions: 2,
        });
        let _store = kanban_vector::LanceDbStore::connect(kanban_vector::LanceDbConfig::new(
            kanban_local::vector_store_path(seed_db_path),
            provider,
        ))?;
        Ok::<(), kanban_vector::VectorError>(())
    })
    .await
    .context("seed LanceDB table task panicked")?
    .context("seed 2-dimensional LanceDB table")?;

    let vector_config = test.dir_path().join("mismatched-vector.toml");
    std::fs::write(
        &vector_config,
        r#"[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:1"
model = "offline-api-test-model"
dimensions = 3
"#,
    )?;
    let app =
        build_router(AppState::new(&db_path, "api-test").with_vector_config_path(vector_config));

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/tasks/{}/context?board=default&lexical_limit=3",
            task.id
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["subject"], format!("kb://task/{}", task.id));
    assert!(
        json["data"]["degraded"]
            .as_array()
            .context("value")?
            .contains(&json!("vector_error"))
    );
    assert!(
        json["data"]["diagnostics"]
            .as_array()
            .context("value")?
            .iter()
            .any(|diagnostic| diagnostic["source"] == "vector"
                && diagnostic["code"] == "vector_error")
    );

    let (status, json) = get_json(app, "/api/v1/vector/status?board=default").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], true);
    assert!(
        json["data"]["message"]
            .as_str()
            .context("value")?
            .contains("offline-api-test-model")
    );
    assert!(
        json["data"]["message"]
            .as_str()
            .context("value")?
            .contains("http://127.0.0.1:1")
    );
    Ok(())
}
