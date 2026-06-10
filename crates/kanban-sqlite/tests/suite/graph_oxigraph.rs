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
