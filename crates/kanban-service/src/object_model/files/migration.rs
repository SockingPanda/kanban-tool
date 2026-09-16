//! Independently fingerprinted migration. No original v4 or object-model v2 DDL is rewritten.
use super::super::{catalog, migration as objects, relations, store::*};
use super::path;
use crate::StoreError;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
use turso::{Connection, transaction::TransactionBehavior};

pub(crate) const SQL: &str = include_str!("schema.sql");
pub(crate) const SEED: &str = include_str!("seed.json");
pub(crate) const TABLES: &[&str] = &["file_model_schema", "file_blobs", "file_objects"];

pub(crate) fn checksum() -> String {
    let mut digest = Sha256::new();
    digest.update(SQL);
    digest.update([0]);
    digest.update(SEED);
    format!("sha256:{:x}", digest.finalize())
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
fn error(value: impl std::fmt::Display) -> StoreError {
    StoreError::SchemaMismatch(format!("file-model v1: {value}"))
}

pub(crate) async fn exclude_validated_tables(
    c: &Connection,
    names: &mut BTreeSet<String>,
) -> Result<(), StoreError> {
    let found = TABLES.iter().filter(|name| names.contains(**name)).count();
    if found == 0 {
        return Ok(());
    }
    if found != TABLES.len() || !names.contains("object_model_schema") {
        return Err(error("partial file lineage"));
    }
    validate(c).await?;
    for name in TABLES {
        names.remove(*name);
    }
    Ok(())
}

pub(crate) async fn validate(c: &Connection) -> Result<(), StoreError> {
    let marker = rows(
        c,
        "SELECT singleton,version,checksum,imported_attachments FROM file_model_schema",
        vec![],
    )
    .await
    .map_err(error)?;
    if marker.len() != 1
        || int(&marker[0][0]).map_err(error)? != 1
        || int(&marker[0][1]).map_err(error)? != 1
        || text(&marker[0][2]).map_err(error)? != checksum()
        || int(&marker[0][3]).map_err(error)? < 0
    {
        return Err(error("unknown version or checksum"));
    }
    for (kind, name, ddl) in ddl_objects() {
        let actual = rows(
            c,
            "SELECT sql FROM sqlite_master WHERE type=?1 AND name=?2",
            vec![s(kind), s(name)],
        )
        .await
        .map_err(error)?;
        if actual.len() != 1
            || objects::compact_sql(&text(&actual[0][0]).map_err(error)?)
                != objects::compact_sql(ddl)
        {
            return Err(error(format!("DDL drift: {name}")));
        }
    }
    if count(c, "SELECT COUNT(*) FROM task_attachments", vec![])
        .await
        .map_err(error)?
        != 0
    {
        return Err(error("retired task_attachments table must be empty"));
    }
    if exists(c, "SELECT 1 FROM objects o LEFT JOIN file_objects f ON f.object_id=o.id WHERE o.type_key='file' AND f.object_id IS NULL", vec![]).await.map_err(error)? {
        return Err(error("file object has no content capability"));
    }
    if exists(c, "SELECT 1 FROM file_objects f JOIN objects o ON o.id=f.object_id WHERE o.type_key!='file' OR o.board_id!=f.board_id", vec![]).await.map_err(error)? {
        return Err(error("file scope/type mismatch"));
    }
    for row in rows(c, "SELECT storage_key FROM file_blobs", vec![])
        .await
        .map_err(error)?
    {
        path::relative(&text(&row[0]).map_err(error)?).map_err(error)?;
    }
    if !rows(c, "PRAGMA foreign_key_check", vec![])
        .await
        .map_err(error)?
        .is_empty()
    {
        return Err(error("foreign key violation"));
    }
    Ok(())
}

pub(crate) async fn apply(
    c: &mut Connection,
    path: &Path,
    hook: Option<&dyn crate::db::UpgradeBackupHook>,
    backup_required: bool,
) -> Result<(), StoreError> {
    let names = objects::names(c).await?;
    let found = TABLES.iter().filter(|name| names.contains(**name)).count();
    if found != 0 {
        if found != TABLES.len() {
            return Err(error("partial migration, restore the verified backup"));
        }
        return validate(c).await;
    }
    objects::validate(c).await?;
    if backup_required && path != Path::new(":memory:") {
        let backup = objects::verified_backup(c, path, &names).await?;
        if let Some(hook) = hook {
            hook.before_upgrade(&crate::db::UpgradeBackupRequest {
                source_path: path.to_owned(),
                backup_path: backup,
                family: "kanban.turso.files".into(),
                from_version: 0,
                to_version: 1,
                fingerprint: checksum(),
            })?;
        }
    }
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await?;
    let result = async {
        tx.execute_batch(SQL).await?;
        seed(&tx).await?;
        migrate_legacy_attachments(&tx).await?;
        catalog::changed(&tx).await.map_err(error)?;
        validate(&tx).await?;
        objects::validate(&tx).await?;
        Ok::<_, StoreError>(())
    }
    .await;
    match result {
        Ok(()) => {
            tx.commit().await?;
            Ok(())
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

/// 导入与首次迁移共享种子和附件转换；调用方持有同一 canonical 事务。
pub(crate) async fn seed(c: &Connection) -> Result<(), StoreError> {
    exec(c, "INSERT INTO file_model_schema(singleton,version,checksum,installed_at,imported_attachments) VALUES (1,1,?1,?2,?3)", vec![s(&checksum()),n(crate::shared::now_ms()),n(0)]).await.map_err(error)?;
    let seed: catalog::Seed = serde_json::from_str(SEED).map_err(error)?;
    for ty in seed.types {
        catalog::insert_type(c, &ty.definition, &ty.capability, true)
            .await
            .map_err(error)?;
    }
    for relation in seed.relations {
        relations::define(c, &relation, true).await.map_err(error)?;
    }
    Ok(())
}

pub(crate) async fn migrate_legacy_attachments(c: &Connection) -> Result<(), StoreError> {
    let legacy = rows(c, "SELECT id,board_id,task_id,filename,rel_path,content_type,size_bytes,sha256,created_by,created_at FROM task_attachments ORDER BY id", vec![]).await.map_err(error)?;
    for row in &legacy {
        let id = text(&row[0]).map_err(error)?;
        let board = text(&row[1]).map_err(error)?;
        let owner = text(&row[2]).map_err(error)?;
        let filename = text(&row[3]).map_err(error)?;
        let key = text(&row[4]).map_err(error)?;
        let mime = opt_text(&row[5]).map_err(error)?;
        let size = int(&row[6]).map_err(error)?;
        let sha = opt_text(&row[7])
            .map_err(error)?
            .map(|hash| hash.to_ascii_lowercase());
        let actor = text(&row[8]).map_err(error)?;
        let created = int(&row[9]).map_err(error)?;
        path::id(&id).map_err(error)?;
        path::filename(&filename).map_err(error)?;
        path::relative(&key).map_err(error)?;
        if !(0..=crate::MAX_ATTACHMENT_BYTES as i64).contains(&size) {
            return Err(error(format!("invalid size for {id}")));
        }
        if let Some(sha) = &sha {
            path::sha256(sha).map_err(error)?;
        }
        // A previously shared path can become one shared blob only when its metadata agrees.
        let old = rows(
            c,
            "SELECT id,size_bytes,sha256 FROM file_blobs WHERE storage_key=?1",
            vec![s(&key)],
        )
        .await
        .map_err(error)?;
        let blob_id = if let Some(old) = old.first() {
            if int(&old[1]).map_err(error)? != size || opt_text(&old[2]).map_err(error)? != sha {
                return Err(error(format!("conflicting legacy path for {id}")));
            }
            text(&old[0]).map_err(error)?
        } else {
            let blob = format!("legacy_{id}");
            exec(c, "INSERT INTO file_blobs(id,storage_key,size_bytes,sha256,verification,created_at) VALUES (?1,?2,?3,?4,'legacy',?5)", vec![s(&blob),s(&key),n(size),os(sha.as_deref()),n(created)]).await.map_err(error)?;
            blob
        };
        let title = filename.chars().take(200).collect::<String>();
        exec(c, "INSERT INTO objects(id,board_id,type_key,task_id,title,body,version,created_at,updated_at,archived_at) VALUES (?1,?2,'file',NULL,?3,NULL,1,?4,?4,NULL)", vec![s(&id),s(&board),s(&title),n(created)]).await.map_err(error)?;
        exec(c, "INSERT INTO file_objects(object_id,board_id,type_key,blob_id,original_filename,content_type,created_by,created_at) VALUES (?1,?2,'file',?3,?4,?5,?6,?7)", vec![s(&id),s(&board),s(&blob_id),s(&filename),os(mime.as_deref()),s(&actor),n(created)]).await.map_err(error)?;
        exec(c, "INSERT INTO object_relation_edges(board_id,relation_key,source_id,target_id,source_type,source_cardinality,target_cardinality,created_at) VALUES (?1,'file.attachment',?2,?3,'file','many','many',?4)", vec![s(&board),s(&id),s(&owner),n(created)]).await.map_err(error)?;
    }
    // The old table remains only to preserve the original baseline DDL identity.
    // Its rows have moved, not been duplicated. Future INSERTs are rejected by a trigger.
    exec(c, "DELETE FROM task_attachments", vec![])
        .await
        .map_err(error)?;

    exec(c, "UPDATE file_model_schema SET imported_attachments=imported_attachments+?1 WHERE singleton=1", vec![n(legacy.len() as i64)]).await.map_err(error)?;
    Ok(())
}

pub(crate) async fn restore_legacy_guard(c: &Connection) -> Result<(), StoreError> {
    let (_, _, sql) = ddl_objects()
        .into_iter()
        .find(|(_, name, _)| *name == "file_legacy_attachment_insert_disabled")
        .ok_or_else(|| error("缺少旧附件写入保护"))?;
    c.execute_batch(sql).await?;
    Ok(())
}
