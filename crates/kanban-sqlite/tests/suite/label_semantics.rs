use crate::common::*;
use rusqlite::OptionalExtension;

#[test]
fn task_label_suggestions_degrade_when_vector_store_disabled() -> anyhow::Result<()> {
    let temp = TempDb::new("task_label_suggestions_degrade_when_vector_store_disabled")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("label suggestion target"),
    )?;

    let result = kanban_sqlite::suggest_task_labels(
        &temp.path,
        "default",
        &task.id,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;

    assert_eq!(result.task_id, task.id);
    assert!(result.degraded);
    assert!(result.selected_labels.is_empty());
    assert_eq!(result.coverage, 0.0);
    assert_eq!(result.residual_norm, 1.0);
    assert!(!result.needs_new_label);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "vector_store_disabled")
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "solver_refit_unavailable")
    );
    Ok(())
}

#[test]
fn task_label_suggestions_aggregate_atom_hits_and_penalize_negative_evidence() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("task_label_suggestions_aggregate_atom_hits_and_penalize_negative_evidence")?;
    init_database(&temp.path, "tester")?;
    let backend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    let frontend = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "frontend".to_owned(),
            color: None,
        },
    )?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        kanban_sqlite::CreateTask {
            title: "Fix API route".to_owned(),
            description: Some("Touches server handler code".to_owned()),
            ..CreateTask::ready("unused")
        },
    )?;
    let labeled =
        kanban_sqlite::add_task_label(&temp.path, "default", "tester", &task.id, "backend")?;

    let store = StaticLabelAtomStore {
        hits: vec![
            atom_hit(&backend, "positive", "applies_when", "server handlers", 0.0),
            atom_hit(&frontend, "positive", "name", "frontend", 0.2),
            atom_hit(&frontend, "negative", "excludes_when", "server only", 0.0),
        ],
    };
    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &labeled.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions {
            limit: 5,
            atom_limit: 10,
            min_score: 0.01,
        },
    )?;

    assert_eq!(result.candidates[0].label_name, "backend");
    assert_eq!(result.candidates[0].score, 1.0);
    assert!(result.candidates[0].already_applied);
    assert_eq!(result.selected_labels.len(), 2);
    assert_eq!(result.selected_labels[0].label_name, "backend");
    assert_eq!(result.selected_labels[1].label_name, "frontend");
    assert!(
        result.selected_labels[1].score < 0.04,
        "negative evidence should suppress frontend score: {result:?}"
    );
    assert!(result.coverage > 0.99);
    assert!(!result.needs_new_label);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "solver_refit_unavailable")
    );
    Ok(())
}

#[test]
fn task_label_suggestions_signal_new_label_when_enabled_index_has_no_selected_labels()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "task_label_suggestions_signal_new_label_when_enabled_index_has_no_selected_labels",
    )?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("uncovered task"),
    )?;
    let store = StaticLabelAtomStore { hits: Vec::new() };

    let result = kanban_sqlite::suggest_task_labels_with(
        &temp.path,
        "default",
        &task.id,
        &store,
        kanban_sqlite::LabelSuggestionOptions::default(),
    )?;

    assert!(result.needs_new_label);
    assert_eq!(result.coverage, 0.0);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|code| code == "label_atom_index_empty")
    );
    Ok(())
}

struct StaticLabelAtomStore {
    hits: Vec<kanban_vector::LabelAtomHit>,
}

impl kanban_vector::VectorStore for StaticLabelAtomStore {
    fn chunk_embedding_model(&self) -> &str {
        "test-model"
    }

    fn status(&self) -> kanban_vector::VectorStoreStatus {
        kanban_vector::VectorStoreStatus {
            backend: "test-vector".to_owned(),
            enabled: true,
            message: "test vector store; dirty=false last_error=none; board_dirty=false".to_owned(),
        }
    }

    fn delete_board(&self, _board_id: &str) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn delete_entities(&self, _entity_uris: &[String]) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn upsert(
        &self,
        _chunks: &[kanban_vector::EmbeddingChunk],
    ) -> Result<(), kanban_vector::VectorError> {
        Ok(())
    }

    fn query(
        &self,
        _query: &kanban_vector::VectorQuery,
    ) -> Result<Vec<kanban_vector::VectorHit>, kanban_vector::VectorError> {
        Ok(Vec::new())
    }

    fn query_label_atoms(
        &self,
        query: &kanban_vector::LabelAtomQuery,
    ) -> Result<Vec<kanban_vector::LabelAtomHit>, kanban_vector::VectorError> {
        Ok(self
            .hits
            .iter()
            .filter(|hit| {
                query
                    .board_id
                    .as_ref()
                    .is_none_or(|board_id| &hit.board_id == board_id)
                    && query
                        .embedding_model
                        .as_ref()
                        .is_none_or(|model| &hit.embedding_model == model)
                    && query
                        .polarity
                        .as_ref()
                        .is_none_or(|polarity| &hit.polarity == polarity)
            })
            .take(query.limit)
            .cloned()
            .collect())
    }
}

fn atom_hit(
    label: &kanban_sqlite::LabelRecord,
    polarity: &str,
    kind: &str,
    text: &str,
    distance: f32,
) -> kanban_vector::LabelAtomHit {
    kanban_vector::LabelAtomHit {
        atom_id: format!("la_{}_{}", label.name, kind),
        label_id: label.id.clone(),
        label_name: label.name.clone(),
        board_id: label.board_id.clone(),
        polarity: polarity.to_owned(),
        kind: kind.to_owned(),
        text: text.to_owned(),
        ordinal: 0,
        content_hash: "hash".to_owned(),
        embedding_model: "test-model".to_owned(),
        score: distance,
    }
}

#[test]
fn label_semantics_crud_expands_stable_atoms_and_keeps_label_binding() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_crud_expands_stable_atoms_and_keeps_label_binding")?;
    init_database(&temp.path, "tester")?;
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("label target"),
    )?;
    let labeled =
        kanban_sqlite::add_task_label(&temp.path, "default", "tester", &task.id, "backend")?;
    let label = labeled
        .labels
        .first()
        .ok_or_else(|| test_error("expected task label"))?
        .clone();

    let semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend implementation work".to_owned()),
            applies_when: vec!["  touches server code  ".to_owned(), "".to_owned()],
            positive_examples: vec!["API handlers".to_owned()],
            excludes_when: vec!["frontend only".to_owned()],
            negative_examples: vec!["CSS polish".to_owned()],
        },
    )?;

    assert_eq!(semantics.label_id, label.id);
    assert_eq!(semantics.applies_when, vec!["touches server code"]);
    assert_eq!(semantics.atoms.len(), 6);
    assert_eq!(semantics.atoms[0].kind, "name");
    assert_eq!(semantics.atoms[0].text, "backend");
    assert!(
        semantics
            .atoms
            .iter()
            .any(|atom| atom.polarity == "negative" && atom.kind == "negative_example")
    );
    let first_ids = semantics
        .atoms
        .iter()
        .map(|atom| atom.id.clone())
        .collect::<Vec<_>>();

    let reread = get_label_semantics(&temp.path, "default", &label.id)?;
    assert_eq!(
        reread
            .atoms
            .iter()
            .map(|atom| atom.id.clone())
            .collect::<Vec<_>>(),
        first_ids
    );

    let fresh_task = get_task(&temp.path, "default", &task.id)?;
    assert_eq!(fresh_task.labels.len(), 1);
    assert_eq!(fresh_task.labels[0].id, label.id);

    delete_label_semantics(&temp.path, "default", "backend")?;
    assert!(
        result_err(get_label_semantics(&temp.path, "default", "backend"))?
            .to_string()
            .contains("not found")
    );
    assert!(list_label_atoms(&temp.path, "default")?.is_empty());
    assert_eq!(get_task(&temp.path, "default", &task.id)?.labels.len(), 1);
    Ok(())
}

#[test]
fn label_semantics_resolves_l_prefixed_label_name_before_id_fallback() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_resolves_l_prefixed_label_name_before_id_fallback")?;
    init_database(&temp.path, "tester")?;
    let label = create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "l_foo".to_owned(),
            color: None,
        },
    )?;

    let semantics = upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "l_foo".to_owned(),
            description: Some("Name starts with an id-like prefix".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;

    assert_eq!(semantics.label_id, label.id);
    assert_eq!(semantics.label_name, "l_foo");
    let reread = get_label_semantics(&temp.path, "default", "l_foo")?;
    assert_eq!(reread.label_id, label.id);
    delete_label_semantics(&temp.path, "default", "l_foo")?;
    assert!(list_label_atoms(&temp.path, "default")?.is_empty());
    Ok(())
}

#[test]
fn label_semantics_jsonl_export_import_round_trips_truth_and_atoms() -> anyhow::Result<()> {
    let source =
        TempDb::new("label_semantics_jsonl_export_import_round_trips_truth_and_atoms_source")?;
    init_database(&source.path, "tester")?;
    create_label(
        &source.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: Some("blue".to_owned()),
        },
    )?;
    let semantics = upsert_label_semantics(
        &source.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend implementation work".to_owned()),
            applies_when: vec!["touches Rust service code".to_owned()],
            positive_examples: vec!["SQLite repository change".to_owned()],
            excludes_when: vec!["UI-only polish".to_owned()],
            negative_examples: vec!["CSS-only tweak".to_owned()],
        },
    )?;
    let source_atoms = semantics
        .atoms
        .iter()
        .map(|atom| {
            (
                atom.id.clone(),
                atom.content_hash.clone(),
                atom.text.clone(),
            )
        })
        .collect::<Vec<_>>();

    let export_path = source.dir.join("labels.jsonl");
    export_jsonl(&source.path, "default", &export_path)?;
    let export = std::fs::read_to_string(&export_path)?;
    assert!(export.contains("\"type\":\"label_semantics\""));
    assert!(export.contains("\"type\":\"label_atom\""));

    let target =
        TempDb::new("label_semantics_jsonl_export_import_round_trips_truth_and_atoms_target")?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &export_path, true)?;

    let imported = get_label_semantics(&target.path, "default", "backend")?;
    assert_eq!(
        imported.description.as_deref(),
        Some("Backend implementation work")
    );
    assert_eq!(imported.applies_when, vec!["touches Rust service code"]);
    assert_eq!(
        imported
            .atoms
            .iter()
            .map(|atom| (
                atom.id.clone(),
                atom.content_hash.clone(),
                atom.text.clone()
            ))
            .collect::<Vec<_>>(),
        source_atoms
    );
    assert!(
        label_atom_store_dirty(&target.path)?,
        "imported label truth must mark global label atom store dirty"
    );
    assert!(
        label_atom_board_dirty(&target.path, "default")?,
        "imported label truth must mark the imported board dirty"
    );
    Ok(())
}

#[test]
fn label_semantics_rejects_missing_or_cross_board_label() -> anyhow::Result<()> {
    let temp = TempDb::new("label_semantics_rejects_missing_or_cross_board_label")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "second".to_owned(),
            name: "Second".to_owned(),
            description: None,
        },
    )?;
    let second_label = create_label(
        &temp.path,
        "second",
        kanban_sqlite::CreateLabel {
            name: "shared".to_owned(),
            color: None,
        },
    )?;

    let missing = result_err(upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "missing".to_owned(),
            ..UpsertLabelSemantics::default()
        },
    ))?;
    assert!(missing.to_string().contains("not found"));

    let cross_board = result_err(upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: second_label.id,
            ..UpsertLabelSemantics::default()
        },
    ))?;
    assert!(cross_board.to_string().contains("not found"));
    Ok(())
}

#[test]
fn doctor_reports_missing_label_semantics_tables_unhealthy() -> anyhow::Result<()> {
    for table in ["label_semantics", "label_atoms", "label_atom_index_boards"] {
        let temp = TempDb::new(&format!(
            "doctor_reports_missing_label_semantics_tables_unhealthy_{table}"
        ))?;
        init_database(&temp.path, "tester")?;
        connect_file(&temp.path)?.execute_batch(&format!("DROP TABLE {table};"))?;

        let report = doctor_database(&temp.path)?;

        assert_eq!(report.migration_version, Some(8));
        assert_eq!(report.user_version, 8);
        assert!(!report.ok, "{table} missing should make doctor unhealthy");
    }
    Ok(())
}

#[test]
fn label_atom_store_is_seeded_and_not_dirtied_by_task_outbox() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_store_is_seeded_and_not_dirtied_by_task_outbox")?;
    init_database(&temp.path, "tester")?;

    let stores = derived_store_statuses(&temp.path)?;
    assert!(
        stores
            .iter()
            .any(|store| store.store_name == "lancedb_label_atoms")
    );
    assert_eq!(stores.len(), 4);

    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("task outbox should not dirty label atoms"),
    )?;
    let stores = derived_store_statuses(&temp.path)?;
    let chunks = stores
        .iter()
        .find(|store| store.store_name == "lancedb_chunks")
        .ok_or_else(|| test_error("missing lancedb_chunks"))?;
    let label_atoms = stores
        .iter()
        .find(|store| store.store_name == "lancedb_label_atoms")
        .ok_or_else(|| test_error("missing lancedb_label_atoms"))?;
    assert!(chunks.dirty);
    assert!(!label_atoms.dirty);
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_atom_rebuild_status_query_and_failure_are_independent() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_rebuild_status_query_and_failure_are_independent")?;
    init_database(&temp.path, "tester")?;
    create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("task vector dirty"),
    )?;
    create_label(
        &temp.path,
        "default",
        kanban_sqlite::CreateLabel {
            name: "backend".to_owned(),
            color: None,
        },
    )?;
    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend work".to_owned()),
            excludes_when: vec!["frontend only".to_owned()],
            ..UpsertLabelSemantics::default()
        },
    )?;

    let store = RecordingVectorStore::with_embedding_model("static-test");
    let status = rebuild_label_atom_index_with(&temp.path, "default", &store)?;
    assert!(status.message.contains("rebuilt 3 label atom(s)"));
    assert_eq!(store.upserted_label_atoms()?.len(), 3);
    let hits = query_label_atom_index_with(
        &temp.path,
        "default",
        &store,
        LabelAtomQuery {
            text: "backend".to_owned(),
            limit: 10,
            board_id: None,
            embedding_model: Some("static-test".to_owned()),
            polarity: Some("positive".to_owned()),
        },
    )?;
    assert_eq!(hits.len(), 2);
    assert!(hits.iter().all(|hit| hit.polarity == "positive"));

    let stores = derived_store_statuses(&temp.path)?;
    assert!(
        stores
            .iter()
            .find(|store| store.store_name == "lancedb_chunks")
            .ok_or_else(|| test_error("missing lancedb_chunks"))?
            .dirty,
        "task chunk outbox remains dirty"
    );
    assert!(
        !stores
            .iter()
            .find(|store| store.store_name == "lancedb_label_atoms")
            .ok_or_else(|| test_error("missing lancedb_label_atoms"))?
            .dirty
    );

    upsert_label_semantics(
        &temp.path,
        "default",
        UpsertLabelSemantics {
            label_ref: "backend".to_owned(),
            description: Some("Backend work updated".to_owned()),
            ..UpsertLabelSemantics::default()
        },
    )?;
    let failure = result_err(rebuild_label_atom_index_with(
        &temp.path,
        "default",
        &FailingVectorStore,
    ))?;
    assert!(failure.to_string().contains("dimension mismatch"));
    let status = label_atom_index_status_with(&temp.path, "default", &store)?;
    assert!(status.message.contains("dirty=true"));
    assert!(status.message.contains("dimension mismatch"));
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_atom_rebuild_keeps_global_dirty_until_all_dirty_boards_rebuild() -> anyhow::Result<()> {
    let temp = TempDb::new("label_atom_rebuild_keeps_global_dirty_until_all_dirty_boards_rebuild")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "second".to_owned(),
            name: "Second".to_owned(),
            description: None,
        },
    )?;
    for (board, label) in [("default", "backend"), ("second", "frontend")] {
        create_label(
            &temp.path,
            board,
            kanban_sqlite::CreateLabel {
                name: label.to_owned(),
                color: None,
            },
        )?;
        upsert_label_semantics(
            &temp.path,
            board,
            UpsertLabelSemantics {
                label_ref: label.to_owned(),
                description: Some(format!("{label} work")),
                ..UpsertLabelSemantics::default()
            },
        )?;
    }

    let store = RecordingVectorStore::with_embedding_model("static-test");
    rebuild_label_atom_index_with(&temp.path, "default", &store)?;
    let stores = derived_store_statuses(&temp.path)?;
    assert!(
        stores
            .iter()
            .find(|store| store.store_name == "lancedb_label_atoms")
            .ok_or_else(|| test_error("missing lancedb_label_atoms"))?
            .dirty,
        "second board remains dirty after rebuilding only default"
    );

    rebuild_label_atom_index_with(&temp.path, "second", &store)?;
    let stores = derived_store_statuses(&temp.path)?;
    assert!(
        !stores
            .iter()
            .find(|store| store.store_name == "lancedb_label_atoms")
            .ok_or_else(|| test_error("missing lancedb_label_atoms"))?
            .dirty,
        "all dirty label atom boards were rebuilt"
    );
    Ok(())
}

#[cfg(feature = "vector-lancedb")]
#[test]
fn label_semantics_jsonl_import_marks_label_atom_boards_dirty_and_rebuild_clears_per_board()
-> anyhow::Result<()> {
    let source = TempDb::new(
        "label_semantics_jsonl_import_marks_label_atom_boards_dirty_and_rebuild_clears_source",
    )?;
    init_database(&source.path, "tester")?;
    create_board(
        &source.path,
        "tester",
        CreateBoard {
            slug: "second".to_owned(),
            name: "Second".to_owned(),
            description: None,
        },
    )?;
    for (board, label) in [("default", "backend"), ("second", "frontend")] {
        create_label(
            &source.path,
            board,
            kanban_sqlite::CreateLabel {
                name: label.to_owned(),
                color: None,
            },
        )?;
        upsert_label_semantics(
            &source.path,
            board,
            UpsertLabelSemantics {
                label_ref: label.to_owned(),
                description: Some(format!("{label} work")),
                applies_when: vec![format!("{label} scope")],
                ..UpsertLabelSemantics::default()
            },
        )?;
    }

    let default_export = source.dir.join("default-labels.jsonl");
    let second_export = source.dir.join("second-labels.jsonl");
    let import_path = source.dir.join("two-board-labels.jsonl");
    export_jsonl(&source.path, "default", &default_export)?;
    export_jsonl(&source.path, "second", &second_export)?;
    let merged_export = format!(
        "{}{}",
        std::fs::read_to_string(&default_export)?,
        std::fs::read_to_string(&second_export)?
    );
    std::fs::write(&import_path, merged_export)?;

    let target = TempDb::new(
        "label_semantics_jsonl_import_marks_label_atom_boards_dirty_and_rebuild_clears_target",
    )?;
    init_database(&target.path, "tester")?;
    import_jsonl(&target.path, &import_path, true)?;

    assert!(label_atom_store_dirty(&target.path)?);
    assert!(label_atom_board_dirty(&target.path, "default")?);
    assert!(label_atom_board_dirty(&target.path, "second")?);
    let status = label_atom_index_status_with(
        &target.path,
        "default",
        &RecordingVectorStore::with_embedding_model("static-test"),
    )?;
    assert!(status.message.contains("dirty=true"));
    assert!(status.message.contains("board_dirty=true"));

    let store = RecordingVectorStore::with_embedding_model("static-test");
    rebuild_label_atom_index_with(&target.path, "default", &store)?;
    assert!(
        !label_atom_board_dirty(&target.path, "default")?,
        "rebuilt board should be clean"
    );
    assert!(
        label_atom_board_dirty(&target.path, "second")?,
        "unrebuilt imported board must remain dirty"
    );
    assert!(
        label_atom_store_dirty(&target.path)?,
        "global store remains dirty while another imported board is dirty"
    );

    rebuild_label_atom_index_with(&target.path, "second", &store)?;
    assert!(!label_atom_board_dirty(&target.path, "second")?);
    assert!(!label_atom_store_dirty(&target.path)?);
    Ok(())
}

fn label_atom_store_dirty(path: &Path) -> anyhow::Result<bool> {
    let stores = derived_store_statuses(path)?;
    Ok(stores
        .iter()
        .find(|store| store.store_name == "lancedb_label_atoms")
        .ok_or_else(|| test_error("missing lancedb_label_atoms"))?
        .dirty)
}

fn label_atom_board_dirty(path: &Path, board: &str) -> anyhow::Result<bool> {
    let board = get_board(path, board)?;
    Ok(connect_file(path)?
        .query_row(
            "SELECT dirty FROM label_atom_index_boards \
             WHERE store_name='lancedb_label_atoms' AND board_id=?1",
            [board.id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(false))
}
