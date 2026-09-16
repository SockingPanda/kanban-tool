use super::super::{ObjectError, ObjectResult, RelationLink, relations, store::*, workflow};
use super::{FileInfo, FileUploadSpec, StoredFile, path};
use crate::{AttachmentUpload, TursoStore};
use std::{fs::File, path::Path};
use turso::{Connection, transaction::TransactionBehavior};

const SELECT: &str = "SELECT f.object_id,f.board_id,f.original_filename,f.content_type,b.size_bytes,b.sha256,f.created_by,f.created_at,o.version,b.storage_key FROM file_objects f JOIN file_blobs b ON b.id=f.blob_id JOIN objects o ON o.id=f.object_id";

pub(crate) async fn owner(
    c: &Connection,
    board_id: &str,
    owner_id: &str,
    write: bool,
) -> ObjectResult<super::super::ObjectRecord> {
    board(c, board_id, write).await?;
    let object = get(c, board_id, owner_id).await?;
    if write {
        if object.archived_at.is_some() {
            return Err(ObjectError::precondition("file.owner_archived"));
        }
        workflow::ensure_editable(c, &object).await?;
    }
    Ok(object)
}

fn from_row(row: &[turso::Value], owner_id: &str) -> ObjectResult<StoredFile> {
    Ok(StoredFile {
        info: FileInfo {
            id: text(&row[0])?,
            board_id: text(&row[1])?,
            owner_id: owner_id.to_owned(),
            filename: text(&row[2])?,
            content_type: opt_text(&row[3])?,
            size_bytes: int(&row[4])?,
            sha256: opt_text(&row[5])?,
            created_by: text(&row[6])?,
            created_at: int(&row[7])?,
            object_version: int(&row[8])?,
        },
        storage_key: text(&row[9])?,
    })
}

pub(crate) async fn scope(c: &Connection, owner_id: &str) -> ObjectResult<String> {
    let result = rows(
        c,
        "SELECT board_id FROM objects WHERE id=?1",
        vec![s(owner_id)],
    )
    .await?;
    result
        .first()
        .map(|row| text(&row[0]))
        .transpose()?
        .ok_or_else(|| ObjectError::missing("file.owner_not_found"))
}

pub(crate) async fn lookup(
    c: &Connection,
    board_id: &str,
    owner_id: &str,
    id: &str,
) -> ObjectResult<StoredFile> {
    owner(c, board_id, owner_id, false).await?;
    let result = rows(c, &format!("{SELECT} WHERE f.board_id=?1 AND f.object_id=?2 AND EXISTS (SELECT 1 FROM object_relation_edges e WHERE e.board_id=f.board_id AND e.relation_key='file.attachment' AND e.source_id=f.object_id AND e.target_id=?3)"), vec![s(board_id), s(id), s(owner_id)]).await?;
    from_row(
        result
            .first()
            .ok_or_else(|| ObjectError::missing("file.reference_not_found"))?,
        owner_id,
    )
}

pub(crate) async fn list(
    c: &Connection,
    board_id: &str,
    owner_id: &str,
) -> ObjectResult<Vec<StoredFile>> {
    owner(c, board_id, owner_id, false).await?;
    rows(c, &format!("{SELECT} WHERE f.board_id=?1 AND EXISTS (SELECT 1 FROM object_relation_edges e WHERE e.board_id=f.board_id AND e.relation_key='file.attachment' AND e.source_id=f.object_id AND e.target_id=?2) ORDER BY f.created_at,f.object_id"), vec![s(board_id), s(owner_id)]).await?
        .iter().map(|row| from_row(row, owner_id)).collect()
}

pub(crate) fn validate_spec(spec: &FileUploadSpec) -> ObjectResult<()> {
    path::id(&spec.file_id)?;
    path::filename(&spec.filename)?;
    path::sha256(&spec.sha256)?;
    super::super::validation::label(&spec.board_id, "board_id", 128)?;
    super::super::validation::label(&spec.owner_id, "owner_id", 128)?;
    if spec.actor.trim().is_empty()
        || spec.actor.len() > 256
        || spec.actor.chars().any(char::is_control)
    {
        return Err(ObjectError::invalid("file.actor_invalid"));
    }
    if spec.size_bytes > crate::MAX_ATTACHMENT_BYTES {
        return Err(ObjectError::invalid("file.too_large"));
    }
    if let Some(mime) = &spec.content_type {
        let token = |s: &str| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
        };
        if mime.len() > 127
            || !mime
                .split_once('/')
                .is_some_and(|(a, b)| token(a) && token(b))
        {
            return Err(ObjectError::invalid("file.mime_invalid"));
        }
    }
    Ok(())
}

/// 重放不会重新创建用户已经解除的引用。
pub(crate) async fn replay(
    c: &Connection,
    spec: &FileUploadSpec,
) -> ObjectResult<Option<StoredFile>> {
    let result = rows(
        c,
        &format!("{SELECT} WHERE f.object_id=?1"),
        vec![s(&spec.file_id)],
    )
    .await?;
    let Some(row) = result.first() else {
        return Ok(None);
    };
    let old = from_row(row, &spec.owner_id)?;
    if old.info.board_id != spec.board_id
        || old.info.filename != spec.filename
        || old.info.content_type != spec.content_type
        || old.info.size_bytes != spec.size_bytes as i64
        || old.info.sha256.as_deref() != Some(spec.sha256.as_str())
        || old.info.created_by != spec.actor
    {
        return Err(ObjectError::conflict("file.idempotency_conflict"));
    }
    if !exists(c, "SELECT 1 FROM object_relation_edges WHERE board_id=?1 AND relation_key='file.attachment' AND source_id=?2 AND target_id=?3", vec![s(&spec.board_id), s(&spec.file_id), s(&spec.owner_id)]).await? {
        return Err(ObjectError::precondition("file.reference_was_removed"));
    }
    Ok(Some(old))
}

pub(crate) async fn open(root: &Path, record: &StoredFile) -> ObjectResult<File> {
    let root = root.to_owned();
    let key = record.storage_key.clone();
    let size = record.info.size_bytes;
    let sha = record.info.sha256.clone();
    tokio::task::spawn_blocking(move || {
        let path = path::guarded(&root, &key, false)?;
        crate::attachment_stream::open_verified(&path, size, sha.as_deref())
            .map_err(ObjectError::storage)
    })
    .await
    .map_err(ObjectError::storage)?
}

/// 调用方持有共享 mutation gate，blob 发布先于 metadata 事务提交。
/// 失败时保留已发布的内容，避免删除可能被共享的 blob。
pub(crate) async fn commit(
    store: &TursoStore,
    root: &Path,
    spec: &FileUploadSpec,
    upload: AttachmentUpload,
    event_id: &str,
    now: i64,
) -> ObjectResult<StoredFile> {
    validate_spec(spec)?;
    if upload.size_bytes() != spec.size_bytes || upload.sha256() != spec.sha256 {
        return Err(ObjectError::invalid("file.content_mismatch"));
    }
    let mut c = store.connection().await.map_err(ObjectError::storage)?;
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await?;
    let result = async {
        if let Some(record) = replay(&tx, spec).await? {
            let _verified = open(root, &record).await?;
            return Ok(record);
        }
        let before = owner(&tx, &spec.board_id, &spec.owner_id, true).await?;
        let key = format!("blobs/{}", spec.sha256);
        let owned_root = root.to_owned(); let owned_key = key.clone();
        tokio::task::spawn_blocking(move || {
            let target = path::guarded(&owned_root, &owned_key, true)?;
            upload.publish(&target).map_err(ObjectError::storage)?;
            Ok::<_, ObjectError>(())
        }).await.map_err(ObjectError::storage)??;
        let blob_id = format!("sha256_{}", spec.sha256);
        exec(&tx, "INSERT INTO file_blobs(id,storage_key,size_bytes,sha256,verification,created_at) VALUES (?1,?2,?3,?4,'verified',?5) ON CONFLICT(id) DO NOTHING", vec![s(&blob_id),s(&key),n(spec.size_bytes as i64),s(&spec.sha256),n(now)]).await?;
        let blob = rows(&tx, "SELECT storage_key,size_bytes,sha256 FROM file_blobs WHERE id=?1", vec![s(&blob_id)]).await?;
        let blob = blob.first().ok_or_else(|| ObjectError::storage("file.blob_missing_after_insert"))?;
        if text(&blob[0])? != key || int(&blob[1])? != spec.size_bytes as i64 || opt_text(&blob[2])?.as_deref() != Some(spec.sha256.as_str()) {
            return Err(ObjectError::precondition("file.blob_identity_mismatch"));
        }
        let title = spec.filename.chars().take(200).collect::<String>();
        exec(&tx, "INSERT INTO objects(id,board_id,type_key,task_id,title,body,version,created_at,updated_at,archived_at) VALUES (?1,?2,'file',NULL,?3,NULL,1,?4,?4,NULL)", vec![s(&spec.file_id),s(&spec.board_id),s(&title),n(now)]).await?;
        exec(&tx, "INSERT INTO file_objects(object_id,board_id,type_key,blob_id,original_filename,content_type,created_by,created_at) VALUES (?1,?2,'file',?3,?4,?5,?6,?7)", vec![s(&spec.file_id),s(&spec.board_id),s(&blob_id),s(&spec.filename),os(spec.content_type.as_deref()),s(&spec.actor),n(now)]).await?;
        relations::write_links(&tx, &spec.board_id, &[], &[RelationLink { relation_key: "file.attachment".into(), source_id: spec.file_id.clone(), target_id: spec.owner_id.clone() }], now, false).await?;
        bump(&tx, &before, now).await?;
        let object = get(&tx, &spec.board_id, &spec.file_id).await?;
        snapshot(&tx, &spec.board_id, &spec.file_id, "revision", &serde_json::to_value(&object)?, now).await?;
        event(&tx, &spec.board_id, &spec.owner_id, &spec.file_id, &spec.actor, event_id, now, true).await?;
        lookup(&tx, &spec.board_id, &spec.owner_id, &spec.file_id).await
    }.await;
    match result {
        Ok(record) => {
            tx.commit().await.map_err(|error| ObjectError {
                code: super::super::ObjectErrorCode::CommitUnknown,
                message: format!("file.commit_unknown; retry original file_id and hash: {error}"),
            })?;
            Ok(record)
        }
        Err(error) => {
            tx.rollback().await.map_err(ObjectError::storage)?;
            Err(error)
        }
    }
}

/// 旧任务附件删除仅解除对应关系，文件对象和 blob 继续保留。
pub(crate) async fn unlink_legacy(
    store: &TursoStore,
    board_id: &str,
    owner_id: &str,
    file_id: &str,
    actor: &str,
    event_id: &str,
    now: i64,
) -> ObjectResult<bool> {
    let mut c = store.connection().await.map_err(ObjectError::storage)?;
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .await?;
    let result = async {
        let before = owner(&tx, board_id, owner_id, true).await?;
        if !exists(&tx, "SELECT 1 FROM object_relation_edges WHERE board_id=?1 AND relation_key='file.attachment' AND source_id=?2 AND target_id=?3", vec![s(board_id),s(file_id),s(owner_id)]).await? { return Ok(false); }
        let file = get(&tx, board_id, file_id).await?;
        relations::write_links(&tx, board_id, &[RelationLink { relation_key: "file.attachment".into(), source_id: file_id.to_owned(), target_id: owner_id.to_owned() }], &[], now, false).await?;
        bump(&tx, &before, now).await?; bump(&tx, &file, now).await?;
        event(&tx, board_id, owner_id, file_id, actor, event_id, now, false).await?;
        Ok(true)
    }.await;
    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "在同一事务中记录文件、所属对象和显式事件身份"
)]
async fn event(
    c: &Connection,
    board_id: &str,
    owner_id: &str,
    file_id: &str,
    actor: &str,
    event_id: &str,
    now: i64,
    created: bool,
) -> ObjectResult<()> {
    let owner = get(c, board_id, owner_id).await?;
    let kind = if owner.type_key == "task" {
        if created {
            "task.attachment.created"
        } else {
            "task.attachment.deleted"
        }
    } else if created {
        "object.file.created"
    } else {
        "object.file.unlinked"
    };
    let metadata = rows(
        c,
        &format!("{SELECT} WHERE f.object_id=?1"),
        vec![s(file_id)],
    )
    .await?;
    let metadata = from_row(
        metadata
            .first()
            .ok_or_else(|| ObjectError::missing("file.not_found"))?,
        owner_id,
    )?;
    let payload = serde_json::json!({"format":"kanban.file-event.v1","file_id":file_id,"attachment_id":file_id,"owner_id":owner_id,"filename":metadata.info.filename,"size_bytes":metadata.info.size_bytes,"sha256":metadata.info.sha256,"invalidate_board":true});
    exec(c, "INSERT INTO task_events(event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at) VALUES (?1,?2,?3,NULL,?4,?5,?6,?7)", vec![s(event_id),s(board_id),os((owner.type_key=="task").then_some(owner_id)),s(kind),s(actor),s(&serde_json::to_string(&payload)?),n(now)]).await?;
    let sequence = count(
        c,
        "SELECT id FROM task_events WHERE event_id=?1",
        vec![s(event_id)],
    )
    .await?;
    for id in [owner_id, file_id] {
        exec(
            c,
            "INSERT INTO object_event_links(board_id,object_id,event_sequence) VALUES (?1,?2,?3)",
            vec![s(board_id), s(id), n(sequence)],
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn page(
    c: &Connection,
    board_id: &str,
    owner_id: &str,
    limit: u32,
    after: &str,
) -> ObjectResult<super::model::FilePage> {
    if !(1..=200).contains(&limit) || after.len() > 128 {
        return Err(ObjectError::invalid("file.page_invalid"));
    }
    owner(c, board_id, owner_id, false).await?;
    let data = rows(c, &format!("{SELECT} WHERE f.board_id=?1 AND f.object_id>?3 AND EXISTS (SELECT 1 FROM object_relation_edges e WHERE e.board_id=f.board_id AND e.relation_key='file.attachment' AND e.source_id=f.object_id AND e.target_id=?2) ORDER BY f.object_id LIMIT ?4"), vec![s(board_id), s(owner_id), s(after), n(i64::from(limit)+1)]).await?;
    let mut items = data
        .iter()
        .map(|row| from_row(row, owner_id).map(|file| file.info))
        .collect::<ObjectResult<Vec<_>>>()?;
    let more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next_id = if more {
        items.last().map(|item| item.id.clone())
    } else {
        None
    };
    Ok(super::model::FilePage { items, next_id })
}
