//! 接入已有 portable JSONL 的 v5 契约。所有钩子必须运行在导入方的同一事务内。
use super::{catalog, diagnostics, migration, store::*};
use crate::StoreError;
use std::collections::BTreeSet;
use turso::Connection;

pub(crate) const CATALOG_TABLES: &[&str] = &[
    "file_model_schema",
    "object_model_schema",
    "object_types",
    "object_properties",
    "object_options",
    "object_type_properties",
    "object_relation_types",
    "object_workflows",
    "object_rollups",
];

/// 旧格式的附件事实视图；转换后的身份和存储路径仍可用于原格式的计数与重放证明。
pub(crate) const LEGACY_ATTACHMENTS_SQL: &str = "SELECT f.object_id AS id,f.board_id,e.target_id AS task_id,f.original_filename AS filename,b.storage_key AS rel_path,f.content_type,b.size_bytes,b.sha256,f.created_by,f.created_at FROM file_objects f JOIN file_blobs b ON b.id=f.blob_id JOIN object_relation_edges e ON e.source_id=f.object_id AND e.board_id=f.board_id AND e.relation_key='file.attachment' JOIN tasks t ON t.id=e.target_id AND t.board_id=e.board_id";

pub(crate) fn schema_fingerprint() -> String {
    format!(
        "{};object-model-v2:{};files-v1:{}",
        crate::migration::full_schema_fingerprint(),
        migration::checksum(),
        super::files::migration::checksum()
    )
}

/// 空目标的内置目录不属于用户数据。自定义类型或属性仍必须阻止无 replace 导入。
pub(crate) async fn custom_catalog_count(c: &Connection) -> Result<i64, StoreError> {
    count(c, "SELECT (SELECT COUNT(*) FROM object_types WHERE system=0) + (SELECT COUNT(*) FROM object_properties WHERE system=0) + (SELECT COUNT(*) FROM object_relation_types WHERE system=0) + (SELECT COUNT(*) FROM object_workflows WHERE system=0) + (SELECT COUNT(*) FROM object_rollups WHERE system=0)", vec![])
        .await.map_err(migration::schema_error)
}

pub(crate) async fn before_import(c: &Connection) -> Result<(), StoreError> {
    if count(c, "SELECT COUNT(*) FROM objects", vec![])
        .await
        .map_err(migration::schema_error)?
        != 0
    {
        return Err(migration::schema_error(
            "导入准备阶段仍有对象，必须先按依赖顺序清空目标",
        ));
    }
    // task rows 和 object identities 都由本次快照严格 INSERT，避免 trigger 重复创建身份。
    c.execute("DROP TRIGGER object_task_identity_insert", ())
        .await?;
    for table in [
        "file_model_schema",
        "object_workflows",
        "object_rollups",
        "object_relation_types",
        "object_type_properties",
        "object_options",
        "object_properties",
        "object_types",
        "object_model_schema",
    ] {
        c.execute(format!("DELETE FROM {table}"), ()).await?;
    }
    Ok(())
}

pub(crate) async fn after_import(c: &Connection) -> Result<(), StoreError> {
    let (_, _, trigger) = migration::ddl_objects()
        .into_iter()
        .find(|(_, name, _)| *name == "object_task_identity_insert")
        .ok_or_else(|| migration::schema_error("缺少任务身份 trigger 定义"))?;
    c.execute_batch(trigger).await?;
    // 快照恢复目录版本；不重新 seed，以免吞掉坏快照或改写用户目录。
    migration::validate(c).await?;
    super::files::migration::validate(c).await?;
    diagnostics::require_valid(c)
        .await
        .map_err(migration::schema_error)
}

/// 旧 portable 只含基础事实。先恢复内置目录，再由任务 trigger 和附件转换建立对象身份。
pub(crate) async fn before_legacy_portable(c: &Connection) -> Result<(), StoreError> {
    before_import(c).await?;
    catalog::seed(c).await.map_err(migration::schema_error)?;
    c.execute("INSERT INTO object_model_schema(singleton,version,checksum,catalog_version,installed_at) VALUES (1,2,?1,1,?2)", (migration::checksum(),crate::shared::now_ms())).await?;
    super::files::migration::seed(c).await?;
    catalog::changed(c).await.map_err(migration::schema_error)?;
    let (_, _, sql) = migration::ddl_objects()
        .into_iter()
        .find(|(_, name, _)| *name == "object_task_identity_insert")
        .ok_or_else(|| migration::schema_error("缺少任务身份 trigger"))?;
    c.execute_batch(sql).await?;
    begin_legacy_rows(c).await
}

/// 只有受控导入事务临时开放旧表，转换与保护恢复必须先于 commit。
pub(crate) async fn begin_legacy_rows(c: &Connection) -> Result<(), StoreError> {
    c.execute("DROP TRIGGER file_legacy_attachment_insert_disabled", ())
        .await?;
    Ok(())
}

pub(crate) async fn finish_legacy_rows(c: &Connection) -> Result<(), StoreError> {
    super::files::migration::migrate_legacy_attachments(c).await?;
    super::files::migration::restore_legacy_guard(c).await?;
    migration::validate(c).await?;
    super::files::migration::validate(c).await?;
    diagnostics::require_valid(c)
        .await
        .map_err(migration::schema_error)
}

/// 旧格式不携带自定义对象事实，不能用基础表计数掩盖目标中的新增数据。
pub(crate) async fn require_legacy_only(c: &Connection) -> Result<(), StoreError> {
    let extra=count(c,"SELECT (SELECT COUNT(*) FROM objects WHERE type_key NOT IN ('task','file') OR version!=1 OR body IS NOT NULL) + (SELECT COUNT(*) FROM object_property_slots) + (SELECT COUNT(*) FROM object_requests) + (SELECT COUNT(*) FROM object_snapshots) + (SELECT COUNT(*) FROM object_event_links) + (SELECT COUNT(*) FROM object_relation_edges WHERE relation_key!='file.attachment')",vec![]).await.map_err(migration::schema_error)?;
    if extra != 0 || custom_catalog_count(c).await? != 0 {
        return Err(migration::schema_error("目标包含旧格式未描述的对象事实"));
    }
    Ok(())
}

/// v5 以 tagged object 编码 BLOB，字符串 "hex:..." 始终保持文本。
/// 数据库 JSON 列仍然是文本；这里只允许数据库 scalar 和精确的 BLOB tag。
pub(crate) fn validate_wire_record(
    data: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), StoreError> {
    for value in data.values() {
        match value {
            serde_json::Value::Null | serde_json::Value::String(_) => {}
            serde_json::Value::Number(n) if n.is_i64() || n.is_f64() => {}
            serde_json::Value::Object(object) if object.len() == 1 => {
                let hex = object
                    .get("$kanban_blob_hex")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| migration::schema_error("portable v5 BLOB tag 不合法"))?;
                if hex.len() % 2 != 0 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(migration::schema_error(
                        "portable v5 BLOB 必须是偶数位十六进制",
                    ));
                }
            }
            _ => {
                return Err(migration::schema_error(
                    "portable v5 数据值必须是 SQL scalar 或 BLOB tag",
                ));
            }
        }
    }
    Ok(())
}

/// 供旧 baseline 的空目标检查识别新增 bootstrap 表，避免把类型种子计为用户事实。
pub(crate) fn is_catalog_table(table: &str) -> bool {
    CATALOG_TABLES.contains(&table)
}

/// 独立 doctor 返回数据问题；不借用 transport 类型，也不修改数据库。
pub(crate) async fn diagnostic_messages(
    c: &Connection,
) -> Result<Vec<super::ObjectDiagnostic>, StoreError> {
    let names: BTreeSet<String> = migration::names(c).await?;
    if !names.contains("object_model_schema") {
        return Ok(Vec::new());
    }
    let mut issues = Vec::new();
    if let Err(error) = catalog::validate_system(c).await {
        issues.push(super::ObjectDiagnostic {
            code: "object_catalog".into(),
            object_id: None,
            detail: error.to_string(),
        });
    }
    for row in rows(c, "SELECT id FROM boards ORDER BY id", vec![])
        .await
        .map_err(migration::schema_error)?
    {
        issues.extend(
            diagnostics::board(c, &text(&row[0]).map_err(migration::schema_error)?)
                .await
                .map_err(migration::schema_error)?,
        );
    }
    Ok(issues)
}

pub(crate) fn canonical_fields(
    data: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    data.into_iter()
        .collect::<std::collections::BTreeMap<_, _>>()
        .into_iter()
        .collect()
}
