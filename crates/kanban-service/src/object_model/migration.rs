//! 基线 v2 的 SQL/checksum 保持不变，通用对象层使用独立、严格检查的 schema lineage。
use super::{catalog, diagnostics, store::*};
use crate::StoreError;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use turso::{Connection, transaction::TransactionBehavior};

pub(crate) const SQL: &str = include_str!("schema.sql");
pub(crate) const TABLES: &[&str] = &[
    "object_model_schema",
    "object_types",
    "object_properties",
    "object_options",
    "object_type_properties",
    "objects",
    "object_property_slots",
    "object_property_values",
    "object_requests",
    "object_snapshots",
    "object_event_links",
    "object_relation_types",
    "object_relation_edges",
    "object_workflows",
    "object_rollups",
];
pub(crate) fn schema_error(error: impl std::fmt::Display) -> StoreError {
    StoreError::SchemaMismatch(format!("object-model v2: {error}"))
}
pub(crate) fn checksum() -> String {
    let mut hash = Sha256::new();
    hash.update(SQL.as_bytes());
    hash.update([0]);
    hash.update(catalog::SEED.as_bytes());
    format!("sha256:{:x}", hash.finalize())
}
pub(crate) fn ddl_objects() -> Vec<(&'static str, &'static str, &'static str)> {
    SQL.split("-- @object ")
        .skip(1)
        .filter_map(|part| {
            let (header, sql) = part.split_once('\n')?;
            let mut words = header.split_whitespace();
            Some((words.next()?, words.next()?, sql.trim()))
        })
        .collect()
}
/// Turso 会为 key 等标识符补双引号；统一标识符与空白，保留字符串字面量。
pub(crate) fn compact_sql(sql: &str) -> String {
    let mut out = String::new();
    let mut quote = None;
    let mut chars = sql.trim().trim_end_matches(';').chars().peekable();
    while let Some(ch) = chars.next() {
        if let Some(q) = quote {
            out.push(ch);
            if ch == q {
                if chars.peek() == Some(&q) {
                    out.push(chars.next().unwrap_or(q));
                } else {
                    quote = None;
                }
            }
        } else if ch == '"' {
            let mut identifier = String::new();
            for next in chars.by_ref() {
                if next == '"' {
                    break;
                }
                identifier.push(next);
            }
            if !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                out.push_str(&identifier.to_ascii_lowercase());
            } else {
                out.push('"');
                out.push_str(&identifier);
                out.push('"');
            }
        } else if ch == '\'' {
            quote = Some(ch);
            out.push(ch.to_ascii_lowercase());
        } else if !ch.is_whitespace() {
            out.push(ch);
        }
    }
    out
}
pub(crate) async fn names(c: &Connection) -> Result<BTreeSet<String>, StoreError> {
    rows(
        c,
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        vec![],
    )
    .await
    .map_err(schema_error)?
    .iter()
    .map(|r| text(&r[0]).map_err(schema_error))
    .collect()
}
/// 在原 baseline 的精确表集检查前调用；只接受已验证的当前对象 schema。
pub(crate) async fn exclude_validated_tables(
    c: &Connection,
    names: &mut BTreeSet<String>,
) -> Result<(), StoreError> {
    let found = TABLES.iter().filter(|name| names.contains(**name)).count();
    if found > 0 {
        if !names.contains("object_model_schema") {
            return Err(schema_error("对象层缺少版本标记"));
        }
        if found != TABLES.len() {
            return Err(schema_error("对象层表集不完整"));
        }
        validate(c).await?;
        for name in TABLES {
            names.remove(*name);
        }
    }
    Ok(())
}
pub(crate) async fn validate(c: &Connection) -> Result<(), StoreError> {
    let r = rows(
        c,
        "SELECT singleton,version,checksum,catalog_version FROM object_model_schema",
        vec![],
    )
    .await
    .map_err(schema_error)?;
    if r.len() != 1
        || int(&r[0][0]).map_err(schema_error)? != 1
        || int(&r[0][1]).map_err(schema_error)? != 2
        || text(&r[0][2]).map_err(schema_error)? != checksum()
        || int(&r[0][3]).map_err(schema_error)? < 1
    {
        return Err(schema_error("未知的对象层版本或 checksum，自动降级被禁止"));
    }
    for (kind, name, sql) in ddl_objects() {
        let r = rows(
            c,
            "SELECT sql FROM sqlite_master WHERE type=?1 AND name=?2",
            vec![s(kind), s(name)],
        )
        .await
        .map_err(schema_error)?;
        if r.len() != 1 || compact_sql(&text(&r[0][0]).map_err(schema_error)?) != compact_sql(sql) {
            return Err(schema_error(format!("DDL 不匹配: {name}")));
        }
    }
    catalog::validate_system(c).await.map_err(schema_error)?;
    if !rows(c, "PRAGMA foreign_key_check", vec![])
        .await
        .map_err(schema_error)?
        .is_empty()
    {
        return Err(schema_error("外键检查失败"));
    }
    Ok(())
}
pub(crate) async fn apply(
    c: &mut Connection,
    path: &Path,
    hook: Option<&dyn crate::db::UpgradeBackupHook>,
    backup_required: bool,
) -> Result<(), StoreError> {
    let tables = names(c).await?;
    if tables.contains("object_model_schema") {
        return validate(c).await;
    }
    if backup_required && path != Path::new(":memory:") {
        let backup = verified_backup(c, path, &tables).await?;
        if let Some(hook) = hook {
            hook.before_upgrade(&crate::db::UpgradeBackupRequest {
                source_path: path.to_owned(),
                backup_path: backup,
                family: "kanban.turso.object-model".into(),
                from_version: 0,
                to_version: 2,
                fingerprint: checksum(),
            })?;
        }
    }
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await?;
    let result=async {
        {
            tx.execute_batch(SQL).await?;
            catalog::seed(&tx).await.map_err(schema_error)?;
            tx.execute("INSERT INTO object_model_schema(singleton,version,checksum,catalog_version,installed_at) VALUES (1,2,?1,1,?2)",(checksum(),crate::shared::now_ms())).await?;
            tx.execute("INSERT INTO objects(id,board_id,type_key,task_id,title,body,version,created_at,updated_at,archived_at) SELECT id,board_id,'task',id,NULL,NULL,1,created_at,created_at,NULL FROM tasks",()).await?;
        }
        diagnostics::require_valid(&tx).await.map_err(schema_error)?;
        validate(&tx).await?;
        Ok::<(),StoreError>(())
    }.await;
    match result {
        Ok(()) => {
            tx.commit().await?;
            Ok(())
        }
        Err(error) => {
            if let Err(rollback) = tx.rollback().await {
                return Err(schema_error(format!("{error}; rollback: {rollback}")));
            }
            Err(error)
        }
    }
}
pub(super) async fn verified_backup(
    c: &Connection,
    path: &Path,
    tables: &BTreeSet<String>,
) -> Result<PathBuf, StoreError> {
    let file = path
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or_else(|| StoreError::BackupRequired("数据库文件名不可用".into()))?;
    let parent = path
        .parent()
        .ok_or_else(|| StoreError::BackupRequired("数据库没有父目录".into()))?;
    let backup = parent.join(format!(
        "{file}.pre-object-model-{}.turso-backup",
        ulid::Ulid::new()
    ));
    let literal = backup
        .to_str()
        .ok_or_else(|| StoreError::BackupRequired("备份路径不是 UTF-8".into()))?
        .replace('\'', "''");
    c.execute(format!("VACUUM INTO '{literal}'"), ())
        .await
        .map_err(|e| StoreError::BackupRequired(e.to_string()))?;
    let db = turso::Builder::new_local(backup.to_str().ok_or_else(|| schema_error("备份路径"))?)
        .experimental_index_method(true)
        .experimental_vacuum(true)
        .build()
        .await?;
    let conn = db.connect()?;
    let check = rows(&conn, "PRAGMA integrity_check", vec![])
        .await
        .map_err(schema_error)?;
    if check.len() != 1 || text(&check[0][0]).map_err(schema_error)? != "ok" {
        return Err(StoreError::BackupRequired("备份完整性检查未通过".into()));
    }
    for table in tables {
        let sql = format!("SELECT COUNT(*) FROM \"{}\"", table.replace('"', "\"\""));
        if count(c, &sql, vec![]).await.map_err(schema_error)?
            != count(&conn, &sql, vec![]).await.map_err(schema_error)?
        {
            return Err(StoreError::BackupRequired(format!(
                "备份表 {table} 行数不匹配"
            )));
        }
    }
    Ok(backup)
}
