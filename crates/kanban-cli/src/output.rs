use std::path::Path;

use anyhow::Result;
use kanban_contract::{
    ApiClaim, ApiComment, ApiExecutionPlan, ApiExecutionPlanState, ApiLabel, ApiRun, ApiRunStatus,
    ApiStepStatus, ApiTask, ApiTaskPriority, ApiTaskStatus, ApiTaskStep, ApiTaskSteps,
    CliDependencyEdge, CliDependencyMutation, CliDependencySnapshot, CliDependencyTask, CliEvent,
    CliTaskShowOutput, CommentAuthorType, CommentKind, TaskOntologyDetails,
    TaskOntologyDetailsMeta, TaskOntologySummary,
};
use kanban_core::{KanbanError, TaskStatus};
use kanban_sqlite::api::{
    ClaimResult, CommentRecord, DependencyEdgeRecord, DependencyMutation, DependencySnapshot,
    DependencyTaskRecord, EventRecord, LabelRecord, RunRecord, StepPlanState,
    TaskExecutionPlanRecord, TaskRecord, TaskStepRecord, get_run_by_id_global,
};

use crate::args::SearchOutputHit;

fn parse_persisted_json<T: serde::de::DeserializeOwned>(
    raw: &str,
    owner: impl std::fmt::Display,
) -> Result<T> {
    serde_json::from_str(raw).map_err(|error| {
        KanbanError::Storage(format!("{owner} contains invalid persisted JSON: {error}")).into()
    })
}

pub(crate) fn print_contract_or_human<T: serde::Serialize>(
    json: bool,
    output: &T,
    human: impl FnOnce() -> String,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(output)?);
    } else {
        println!("{}", human());
    }
    Ok(())
}

pub(crate) fn print_task(json: bool, task: &kanban_sqlite::api::TaskRecord) -> Result<()> {
    print_task_with_details(json, false, task, None)
}

pub(crate) fn print_task_with_details(
    json: bool,
    details: bool,
    task: &kanban_sqlite::api::TaskRecord,
    ontology_summary: Option<&kanban_sqlite::api::TaskOntologySummary>,
) -> Result<()> {
    if json {
        let task = api_task_from_record(task)?;
        let meta = if details {
            let ontology_summary = ontology_summary
                .map(|summary| {
                    serde_json::from_value::<TaskOntologySummary>(serde_json::to_value(summary)?)
                })
                .transpose()?;
            Some(TaskOntologyDetailsMeta {
                details: TaskOntologyDetails { ontology_summary },
            })
        } else {
            None
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&CliTaskShowOutput::new(task, meta))?
        );
        return Ok(());
    }
    print_human(|| {
        if details {
            task_details(task, ontology_summary)
        } else {
            task_line(task)
        }
    })
}

pub(crate) fn api_task_from_record(task: &TaskRecord) -> Result<ApiTask> {
    Ok(ApiTask {
        id: task.id.clone(),
        board_id: task.board_id.clone(),
        board_slug: task.board_slug.clone(),
        task_ref: task.task_ref.clone(),
        seq: task.seq,
        title: task.title.clone(),
        description: task.description.clone(),
        status: match task.status {
            TaskStatus::Triage => ApiTaskStatus::Triage,
            TaskStatus::Todo => ApiTaskStatus::Todo,
            TaskStatus::Scheduled => ApiTaskStatus::Scheduled,
            TaskStatus::Ready => ApiTaskStatus::Ready,
            TaskStatus::Running => ApiTaskStatus::Running,
            TaskStatus::Blocked => ApiTaskStatus::Blocked,
            TaskStatus::Review => ApiTaskStatus::Review,
            TaskStatus::Done => ApiTaskStatus::Done,
            TaskStatus::Archived => ApiTaskStatus::Archived,
        },
        status_reason: task.status_reason.clone(),
        assignee: task.assignee.clone(),
        priority: ApiTaskPriority::try_from(task.priority).map_err(|priority| {
            anyhow::anyhow!("task {} has invalid priority {priority}", task.id)
        })?,
        position: task.position,
        scheduled_at: task.scheduled_at,
        due_at: task.due_at,
        created_by: task.created_by.clone(),
        created_at: task.created_at,
        updated_at: task.updated_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
        archived_at: task.archived_at,
        claim_owner: task.claim_owner.clone(),
        claim_expires_at: task.claim_expires_at,
        last_heartbeat_at: task.last_heartbeat_at,
        current_run_id: task.current_run_id.clone(),
        retry_count: task.retry_count,
        max_retries: task.max_retries,
        result_summary: task.result_summary.clone(),
        result: task
            .result_json
            .as_deref()
            .map(|raw| parse_persisted_json(raw, format_args!("task {} result_json", task.id)))
            .transpose()?,
        metadata: parse_persisted_json(
            &task.metadata_json,
            format_args!("task {} metadata_json", task.id),
        )?,
        lock_version: task.lock_version,
        dependency_blocked: task.dependency_blocked,
        unfinished_parent_count: task.unfinished_parent_count,
        execution_plan_state: match task.execution_plan_state {
            StepPlanState::Unplanned => ApiExecutionPlanState::Unplanned,
            StepPlanState::Planned => ApiExecutionPlanState::Planned,
            StepPlanState::NotRequired => ApiExecutionPlanState::NotRequired,
        },
        required_step_count: task.required_step_count,
        completed_required_step_count: task.completed_required_step_count,
        optional_step_count: task.optional_step_count,
        labels: task.labels.iter().map(api_label_from_record).collect(),
    })
}

pub(crate) fn api_task_steps_from_records(
    task_id: String,
    execution_plan: TaskExecutionPlanRecord,
    steps: &[TaskStepRecord],
) -> Result<ApiTaskSteps> {
    Ok(ApiTaskSteps {
        task_id,
        execution_plan: api_execution_plan_from_record(&execution_plan),
        steps: steps
            .iter()
            .map(api_task_step_from_record)
            .collect::<Result<Vec<_>>>()?,
    })
}

pub(crate) fn api_execution_plan_from_record(
    execution_plan: &TaskExecutionPlanRecord,
) -> ApiExecutionPlan {
    ApiExecutionPlan {
        board_id: execution_plan.board_id.clone(),
        task_id: execution_plan.task_id.clone(),
        state: match execution_plan.state {
            StepPlanState::Unplanned => ApiExecutionPlanState::Unplanned,
            StepPlanState::Planned => ApiExecutionPlanState::Planned,
            StepPlanState::NotRequired => ApiExecutionPlanState::NotRequired,
        },
        reason: execution_plan.reason.clone(),
        updated_by: execution_plan.updated_by.clone(),
        updated_at: execution_plan.updated_at,
    }
}

pub(crate) fn api_task_step_from_record(step: &TaskStepRecord) -> Result<ApiTaskStep> {
    Ok(ApiTaskStep {
        id: step.id.clone(),
        parent_task_id: step.parent_task_id.clone(),
        title: step.title.clone(),
        body: step.body.clone(),
        linked_task: step
            .linked_task
            .as_ref()
            .map(api_task_from_record)
            .transpose()?,
        position: step.position,
        required: step.required,
        status: match step.status {
            kanban_sqlite::api::StepStatus::Todo => ApiStepStatus::Todo,
            kanban_sqlite::api::StepStatus::Done => ApiStepStatus::Done,
            kanban_sqlite::api::StepStatus::Skipped => ApiStepStatus::Skipped,
        },
        resolution_note: step.resolution_note.clone(),
        resolved_by: step.resolved_by.clone(),
        resolved_at: step.resolved_at,
        created_by: step.created_by.clone(),
        created_at: step.created_at,
        updated_by: step.updated_by.clone(),
        updated_at: step.updated_at,
    })
}

pub(crate) fn api_run_from_record(run: &RunRecord) -> Result<ApiRun> {
    Ok(ApiRun {
        id: run.id.clone(),
        task_id: run.task_id.clone(),
        status: ApiRunStatus::try_from(run.status.as_str())
            .map_err(|error| anyhow::anyhow!(error))?,
        worker_profile: run.worker_profile.clone(),
        worker_pid: run.worker_pid,
        claim_owner: run.claim_owner.clone(),
        started_at: run.started_at,
        finished_at: run.finished_at,
        exit_code: run.exit_code,
        summary: run.summary.clone(),
        error: run.error.clone(),
        has_log: run.log_path.is_some(),
        metadata: parse_persisted_json(
            &run.metadata_json,
            format_args!("run {} metadata_json", run.id),
        )?,
    })
}

pub(crate) fn api_claim_from_result(db_path: &Path, claim: &ClaimResult) -> Result<ApiClaim> {
    let run = get_run_by_id_global(db_path, &claim.run_id)?;
    Ok(ApiClaim {
        task: api_task_from_record(&claim.task)?,
        run: api_run_from_record(&run)?,
        claim_token: claim.claim_token.clone(),
        claim_expires_at: claim.task.claim_expires_at,
    })
}

fn api_label_from_record(label: &LabelRecord) -> ApiLabel {
    ApiLabel {
        id: label.id.clone(),
        board_id: label.board_id.clone(),
        name: label.name.clone(),
        color: label.color.clone(),
        created_at: label.created_at,
        updated_at: label.updated_at,
    }
}

pub(crate) fn api_comment_from_record(comment: &CommentRecord) -> Result<ApiComment> {
    let author_type = match comment.author_type.as_str() {
        "user" => CommentAuthorType::User,
        "agent" => CommentAuthorType::Agent,
        value => anyhow::bail!("comment {} has invalid author_type {value}", comment.id),
    };
    let kind = match comment.kind.as_str() {
        "note" => CommentKind::Note,
        "decision" => CommentKind::Decision,
        "signal" => CommentKind::Signal,
        value => anyhow::bail!("comment {} has invalid kind {value}", comment.id),
    };
    Ok(ApiComment {
        id: comment.id.clone(),
        board_id: comment.board_id.clone(),
        task_id: comment.task_id.clone(),
        author: comment.author.clone(),
        author_type,
        agent_type: comment.agent_type.clone(),
        body: comment.body.clone(),
        kind,
        metadata: parse_persisted_json(
            &comment.metadata_json,
            format_args!("comment {} metadata_json", comment.id),
        )?,
        created_at: comment.created_at,
    })
}

pub(crate) fn cli_dependency_snapshot(snapshot: &DependencySnapshot) -> CliDependencySnapshot {
    CliDependencySnapshot {
        task: cli_dependency_task(&snapshot.task),
        parents: snapshot.parents.iter().map(cli_dependency_task).collect(),
        children: snapshot.children.iter().map(cli_dependency_task).collect(),
        edges: snapshot.edges.iter().map(cli_dependency_edge).collect(),
    }
}

pub(crate) fn cli_dependency_mutation(mutation: &DependencyMutation) -> CliDependencyMutation {
    CliDependencyMutation {
        edge: cli_dependency_edge(&mutation.edge),
        dependencies: cli_dependency_snapshot(&mutation.dependencies),
    }
}

fn cli_dependency_task(task: &DependencyTaskRecord) -> CliDependencyTask {
    CliDependencyTask {
        id: task.id.clone(),
        board_id: task.board_id.clone(),
        board_slug: task.board_slug.clone(),
        task_ref: task.task_ref.clone(),
        title: task.title.clone(),
        status: match task.status {
            TaskStatus::Triage => ApiTaskStatus::Triage,
            TaskStatus::Todo => ApiTaskStatus::Todo,
            TaskStatus::Scheduled => ApiTaskStatus::Scheduled,
            TaskStatus::Ready => ApiTaskStatus::Ready,
            TaskStatus::Running => ApiTaskStatus::Running,
            TaskStatus::Blocked => ApiTaskStatus::Blocked,
            TaskStatus::Review => ApiTaskStatus::Review,
            TaskStatus::Done => ApiTaskStatus::Done,
            TaskStatus::Archived => ApiTaskStatus::Archived,
        },
    }
}

fn cli_dependency_edge(edge: &DependencyEdgeRecord) -> CliDependencyEdge {
    CliDependencyEdge {
        parent: cli_dependency_task(&edge.parent),
        child: cli_dependency_task(&edge.child),
    }
}

pub(crate) fn cli_event_from_record(event: &EventRecord) -> Result<CliEvent> {
    Ok(CliEvent {
        id: event.id,
        event_id: event.event_id.clone(),
        task_id: event.task_id.clone(),
        run_id: event.run_id.clone(),
        kind: event.kind.clone(),
        actor: event.actor.clone(),
        payload: parse_persisted_json(
            &event.payload_json,
            format_args!("event {} payload_json", event.event_id),
        )?,
        created_at: event.created_at,
    })
}

pub(crate) fn print_human(human: impl FnOnce() -> String) -> Result<()> {
    println!("{}", human());
    Ok(())
}

pub(crate) fn task_line(task: &kanban_sqlite::api::TaskRecord) -> String {
    let labels = task_label_suffix(task);
    format!(
        "{} [{}] P{} {}{} · plan: {} · steps: {}/{}",
        task.task_ref,
        task.status.as_str(),
        task.priority,
        task.title,
        labels,
        task.execution_plan_state.as_str(),
        task.completed_required_step_count,
        task.required_step_count,
    )
}

pub(crate) fn task_details(
    task: &kanban_sqlite::api::TaskRecord,
    ontology_summary: Option<&kanban_sqlite::api::TaskOntologySummary>,
) -> String {
    let mut lines = Vec::new();
    lines.extend([
        "Task".to_owned(),
        format!("  ref: {}", task.task_ref),
        format!("  id: {}", task.id),
        format!("  board_slug: {}", task.board_slug),
        format!("  board_id: {}", task.board_id),
        format!("  seq: {}", task.seq),
        format!("  status: {}", task.status.as_str()),
        format!(
            "  status_reason: {}",
            option_display(task.status_reason.as_deref())
        ),
        format!("  title: {}", task.title),
        format!("  labels: {}", task_labels_display(task)),
        format!("  assignee: {}", option_display(task.assignee.as_deref())),
        format!("  priority: P{}", task.priority),
        "Description".to_owned(),
    ]);
    push_indented_multiline(&mut lines, task.description.as_deref());
    lines.extend([
        "Plan".to_owned(),
        format!("  state: {}", task.execution_plan_state.as_str()),
        format!(
            "  required_steps: {}/{}",
            task.completed_required_step_count, task.required_step_count
        ),
        format!("  optional_steps: {}", task.optional_step_count),
        format!("  position: {}", task.position),
        "Schedule".to_owned(),
        format!("  scheduled_at: {}", option_i64(task.scheduled_at)),
        format!("  due_at: {}", option_i64(task.due_at)),
        "Timestamps".to_owned(),
        format!("  created_by: {}", task.created_by),
        format!("  created_at: {}", task.created_at),
        format!("  updated_at: {}", task.updated_at),
        format!("  started_at: {}", option_i64(task.started_at)),
        format!("  completed_at: {}", option_i64(task.completed_at)),
        format!("  archived_at: {}", option_i64(task.archived_at)),
        "Execution".to_owned(),
        format!(
            "  claim_owner: {}",
            option_display(task.claim_owner.as_deref())
        ),
        format!(
            "  claim_token: {}",
            option_display(task.claim_token.as_deref())
        ),
        format!("  claim_expires_at: {}", option_i64(task.claim_expires_at)),
        format!(
            "  last_heartbeat_at: {}",
            option_i64(task.last_heartbeat_at)
        ),
        format!(
            "  current_run_id: {}",
            option_display(task.current_run_id.as_deref())
        ),
        format!("  retry_count: {}", task.retry_count),
        format!("  max_retries: {}", option_i64(task.max_retries)),
        "Result".to_owned(),
        format!(
            "  result_summary: {}",
            option_display(task.result_summary.as_deref())
        ),
        format!(
            "  result_json: {}",
            option_display(task.result_json.as_deref())
        ),
        "Metadata".to_owned(),
        format!("  metadata_json: {}", task.metadata_json),
        format!("  lock_version: {}", task.lock_version),
    ]);
    if let Some(summary) = ontology_summary {
        push_task_ontology_summary(&mut lines, summary);
    }
    lines.join("\n")
}

pub(crate) fn label_line(label: &kanban_sqlite::api::LabelRecord) -> String {
    let color = label.color.as_deref().unwrap_or("-");
    format!("{} {} color={}", label.name, label.id, color)
}

fn task_label_suffix(task: &kanban_sqlite::api::TaskRecord) -> String {
    if task.labels.is_empty() {
        String::new()
    } else {
        format!(
            " [{}]",
            task.labels
                .iter()
                .map(|label| label.name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

fn task_labels_display(task: &kanban_sqlite::api::TaskRecord) -> String {
    if task.labels.is_empty() {
        "-".to_owned()
    } else {
        task.labels
            .iter()
            .map(|label| label.name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn push_indented_multiline(lines: &mut Vec<String>, value: Option<&str>) {
    match value {
        Some(value) if !value.is_empty() => {
            lines.extend(value.lines().map(|line| format!("  {line}")));
        }
        _ => lines.push("  -".to_owned()),
    }
}

fn option_display(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn option_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn push_task_ontology_summary(
    lines: &mut Vec<String>,
    summary: &kanban_sqlite::api::TaskOntologySummary,
) {
    lines.extend([
        "ontology_summary:".to_owned(),
        format!(
            "  signals: total={} open={} confirmed={} resolved={} rejected={} superseded={} degraded={} stale={} incomparable={} actions={}",
            summary.signal_count,
            summary.open_count,
            summary.confirmed_count,
            summary.resolved_count,
            summary.rejected_count,
            summary.superseded_count,
            summary.degraded_count,
            summary.stale_count,
            summary.incomparable_count,
            summary.action_count
        ),
        format!(
            "  oldest_open_confirmed_signal_at: {}",
            option_i64(summary.oldest_open_confirmed_signal_at)
        ),
        format!(
            "  latest_signal_at: {}",
            option_i64(summary.latest_signal_at)
        ),
        format!(
            "  latest_action_at: {}",
            option_i64(summary.latest_action_at)
        ),
    ]);
    for signal in &summary.sample_signals {
        lines.push(format!(
            "  signal {} kind={} status={} action={} degraded={} stale={} actions={}",
            signal.id,
            signal.kind,
            signal.status,
            signal.proposed_action,
            signal.degraded,
            signal.stale,
            signal.action_count
        ));
    }
}

pub(crate) fn search_hit_line(hit: &SearchOutputHit) -> String {
    let snippet = hit
        .snippet
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!(" - {value}"))
        .unwrap_or_default();
    format!(
        "{} [{}] score={:.1} {}{}",
        hit.task.task_ref,
        hit.task.status.as_str(),
        hit.score,
        hit.task.title,
        snippet
    )
}

#[cfg(test)]
mod contract_adapter_tests {
    use super::*;

    fn comment(author_type: &str, kind: &str) -> CommentRecord {
        CommentRecord {
            id: "c_test".to_owned(),
            board_id: "b_test".to_owned(),
            task_id: "t_test".to_owned(),
            author: "tester".to_owned(),
            author_type: author_type.to_owned(),
            agent_type: None,
            body: "body".to_owned(),
            kind: kind.to_owned(),
            metadata_json: "{}".to_owned(),
            created_at: 1,
        }
    }

    #[test]
    fn comment_adapter_rejects_unknown_author_type() {
        let error = api_comment_from_record(&comment("service", "note")).unwrap_err();
        assert!(error.to_string().contains("invalid author_type"));
    }

    #[test]
    fn comment_adapter_rejects_unknown_kind() {
        let error = api_comment_from_record(&comment("user", "memo")).unwrap_err();
        assert!(error.to_string().contains("invalid kind"));
    }

    #[test]
    fn comment_adapter_maps_agent_decision() {
        let mapped = api_comment_from_record(&comment("agent", "decision")).unwrap();
        assert_eq!(mapped.author_type, CommentAuthorType::Agent);
        assert_eq!(mapped.kind, CommentKind::Decision);
    }

    #[test]
    fn event_and_run_adapters_reject_malformed_persisted_json() {
        let event = EventRecord {
            id: 1,
            event_id: "evt_test".to_owned(),
            task_id: None,
            run_id: None,
            kind: "test".to_owned(),
            actor: None,
            payload_json: "{".to_owned(),
            created_at: 1,
        };
        assert!(cli_event_from_record(&event).is_err());

        let run = RunRecord {
            id: "run_test".to_owned(),
            task_id: "task_test".to_owned(),
            status: "running".to_owned(),
            worker_profile: None,
            worker_pid: None,
            claim_token: "claim_test".to_owned(),
            claim_owner: "tester".to_owned(),
            started_at: 1,
            finished_at: None,
            exit_code: None,
            summary: None,
            error: None,
            log_path: None,
            metadata_json: "{".to_owned(),
        };
        assert!(api_run_from_record(&run).is_err());
    }
}
