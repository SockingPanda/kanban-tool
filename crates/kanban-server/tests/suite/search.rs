use crate::common::*;

#[tokio::test]
async fn search_api_returns_hits_with_tasks_and_sqlite_status() {
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    for (title, assignee) in [
        ("alpha api search", Some("worker-a")),
        ("beta api search", Some("worker-b")),
    ] {
        kanban_sqlite::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::CreateTask {
                title: title.to_owned(),
                description: Some("ready spec api-needle".to_owned()),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: assignee.map(str::to_owned),
                priority: 0,
                scheduled_at: None,
                due_at: None,
                metadata_json: "{}".to_owned(),
            },
        )
        .expect("seed task");
    }
    let app = test.router();

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/search/tasks?board=default&q=api-needle&assignee=worker-a&limit=10",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["meta"]["backend"], "sqlite");
    assert_eq!(json["meta"]["limit"], 10);
    assert_eq!(json["meta"]["offset"], 0);
    let hits = json["data"]["hits"].as_array().expect("hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "alpha api search");
    assert!(hits[0]["score"].as_f64().unwrap() > 0.0);
    assert!(hits[0]["snippet"].as_str().unwrap().contains("api-needle"));

    let (status, json) = get_json(app, "/api/v1/search/status?board=default").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["backend"], "sqlite");
    assert_eq!(json["data"]["derived_index"], false);
    assert_eq!(json["data"]["stale"], false);
}

#[cfg(feature = "tantivy-backend")]
mod tantivy_backend {
    use super::*;

    #[tokio::test]
    async fn search_api_uses_tantivy_index_when_rebuilt() {
        let test = TestApp::new();
        let db_path = test.db_path().to_path_buf();
        kanban_sqlite::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::CreateTask {
                title: "api tantivy comet".to_owned(),
                description: Some("ready spec".to_owned()),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: Some("worker-a".to_owned()),
                priority: 0,
                scheduled_at: None,
                due_at: None,
                metadata_json: "{}".to_owned(),
            },
        )
        .expect("seed task");
        kanban_sqlite::rebuild_search_index(&db_path, "default").expect("rebuild index");
        let app = test.router();

        let (status, json) = get_json(
            app,
            "/api/v1/search/tasks?board=default&q=comet&assignee=worker-a&limit=10",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["meta"]["backend"], "tantivy");
        assert_eq!(
            json["data"]["hits"][0]["task"]["title"],
            "api tantivy comet"
        );
    }
}

#[tokio::test]
async fn search_api_rejects_unbounded_limit() {
    let test = TestApp::new();
    let app = test.router();

    let (status, json) = get_json(
        app,
        &format!(
            "/api/v1/search/tasks?board=default&q=needle&limit={}",
            usize::MAX
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("limit must be <= 1000")
    );
}

#[tokio::test]
async fn search_api_treats_like_wildcards_as_literal_text() {
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    for title in ["literal percent % api", "plain api control"] {
        kanban_sqlite::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::CreateTask {
                title: title.to_owned(),
                description: Some("ready spec".to_owned()),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: None,
                priority: 0,
                scheduled_at: None,
                due_at: None,
                metadata_json: "{}".to_owned(),
            },
        )
        .expect("seed task");
    }
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/search/tasks?board=default&q=%25").await;
    assert_eq!(status, StatusCode::OK);
    let hits = json["data"]["hits"].as_array().expect("hits array");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "literal percent % api");
}
