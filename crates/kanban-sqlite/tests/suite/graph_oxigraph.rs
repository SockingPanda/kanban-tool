#![cfg(feature = "graph-oxigraph")]

use crate::common::*;

#[test]
fn graph_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending() -> anyhow::Result<()> {
    use kanban_entity::{EntityUri, Predicate};
    use kanban_graph::{OxigraphStore, RelationGraph};

    let temp = TempDb::new("graph_rebuild_keeps_store_dirty_while_other_board_outbox_is_pending")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "second", "b_second")?;
    let default_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default board graph task"),
    )?;
    let second_task = create_task(
        &temp.path,
        "second",
        "tester",
        CreateTask::ready("second board graph task"),
    )?;

    kanban_sqlite::rebuild_graph_store(&temp.path, "default")?;
    let graph = OxigraphStore::open(kanban_local::graph_store_path(temp.path.clone()))?;
    assert_eq!(
        graph
            .neighbors(
                &EntityUri::task(&default_task.id),
                Some(Predicate::BelongsToBoard),
                10,
            )?
            .len(),
        1
    );

    assert_eq!(
        graph_outbox_statuses_for_board(&temp.path, "default")?,
        vec!["done"]
    );
    assert_eq!(
        graph_outbox_statuses_for_board(&temp.path, "second")?,
        vec!["pending"]
    );
    let derived = derived_store_statuses(&temp.path)?;
    let graph = derived
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .ok_or_else(|| test_error("missing oxigraph_relations derived store status"))?;
    assert!(graph.dirty, "second board still has pending graph outbox");

    kanban_sqlite::rebuild_graph_store(&temp.path, "second")?;
    let graph = OxigraphStore::open(kanban_local::graph_store_path(temp.path.clone()))?;
    assert_eq!(
        graph
            .neighbors(
                &EntityUri::task(&default_task.id),
                Some(Predicate::BelongsToBoard),
                10,
            )?
            .len(),
        1,
        "rebuilding the second board must preserve the first board graph"
    );
    assert_eq!(
        graph
            .neighbors(
                &EntityUri::task(&second_task.id),
                Some(Predicate::BelongsToBoard),
                10,
            )?
            .len(),
        1
    );

    assert_eq!(
        graph_outbox_statuses_for_board(&temp.path, "second")?,
        vec!["done"]
    );
    let derived = derived_store_statuses(&temp.path)?;
    let graph = derived
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .ok_or_else(|| test_error("missing oxigraph_relations derived store status"))?;
    assert!(!graph.dirty);
    let default_status = kanban_sqlite::graph_store_status(&temp.path, "default")?;
    assert!(
        default_status.message.contains("lag=0"),
        "default board has no unfinished graph outbox even though the shared watermark advanced: {}",
        default_status.message
    );
    Ok(())
}

#[test]
fn graph_rebuild_persists_board_and_dependency_relations() -> anyhow::Result<()> {
    use kanban_entity::{EntityUri, Predicate};
    use kanban_graph::{OxigraphStore, RelationGraph};

    let temp = TempDb::new("graph_rebuild_persists_board_and_dependency_relations")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("graph parent"),
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("graph child"),
    )?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;

    let snapshot = kanban_sqlite::graph_relation_snapshot(&temp.path, "default")?;
    assert!(
        snapshot.iter().any(
            |relation| relation.subject_uri == EntityUri::task(&child.id)
                && relation.predicate == Predicate::DependsOn
                && relation.object_uri == EntityUri::task(&parent.id)
        ),
        "SQLite relation snapshot should mirror the authoritative dependency edge"
    );

    kanban_sqlite::rebuild_graph_store(&temp.path, "default")?;

    let graph = OxigraphStore::open(kanban_local::graph_store_path(temp.path.clone()))?;
    let child_uri = EntityUri::task(&child.id);
    let dependency_neighbors = graph.neighbors(&child_uri, Some(Predicate::DependsOn), 10)?;
    assert_eq!(dependency_neighbors.len(), 1);
    assert_eq!(
        dependency_neighbors[0].object_uri,
        EntityUri::task(&parent.id)
    );

    let board_neighbors = graph.neighbors(&child_uri, Some(Predicate::BelongsToBoard), 10)?;
    assert_eq!(board_neighbors.len(), 1);
    assert_eq!(
        board_neighbors[0].object_uri,
        EntityUri::board(&child.board_id)
    );
    Ok(())
}

#[test]
fn graph_store_can_be_deleted_and_recreated_from_sqlite_snapshot() -> anyhow::Result<()> {
    use kanban_entity::{EntityUri, Predicate};
    use kanban_graph::{OxigraphStore, RelationGraph};

    let temp = TempDb::new("graph_store_can_be_deleted_and_recreated_from_sqlite_snapshot")?;
    init_database(&temp.path, "tester")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("graph rebuild equality parent"),
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("graph rebuild equality child"),
    )?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    let snapshot = kanban_sqlite::graph_relation_snapshot(&temp.path, "default")?;
    let expected = relation_keys(&snapshot);

    kanban_sqlite::rebuild_graph_store(&temp.path, "default")?;
    let graph_path = kanban_local::graph_store_path(temp.path.clone());
    assert_graph_contains_relation_keys(&graph_path, &snapshot, &expected)?;

    if graph_path.is_dir() {
        std::fs::remove_dir_all(&graph_path)?;
    } else if graph_path.exists() {
        std::fs::remove_file(&graph_path)?;
    }

    kanban_sqlite::rebuild_graph_store(&temp.path, "default")?;
    assert_graph_contains_relation_keys(&graph_path, &snapshot, &expected)?;
    let status = kanban_sqlite::graph_store_status(&temp.path, "default")?;
    assert!(
        status.message.contains("dirty=false"),
        "recreated graph should leave the derived store clean: {}",
        status.message
    );

    let child_uri = EntityUri::task(&child.id);
    let graph = OxigraphStore::open(graph_path)?;
    assert_eq!(
        graph
            .neighbors(&child_uri, Some(Predicate::DependsOn), 10)?
            .len(),
        1
    );
    Ok(())
}

#[test]
fn graph_rebuild_failure_does_not_mutate_canonical_task_label_or_ledger_state() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("graph_rebuild_failure_does_not_mutate_canonical_task_label_or_ledger_state")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("graph failure canonical isolation"),
    )?;
    create_label(
        &temp.path,
        "default",
        CreateLabel {
            name: "graph-boundary".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "graph-boundary".to_owned(),
            description: Some("Graph boundary regression label".to_owned()),
            applies_when: vec!["testing graph derived-store isolation".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let task_before = get_task(&temp.path, "default", &task.id)?;
    let labels_before = list_labels(&temp.path, "default")?;
    let semantics_before = get_label_semantics(&temp.path, "default", "graph-boundary")?;
    let atoms_before = list_label_atoms(&temp.path, "default")?;
    let canonical_counts_before = canonical_counts(&temp.path)?;

    let graph_path = kanban_local::graph_store_path(temp.path.clone());
    if graph_path.is_dir() {
        std::fs::remove_dir_all(&graph_path)?;
    }
    if let Some(parent) = graph_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&graph_path, "not a graph directory")?;

    let error = result_err(kanban_sqlite::rebuild_graph_store(&temp.path, "default"))?;
    assert!(
        error.to_string().contains("File exists")
            || error.to_string().contains("Not a directory")
            || error.to_string().contains("graph"),
        "unexpected graph rebuild error: {error}"
    );

    assert_eq!(get_task(&temp.path, "default", &task.id)?, task_before);
    assert_eq!(list_labels(&temp.path, "default")?, labels_before);
    assert_eq!(
        get_label_semantics(&temp.path, "default", "graph-boundary")?,
        semantics_before
    );
    assert_eq!(list_label_atoms(&temp.path, "default")?, atoms_before);
    assert_eq!(canonical_counts(&temp.path)?, canonical_counts_before);

    let derived = derived_store_statuses(&temp.path)?;
    let graph_status = derived
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .ok_or_else(|| test_error("missing oxigraph_relations derived store status"))?;
    assert!(graph_status.dirty);
    assert!(graph_status.last_error.is_some());
    Ok(())
}

fn relation_keys(relations: &[kanban_entity::Relation]) -> Vec<(String, String, String)> {
    let mut keys = relations
        .iter()
        .map(|relation| {
            (
                relation.subject_uri.as_str().to_owned(),
                relation.predicate.as_str().to_owned(),
                relation.object_uri.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn assert_graph_contains_relation_keys(
    graph_path: &std::path::Path,
    snapshot: &[kanban_entity::Relation],
    expected: &[(String, String, String)],
) -> anyhow::Result<()> {
    use kanban_entity::{EntityUri, Predicate};
    use kanban_graph::{OxigraphStore, RelationGraph};

    let graph = OxigraphStore::open(graph_path)?;
    let mut actual = Vec::new();
    for relation in snapshot {
        let neighbors = graph.neighbors(&relation.subject_uri, Some(relation.predicate), 100)?;
        for neighbor in neighbors {
            actual.push((
                neighbor.subject_uri.as_str().to_owned(),
                neighbor.predicate.as_str().to_owned(),
                neighbor.object_uri.as_str().to_owned(),
            ));
        }
    }
    actual.sort();
    actual.dedup();
    assert_eq!(actual, expected);

    let child_edges = expected
        .iter()
        .filter(|(_, predicate, _)| predicate == Predicate::DependsOn.as_str())
        .collect::<Vec<_>>();
    for (subject, _, object) in child_edges {
        let neighbors = graph.neighbors(
            &EntityUri::new(subject.clone())?,
            Some(Predicate::DependsOn),
            100,
        )?;
        assert!(
            neighbors
                .iter()
                .any(|relation| relation.object_uri.as_str() == object),
            "missing rebuilt dependency edge {subject} -> {object}"
        );
    }
    Ok(())
}

fn canonical_counts(path: &std::path::Path) -> anyhow::Result<Vec<(&'static str, i64)>> {
    let conn = connect_file(path)?;
    [
        "tasks",
        "labels",
        "label_semantics",
        "label_atoms",
        "label_ontology_actions",
        "label_ontology_signals",
    ]
    .into_iter()
    .map(|table| {
        let count = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })?;
        Ok((table, count))
    })
    .collect()
}
