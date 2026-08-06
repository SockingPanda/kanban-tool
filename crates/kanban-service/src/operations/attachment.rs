use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};

use crate::{AttachmentContentRecord, AttachmentRecord, KanbanService};

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

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn create_attachment(
        &self,
        command: CreateAttachmentCommand,
    ) -> Result<AttachmentRecord> {
        let task_id = canonical_task_id(&command.task_id)?;
        let filename = command.filename.trim();
        if filename.is_empty() || filename.contains(['/', '\\', '\0']) {
            return Err(KanbanError::InvalidInput(
                "attachment filename must be a single safe path component".to_owned(),
            ));
        }
        let created_by = command.created_by.trim();
        if created_by.is_empty() {
            return Err(KanbanError::InvalidInput(
                "attachment created_by is required".to_owned(),
            ));
        }
        if command.content.len() as u64 > 256 * 1024 * 1024 {
            return Err(KanbanError::InvalidInput(
                "attachment content exceeds the 256 MiB host limit".to_owned(),
            ));
        }
        let id = command
            .id
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| new_typed_id("a"));
        if !safe_attachment_id(&id) {
            return Err(KanbanError::InvalidInput(
                "attachment id must start with a_".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let root = self.application.store.attachment_root().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".to_owned())
        })?;
        self.application
            .store
            .store
            .create_attachment(
                &task_id,
                crate::CreateAttachmentInput {
                    id,
                    filename: filename.to_owned(),
                    rel_path: command.rel_path,
                    content_type: command
                        .content_type
                        .map(|value| value.trim().to_owned())
                        .filter(|value| !value.is_empty()),
                    content: command.content,
                    sha256: command.sha256,
                    created_by: created_by.to_owned(),
                    event_id: new_event_id(),
                    created_at: self.clock.now_ms(),
                },
                root,
            )
            .await
            .map_err(crate::adapter::store_error)
            .and_then(application_attachment)
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn list_attachments(&self, task_id: &str) -> Result<Vec<AttachmentRecord>> {
        self.application
            .store
            .store
            .list_attachments(&canonical_task_id(task_id)?)
            .await
            .map_err(crate::adapter::store_error)?
            .into_iter()
            .map(application_attachment)
            .collect()
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn read_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<AttachmentContentRecord> {
        let task_id = canonical_task_id(task_id)?;
        if !safe_attachment_id(attachment_id.trim()) {
            return Err(KanbanError::InvalidInput(
                "attachment id must start with a_".to_owned(),
            ));
        }
        let root = self.application.store.attachment_root().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".to_owned())
        })?;
        let (record, content) = self
            .application
            .store
            .store
            .read_attachment(&task_id, attachment_id.trim(), root)
            .await
            .map_err(crate::adapter::store_error)?;
        Ok(AttachmentContentRecord {
            attachment: application_attachment(record)?,
            content,
        })
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn delete_attachment(&self, command: DeleteAttachmentCommand) -> Result<bool> {
        let task_id = canonical_task_id(&command.task_id)?;
        let attachment_id = command.attachment_id.trim();
        let actor = command.actor.trim();
        if !safe_attachment_id(attachment_id) {
            return Err(KanbanError::InvalidInput(
                "attachment id must start with a_".to_owned(),
            ));
        }
        if actor.is_empty() {
            return Err(KanbanError::InvalidInput(
                "attachment actor is required".to_owned(),
            ));
        }
        let _mutation = self.mutation_gate.lock().await;
        let root = self.application.store.attachment_root().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".to_owned())
        })?;
        self.application
            .store
            .store
            .delete_attachment(
                &task_id,
                attachment_id,
                root,
                actor,
                &new_event_id(),
                self.clock.now_ms(),
            )
            .await
            .map_err(crate::adapter::store_error)
    }
}

fn application_attachment(attachment: crate::domain::AttachmentRecord) -> Result<AttachmentRecord> {
    Ok(AttachmentRecord {
        id: attachment.id,
        board_id: attachment.board_id,
        task_id: attachment.task_id,
        filename: attachment.filename,
        rel_path: attachment.rel_path,
        content_type: attachment.content_type,
        size_bytes: attachment.size_bytes,
        sha256: attachment.sha256,
        created_by: attachment.created_by,
        created_at: attachment.created_at,
    })
}

fn canonical_task_id(value: &str) -> Result<String> {
    let value = value.trim();
    if !value.starts_with("t_") || value.len() <= 2 {
        return Err(KanbanError::InvalidInput(
            "task_id must be a global t_... id".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn safe_attachment_id(value: &str) -> bool {
    value.starts_with("a_")
        && value.len() > 2
        && !value.contains(['/', '\\', '\0'])
        && value != "."
        && value != ".."
}
