use crate::{
    AttachmentContentRecord, AttachmentCreate, AttachmentDelete, AttachmentList, AttachmentRead,
    AttachmentRecord as ApplicationAttachment,
    CreateAttachmentRecord as ApplicationCreateAttachment, DeleteAttachmentCommand,
};
use kanban_core::{Clock, KanbanError, Result};
use crate::CreateAttachmentInput as StoreCreateAttachment;

use crate::adapter::{TursoApplicationStore, store_error};

impl AttachmentCreate for TursoApplicationStore {
    async fn create_attachment(
        &self,
        task_id: &str,
        input: ApplicationCreateAttachment,
    ) -> Result<ApplicationAttachment> {
        let root = self.attachment_root().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".to_owned())
        })?;
        self.store
            .create_attachment(
                task_id,
                StoreCreateAttachment {
                    id: input.id,
                    filename: input.filename,
                    rel_path: input.rel_path,
                    content_type: input.content_type,
                    content: input.content,
                    sha256: input.sha256,
                    created_by: input.created_by,
                    created_at: input.created_at,
                    event_id: input.event_id,
                },
                root,
            )
            .await
            .map_err(store_error)
            .and_then(application_attachment)
    }
}

impl AttachmentList for TursoApplicationStore {
    async fn list_attachments(&self, task_id: &str) -> Result<Vec<ApplicationAttachment>> {
        self.store
            .list_attachments(task_id)
            .await
            .map_err(store_error)?
            .into_iter()
            .map(application_attachment)
            .collect()
    }
}

impl AttachmentRead for TursoApplicationStore {
    async fn read_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<AttachmentContentRecord> {
        let root = self.attachment_root().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".to_owned())
        })?;
        let (record, content) = self
            .store
            .read_attachment(task_id, attachment_id, root)
            .await
            .map_err(store_error)?;
        Ok(AttachmentContentRecord {
            attachment: application_attachment(record)?,
            content,
        })
    }
}

impl AttachmentDelete for TursoApplicationStore {
    async fn delete_attachment(&self, command: DeleteAttachmentCommand) -> Result<bool> {
        let root = self.attachment_root().ok_or_else(|| {
            KanbanError::Storage("attachment root is not configured for this host".to_owned())
        })?;
        self.store
            .delete_attachment(
                &command.task_id,
                &command.attachment_id,
                root,
                &command.actor,
                &kanban_core::new_event_id(),
                kanban_core::SystemClock.now_ms(),
            )
            .await
            .map_err(store_error)
    }
}

fn application_attachment(
    attachment: crate::domain::AttachmentRecord,
) -> Result<ApplicationAttachment> {
    Ok(ApplicationAttachment {
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
