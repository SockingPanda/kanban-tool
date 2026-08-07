use std::collections::HashSet;

use turso::transaction::TransactionBehavior;

use crate::{
    db::TursoStore,
    domain::{
        CommentRecord, SignalLifecycle, SignalObservationRecord, SignalRecord, SignalRecordResult,
    },
    error::StoreError,
    shared::*,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSignalInput {
    pub id: String,
    pub observation_id: String,
    pub event_id: String,
    pub board: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub severity: String,
    pub task_ref: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub comment_id: Option<String>,
    pub actor: String,
    pub agent_type: Option<String>,
    pub dedupe_key: Option<String>,
    pub source: Option<String>,
    pub evidence_json: String,
    pub comment_body: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewSignalsInput {
    pub board: Option<String>,
    pub signal_ids: Vec<String>,
    pub lifecycle: SignalLifecycle,
    pub replacement_signal_id: Option<String>,
    pub actor: String,
    pub reason: String,
    pub event_ids: Vec<String>,
    pub now: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalListOptions {
    pub statuses: Vec<String>,
    pub kinds: Vec<String>,
    pub task_ref: Option<String>,
    pub include_all: bool,
    pub limit: usize,
}

const SIGNAL_SELECT: &str = "SELECT s.id, s.board_id, s.observation_id, s.kind, s.title, s.summary, s.severity, s.status, s.dedupe_key, s.superseded_by_signal_id, s.reviewed_by, s.reviewed_at, s.review_reason, s.created_at, s.updated_at, o.id, o.board_id, o.task_id, o.task_ref_snapshot, o.run_id, o.comment_id, o.actor, o.agent_type, o.source, o.evidence_json, o.created_at FROM signals AS s JOIN signal_observations AS o ON o.id = s.observation_id AND o.board_id = s.board_id";

impl TursoStore {
    pub async fn record_signal(
        &self,
        input: CreateSignalInput,
    ) -> Result<SignalRecordResult, StoreError> {
        validate_create_signal_input(&input)?;
        let mut connection = self.connection().await?;
        let mut transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let (board_id, board_slug) = resolve_board_tx(&mut transaction, &input.board).await?;
        let evidence_json = validate_json_object(&mut transaction, &input.evidence_json).await?;
        let task = resolve_task_tx(
            &mut transaction,
            &board_id,
            &board_slug,
            input.task_ref.as_deref().or(input.task_id.as_deref()),
        )
        .await?;
        if let Some(run_id) = input.run_id.as_deref() {
            ensure_board_row_tx(&mut transaction, "task_runs", run_id, &board_id, "run").await?;
        }
        if let Some(comment_id) = input.comment_id.as_deref() {
            ensure_board_row_tx(
                &mut transaction,
                "task_comments",
                comment_id,
                &board_id,
                "comment",
            )
            .await?;
        }

        if let Some(dedupe_key) = input.dedupe_key.as_deref()
            && let Some(existing) =
                find_signal_by_dedupe_tx(&mut transaction, &board_id, dedupe_key).await?
        {
            let matches = signal_payload_matches(
                &existing,
                &input,
                task.as_ref().map(|value| value.0.as_str()),
                evidence_json.as_str(),
            );
            if matches {
                let backlink = find_backlink_tx(&mut transaction, &board_id, &existing.id).await?;
                transaction.commit().await?;
                return Ok(SignalRecordResult {
                    signal: existing,
                    backlink_comment: backlink,
                });
            }
            return Err(StoreError::SignalIdempotencyConflict {
                board_id,
                key: dedupe_key.to_owned(),
                existing_signal_id: existing.id,
            });
        }

        let task_id = task.as_ref().map(|value| value.0.clone());
        let task_ref_snapshot = task.as_ref().map(|value| value.1.clone());
        transaction
            .execute(
                "INSERT INTO signal_observations(id, board_id, task_id, task_ref_snapshot, run_id, comment_id, actor, agent_type, source, evidence_json, created_at) VALUES (:id, :board_id, :task_id, :task_ref_snapshot, :run_id, :comment_id, :actor, :agent_type, :source, :evidence_json, :created_at)",
                (
                    (":id", input.observation_id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":task_id", task_id.as_deref()),
                    (":task_ref_snapshot", task_ref_snapshot.as_deref()),
                    (":run_id", input.run_id.as_deref()),
                    (":comment_id", input.comment_id.as_deref()),
                    (":actor", input.actor.as_str()),
                    (":agent_type", input.agent_type.as_deref()),
                    (":source", input.source.as_deref()),
                    (":evidence_json", evidence_json.as_str()),
                    (":created_at", input.created_at),
                ),
            )
            .await
            .map_err(map_insert_signal_error)?;
        transaction
            .execute(
                "INSERT INTO signals(id, board_id, observation_id, kind, title, summary, severity, status, dedupe_key, created_at, updated_at) VALUES (:id, :board_id, :observation_id, :kind, :title, :summary, :severity, 'open', :dedupe_key, :created_at, :updated_at)",
                (
                    (":id", input.id.as_str()),
                    (":board_id", board_id.as_str()),
                    (":observation_id", input.observation_id.as_str()),
                    (":kind", input.kind.as_str()),
                    (":title", input.title.as_str()),
                    (":summary", input.summary.as_str()),
                    (":severity", input.severity.as_str()),
                    (":dedupe_key", input.dedupe_key.as_deref()),
                    (":created_at", input.created_at),
                    (":updated_at", input.created_at),
                ),
            )
            .await
            .map_err(map_insert_signal_error)?;

        let backlink_comment = if let Some(task_id) = task_id.as_deref() {
            let body = input
                .comment_body
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Signal: {}", input.title));
            Some(
                insert_signal_comment_tx(&mut transaction, &board_id, task_id, &input, &body)
                    .await?,
            )
        } else {
            None
        };

        insert_event_tx(
            &mut transaction,
            EventInsertInput {
                board_id: &board_id,
                task_id: task_id.as_deref(),
                run_id: input.run_id.as_deref(),
                kind: "signal.recorded",
                actor: &input.actor,
                payload_json: &serde_json::json!({
                    "signal_id": input.id,
                    "observation_id": input.observation_id,
                    "kind": input.kind,
                    "status": "open"
                })
                .to_string(),
                created_at: input.created_at,
                event_id: &input.event_id,
            },
        )
        .await?;

        let signal = load_signal_tx(&mut transaction, &input.id).await?;
        transaction.commit().await?;
        Ok(SignalRecordResult {
            signal,
            backlink_comment,
        })
    }

    pub async fn list_signals(
        &self,
        board: &str,
        options: SignalListOptions,
    ) -> Result<Vec<SignalRecord>, StoreError> {
        validate_list_options(&options)?;
        let connection = self.connection().await?;
        let (board_id, board_slug) = resolve_board(&connection, board).await?;
        let mut predicates = vec!["s.board_id = :board_id".to_owned()];
        let mut params = vec![(":board_id".to_owned(), Value::Text(board_id.clone()))];
        if options.statuses.is_empty() && !options.include_all {
            predicates.push("s.status IN ('open', 'confirmed')".to_owned());
        } else if !options.statuses.is_empty() {
            let placeholders = options
                .statuses
                .iter()
                .enumerate()
                .map(|(index, _)| format!(":status_{index}"))
                .collect::<Vec<_>>();
            predicates.push(format!("s.status IN ({})", placeholders.join(", ")));
            for (index, status) in options.statuses.iter().enumerate() {
                params.push((format!(":status_{index}"), Value::Text(status.clone())));
            }
        }
        if !options.kinds.is_empty() {
            let placeholders = options
                .kinds
                .iter()
                .enumerate()
                .map(|(index, _)| format!(":kind_{index}"))
                .collect::<Vec<_>>();
            predicates.push(format!("s.kind IN ({})", placeholders.join(", ")));
            for (index, kind) in options.kinds.iter().enumerate() {
                params.push((format!(":kind_{index}"), Value::Text(kind.clone())));
            }
        }
        if let Some(task_ref) = options.task_ref.as_deref() {
            let (task_id, _) = resolve_task(&connection, &board_id, &board_slug, task_ref).await?;
            predicates.push("o.task_id = :task_id".to_owned());
            params.push((":task_id".to_owned(), Value::Text(task_id)));
        }
        params.push((":limit".to_owned(), Value::Integer(options.limit as i64)));
        let mut rows = connection
            .query(
                &format!(
                    "{SIGNAL_SELECT} WHERE {} ORDER BY s.created_at DESC, s.id ASC LIMIT :limit",
                    predicates.join(" AND ")
                ),
                params,
            )
            .await?;
        let mut signals = Vec::new();
        while let Some(row) = rows.next().await? {
            signals.push(signal_from_row(row)?);
        }
        Ok(signals)
    }

    pub async fn get_signal(&self, signal_id: &str) -> Result<SignalRecord, StoreError> {
        let connection = self.connection().await?;
        load_signal_connection(&connection, signal_id).await
    }

    pub async fn review_signals(
        &self,
        input: ReviewSignalsInput,
    ) -> Result<Vec<SignalRecord>, StoreError> {
        validate_review_input(&input)?;
        if input.signal_ids.len() != input.event_ids.len() {
            return Err(StoreError::InvalidInput(
                "review event ids must match signal ids".to_owned(),
            ));
        }
        let mut connection = self.connection().await?;
        let mut transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await?;
        let mut current: Vec<SignalRecord> = Vec::with_capacity(input.signal_ids.len());
        for signal_id in &input.signal_ids {
            let signal = load_signal_tx(&mut transaction, signal_id).await?;
            if let Some(first) = current.first()
                && first.board_id != signal.board_id
            {
                return Err(StoreError::InvalidInput(
                    "all reviewed signals must belong to the same board".to_owned(),
                ));
            }
            current.push(signal);
        }
        if let Some(board) = input.board.as_deref() {
            let (expected_board_id, _) = resolve_board_tx(&mut transaction, board).await?;
            if current
                .iter()
                .any(|signal| signal.board_id != expected_board_id)
            {
                return Err(StoreError::InvalidInput(
                    "signal does not belong to the requested board".to_owned(),
                ));
            }
        }
        let target = target_status(&input.lifecycle);
        for signal in &current {
            validate_signal_transition(signal.status.as_str(), target)?;
        }
        let replacement = if let Some(replacement_id) = input.replacement_signal_id.as_deref() {
            let replacement = load_signal_tx(&mut transaction, replacement_id).await?;
            if replacement.board_id != current[0].board_id {
                return Err(StoreError::InvalidInput(
                    "replacement signal must be on the same board".to_owned(),
                ));
            }
            for signal in &current {
                if replacement.id == signal.id
                    || supersede_path_contains_tx(&mut transaction, replacement_id, &signal.id)
                        .await?
                {
                    return Err(StoreError::InvalidInput(
                        "signal supersede cycle detected".to_owned(),
                    ));
                }
            }
            Some(replacement_id.to_owned())
        } else {
            None
        };
        let board_id = current[0].board_id.clone();
        for (index, signal) in current.iter().enumerate() {
            transaction
                .execute(
                    "UPDATE signals SET status = :status, superseded_by_signal_id = :replacement, reviewed_by = :actor, reviewed_at = :reviewed_at, review_reason = :reason, updated_at = :reviewed_at WHERE id = :signal_id AND board_id = :board_id",
                    (
                        (":status", target),
                        (":replacement", replacement.as_deref()),
                        (":actor", input.actor.as_str()),
                        (":reviewed_at", input.now),
                        (":reason", input.reason.as_str()),
                        (":signal_id", signal.id.as_str()),
                        (":board_id", board_id.as_str()),
                    ),
                )
                .await?;
            transaction
                .execute(
                    "UPDATE task_comments SET metadata_json = json_set(metadata_json, '$.signal_status', :status) WHERE board_id = :board_id AND kind = 'signal' AND json_extract(metadata_json, '$.signal_id') = :signal_id",
                    (
                        (":status", target),
                        (":board_id", board_id.as_str()),
                        (":signal_id", signal.id.as_str()),
                    ),
                )
                .await?;
            insert_event_tx(
                &mut transaction,
                EventInsertInput {
                    board_id: &board_id,
                    task_id: signal.observation.task_id.as_deref(),
                    run_id: signal.observation.run_id.as_deref(),
                    kind: "signal.reviewed",
                    actor: &input.actor,
                    payload_json: &serde_json::json!({
                        "signal_id": signal.id,
                        "status": target,
                        "reason": input.reason
                    })
                    .to_string(),
                    created_at: input.now,
                    event_id: &input.event_ids[index],
                },
            )
            .await?;
        }
        let mut result = Vec::with_capacity(current.len());
        for signal in &current {
            result.push(load_signal_tx(&mut transaction, &signal.id).await?);
        }
        transaction.commit().await?;
        Ok(result)
    }
}

use turso::Value;

fn validate_create_signal_input(input: &CreateSignalInput) -> Result<(), StoreError> {
    for (field, value) in [
        ("signal id", input.id.as_str()),
        ("observation id", input.observation_id.as_str()),
        ("event id", input.event_id.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(StoreError::InvalidInput(format!("{field} is required")));
        }
    }
    if !input.id.starts_with("sig_") || !input.observation_id.starts_with("obs_") {
        return Err(StoreError::InvalidInput(
            "signal ids must use sig_ and obs_ prefixes".to_owned(),
        ));
    }
    if !input.event_id.starts_with("e_") {
        return Err(StoreError::InvalidInput(
            "event id must start with e_".to_owned(),
        ));
    }
    for (field, value) in [
        ("kind", input.kind.as_str()),
        ("title", input.title.as_str()),
        ("summary", input.summary.as_str()),
        ("severity", input.severity.as_str()),
        ("actor", input.actor.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(StoreError::InvalidInput(format!("{field} is required")));
        }
    }
    Ok(())
}

fn validate_list_options(options: &SignalListOptions) -> Result<(), StoreError> {
    if options.limit == 0 || options.limit > 1000 {
        return Err(StoreError::InvalidInput(
            "signal list limit must be between 1 and 1000".to_owned(),
        ));
    }
    if options.kinds.iter().any(|kind| kind.trim().is_empty()) {
        return Err(StoreError::InvalidInput(
            "signal kind filters must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn validate_review_input(input: &ReviewSignalsInput) -> Result<(), StoreError> {
    if input.signal_ids.is_empty() || input.event_ids.is_empty() {
        return Err(StoreError::InvalidInput(
            "at least one signal id is required".to_owned(),
        ));
    }
    if input.actor.trim().is_empty() || input.reason.trim().is_empty() {
        return Err(StoreError::InvalidInput(
            "actor and reason are required".to_owned(),
        ));
    }
    if matches!(input.lifecycle, SignalLifecycle::Supersede)
        != input.replacement_signal_id.is_some()
    {
        return Err(StoreError::InvalidInput(
            "supersede requires replacement signal id".to_owned(),
        ));
    }
    Ok(())
}

fn target_status(lifecycle: &SignalLifecycle) -> &'static str {
    match lifecycle {
        SignalLifecycle::Confirm => "confirmed",
        SignalLifecycle::Reject => "rejected",
        SignalLifecycle::Resolve => "resolved",
        SignalLifecycle::Supersede => "superseded",
    }
}

fn validate_signal_transition(current: &str, target: &str) -> Result<(), StoreError> {
    let valid = matches!(
        (current, target),
        ("open", "confirmed")
            | ("open", "rejected")
            | ("open", "resolved")
            | ("open", "superseded")
            | ("confirmed", "resolved")
    );
    if valid {
        Ok(())
    } else {
        Err(StoreError::InvalidTransition(format!(
            "invalid signal transition: {current} -> {target}"
        )))
    }
}

async fn resolve_board(
    connection: &turso::Connection,
    selector: &str,
) -> Result<(String, String), StoreError> {
    let row = first_row(
        connection
            .query(
                "SELECT id, slug FROM boards WHERE id = :selector OR slug = :selector LIMIT 1",
                [(":selector", selector.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    Ok((
        text_value(row.get_value(0)?, "boards.id")?,
        text_value(row.get_value(1)?, "boards.slug")?,
    ))
}

async fn resolve_board_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    selector: &str,
) -> Result<(String, String), StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT id, slug FROM boards WHERE id = :selector OR slug = :selector LIMIT 1",
                [(":selector", selector.trim())],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::BoardNotFound(selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    Ok((
        text_value(row.get_value(0)?, "boards.id")?,
        text_value(row.get_value(1)?, "boards.slug")?,
    ))
}

async fn resolve_task(
    connection: &turso::Connection,
    board_id: &str,
    board_slug: &str,
    selector: &str,
) -> Result<(String, String), StoreError> {
    resolve_task_query(connection, board_id, board_slug, selector).await
}

async fn resolve_task_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    board_id: &str,
    board_slug: &str,
    selector: Option<&str>,
) -> Result<Option<(String, String)>, StoreError> {
    let Some(selector) = selector else {
        return Ok(None);
    };
    resolve_task_query_tx(transaction, board_id, board_slug, selector)
        .await
        .map(Some)
}

async fn resolve_task_query(
    connection: &turso::Connection,
    board_id: &str,
    board_slug: &str,
    selector: &str,
) -> Result<(String, String), StoreError> {
    let selector = selector.trim();
    let query = if selector.starts_with("t_") {
        "SELECT t.id, b.slug, t.seq FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :selector LIMIT 1"
    } else {
        "SELECT t.id, b.slug, t.seq FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.board_id = :board_id AND ((:selector LIKE '%#%' AND b.slug = substr(:selector, 1, instr(:selector, '#') - 1) AND t.seq = CAST(substr(:selector, instr(:selector, '#') + 1) AS INTEGER)) OR (:selector NOT LIKE '%#%' AND t.seq = CAST(:selector AS INTEGER))) LIMIT 1"
    };
    let row = first_row(
        connection
            .query(query, [(":selector", selector), (":board_id", board_id)])
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    let found_board = text_value(row.get_value(1)?, "boards.slug")?;
    let id = text_value(row.get_value(0)?, "tasks.id")?;
    let seq = integer_value(row.get_value(2)?, "tasks.seq")?;
    if found_board != board_slug {
        return Err(StoreError::InvalidInput(
            "task belongs to a different board".to_owned(),
        ));
    }
    Ok((id, format!("{board_slug}#{seq}")))
}

async fn resolve_task_query_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    board_id: &str,
    board_slug: &str,
    selector: &str,
) -> Result<(String, String), StoreError> {
    let selector = selector.trim();
    let query = if selector.starts_with("t_") {
        "SELECT t.id, b.slug, t.seq FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.id = :selector LIMIT 1"
    } else {
        "SELECT t.id, b.slug, t.seq FROM tasks AS t JOIN boards AS b ON b.id = t.board_id WHERE t.board_id = :board_id AND ((:selector LIKE '%#%' AND b.slug = substr(:selector, 1, instr(:selector, '#') - 1) AND t.seq = CAST(substr(:selector, instr(:selector, '#') + 1) AS INTEGER)) OR (:selector NOT LIKE '%#%' AND t.seq = CAST(:selector AS INTEGER))) LIMIT 1"
    };
    let row = first_row(
        transaction
            .query(query, [(":selector", selector), (":board_id", board_id)])
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::TaskNotFound(selector.to_owned()),
        other => StoreError::Turso(other),
    })?;
    let found_board = text_value(row.get_value(1)?, "boards.slug")?;
    let id = text_value(row.get_value(0)?, "tasks.id")?;
    let seq = integer_value(row.get_value(2)?, "tasks.seq")?;
    if found_board != board_slug {
        return Err(StoreError::InvalidInput(
            "task belongs to a different board".to_owned(),
        ));
    }
    Ok((id, format!("{board_slug}#{seq}")))
}

async fn ensure_board_row_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    table: &str,
    id: &str,
    board_id: &str,
    label: &str,
) -> Result<(), StoreError> {
    let row = first_row(
        transaction
            .query(
                &format!("SELECT board_id FROM {table} WHERE id = :id LIMIT 1"),
                [(":id", id)],
            )
            .await?,
    )
    .await;
    match row {
        Ok(row) => {
            let found = text_value(row.get_value(0)?, "board_id")?;
            if found == board_id {
                Ok(())
            } else {
                Err(StoreError::InvalidInput(format!(
                    "{label} belongs to a different board"
                )))
            }
        }
        Err(turso::Error::QueryReturnedNoRows) => {
            Err(StoreError::InvalidInput(format!("{label} not found: {id}")))
        }
        Err(error) => Err(StoreError::Turso(error)),
    }
}

async fn validate_json_object(
    transaction: &mut turso::transaction::Transaction<'_>,
    json: &str,
) -> Result<String, StoreError> {
    let row = first_row(
        transaction
            .query(
                "SELECT json_valid(:json), json_type(:json)",
                [(":json", json)],
            )
            .await?,
    )
    .await?;
    let valid = integer_value(row.get_value(0)?, "signal evidence valid")? != 0;
    let object = optional_text_value(row.get_value(1)?, "signal evidence type")?;
    if !valid || object.as_deref() != Some("object") {
        return Err(StoreError::InvalidInput(
            "evidence must be a JSON object".to_owned(),
        ));
    }
    Ok(json.to_owned())
}

async fn find_signal_by_dedupe_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    board_id: &str,
    dedupe_key: &str,
) -> Result<Option<SignalRecord>, StoreError> {
    let rows = transaction
        .query(
            &format!("{SIGNAL_SELECT} WHERE s.board_id = :board_id AND s.dedupe_key = :dedupe_key LIMIT 1"),
            [(":board_id", board_id), (":dedupe_key", dedupe_key)],
        )
        .await?;
    match first_row(rows).await {
        Ok(row) => Ok(Some(signal_from_row(row)?)),
        Err(turso::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(StoreError::Turso(error)),
    }
}

fn signal_payload_matches(
    existing: &SignalRecord,
    input: &CreateSignalInput,
    task_id: Option<&str>,
    evidence_json: &str,
) -> bool {
    existing.kind == input.kind
        && existing.title == input.title
        && existing.summary == input.summary
        && existing.severity == input.severity
        && existing.observation.task_id.as_deref() == task_id
        && existing.observation.run_id == input.run_id
        && existing.observation.comment_id == input.comment_id
        && existing.observation.actor == input.actor
        && existing.observation.agent_type == input.agent_type
        && existing.observation.source == input.source
        && existing.observation.evidence_json == evidence_json
}

async fn find_backlink_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    board_id: &str,
    signal_id: &str,
) -> Result<Option<CommentRecord>, StoreError> {
    let rows = transaction
        .query(
            "SELECT id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at FROM task_comments WHERE board_id = :board_id AND kind = 'signal' AND json_extract(metadata_json, '$.signal_id') = :signal_id ORDER BY created_at ASC, id ASC LIMIT 1",
            [(":board_id", board_id), (":signal_id", signal_id)],
        )
        .await?;
    match first_row(rows).await {
        Ok(row) => Ok(Some(comment_from_row(row)?)),
        Err(turso::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(StoreError::Turso(error)),
    }
}

async fn insert_signal_comment_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    board_id: &str,
    task_id: &str,
    input: &CreateSignalInput,
    body: &str,
) -> Result<CommentRecord, StoreError> {
    let id = kanban_core::new_typed_id("c");
    let metadata_json = serde_json::json!({
        "type": "signal_link",
        "signal_id": input.id,
        "observation_id": input.observation_id,
        "signal_kind": input.kind,
        "signal_status": "open"
    })
    .to_string();
    transaction
        .execute(
            "INSERT INTO task_comments(id, board_id, task_id, idempotency_key, author, author_type, agent_type, body, kind, metadata_json, created_at) VALUES (:id, :board_id, :task_id, NULL, :author, 'agent', :agent_type, :body, 'signal', :metadata_json, :created_at)",
            (
                (":id", id.as_str()),
                (":board_id", board_id),
                (":task_id", task_id),
                (":author", input.actor.as_str()),
                (":agent_type", input.agent_type.as_deref()),
                (":body", body),
                (":metadata_json", metadata_json.as_str()),
                (":created_at", input.created_at),
            ),
        )
        .await?;
    let event_id = kanban_core::new_event_id();
    insert_event_tx(
        transaction,
        EventInsertInput {
            board_id,
            task_id: Some(task_id),
            run_id: input.run_id.as_deref(),
            kind: "task.comment.created",
            actor: &input.actor,
            payload_json: &serde_json::json!({
                "comment_id": id,
                "kind": "signal",
                "author_type": "agent",
                "agent_type": input.agent_type
            })
            .to_string(),
            created_at: input.created_at,
            event_id: &event_id,
        },
    )
    .await?;
    Ok(CommentRecord {
        id,
        board_id: board_id.to_owned(),
        task_id: task_id.to_owned(),
        idempotency_key: None,
        author: input.actor.clone(),
        author_type: "agent".to_owned(),
        agent_type: input.agent_type.clone(),
        body: body.to_owned(),
        kind: "signal".to_owned(),
        metadata_json,
        created_at: input.created_at,
    })
}

struct EventInsertInput<'a> {
    board_id: &'a str,
    task_id: Option<&'a str>,
    run_id: Option<&'a str>,
    kind: &'a str,
    actor: &'a str,
    payload_json: &'a str,
    created_at: i64,
    event_id: &'a str,
}

async fn insert_event_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    input: EventInsertInput<'_>,
) -> Result<(), StoreError> {
    let EventInsertInput {
        board_id,
        task_id,
        run_id,
        kind,
        actor,
        payload_json,
        created_at,
        event_id,
    } = input;
    transaction
        .execute(
            "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (:event_id, :board_id, :task_id, :run_id, :kind, :actor, :payload_json, :created_at)",
            (
                (":event_id", event_id),
                (":board_id", board_id),
                (":task_id", task_id),
                (":run_id", run_id),
                (":kind", kind),
                (":actor", actor),
                (":payload_json", payload_json),
                (":created_at", created_at),
            ),
        )
        .await?;
    Ok(())
}

async fn load_signal_connection(
    connection: &turso::Connection,
    signal_id: &str,
) -> Result<SignalRecord, StoreError> {
    let row = first_row(
        connection
            .query(
                &format!("{SIGNAL_SELECT} WHERE s.id = :signal_id LIMIT 1"),
                [(":signal_id", signal_id)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::SignalNotFound(signal_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    signal_from_row(row)
}

async fn load_signal_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    signal_id: &str,
) -> Result<SignalRecord, StoreError> {
    let row = first_row(
        transaction
            .query(
                &format!("{SIGNAL_SELECT} WHERE s.id = :signal_id LIMIT 1"),
                [(":signal_id", signal_id)],
            )
            .await?,
    )
    .await
    .map_err(|error| match error {
        turso::Error::QueryReturnedNoRows => StoreError::SignalNotFound(signal_id.to_owned()),
        other => StoreError::Turso(other),
    })?;
    signal_from_row(row)
}

async fn supersede_path_contains_tx(
    transaction: &mut turso::transaction::Transaction<'_>,
    start: &str,
    needle: &str,
) -> Result<bool, StoreError> {
    let mut seen = HashSet::new();
    let mut current = Some(start.to_owned());
    while let Some(id) = current {
        if id == needle {
            return Ok(true);
        }
        if !seen.insert(id.clone()) {
            return Ok(true);
        }
        let rows = transaction
            .query(
                "SELECT superseded_by_signal_id FROM signals WHERE id = :id LIMIT 1",
                [(":id", id.as_str())],
            )
            .await?;
        let row = match first_row(rows).await {
            Ok(row) => row,
            Err(turso::Error::QueryReturnedNoRows) => return Ok(false),
            Err(error) => return Err(StoreError::Turso(error)),
        };
        current = optional_text_value(row.get_value(0)?, "signals.superseded_by_signal_id")?;
    }
    Ok(false)
}

fn signal_from_row(row: turso::Row) -> Result<SignalRecord, StoreError> {
    let status = text_value(row.get_value(7)?, "signals.status")?;
    Ok(SignalRecord {
        id: text_value(row.get_value(0)?, "signals.id")?,
        board_id: text_value(row.get_value(1)?, "signals.board_id")?,
        observation_id: text_value(row.get_value(2)?, "signals.observation_id")?,
        kind: text_value(row.get_value(3)?, "signals.kind")?,
        title: text_value(row.get_value(4)?, "signals.title")?,
        summary: text_value(row.get_value(5)?, "signals.summary")?,
        severity: text_value(row.get_value(6)?, "signals.severity")?,
        status,
        dedupe_key: optional_text_value(row.get_value(8)?, "signals.dedupe_key")?,
        superseded_by_signal_id: optional_text_value(
            row.get_value(9)?,
            "signals.superseded_by_signal_id",
        )?,
        reviewed_by: optional_text_value(row.get_value(10)?, "signals.reviewed_by")?,
        reviewed_at: optional_integer_value(row.get_value(11)?, "signals.reviewed_at")?,
        review_reason: optional_text_value(row.get_value(12)?, "signals.review_reason")?,
        created_at: integer_value(row.get_value(13)?, "signals.created_at")?,
        updated_at: integer_value(row.get_value(14)?, "signals.updated_at")?,
        observation: SignalObservationRecord {
            id: text_value(row.get_value(15)?, "signal_observations.id")?,
            board_id: text_value(row.get_value(16)?, "signal_observations.board_id")?,
            task_id: optional_text_value(row.get_value(17)?, "signal_observations.task_id")?,
            task_ref_snapshot: optional_text_value(
                row.get_value(18)?,
                "signal_observations.task_ref_snapshot",
            )?,
            run_id: optional_text_value(row.get_value(19)?, "signal_observations.run_id")?,
            comment_id: optional_text_value(row.get_value(20)?, "signal_observations.comment_id")?,
            actor: text_value(row.get_value(21)?, "signal_observations.actor")?,
            agent_type: optional_text_value(row.get_value(22)?, "signal_observations.agent_type")?,
            source: optional_text_value(row.get_value(23)?, "signal_observations.source")?,
            evidence_json: text_value(row.get_value(24)?, "signal_observations.evidence_json")?,
            created_at: integer_value(row.get_value(25)?, "signal_observations.created_at")?,
        },
    })
}

fn map_insert_signal_error(error: turso::Error) -> StoreError {
    let message = error.to_string();
    if message.contains("UNIQUE") && message.contains("dedupe") {
        StoreError::SignalConflict("dedupe key already exists".to_owned())
    } else {
        StoreError::Turso(error)
    }
}
