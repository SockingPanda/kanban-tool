use crate::common::*;

#[test]
fn task_events_fan_out_target_specific_outbox_and_mark_derived_stores_dirty() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("task_events_fan_out_target_specific_outbox_and_mark_derived_stores_dirty")?;
    init_database(&temp.path, "tester")?;

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("outbox fanout"),
    )?;

    let jobs = list_outbox(
        &temp.path,
        kanban_sqlite::OutboxListOptions {
            status: Some("pending".to_owned()),
            limit: 10,
        },
    )?;
    assert_eq!(jobs.len(), 3);
    assert_eq!(
        jobs.iter()
            .map(|job| job.target.as_str())
            .collect::<Vec<_>>(),
        vec!["tantivy", "oxigraph", "lancedb"]
    );
    assert!(
        jobs.iter()
            .all(|job| job.entity_uri == format!("kb://task/{}", task.id))
    );

    let statuses = derived_store_statuses(&temp.path)?;
    for store in ["tantivy_tasks", "oxigraph_relations", "lancedb_chunks"] {
        let status = statuses
            .iter()
            .find(|status| status.store_name == store)
            .ok_or_else(|| test_error(format!("missing derived store status for {store}")))?;
        assert!(status.dirty, "{store} should be dirty");
        assert_eq!(status.last_event_id, 0);
    }
    Ok(())
}
