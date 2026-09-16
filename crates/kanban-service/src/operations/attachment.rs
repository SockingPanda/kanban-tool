use crate::{AttachmentContentRecord, AttachmentRecord, AttachmentUpload, KanbanService};
use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAttachmentCommand {
    pub task_id: String,
    pub id: Option<String>,
    pub filename: String,
    pub rel_path: Option<String>,
    pub content_type: Option<String>,
    pub content: Vec<u8>,
    pub sha256: Option<String>,
    pub created_by: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAttachmentRecord {
    pub id: String,
    pub filename: String,
    pub rel_path: Option<String>,
    pub content_type: Option<String>,
    pub content: Vec<u8>,
    pub sha256: Option<String>,
    pub created_by: String,
    pub event_id: String,
    pub created_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteAttachmentCommand {
    pub task_id: String,
    pub attachment_id: String,
    pub actor: String,
}
/// 仅向 host 返回已校验的打开句柄，不暴露任意路径读取能力。
#[derive(Debug)]
pub struct AttachmentStreamRecord {
    pub attachment: AttachmentRecord,
    pub file: std::fs::File,
}
impl<C: Clock> KanbanService<C> {
    pub async fn begin_attachment_upload(&self, task_id: &str) -> Result<AttachmentUpload> {
        let task_id = canonical_task_id(task_id)?;
        self.store
            .validate_attachment_target(&task_id)
            .await
            .map_err(crate::error::store_error)?;
        let root = self
            .attachment_root
            .as_deref()
            .ok_or_else(|| {
                KanbanError::Storage("attachment root is not configured for this host".into())
            })?
            .to_path_buf();
        tokio::task::spawn_blocking(move || {
            crate::attachment_stream::AttachmentUpload::create(&root)
        })
        .await
        .map_err(|error| KanbanError::Storage(format!("附件暂存 worker 失败：{error}")))?
        .map_err(crate::error::store_error)
    }

    /// 保留 CLI/MCP 的 JSON bytes 兼容入口；新 Web 上传使用 begin + commit。
    pub async fn create_attachment(
        &self,
        mut command: CreateAttachmentCommand,
    ) -> Result<AttachmentRecord> {
        if command.content.len() as u64 > crate::MAX_ATTACHMENT_BYTES {
            return Err(KanbanError::InvalidInput(
                "attachment content exceeds the 256 MiB host limit".into(),
            ));
        }
        let upload = self.begin_attachment_upload(&command.task_id).await?;
        let content = std::mem::take(&mut command.content);
        let upload = tokio::task::spawn_blocking(move || {
            let mut upload = upload;
            for chunk in content.chunks(crate::ATTACHMENT_IO_CHUNK_BYTES) {
                upload = upload.write_chunk(chunk)?;
            }
            upload.seal()
        })
        .await
        .map_err(|error| KanbanError::Storage(format!("附件写入 worker 失败：{error}")))??;
        self.commit_attachment_upload(command, upload).await
    }

    /// 接收完成后才进入 canonical mutation gate。附件记录与 created event 在同一事务内提交。
    pub async fn commit_attachment_upload(
        &self,
        command: CreateAttachmentCommand,
        upload: AttachmentUpload,
    ) -> Result<AttachmentRecord> {
        if !command.content.is_empty() {
            return Err(KanbanError::InvalidInput(
                "流式提交不能同时携带 content 数组".into(),
            ));
        }
        let task_id = canonical_task_id(&command.task_id)?;
        let filename = command.filename.trim();
        if filename.is_empty()
            || filename == "."
            || filename == ".."
            || filename.len() > 255
            || filename
                .chars()
                .any(|c| c.is_control() || c == '/' || c == '\\')
        {
            return Err(KanbanError::InvalidInput(
                "attachment filename must be a safe component of at most 255 UTF-8 bytes".into(),
            ));
        }
        let created_by = command.created_by.trim();
        if created_by.is_empty()
            || created_by.len() > 256
            || created_by.chars().any(char::is_control)
        {
            return Err(KanbanError::InvalidInput(
                "attachment created_by is invalid".into(),
            ));
        }
        let id = command
            .id
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| new_typed_id("a"));
        if !safe_attachment_id(&id) {
            return Err(KanbanError::InvalidInput(
                "attachment id must be a safe a_... component".into(),
            ));
        }
        let content_type = normalized_content_type(command.content_type.as_deref())?;
        let _mutation = self.mutation_gate.lock().await;
        let root = self.attachment_root.as_deref().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".into())
        })?;
        self.store
            .create_attachment_with_upload(
                &task_id,
                crate::CreateAttachmentInput {
                    id,
                    filename: filename.to_owned(),
                    rel_path: command.rel_path,
                    content_type,
                    content: Vec::new(),
                    sha256: command.sha256,
                    created_by: created_by.to_owned(),
                    event_id: new_event_id(),
                    created_at: self.clock.now_ms(),
                },
                root,
                upload,
            )
            .await
            .map_err(crate::error::store_error)
            .and_then(application_attachment)
    }
    pub async fn list_attachments(&self, task_id: &str) -> Result<Vec<AttachmentRecord>> {
        self.store
            .list_attachments(&canonical_task_id(task_id)?)
            .await
            .map_err(crate::error::store_error)?
            .into_iter()
            .map(application_attachment)
            .collect()
    }
    pub async fn open_attachment_stream(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<AttachmentStreamRecord> {
        let task_id = canonical_task_id(task_id)?;
        if !safe_attachment_id(attachment_id.trim()) {
            return Err(KanbanError::InvalidInput(
                "attachment id must be a safe a_... component".into(),
            ));
        }
        let root = self.attachment_root.as_deref().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".into())
        })?;
        let (record, file) = self
            .store
            .open_attachment_stream(&task_id, attachment_id.trim(), root)
            .await
            .map_err(crate::error::store_error)?;
        Ok(AttachmentStreamRecord {
            attachment: application_attachment(record)?,
            file,
        })
    }
    pub async fn read_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<AttachmentContentRecord> {
        let stream = self.open_attachment_stream(task_id, attachment_id).await?;
        let attachment = stream.attachment;
        let expected_size = attachment.size_bytes;
        let expected_hash = attachment.sha256.clone();
        let content = tokio::task::spawn_blocking(move || {
            use sha2::{Digest, Sha256};
            use std::io::Read;
            // The opened file was checked, but local edits may race the subsequent read.
            let mut file = stream.file.take(crate::MAX_ATTACHMENT_BYTES + 1);
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|error| KanbanError::Storage(error.to_string()))?;
            if bytes.len() as u64 > crate::MAX_ATTACHMENT_BYTES
                || bytes.len() as i64 != expected_size
            {
                return Err(KanbanError::Storage(
                    "attachment changed while reading".into(),
                ));
            }
            if expected_hash
                .is_some_and(|expected| expected != format!("{:x}", Sha256::digest(&bytes)))
            {
                return Err(KanbanError::Storage(
                    "attachment hash changed while reading".into(),
                ));
            }
            Ok::<_, KanbanError>(bytes)
        })
        .await
        .map_err(|error| KanbanError::Storage(format!("附件读取 worker 失败：{error}")))??;
        Ok(AttachmentContentRecord {
            attachment,
            content,
        })
    }
    pub async fn delete_attachment(&self, command: DeleteAttachmentCommand) -> Result<bool> {
        let task_id = canonical_task_id(&command.task_id)?;
        let attachment_id = command.attachment_id.trim();
        let actor = command.actor.trim();
        if !safe_attachment_id(attachment_id) || actor.is_empty() {
            return Err(KanbanError::InvalidInput(
                "attachment id or actor is invalid".into(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let root = self.attachment_root.as_deref().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".into())
        })?;
        self.store
            .delete_attachment(
                &task_id,
                attachment_id,
                root,
                actor,
                &new_event_id(),
                self.clock.now_ms(),
            )
            .await
            .map_err(crate::error::store_error)
    }
}
fn application_attachment(value: crate::domain::AttachmentRecord) -> Result<AttachmentRecord> {
    Ok(AttachmentRecord {
        id: value.id,
        board_id: value.board_id,
        task_id: value.task_id,
        filename: value.filename,
        rel_path: value.rel_path,
        content_type: value.content_type,
        size_bytes: value.size_bytes,
        sha256: value.sha256,
        created_by: value.created_by,
        created_at: value.created_at,
    })
}
fn canonical_task_id(value: &str) -> Result<String> {
    let value = value.trim();
    if !value.starts_with("t_")
        || value.len() <= 2
        || value.len() > 128
        || value
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err(KanbanError::InvalidInput(
            "task_id must be a global t_... id".into(),
        ));
    }
    Ok(value.to_owned())
}
fn safe_attachment_id(value: &str) -> bool {
    value.starts_with("a_")
        && value.len() > 2
        && value.len() <= 128
        && !value
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
}
fn normalized_content_type(value: Option<&str>) -> Result<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let value = value.split(';').next().unwrap_or_default().trim();
    let valid_token = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
    };
    let Some((major, minor)) = value.split_once('/') else {
        return Err(KanbanError::InvalidInput("附件 MIME 类型无效".into()));
    };
    if value.len() > 127 || !valid_token(major) || !valid_token(minor) {
        return Err(KanbanError::InvalidInput("附件 MIME 类型无效".into()));
    }
    Ok(Some(value.to_ascii_lowercase()))
}
