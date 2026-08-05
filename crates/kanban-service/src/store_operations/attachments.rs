use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};
use turso::transaction::TransactionBehavior;

use crate::{
    db::TursoStore, domain::AttachmentRecord, error::StoreError,
    store_operations::shared::validate_task_id, shared::*,
};

/// 创建附件时由 service 传入的元数据和内容。内容只经过 host 文件系统，绝不写入 Turso。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAttachmentInput {
    pub id: String,
    pub filename: String,
    pub rel_path: Option<String>,
    pub content_type: Option<String>,
    pub content: Vec<u8>,
    pub sha256: Option<String>,
    pub created_by: String,
    pub created_at: i64,
    pub event_id: String,
}

const ATTACHMENT_SELECT: &str = "SELECT id, board_id, task_id, filename, rel_path, content_type, size_bytes, sha256, created_by, created_at FROM task_attachments";

impl TursoStore {
    /// 把内容先原子发布到 host-owned root，再在同一条 application path 写入 metadata。
    pub async fn create_attachment(
        &self,
        task_id: &str,
        input: CreateAttachmentInput,
        root: &Path,
    ) -> Result<AttachmentRecord, StoreError> {
        let task_id = task_id.trim();
        validate_task_id(task_id)?;
        let id = validate_attachment_id(input.id.trim())?;
        let id = id.as_str();
        let filename = validate_filename(&input.filename)?;
        let created_by = input.created_by.trim();
        if created_by.is_empty() {
            return Err(StoreError::InvalidInput(
                "attachment created_by is required".to_owned(),
            ));
        }
        let event_id = input.event_id.trim();
        if !is_safe_id(event_id, "e_") {
            return Err(StoreError::InvalidInput(
                "attachment event_id must start with e_".to_owned(),
            ));
        }
        let observed_sha256 = sha256_hex(&input.content);
        let sha256 = input
            .sha256
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        if sha256
            .as_deref()
            .is_some_and(|value| value != observed_sha256)
        {
            return Err(StoreError::InvalidInput(
                "attachment sha256 does not match content".to_owned(),
            ));
        }

        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let task = first_row(
            transaction
                .query(
                    "SELECT board_id, archived_at FROM tasks WHERE id = :task_id LIMIT 1",
                    [(":task_id", task_id)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
            other => StoreError::Turso(other),
        })?;
        let board_id = text_value(task.get_value(0)?, "tasks.board_id")?;
        if optional_integer_value(task.get_value(1)?, "tasks.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task cannot receive attachments".to_owned(),
            ));
        }
        let board = first_row(
            transaction
                .query(
                    "SELECT archived_at FROM boards WHERE id = :board_id LIMIT 1",
                    [(":board_id", board_id.as_str())],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(board_id.clone()),
            other => StoreError::Turso(other),
        })?;
        if optional_integer_value(board.get_value(0)?, "boards.archived_at")?.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived board cannot receive attachments".to_owned(),
            ));
        }

        let existing = transaction
            .query(
                &format!("{ATTACHMENT_SELECT} WHERE id = :id LIMIT 1"),
                [(":id", id)],
            )
            .await?;
        let mut existing_rows = existing;
        if let Some(row) = existing_rows.next().await? {
            let existing = attachment_from_row(row)?;
            if existing.task_id == task_id
                && existing.filename == filename
                && existing.size_bytes == input.content.len() as i64
                && existing.sha256.as_deref() == Some(observed_sha256.as_str())
            {
                transaction.commit().await?;
                return Ok(existing);
            }
            return Err(StoreError::AttachmentConflict(format!(
                "attachment id {} already exists with different metadata",
                id
            )));
        }

        let rel_path = input
            .rel_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{board_id}/{task_id}/{id}-{filename}"));
        validate_rel_path(&rel_path)?;
        let task_root = format!("{board_id}/{task_id}/");
        if !rel_path.starts_with(&task_root) {
            return Err(StoreError::InvalidInput(
                "attachment rel_path must stay inside its board/task directory".to_owned(),
            ));
        }
        let final_path = guarded_path(root, &rel_path)?;
        publish_file(&final_path, &input.content, id)?;

        let write_result = async {
            transaction
                .execute(
                    "INSERT INTO task_attachments(id, board_id, task_id, filename, rel_path, content_type, size_bytes, sha256, created_by, created_at) VALUES (:id, :board_id, :task_id, :filename, :rel_path, :content_type, :size_bytes, :sha256, :created_by, :created_at)",
                    (
                        (":id", id),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":filename", filename.as_str()),
                        (":rel_path", rel_path.as_str()),
                        (":content_type", input.content_type.as_deref()),
                        (":size_bytes", input.content.len() as i64),
                        (":sha256", Some(observed_sha256.as_str())),
                        (":created_by", created_by),
                        (":created_at", input.created_at),
                    ),
                )
                .await?;
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.attachment.created', :actor, json_object('attachment_id', :attachment_id, 'filename', :filename, 'size_bytes', :size_bytes, 'sha256', :sha256), :created_at)",
                    (
                        (":event_id", event_id),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":actor", created_by),
                        (":attachment_id", id),
                        (":filename", filename.as_str()),
                        (":size_bytes", input.content.len() as i64),
                        (":sha256", observed_sha256.as_str()),
                        (":created_at", input.created_at),
                    ),
                )
                .await?;
            let row = first_row(
                transaction
                    .query(
                        &format!("{ATTACHMENT_SELECT} WHERE board_id = :board_id AND id = :id LIMIT 1"),
                        [(":board_id", board_id.as_str()), (":id", id)],
                    )
                    .await?,
            )
            .await?;
            attachment_from_row(row)
        }
        .await;
        match write_result {
            Ok(record) => {
                if let Err(error) = transaction.commit().await {
                    let _ = fs::remove_file(&final_path);
                    return Err(StoreError::Turso(error));
                }
                Ok(record)
            }
            Err(error) => {
                let _ = fs::remove_file(&final_path);
                Err(error)
            }
        }
    }

    pub async fn list_attachments(
        &self,
        task_id: &str,
    ) -> Result<Vec<AttachmentRecord>, StoreError> {
        let task_id = task_id.trim();
        validate_task_id(task_id)?;
        let connection = self.connection().await?;
        let task = first_row(
            connection
                .query(
                    "SELECT id FROM tasks WHERE id = :task_id LIMIT 1",
                    [(":task_id", task_id)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
            other => StoreError::Turso(other),
        })?;
        let _ = task;
        let mut rows = connection
            .query(
                &format!(
                    "{ATTACHMENT_SELECT} WHERE task_id = :task_id ORDER BY created_at ASC, id ASC"
                ),
                [(":task_id", task_id)],
            )
            .await?;
        let mut records = Vec::new();
        while let Some(row) = rows.next().await? {
            records.push(attachment_from_row(row)?);
        }
        Ok(records)
    }

    pub async fn read_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
        root: &Path,
    ) -> Result<(AttachmentRecord, Vec<u8>), StoreError> {
        let record = self.find_attachment(task_id, attachment_id).await?;
        ensure_attachment_scope(&record)?;
        let path = guarded_path(root, &record.rel_path)?;
        let mut file = File::open(&path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => {
                StoreError::AttachmentFileMissing(record.rel_path.clone())
            }
            _ => StoreError::AttachmentIo(error.to_string()),
        })?;
        let capacity = usize::try_from(record.size_bytes).map_err(|_| {
            StoreError::AttachmentIntegrity(format!(
                "attachment {} has an invalid stored size",
                record.id
            ))
        })?;
        let mut bytes = Vec::with_capacity(capacity);
        file.read_to_end(&mut bytes)
            .map_err(|error| StoreError::AttachmentIo(error.to_string()))?;
        if bytes.len() as i64 != record.size_bytes
            || record
                .sha256
                .as_deref()
                .is_some_and(|sha| sha != sha256_hex(&bytes))
        {
            return Err(StoreError::AttachmentIntegrity(format!(
                "attachment {} failed size/checksum verification",
                record.id
            )));
        }
        Ok((record, bytes))
    }

    pub async fn delete_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
        root: &Path,
        actor: &str,
        event_id: &str,
        deleted_at: i64,
    ) -> Result<bool, StoreError> {
        let actor = actor.trim();
        if actor.is_empty() {
            return Err(StoreError::InvalidInput(
                "attachment actor is required".to_owned(),
            ));
        }
        let event_id = event_id.trim();
        if !is_safe_id(event_id, "e_") {
            return Err(StoreError::InvalidInput(
                "attachment event_id must start with e_".to_owned(),
            ));
        }
        let record = self.find_attachment(task_id, attachment_id).await?;
        ensure_attachment_scope(&record)?;
        self.ensure_task_active(task_id).await?;
        let path = guarded_path(root, &record.rel_path)?;
        let canonical_root = canonical_attachment_root(root)?;
        let trash_root = canonical_root.join(".trash");
        fs::create_dir_all(&trash_root)
            .map_err(|error| StoreError::AttachmentIo(error.to_string()))?;
        let trash_path = trash_root.join(format!("{}-{}-{}", deleted_at, record.id, event_id));
        fs::rename(&path, &trash_path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => {
                StoreError::AttachmentFileMissing(record.rel_path.clone())
            }
            _ => StoreError::AttachmentIo(error.to_string()),
        })?;
        if let Some(parent) = path.parent() {
            if let Err(error) = sync_directory(parent) {
                let _ = restore_deleted_file(&trash_path, &path);
                return Err(error);
            }
        }
        if let Err(error) = sync_directory(&trash_root) {
            let _ = restore_deleted_file(&trash_path, &path);
            return Err(error);
        }

        let result: Result<(), StoreError> = async {
            let mut connection = self.connection().await?;
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .await?;
            transaction
                .execute(
                    "DELETE FROM task_attachments WHERE board_id = :board_id AND task_id = :task_id AND id = :id",
                    (
                        (":board_id", record.board_id.as_str()),
                        (":task_id", task_id),
                        (":id", attachment_id),
                    ),
                )
                .await?;
            transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.attachment.deleted', :actor, json_object('attachment_id', :attachment_id, 'filename', :filename), :created_at)",
                    (
                        (":event_id", event_id),
                        (":board_id", record.board_id.as_str()),
                        (":task_id", task_id),
                        (":actor", actor.trim()),
                        (":attachment_id", attachment_id),
                        (":filename", record.filename.as_str()),
                        (":created_at", deleted_at),
                    ),
                )
                .await?;
            transaction.commit().await?;
            Ok(())
        }
        .await;
        match result {
            Ok(()) => Ok(true),
            Err(error) => {
                let _ = restore_deleted_file(&trash_path, &path);
                Err(error)
            }
        }
    }

    async fn find_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<AttachmentRecord, StoreError> {
        let task_id = task_id.trim();
        validate_task_id(task_id)?;
        let attachment_id = validate_attachment_id(attachment_id)?;
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    &format!("{ATTACHMENT_SELECT} WHERE task_id = :task_id AND id = :id LIMIT 1"),
                    [(":task_id", task_id), (":id", attachment_id.as_str())],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::AttachmentNotFound(attachment_id),
            other => StoreError::Turso(other),
        })?;
        attachment_from_row(row)
    }

    async fn ensure_task_active(&self, task_id: &str) -> Result<(), StoreError> {
        let task_id = task_id.trim();
        let connection = self.connection().await?;
        let row = first_row(
            connection
                .query(
                    "SELECT t.archived_at, b.archived_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
                    [(":task_id", task_id)],
                )
                .await?,
        )
        .await
        .map_err(|error| match error {
            turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(task_id.to_owned()),
            other => StoreError::Turso(other),
        })?;
        if optional_integer_value(row.get_value(0)?, "tasks.archived_at")?.is_some()
            || optional_integer_value(row.get_value(1)?, "boards.archived_at")?.is_some()
        {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot mutate attachments".to_owned(),
            ));
        }
        Ok(())
    }
}

fn attachment_from_row(row: turso::Row) -> Result<AttachmentRecord, StoreError> {
    Ok(AttachmentRecord {
        id: text_value(row.get_value(0)?, "task_attachments.id")?,
        board_id: text_value(row.get_value(1)?, "task_attachments.board_id")?,
        task_id: text_value(row.get_value(2)?, "task_attachments.task_id")?,
        filename: text_value(row.get_value(3)?, "task_attachments.filename")?,
        rel_path: text_value(row.get_value(4)?, "task_attachments.rel_path")?,
        content_type: optional_text_value(row.get_value(5)?, "task_attachments.content_type")?,
        size_bytes: integer_value(row.get_value(6)?, "task_attachments.size_bytes")?,
        sha256: optional_text_value(row.get_value(7)?, "task_attachments.sha256")?,
        created_by: text_value(row.get_value(8)?, "task_attachments.created_by")?,
        created_at: integer_value(row.get_value(9)?, "task_attachments.created_at")?,
    })
}

fn validate_filename(filename: &str) -> Result<String, StoreError> {
    let filename = filename.trim();
    if filename.is_empty()
        || filename == "."
        || filename == ".."
        || filename.contains(['/', '\\', '\0'])
    {
        return Err(StoreError::InvalidInput(
            "attachment filename must be a single safe path component".to_owned(),
        ));
    }
    Ok(filename.to_owned())
}

fn validate_rel_path(rel_path: &str) -> Result<(), StoreError> {
    let path = Path::new(rel_path);
    if rel_path.trim().is_empty() || path.is_absolute() || rel_path.contains('\0') {
        return Err(StoreError::InvalidInput(
            "attachment rel_path must be relative".to_owned(),
        ));
    }
    for component in path.components() {
        match component {
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(StoreError::InvalidInput(
                    "attachment rel_path must not contain traversal or root components".to_owned(),
                ));
            }
            Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn ensure_attachment_scope(record: &AttachmentRecord) -> Result<(), StoreError> {
    let prefix = format!("{}/{}/", record.board_id, record.task_id);
    if !record.rel_path.starts_with(&prefix) {
        return Err(StoreError::AttachmentIntegrity(format!(
            "attachment {} path is outside its board/task scope",
            record.id
        )));
    }
    Ok(())
}

fn guarded_path(root: &Path, rel_path: &str) -> Result<PathBuf, StoreError> {
    validate_rel_path(rel_path)?;
    let root = canonical_attachment_root(root)?;
    let candidate = root.join(rel_path);
    let parent = candidate
        .parent()
        .ok_or_else(|| StoreError::InvalidInput("attachment path has no parent".to_owned()))?;
    fs::create_dir_all(parent).map_err(|error| StoreError::AttachmentIo(error.to_string()))?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|error| StoreError::AttachmentIo(error.to_string()))?;
    if !canonical_parent.starts_with(&root) {
        return Err(StoreError::InvalidInput(
            "attachment path escapes host attachment root".to_owned(),
        ));
    }
    match fs::symlink_metadata(&candidate) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(StoreError::InvalidInput(
                    "attachment destination must not be a symlink".to_owned(),
                ));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(StoreError::AttachmentIo(error.to_string())),
    }
    Ok(candidate)
}

fn canonical_attachment_root(root: &Path) -> Result<PathBuf, StoreError> {
    let metadata =
        fs::symlink_metadata(root).map_err(|error| StoreError::AttachmentIo(error.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(StoreError::InvalidInput(
            "attachment root must not be a symlink".to_owned(),
        ));
    }
    fs::canonicalize(root).map_err(|error| StoreError::AttachmentIo(error.to_string()))
}

fn validate_attachment_id(value: &str) -> Result<String, StoreError> {
    let value = value.trim();
    if !is_safe_id(value, "a_") {
        return Err(StoreError::InvalidInput(
            "attachment id must be a single safe a_... path component".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn is_safe_id(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && !value.contains(['/', '\\', '\0'])
        && value != "."
        && value != ".."
}

fn publish_file(final_path: &Path, content: &[u8], id: &str) -> Result<(), StoreError> {
    let parent = final_path
        .parent()
        .ok_or_else(|| StoreError::InvalidInput("attachment path has no parent".to_owned()))?;
    let temp = parent.join(format!(".{id}.staging"));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|error| StoreError::AttachmentIo(error.to_string()))?;
    if let Err(error) = file.write_all(content).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(StoreError::AttachmentIo(error.to_string()));
    }
    // Hard-linking the staged inode is atomic and refuses to replace an
    // existing destination, so a stale file can never be silently clobbered.
    if let Err(error) = fs::hard_link(&temp, final_path) {
        let _ = fs::remove_file(&temp);
        return Err(StoreError::AttachmentIo(error.to_string()));
    }
    if let Err(error) = fs::remove_file(&temp) {
        let _ = fs::remove_file(final_path);
        let _ = fs::remove_file(&temp);
        return Err(StoreError::AttachmentIo(error.to_string()));
    }
    let directory =
        File::open(parent).map_err(|error| StoreError::AttachmentIo(error.to_string()))?;
    if let Err(error) = directory.sync_all() {
        let _ = fs::remove_file(final_path);
        let _ = directory.sync_all();
        return Err(StoreError::AttachmentIo(error.to_string()));
    }
    Ok(())
}

fn restore_deleted_file(trash_path: &Path, original_path: &Path) -> Result<(), StoreError> {
    fs::rename(trash_path, original_path)
        .map_err(|error| StoreError::AttachmentIo(error.to_string()))?;
    if let Some(parent) = original_path.parent() {
        sync_directory(parent)?;
    }
    if let Some(parent) = trash_path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), StoreError> {
    File::open(path)
        .map_err(|error| StoreError::AttachmentIo(error.to_string()))?
        .sync_all()
        .map_err(|error| StoreError::AttachmentIo(error.to_string()))
}

fn sha256_hex(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
