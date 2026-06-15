use crate::common::*;

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
