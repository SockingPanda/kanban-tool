use super::support::request_actor;
use crate::application::support::CallContext;
use crate::{error::ApiError, state::AppState};
use kanban_protocol::{
    ApiAttachment, CreateAttachmentPath, CreateAttachmentRequest, CreateAttachmentResponse,
    DeleteAttachmentPath, DeleteAttachmentResponse, GetAttachmentPath, ListAttachmentsPath,
    ListAttachmentsResponse,
};
use kanban_service::{CreateAttachmentCommand, DeleteAttachmentCommand};
pub(crate) async fn list_attachments(
    state: AppState,
    ListAttachmentsPath { task_id }: ListAttachmentsPath,
) -> Result<ListAttachmentsResponse, ApiError> {
    let data = state
        .application()
        .list_attachments(&task_id)
        .await?
        .into_iter()
        .map(api_attachment)
        .collect();
    Ok(ListAttachmentsResponse { data })
}
pub(crate) async fn create_attachment(
    state: AppState,
    CreateAttachmentPath { task_id }: CreateAttachmentPath,
    headers: CallContext,
    body: CreateAttachmentRequest,
) -> Result<CreateAttachmentResponse, ApiError> {
    let actor = request_actor(body.actor.as_deref(), &headers, state.default_actor())?;
    let attachment = state
        .application()
        .create_attachment(CreateAttachmentCommand {
            task_id,
            id: body.id,
            filename: body.filename,
            rel_path: body.rel_path,
            content_type: body.content_type,
            content: body.content,
            sha256: body.sha256,
            created_by: actor,
        })
        .await?;
    Ok(CreateAttachmentResponse {
        data: api_attachment(attachment),
    })
}
pub(crate) async fn download_attachment(
    state: AppState,
    GetAttachmentPath {
        task_id,
        attachment_id,
    }: GetAttachmentPath,
) -> Result<(ApiAttachment, Vec<u8>), ApiError> {
    let content = state
        .application()
        .read_attachment(&task_id, &attachment_id)
        .await?;
    Ok((api_attachment(content.attachment), content.content))
}
pub(crate) async fn delete_attachment(
    state: AppState,
    DeleteAttachmentPath {
        task_id,
        attachment_id,
    }: DeleteAttachmentPath,
    headers: CallContext,
) -> Result<DeleteAttachmentResponse, ApiError> {
    let actor = request_actor(None, &headers, state.default_actor())?;
    let deleted = state
        .application()
        .delete_attachment(DeleteAttachmentCommand {
            task_id,
            attachment_id,
            actor,
        })
        .await?;
    Ok(DeleteAttachmentResponse {
        data: kanban_protocol::DeleteResult { deleted },
    })
}
fn api_attachment(attachment: kanban_service::AttachmentRecord) -> ApiAttachment {
    ApiAttachment {
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
    }
}
