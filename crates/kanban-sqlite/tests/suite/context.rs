use crate::common::*;

#[test]
fn context_broker_hydrates_subject_and_reports_disabled_derived_stores() {
    let temp = TempDb::new("context_broker_hydrates_subject_and_reports_disabled_derived_stores");
    init_database(&temp.path, "tester").unwrap();
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "context broker source".into(),
            description: Some("ready spec broker-needle".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();
    let related = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "related context broker source".into(),
            description: Some("ready spec broker-needle".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    let pack = build_context_pack(
        &temp.path,
        "default",
        &subject.id,
        kanban_context::ContextPolicy {
            lexical_limit: 5,
            graph_limit: 5,
            vector_limit: 5,
            max_items: 10,
        },
    )
    .unwrap();

    assert_eq!(pack.subject, kanban_entity::EntityUri::task(&subject.id));
    assert_eq!(
        pack.items[0].entity_uri,
        kanban_entity::EntityUri::task(&subject.id)
    );
    assert_eq!(pack.items[0].source, "subject");
    assert!(
        pack.items
            .iter()
            .any(|item| item.entity_uri == kanban_entity::EntityUri::task(&related.id))
    );
    #[cfg(not(feature = "graph-oxigraph"))]
    assert!(
        pack.degraded
            .iter()
            .any(|marker| marker == "graph_disabled")
    );
    #[cfg(feature = "graph-oxigraph")]
    assert!(
        !pack
            .degraded
            .iter()
            .any(|marker| marker == "graph_disabled")
    );
    #[cfg(not(feature = "vector-lancedb"))]
    assert!(
        pack.degraded
            .iter()
            .any(|marker| marker == "vector_disabled")
    );
    #[cfg(feature = "vector-lancedb")]
    assert!(pack.degraded.iter().any(|marker| marker == "vector_dirty"));
}

#[test]
fn context_broker_rejects_zero_max_items_and_counts_subject_toward_budget() {
    let temp =
        TempDb::new("context_broker_rejects_zero_max_items_and_counts_subject_toward_budget");
    init_database(&temp.path, "tester").unwrap();
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("budget context subject"),
    )
    .unwrap();
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("budget context related"),
    )
    .unwrap();

    let pack = build_context_pack(
        &temp.path,
        "default",
        &subject.id,
        kanban_context::ContextPolicy {
            lexical_limit: 5,
            graph_limit: 5,
            vector_limit: 5,
            max_items: 1,
        },
    )
    .unwrap();
    assert_eq!(pack.items.len(), 1);
    assert_eq!(
        pack.items[0].entity_uri,
        kanban_entity::EntityUri::task(&subject.id)
    );

    let error = build_context_pack(
        &temp.path,
        "default",
        &subject.id,
        kanban_context::ContextPolicy {
            lexical_limit: 5,
            graph_limit: 5,
            vector_limit: 5,
            max_items: 0,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("max_items must be >= 1"));
}

#[cfg(feature = "graph-oxigraph")]
#[test]
fn context_broker_reports_graph_dirty_and_stale_before_sync() {
    let temp = TempDb::new("context_broker_reports_graph_dirty_and_stale_before_sync");
    init_database(&temp.path, "tester").unwrap();
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("dirty graph context subject"),
    )
    .unwrap();

    let pack = build_context_pack(
        &temp.path,
        "default",
        &subject.id,
        kanban_context::ContextPolicy {
            lexical_limit: 5,
            graph_limit: 5,
            vector_limit: 0,
            max_items: 10,
        },
    )
    .unwrap();

    assert!(pack.degraded.iter().any(|marker| marker == "graph_dirty"));
    assert!(pack.degraded.iter().any(|marker| marker == "graph_stale"));
}

#[cfg(feature = "graph-oxigraph")]
#[test]
fn context_broker_reports_graph_error_diagnostic_without_failing_pack() {
    let temp = TempDb::new("context_broker_reports_graph_error_diagnostic_without_failing_pack");
    init_database(&temp.path, "tester").unwrap();
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("broken graph context subject"),
    )
    .unwrap();
    let graph_path = kanban_local::graph_store_path(temp.path.clone());
    std::fs::create_dir_all(graph_path.parent().unwrap()).unwrap();
    std::fs::write(&graph_path, "not a graph directory").unwrap();

    let pack = build_context_pack(
        &temp.path,
        "default",
        &subject.id,
        kanban_context::ContextPolicy {
            lexical_limit: 5,
            graph_limit: 5,
            vector_limit: 0,
            max_items: 10,
        },
    )
    .unwrap();

    assert_eq!(
        pack.items[0].entity_uri,
        kanban_entity::EntityUri::task(&subject.id)
    );
    assert!(pack.degraded.iter().any(|marker| marker == "graph_error"));
    assert!(pack.diagnostics.iter().any(|diagnostic| {
        diagnostic.source == "graph"
            && diagnostic.code == "graph_error"
            && !diagnostic.message.is_empty()
            && diagnostic.message.len() <= 243
    }));
}

#[cfg(feature = "vector-lancedb")]
#[test]
fn context_broker_reports_vector_query_error_without_failing_pack() {
    let temp = TempDb::new("context_broker_reports_vector_query_error_without_failing_pack");
    init_database(&temp.path, "tester").unwrap();
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("query failing vector context subject"),
    )
    .unwrap();
    let store = QueryFailingVectorStore;

    let pack = kanban_sqlite::build_context_pack_with_vector_store(
        &temp.path,
        "default",
        &subject.id,
        kanban_context::ContextPolicy {
            lexical_limit: 5,
            graph_limit: 0,
            vector_limit: 5,
            max_items: 10,
        },
        &store,
    )
    .unwrap();

    assert_eq!(
        pack.items[0].entity_uri,
        kanban_entity::EntityUri::task(&subject.id)
    );
    assert!(pack.degraded.iter().any(|marker| marker == "vector_dirty"));
    assert!(pack.degraded.iter().any(|marker| marker == "vector_stale"));
    assert!(pack.degraded.iter().any(|marker| marker == "vector_error"));
    assert!(pack.diagnostics.iter().any(|diagnostic| {
        diagnostic.source == "vector"
            && diagnostic.code == "vector_error"
            && diagnostic.message.contains("query exploded")
            && diagnostic.message.len() <= 243
    }));
}
