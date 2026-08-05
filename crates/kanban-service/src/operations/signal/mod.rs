//! 通用产品/agent signal ledger 的共享 application service path。

use std::{collections::BTreeMap, future::Future};

use kanban_core::{Clock, KanbanError, Result, new_event_id, new_typed_id};
use serde_json::Value;

use crate::{ApplicationService, ApplicationStore, CommentRecord};

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
pub struct SignalCreateRecord {
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
pub struct SignalReviewRecord {
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

pub trait SignalLedger: ApplicationStore {
    fn record_signal(
        &self,
        input: SignalCreateRecord,
    ) -> impl Future<Output = Result<SignalRecordResult>> + Send;

    fn list_signals(
        &self,
        board: &str,
        options: SignalListOptions,
    ) -> impl Future<Output = Result<Vec<SignalRecord>>> + Send;

    fn get_signal(&self, signal_id: &str) -> impl Future<Output = Result<SignalRecord>> + Send;

    fn review_signals(
        &self,
        input: SignalReviewRecord,
    ) -> impl Future<Output = Result<Vec<SignalRecord>>> + Send;
}

impl<S, C> ApplicationService<S, C>
where
    S: SignalLedger,
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
            .record_signal(SignalCreateRecord {
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
    }

    pub async fn list_signals(
        &self,
        board: &str,
        options: SignalListOptions,
    ) -> Result<Vec<SignalRecord>> {
        let board = normalize_required(board, "board")?;
        validate_list_options(&options)?;
        self.store.list_signals(&board, options).await
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
        self.store
            .review_signals(SignalReviewRecord {
                board: command.board.map(|board| board.trim().to_owned()),
                signal_ids: command.signal_ids,
                lifecycle: command.lifecycle,
                replacement_signal_id: command.replacement_signal_id,
                actor: command.actor,
                reason: command.reason,
                event_ids,
                now: self.clock.now_ms(),
            })
            .await
    }

    pub async fn get_signal(&self, signal_id: &str) -> Result<SignalRecord> {
        let signal_id = normalize_required(signal_id, "signal id")?;
        if !signal_id.starts_with("sig_") {
            return Err(KanbanError::InvalidInput(
                "signal id must start with sig_".to_owned(),
            ));
        }
        self.store.get_signal(&signal_id).await
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

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, atomic::AtomicUsize},
    };

    use kanban_core::{KanbanError, Result};
    use serde_json::Value;

    use crate::operations::test_support::{FixedClock, StubStore};
    use crate::*;

    impl SignalLedger for StubStore {
        async fn record_signal(&self, input: SignalCreateRecord) -> Result<SignalRecordResult> {
            Ok(SignalRecordResult {
                signal: SignalRecord {
                    id: input.id,
                    board_id: "b_default".into(),
                    observation_id: input.observation_id.clone(),
                    kind: input.kind,
                    title: input.title,
                    summary: input.summary,
                    severity: input.severity,
                    status: SignalStatus::Open,
                    dedupe_key: input.dedupe_key,
                    superseded_by_signal_id: None,
                    reviewed_by: None,
                    reviewed_at: None,
                    review_reason: None,
                    created_at: input.created_at,
                    updated_at: input.created_at,
                    observation: SignalObservationRecord {
                        id: input.observation_id,
                        board_id: "b_default".into(),
                        task_id: input.task_id,
                        task_ref_snapshot: input.task_ref,
                        run_id: input.run_id,
                        comment_id: input.comment_id,
                        actor: input.actor,
                        agent_type: input.agent_type,
                        source: input.source,
                        evidence_json: input.evidence_json,
                        created_at: input.created_at,
                    },
                },
                backlink_comment: None,
            })
        }

        async fn list_signals(
            &self,
            _board: &str,
            _options: SignalListOptions,
        ) -> Result<Vec<SignalRecord>> {
            Ok(Vec::new())
        }

        async fn get_signal(&self, _signal_id: &str) -> Result<SignalRecord> {
            Err(KanbanError::NotFound("signal".into()))
        }

        async fn review_signals(&self, _input: SignalReviewRecord) -> Result<Vec<SignalRecord>> {
            Ok(Vec::new())
        }
    }

    #[tokio::test]
    async fn record_normalizes_fields_and_generates_typed_ids() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let result = service
            .record_signal(SignalRecordCommand {
                board: " default ".into(),
                kind: " failure ".into(),
                title: " Bad flag ".into(),
                summary: " Summary ".into(),
                severity: Some(" medium ".into()),
                task_ref: None,
                task_id: None,
                run_id: None,
                comment_id: None,
                actor: " codex ".into(),
                agent_type: Some(" executor ".into()),
                dedupe_key: Some(" key ".into()),
                source: Some(" test ".into()),
                evidence: BTreeMap::from([(String::from("stderr"), Value::String("bad".into()))]),
                comment_body: None,
            })
            .await
            .unwrap();
        assert!(result.signal.id.starts_with("sig_"));
        assert_eq!(result.signal.title, "Bad flag");
        assert_eq!(result.signal.observation.actor, "codex");
    }

    #[tokio::test]
    async fn review_requires_reason_and_ids() {
        let service = ApplicationService::with_clock(
            StubStore {
                calls: Arc::new(AtomicUsize::new(0)),
            },
            FixedClock(100),
        );
        let error = service
            .review_signals(SignalReviewCommand {
                board: None,
                signal_ids: Vec::new(),
                lifecycle: SignalLifecycle::Confirm,
                replacement_signal_id: None,
                actor: "codex".into(),
                reason: "ok".into(),
            })
            .await
            .expect_err("empty review should fail");
        assert!(matches!(error, KanbanError::InvalidInput(_)));
    }
}
