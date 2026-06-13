use crate::common::*;

#[test]
fn dag_snapshot_reports_raw_and_derived_graph_with_frontier() -> anyhow::Result<()> {
    let temp = TempDb::new("dag_snapshot_reports_raw_and_derived_graph_with_frontier")?;
    init_database(&temp.path, "tester")?;

    let a = create_ready(&temp, "A done parent", 3)?;
    let b = create_ready(&temp, "B done parent", 3)?;
    let c = create_ready(&temp, "C todo all parents done", 0)?;
    let d = create_ready(&temp, "D ready no deps", 0)?;
    let e = create_ready(&temp, "E waits on C", 1)?;
    let f = create_ready(&temp, "F blocked", 2)?;
    let g = create_ready(&temp, "G running", 2)?;
    let h = create_ready(&temp, "H review", 2)?;
    let i = create_ready(&temp, "I done leaf", 3)?;
    let j = create_ready(&temp, "J archived", 2)?;

    complete_ready(&temp, &a.id)?;
    complete_ready(&temp, &b.id)?;
    complete_ready(&temp, &i.id)?;
    add_dependency(&temp.path, "default", "tester", &a.id, &c.id)?;
    add_dependency(&temp.path, "default", "tester", &b.id, &c.id)?;
    add_dependency(&temp.path, "default", "tester", &c.id, &e.id)?;
    block_task(
        &temp.path,
        "default",
        "tester",
        &f.id,
        "waiting for input",
        None,
        false,
    )?;
    claim_task(&temp.path, "default", "worker", &g.id, 300_000)?;
    submit_for_review(&temp, &h.id)?;
    archive_task(&temp.path, "default", "tester", &j.id, false)?;

    let snapshot = kanban_sqlite::dag_snapshot(&temp.path, "default")?;

    assert_eq!(snapshot.board.slug, "default");
    assert_eq!(snapshot.snapshot.node_count, 9);
    assert_eq!(snapshot.snapshot.edge_count, 3);
    assert!(!snapshot.raw.nodes.iter().any(|node| node.id == j.id));
    assert_eq!(
        snapshot
            .raw
            .edges
            .iter()
            .map(|edge| (edge.parent.as_str(), edge.child.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (a.id.as_str(), c.id.as_str()),
            (b.id.as_str(), c.id.as_str()),
            (c.id.as_str(), e.id.as_str()),
        ]
    );
    assert_eq!(
        snapshot
            .derived
            .blocked_by
            .iter()
            .find(|entry| entry.task_id == c.id)
            .map(|entry| entry.tasks.clone()),
        Some(vec![a.id.clone(), b.id.clone()])
    );
    assert_eq!(
        snapshot
            .derived
            .unblocks
            .iter()
            .find(|entry| entry.task_id == c.id)
            .map(|entry| entry.tasks.clone()),
        Some(vec![e.id.clone()])
    );

    let actionable_ids = ids(&snapshot.derived.actionable);
    assert!(actionable_ids.contains(&c.id));
    assert!(actionable_ids.contains(&d.id));
    assert!(actionable_ids.contains(&e.id));

    let frontier_ids = ids(&snapshot.derived.frontier);
    assert_eq!(frontier_ids, vec![c.id.clone(), d.id.clone()]);
    for excluded in [&e, &f, &g, &h, &i, &j] {
        assert!(
            !frontier_ids.contains(&excluded.id),
            "unexpected frontier task {}",
            excluded.title
        );
    }
    assert!(snapshot.derived.frontier.iter().all(
        |entry| entry.why.contains("frontier") && entry.why.contains("前置依赖已完成或不存在")
    ));
    assert!(snapshot.raw.nodes[0].why.contains("当前状态为"));
    assert!(snapshot.raw.edges[0].why.contains("必须先完成"));
    assert!(
        snapshot
            .derived
            .blocked_by
            .iter()
            .any(|entry| entry.why.contains("被以下前置任务阻塞"))
    );
    Ok(())
}

#[test]
fn dag_snapshot_sort_is_stable_and_uses_documented_keys() -> anyhow::Result<()> {
    let temp = TempDb::new("dag_snapshot_sort_is_stable_and_uses_documented_keys")?;
    init_database(&temp.path, "tester")?;

    let parent = create_ready(&temp, "fan out", 1)?;
    let child_a = create_ready(&temp, "fan child a", 1)?;
    let child_b = create_ready(&temp, "fan child b", 1)?;
    let due = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "earliest due".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: Some(100),
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let no_due = create_ready(&temp, "no due", 0)?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child_a.id)?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child_b.id)?;

    let snapshot = kanban_sqlite::dag_snapshot(&temp.path, "default")?;
    let node_ids = snapshot
        .raw
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<Vec<_>>();

    assert_eq!(snapshot.snapshot.sort[0], "priority asc");
    assert_eq!(node_ids[0], due.id);
    assert_eq!(node_ids[1], no_due.id);
    assert_eq!(node_ids[2], parent.id);
    Ok(())
}

#[test]
fn dag_ancestors_returns_topological_subset_and_excludes_archived() -> anyhow::Result<()> {
    let temp = TempDb::new("dag_ancestors_returns_topological_subset_and_excludes_archived")?;
    init_database(&temp.path, "tester")?;

    let root = create_ready(&temp, "root", 0)?;
    let sibling = create_ready(&temp, "sibling", 1)?;
    let middle = create_ready(&temp, "middle", 2)?;
    let target = create_ready(&temp, "target", 3)?;
    let archived_parent = create_ready(&temp, "archived parent", 0)?;

    add_dependency(&temp.path, "default", "tester", &root.id, &middle.id)?;
    add_dependency(&temp.path, "default", "tester", &sibling.id, &target.id)?;
    add_dependency(&temp.path, "default", "tester", &middle.id, &target.id)?;
    add_dependency(
        &temp.path,
        "default",
        "tester",
        &archived_parent.id,
        &target.id,
    )?;
    archive_task(&temp.path, "default", "tester", &archived_parent.id, false)?;

    let ancestors = kanban_sqlite::dag_ancestors(&temp.path, "default", &target.id)?;

    assert_eq!(ancestors.target.id, target.id);
    assert_eq!(
        ancestors
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            root.id.as_str(),
            sibling.id.as_str(),
            middle.id.as_str(),
            target.id.as_str()
        ]
    );
    assert_eq!(
        ancestors.ordered_refs,
        ancestors
            .nodes
            .iter()
            .map(|node| node.task_ref.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        !ancestors
            .nodes
            .iter()
            .any(|node| node.id == archived_parent.id)
    );
    assert!(
        ancestors
            .edges
            .iter()
            .all(|edge| edge.why.contains("必须先完成"))
    );
    Ok(())
}

fn create_ready(temp: &TempDb, title: &str, priority: i64) -> anyhow::Result<TaskRecord> {
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: title.into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )
    .map_err(Into::into)
}

fn complete_ready(temp: &TempDb, task_id: &str) -> anyhow::Result<()> {
    let claim = claim_task(&temp.path, "default", "worker", task_id, 300_000)?;
    complete_task(
        &temp.path,
        "default",
        "worker",
        task_id,
        Some(&claim.claim_token),
        false,
    )?;
    Ok(())
}

fn submit_for_review(temp: &TempDb, task_id: &str) -> anyhow::Result<()> {
    let claim = claim_task(&temp.path, "default", "worker", task_id, 300_000)?;
    submit_review_task(
        &temp.path,
        "default",
        "worker",
        task_id,
        Some(&claim.claim_token),
        false,
    )?;
    Ok(())
}

fn ids(entries: &[kanban_sqlite::DagTaskReason]) -> Vec<String> {
    entries.iter().map(|entry| entry.task_id.clone()).collect()
}
