use crate::common::*;

#[tokio::test]
async fn search_returns_hits_with_tasks_and_sqlite_status() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    for name in ["backend", "frontend"] {
        kanban_sqlite::api::create_label(
            &db_path,
            "default",
            kanban_sqlite::api::CreateLabel {
                name: name.to_owned(),
                color: None,
            },
        )?;
    }
    for (title, assignee, labels) in [
        (
            "alpha api search",
            Some("worker-a"),
            vec!["backend".to_owned()],
        ),
        (
            "beta api search",
            Some("worker-b"),
            vec!["frontend".to_owned()],
        ),
    ] {
        kanban_sqlite::api::create_task_with_labels(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::api::CreateTask {
                title: title.to_owned(),
                description: Some("ready spec api-needle".to_owned()),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: assignee.map(str::to_owned),
                priority: 0,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".to_owned(),
            },
            &labels,
        )
        .context("seed task")?;
    }
    let app = test.router();

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/search/tasks?board=default&q=api-needle&assignee=worker-a&limit=10",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["meta"]["backend"], "sqlite");
    assert_eq!(json["meta"]["limit"], 10);
    assert_eq!(json["meta"]["offset"], 0);
    let hits = json["data"]["hits"].as_array().context("hits array")?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "alpha api search");
    assert!(hits[0]["score"].as_f64().context("value")? > 0.0);
    assert!(
        hits[0]["snippet"]
            .as_str()
            .context("value")?
            .contains("api-needle")
    );

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/search/tasks?board=default&q=api-needle&label=frontend&limit=10",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let hits = json["data"]["hits"].as_array().context("hits array")?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "beta api search");
    assert_eq!(hits[0]["task"]["labels"][0]["name"], "frontend");

    let (status, json) = get_json(app, "/api/v1/search/status?board=default").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["backend"], "sqlite");
    assert_eq!(json["data"]["derived_index"], false);
    assert_eq!(json["data"]["stale"], false);
    Ok(())
}

#[tokio::test]
async fn search_by_status_returns_per_status_windows() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    create_ready_task_for_test(&db_path, "default", "seed", "ready batch needle")
        .context("seed ready task")?;
    for (title, status) in [
        ("todo batch needle", kanban_core::TaskStatus::Todo),
        ("triage batch needle", kanban_core::TaskStatus::Triage),
    ] {
        kanban_sqlite::api::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::api::CreateTask {
                title: title.to_owned(),
                description: Some("search batch spec".to_owned()),
                status: Some(status),
                assignee: None,
                priority: 0,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".to_owned(),
            },
        )
        .context("seed task")?;
    }
    let app = test.router();

    let (status, json) = get_json(
        app,
        "/api/v1/search/tasks/by-status?board=default&q=batch&status=ready&status=todo&limit=1",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["meta"]["limit"], 1);
    assert_eq!(json["meta"]["offset"], 0);
    let windows = json["data"]["statuses"]
        .as_array()
        .context("status windows")?;
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0]["status"], "ready");
    assert_eq!(
        windows[0]["tasks"].as_array().context("ready tasks")?.len(),
        1
    );
    assert_eq!(windows[0]["search_meta"]["backend"], "sqlite");
    assert_eq!(windows[1]["status"], "todo");
    assert_eq!(
        windows[1]["tasks"].as_array().context("todo tasks")?.len(),
        1
    );
    assert_eq!(windows[1]["search_meta"]["backend"], "sqlite");
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
mod tantivy_backend {
    use super::*;

    #[tokio::test]
    async fn search_uses_tantivy_index_when_rebuilt() -> anyhow::Result<()> {
        let test = TestApp::new()?;
        let db_path = test.db_path().to_path_buf();
        kanban_sqlite::api::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::api::CreateTask {
                title: "api tantivy comet".to_owned(),
                description: Some("ready spec".to_owned()),
                status: Some(kanban_core::TaskStatus::Ready),
                assignee: Some("worker-a".to_owned()),
                priority: 0,
                scheduled_at: None,
                due_at: None,
                max_retries: None,
                metadata_json: "{}".to_owned(),
            },
        )
        .context("seed task")?;
        kanban_sqlite::api::rebuild_search_index(&db_path, "default").context("rebuild index")?;
        let app = test.router();

        let (status, json) = get_json(
            app,
            "/api/v1/search/tasks?board=default&q=comet&assignee=worker-a&limit=10",
        )
        .await?;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["data"]["meta"]["backend"], "tantivy");
        assert_eq!(
            json["data"]["hits"][0]["task"]["title"],
            "api tantivy comet"
        );
        Ok(())
    }
}

#[tokio::test]
async fn search_rejects_unbounded_limit() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    let (status, json) = get_json(
        app,
        &format!(
            "/api/v1/search/tasks?board=default&q=needle&limit={}",
            usize::MAX
        ),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("value")?
            .contains("limit must be <= 1000")
    );
    Ok(())
}

#[tokio::test]
async fn search_treats_like_wildcards_as_literal_text() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    for title in ["literal percent % api", "plain api control"] {
        kanban_sqlite::api::create_task(
            &db_path,
            "default",
            "seed",
            kanban_sqlite::api::CreateTask {
                title: title.to_owned(),
                description: Some("ready spec".to_owned()),
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
    }
    let app = test.router();

    let (status, json) = get_json(app, "/api/v1/search/tasks?board=default&q=%25").await?;
    assert_eq!(status, StatusCode::OK);
    let hits = json["data"]["hits"].as_array().context("hits array")?;
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "literal percent % api");
    Ok(())
}

#[tokio::test]
async fn search_matches_task_refs_exactly() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let first = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("first api task"),
    )
    .context("seed first task")?;
    let _second = kanban_sqlite::api::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::api::CreateTask::ready("title mentions 1 but must not match numeric search"),
    )
    .context("seed second task")?;
    let app = test.router();

    for query in ["1", "%231", "default%231", first.id.as_str()] {
        let (status, json) = get_json(
            app.clone(),
            &format!("/api/v1/search/tasks?board=default&q={query}&limit=10"),
        )
        .await?;
        assert_eq!(status, StatusCode::OK, "{query}: {json}");
        let hits = json["data"]["hits"].as_array().context("hits array")?;
        assert_eq!(hits.len(), 1, "{query}: {json}");
        assert_eq!(hits[0]["task"]["id"], first.id, "{query}: {json}");
    }

    let (status, json) = get_json(
        app,
        "/api/v1/search/tasks?board=default&q=other%231&limit=10",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(
        json["data"]["hits"]
            .as_array()
            .context("hits array")?
            .is_empty()
    );
    Ok(())
}
