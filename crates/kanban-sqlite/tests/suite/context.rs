use crate::common::*;

#[test]
fn context_broker_hydrates_subject_and_reports_disabled_derived_stores() -> anyhow::Result<()> {
    let temp = TempDb::new("context_broker_hydrates_subject_and_reports_disabled_derived_stores")?;
    init_database(&temp.path, "tester")?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
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
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

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
    )?;

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
    assert!(
        pack.degraded
            .iter()
            .any(|marker| marker == "graph_disabled")
    );
    assert!(
        pack.degraded
            .iter()
            .any(|marker| marker == "vector_disabled")
    );
    Ok(())
}

#[test]
fn context_broker_rejects_zero_max_items_and_counts_subject_toward_budget() -> anyhow::Result<()> {
    let temp =
        TempDb::new("context_broker_rejects_zero_max_items_and_counts_subject_toward_budget")?;
    init_database(&temp.path, "tester")?;
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("budget context subject"),
    )?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("budget context related"),
    )?;

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
    )?;
    assert_eq!(pack.items.len(), 1);
    assert_eq!(
        pack.items[0].entity_uri,
        kanban_entity::EntityUri::task(&subject.id)
    );

    let error = result_err(build_context_pack(
        &temp.path,
        "default",
        &subject.id,
        kanban_context::ContextPolicy {
            lexical_limit: 5,
            graph_limit: 5,
            vector_limit: 5,
            max_items: 0,
        },
    ))?;
    assert!(error.to_string().contains("max_items must be >= 1"));
    Ok(())
}

#[test]
fn context_broker_reports_graph_dirty_and_stale_before_sync() -> anyhow::Result<()> {
    let temp = TempDb::new("context_broker_reports_graph_dirty_and_stale_before_sync")?;
    init_database(&temp.path, "tester")?;
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("dirty graph context subject"),
    )?;

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
    )?;

    assert!(pack.degraded.iter().any(|marker| marker == "graph_dirty"));
    assert!(pack.degraded.iter().any(|marker| marker == "graph_stale"));
    Ok(())
}

#[test]
fn context_broker_reports_vector_query_error_without_failing_pack() -> anyhow::Result<()> {
    let temp = TempDb::new("context_broker_reports_vector_query_error_without_failing_pack")?;
    init_database(&temp.path, "tester")?;
    let subject = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("query failing vector context subject"),
    )?;
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
    )?;

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
    Ok(())
}
