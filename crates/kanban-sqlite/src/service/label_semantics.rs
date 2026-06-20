use crate::connect_file;

use super::{
    LabelAtomExplainAction, LabelAtomExplainRecord, LabelAtomExplainSignal,
    LabelAtomExplainValidation, LabelAtomRecord, LabelOntologyActionRecord,
    LabelOntologyActionType, LabelOntologyObservationRecord, LabelOntologySemanticsMutationInput,
    LabelOntologySignalRecord, LabelProposalCandidate, LabelSemanticsMutationOptions,
    LabelSemanticsRecord, TaskRecord, UpsertLabelSemantics, board_id, derived_status_by_name,
    get_task_by_id, label_ontology_mutation_atoms,
    label_ontology_semantics_snapshot_for_definition, label_ontology_semantics_snapshot_in_tx,
    record_label_ontology_semantics_mutation_in_tx, storage, vector_storage, with_immediate_tx,
};

use std::{collections::BTreeSet, path::Path, str::FromStr};

use kanban_core::{Clock, KanbanError, Result, SystemClock};
use kanban_indexer::{DERIVED_STORE_SCHEMA_VERSION, LANCEDB_LABEL_ATOMS_STORE};
use kanban_labels::{LabelAtomKind, LabelAtomPolarity, LabelDefinition};
use kanban_vector::{
    LabelAtomHit, LabelAtomQuery, LabelAtomVector, LabelAtomVectorHit, LabelAtomVectorQuery,
    LabelAtomVectorStore, VectorStoreBackend, VectorStoreStatus,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::Value as JsonValue;

pub fn upsert_label_semantics(
    path: impl AsRef<Path>,
    board: &str,
    input: UpsertLabelSemantics,
) -> Result<LabelSemanticsRecord> {
    upsert_label_semantics_with_options(
        path,
        board,
        input,
        LabelSemanticsMutationOptions::manual_actor("system"),
    )
}

pub fn upsert_label_semantics_with_options(
    path: impl AsRef<Path>,
    board: &str,
    input: UpsertLabelSemantics,
    options: LabelSemanticsMutationOptions,
) -> Result<LabelSemanticsRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let label = resolve_label(&conn, &board_id, &input.label_ref)?;
        upsert_label_semantics_resolved_in_tx(&conn, &label, input, options, now)
    })
}

pub fn upsert_label_semantics_by_id(
    path: impl AsRef<Path>,
    board: &str,
    label_id: &str,
    input: UpsertLabelSemantics,
) -> Result<LabelSemanticsRecord> {
    upsert_label_semantics_by_id_with_options(
        path,
        board,
        label_id,
        input,
        LabelSemanticsMutationOptions::manual_actor("system"),
    )
}

pub fn upsert_label_semantics_by_id_with_options(
    path: impl AsRef<Path>,
    board: &str,
    label_id: &str,
    input: UpsertLabelSemantics,
    options: LabelSemanticsMutationOptions,
) -> Result<LabelSemanticsRecord> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let label = resolve_label_by_id_exact(&conn, &board_id, label_id)?;
        upsert_label_semantics_resolved_in_tx(&conn, &label, input, options, now)
    })
}

fn upsert_label_semantics_resolved_in_tx(
    conn: &Connection,
    label: &ResolvedLabel,
    input: UpsertLabelSemantics,
    options: LabelSemanticsMutationOptions,
    now: i64,
) -> Result<LabelSemanticsRecord> {
    let before =
        label_ontology_semantics_snapshot_in_tx(conn, &label.board_id, &label.id, &label.name)?;
    if let Some(expected) = normalize_optional_text(input.expected_semantics_hash.clone())
        && expected != before.hash
    {
        return Err(KanbanError::Conflict(format!(
            "label semantics hash mismatch for {}: expected {expected}, current {}",
            label.name, before.hash
        )));
    }
    let current = get_label_semantics_conn_optional(conn, &label.board_id, &label.id)?;
    let definition = label_definition_for_semantics_mutation(label, current.as_ref(), input)?;
    let after =
        label_ontology_semantics_snapshot_for_definition(&label.id, &label.name, &definition)?;
    if current.is_some() && before.hash == after.hash {
        return get_label_semantics_conn(conn, &label.board_id, &label.id);
    }
    let before_atoms = label_ontology_mutation_atoms(conn, &label.board_id, &label.id)?;
    upsert_label_semantics_in_tx(conn, &label.board_id, &definition, now)?;
    mark_label_atom_store_dirty(conn, &label.board_id, now)?;
    record_label_ontology_semantics_mutation_in_tx(
        conn,
        LabelOntologySemanticsMutationInput {
            board_id: &label.board_id,
            label_id: &label.id,
            label_name: &label.name,
            action_type: LabelOntologyActionType::UpdateSemantics,
            before,
            before_atoms,
            include_description_effects: false,
            options,
        },
        now,
    )?;
    get_label_semantics_conn(conn, &label.board_id, &label.id)
}

fn label_definition_for_semantics_mutation(
    label: &ResolvedLabel,
    current: Option<&LabelSemanticsRecord>,
    input: UpsertLabelSemantics,
) -> Result<LabelDefinition> {
    if input.replace {
        if !input.remove_applies_when.is_empty()
            || !input.remove_excludes_when.is_empty()
            || !input.remove_positive_examples.is_empty()
            || !input.remove_negative_examples.is_empty()
        {
            return Err(KanbanError::InvalidInput(
                "remove_* fields cannot be combined with replace semantics".into(),
            ));
        }
        return Ok(LabelDefinition {
            id: label.id.clone(),
            name: label.name.clone(),
            description: normalize_optional_text(input.description),
            applies_when: normalize_text_list(input.applies_when),
            positive_examples: normalize_text_list(input.positive_examples),
            excludes_when: normalize_text_list(input.excludes_when),
            negative_examples: normalize_text_list(input.negative_examples),
        });
    }

    let mut description = current.and_then(|record| record.description.clone());
    if let Some(next_description) = normalize_optional_text(input.description) {
        description = Some(next_description);
    }

    let mut applies_when = current
        .map(|record| record.applies_when.clone())
        .unwrap_or_default();
    let mut excludes_when = current
        .map(|record| record.excludes_when.clone())
        .unwrap_or_default();
    let mut positive_examples = current
        .map(|record| record.positive_examples.clone())
        .unwrap_or_default();
    let mut negative_examples = current
        .map(|record| record.negative_examples.clone())
        .unwrap_or_default();

    remove_semantics_items(&mut applies_when, input.remove_applies_when);
    remove_semantics_items(&mut excludes_when, input.remove_excludes_when);
    remove_semantics_items(&mut positive_examples, input.remove_positive_examples);
    remove_semantics_items(&mut negative_examples, input.remove_negative_examples);
    append_semantics_items(&mut applies_when, input.applies_when);
    append_semantics_items(&mut excludes_when, input.excludes_when);
    append_semantics_items(&mut positive_examples, input.positive_examples);
    append_semantics_items(&mut negative_examples, input.negative_examples);

    Ok(LabelDefinition {
        id: label.id.clone(),
        name: label.name.clone(),
        description,
        applies_when,
        positive_examples,
        excludes_when,
        negative_examples,
    })
}

pub(crate) fn upsert_label_semantics_candidate_in_tx(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
    label_name: &str,
    candidate: &LabelProposalCandidate,
    now: i64,
) -> Result<()> {
    let definition = LabelDefinition {
        id: label_id.to_owned(),
        name: label_name.to_owned(),
        description: candidate.description.clone(),
        applies_when: candidate.applies_when.clone(),
        positive_examples: candidate.positive_examples.clone(),
        excludes_when: candidate.excludes_when.clone(),
        negative_examples: candidate.negative_examples.clone(),
    };
    upsert_label_semantics_in_tx(conn, board_id, &definition, now)
}

pub(crate) fn upsert_label_semantics_in_tx(
    conn: &Connection,
    board_id: &str,
    definition: &LabelDefinition,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO label_semantics(label_id, board_id, description, applies_when, excludes_when, positive_examples, negative_examples, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8) \
         ON CONFLICT(label_id) DO UPDATE SET description=excluded.description, applies_when=excluded.applies_when, excludes_when=excluded.excludes_when, positive_examples=excluded.positive_examples, negative_examples=excluded.negative_examples, updated_at=excluded.updated_at",
        params![
            definition.id,
            board_id,
            definition.description,
            json_array(&definition.applies_when)?,
            json_array(&definition.excludes_when)?,
            json_array(&definition.positive_examples)?,
            json_array(&definition.negative_examples)?,
            now
        ],
    )
    .map_err(storage)?;
    rebuild_atoms_for_label(conn, definition, board_id, now)
}

pub fn get_label_semantics(
    path: impl AsRef<Path>,
    board: &str,
    label_ref: &str,
) -> Result<LabelSemanticsRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let label = resolve_label(&conn, &board_id, label_ref)?;
    get_label_semantics_conn(&conn, &board_id, &label.id)
}

pub fn get_label_semantics_by_id(
    path: impl AsRef<Path>,
    board: &str,
    label_id: &str,
) -> Result<LabelSemanticsRecord> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let label = resolve_label_by_id_exact(&conn, &board_id, label_id)?;
    get_label_semantics_conn(&conn, &board_id, &label.id)
}

pub fn list_label_semantics(
    path: impl AsRef<Path>,
    board: &str,
) -> Result<Vec<LabelSemanticsRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let mut stmt = conn
        .prepare(
            "SELECT label_id FROM label_semantics WHERE board_id=?1 ORDER BY updated_at DESC, label_id ASC",
        )
        .map_err(storage)?;
    let ids = stmt
        .query_map([&board_id], |row| row.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    ids.into_iter()
        .map(|label_id| get_label_semantics_conn(&conn, &board_id, &label_id))
        .collect()
}

pub fn clear_label_semantics_with_options(
    path: impl AsRef<Path>,
    board: &str,
    label_ref: &str,
    expected_semantics_hash: String,
    options: LabelSemanticsMutationOptions,
) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let label = resolve_label(&conn, &board_id, label_ref)?;
        clear_label_semantics_resolved_in_tx(&conn, &label, expected_semantics_hash, options, now)
    })
}

pub fn clear_label_semantics_by_id_with_options(
    path: impl AsRef<Path>,
    board: &str,
    label_id: &str,
    expected_semantics_hash: String,
    options: LabelSemanticsMutationOptions,
) -> Result<()> {
    let conn = connect_file(path.as_ref())?;
    let now = SystemClock.now_ms();
    with_immediate_tx(&conn, || {
        let board_id = board_id(&conn, board)?;
        let label = resolve_label_by_id_exact(&conn, &board_id, label_id)?;
        clear_label_semantics_resolved_in_tx(&conn, &label, expected_semantics_hash, options, now)
    })
}

fn clear_label_semantics_resolved_in_tx(
    conn: &Connection,
    label: &ResolvedLabel,
    expected_semantics_hash: String,
    mut options: LabelSemanticsMutationOptions,
    now: i64,
) -> Result<()> {
    let expected = normalize_optional_text(Some(expected_semantics_hash))
        .ok_or_else(|| KanbanError::InvalidInput("expected_semantics_hash is required".into()))?;
    let reason = normalize_optional_text(options.reason.clone())
        .ok_or_else(|| KanbanError::InvalidInput("reason is required".into()))?;
    options.reason = Some(reason);
    get_label_semantics_conn(conn, &label.board_id, &label.id)?;
    let before =
        label_ontology_semantics_snapshot_in_tx(conn, &label.board_id, &label.id, &label.name)?;
    if expected != before.hash {
        return Err(KanbanError::Conflict(format!(
            "label semantics hash mismatch for {}: expected {expected}, current {}",
            label.name, before.hash
        )));
    }
    let before_atoms = label_ontology_mutation_atoms(conn, &label.board_id, &label.id)?;
    conn.execute(
        "DELETE FROM label_semantics WHERE board_id=?1 AND label_id=?2",
        params![label.board_id, label.id],
    )
    .map_err(storage)?;
    conn.execute(
        "DELETE FROM label_atoms WHERE board_id=?1 AND label_id=?2",
        params![label.board_id, label.id],
    )
    .map_err(storage)?;
    mark_label_atom_store_dirty(conn, &label.board_id, now)?;
    record_label_ontology_semantics_mutation_in_tx(
        conn,
        LabelOntologySemanticsMutationInput {
            board_id: &label.board_id,
            label_id: &label.id,
            label_name: &label.name,
            action_type: LabelOntologyActionType::UpdateSemantics,
            before,
            before_atoms,
            include_description_effects: true,
            options,
        },
        now,
    )?;
    Ok(())
}

pub fn list_label_atoms(path: impl AsRef<Path>, board: &str) -> Result<Vec<LabelAtomRecord>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    label_atoms_for_board(&conn, &board_id)
}

pub fn explain_label_atom(
    path: impl AsRef<Path>,
    board: &str,
    atom_ref: &str,
) -> Result<LabelAtomExplainRecord> {
    let atom_ref = atom_ref.trim();
    if atom_ref.is_empty() {
        return Err(KanbanError::InvalidInput(
            "label atom ref is required".into(),
        ));
    }
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let atom = label_atom_by_id_or_hash(&conn, &board_id, atom_ref)?;
    let content_hash = atom
        .as_ref()
        .map(|atom| atom.content_hash.clone())
        .unwrap_or_else(|| atom_ref.to_owned());
    let current_semantics = match &atom {
        Some(atom) => match get_label_semantics_conn(&conn, &board_id, &atom.label_id) {
            Ok(semantics) => Some(semantics),
            Err(KanbanError::NotFound(_)) => None,
            Err(error) => return Err(error),
        },
        None => None,
    };
    let provenance_actions = label_atom_provenance_actions(
        &conn,
        &board_id,
        atom_ref,
        atom.as_ref().map(|atom| atom.id.as_str()),
        &content_hash,
    )?;

    if atom.is_none() && provenance_actions.is_empty() {
        return Err(KanbanError::NotFound(format!("label atom {atom_ref}")));
    }

    let supporting_signals = label_atom_supporting_signals(&conn, &provenance_actions)?;
    let validation_history = label_atom_validation_history(&conn, &provenance_actions)?;
    let legacy_untracked = atom.is_some() && provenance_actions.is_empty();
    let legacy_reason = legacy_untracked.then(|| {
        "no ontology provenance action, atom effect, or legacy result atom reference matches this atom id or content hash"
            .to_owned()
    });
    Ok(LabelAtomExplainRecord {
        query: atom_ref.to_owned(),
        atom,
        current_semantics,
        provenance_actions,
        supporting_signals,
        validation_history,
        legacy_untracked,
        legacy_reason,
    })
}

pub fn label_atom_index_status(path: impl AsRef<Path>, board: &str) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    label_atom_index_status_from_base(
        &conn,
        &board_id,
        VectorStoreStatus::new(
            "disabled",
            false,
            "Label atom vector store is disabled or has no provider",
        ),
    )
}

pub fn label_atom_index_status_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &(impl VectorStoreBackend + ?Sized),
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    label_atom_index_status_from_base(&conn, &board_id, store.status())
}

pub fn rebuild_label_atom_index_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl LabelAtomVectorStore,
) -> Result<VectorStoreStatus> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    let atoms = label_atom_vectors_for_board(&conn, &board_id, store.embedding_model())?;
    match store
        .delete_label_atoms_for_board(&board_id)
        .and_then(|()| store.upsert_label_atoms(&atoms))
    {
        Ok(()) => {
            let now = SystemClock.now_ms();
            mark_label_atom_store_success(&conn, &board_id, now)?;
            let derived = derived_status_by_name(&conn, LANCEDB_LABEL_ATOMS_STORE)?;
            let mut status = store.status();
            status.message = format!(
                "{}; rebuilt {} label atom(s); dirty={} last_error={}",
                status.message,
                atoms.len(),
                derived.dirty,
                derived.last_error.as_deref().unwrap_or("none")
            );
            Ok(status)
        }
        Err(error) => {
            mark_label_atom_store_failure(
                &conn,
                &board_id,
                &error.to_string(),
                SystemClock.now_ms(),
            )?;
            Err(vector_storage(error))
        }
    }
}

pub fn query_label_atom_index_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl LabelAtomVectorStore,
    mut query: LabelAtomQuery,
) -> Result<Vec<LabelAtomHit>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    query.board_id = Some(board_id);
    store.query_label_atoms(&query).map_err(vector_storage)
}

pub fn query_label_atom_index_by_vector_with(
    path: impl AsRef<Path>,
    board: &str,
    store: &impl LabelAtomVectorStore,
    mut query: LabelAtomVectorQuery,
) -> Result<Vec<LabelAtomVectorHit>> {
    let conn = connect_file(path.as_ref())?;
    let board_id = board_id(&conn, board)?;
    query.board_id = Some(board_id);
    store
        .query_label_atoms_by_vector(&query)
        .map_err(vector_storage)
}

pub(crate) fn get_label_semantics_conn(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<LabelSemanticsRecord> {
    get_label_semantics_conn_optional(conn, board_id, label_id)?
        .ok_or_else(|| KanbanError::NotFound(format!("label semantics {label_id}")))
}

fn get_label_semantics_conn_optional(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<Option<LabelSemanticsRecord>> {
    let mut record = conn
        .query_row(
            "SELECT s.label_id,s.board_id,l.name,s.description,s.applies_when,s.excludes_when,s.positive_examples,s.negative_examples,s.created_at,s.updated_at \
             FROM label_semantics s JOIN labels l ON l.id=s.label_id AND l.board_id=s.board_id \
             WHERE s.board_id=?1 AND s.label_id=?2",
            params![board_id, label_id],
            label_semantics_from_row,
        )
        .optional()
        .map_err(storage)?;
    if let Some(record) = record.as_mut() {
        record.semantics_hash = label_semantics_record_hash(record)?;
        record.atoms = label_atoms_for_label(conn, board_id, label_id)?;
    }
    Ok(record)
}

fn label_semantics_from_row(row: &Row<'_>) -> rusqlite::Result<LabelSemanticsRecord> {
    Ok(LabelSemanticsRecord {
        label_id: row.get(0)?,
        board_id: row.get(1)?,
        label_name: row.get(2)?,
        semantics_hash: String::new(),
        description: row.get(3)?,
        applies_when: json_vec(row.get::<_, String>(4)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        excludes_when: json_vec(row.get::<_, String>(5)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        positive_examples: json_vec(row.get::<_, String>(6)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        negative_examples: json_vec(row.get::<_, String>(7)?)
            .map_err(|err| rusqlite::Error::InvalidParameterName(err.to_string()))?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        atoms: Vec::new(),
    })
}

struct LabelSemanticDefinitionRow {
    board_id: String,
    definition: LabelDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StableLabelAtomKey {
    id: String,
    label_id: String,
    board_id: String,
    polarity: String,
    kind: String,
    text: String,
    ordinal: i64,
    content_hash: String,
}

fn label_semantic_definition_rows(conn: &Connection) -> Result<Vec<LabelSemanticDefinitionRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT s.label_id,s.board_id,l.name,s.description,s.applies_when,s.excludes_when,s.positive_examples,s.negative_examples \
             FROM label_semantics s JOIN labels l ON l.id=s.label_id AND l.board_id=s.board_id \
             ORDER BY s.board_id ASC, s.label_id ASC",
        )
        .map_err(storage)?;
    stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, String>(7)?,
        ))
    })
    .map_err(storage)?
    .map(|row| {
        let (
            label_id,
            board_id,
            name,
            description,
            applies_when,
            excludes_when,
            positive_examples,
            negative_examples,
        ) = row.map_err(storage)?;
        Ok(LabelSemanticDefinitionRow {
            board_id,
            definition: LabelDefinition {
                id: label_id,
                name,
                description,
                applies_when: json_vec(applies_when)?,
                excludes_when: json_vec(excludes_when)?,
                positive_examples: json_vec(positive_examples)?,
                negative_examples: json_vec(negative_examples)?,
            },
        })
    })
    .collect()
}

fn stable_label_atom_keys(definition: &LabelDefinition, board_id: &str) -> Vec<StableLabelAtomKey> {
    let mut atoms = Vec::new();
    let mut seen_semantic_keys = std::collections::HashSet::new();
    for (ordinal, source) in definition.atom_sources().into_iter().enumerate() {
        let polarity = polarity_to_str(source.polarity).to_owned();
        let kind = kind_to_str(source.kind).to_owned();
        let text = normalize_atom_text(&source.text);
        if text.is_empty() {
            continue;
        }
        let semantic_key = format!("{}\n{}\n{}\n{}", definition.id, polarity, kind, text);
        if !seen_semantic_keys.insert(semantic_key.clone()) {
            continue;
        }
        let content_hash = stable_hash(&semantic_key);
        atoms.push(StableLabelAtomKey {
            id: format!("la_{content_hash}"),
            label_id: source.label_id,
            board_id: board_id.to_owned(),
            polarity,
            kind,
            text,
            ordinal: ordinal as i64,
            content_hash,
        });
    }
    atoms
}

fn current_label_atom_keys(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<Vec<StableLabelAtomKey>> {
    let mut stmt = conn
        .prepare(
            "SELECT id,label_id,board_id,polarity,kind,text,ordinal,content_hash \
             FROM label_atoms WHERE board_id=?1 AND label_id=?2 ORDER BY ordinal ASC, id ASC",
        )
        .map_err(storage)?;
    stmt.query_map(params![board_id, label_id], |row| {
        Ok(StableLabelAtomKey {
            id: row.get(0)?,
            label_id: row.get(1)?,
            board_id: row.get(2)?,
            polarity: row.get(3)?,
            kind: row.get(4)?,
            text: row.get(5)?,
            ordinal: row.get(6)?,
            content_hash: row.get(7)?,
        })
    })
    .map_err(storage)?
    .collect::<std::result::Result<Vec<_>, _>>()
    .map_err(storage)
}

pub(crate) fn stable_label_atom_hash_backfill_needed(conn: &Connection) -> Result<bool> {
    for row in label_semantic_definition_rows(conn)? {
        let expected = stable_label_atom_keys(&row.definition, &row.board_id);
        let actual = current_label_atom_keys(conn, &row.board_id, &row.definition.id)?;
        if actual != expected {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rebuild_atoms_for_label(
    conn: &Connection,
    definition: &LabelDefinition,
    board_id: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "DELETE FROM label_atoms WHERE board_id=?1 AND label_id=?2",
        params![board_id, definition.id],
    )
    .map_err(storage)?;

    for atom in stable_label_atom_keys(definition, board_id) {
        conn.execute(
            "INSERT INTO label_atoms(id, label_id, board_id, polarity, kind, text, ordinal, content_hash, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                atom.id,
                atom.label_id,
                atom.board_id,
                atom.polarity,
                atom.kind,
                atom.text,
                atom.ordinal,
                atom.content_hash,
                now
            ],
        )
        .map_err(storage)?;
    }
    Ok(())
}

pub(crate) fn rebuild_label_atoms_for_stable_hash_migration(
    conn: &Connection,
    now: i64,
) -> Result<()> {
    with_immediate_tx(conn, || {
        let mut dirty_boards = std::collections::BTreeSet::new();
        for row in label_semantic_definition_rows(conn)? {
            rebuild_atoms_for_label(conn, &row.definition, &row.board_id, now)?;
            dirty_boards.insert(row.board_id);
        }

        for board_id in dirty_boards {
            mark_label_atom_store_dirty(conn, &board_id, now)?;
        }
        Ok(())
    })
}

fn label_atoms_for_board(conn: &Connection, board_id: &str) -> Result<Vec<LabelAtomRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id,a.label_id,a.board_id,l.name,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at \
             FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id \
             WHERE a.board_id=?1 ORDER BY l.name ASC, a.ordinal ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map([board_id], label_atom_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn label_atom_by_id_or_hash(
    conn: &Connection,
    board_id: &str,
    atom_ref: &str,
) -> Result<Option<LabelAtomRecord>> {
    conn.query_row(
        "SELECT a.id,a.label_id,a.board_id,l.name,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at \
         FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id \
         WHERE a.board_id=?1 AND (a.id=?2 OR a.content_hash=?2) \
         ORDER BY CASE WHEN a.id=?2 THEN 0 ELSE 1 END, l.name ASC, a.ordinal ASC, a.id ASC \
         LIMIT 1",
        params![board_id, atom_ref],
        label_atom_from_row,
    )
    .optional()
    .map_err(storage)
}

fn label_atoms_for_label(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<Vec<LabelAtomRecord>> {
    let mut stmt = conn
        .prepare(
            "SELECT a.id,a.label_id,a.board_id,l.name,a.polarity,a.kind,a.text,a.ordinal,a.content_hash,a.created_at,a.updated_at \
             FROM label_atoms a JOIN labels l ON l.id=a.label_id AND l.board_id=a.board_id \
             WHERE a.board_id=?1 AND a.label_id=?2 ORDER BY a.ordinal ASC",
        )
        .map_err(storage)?;
    let rows = stmt
        .query_map(params![board_id, label_id], label_atom_from_row)
        .map_err(storage)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)
}

fn label_atom_vectors_for_board(
    conn: &Connection,
    board_id: &str,
    embedding_model: &str,
) -> Result<Vec<LabelAtomVector>> {
    Ok(label_atoms_for_board(conn, board_id)?
        .into_iter()
        .map(|atom| LabelAtomVector {
            atom_id: atom.id,
            label_id: atom.label_id,
            label_name: atom.label_name,
            board_id: atom.board_id,
            polarity: atom.polarity,
            kind: atom.kind,
            text: atom.text,
            ordinal: atom.ordinal,
            content_hash: atom.content_hash,
            embedding_model: embedding_model.to_owned(),
            created_at: atom.created_at,
            updated_at: atom.updated_at,
        })
        .collect())
}

fn label_atom_from_row(row: &Row<'_>) -> rusqlite::Result<LabelAtomRecord> {
    Ok(LabelAtomRecord {
        id: row.get(0)?,
        label_id: row.get(1)?,
        board_id: row.get(2)?,
        label_name: row.get(3)?,
        polarity: row.get(4)?,
        kind: row.get(5)?,
        text: row.get(6)?,
        ordinal: row.get(7)?,
        content_hash: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

const ONTOLOGY_OBSERVATION_COLUMNS: &str = "id,board_id,task_id,task_ref_snapshot,task_snapshot_json,suggest_input_hash,agent_candidates_json,suggestion_snapshot_json,final_decision_json,suggest_coverage,suggest_coverage_cosine,suggest_residual_norm,suggest_needs_new_label,suggest_degraded,diagnostics_json,capture_fingerprint,created_by,created_by_type,agent_type,created_at";

const ONTOLOGY_SIGNAL_COLUMNS: &str = "s.id,s.observation_id,s.board_id,s.kind,s.status,s.target_label_id,s.target_label_name_snapshot,s.related_labels_json,s.proposed_action,s.candidate_atom_polarity,s.candidate_atom_kind,s.candidate_text,s.candidate_content_hash,s.proposed_label_name,s.proposed_label_name_normalized,s.proposal_json,s.agent_selected,s.suggest_state,s.suggest_score,s.suggest_rank,s.final_selected,s.rationale,s.confidence,s.signal_key,s.superseded_by_signal_id,s.status_reason,s.created_at,s.updated_at,s.reviewed_at,s.closed_at";

const ONTOLOGY_ACTION_COLUMNS: &str = "a.id,a.board_id,a.parent_action_id,a.action_type,a.reason,a.target_label_id,a.result_label_id,a.result_atom_id,a.result_atom_content_hash,a.result_proposal_id,a.canonical_before_hash,a.canonical_after_hash,a.change_json,a.validation_status,a.validation_json,a.created_by,a.created_by_type,a.agent_type,a.created_at";

fn label_atom_provenance_actions(
    conn: &Connection,
    board_id: &str,
    atom_ref: &str,
    current_atom_id: Option<&str>,
    content_hash: &str,
) -> Result<Vec<LabelAtomExplainAction>> {
    let mut seen = BTreeSet::new();
    let mut provenance_actions = Vec::new();
    let atom_id_ref = current_atom_id.unwrap_or(atom_ref);
    let mut effect_stmt = conn
        .prepare(&format!(
            "SELECT {ONTOLOGY_ACTION_COLUMNS} FROM label_ontology_action_atom_effects e \
             JOIN label_ontology_actions a ON a.board_id=e.board_id AND a.id=e.action_id \
             WHERE e.board_id=?1 \
               AND a.action_type IN ('add_positive_atom','add_negative_atom','update_semantics','bootstrap_label','revert_ontology_mutation') \
               AND (e.atom_id_snapshot=?2 OR e.atom_id_snapshot=?3 OR e.atom_content_hash=?2 OR e.atom_content_hash=?4) \
             ORDER BY a.created_at ASC, a.id ASC"
        ))
        .map_err(storage)?;
    let effect_rows = effect_stmt
        .query_map(
            params![board_id, atom_ref, atom_id_ref, content_hash],
            ontology_action_from_row,
        )
        .map_err(storage)?;
    for row in effect_rows {
        let mut action = row.map_err(storage)?;
        if !seen.insert(action.id.clone()) {
            continue;
        }
        hydrate_action_signal_ids(conn, &mut action)?;
        provenance_actions.push(LabelAtomExplainAction {
            action,
            matched_by: "atom_effect".to_owned(),
        });
    }

    let mut legacy_stmt = conn
        .prepare(&format!(
            "SELECT {ONTOLOGY_ACTION_COLUMNS} FROM label_ontology_actions a \
             WHERE a.board_id=?1 \
               AND a.action_type IN ('add_positive_atom','add_negative_atom','adopt_existing_atom','update_semantics','bootstrap_label','revert_ontology_mutation') \
               AND (a.result_atom_id=?2 OR a.result_atom_id=?3 OR a.result_atom_content_hash=?2 OR a.result_atom_content_hash=?4) \
             ORDER BY a.created_at ASC, a.id ASC"
        ))
        .map_err(storage)?;
    let legacy_rows = legacy_stmt
        .query_map(
            params![board_id, atom_ref, atom_id_ref, content_hash],
            ontology_action_from_row,
        )
        .map_err(storage)?;
    for row in legacy_rows {
        let mut action = row.map_err(storage)?;
        if !seen.insert(action.id.clone()) {
            continue;
        }
        let matched_by = if action.result_atom_id.as_deref() == Some(atom_ref)
            || current_atom_id
                .is_some_and(|atom_id| action.result_atom_id.as_deref() == Some(atom_id))
        {
            "legacy_result_atom_id"
        } else {
            "legacy_result_atom_hash"
        };
        hydrate_action_signal_ids(conn, &mut action)?;
        provenance_actions.push(LabelAtomExplainAction {
            action,
            matched_by: matched_by.to_owned(),
        });
    }
    provenance_actions.sort_by(|left, right| {
        match left.action.created_at.cmp(&right.action.created_at) {
            std::cmp::Ordering::Equal => left.action.id.cmp(&right.action.id),
            ordering => ordering,
        }
    });
    Ok(provenance_actions)
}

fn label_atom_supporting_signals(
    conn: &Connection,
    provenance_actions: &[LabelAtomExplainAction],
) -> Result<Vec<LabelAtomExplainSignal>> {
    let mut seen = BTreeSet::new();
    let mut signals = Vec::new();
    for action in provenance_actions {
        for signal_id in &action.action.signal_ids {
            if !seen.insert(signal_id.clone()) {
                continue;
            }
            let signal = ontology_signal_by_id(conn, signal_id)?;
            let observation = ontology_observation_by_id(conn, &signal.observation_id)?;
            let source_task = get_task_by_id(conn, &observation.board_id, &observation.task_id)?;
            let mut warnings = Vec::new();
            let current_suggest_hash = suggest_input_hash_for_task(&source_task);
            let suggest_input_stale = match observation.suggest_input_hash.as_deref() {
                Some(hash) if hash == current_suggest_hash => false,
                Some(_) => {
                    warnings.push("suggest_input_drift".to_owned());
                    true
                }
                None => {
                    warnings.push("legacy_suggest_input_hash_missing".to_owned());
                    true
                }
            };
            if observation.suggest_degraded {
                warnings.push("suggest_degraded".to_owned());
            }
            signals.push(LabelAtomExplainSignal {
                task_ref_snapshot: observation.task_ref_snapshot.clone(),
                suggest_input_stale,
                suggest_degraded: observation.suggest_degraded,
                warnings,
                signal,
                observation,
                source_task,
            });
        }
    }
    Ok(signals)
}

fn label_atom_validation_history(
    conn: &Connection,
    provenance_actions: &[LabelAtomExplainAction],
) -> Result<Vec<LabelAtomExplainValidation>> {
    let mut validations = Vec::new();
    for provenance in provenance_actions {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {ONTOLOGY_ACTION_COLUMNS} FROM label_ontology_actions a \
                 WHERE a.board_id=?1 AND a.parent_action_id=?2 AND a.action_type='validate' \
                 ORDER BY a.created_at ASC, a.id ASC"
            ))
            .map_err(storage)?;
        let rows = stmt
            .query_map(
                params![provenance.action.board_id, provenance.action.id],
                ontology_action_from_row,
            )
            .map_err(storage)?;
        for row in rows {
            let mut action = row.map_err(storage)?;
            hydrate_action_signal_ids(conn, &mut action)?;
            let envelope = parse_json_value(&action.validation_json)?;
            validations.push(LabelAtomExplainValidation {
                parent_action_id: provenance.action.id.clone(),
                validation_status: action.validation_status,
                manual: envelope.get("manual").cloned().unwrap_or(JsonValue::Null),
                summary: envelope.get("summary").cloned().unwrap_or(JsonValue::Null),
                cases: envelope
                    .get("cases")
                    .cloned()
                    .unwrap_or_else(|| JsonValue::Array(Vec::new())),
                warnings: validation_warnings(&envelope),
                action,
            });
        }
    }
    Ok(validations)
}

fn validation_warnings(envelope: &JsonValue) -> Vec<String> {
    let mut warnings = BTreeSet::new();
    if let Some(cases) = envelope.get("cases").and_then(JsonValue::as_array) {
        for case in cases {
            if let Some(items) = case.get("warnings").and_then(JsonValue::as_array) {
                for item in items {
                    if let Some(warning) = item.as_str() {
                        warnings.insert(warning.to_owned());
                    }
                }
            }
        }
    }
    warnings.into_iter().collect()
}

fn ontology_observation_by_id(
    conn: &Connection,
    observation_id: &str,
) -> Result<LabelOntologyObservationRecord> {
    conn.query_row(
        &format!(
            "SELECT {ONTOLOGY_OBSERVATION_COLUMNS} FROM label_ontology_observations WHERE id=?1"
        ),
        [observation_id],
        ontology_observation_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("label ontology observation {observation_id}")))
}

fn ontology_signal_by_id(conn: &Connection, signal_id: &str) -> Result<LabelOntologySignalRecord> {
    conn.query_row(
        &format!("SELECT {ONTOLOGY_SIGNAL_COLUMNS} FROM label_ontology_signals s WHERE s.id=?1"),
        [signal_id],
        ontology_signal_from_row,
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("label ontology signal {signal_id}")))
}

fn hydrate_action_signal_ids(
    conn: &Connection,
    action: &mut LabelOntologyActionRecord,
) -> Result<()> {
    let mut stmt = conn
        .prepare(
            "SELECT signal_id FROM label_ontology_action_signals WHERE action_id=?1 ORDER BY signal_id ASC",
        )
        .map_err(storage)?;
    action.signal_ids = stmt
        .query_map([&action.id], |row| row.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    Ok(())
}

fn ontology_observation_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<LabelOntologyObservationRecord> {
    Ok(LabelOntologyObservationRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        task_id: row.get(2)?,
        task_ref_snapshot: row.get(3)?,
        task_snapshot_json: row.get(4)?,
        suggest_input_hash: row.get(5)?,
        agent_candidates_json: row.get(6)?,
        suggestion_snapshot_json: row.get(7)?,
        final_decision_json: row.get(8)?,
        suggest_coverage: row.get(9)?,
        suggest_coverage_cosine: row.get(10)?,
        suggest_residual_norm: row.get(11)?,
        suggest_needs_new_label: int_bool(row.get(12)?),
        suggest_degraded: int_bool(row.get(13)?),
        diagnostics_json: row.get(14)?,
        capture_fingerprint: row.get(15)?,
        created_by: row.get(16)?,
        created_by_type: row.get(17)?,
        agent_type: row.get(18)?,
        created_at: row.get(19)?,
        signals: Vec::new(),
    })
}

fn ontology_signal_from_row(row: &Row<'_>) -> rusqlite::Result<LabelOntologySignalRecord> {
    let kind: String = row.get(3)?;
    let status: String = row.get(4)?;
    let proposed_action: String = row.get(8)?;
    let suggest_state: Option<String> = row.get(17)?;
    Ok(LabelOntologySignalRecord {
        id: row.get(0)?,
        observation_id: row.get(1)?,
        board_id: row.get(2)?,
        kind: parse_row_enum(&kind)?,
        status: parse_row_enum(&status)?,
        target_label_id: row.get(5)?,
        target_label_name_snapshot: row.get(6)?,
        related_labels_json: row.get(7)?,
        proposed_action: parse_row_enum(&proposed_action)?,
        candidate_atom_polarity: row.get(9)?,
        candidate_atom_kind: row.get(10)?,
        candidate_text: row.get(11)?,
        candidate_content_hash: row.get(12)?,
        proposed_label_name: row.get(13)?,
        proposed_label_name_normalized: row.get(14)?,
        proposal_json: row.get(15)?,
        agent_selected: int_bool(row.get(16)?),
        suggest_state: suggest_state.as_deref().map(parse_row_enum).transpose()?,
        suggest_score: row.get(18)?,
        suggest_rank: row.get(19)?,
        final_selected: int_bool(row.get(20)?),
        rationale: row.get(21)?,
        confidence: row.get(22)?,
        signal_key: row.get(23)?,
        superseded_by_signal_id: row.get(24)?,
        status_reason: row.get(25)?,
        created_at: row.get(26)?,
        updated_at: row.get(27)?,
        reviewed_at: row.get(28)?,
        closed_at: row.get(29)?,
    })
}

fn ontology_action_from_row(row: &Row<'_>) -> rusqlite::Result<LabelOntologyActionRecord> {
    let action_type: String = row.get(3)?;
    let validation_status: String = row.get(13)?;
    Ok(LabelOntologyActionRecord {
        id: row.get(0)?,
        board_id: row.get(1)?,
        parent_action_id: row.get(2)?,
        action_type: parse_row_enum(&action_type)?,
        reason: row.get(4)?,
        target_label_id: row.get(5)?,
        result_label_id: row.get(6)?,
        result_atom_id: row.get(7)?,
        result_atom_content_hash: row.get(8)?,
        result_proposal_id: row.get(9)?,
        canonical_before_hash: row.get(10)?,
        canonical_after_hash: row.get(11)?,
        change_json: row.get(12)?,
        validation_status: parse_row_enum(&validation_status)?,
        validation_json: row.get(14)?,
        created_by: row.get(15)?,
        created_by_type: row.get(16)?,
        agent_type: row.get(17)?,
        created_at: row.get(18)?,
        signal_ids: Vec::new(),
    })
}

fn parse_row_enum<T>(value: &str) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|err: T::Err| rusqlite::Error::InvalidParameterName(err.to_string()))
}

fn parse_json_value(json: &str) -> Result<JsonValue> {
    serde_json::from_str(json).map_err(|err| KanbanError::Storage(err.to_string()))
}

fn suggest_input_hash_for_task(task: &TaskRecord) -> String {
    stable_hash(&task_suggest_input_text(
        &task.title,
        task.description.as_deref(),
    ))
}

fn task_suggest_input_text(title: &str, description: Option<&str>) -> String {
    match description.map(str::trim).filter(|value| !value.is_empty()) {
        Some(description) => format!("{}\n\n{}", title.trim(), description),
        None => title.trim().to_owned(),
    }
}

fn int_bool(value: i64) -> bool {
    value != 0
}

fn resolve_label(conn: &Connection, board_id: &str, label_ref: &str) -> Result<ResolvedLabel> {
    let label_ref = label_ref.trim();
    if label_ref.is_empty() {
        return Err(KanbanError::InvalidInput("label ref is required".into()));
    }
    let label = conn
        .query_row(
            "SELECT id,board_id,name FROM labels WHERE board_id=?1 AND name=?2",
            params![board_id, label_ref],
            |row| {
                Ok(ResolvedLabel {
                    id: row.get(0)?,
                    board_id: row.get(1)?,
                    name: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(storage)?;
    let label = if label.is_some() || !label_ref.starts_with("l_") {
        label
    } else {
        conn.query_row(
            "SELECT id,board_id,name FROM labels WHERE board_id=?1 AND id=?2",
            params![board_id, label_ref],
            |row| {
                Ok(ResolvedLabel {
                    id: row.get(0)?,
                    board_id: row.get(1)?,
                    name: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(storage)?
    };
    label.ok_or_else(|| KanbanError::NotFound(format!("label {label_ref}")))
}

fn resolve_label_by_id_exact(
    conn: &Connection,
    board_id: &str,
    label_id: &str,
) -> Result<ResolvedLabel> {
    let label_id = label_id.trim();
    if label_id.is_empty() {
        return Err(KanbanError::InvalidInput("label id is required".into()));
    }
    if !label_id.starts_with("l_") {
        return Err(KanbanError::InvalidInput(
            "label id must be a canonical l_ id".into(),
        ));
    }
    conn.query_row(
        "SELECT id,board_id,name FROM labels WHERE board_id=?1 AND id=?2",
        params![board_id, label_id],
        |row| {
            Ok(ResolvedLabel {
                id: row.get(0)?,
                board_id: row.get(1)?,
                name: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(storage)?
    .ok_or_else(|| KanbanError::NotFound(format!("label {label_id}")))
}

struct ResolvedLabel {
    id: String,
    board_id: String,
    name: String,
}

fn label_atom_index_status_from_base(
    conn: &Connection,
    board_id: &str,
    mut status: VectorStoreStatus,
) -> Result<VectorStoreStatus> {
    let state = derived_status_by_name(conn, LANCEDB_LABEL_ATOMS_STORE)?;
    let board = label_atom_index_board_status(conn, board_id)?;
    status.dirty = Some(state.dirty);
    status.board_dirty = Some(board.dirty);
    status.generation = board.last_rebuild_at;
    if !status.enabled {
        push_status_diagnostic(&mut status.diagnostics, "label_atom_index_disabled");
    }
    if state.dirty || board.dirty {
        push_status_diagnostic(&mut status.diagnostics, "label_atom_index_dirty");
    }
    if state.last_error.is_some() || board.last_error.is_some() {
        push_status_diagnostic(&mut status.diagnostics, "label_atom_index_error");
    }
    status.message = format!(
        "{}; dirty={} last_error={}; board_dirty={} board_last_rebuild_at={} board_last_error={}",
        status.message,
        state.dirty,
        state.last_error.as_deref().unwrap_or("none"),
        board.dirty,
        board
            .last_rebuild_at
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        board.last_error.as_deref().unwrap_or("none")
    );
    Ok(status)
}

fn push_status_diagnostic(diagnostics: &mut Vec<String>, code: &str) {
    if !diagnostics.iter().any(|diagnostic| diagnostic == code) {
        diagnostics.push(code.to_owned());
    }
}

pub(crate) fn mark_label_atom_store_dirty(
    conn: &Connection,
    board_id: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, 1, NULL, NULL, NULL, ?3) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, DERIVED_STORE_SCHEMA_VERSION, now],
    )
    .map_err(storage)?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(store_name, board_id, dirty, last_rebuild_at, last_error, updated_at) \
         VALUES (?1, ?2, 1, NULL, NULL, ?3) \
         ON CONFLICT(store_name, board_id) DO UPDATE SET dirty=1, last_error=NULL, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, board_id, now],
    )
    .map_err(storage)?;
    Ok(())
}

fn mark_label_atom_store_success(conn: &Connection, board_id: &str, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO label_atom_index_boards(store_name, board_id, dirty, last_rebuild_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, ?3, NULL, ?3) \
         ON CONFLICT(store_name, board_id) DO UPDATE SET dirty=0, last_rebuild_at=excluded.last_rebuild_at, last_error=NULL, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, board_id, now],
    )
    .map_err(storage)?;
    let dirty = has_dirty_label_atom_boards(conn)?;
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, ?3, ?4, NULL, NULL, ?5) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=excluded.dirty, last_rebuild_at=COALESCE(excluded.last_rebuild_at, derived_store_state.last_rebuild_at), last_error=CASE WHEN excluded.dirty=0 THEN NULL ELSE derived_store_state.last_error END, updated_at=excluded.updated_at",
        params![
            LANCEDB_LABEL_ATOMS_STORE,
            DERIVED_STORE_SCHEMA_VERSION,
            i64::from(dirty),
            now,
            now
        ],
    )
    .map_err(storage)?;
    Ok(())
}

fn mark_label_atom_store_failure(
    conn: &Connection,
    board_id: &str,
    error: &str,
    now: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO derived_store_state(store_name, schema_version, last_event_id, dirty, last_rebuild_at, last_sync_at, last_error, updated_at) \
         VALUES (?1, ?2, 0, 1, NULL, NULL, ?3, ?4) \
         ON CONFLICT(store_name) DO UPDATE SET dirty=1, last_error=excluded.last_error, updated_at=excluded.updated_at",
        params![
            LANCEDB_LABEL_ATOMS_STORE,
            DERIVED_STORE_SCHEMA_VERSION,
            error,
            now
        ],
    )
    .map_err(storage)?;
    conn.execute(
        "INSERT INTO label_atom_index_boards(store_name, board_id, dirty, last_rebuild_at, last_error, updated_at) \
         VALUES (?1, ?2, 1, NULL, ?3, ?4) \
         ON CONFLICT(store_name, board_id) DO UPDATE SET dirty=1, last_error=excluded.last_error, updated_at=excluded.updated_at",
        params![LANCEDB_LABEL_ATOMS_STORE, board_id, error, now],
    )
    .map_err(storage)?;
    Ok(())
}

fn has_dirty_label_atom_boards(conn: &Connection) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM label_atom_index_boards WHERE store_name=?1 AND dirty=1)",
        [LANCEDB_LABEL_ATOMS_STORE],
        |row| row.get(0),
    )
    .map_err(storage)
}

struct LabelAtomIndexBoardStatus {
    dirty: bool,
    last_rebuild_at: Option<i64>,
    last_error: Option<String>,
}

fn label_atom_index_board_status(
    conn: &Connection,
    board_id: &str,
) -> Result<LabelAtomIndexBoardStatus> {
    conn.query_row(
        "SELECT dirty,last_rebuild_at,last_error \
         FROM label_atom_index_boards WHERE store_name=?1 AND board_id=?2",
        params![LANCEDB_LABEL_ATOMS_STORE, board_id],
        |row| {
            Ok(LabelAtomIndexBoardStatus {
                dirty: row.get::<_, bool>(0)?,
                last_rebuild_at: row.get(1)?,
                last_error: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(storage)
    .map(|status| {
        status.unwrap_or(LabelAtomIndexBoardStatus {
            dirty: false,
            last_rebuild_at: None,
            last_error: None,
        })
    })
}

fn normalize_optional_text(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn normalize_text_list(items: Vec<String>) -> Vec<String> {
    items
        .into_iter()
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

fn remove_semantics_items(target: &mut Vec<String>, removals: Vec<String>) {
    let removals = normalize_text_list(removals)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if removals.is_empty() {
        return;
    }
    target.retain(|item| !removals.contains(item.trim()));
}

fn append_semantics_items(target: &mut Vec<String>, additions: Vec<String>) {
    let mut existing = target
        .iter()
        .map(|item| item.trim().to_owned())
        .collect::<BTreeSet<_>>();
    for item in normalize_text_list(additions) {
        if existing.insert(item.clone()) {
            target.push(item);
        }
    }
}

fn label_semantics_record_hash(record: &LabelSemanticsRecord) -> Result<String> {
    let snapshot = serde_json::to_string(&serde_json::json!({
        "label_id": &record.label_id,
        "label_name": &record.label_name,
        "description": &record.description,
        "applies_when": &record.applies_when,
        "excludes_when": &record.excludes_when,
        "positive_examples": &record.positive_examples,
        "negative_examples": &record.negative_examples,
    }))
    .map_err(|err| KanbanError::InvalidInput(err.to_string()))?;
    Ok(stable_hash(&snapshot))
}

fn json_array(items: &[String]) -> Result<String> {
    serde_json::to_string(items).map_err(|err| KanbanError::InvalidInput(err.to_string()))
}

fn json_vec(json: String) -> Result<Vec<String>> {
    serde_json::from_str(&json).map_err(|err| KanbanError::Storage(err.to_string()))
}

fn polarity_to_str(polarity: LabelAtomPolarity) -> &'static str {
    match polarity {
        LabelAtomPolarity::Positive => "positive",
        LabelAtomPolarity::Negative => "negative",
    }
}

fn kind_to_str(kind: LabelAtomKind) -> &'static str {
    match kind {
        LabelAtomKind::Name => "name",
        LabelAtomKind::Description => "description",
        LabelAtomKind::AppliesWhen => "applies_when",
        LabelAtomKind::PositiveExample => "positive_example",
        LabelAtomKind::ExcludesWhen => "excludes_when",
        LabelAtomKind::NegativeExample => "negative_example",
    }
}

fn normalize_atom_text(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn stable_hash(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
