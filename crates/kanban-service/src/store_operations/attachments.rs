//! Compatibility adapter for the existing task RPC/CLI/MCP contracts.
//! All reads and writes use file_objects + file_blobs + object_relation_edges.
use super::shared::validate_task_id;
use crate::object_model::{
    ObjectError, ObjectErrorCode,
    files::{self, FileUploadSpec, StoredFile},
};
use crate::{AttachmentUpload, StoreError, TursoStore, domain::AttachmentRecord};
use std::{fs::File, path::Path};

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

fn failure(error: ObjectError) -> StoreError {
    match error.code {
        ObjectErrorCode::InvalidArgument => StoreError::InvalidInput(error.message),
        ObjectErrorCode::NotFound => StoreError::AttachmentNotFound(error.message),
        ObjectErrorCode::Conflict => StoreError::AttachmentConflict(error.message),
        ObjectErrorCode::FailedPrecondition => StoreError::InvalidTransition(error.message),
        ObjectErrorCode::Storage | ObjectErrorCode::CommitUnknown => {
            StoreError::AttachmentIo(error.message)
        }
    }
}
fn legacy(file: StoredFile) -> AttachmentRecord {
    AttachmentRecord {
        id: file.info.id,
        board_id: file.info.board_id,
        task_id: file.info.owner_id,
        filename: file.info.filename,
        rel_path: file.storage_key,
        content_type: file.info.content_type,
        size_bytes: file.info.size_bytes,
        sha256: file.info.sha256,
        created_by: file.info.created_by,
        created_at: file.info.created_at,
    }
}

impl TursoStore {
    pub(crate) async fn validate_attachment_target(&self, task_id: &str) -> Result<(), StoreError> {
        validate_task_id(task_id)?;
        let c = self.connection().await?;
        let board = files::store::scope(&c, task_id).await.map_err(failure)?;
        // Keep this check in the task adapter; generic file owners use the object service.
        let mut rows = c
            .query("SELECT id FROM tasks WHERE id=?1", [task_id])
            .await?;
        if rows.next().await?.is_none() {
            return Err(StoreError::TaskNotFound(task_id.to_owned()));
        }
        files::store::owner(&c, &board, task_id, true)
            .await
            .map_err(failure)?;
        Ok(())
    }
    pub(crate) async fn create_attachment_with_upload(
        &self,
        task_id: &str,
        input: CreateAttachmentInput,
        root: &Path,
        upload: AttachmentUpload,
    ) -> Result<AttachmentRecord, StoreError> {
        validate_task_id(task_id)?;
        if !input.content.is_empty() {
            return Err(StoreError::InvalidInput(
                "file.unexpected_inline_content".into(),
            ));
        }
        let sha = upload.sha256();
        if input.sha256.as_deref().is_some_and(|value| value != sha) {
            return Err(StoreError::InvalidInput("file.content_mismatch".into()));
        }
        if input
            .rel_path
            .as_deref()
            .filter(|path| !path.is_empty())
            .is_some_and(|path| path != format!("blobs/{sha}"))
        {
            return Err(StoreError::InvalidInput(
                "file.storage_path_is_server_owned".into(),
            ));
        }
        let c = self.connection().await?;
        let board_id = files::store::scope(&c, task_id).await.map_err(failure)?;
        let spec = FileUploadSpec {
            board_id,
            owner_id: task_id.to_owned(),
            file_id: input.id,
            filename: input.filename,
            content_type: input.content_type,
            size_bytes: upload.size_bytes(),
            sha256: sha,
            actor: input.created_by,
        };
        files::store::commit(self, root, &spec, upload, &input.event_id, input.created_at)
            .await
            .map(legacy)
            .map_err(failure)
    }
    pub async fn list_attachments(
        &self,
        task_id: &str,
    ) -> Result<Vec<AttachmentRecord>, StoreError> {
        validate_task_id(task_id)?;
        let mut c = self.connection().await?;
        let tx = c.transaction().await?;
        let board = files::store::scope(&tx, task_id).await.map_err(failure)?;
        let result = files::store::list(&tx, &board, task_id)
            .await
            .map_err(failure)?
            .into_iter()
            .map(legacy)
            .collect();
        tx.commit().await?;
        Ok(result)
    }
    pub(crate) async fn open_attachment_stream(
        &self,
        task_id: &str,
        id: &str,
        root: &Path,
    ) -> Result<(AttachmentRecord, File), StoreError> {
        validate_task_id(task_id)?;
        let mut c = self.connection().await?;
        let tx = c.transaction().await?;
        let board = files::store::scope(&tx, task_id).await.map_err(failure)?;
        let record = files::store::lookup(&tx, &board, task_id, id)
            .await
            .map_err(failure)?;
        tx.commit().await?;
        let file = files::store::open(root, &record).await.map_err(failure)?;
        Ok((legacy(record), file))
    }
    pub async fn delete_attachment(
        &self,
        task_id: &str,
        id: &str,
        _root: &Path,
        actor: &str,
        event_id: &str,
        now: i64,
    ) -> Result<bool, StoreError> {
        validate_task_id(task_id)?;
        if actor.trim().is_empty() || actor.len() > 256 || actor.chars().any(char::is_control) {
            return Err(StoreError::InvalidInput("file.actor_invalid".into()));
        }
        let c = self.connection().await?;
        let board = files::store::scope(&c, task_id).await.map_err(failure)?;
        files::store::unlink_legacy(self, &board, task_id, id, actor, event_id, now)
            .await
            .map_err(failure)
    }
}
