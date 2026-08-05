use std::future::Future;

use kanban_core::{Clock, KanbanError, Result};
use serde_json::{Value, json};

use crate::{
    ApplicationService, ApplicationStore, CommentList, CommentRecord, DependencyList,
    DependencySnapshotRecord, EventList, EventListOptions, EventRecord, LabelOntologyOperations,
    RunList, RunRecord, StepList, StepRecord, TaskRecord,
};

/// task show 的可选聚合读取结果。各集合均来自 canonical host，adapter 不负责拼接业务语义。
#[derive(Debug, Clone, PartialEq)]
pub struct TaskDetailRecord {
    pub task: TaskRecord,
    pub labels: Vec<crate::LabelRecord>,
    pub dependencies: DependencySnapshotRecord,
    pub execution_plan: crate::ExecutionPlanRecord,
    pub steps: Vec<StepRecord>,
    pub comments: Vec<CommentRecord>,
    pub runs: Vec<RunRecord>,
    pub events: Vec<EventRecord>,
    pub ontology: TaskDetailOntologyRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskDetailOntologyRecord {
    /// canonical ontology 摘要；provider 能力状态由同级元数据表达。
    pub summary: Option<TaskOntologySummaryRecord>,
    pub degraded: bool,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskOntologySummaryRecord {
    pub task_id: String,
    pub observation_count: i64,
    pub signal_count: i64,
    pub open_count: i64,
    pub confirmed_count: i64,
    pub resolved_count: i64,
    pub rejected_count: i64,
    pub superseded_count: i64,
    pub degraded_count: i64,
    pub stale_count: i64,
    pub suggest_input_drift_count: i64,
    pub legacy_incomparable_count: i64,
    pub incomparable_count: i64,
    pub action_count: i64,
    pub oldest_open_confirmed_signal_at: Option<i64>,
    pub oldest_open_confirmed_signal_age_ms: Option<i64>,
    pub latest_signal_at: Option<i64>,
    pub latest_action_at: Option<i64>,
    pub current_suggest_input_hash: String,
    pub sample_signals: Vec<TaskOntologySignalSummaryRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskOntologySignalSummaryRecord {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub proposed_action: String,
    pub target_label_id: Option<String>,
    pub target_label_name: Option<String>,
    pub candidate_atom_polarity: Option<String>,
    pub candidate_atom_kind: Option<String>,
    pub candidate_text: Option<String>,
    pub candidate_content_hash: Option<String>,
    pub proposed_label_name: Option<String>,
    pub proposed_label_name_normalized: Option<String>,
    pub suggest_score: Option<f64>,
    pub suggest_rank: Option<i64>,
    pub degraded: bool,
    pub stale: bool,
    pub legacy_incomparable: bool,
    pub suggest_input_drift: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub latest_action_at: Option<i64>,
    pub action_count: i64,
}

/// 由 application service 聚合 task 的 canonical 相关记录。
pub trait TaskDetailRead: ApplicationStore {
    fn get_task_detail_parts(
        &self,
        task_id: &str,
    ) -> impl Future<Output = Result<TaskDetailRecord>> + Send;
}

impl<S> TaskDetailRead for S
where
    S: ApplicationStore
        + crate::TaskShow
        + DependencyList
        + StepList
        + CommentList
        + RunList
        + EventList
        + LabelOntologyOperations,
{
    async fn get_task_detail_parts(&self, task_id: &str) -> Result<TaskDetailRecord> {
        let task = self.get_task(task_id).await?;
        let task_id = task.id.clone();
        let board = task.board_slug.clone();
        let labels = task.labels.clone();
        let dependencies = self.list_dependencies(&task_id).await?;
        let task_steps = self.list_steps(&task_id).await?;
        let comments = self.list_comments(&task_id).await?;
        let runs = self.list_runs(&task_id).await?;
        let mut events = self
            .list_events(
                &board,
                EventListOptions {
                    task_id: Some(task_id.clone()),
                    after: 0,
                    limit: 1_000,
                },
            )
            .await?
            .events;
        if events.len() > 100 {
            let start = events.len() - 100;
            events.drain(..start);
        }
        let ontology = ontology_summary(self, &board, &task.id, &task.task_ref).await;
        Ok(TaskDetailRecord {
            task,
            labels,
            dependencies,
            execution_plan: task_steps.execution_plan,
            steps: task_steps.steps,
            comments,
            runs,
            events,
            ontology,
        })
    }
}

async fn ontology_summary<S>(
    store: &S,
    board: &str,
    task_id: &str,
    task_ref: &str,
) -> TaskDetailOntologyRecord
where
    S: ApplicationStore + LabelOntologyOperations,
{
    let signals = store
        .label_ontology(
            "list_signals",
            board,
            json!({
                "task_ref": task_ref,
                "include_all": true,
                "limit": 100,
            }),
        )
        .await;
    let index = store.label_ontology("index_status", board, json!({})).await;

    let mut diagnostics = Vec::new();
    let mut degraded = false;
    let mut summary = None;
    match signals {
        Ok(value) => {
            summary = summarize_ontology_signals(task_id, value);
        }
        Err(error) => {
            degraded = true;
            diagnostics.push(format!("ontology_signals_unavailable: {error}"));
        }
    }
    match index {
        Ok(value) => {
            if value.get("enabled").and_then(Value::as_bool) == Some(false) {
                degraded = true;
            }
            if let Some(values) = value.get("diagnostics").and_then(Value::as_array) {
                diagnostics.extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
            }
        }
        Err(error) => {
            degraded = true;
            diagnostics.push(format!("ontology_index_unavailable: {error}"));
        }
    }
    diagnostics.sort();
    diagnostics.dedup();
    TaskDetailOntologyRecord {
        summary,
        degraded,
        diagnostics,
    }
}

fn summarize_ontology_signals(task_id: &str, value: Value) -> Option<TaskOntologySummaryRecord> {
    let Value::Array(signals) = value else {
        return None;
    };
    if signals.is_empty() {
        return None;
    }
    let mut observation_ids = std::collections::BTreeSet::new();
    let mut counts = std::collections::BTreeMap::<&str, i64>::new();
    let mut oldest_open_confirmed = None;
    let mut latest_signal = None;
    let mut samples = Vec::new();
    let mut signal_count = 0;
    let mut degraded_count = 0;
    let mut stale_count = 0;
    let mut suggest_input_drift_count = 0;
    let mut legacy_incomparable_count = 0;
    let mut action_count = 0;
    let mut latest_action_at: Option<i64> = None;
    for signal in &signals {
        let Some(object) = signal.as_object() else {
            continue;
        };
        signal_count += 1;
        if let Some(observation_id) = object.get("observation_id").and_then(Value::as_str) {
            observation_ids.insert(observation_id.to_owned());
        }
        let status = object
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("open");
        *counts.entry(status).or_default() += 1;
        let created_at = object
            .get("created_at")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        latest_signal = Some(latest_signal.map_or(created_at, |value: i64| value.max(created_at)));
        if matches!(status, "open" | "confirmed") {
            oldest_open_confirmed =
                Some(oldest_open_confirmed.map_or(created_at, |value: i64| value.min(created_at)));
        }
        let degraded = object
            .get("degraded")
            .or_else(|| object.get("suggest_degraded"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let stale = object
            .get("stale")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let legacy_incomparable = object
            .get("legacy_incomparable")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let suggest_input_drift = object
            .get("suggest_input_drift")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let signal_action_count = object
            .get("action_count")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let signal_latest_action_at = object.get("latest_action_at").and_then(Value::as_i64);
        degraded_count += i64::from(degraded);
        stale_count += i64::from(stale);
        suggest_input_drift_count += i64::from(suggest_input_drift);
        legacy_incomparable_count += i64::from(legacy_incomparable);
        action_count += signal_action_count;
        if let Some(value) = signal_latest_action_at {
            latest_action_at = Some(latest_action_at.map_or(value, |current| current.max(value)));
        }
        samples.push(TaskOntologySignalSummaryRecord {
            id: string_field(object, "id"),
            kind: string_field(object, "kind"),
            status: status.to_owned(),
            proposed_action: string_field(object, "proposed_action"),
            target_label_id: optional_string_field(object, "target_label_id"),
            target_label_name: optional_string_field(object, "target_label_name_snapshot"),
            candidate_atom_polarity: optional_string_field(object, "candidate_atom_polarity"),
            candidate_atom_kind: optional_string_field(object, "candidate_atom_kind"),
            candidate_text: optional_string_field(object, "candidate_text"),
            candidate_content_hash: optional_string_field(object, "candidate_content_hash"),
            proposed_label_name: optional_string_field(object, "proposed_label_name"),
            proposed_label_name_normalized: optional_string_field(
                object,
                "proposed_label_name_normalized",
            ),
            suggest_score: object.get("suggest_score").and_then(Value::as_f64),
            suggest_rank: object.get("suggest_rank").and_then(Value::as_i64),
            degraded,
            stale,
            legacy_incomparable,
            suggest_input_drift,
            created_at,
            updated_at: object
                .get("updated_at")
                .and_then(Value::as_i64)
                .unwrap_or(created_at),
            latest_action_at: signal_latest_action_at,
            action_count: signal_action_count,
        });
    }
    if signal_count == 0 {
        return None;
    }
    samples.sort_by(|left, right| {
        signal_status_priority(&left.status)
            .cmp(&signal_status_priority(&right.status))
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| left.id.cmp(&right.id))
    });
    samples.truncate(5);
    Some(TaskOntologySummaryRecord {
        task_id: task_id.to_owned(),
        observation_count: observation_ids.len() as i64,
        signal_count,
        open_count: counts.get("open").copied().unwrap_or_default(),
        confirmed_count: counts.get("confirmed").copied().unwrap_or_default(),
        resolved_count: counts.get("resolved").copied().unwrap_or_default(),
        rejected_count: counts.get("rejected").copied().unwrap_or_default(),
        superseded_count: counts.get("superseded").copied().unwrap_or_default(),
        degraded_count,
        stale_count,
        suggest_input_drift_count,
        legacy_incomparable_count,
        incomparable_count: stale_count + degraded_count,
        action_count,
        oldest_open_confirmed_signal_at: oldest_open_confirmed,
        oldest_open_confirmed_signal_age_ms: None,
        latest_signal_at: latest_signal,
        latest_action_at,
        current_suggest_input_hash: String::new(),
        sample_signals: samples,
    })
}

fn string_field(object: &serde_json::Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn optional_string_field(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn signal_status_priority(status: &str) -> u8 {
    match status {
        "open" => 0,
        "confirmed" => 1,
        "resolved" => 2,
        "rejected" => 3,
        "superseded" => 4,
        _ => 5,
    }
}

impl<S, C> ApplicationService<S, C>
where
    S: TaskDetailRead,
    C: Clock,
{
    pub async fn get_task_details(&self, task_id: &str) -> Result<TaskDetailRecord> {
        let task_id = task_id.trim();
        if !task_id.starts_with("t_") || task_id.len() <= 2 {
            return Err(KanbanError::InvalidInput(
                "task_id must be a global t_... id".to_owned(),
            ));
        }
        self.store.get_task_detail_parts(task_id).await
    }
}
