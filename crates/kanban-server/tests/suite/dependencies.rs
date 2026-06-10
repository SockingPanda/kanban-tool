use crate::common::*;

#[tokio::test]
async fn dependencies_api_add_remove_list_and_cycle_error() {
    let test = TestApp::new();
    let db_path = test.db_path().to_path_buf();
    let parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("parent"),
    )
    .expect("parent");
    let child = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("child"),
    )
    .expect("child");
    let app = test.router();

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", child.id),
        json!({"parent_task_id":parent.id,"actor":"dep-actor"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["data"]["parents"][0]["id"], parent.id);
    assert_eq!(json["data"]["children"].as_array().unwrap().len(), 0);

    let (status, json) = get_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", parent.id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["children"][0]["id"], child.id);

    let (status, json) = post_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies", parent.id),
        json!({"parent_task_id":child.id}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "dependency_cycle");

    let (status, json) = delete_json(
        app.clone(),
        &format!("/api/v1/tasks/{}/dependencies/{}", child.id, parent.id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["data"]["parents"].as_array().unwrap().is_empty());
}
