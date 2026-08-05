use turso::transaction::TransactionBehavior;

use crate::{db::TursoStore, domain::*, error::StoreError, shared::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCommentInput {
    pub id: String,
    pub idempotency_key: Option<String>,
    pub author: String,
    pub author_type: String,
    pub agent_type: Option<String>,
    pub body: String,
    pub kind: String,
    pub metadata_json: String,
    pub event_id: String,
    pub created_at: i64,
}
impl TursoStore {
    pub async fn create_comment(
        &self,
        task_id: &str,
        input: CreateCommentInput,
    ) -> Result<CommentRecord, StoreError> {
        validate_create_comment_input(task_id, &input)?;
        let id = input.id.trim().to_owned();
        let idempotency_key = input
            .idempotency_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let author = input.author.trim().to_owned();
        let author_type = input.author_type.trim().to_owned();
        let agent_type = input
            .agent_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let body = input.body.trim().to_owned();
        let kind = input.kind.trim().to_owned();
        let metadata_json = input.metadata_json.trim();
        let metadata_json = if metadata_json.is_empty() {
            "{}".to_owned()
        } else {
            metadata_json.to_owned()
        };
        let event_id = input.event_id.trim().to_owned();
        let mut connection = self.connection().await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;

        let metadata_valid = first_row(
            transaction
                .query(
                    "SELECT json_valid(:metadata_json)",
                    [(":metadata_json", metadata_json.as_str())],
                )
                .await?,
        )
        .await?;
        if integer_value(
            metadata_valid.get_value(0)?,
            "task_comments.metadata_json_valid",
        )? == 0
        {
            return Err(StoreError::InvalidInput(
                "metadata_json must be valid JSON".to_owned(),
            ));
        }
        let metadata_object = first_row(
            transaction
                .query(
                    "SELECT json_type(:metadata_json) = 'object'",
                    [(":metadata_json", metadata_json.as_str())],
                )
                .await?,
        )
        .await?;
        if integer_value(
            metadata_object.get_value(0)?,
            "task_comments.metadata_json_object",
        )? == 0
        {
            return Err(StoreError::InvalidInput(
                "metadata_json must be a JSON object".to_owned(),
            ));
        }
        let decision_metadata_valid = first_row(
                transaction
                    .query(
                        r#"SELECT CASE
                            WHEN :kind != 'decision' THEN 1
                            WHEN COALESCE(json_type(:metadata_json, '$.options'), '') != 'array'
                              OR json_array_length(json_extract(:metadata_json, '$.options')) <= 0 THEN 0
                            WHEN COALESCE(json_type(:metadata_json, '$.selected'), '') != 'text'
                              OR length(trim(json_extract(:metadata_json, '$.selected'))) = 0 THEN 0
                            WHEN COALESCE(json_type(:metadata_json, '$.reason'), '') != 'text'
                              OR length(trim(json_extract(:metadata_json, '$.reason'))) = 0 THEN 0
                            WHEN json_type(:metadata_json, '$.risk') IS NOT NULL
                              AND (COALESCE(json_type(:metadata_json, '$.risk'), '') != 'text'
                                OR length(trim(json_extract(:metadata_json, '$.risk'))) = 0) THEN 0
                            WHEN json_type(:metadata_json, '$.verification') IS NOT NULL
                              AND (COALESCE(json_type(:metadata_json, '$.verification'), '') != 'text'
                                OR length(trim(json_extract(:metadata_json, '$.verification'))) = 0) THEN 0
                            WHEN EXISTS (
                                SELECT 1 FROM json_each(json_extract(:metadata_json, '$.options')) AS option
                                WHERE COALESCE(json_type(option.value), '') != 'object'
                                  OR COALESCE(json_type(option.value, '$.slug'), '') != 'text'
                                  OR length(trim(json_extract(option.value, '$.slug'))) = 0
                                  OR json_extract(option.value, '$.slug') GLOB '*[^a-z0-9-]*'
                                  OR substr(json_extract(option.value, '$.slug'), 1, 1) GLOB '[^a-z0-9]'
                                  OR COALESCE(json_type(option.value, '$.title'), '') != 'text'
                                  OR length(trim(json_extract(option.value, '$.title'))) = 0
                                  OR COALESCE(json_type(option.value, '$.detail'), '') != 'text'
                                  OR length(trim(json_extract(option.value, '$.detail'))) = 0
                            ) THEN 0
                            WHEN (SELECT COUNT(*) FROM json_each(json_extract(:metadata_json, '$.options')))
                              != (SELECT COUNT(DISTINCT json_extract(option.value, '$.slug'))
                                  FROM json_each(json_extract(:metadata_json, '$.options')) AS option) THEN 0
                            WHEN NOT EXISTS (
                                SELECT 1 FROM json_each(json_extract(:metadata_json, '$.options')) AS option
                                WHERE json_extract(option.value, '$.slug') = json_extract(:metadata_json, '$.selected')
                            ) THEN 0
                            ELSE 1
                        END"#,
                        [
                            (":kind", kind.as_str()),
                            (":metadata_json", metadata_json.as_str()),
                        ],
                    )
                    .await?,
            )
            .await?;
        if integer_value(
            decision_metadata_valid.get_value(0)?,
            "task_comments.decision_metadata_valid",
        )? == 0
        {
            return Err(StoreError::InvalidInput(
                "invalid decision comment metadata".to_owned(),
            ));
        }

        let task = first_row(
                transaction
                    .query(
                        "SELECT t.board_id, t.archived_at, b.archived_at FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :task_id LIMIT 1",
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
        let task_archived_at = optional_integer_value(task.get_value(1)?, "tasks.archived_at")?;
        let board_archived_at = optional_integer_value(task.get_value(2)?, "boards.archived_at")?;
        if task_archived_at.is_some() || board_archived_at.is_some() {
            return Err(StoreError::InvalidTransition(
                "archived task or board cannot receive comments".to_owned(),
            ));
        }

        if let Some(idempotency_key) = idempotency_key.as_deref() {
            let existing = first_row(
                    transaction
                        .query(
                            "SELECT id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at FROM task_comments WHERE board_id = :board_id AND task_id = :task_id AND idempotency_key = :idempotency_key LIMIT 1",
                            [
                                (":board_id", board_id.as_str()),
                                (":task_id", task_id),
                                (":idempotency_key", idempotency_key),
                            ],
                        )
                        .await?,
                )
                .await;
            match existing {
                Ok(row) => {
                    let existing = comment_from_row(row)?;
                    if comment_payload_matches(
                        &existing,
                        idempotency_key,
                        &author,
                        &author_type,
                        agent_type.as_deref(),
                        &body,
                        &kind,
                        &metadata_json,
                    ) {
                        transaction.commit().await?;
                        return Ok(existing);
                    }
                    return Err(StoreError::IdempotencyConflict {
                        board_id,
                        key: idempotency_key.to_owned(),
                        existing_task_id: task_id.to_owned(),
                    });
                }
                Err(turso::Error::QueryReturnedNoRows) => {}
                Err(error) => return Err(StoreError::Turso(error)),
            }
        }

        transaction
                .execute(
                    "INSERT INTO task_comments(id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at) VALUES (:id, :board_id, :task_id, :idempotency_key, :author, :author_type, :agent_type, :body, :kind, :metadata_json, :created_at)",
                    (
                        (":id", id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":idempotency_key", idempotency_key.as_deref()),
                        (":author", author.as_str()),
                        (":author_type", author_type.as_str()),
                        (":agent_type", agent_type.as_deref()),
                        (":body", body.as_str()),
                        (":kind", kind.as_str()),
                        (":metadata_json", metadata_json.as_str()),
                        (":created_at", input.created_at),
                    ),
                )
                .await?;
        transaction
                .execute(
                    "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, NULL, 'task.comment.created', :actor, json_object('comment_id', :comment_id, 'kind', :kind, 'author_type', :author_type, 'agent_type', :agent_type), :created_at)",
                    (
                        (":event_id", event_id.as_str()),
                        (":board_id", board_id.as_str()),
                        (":task_id", task_id),
                        (":actor", author.as_str()),
                        (":comment_id", id.as_str()),
                        (":kind", kind.as_str()),
                        (":author_type", author_type.as_str()),
                        (":agent_type", agent_type.as_deref()),
                        (":created_at", input.created_at),
                    ),
                )
                .await?;

        let comment = comment_from_row(
                first_row(
                    transaction
                        .query(
                            "SELECT id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at FROM task_comments WHERE board_id = :board_id AND id = :id LIMIT 1",
                            [(":board_id", board_id.as_str()), (":id", id.as_str())],
                        )
                        .await?,
                )
                .await?,
            )?;

        transaction.commit().await?;
        Ok(comment)
    }
}

pub(crate) fn validate_create_comment_input(
    task_id: &str,
    input: &CreateCommentInput,
) -> Result<(), StoreError> {
    if !task_id.starts_with("t_") || task_id.len() <= 2 {
        return Err(StoreError::InvalidInput(
            "task id must start with t_".to_owned(),
        ));
    }
    if !input.id.trim().starts_with("c_") || input.id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "comment id must start with c_".to_owned(),
        ));
    }
    if input
        .idempotency_key
        .as_deref()
        .is_some_and(|key| key.trim().is_empty())
    {
        return Err(StoreError::InvalidInput(
            "idempotency_key must not be empty".to_owned(),
        ));
    }
    if input.author.trim().is_empty() {
        return Err(StoreError::InvalidInput("author is required".to_owned()));
    }
    if !matches!(input.author_type.trim(), "user" | "agent") {
        return Err(StoreError::InvalidInput(
            "author_type must be user or agent".to_owned(),
        ));
    }
    if input.agent_type.as_deref().is_some_and(|agent_type| {
        !agent_type.trim().is_empty() && input.author_type.trim() != "agent"
    }) {
        return Err(StoreError::InvalidInput(
            "agent_type is only allowed when author_type is agent".to_owned(),
        ));
    }
    if input.body.trim().is_empty() {
        return Err(StoreError::InvalidInput("body is required".to_owned()));
    }
    if !matches!(input.kind.trim(), "note" | "decision") {
        return Err(StoreError::InvalidInput(
            "kind must be note or decision".to_owned(),
        ));
    }
    if !input.event_id.trim().starts_with("e_") || input.event_id.trim().len() <= 2 {
        return Err(StoreError::InvalidInput(
            "event_id must start with e_".to_owned(),
        ));
    }
    if input.created_at < 0 {
        return Err(StoreError::InvalidInput(
            "created_at must be non-negative".to_owned(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn comment_payload_matches(
    existing: &CommentRecord,
    idempotency_key: &str,
    author: &str,
    author_type: &str,
    agent_type: Option<&str>,
    body: &str,
    kind: &str,
    metadata_json: &str,
) -> bool {
    existing.idempotency_key.as_deref() == Some(idempotency_key)
        && existing.author == author
        && existing.author_type == author_type
        && existing.agent_type.as_deref() == agent_type
        && existing.body == body
        && existing.kind == kind
        && existing.metadata_json == metadata_json
}
