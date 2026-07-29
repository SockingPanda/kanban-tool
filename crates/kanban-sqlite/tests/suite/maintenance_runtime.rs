use crate::common::*;

use kanban_sqlite::api::{
    MaintenanceMode, MaintenanceRunOptions, MaintenanceSession, maintenance_status,
};

#[test]
fn maintenance_owner_is_singleton_and_expired_token_cannot_release_successor() -> anyhow::Result<()>
{
    let temp = TempDb::new("maintenance_owner_singleton")?;
    init_database(&temp.path, "tester")?;

    let first = MaintenanceSession::start(
        &temp.path,
        "owner-a",
        MaintenanceMode::Continuous,
        MaintenanceRunOptions::default(),
    )?;
    let status = maintenance_status(&temp.path)?;
    assert!(status.maintenance_owner.active);
    assert_eq!(status.maintenance_owner.owner.as_deref(), Some("owner-a"));
    assert_eq!(status.maintenance_owner.mode.as_deref(), Some("continuous"));
    assert!(
        !serde_json::to_value(&status)?
            .to_string()
            .contains("pmlease_"),
        "operator status must not expose the owner lease token"
    );

    let conflict = result_err(MaintenanceSession::start(
        &temp.path,
        "owner-b",
        MaintenanceMode::Once,
        MaintenanceRunOptions::default(),
    ))?;
    assert!(matches!(conflict, KanbanError::Conflict(_)));

    connect_file(&temp.path)?.execute(
        "UPDATE projection_maintenance_owner SET lease_expires_at=0 WHERE singleton=1",
        [],
    )?;
    let second = MaintenanceSession::start(
        &temp.path,
        "owner-b",
        MaintenanceMode::Once,
        MaintenanceRunOptions::default(),
    )?;
    drop(first);
    let status = maintenance_status(&temp.path)?;
    assert!(status.maintenance_owner.active);
    assert_eq!(status.maintenance_owner.owner.as_deref(), Some("owner-b"));

    second.finish()?;
    let status = maintenance_status(&temp.path)?;
    assert!(!status.maintenance_owner.active);
    assert_eq!(status.maintenance_owner.owner, None);
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_bootstraps_db_scoped_multi_board_tantivy_and_catches_up() -> anyhow::Result<()> {
    use kanban_search::SearchQuery;
    use kanban_sqlite::api::{
        maintenance_run_once, rebuild_search_index, search_index_status, search_tasks,
    };

    let temp = TempDb::new("maintenance_multi_board_tantivy")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "other", "b_other")?;
    let default_task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default projection alpha"),
    )?;
    let other_task = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other projection beta"),
    )?;

    let first = maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    assert_eq!(first.database_instance_id.len(), 29);
    assert_eq!(first.stores.len(), 1);
    assert_eq!(first.stores[0].store_name, "tantivy_tasks");
    assert_eq!(first.stores[0].lifecycle_status, "ready");
    assert_eq!(first.stores[0].fallback_reason, None);

    let query = |board: &str, text: &str| SearchQuery {
        board: board.to_owned(),
        q: Some(text.to_owned()),
        statuses: Vec::new(),
        labels: Vec::new(),
        assignee: None,
        include_archived: false,
        limit: 20,
        offset: 0,
    };
    let default = search_tasks(&temp.path, query("default", "projection"))?;
    assert_eq!(default.meta.backend, "tantivy");
    assert_eq!(
        default.meta.database_instance_id.as_deref(),
        Some(first.database_instance_id.as_str())
    );
    assert_eq!(default.meta.protocol_version, Some(first.protocol_version));
    assert!(default.meta.generation.is_some());
    let healthy_generation = default.meta.generation.clone();
    assert_eq!(default.meta.resolved_board_id, default_task.board_id);
    assert_eq!(default.meta.fallback_reason, None);
    assert_eq!(
        default
            .hits
            .iter()
            .map(|hit| hit.task_id.as_str())
            .collect::<Vec<_>>(),
        vec![default_task.id.as_str()]
    );
    let other = search_tasks(&temp.path, query("other", "projection"))?;
    assert_eq!(other.meta.backend, "tantivy");
    assert_eq!(other.meta.resolved_board_id, "b_other");
    assert_eq!(other.meta.fallback_reason, None);
    assert_eq!(
        other
            .hits
            .iter()
            .map(|hit| hit.task_id.as_str())
            .collect::<Vec<_>>(),
        vec![other_task.id.as_str()]
    );

    let new_other = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other projection gamma"),
    )?;
    let fallback = search_tasks(&temp.path, query("other", "gamma"))?;
    assert_eq!(fallback.meta.backend, "sqlite");
    assert!(fallback.meta.stale);
    assert_eq!(
        fallback.meta.database_instance_id.as_deref(),
        Some(first.database_instance_id.as_str())
    );
    assert_eq!(fallback.meta.protocol_version, Some(first.protocol_version));
    assert_eq!(
        fallback.meta.generation.as_deref(),
        healthy_generation.as_deref()
    );
    assert_eq!(fallback.meta.resolved_board_id, "b_other");
    assert!(fallback.meta.fallback_reason.is_some());
    assert_eq!(fallback.hits[0].task_id, new_other.id);

    let second =
        maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    assert_eq!(second.stores[0].action, "batch_applied");
    let caught_up = search_tasks(&temp.path, query("other", "gamma"))?;
    assert_eq!(caught_up.meta.backend, "tantivy");
    assert_eq!(caught_up.hits[0].task_id, new_other.id);
    let status = search_index_status(&temp.path, "other")?;
    assert_eq!(status.backend, "tantivy");
    assert!(!status.stale);

    let legacy = result_err(rebuild_search_index(&temp.path, "default"))?;
    assert!(legacy.to_string().contains("maintenance v2"));
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_acknowledges_board_level_noops_after_v2_activation() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    let temp = TempDb::new("maintenance_board_level_noops")?;
    init_database(&temp.path, "tester")?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let before = maintenance_status(&temp.path)?;
    let initial_checkpoint = before
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status")
        .checkpoint_cursor;

    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "post-v2".to_owned(),
            name: "Post v2".to_owned(),
            description: None,
        },
    )?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let after_create = maintenance_status(&temp.path)?;
    let after_create_checkpoint = after_create
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status")
        .checkpoint_cursor;
    let store = after_create
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    assert!(store.checkpoint_cursor > initial_checkpoint);
    assert_eq!((store.pending, store.running, store.failed), (0, 0, 0));

    archive_board(&temp.path, "post-v2", "tester")?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let after_archive = maintenance_status(&temp.path)?;
    let store = after_archive
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    assert!(store.checkpoint_cursor > after_create_checkpoint);
    assert_eq!((store.pending, store.running, store.failed), (0, 0, 0));
    assert_eq!(store.lifecycle_status, "ready");
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_rebuild_keeps_previous_tantivy_generation() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_rebuild_store, maintenance_run_once, projection_status};

    let temp = TempDb::new("maintenance_tantivy_previous_generation")?;
    init_database(&temp.path, "tester")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("generation one"),
    )?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let before = projection_status(&temp.path)?;
    let first = before
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status")
        .active_generation
        .clone()
        .expect("first generation");

    maintenance_rebuild_store(
        &temp.path,
        "runtime-test",
        "tantivy_tasks",
        MaintenanceRunOptions::default(),
    )?;
    let after = projection_status(&temp.path)?;
    let store = after
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    assert_ne!(store.active_generation.as_deref(), Some(first.as_str()));
    assert_eq!(store.previous_generation.as_deref(), Some(first.as_str()));
    assert!(
        temp.dir
            .join("index/v2/tantivy_tasks/generations")
            .join(first)
            .join("published")
            .is_file()
    );
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_status_detects_and_run_repairs_missing_physical_generation() -> anyhow::Result<()> {
    use kanban_search::SearchQuery;
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    let temp = TempDb::new("maintenance_repairs_missing_tantivy_generation")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("physical recovery"),
    )?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let before = maintenance_status(&temp.path)?;
    let store = before
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    let generation = store.active_generation.clone().expect("active generation");
    std::fs::remove_dir_all(
        temp.dir
            .join("index/v2/tantivy_tasks/generations")
            .join(&generation),
    )?;

    let degraded = maintenance_status(&temp.path)?;
    let store = degraded
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    assert_eq!(store.lifecycle_status, "error");
    assert_eq!(
        store.fallback_reason.as_deref(),
        Some("physical_generation_unavailable")
    );
    assert!(
        store
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("is missing"))
    );
    let fallback = kanban_sqlite::api::search_tasks(
        &temp.path,
        SearchQuery {
            board: "default".to_owned(),
            q: Some("physical recovery".to_owned()),
            statuses: Vec::new(),
            labels: Vec::new(),
            assignee: None,
            include_archived: false,
            limit: 20,
            offset: 0,
        },
    )?;
    assert_eq!(fallback.meta.backend, "sqlite");
    assert_eq!(
        fallback.meta.database_instance_id.as_deref(),
        Some(degraded.database_instance_id.as_str())
    );
    assert_eq!(
        fallback.meta.protocol_version,
        Some(degraded.protocol_version)
    );
    assert_eq!(
        fallback.meta.generation.as_deref(),
        Some(generation.as_str())
    );
    assert_eq!(fallback.meta.resolved_board_id, task.board_id);
    assert_eq!(
        fallback.meta.fallback_reason.as_deref(),
        Some("physical_generation_unavailable")
    );

    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let repaired = maintenance_status(&temp.path)?;
    let store = repaired
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    assert_eq!(store.lifecycle_status, "ready");
    assert_eq!(store.fallback_reason, None);
    assert_ne!(
        store.active_generation.as_deref(),
        Some(generation.as_str())
    );
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn maintenance_quarantines_readable_metadata_mismatch_and_recovers() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    let temp = TempDb::new("maintenance_recovers_metadata_mismatch")?;
    init_database(&temp.path, "tester")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("metadata mismatch recovery"),
    )?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let before = maintenance_status(&temp.path)?;
    let store = before
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    let generation = store.active_generation.clone().expect("active generation");
    let generation_path = temp
        .dir
        .join("index/v2/tantivy_tasks/generations")
        .join(&generation);
    let metadata_path = generation_path.join("kb-projection-meta.json");
    let mut metadata: serde_json::Value = serde_json::from_slice(&std::fs::read(&metadata_path)?)?;
    metadata["database_instance_id"] = serde_json::json!("db_foreign_projection");
    std::fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)?;

    let degraded = maintenance_status(&temp.path)?;
    let store = degraded
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    assert_eq!(
        store.fallback_reason.as_deref(),
        Some("physical_generation_unavailable")
    );

    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let repaired = maintenance_status(&temp.path)?;
    let store = repaired
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    assert_eq!(store.lifecycle_status, "ready");
    assert_eq!(store.fallback_reason, None);
    assert_ne!(
        store.active_generation.as_deref(),
        Some(generation.as_str())
    );
    assert!(generation_path.is_dir(), "recovery must preserve evidence");
    assert!(
        !generation_path.join("published").exists(),
        "mismatched evidence must be quarantined from the active set"
    );
    assert_eq!(store.previous_generation, None);
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn corrupted_tantivy_artifact_evidence_forces_every_search_surface_to_sqlite() -> anyhow::Result<()>
{
    use kanban_search::SearchQuery;
    use kanban_sqlite::api::{
        maintenance_run_once, maintenance_status, search_index_status, search_tasks,
    };

    for (case, field, value) in [
        (
            "provider_fingerprint",
            "provider_fingerprint",
            "tantivy-tasks-v2-corrupt",
        ),
        ("artifact_fingerprint", "fingerprint", "fnv64:corrupt"),
        ("coverage_digest", "canonical_digest", "fnv64:corrupt"),
    ] {
        let temp = TempDb::new(&format!("maintenance_corrupt_{case}"))?;
        init_database(&temp.path, "tester")?;
        let task = create_task(
            &temp.path,
            "default",
            "tester",
            CreateTask::ready(format!("artifact evidence {case}")),
        )?;
        maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
        let before = maintenance_status(&temp.path)?;
        let generation = before
            .stores
            .iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy status")
            .active_generation
            .clone()
            .expect("active generation");
        let metadata_path = temp
            .dir
            .join("index/v2/tantivy_tasks/generations")
            .join(&generation)
            .join("kb-projection-meta.json");
        let mut metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&metadata_path)?)?;
        metadata[field] = serde_json::json!(value);
        std::fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)?;

        let degraded = maintenance_status(&temp.path)?;
        let store = degraded
            .stores
            .iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy status");
        assert_eq!(
            store.fallback_reason.as_deref(),
            Some("physical_generation_unavailable"),
            "{case}: maintenance status must distrust corrupt physical evidence"
        );

        let status = search_index_status(&temp.path, "default")?;
        assert_eq!(status.backend, "sqlite", "{case}");
        assert!(status.stale, "{case}");
        assert_eq!(
            status.fallback_reason.as_deref(),
            Some("physical_generation_unavailable"),
            "{case}"
        );

        let results = search_tasks(
            &temp.path,
            SearchQuery {
                board: "default".to_owned(),
                q: Some("artifact evidence".to_owned()),
                statuses: Vec::new(),
                labels: Vec::new(),
                assignee: None,
                include_archived: false,
                limit: 20,
                offset: 0,
            },
        )?;
        assert_eq!(results.meta.backend, "sqlite", "{case}");
        assert!(results.meta.stale, "{case}");
        assert_eq!(
            results.meta.fallback_reason.as_deref(),
            Some("physical_generation_unavailable"),
            "{case}"
        );
        assert_eq!(results.hits[0].task_id, task.id, "{case}");
    }
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn unmappable_tantivy_delivery_fails_without_advancing_checkpoint() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    let temp = TempDb::new("maintenance_unmappable_tantivy_delivery")?;
    init_database(&temp.path, "tester")?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("unmappable delivery"),
    )?;
    let before = maintenance_status(&temp.path)?;
    let checkpoint = before
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status")
        .checkpoint_cursor;
    connect_file(&temp.path)?.execute(
        "UPDATE projection_deliveries
         SET entity_uri='kb://unknown/unmappable',source_event_id=NULL
         WHERE id=(
           SELECT id FROM projection_deliveries
           WHERE store_name='tantivy_tasks' AND status='pending'
           ORDER BY cursor LIMIT 1
         )",
        [],
    )?;

    let error = result_err(maintenance_run_once(
        &temp.path,
        "runtime-test",
        MaintenanceRunOptions::default(),
    ))?;
    assert!(error.to_string().contains("cannot be mapped"));
    let after = maintenance_status(&temp.path)?;
    let store = after
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy status");
    assert_eq!(store.checkpoint_cursor, checkpoint);
    assert_eq!(store.failed, 1);
    Ok(())
}
