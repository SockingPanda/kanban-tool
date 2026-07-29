use crate::db::connect_file;

use super::{
    DoctorReport, ExportResult, ImportResult, board_id,
    comment_metadata::normalize_comment_metadata_json,
    connect_existing_database, doctor_report_conn, mark_label_atom_store_dirty,
    portable::{
        ExportScope, decode_portable_record, encode_portable_output_envelope,
        portable_record_catalog, portable_record_descriptor,
    },
    storage, with_immediate_tx, with_read_tx,
};

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_event_id};

use rusqlite::{
    Connection, OptionalExtension, params, params_from_iter, types::Value, types::ValueRef,
};

use serde_json::{Map, json};

pub fn export_jsonl(
    path: impl AsRef<Path>,
    board: &str,
    out_path: impl AsRef<Path>,
) -> Result<ExportResult> {
    let conn = connect_existing_database(path.as_ref())?;
    let out_path = out_path.as_ref();
    if out_path.exists() {
        return Err(KanbanError::InvalidInput(format!(
            "export target already exists: {}",
            out_path.display()
        )));
    }
    if let Some(parent) = out_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| KanbanError::Storage(error.to_string()))?;
    }
    let export_now = SystemClock.now_ms();
    let (records, temp_path) = with_read_tx(&conn, || {
        let (temp_path, mut file) = create_temp_export_file(out_path)?;
        let records = write_jsonl_export(&conn, board, &mut file, export_now)?;
        file.sync_all()
            .map_err(|error| KanbanError::Storage(error.to_string()))?;
        Ok((records, temp_path))
    })?;
    if let Err(error) = fs::rename(&temp_path, out_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(KanbanError::Storage(error.to_string()));
    }
    Ok(ExportResult {
        out_path: out_path.to_path_buf(),
        records,
    })
}

pub fn export_jsonl_to_writer(
    path: impl AsRef<Path>,
    board: &str,
    writer: &mut impl Write,
) -> Result<usize> {
    let conn = connect_existing_database(path.as_ref())?;
    let export_now = SystemClock.now_ms();
    with_read_tx(&conn, || {
        write_jsonl_export(&conn, board, writer, export_now)
    })
}

fn write_jsonl_export(
    conn: &Connection,
    board: &str,
    writer: &mut impl Write,
    export_now: i64,
) -> Result<usize> {
    let board_id = board_id(conn, board)?;
    let mut records = 0;
    let portable_records = portable_record_catalog()?;
    for descriptor in portable_records
        .iter()
        .filter(|descriptor| descriptor.scope != ExportScope::Global)
    {
        let (where_sql, params) = match descriptor.scope {
            ExportScope::SelectedBoard => ("WHERE id=?", vec![Value::Text(board_id.clone())]),
            ExportScope::BoardScoped => ("WHERE board_id=?", vec![Value::Text(board_id.clone())]),
            ExportScope::Global => unreachable!("global descriptors are exported after events"),
        };
        records += write_jsonl_table(
            conn,
            writer,
            descriptor.contract.discriminator,
            descriptor.table,
            where_sql,
            params,
            export_now,
        )?;
    }
    records += write_export_sanitized_events(conn, writer, &board_id, export_now)?;
    for descriptor in portable_records
        .iter()
        .filter(|descriptor| descriptor.scope == ExportScope::Global)
    {
        records += write_jsonl_table(
            conn,
            writer,
            descriptor.contract.discriminator,
            descriptor.table,
            "",
            Vec::new(),
            export_now,
        )?;
    }
    Ok(records)
}

pub fn import_jsonl(
    path: impl AsRef<Path>,
    input_path: impl AsRef<Path>,
    replace: bool,
) -> Result<ImportResult> {
    let db_path = path.as_ref();
    let conn = connect_file(db_path)?;
    if !replace && database_has_user_records(&conn)? {
        return Err(KanbanError::InvalidInput(
            "import requires --replace when the database already has records".into(),
        ));
    }
    let input_path = input_path.as_ref();
    let file = File::open(input_path).map_err(|error| KanbanError::Storage(error.to_string()))?;
    with_immediate_tx(&conn, || {
        conn.execute_batch("PRAGMA defer_foreign_keys = ON;")
            .map_err(storage)?;
        if replace {
            for table in IMPORT_DELETE_ORDER {
                conn.execute(&format!("DELETE FROM {table}"), [])
                    .map_err(storage)?;
            }
        }
        let mut records = 0;
        let mut import_format = PortableImportFormat::Unknown;
        let mut deferred_ontology_links = DeferredOntologyLinks::default();
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| KanbanError::Storage(error.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line)
                .map_err(|error| KanbanError::InvalidInput(error.to_string()))?;
            insert_jsonl_record(
                &conn,
                &value,
                &mut import_format,
                &mut deferred_ontology_links,
            )?;
            records += 1;
        }
        restore_deferred_ontology_links(&conn, &deferred_ontology_links)?;
        if replace {
            rebuild_imported_derived_substrate(&conn)?;
        }
        validate_imported_ontology_ledger(&conn)?;
        reject_imported_active_claims(&conn)?;
        validate_imported_snapshot(&conn)?;
        mark_imported_label_atom_boards_dirty(&conn)?;
        let report = doctor_report_conn(&conn, db_path.parent())?;
        if !report.ok {
            return Err(KanbanError::InvalidInput(import_doctor_failure_message(
                &report,
            )));
        }
        Ok(ImportResult {
            input_path: input_path.to_path_buf(),
            records,
        })
    })
}

pub(crate) const IMPORT_DELETE_ORDER: &[&str] = &[
    "projection_deliveries",
    "index_outbox",
    "entity_relations",
    "entities",
    "label_ontology_action_signals",
    "label_ontology_action_atom_effects",
    "label_ontology_actions",
    "label_ontology_signals",
    "label_ontology_observations",
    "signals",
    "signal_observations",
    "task_labels",
    "label_semantic_proposals",
    "label_atoms",
    "label_semantics",
    "labels",
    "task_attachments",
    "task_events",
    "task_comments",
    "task_runs",
    "task_dependencies",
    "tasks",
    "board_columns",
    "boards",
    "app_settings",
];

fn rebuild_imported_derived_substrate(conn: &Connection) -> Result<()> {
    crate::init::ensure_knowledge_substrate(conn)?;
    let now = SystemClock.now_ms();
    conn.execute(
        "UPDATE projection_store_state
         SET control_plane='legacy',
             active_generation=NULL,active_fingerprint=NULL,active_fence_epoch=NULL,
             active_snapshot_cursor=NULL,
             previous_generation=NULL,previous_fingerprint=NULL,previous_fence_epoch=NULL,
             previous_snapshot_cursor=NULL,
             previous_provider=NULL,previous_provider_fingerprint=NULL,
             previous_canonical_count=NULL,previous_canonical_digest=NULL,
             previous_delivery_count=NULL,previous_delivery_digest=NULL,
             building_generation=NULL,building_fingerprint=NULL,building_fence_epoch=NULL,
             building_provider=NULL,building_provider_fingerprint=NULL,
             building_canonical_count=NULL,building_canonical_digest=NULL,
             building_delivery_count=NULL,building_delivery_digest=NULL,building_phase=NULL,
             active_provider=NULL,active_provider_fingerprint=NULL,
             active_canonical_count=NULL,active_canonical_digest=NULL,
             active_delivery_count=NULL,active_delivery_digest=NULL,
             snapshot_cursor=0,checkpoint_cursor=0,legacy_checkpoint_cursor=0,
             lifecycle_status='bootstrap_required',fence_epoch=fence_epoch+1,
             lease_owner=NULL,lease_token=NULL,lease_expires_at=NULL,
             last_success_at=NULL,last_error=NULL,updated_at=?1",
        [now],
    )
    .map_err(storage)?;
    conn.execute(
        "UPDATE derived_store_state
         SET last_event_id=0,dirty=1,last_rebuild_at=NULL,last_sync_at=NULL,
             last_error=NULL,updated_at=?1",
        [now],
    )
    .map_err(storage)?;
    conn.execute(
        "INSERT INTO index_outbox(
           source_event_id,target,entity_uri,action,payload_json,status,attempts,last_error,
           created_at,updated_at
         )
         SELECT NULL,'all','kb://board/' || id,'rebuild','{}','pending',0,NULL,?1,?1
         FROM boards
         ORDER BY id",
        [now],
    )
    .map_err(storage)?;
    Ok(())
}

pub(crate) fn write_jsonl_table(
    conn: &Connection,
    writer: &mut impl Write,
    record_type: &str,
    table: &str,
    where_sql: &str,
    params: Vec<Value>,
    export_now: i64,
) -> Result<usize> {
    let sql = format!("SELECT * FROM {table} {where_sql}");
    let mut stmt = conn.prepare(&sql).map_err(storage)?;
    let columns = stmt
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = stmt
        .query(params_from_iter(params.iter()))
        .map_err(storage)?;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(storage)? {
        let mut data = serde_json::Map::new();
        for (index, column) in columns.iter().enumerate() {
            data.insert(
                column.clone(),
                value_ref_to_json(row.get_ref(index).map_err(storage)?),
            );
        }
        scrub_jsonl_export_record(record_type, &mut data, export_now);
        let record = encode_portable_output_envelope(record_type, data)?;
        writeln!(writer, "{record}").map_err(|error| KanbanError::Storage(error.to_string()))?;
        count += 1;
    }
    Ok(count)
}

pub(crate) fn write_export_sanitized_events(
    conn: &Connection,
    writer: &mut impl Write,
    board_id: &str,
    export_now: i64,
) -> Result<usize> {
    let mut stmt = conn
        .prepare(
            "SELECT id,current_run_id,claim_owner,claim_expires_at \
             FROM tasks WHERE board_id=?1 AND status='running' ORDER BY id ASC",
        )
        .map_err(storage)?;
    let tasks = stmt
        .query_map([board_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    if tasks.is_empty() {
        return Ok(0);
    }

    let mut next_id: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(id), 0) + 1 FROM task_events WHERE board_id=?1",
            [board_id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    let first_id = next_id;
    for (task_id, run_id, claim_owner, claim_expires_at) in tasks {
        let payload = json!({
            "from_status": "running",
            "to_status": "ready",
            "run_status": "canceled",
            "original_run_id": run_id,
            "claim_owner": claim_owner,
            "claim_expires_at": claim_expires_at,
            "reason": "jsonl export clears non-portable live claim"
        })
        .to_string();
        let data = json!({
            "id": next_id,
            "event_id": new_event_id(),
            "board_id": board_id,
            "task_id": task_id,
            "run_id": run_id,
            "kind": "task.export_sanitized",
            "actor": "kanban export",
            "payload_json": payload,
            "created_at": export_now
        })
        .as_object()
        .cloned()
        .expect("synthetic export event data is an object");
        let record = encode_portable_output_envelope("event", data)?;
        writeln!(writer, "{record}").map_err(|error| KanbanError::Storage(error.to_string()))?;
        next_id += 1;
    }
    Ok((next_id - first_id) as usize)
}

pub(crate) fn scrub_jsonl_export_record(
    record_type: &str,
    data: &mut serde_json::Map<String, serde_json::Value>,
    export_now: i64,
) {
    if record_type == "task"
        && data
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|status| status == "running")
    {
        data.insert("status".into(), json!("ready"));
        data.insert("claim_token".into(), serde_json::Value::Null);
        data.insert("claim_owner".into(), serde_json::Value::Null);
        data.insert("claim_expires_at".into(), serde_json::Value::Null);
        data.insert("last_heartbeat_at".into(), serde_json::Value::Null);
        data.insert("current_run_id".into(), serde_json::Value::Null);
        data.insert("started_at".into(), serde_json::Value::Null);
    }

    if record_type == "run" {
        data.insert("log_path".into(), serde_json::Value::Null);
        if data
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|status| status == "running")
        {
            data.insert("status".into(), json!("canceled"));
            data.insert("finished_at".into(), json!(export_now));
            data.insert(
                "error".into(),
                json!("canceled by jsonl export; claim is not portable"),
            );
        }
    }
}

pub(crate) fn validate_imported_snapshot(conn: &Connection) -> Result<()> {
    let board_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .map_err(storage)?;
    if board_count == 0 {
        return Err(KanbanError::InvalidInput(
            "imported data must contain at least one board".into(),
        ));
    }

    let boards_without_columns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM boards b \
             WHERE NOT EXISTS (SELECT 1 FROM board_columns c WHERE c.board_id=b.id)",
            [],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if boards_without_columns > 0 {
        return Err(KanbanError::InvalidInput(
            "imported data must contain columns for every board".into(),
        ));
    }
    Ok(())
}

fn import_doctor_failure_message(report: &DoctorReport) -> String {
    report
        .consistency_issues
        .iter()
        .chain(report.ontology_ledger_issues.iter())
        .find(|issue| issue.severity == "error")
        .map(|issue| format!("imported data failed doctor checks: {}", issue.message))
        .unwrap_or_else(|| "imported data failed doctor checks".to_owned())
}

fn mark_imported_label_atom_boards_dirty(conn: &Connection) -> Result<()> {
    let now = SystemClock.now_ms();
    let mut stmt = conn
        .prepare(
            "SELECT board_id FROM label_semantics \
             UNION \
             SELECT board_id FROM label_atoms \
             ORDER BY board_id ASC",
        )
        .map_err(storage)?;
    let board_ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(storage)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(storage)?;
    for board_id in board_ids {
        mark_label_atom_store_dirty(conn, &board_id, now)?;
    }
    Ok(())
}

pub(crate) fn create_temp_export_file(out_path: &Path) -> Result<(PathBuf, File)> {
    let file_name = out_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("export.jsonl");
    let parent = out_path.parent().unwrap_or_else(|| Path::new("."));
    for attempt in 0..100 {
        let temp_path = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            SystemClock.now_ms() + attempt
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => return Ok((temp_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(KanbanError::Storage(error.to_string())),
        }
    }
    Err(KanbanError::Storage(format!(
        "failed to create temporary export file next to {}",
        out_path.display()
    )))
}

pub(crate) fn reject_imported_active_claims(conn: &Connection) -> Result<()> {
    let now = SystemClock.now_ms();
    let active_running_tasks: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tasks WHERE status='running' AND claim_expires_at > ?1",
            [now],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if active_running_tasks > 0 {
        return Err(KanbanError::InvalidInput(
            "imported data contains active running claims".into(),
        ));
    }
    Ok(())
}

pub(crate) fn value_ref_to_json(value: ValueRef<'_>) -> serde_json::Value {
    match value {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => json!(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => json!(format!("hex:{}", hex_bytes(value))),
    }
}

pub(crate) fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub(crate) fn database_has_user_records(conn: &Connection) -> Result<bool> {
    for table in [
        "boards",
        "board_columns",
        "tasks",
        "task_dependencies",
        "task_runs",
        "task_comments",
        "task_events",
        "task_attachments",
        "signal_observations",
        "signals",
        "labels",
        "label_semantics",
        "label_atoms",
        "label_semantic_proposals",
        "label_ontology_observations",
        "label_ontology_signals",
        "label_ontology_actions",
        "label_ontology_action_atom_effects",
        "label_ontology_action_signals",
        "task_labels",
        "app_settings",
    ] {
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(storage)?;
        if count > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Debug, Default)]
struct DeferredOntologyLinks {
    generic_signal_supersedes: Vec<(String, String)>,
    signal_supersedes: Vec<(String, String)>,
    action_parents: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortableImportFormat {
    Unknown,
    Natural,
    ParentStorageNative,
}

fn insert_jsonl_record(
    conn: &Connection,
    record: &serde_json::Value,
    import_format: &mut PortableImportFormat,
    deferred_ontology_links: &mut DeferredOntologyLinks,
) -> Result<()> {
    let record_type = record
        .get("type")
        .and_then(|value| value.as_str())
        .ok_or_else(|| KanbanError::InvalidInput("export record type is required".into()))?;
    let descriptor = portable_record_descriptor(record_type)?;
    let table = descriptor.table;
    let mut data = descriptor
        .contract
        .decode_input_envelope(record.clone())
        .map_err(|error| {
            KanbanError::InvalidInput(format!("top-level JSONL contract violation: {error}"))
        })?;
    // Reject before format detection and legacy key removal can overwrite an explicitly supplied
    // natural value.
    reject_hybrid_parent_storage_native_record(record_type, &data)?;
    detect_portable_import_format(record_type, &data, import_format)?;
    if *import_format == PortableImportFormat::ParentStorageNative {
        migrate_parent_storage_native_record(record_type, &mut data)?;
    }
    let mut data = decode_portable_record(record_type, data)?;
    validate_import_record(record_type, &data)?;
    capture_deferred_ontology_links(record_type, &mut data, deferred_ontology_links)?;
    if data.is_empty() {
        return Err(KanbanError::InvalidInput(
            "export record data cannot be empty".into(),
        ));
    }
    let columns = data.keys().map(String::as_str).collect::<Vec<_>>();
    if columns.iter().any(|column| !is_sql_identifier(column)) {
        return Err(KanbanError::InvalidInput(
            "export record contains an invalid column name".into(),
        ));
    }
    let placeholders = std::iter::repeat_n("?", columns.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "INSERT OR REPLACE INTO {table} ({}) VALUES ({placeholders})",
        columns.join(",")
    );
    let values = columns
        .iter()
        .map(|column| json_to_sql_value(&data[*column]))
        .collect::<Result<Vec<_>>>()?;
    conn.execute(&sql, params_from_iter(values.iter()))
        .map_err(storage)?;
    Ok(())
}

fn reject_hybrid_parent_storage_native_record(
    record_type: &str,
    data: &Map<String, serde_json::Value>,
) -> Result<()> {
    let mut natural_fields = Vec::new();
    let mut storage_fields = Vec::new();
    for &(storage_field, wire_field, _) in parent_storage_native_json_fields(record_type) {
        if storage_field == wire_field {
            continue;
        }
        if data.contains_key(wire_field) {
            natural_fields.push(wire_field);
        }
        if data.contains_key(storage_field) {
            storage_fields.push(storage_field);
        }
    }
    if natural_fields.is_empty() || storage_fields.is_empty() {
        return Ok(());
    }
    Err(KanbanError::InvalidInput(format!(
        "JSONL {record_type} record cannot contain both natural and parent storage-native keys: natural [{}], storage-native [{}]",
        natural_fields.join(", "),
        storage_fields.join(", ")
    )))
}

fn detect_portable_import_format(
    record_type: &str,
    data: &Map<String, serde_json::Value>,
    import_format: &mut PortableImportFormat,
) -> Result<()> {
    if record_type == "column" {
        let detected = match data.get("hidden") {
            Some(serde_json::Value::Bool(_)) => Some(PortableImportFormat::Natural),
            Some(serde_json::Value::Number(value)) if matches!(value.as_i64(), Some(0 | 1)) => {
                Some(PortableImportFormat::ParentStorageNative)
            }
            _ => None,
        };
        if let Some(detected) = detected {
            merge_portable_import_format(import_format, detected)?;
        }
    }

    for &(storage_field, wire_field, _) in parent_storage_native_json_fields(record_type) {
        let detected = if storage_field == wire_field {
            data.get(storage_field).map(|value| {
                if value.is_string() {
                    PortableImportFormat::ParentStorageNative
                } else {
                    PortableImportFormat::Natural
                }
            })
        } else if data.contains_key(storage_field) {
            Some(PortableImportFormat::ParentStorageNative)
        } else if data.contains_key(wire_field) {
            Some(PortableImportFormat::Natural)
        } else {
            None
        };
        if let Some(detected) = detected {
            merge_portable_import_format(import_format, detected)?;
        }
    }
    Ok(())
}

fn merge_portable_import_format(
    import_format: &mut PortableImportFormat,
    detected: PortableImportFormat,
) -> Result<()> {
    match *import_format {
        PortableImportFormat::Unknown => *import_format = detected,
        current if current == detected => {}
        _ => {
            return Err(KanbanError::InvalidInput(
                "JSONL import cannot mix natural and parent storage-native records".into(),
            ));
        }
    }
    Ok(())
}

fn migrate_parent_storage_native_record(
    record_type: &str,
    data: &mut Map<String, serde_json::Value>,
) -> Result<()> {
    for &(storage_field, wire_field, nullable) in parent_storage_native_json_fields(record_type) {
        let Some(stored) = data.remove(storage_field) else {
            continue;
        };
        let natural = match stored {
            serde_json::Value::Null if nullable => serde_json::Value::Null,
            serde_json::Value::String(text) => serde_json::from_str(&text).map_err(|error| {
                KanbanError::InvalidInput(format!(
                    "legacy {record_type}.{storage_field} contains invalid JSON: {error}"
                ))
            })?,
            _ => {
                return Err(KanbanError::InvalidInput(format!(
                    "legacy {record_type}.{storage_field} must be JSON text{}",
                    if nullable { " or null" } else { "" }
                )));
            }
        };
        data.insert(wire_field.to_owned(), natural);
    }

    let boolean_fields: &[&str] = match record_type {
        "column" => &["hidden"],
        "label_ontology_observation" => &["suggest_needs_new_label", "suggest_degraded"],
        "label_ontology_signal" => &["agent_selected", "final_selected"],
        _ => &[],
    };
    for &field in boolean_fields {
        let Some(value) = data.get_mut(field) else {
            continue;
        };
        *value = match value.as_i64() {
            Some(0) => serde_json::Value::Bool(false),
            Some(1) => serde_json::Value::Bool(true),
            _ => {
                return Err(KanbanError::InvalidInput(format!(
                    "legacy {record_type}.{field} must be SQLite 0 or 1"
                )));
            }
        };
    }
    Ok(())
}

fn parent_storage_native_json_fields(
    record_type: &str,
) -> &'static [(&'static str, &'static str, bool)] {
    match record_type {
        "task" => &[
            ("result_json", "result", true),
            ("metadata_json", "metadata", false),
        ],
        "run" | "comment" => &[("metadata_json", "metadata", false)],
        "event" => &[("payload_json", "payload", false)],
        "label_semantics" => &[
            ("applies_when", "applies_when", false),
            ("excludes_when", "excludes_when", false),
            ("positive_examples", "positive_examples", false),
            ("negative_examples", "negative_examples", false),
        ],
        "label_semantic_proposal" => &[
            ("applies_when", "applies_when", false),
            ("excludes_when", "excludes_when", false),
            ("positive_examples", "positive_examples", false),
            ("negative_examples", "negative_examples", false),
            ("diagnostics_json", "diagnostics", false),
        ],
        "label_ontology_observation" => &[
            ("task_snapshot_json", "task_snapshot", false),
            ("agent_candidates_json", "agent_candidates", false),
            ("suggestion_snapshot_json", "suggestion_snapshot", false),
            ("final_decision_json", "final_decision", false),
            ("diagnostics_json", "diagnostics", false),
        ],
        "label_ontology_signal" => &[
            ("related_labels_json", "related_labels", false),
            ("proposal_json", "proposal", false),
        ],
        "label_ontology_action" => &[
            ("change_json", "change", false),
            ("validation_json", "validation", false),
        ],
        "signal_observation" => &[("evidence_json", "evidence", false)],
        "setting" => &[("value_json", "value", false)],
        _ => &[],
    }
}

fn capture_deferred_ontology_links(
    record_type: &str,
    data: &mut Map<String, serde_json::Value>,
    deferred_ontology_links: &mut DeferredOntologyLinks,
) -> Result<()> {
    match record_type {
        "signal" => {
            if let Some(target) = take_optional_string_field(data, "superseded_by_signal_id")? {
                let id = required_import_id(data, "signal")?;
                deferred_ontology_links
                    .generic_signal_supersedes
                    .push((id, target));
            }
        }
        "label_ontology_signal" => {
            if let Some(target) = take_optional_string_field(data, "superseded_by_signal_id")? {
                let id = required_import_id(data, "label ontology signal")?;
                deferred_ontology_links.signal_supersedes.push((id, target));
            }
        }
        "label_ontology_action" => {
            if let Some(parent) = take_optional_string_field(data, "parent_action_id")? {
                let id = required_import_id(data, "label ontology action")?;
                deferred_ontology_links.action_parents.push((id, parent));
            }
        }
        _ => {}
    }
    Ok(())
}

fn take_optional_string_field(
    data: &mut Map<String, serde_json::Value>,
    field: &str,
) -> Result<Option<String>> {
    let value = match data.get(field) {
        Some(serde_json::Value::Null) | None => return Ok(None),
        Some(serde_json::Value::String(value)) => value.trim().to_owned(),
        Some(_) => {
            return Err(KanbanError::InvalidInput(format!(
                "{field} must be a string or null"
            )));
        }
    };
    if value.is_empty() {
        return Err(KanbanError::InvalidInput(format!(
            "{field} cannot be empty"
        )));
    }
    data.insert(field.to_owned(), serde_json::Value::Null);
    Ok(Some(value))
}

fn required_import_id(data: &Map<String, serde_json::Value>, context: &str) -> Result<String> {
    data.get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| KanbanError::InvalidInput(format!("{context} import record requires id")))
}

fn restore_deferred_ontology_links(
    conn: &Connection,
    deferred_ontology_links: &DeferredOntologyLinks,
) -> Result<()> {
    for (signal_id, replacement_id) in &deferred_ontology_links.generic_signal_supersedes {
        validate_generic_signal_supersede_link(conn, signal_id, replacement_id)?;
        conn.execute(
            "UPDATE signals SET superseded_by_signal_id=?1 WHERE id=?2",
            params![replacement_id, signal_id],
        )
        .map_err(storage)?;
    }
    for (signal_id, replacement_id) in &deferred_ontology_links.signal_supersedes {
        validate_signal_supersede_link(conn, signal_id, replacement_id)?;
        conn.execute(
            "UPDATE label_ontology_signals SET superseded_by_signal_id=?1 WHERE id=?2",
            params![replacement_id, signal_id],
        )
        .map_err(storage)?;
    }
    for (action_id, parent_action_id) in &deferred_ontology_links.action_parents {
        validate_action_parent_link(conn, action_id, parent_action_id)?;
        conn.execute(
            "UPDATE label_ontology_actions SET parent_action_id=?1 WHERE id=?2",
            params![parent_action_id, action_id],
        )
        .map_err(storage)?;
    }
    Ok(())
}

fn validate_generic_signal_supersede_link(
    conn: &Connection,
    signal_id: &str,
    replacement_id: &str,
) -> Result<()> {
    if signal_id == replacement_id {
        return Err(KanbanError::InvalidInput(
            "signal supersede self-reference".into(),
        ));
    }
    let boards = conn
        .query_row(
            "SELECT s.board_id, r.board_id \
             FROM signals s \
             JOIN signals r ON r.id=?2 \
             WHERE s.id=?1",
            params![signal_id, replacement_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(storage)?
        .ok_or_else(|| {
            KanbanError::InvalidInput("signal supersede link references missing signal".into())
        })?;
    if boards.0 != boards.1 {
        return Err(KanbanError::InvalidInput(
            "signal supersede board mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_signal_supersede_link(
    conn: &Connection,
    signal_id: &str,
    replacement_id: &str,
) -> Result<()> {
    if signal_id == replacement_id {
        return Err(KanbanError::InvalidInput(
            "label ontology signal supersede self-reference".into(),
        ));
    }
    let boards = conn
        .query_row(
            "SELECT s.board_id, r.board_id \
             FROM label_ontology_signals s \
             JOIN label_ontology_signals r ON r.id=?2 \
             WHERE s.id=?1",
            params![signal_id, replacement_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(storage)?
        .ok_or_else(|| {
            KanbanError::InvalidInput(
                "label ontology signal supersede link references missing signal".into(),
            )
        })?;
    if boards.0 != boards.1 {
        return Err(KanbanError::InvalidInput(
            "label ontology signal supersede board mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_action_parent_link(
    conn: &Connection,
    action_id: &str,
    parent_action_id: &str,
) -> Result<()> {
    if action_id == parent_action_id {
        return Err(KanbanError::InvalidInput(
            "label ontology action parent self-reference".into(),
        ));
    }
    let boards = conn
        .query_row(
            "SELECT a.board_id, p.board_id \
             FROM label_ontology_actions a \
             JOIN label_ontology_actions p ON p.id=?2 \
             WHERE a.id=?1",
            params![action_id, parent_action_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(storage)?
        .ok_or_else(|| {
            KanbanError::InvalidInput(
                "label ontology action parent link references missing action".into(),
            )
        })?;
    if boards.0 != boards.1 {
        return Err(KanbanError::InvalidInput(
            "label ontology action parent board mismatch".into(),
        ));
    }
    Ok(())
}

fn validate_imported_ontology_ledger(conn: &Connection) -> Result<()> {
    reject_exists(
        conn,
        "SELECT s.id FROM label_ontology_signals s \
         JOIN label_ontology_observations o ON o.id=s.observation_id \
         WHERE s.board_id<>o.board_id LIMIT 1",
        "label ontology signal observation board mismatch",
    )?;
    reject_exists(
        conn,
        "SELECT s.id FROM label_ontology_signals s \
         JOIN labels l ON l.id=s.target_label_id \
         WHERE s.target_label_id IS NOT NULL AND s.board_id<>l.board_id LIMIT 1",
        "label ontology signal target label board mismatch",
    )?;
    reject_exists(
        conn,
        "SELECT s.id FROM label_ontology_signals s \
         JOIN label_ontology_signals r ON r.id=s.superseded_by_signal_id \
         WHERE s.superseded_by_signal_id IS NOT NULL AND s.board_id<>r.board_id LIMIT 1",
        "label ontology signal supersede board mismatch",
    )?;
    reject_exists(
        conn,
        "SELECT a.id FROM label_ontology_actions a \
         JOIN label_ontology_actions p ON p.id=a.parent_action_id \
         WHERE a.parent_action_id IS NOT NULL AND a.board_id<>p.board_id LIMIT 1",
        "label ontology action parent board mismatch",
    )?;
    reject_exists(
        conn,
        "SELECT a.id FROM label_ontology_actions a \
         JOIN labels l ON l.id=a.target_label_id \
         WHERE a.target_label_id IS NOT NULL AND a.board_id<>l.board_id LIMIT 1",
        "label ontology action target label board mismatch",
    )?;
    reject_exists(
        conn,
        "SELECT a.id FROM label_ontology_actions a \
         JOIN labels l ON l.id=a.result_label_id \
         WHERE a.result_label_id IS NOT NULL AND a.board_id<>l.board_id LIMIT 1",
        "label ontology action result label board mismatch",
    )?;
    reject_exists(
        conn,
        "SELECT a.id FROM label_ontology_actions a \
         JOIN label_semantic_proposals p ON p.id=a.result_proposal_id \
         WHERE a.result_proposal_id IS NOT NULL AND a.board_id<>p.board_id LIMIT 1",
        "label ontology action proposal board mismatch",
    )?;
    reject_exists(
        conn,
        "SELECT x.action_id FROM label_ontology_action_signals x \
         LEFT JOIN label_ontology_actions a ON a.id=x.action_id \
         LEFT JOIN label_ontology_signals s ON s.id=x.signal_id \
         WHERE a.id IS NULL OR s.id IS NULL LIMIT 1",
        "label ontology action-signal link references missing action or signal",
    )?;
    reject_exists(
        conn,
        "SELECT x.action_id FROM label_ontology_action_signals x \
         JOIN label_ontology_actions a ON a.id=x.action_id \
         JOIN label_ontology_signals s ON s.id=x.signal_id \
         WHERE x.board_id<>a.board_id OR x.board_id<>s.board_id LIMIT 1",
        "label ontology action-signal board mismatch",
    )?;
    reject_signal_supersede_cycles(conn)?;
    reject_action_parent_cycles(conn)?;
    Ok(())
}

fn reject_exists(conn: &Connection, sql: &str, message: &str) -> Result<()> {
    let found: Option<String> = conn
        .query_row(sql, [], |row| row.get(0))
        .optional()
        .map_err(storage)?;
    if found.is_some() {
        Err(KanbanError::InvalidInput(message.into()))
    } else {
        Ok(())
    }
}

fn reject_signal_supersede_cycles(conn: &Connection) -> Result<()> {
    let links = string_links(
        conn,
        "SELECT id, superseded_by_signal_id FROM label_ontology_signals \
         WHERE superseded_by_signal_id IS NOT NULL",
    )?;
    reject_link_cycles(&links, "label ontology signal supersede cycle")
}

fn reject_action_parent_cycles(conn: &Connection) -> Result<()> {
    let links = string_links(
        conn,
        "SELECT id, parent_action_id FROM label_ontology_actions \
         WHERE parent_action_id IS NOT NULL",
    )?;
    reject_link_cycles(&links, "label ontology action parent cycle")
}

fn string_links(conn: &Connection, sql: &str) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare(sql).map_err(storage)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(storage)?;
    rows.collect::<std::result::Result<HashMap<_, _>, _>>()
        .map_err(storage)
}

fn reject_link_cycles(links: &HashMap<String, String>, message: &str) -> Result<()> {
    for start in links.keys() {
        let mut seen = HashSet::new();
        let mut current = start.as_str();
        while let Some(next) = links.get(current) {
            if !seen.insert(current.to_owned()) {
                return Err(KanbanError::InvalidInput(format!("{message}: {start}")));
            }
            current = next;
        }
    }
    Ok(())
}

pub(crate) fn validate_import_record(
    record_type: &str,
    data: &Map<String, serde_json::Value>,
) -> Result<()> {
    if record_type == "comment" {
        let kind = data
            .get("kind")
            .and_then(|value| value.as_str())
            .ok_or_else(|| KanbanError::InvalidInput("comment kind is required".into()))?;
        let metadata_json = data
            .get("metadata_json")
            .and_then(|value| value.as_str())
            .ok_or_else(|| KanbanError::InvalidInput("comment metadata is required".into()))?;
        normalize_comment_metadata_json(kind, Some(metadata_json))?;
    }
    Ok(())
}

pub(crate) fn is_sql_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
pub(crate) fn import_table_for_type(record_type: &str) -> Result<&'static str> {
    portable_record_descriptor(record_type).map(|descriptor| descriptor.table)
}

pub(crate) fn json_to_sql_value(value: &serde_json::Value) -> Result<Value> {
    match value {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(value) => Ok(Value::Integer(i64::from(*value))),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Ok(Value::Integer(value))
            } else if let Some(value) = value.as_f64() {
                Ok(Value::Real(value))
            } else {
                Err(KanbanError::InvalidInput(
                    "unsupported numeric export value".into(),
                ))
            }
        }
        serde_json::Value::String(value) => Ok(Value::Text(value.clone())),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Ok(Value::Text(value.to_string()))
        }
    }
}

#[cfg(test)]
mod contract_catalog_tests {
    use std::collections::BTreeSet;

    use kanban_contract::{ContractSurface, surface_operation_keys};

    use super::{import_table_for_type, portable_record_catalog};

    #[test]
    fn jsonl_export_discriminators_match_exact_contract_catalog() {
        let portable_records = portable_record_catalog().expect("portable adapter catalog");
        let actual = portable_records
            .iter()
            .map(|descriptor| descriptor.contract.discriminator.to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual.len(),
            portable_records.len(),
            "portable record descriptor 不得包含重复 discriminator"
        );
        let expected = surface_operation_keys(ContractSurface::Jsonl)
            .map(|key| {
                key.strip_prefix("type=")
                    .expect("JSONL operation key 必须使用 type=<discriminator>")
                    .to_owned()
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            actual, expected,
            "新增、删除或重命名 JSONL export type 时必须同步精确 contract catalog"
        );
        for record_type in expected {
            import_table_for_type(&record_type)
                .expect("catalog 中的 JSONL type 必须被 current importer 接受");
        }
    }
}
