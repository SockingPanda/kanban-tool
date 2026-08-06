use crate::{
    EntityUpsertInput, GraphNeighborsOptions, RelationPredicateInput, RelationUpsertInput,
    TaskNeighborhoodOptions,
};

use crate::test_support::{create_input, store};

#[tokio::test]
async fn graph_reads_canonical_relations_with_cycle_safe_bounded_bfs() {
    let (_directory, store, _path) = store("graph-bounded").await;
    store.initialize().await.expect("initialize");

    for (id, title) in [("t_graph_a", "A"), ("t_graph_b", "B"), ("t_graph_c", "C")] {
        store
            .create_task("default", create_input(id, None, title))
            .await
            .expect("create graph task");
        store
            .upsert_entity(EntityUpsertInput {
                uri: format!("kb://task/{id}"),
                kind: "task".to_owned(),
                source_table: "tasks".to_owned(),
                source_id: id.to_owned(),
                board: Some("default".to_owned()),
                task_id: Some(id.to_owned()),
                title: Some(title.to_owned()),
                summary: None,
                content_hash: None,
                archived_at: None,
            })
            .await
            .expect("upsert graph entity");
    }
    store
        .upsert_relation_predicate(RelationPredicateInput {
            name: "depends_on".to_owned(),
            domain_kind: Some("task".to_owned()),
            range_kind: Some("task".to_owned()),
            cardinality: "many".to_owned(),
            authoritative_store: "turso".to_owned(),
            description: None,
        })
        .await
        .expect("upsert predicate");

    for (subject, object) in [("t_graph_a", "t_graph_b"), ("t_graph_b", "t_graph_c")] {
        store
            .upsert_relation(RelationUpsertInput {
                subject_uri: format!("kb://task/{subject}"),
                predicate: "depends_on".to_owned(),
                object_uri: format!("kb://task/{object}"),
                graph_uri: "kb://graph/default".to_owned(),
                board: Some("default".to_owned()),
                authoritative_store: "turso".to_owned(),
                source_table: Some("tasks".to_owned()),
                source_id: Some(subject.to_owned()),
                source_event_id: None,
                metadata_json: "{}".to_owned(),
            })
            .await
            .expect("upsert relation");
    }

    let neighbors = store
        .graph_neighbors(GraphNeighborsOptions {
            board: "default".to_owned(),
            entity_uri: "kb://task/t_graph_a".to_owned(),
            predicate: None,
            limit: 50,
        })
        .await
        .expect("graph neighbors");
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].object_uri, "kb://task/t_graph_b");

    // 有意添加反向事实：BFS 必须在循环事实图上终止，并受请求的深度和节点上限约束。
    store
        .upsert_relation(RelationUpsertInput {
            subject_uri: "kb://task/t_graph_c".to_owned(),
            predicate: "depends_on".to_owned(),
            object_uri: "kb://task/t_graph_a".to_owned(),
            graph_uri: "kb://graph/default".to_owned(),
            board: Some("default".to_owned()),
            authoritative_store: "turso".to_owned(),
            source_table: Some("tasks".to_owned()),
            source_id: Some("t_graph_c".to_owned()),
            source_event_id: None,
            metadata_json: "{}".to_owned(),
        })
        .await
        .expect("upsert cycle relation");
    let neighborhood = store
        .task_neighborhood(
            "t_graph_a",
            TaskNeighborhoodOptions {
                depth: 8,
                limit_nodes: 2,
                include_archived_context: false,
            },
        )
        .await
        .expect("bounded neighborhood");
    assert_eq!(neighborhood.nodes.len(), 2);
    assert!(neighborhood.meta.truncated);
    assert!(
        neighborhood
            .nodes
            .iter()
            .any(|node| node.task.id == "t_graph_a")
    );
}

#[tokio::test]
async fn graph_neighbors_enforces_board_isolation() {
    let (_directory, store, _path) = store("graph-board-isolation").await;
    store.initialize().await.expect("initialize");
    let connection = store.connection().await.expect("connection");
    connection
        .execute(
            "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_graph_other', 'graph-other', 'Other', 1, 1)",
            (),
        )
        .await
        .expect("other board");
    for (id, board) in [
        ("t_graph_default", "default"),
        ("t_graph_other", "graph-other"),
    ] {
        store
            .create_task(board, create_input(id, None, id))
            .await
            .expect("create board task");
        store
            .upsert_entity(EntityUpsertInput {
                uri: format!("kb://task/{id}"),
                kind: "task".to_owned(),
                source_table: "tasks".to_owned(),
                source_id: id.to_owned(),
                board: Some(board.to_owned()),
                task_id: Some(id.to_owned()),
                title: None,
                summary: None,
                content_hash: None,
                archived_at: None,
            })
            .await
            .expect("upsert board entity");
    }
    let error = store
        .graph_neighbors(GraphNeighborsOptions {
            board: "default".to_owned(),
            entity_uri: "kb://task/t_graph_other".to_owned(),
            predicate: None,
            limit: 10,
        })
        .await
        .expect_err("cross-board entity must be hidden");
    assert!(
        matches!(error, crate::StoreError::EntityNotFound(uri) if uri == "kb://task/t_graph_other")
    );
}

#[tokio::test]
async fn graph_rebuild_materializes_task_entities_and_sync_consumes_jobs() {
    let (_directory, store, _path) = store("graph-maintenance").await;
    store.initialize().await.expect("initialize");
    store
        .create_task(
            "default",
            create_input("t_graph_maintenance", None, "Maintenance"),
        )
        .await
        .expect("create task");

    let rebuilt = store.graph_rebuild("default").await.expect("rebuild graph");
    assert_eq!(rebuilt.mode, "rebuild");
    assert_eq!(rebuilt.validated_tasks, 1);
    assert!(rebuilt.validated_entities >= 2);
    assert!(rebuilt.validated_relations >= 1);
    assert!(!rebuilt.generation.is_empty());
    assert!(!rebuilt.fingerprint.is_empty());

    let synced_before_jobs = store.graph_sync("default").await.expect("sync graph");
    assert_eq!(synced_before_jobs.validated_tasks, rebuilt.validated_tasks);
    assert_eq!(
        synced_before_jobs.validated_entities,
        rebuilt.validated_entities
    );
    assert_eq!(
        synced_before_jobs.validated_relations,
        rebuilt.validated_relations
    );
    assert_eq!(synced_before_jobs.fingerprint, rebuilt.fingerprint);

    let connection = store.connection().await.expect("connection");
    connection
        .execute(
            "INSERT INTO projection_jobs(board_id, target, entity_uri, operation, payload_json, created_at, updated_at) VALUES ('b_default', 'relations', 'kb://task/t_graph_maintenance', 'upsert', '{}', 1, 1)",
            (),
        )
        .await
        .expect("pending graph job");
    let synced = store.graph_sync("default").await.expect("sync graph");
    assert_eq!(synced.mode, "sync");
    assert_eq!(synced.consumed_jobs, 1);
    assert_eq!(synced.pending_jobs, 0);
    assert_eq!(synced.validated_tasks, 1);
}
