use kanban_protocol::{
    ApiAttachment, CreateAttachmentRequest, CreateAttachmentResponse, DeleteAttachmentResponse,
    ListAttachmentsResponse,
};

use crate::{KanbanClient, error::ClientError, transport::encode_path_segment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedAttachment {
    pub content_type: Option<String>,
    pub attachment_id: Option<String>,
    pub sha256: Option<String>,
    pub content: Vec<u8>,
}

impl KanbanClient {
    pub fn list_attachments(&self, task_id: &str) -> Result<Vec<ApiAttachment>, ClientError> {
        let task_id = require_task_id(task_id)?;
        let response: ListAttachmentsResponse = self.get(&format!(
            "/api/v1/tasks/{}/attachments",
            encode_path_segment(task_id)
        ))?;
        Ok(response.data)
    }

    pub fn create_attachment(
        &self,
        task_id: &str,
        request: &CreateAttachmentRequest,
    ) -> Result<ApiAttachment, ClientError> {
        let task_id = require_task_id(task_id)?;
        let response: CreateAttachmentResponse = self.post(
            &format!("/api/v1/tasks/{}/attachments", encode_path_segment(task_id)),
            request,
        )?;
        Ok(response.data)
    }

    pub fn download_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<DownloadedAttachment, ClientError> {
        let task_id = require_task_id(task_id)?;
        let attachment_id = require_attachment_id(attachment_id)?;
        let (content_type, returned_id, sha256, content) = self.get_bytes(
            &format!(
                "/api/v1/tasks/{}/attachments/{}",
                encode_path_segment(task_id),
                encode_path_segment(attachment_id)
            ),
            "application/octet-stream",
        )?;
        Ok(DownloadedAttachment {
            content_type,
            attachment_id: returned_id,
            sha256,
            content,
        })
    }

    pub fn delete_attachment(
        &self,
        task_id: &str,
        attachment_id: &str,
    ) -> Result<bool, ClientError> {
        let task_id = require_task_id(task_id)?;
        let attachment_id = require_attachment_id(attachment_id)?;
        let response: DeleteAttachmentResponse = self.delete(&format!(
            "/api/v1/tasks/{}/attachments/{}",
            encode_path_segment(task_id),
            encode_path_segment(attachment_id)
        ))?;
        Ok(response.data.deleted)
    }
}

fn require_task_id(value: &str) -> Result<&str, ClientError> {
    let value = value.trim();
    if !value.starts_with("t_") || value.len() <= 2 {
        return Err(ClientError::InvalidInput(
            "task selector must resolve to a global t_... id".to_owned(),
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
            "attachment id must start with a_".to_owned(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use crate::{DEFAULT_SERVER_URL, KanbanClient};

    #[test]
    fn attachment_client_requires_global_ids_before_http() {
        let client = KanbanClient::new(DEFAULT_SERVER_URL, "test").unwrap();
        assert_eq!(
            client.list_attachments("default#1").unwrap_err().code(),
            "invalid_input"
        );
        assert_eq!(
            client.download_attachment("t_1", "bad").unwrap_err().code(),
            "invalid_input"
        );
        assert_eq!(
            client
                .download_attachment("t_1", "a_bad/segment")
                .unwrap_err()
                .code(),
            "invalid_input"
        );
    }
}
