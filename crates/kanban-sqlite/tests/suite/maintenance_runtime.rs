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
    let first_tantivy = first
        .stores
        .iter()
        .find(|store| store.store_name == "tantivy_tasks")
        .expect("Tantivy is enabled");
    assert_eq!(first_tantivy.lifecycle_status, "ready");
    assert_eq!(first_tantivy.fallback_reason, None);

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
    assert_eq!(
        second
            .stores
            .iter()
            .find(|store| store.store_name == "tantivy_tasks")
            .expect("Tantivy is enabled")
            .action,
        "batch_applied"
    );
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

#[cfg(all(feature = "tantivy-backend", feature = "oxigraph-backend"))]
#[test]
fn maintenance_run_reports_every_enabled_db_scoped_store() -> anyhow::Result<()> {
    use kanban_sqlite::api::maintenance_run_once;

    let temp = TempDb::new("maintenance_reports_enabled_stores")?;
    init_database(&temp.path, "tester")?;
    let report =
        maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    assert_eq!(
        report
            .stores
            .iter()
            .map(|store| store.store_name.as_str())
            .collect::<Vec<_>>(),
        vec!["tantivy_tasks", "oxigraph_relations"]
    );
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

#[cfg(feature = "oxigraph-backend")]
#[test]
fn maintenance_bootstraps_db_scoped_multi_board_oxigraph_and_removes_relations()
-> anyhow::Result<()> {
    use kanban_entity::{EntityUri, Predicate};
    use kanban_graph::RelationGraph;
    use kanban_graph_oxigraph::OxigraphStore;
    use kanban_sqlite::api::{
        add_dependency, maintenance_run_once, maintenance_status, remove_dependency,
    };

    let temp = TempDb::new("maintenance_multi_board_oxigraph")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "other", "b_other")?;
    let parent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default graph parent"),
    )?;
    let child = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("default graph child"),
    )?;
    let other = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other graph task"),
    )?;
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;

    maintenance_run_once(
        &temp.path,
        "oxigraph-runtime-test",
        MaintenanceRunOptions::default(),
    )?;
    let status = maintenance_status(&temp.path)?;
    let oxigraph = status
        .stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph store is seeded");
    assert_eq!(oxigraph.lifecycle_status, "ready");
    let generation = oxigraph
        .active_generation
        .as_deref()
        .expect("Oxigraph generation is active");
    let root = kanban_local::projection_store_root_path(&temp.path, "oxigraph_relations")?;
    let graph = OxigraphStore::open(root.join("generations").join(generation))?;
    let child_uri = EntityUri::new(format!("kb://task/{}", child.id))?;
    let relations = graph.neighbors(&child_uri, None, 20)?;
    assert!(relations.iter().any(|relation| {
        relation.predicate == Predicate::DependsOn
            && relation.object_uri.as_str() == format!("kb://task/{}", parent.id)
    }));
    assert!(relations.iter().all(|relation| {
        relation.object_uri.as_str() != format!("kb://task/{}", other.id)
            && relation.object_uri.as_str() != "kb://board/b_other"
    }));

    remove_dependency(&temp.path, "default", "tester", &parent.id, &child.id)?;
    maintenance_run_once(
        &temp.path,
        "oxigraph-runtime-test",
        MaintenanceRunOptions::default(),
    )?;
    let graph = OxigraphStore::open(root.join("generations").join(generation))?;
    assert!(
        graph
            .neighbors(&child_uri, Some(Predicate::DependsOn), 20)?
            .is_empty()
    );
    Ok(())
}

#[cfg(feature = "oxigraph-backend")]
#[test]
fn maintenance_detects_tampered_oxigraph_content_and_recovers() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    let temp = TempDb::new("maintenance_tampered_oxigraph")?;
    init_database(&temp.path, "tester")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("tamper evidence"),
    )?;
    maintenance_run_once(
        &temp.path,
        "oxigraph-runtime-test",
        MaintenanceRunOptions::default(),
    )?;
    let ready = maintenance_status(&temp.path)?;
    let store = ready
        .stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status");
    let generation = store
        .active_generation
        .as_deref()
        .expect("active generation");
    let root = kanban_local::projection_store_root_path(&temp.path, "oxigraph_relations")?;
    let generation_path = root.join("generations").join(generation);
    std::fs::write(generation_path.join("relations.json"), b"[]")?;
    let metadata_path = generation_path.join("kb-projection-meta.json");
    let mut metadata: serde_json::Value = serde_json::from_slice(&std::fs::read(&metadata_path)?)?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in (2_u64).to_le_bytes().iter().chain(b"[]") {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    metadata["content_fingerprint"] = serde_json::json!(format!("fnv64:{hash:016x}"));
    std::fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)?;

    let degraded = maintenance_status(&temp.path)?;
    let store = degraded
        .stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status");
    assert_eq!(
        store.fallback_reason.as_deref(),
        Some("physical_generation_unavailable")
    );

    let recovered = maintenance_run_once(
        &temp.path,
        "oxigraph-runtime-test",
        MaintenanceRunOptions::default(),
    )?;
    assert_eq!(
        recovered
            .stores
            .iter()
            .find(|store| store.store_name == "oxigraph_relations")
            .expect("Oxigraph run")
            .action,
        "generation_recovered"
    );
    let ready = maintenance_status(&temp.path)?;
    let store = ready
        .stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status");
    assert_eq!(store.lifecycle_status, "ready");
    assert_eq!(store.fallback_reason, None);
    assert_ne!(store.active_generation.as_deref(), Some(generation));
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

#[cfg(feature = "oxigraph-backend")]
#[test]
fn unmappable_oxigraph_upsert_fails_without_advancing_checkpoint() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    let temp = TempDb::new("maintenance_unmappable_oxigraph_upsert")?;
    init_database(&temp.path, "tester")?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("unmappable graph delivery"),
    )?;
    let before = maintenance_status(&temp.path)?;
    let checkpoint = before
        .stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status")
        .checkpoint_cursor;
    connect_file(&temp.path)?.execute(
        "UPDATE projection_deliveries
         SET entity_uri='kb://unknown/unmappable',action='upsert'
         WHERE id=(
           SELECT id FROM projection_deliveries
           WHERE store_name='oxigraph_relations' AND status='pending'
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
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status");
    assert_eq!(store.checkpoint_cursor, checkpoint);
    assert_eq!(store.failed, 1);
    Ok(())
}

#[cfg(feature = "oxigraph-backend")]
#[test]
fn oxigraph_event_cannot_be_retargeted_to_another_existing_task() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    let temp = TempDb::new("maintenance_oxigraph_event_entity_binding")?;
    init_database(&temp.path, "tester")?;
    let other = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("other graph entity"),
    )?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let source = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("source graph delivery"),
    )?;
    let before = maintenance_status(&temp.path)?;
    let checkpoint = before
        .stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status")
        .checkpoint_cursor;
    let conn = connect_file(&temp.path)?;
    conn.execute(
        "UPDATE projection_deliveries
         SET entity_uri=?1,action='upsert'
         WHERE id=(
           SELECT d.id
           FROM projection_deliveries d
           JOIN task_events e ON e.id=d.source_event_id
           WHERE d.store_name='oxigraph_relations'
             AND d.status='pending'
             AND e.task_id=?2
           ORDER BY d.cursor LIMIT 1
         )",
        rusqlite::params![format!("kb://task/{}", other.id), source.id],
    )?;

    let error = result_err(maintenance_run_once(
        &temp.path,
        "runtime-test",
        MaintenanceRunOptions::default(),
    ))?;
    assert!(error.to_string().contains("cannot be mapped"));
    let store = maintenance_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status");
    assert_eq!(store.checkpoint_cursor, checkpoint);
    assert_eq!(store.failed, 1);
    Ok(())
}

#[cfg(feature = "oxigraph-backend")]
#[test]
fn oxigraph_event_cannot_fall_back_to_legacy_across_boards() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    let temp = TempDb::new("maintenance_oxigraph_cross_board_event_binding")?;
    init_database(&temp.path, "tester")?;
    insert_board(&temp.path, "other", "b_other")?;
    let other = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other-board event"),
    )?;
    maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
    let source = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("source graph delivery"),
    )?;
    let before = maintenance_status(&temp.path)?;
    let checkpoint = before
        .stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status")
        .checkpoint_cursor;
    let conn = connect_file(&temp.path)?;
    let other_event: i64 = conn.query_row(
        "SELECT id FROM task_events WHERE board_id=?1 AND task_id=?2 ORDER BY id LIMIT 1",
        rusqlite::params![other.board_id, other.id],
        |row| row.get(0),
    )?;
    conn.execute_batch("DROP TRIGGER projection_delivery_board_guard_update;")?;
    conn.execute(
        "UPDATE projection_deliveries
         SET source_event_id=?1
         WHERE id=(
           SELECT d.id
           FROM projection_deliveries d
           JOIN task_events e ON e.id=d.source_event_id
           WHERE d.store_name='oxigraph_relations'
             AND d.status='pending'
             AND e.task_id=?2
           ORDER BY d.cursor LIMIT 1
         )",
        rusqlite::params![other_event, source.id],
    )?;

    let error = result_err(maintenance_run_once(
        &temp.path,
        "runtime-test",
        MaintenanceRunOptions::default(),
    ))?;
    assert!(error.to_string().contains("source event"));
    let store = maintenance_status(&temp.path)?
        .stores
        .into_iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("Oxigraph status");
    assert_eq!(store.checkpoint_cursor, checkpoint);
    assert_eq!(store.failed, 1);
    Ok(())
}

#[cfg(feature = "oxigraph-backend")]
#[test]
fn oxigraph_fingerprint_covers_board_global_relations() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    for direction in ["board_to_global", "global_to_board"] {
        let temp = TempDb::new(&format!("maintenance_oxigraph_{direction}"))?;
        init_database(&temp.path, "tester")?;
        let conn = connect_file(&temp.path)?;
        let board_id: String =
            conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
                row.get(0)
            })?;
        conn.execute(
            "INSERT INTO entities(
               uri,kind,source_table,source_id,board_id,created_at,updated_at
             ) VALUES('kb://fixture/scoped','fixture','fixture','scoped',?1,1,1)",
            [&board_id],
        )?;
        conn.execute(
            "INSERT INTO entities(
               uri,kind,source_table,source_id,board_id,created_at,updated_at
             ) VALUES('kb://fixture/global','fixture','fixture','global',NULL,1,1)",
            [],
        )?;
        let (subject, object) = if direction == "board_to_global" {
            ("kb://fixture/scoped", "kb://fixture/global")
        } else {
            ("kb://fixture/global", "kb://fixture/scoped")
        };
        conn.execute(
            "INSERT INTO entity_relations(
               subject_uri,predicate,object_uri,graph_uri,authoritative_store,
               metadata_json,created_at,updated_at
             ) VALUES(?1,'related_to',?2,'kb://graph/indexed','sqlite','{}',1,1)",
            rusqlite::params![subject, object],
        )?;
        drop(conn);

        maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
        let status = maintenance_status(&temp.path)?;
        let store = status
            .stores
            .into_iter()
            .find(|store| store.store_name == "oxigraph_relations")
            .expect("Oxigraph status");
        assert_eq!(store.lifecycle_status, "ready", "{direction}");
        assert_eq!(store.fallback_reason, None, "{direction}");
    }
    Ok(())
}

#[cfg(feature = "oxigraph-backend")]
#[test]
fn oxigraph_board_noop_requires_non_task_source_event() -> anyhow::Result<()> {
    use kanban_sqlite::api::{maintenance_run_once, maintenance_status};

    for case in ["missing_event", "task_event"] {
        let temp = TempDb::new(&format!("maintenance_oxigraph_board_noop_{case}"))?;
        init_database(&temp.path, "tester")?;
        maintenance_run_once(&temp.path, "runtime-test", MaintenanceRunOptions::default())?;
        create_task(
            &temp.path,
            "default",
            "tester",
            CreateTask::ready("board noop evidence"),
        )?;
        let conn = connect_file(&temp.path)?;
        let (delivery_id, board_id): (i64, String) = conn.query_row(
            "SELECT id,board_id FROM projection_deliveries
             WHERE store_name='oxigraph_relations' AND status='pending'
             ORDER BY cursor LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let checkpoint = maintenance_status(&temp.path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == "oxigraph_relations")
            .expect("Oxigraph status")
            .checkpoint_cursor;
        if case == "missing_event" {
            conn.execute(
                "UPDATE projection_deliveries
                 SET entity_uri=?1,source_event_id=NULL
                 WHERE id=?2",
                rusqlite::params![format!("kb://board/{board_id}"), delivery_id],
            )?;
        } else {
            conn.execute(
                "UPDATE projection_deliveries
                 SET entity_uri=?1
                 WHERE id=?2",
                rusqlite::params![format!("kb://board/{board_id}"), delivery_id],
            )?;
        }

        let error = result_err(maintenance_run_once(
            &temp.path,
            "runtime-test",
            MaintenanceRunOptions::default(),
        ))?;
        assert!(
            error.to_string().contains("cannot be mapped"),
            "{case}: {error}"
        );
        let store = maintenance_status(&temp.path)?
            .stores
            .into_iter()
            .find(|store| store.store_name == "oxigraph_relations")
            .expect("Oxigraph status");
        assert_eq!(store.checkpoint_cursor, checkpoint, "{case}");
        assert_eq!(store.failed, 1, "{case}");
    }
    Ok(())
}
