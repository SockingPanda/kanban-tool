use crate::connect_file;

use super::{
    ExportResult, ImportResult, board_id, connect_existing_database, doctor_report_conn, storage,
    with_immediate_tx, with_read_tx,
};

use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use kanban_core::{Clock, KanbanError, Result, SystemClock, new_event_id};

use rusqlite::{Connection, params_from_iter, types::Value, types::ValueRef};

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
        let board_id = board_id(&conn, board)?;
        let (temp_path, mut file) = create_temp_export_file(out_path)?;
        let mut records = 0;
        records += write_jsonl_table(
            &conn,
            &mut file,
            "board",
            "boards",
            "WHERE id=?",
            vec![Value::Text(board_id.clone())],
            export_now,
        )?;
        for (record_type, table) in BOARD_SCOPED_EXPORT_TABLES {
            records += write_jsonl_table(
                &conn,
                &mut file,
                record_type,
                table,
                "WHERE board_id=?",
                vec![Value::Text(board_id.clone())],
                export_now,
            )?;
        }
        records += write_export_sanitized_events(&conn, &mut file, &board_id, export_now)?;
        records += write_jsonl_table(
            &conn,
            &mut file,
            "setting",
            "app_settings",
            "",
            Vec::new(),
            export_now,
        )?;
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
        if replace {
            for table in IMPORT_DELETE_ORDER {
                conn.execute(&format!("DELETE FROM {table}"), [])
                    .map_err(storage)?;
            }
        }
        let mut records = 0;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| KanbanError::Storage(error.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&line)
                .map_err(|error| KanbanError::InvalidInput(error.to_string()))?;
            insert_jsonl_record(&conn, &value)?;
            records += 1;
        }
        reject_imported_active_claims(&conn)?;
        validate_imported_snapshot(&conn)?;
        let report = doctor_report_conn(&conn, db_path.parent())?;
        if !report.ok {
            return Err(KanbanError::InvalidInput(
                "imported data failed doctor checks".into(),
            ));
        }
        Ok(ImportResult {
            input_path: input_path.to_path_buf(),
            records,
        })
    })
}

pub(crate) const BOARD_SCOPED_EXPORT_TABLES: &[(&str, &str)] = &[
    ("column", "board_columns"),
    ("task", "tasks"),
    ("dependency", "task_dependencies"),
    ("run", "task_runs"),
    ("comment", "task_comments"),
    ("event", "task_events"),
    ("attachment", "task_attachments"),
    ("label", "labels"),
    ("task_label", "task_labels"),
];

pub(crate) const IMPORT_DELETE_ORDER: &[&str] = &[
    "task_labels",
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
        let record = json!({ "type": record_type, "data": data });
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
        let record = json!({
            "type": "event",
            "data": {
                "id": next_id,
                "event_id": new_event_id(),
                "board_id": board_id,
                "task_id": task_id,
                "run_id": run_id,
                "kind": "task.export_sanitized",
                "actor": "kanban export",
                "payload_json": payload,
                "created_at": export_now
            }
        });
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
        "labels",
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

pub(crate) fn insert_jsonl_record(conn: &Connection, record: &serde_json::Value) -> Result<()> {
    let record_type = record
        .get("type")
        .and_then(|value| value.as_str())
        .ok_or_else(|| KanbanError::InvalidInput("export record type is required".into()))?;
    let table = import_table_for_type(record_type)?;
    let mut data = record
        .get("data")
        .and_then(|value| value.as_object())
        .cloned()
        .ok_or_else(|| KanbanError::InvalidInput("export record data is required".into()))?;
    normalize_import_record(record_type, &mut data);
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

pub(crate) fn normalize_import_record(
    record_type: &str,
    data: &mut Map<String, serde_json::Value>,
) {
    if record_type != "comment" {
        return;
    }
    if !data.contains_key("author_type") {
        let author_type = data
            .get("kind")
            .and_then(|value| value.as_str())
            .map(infer_comment_author_type)
            .unwrap_or("human");
        data.insert("author_type".into(), json!(author_type));
    }
    data.entry("agent_type").or_insert(serde_json::Value::Null);
}

fn infer_comment_author_type(kind: &str) -> &'static str {
    match kind {
        "worker" => "agent",
        "system" => "system",
        _ => "human",
    }
}

pub(crate) fn is_sql_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

pub(crate) fn import_table_for_type(record_type: &str) -> Result<&'static str> {
    match record_type {
        "board" => Ok("boards"),
        "column" => Ok("board_columns"),
        "task" => Ok("tasks"),
        "dependency" => Ok("task_dependencies"),
        "run" => Ok("task_runs"),
        "comment" => Ok("task_comments"),
        "event" => Ok("task_events"),
        "attachment" => Ok("task_attachments"),
        "label" => Ok("labels"),
        "task_label" => Ok("task_labels"),
        "setting" => Ok("app_settings"),
        _ => Err(KanbanError::InvalidInput(format!(
            "unsupported export record type: {record_type}"
        ))),
    }
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
