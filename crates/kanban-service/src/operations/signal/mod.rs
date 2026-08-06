//! 通用产品/agent signal ledger 的规范 service path。

use std::collections::BTreeMap;

use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};
use serde_json::Value;

use crate::store_operations::{
    CreateSignalInput, ReviewSignalsInput, SignalLifecycleInput, StoreSignalListOptions,
};
use crate::{CommentRecord, KanbanService};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalStatus {
    Open,
    Confirmed,
    Rejected,
    Superseded,
    Resolved,
}

impl SignalStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Confirmed => "confirmed",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
            Self::Resolved => "resolved",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalLifecycle {
    Confirm,
    Reject,
    Resolve,
    Supersede,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignalRecordCommand {
    pub board: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub severity: Option<String>,
    pub task_ref: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub comment_id: Option<String>,
    pub actor: String,
    pub agent_type: Option<String>,
    pub dedupe_key: Option<String>,
    pub source: Option<String>,
    pub evidence: BTreeMap<String, Value>,
    pub comment_body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReviewCommand {
    pub board: Option<String>,
    pub signal_ids: Vec<String>,
    pub lifecycle: SignalLifecycle,
    pub replacement_signal_id: Option<String>,
    pub actor: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalObservationRecord {
    pub id: String,
    pub board_id: String,
    pub task_id: Option<String>,
    pub task_ref_snapshot: Option<String>,
    pub run_id: Option<String>,
    pub comment_id: Option<String>,
    pub actor: String,
    pub agent_type: Option<String>,
    pub source: Option<String>,
    pub evidence_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRecord {
    pub id: String,
    pub board_id: String,
    pub observation_id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub severity: String,
    pub status: SignalStatus,
    pub dedupe_key: Option<String>,
    pub superseded_by_signal_id: Option<String>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<i64>,
    pub review_reason: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub observation: SignalObservationRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRecordResult {
    pub signal: SignalRecord,
    pub backlink_comment: Option<CommentRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalListOptions {
    pub statuses: Vec<SignalStatus>,
    pub kinds: Vec<String>,
    pub task_ref: Option<String>,
    pub include_all: bool,
    pub limit: usize,
}

impl Default for SignalListOptions {
    fn default() -> Self {
        Self {
            statuses: Vec::new(),
            kinds: Vec::new(),
            task_ref: None,
            include_all: false,
            limit: 100,
        }
    }
}

impl<C> KanbanService<C>
where
    C: Clock,
{
    pub async fn record_signal(
        &self,
        mut command: SignalRecordCommand,
    ) -> Result<SignalRecordResult> {
        let board = normalize_required(&command.board, "board")?;
        command.board = board.clone();
        command.kind = normalize_required(&command.kind, "signal kind")?;
        command.title = normalize_required(&command.title, "signal title")?;
        command.summary = normalize_required(&command.summary, "signal summary")?;
        command.actor = normalize_required(&command.actor, "actor")?;
        command.severity = Some(
            command
                .severity
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("info")
                .to_owned(),
        );
        command.agent_type = normalize_optional(command.agent_type);
        command.dedupe_key = normalize_optional(command.dedupe_key);
        command.source = normalize_optional(command.source);
        command.task_ref = normalize_optional(command.task_ref);
        command.task_id = normalize_optional(command.task_id);
        command.run_id = normalize_optional(command.run_id);
        command.comment_id = normalize_optional(command.comment_id);
        command.comment_body = normalize_optional(command.comment_body);
        let evidence_json = serde_json::to_string(&command.evidence)
            .map_err(|error| KanbanError::InvalidInput(format!("invalid evidence: {error}")))?;
        let _mutation = self.mutation_gate.lock().await;
        self.store
            .record_signal(CreateSignalInput {
                id: new_typed_id("sig"),
                observation_id: new_typed_id("obs"),
                event_id: new_event_id(),
                board,
                kind: command.kind,
                title: command.title,
                summary: command.summary,
                severity: command.severity.expect("normalized severity"),
                task_ref: command.task_ref,
                task_id: command.task_id,
                run_id: command.run_id,
                comment_id: command.comment_id,
                actor: command.actor,
                agent_type: command.agent_type,
                dedupe_key: command.dedupe_key,
                source: command.source,
                evidence_json,
                comment_body: command.comment_body,
                created_at: self.clock.now_ms(),
            })
            .await
            .map_err(crate::error::store_error)
            .and_then(application_signal_result)
    }

    pub async fn list_signals(
        &self,
        board: &str,
        options: SignalListOptions,
    ) -> Result<Vec<SignalRecord>> {
        let board = normalize_required(board, "board")?;
        validate_list_options(&options)?;
        self.store
            .list_signals(
                &board,
                StoreSignalListOptions {
                    statuses: options
                        .statuses
                        .into_iter()
                        .map(|status| status.as_str().to_owned())
                        .collect(),
                    kinds: options.kinds,
                    task_ref: options.task_ref,
                    include_all: options.include_all,
                    limit: options.limit,
                },
            )
            .await
            .map_err(crate::error::store_error)?
            .into_iter()
            .map(application_signal)
            .collect()
    }

    pub async fn review_signals(
        &self,
        mut command: SignalReviewCommand,
    ) -> Result<Vec<SignalRecord>> {
        if command.signal_ids.is_empty() {
            return Err(KanbanError::InvalidInput(
                "at least one signal id is required".to_owned(),
            ));
        }
        for signal_id in &mut command.signal_ids {
            *signal_id = normalize_required(signal_id, "signal id")?;
            if !signal_id.starts_with("sig_") {
                return Err(KanbanError::InvalidInput(
                    "signal id must start with sig_".to_owned(),
                ));
            }
        }
        command.actor = normalize_required(&command.actor, "actor")?;
        command.reason = normalize_required(&command.reason, "reason")?;
        command.replacement_signal_id = normalize_optional(command.replacement_signal_id);
        if matches!(command.lifecycle, SignalLifecycle::Supersede)
            && command.replacement_signal_id.is_none()
        {
            return Err(KanbanError::InvalidInput(
                "supersede requires replacement signal id".to_owned(),
            ));
        }
        if command.replacement_signal_id.is_some()
            && !matches!(command.lifecycle, SignalLifecycle::Supersede)
        {
            return Err(KanbanError::InvalidInput(
                "replacement signal id is only valid for supersede".to_owned(),
            ));
        }
        let event_ids = command.signal_ids.iter().map(|_| new_event_id()).collect();
        let _mutation = self.mutation_gate.lock().await;
        let lifecycle = match command.lifecycle {
            SignalLifecycle::Confirm => SignalLifecycleInput::Confirm,
            SignalLifecycle::Reject => SignalLifecycleInput::Reject,
            SignalLifecycle::Resolve => SignalLifecycleInput::Resolve,
            SignalLifecycle::Supersede => SignalLifecycleInput::Supersede,
        };
        self.store
            .review_signals(ReviewSignalsInput {
                board: command.board.map(|board| board.trim().to_owned()),
                signal_ids: command.signal_ids,
                lifecycle,
                replacement_signal_id: command.replacement_signal_id,
                actor: command.actor,
                reason: command.reason,
                event_ids,
                now: self.clock.now_ms(),
            })
            .await
            .map_err(crate::error::store_error)?
            .into_iter()
            .map(application_signal)
            .collect()
    }

    pub async fn get_signal(&self, signal_id: &str) -> Result<SignalRecord> {
        let signal_id = normalize_required(signal_id, "signal id")?;
        if !signal_id.starts_with("sig_") {
            return Err(KanbanError::InvalidInput(
                "signal id must start with sig_".to_owned(),
            ));
        }
        self.store
            .get_signal(&signal_id)
            .await
            .map_err(crate::error::store_error)
            .and_then(application_signal)
    }
}

fn normalize_required(value: &str, field: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(KanbanError::InvalidInput(format!("{field} is required")));
    }
    Ok(value.to_owned())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn validate_list_options(options: &SignalListOptions) -> Result<()> {
    if options.limit == 0 || options.limit > 1000 {
        return Err(KanbanError::InvalidInput(
            "signal list limit must be between 1 and 1000".to_owned(),
        ));
    }
    if options.kinds.iter().any(|kind| kind.trim().is_empty()) {
        return Err(KanbanError::InvalidInput(
            "signal kind filters must not be empty".to_owned(),
        ));
    }
    Ok(())
}

fn application_signal_status(status: String) -> Result<SignalStatus> {
    match status.as_str() {
        "open" => Ok(SignalStatus::Open),
        "confirmed" => Ok(SignalStatus::Confirmed),
        "rejected" => Ok(SignalStatus::Rejected),
        "superseded" => Ok(SignalStatus::Superseded),
        "resolved" => Ok(SignalStatus::Resolved),
        other => Err(KanbanError::Storage(format!(
            "stored signal status is invalid: {other}"
        ))),
    }
}

fn application_signal(signal: crate::domain::SignalRecord) -> Result<SignalRecord> {
    Ok(SignalRecord {
        id: signal.id,
        board_id: signal.board_id,
        observation_id: signal.observation_id,
        kind: signal.kind,
        title: signal.title,
        summary: signal.summary,
        severity: signal.severity,
        status: application_signal_status(signal.status)?,
        dedupe_key: signal.dedupe_key,
        superseded_by_signal_id: signal.superseded_by_signal_id,
        reviewed_by: signal.reviewed_by,
        reviewed_at: signal.reviewed_at,
        review_reason: signal.review_reason,
        created_at: signal.created_at,
        updated_at: signal.updated_at,
        observation: SignalObservationRecord {
            id: signal.observation.id,
            board_id: signal.observation.board_id,
            task_id: signal.observation.task_id,
            task_ref_snapshot: signal.observation.task_ref_snapshot,
            run_id: signal.observation.run_id,
            comment_id: signal.observation.comment_id,
            actor: signal.observation.actor,
            agent_type: signal.observation.agent_type,
            source: signal.observation.source,
            evidence_json: signal.observation.evidence_json,
            created_at: signal.observation.created_at,
        },
    })
}

fn application_signal_result(
    result: crate::domain::SignalRecordResult,
) -> Result<SignalRecordResult> {
    Ok(SignalRecordResult {
        signal: application_signal(result.signal)?,
        backlink_comment: result
            .backlink_comment
            .map(crate::operations::application_comment)
            .transpose()?,
    })
}
