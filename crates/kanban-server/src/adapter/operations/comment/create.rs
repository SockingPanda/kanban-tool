use kanban_application::{
    CommentCreate, CommentRecord as ApplicationComment,
    CreateCommentRecord as ApplicationCreateComment,
};
use kanban_core::Result;
use kanban_store_turso::CreateCommentInput as StoreCreateComment;

use crate::adapter::{TursoApplicationStore, application_comment, store_error};

impl CommentCreate for TursoApplicationStore {
    async fn create_comment(
        &self,
        task_id: &str,
        input: ApplicationCreateComment,
    ) -> Result<ApplicationComment> {
        self.store
            .create_comment(
                task_id,
                StoreCreateComment {
                    id: input.id,
                    idempotency_key: input.idempotency_key,
                    author: input.author,
                    author_type: input.author_type.as_str().to_owned(),
                    agent_type: input.agent_type,
                    body: input.body,
                    kind: input.kind.as_str().to_owned(),
                    metadata_json: input.metadata_json,
                    event_id: input.event_id,
                    created_at: input.created_at,
                },
            )
            .await
            .map_err(store_error)
            .and_then(application_comment)
    }
}
