use crate::common::*;

#[tokio::test]
async fn task_neighborhood_returns_one_hop_nodes_and_visible_internal_edges() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let blocker = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("blocker"),
    )?;
    let peer = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("peer blocker"),
    )?;
    let center = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("center"),
    )?;
    let unlock = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("unlock"),
    )?;
    kanban_sqlite::add_dependency(&db_path, "default", "seed", &blocker.id, &center.id)?;
    kanban_sqlite::add_dependency(&db_path, "default", "seed", &peer.id, &center.id)?;
    kanban_sqlite::add_dependency(&db_path, "default", "seed", &center.id, &unlock.id)?;
    kanban_sqlite::add_dependency(&db_path, "default", "seed", &blocker.id, &peer.id)?;

    let (status, json) = get_json(
        test.router(),
        &format!("/api/v1/tasks/{}/neighborhood?depth=1", center.id),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["center_task_id"], center.id);
    assert_eq!(json["data"]["meta"]["depth"], 1);
    assert_eq!(json["data"]["meta"]["truncated"], false);

    let nodes = json["data"]["nodes"].as_array().context("nodes")?;
    let node_ids = nodes
        .iter()
        .map(|node| node["task"]["id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(node_ids.len(), 4);
    assert!(node_ids.contains(&blocker.id.as_str()));
    assert!(node_ids.contains(&peer.id.as_str()));
    assert!(node_ids.contains(&center.id.as_str()));
    assert!(node_ids.contains(&unlock.id.as_str()));
    assert!(
        nodes
            .iter()
            .any(|node| node["task"]["id"] == center.id && node["role"] == "center")
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["task"]["id"] == blocker.id && node["role"] == "dependency_parent")
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["task"]["id"] == unlock.id && node["role"] == "dependency_child")
    );
    assert!(
        nodes
            .iter()
            .all(|node| node["task"].get("claim_token").is_none())
    );

    let edges = json["data"]["edges"].as_array().context("edges")?;
    assert_eq!(edges.len(), 4);
    assert!(
        edges
            .iter()
            .any(|edge| edge["source_task_id"] == blocker.id && edge["target_task_id"] == peer.id)
    );
    assert!(edges.iter().all(|edge| edge["kind"] == "dependency"));
    assert!(edges.iter().all(|edge| edge["required"] == true));
    assert!(edges.iter().all(|edge| edge["blocking"] == true));

    let (status, limited_json) = get_json(
        test.router(),
        &format!(
            "/api/v1/tasks/{}/neighborhood?depth=1&limit_nodes=1",
            center.id
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(limited_json["data"]["meta"]["truncated"], true);
    let limited_nodes = limited_json["data"]["nodes"].as_array().context("nodes")?;
    assert_eq!(limited_nodes.len(), 1);
    assert_eq!(limited_nodes[0]["task"]["id"], center.id);
    assert_eq!(limited_nodes[0]["role"], "center");
    Ok(())
}

#[tokio::test]
async fn board_task_map_returns_active_graph_with_done_context_and_excludes_archived()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let db_path = test.db_path().to_path_buf();
    let done_parent = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("done context"),
    )?;
    let done_claim =
        kanban_sqlite::claim_task(&db_path, "default", "seed", &done_parent.id, 60_000)?;
    kanban_sqlite::complete_task(
        &db_path,
        "default",
        "seed",
        &done_parent.id,
        Some(&done_claim.claim_token),
        false,
    )?;
    let active = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("active"),
    )?;
    let archived = kanban_sqlite::create_task(
        &db_path,
        "default",
        "seed",
        kanban_sqlite::CreateTask::ready("archived context"),
    )?;
    kanban_sqlite::archive_task(&db_path, "default", "seed", &archived.id, false)?;
    kanban_sqlite::add_dependency(&db_path, "default", "seed", &done_parent.id, &active.id)?;
    kanban_sqlite::add_dependency(&db_path, "default", "seed", &archived.id, &active.id)?;

    let (status, json) = get_json(
        test.router(),
        "/api/v1/boards/default/task-map?active_only=true&context_depth=1",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["meta"]["context_depth"], 1);
    assert_eq!(json["data"]["meta"]["truncated"], false);
    assert_eq!(json["data"]["meta"]["include_archived_context"], false);

    let nodes = json["data"]["nodes"].as_array().context("nodes")?;
    assert!(
        nodes
            .iter()
            .any(|node| node["task"]["id"] == active.id && node["context_only"] == false)
    );
    assert!(nodes.iter().any(|node| node["task"]["id"] == done_parent.id
        && node["context_only"] == true
        && node["role"] == "context"));
    assert!(!nodes.iter().any(|node| node["task"]["id"] == archived.id));

    let edges = json["data"]["edges"].as_array().context("edges")?;
    assert!(edges.iter().any(
        |edge| edge["source_task_id"] == done_parent.id && edge["target_task_id"] == active.id
    ));
    assert!(
        !edges
            .iter()
            .any(|edge| edge["source_task_id"] == archived.id
                || edge["target_task_id"] == archived.id)
    );
    Ok(())
}
