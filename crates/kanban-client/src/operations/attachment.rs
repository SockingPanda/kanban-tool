use kanban_protocol::{ApiAttachment, CreateAttachmentRequest};

use crate::{KanbanClient, error::ClientError, transport::rpc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedAttachment {
    pub content_type: Option<String>,
    pub attachment_id: Option<String>,
    pub sha256: Option<String>,
    pub content: Vec<u8>,
}

impl KanbanClient {
    pub async fn list_attachments(&self, task_id: &str) -> Result<Vec<ApiAttachment>, ClientError> {
        let task_id = require_task_id(task_id)?;
        let response: kanban_protocol::ListAttachmentsResponse = rpc!(
            self,
            list_attachments,
            ListAttachmentsRequest,
            kanban_protocol::ListAttachmentsPath {
                task_id: task_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data)
    }

    pub async fn create_attachment(
        &self,
        task_id: &str,
        request: &CreateAttachmentRequest,
    ) -> Result<ApiAttachment, ClientError> {
        let task_id = require_task_id(task_id)?;
        let response: kanban_protocol::CreateAttachmentResponse = rpc!(
            self,
            create_attachment,
            CreateAttachmentRequest,
            kanban_protocol::CreateAttachmentPath {
                task_id: task_id.to_owned()
            },
            (),
            request.clone()
        )?;
        Ok(response.data)
    }

    pub async fn download_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<DownloadedAttachment, ClientError> {
        let task_id = require_task_id(task_id)?;
        let attachment_id = require_attachment_id(attachment_id)?;
        let response: kanban_protocol::rpc::dto::AttachmentDownload = rpc!(
            self,
            download_attachment,
            DownloadAttachmentRequest,
            kanban_protocol::GetAttachmentPath {
                task_id: task_id.to_owned(),
                attachment_id: attachment_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(DownloadedAttachment {
            content_type: Some(
                response
                    .attachment
                    .content_type
                    .unwrap_or_else(|| "application/octet-stream".to_owned()),
            ),
            attachment_id: Some(response.attachment.id),
            sha256: response.attachment.sha256,
            content: response.content,
        })
    }

    pub async fn delete_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<bool, ClientError> {
        let task_id = require_task_id(task_id)?;
        let attachment_id = require_attachment_id(attachment_id)?;
        let response: kanban_protocol::DeleteAttachmentResponse = rpc!(
            self,
            delete_attachment,
            DeleteAttachmentRequest,
            kanban_protocol::DeleteAttachmentPath {
                task_id: task_id.to_owned(),
                attachment_id: attachment_id.to_owned()
            },
            (),
            ()
        )?;
        Ok(response.data.deleted)
    }
}

fn require_task_id(value: &str) -> Result<&str, ClientError> {
    let value = value.trim();
    if !value.starts_with("t_") || value.len() <= 2 {
        return Err(ClientError::InvalidInput(
            "任务选择器必须解析为全局 t_... ID".to_owned(),
        ));
    }
    Ok(value)
}

fn require_attachment_id(value: &str) -> Result<&str, ClientError> {
    let value = value.trim();
    if !value.starts_with("a_")
        || value.len() <= 2
        || value.contains(['/', '\\', '\0'])
        || value == "."
        || value == ".."
    {
        return Err(ClientError::InvalidInput(
            "附件 ID 必须以 a_ 开头".to_owned(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[tokio::test]
    async fn attachment_client_requires_global_ids_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        assert_eq!(
            client
                .list_attachments("default#1")
                .await
                .unwrap_err()
                .code(),
            "invalid_input"
        );
        assert_eq!(
            client
                .download_attachment("t_1", "bad")
                .await
                .unwrap_err()
                .code(),
            "invalid_input"
        );
        assert_eq!(
            client
                .download_attachment("t_1", "a_bad/segment")
                .await
                .unwrap_err()
                .code(),
            "invalid_input"
        );
    }
}
