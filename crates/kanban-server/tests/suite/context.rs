use crate::common::*;

#[tokio::test]
async fn context_graph_and_vector_apis_return_default_fallbacks() {
    let (_dir, db_path) = temp_db();
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
            metadata_json: "{}".to_owned(),
        },
    )
    .expect("seed task");
    let app = build_router(AppState::new(db_path, "api-test"));

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/tasks/{}/context?board=default&lexical_limit=3",
            task.id
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["subject"], format!("kb://task/{}", task.id));
    assert_eq!(json["data"]["items"][0]["source"], "subject");
    #[cfg(not(feature = "graph-oxigraph"))]
    assert!(
        json["data"]["degraded"]
            .as_array()
            .unwrap()
            .contains(&json!("graph_disabled"))
    );
    #[cfg(feature = "graph-oxigraph")]
    assert!(
        json["data"]["degraded"]
            .as_array()
            .unwrap()
            .contains(&json!("graph_dirty"))
    );
    #[cfg(not(feature = "vector-lancedb"))]
    assert!(
        json["data"]["degraded"]
            .as_array()
            .unwrap()
            .contains(&json!("vector_disabled"))
    );
    #[cfg(feature = "vector-lancedb")]
    assert!(
        json["data"]["degraded"]
            .as_array()
            .unwrap()
            .contains(&json!("vector_disabled"))
    );

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/tasks/{}/context?board=default&max_items=0",
            task.id
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("max_items must be >= 1")
    );

    let (status, json) = get_json(app.clone(), "/api/v1/graph/status?board=default").await;
    assert_eq!(status, StatusCode::OK);
    let graph_enabled = json["data"]["enabled"].as_bool().unwrap();

    let (status, json) = get_json(
        app.clone(),
        &format!(
            "/api/v1/graph/neighbors?entity_uri=kb%3A%2F%2Ftask%2F{}",
            task.id
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    if !graph_enabled {
        assert_eq!(json["data"].as_array().unwrap().len(), 0);
    }

    let (status, json) = get_json(app, "/api/v1/vector/status?board=default").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], false);
}
