use anyhow::Result;

use crate::args::SearchOutputHit;

pub(crate) fn print_task(json: bool, task: &kanban_sqlite::TaskRecord) -> Result<()> {
    print_task_with_details(json, false, task)
}

pub(crate) fn print_task_with_details(
    json: bool,
    details: bool,
    task: &kanban_sqlite::TaskRecord,
) -> Result<()> {
    print_or_json(json, task, || {
        if details {
            task_details(task)
        } else {
            task_line(task)
        }
    })
}

pub(crate) fn print_or_json<T: serde::Serialize>(
    json: bool,
    data: &T,
    human: impl FnOnce() -> String,
) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"data": data}))?
        );
    } else {
        println!("{}", human());
    }
    Ok(())
}

pub(crate) fn task_line(task: &kanban_sqlite::TaskRecord) -> String {
    let labels = task_label_suffix(task);
    format!(
        "{} {} [{}] {}{}",
        task.task_ref,
        task.id,
        task.status.as_str(),
        task.title,
        labels
    )
}

pub(crate) fn task_details(task: &kanban_sqlite::TaskRecord) -> String {
    let mut lines = vec![
        format!("ref: {}", task.task_ref),
        format!("id: {}", task.id),
        format!("board_id: {}", task.board_id),
        format!("board_slug: {}", task.board_slug),
        format!("seq: {}", task.seq),
        format!("status: {}", task.status.as_str()),
        format!("title: {}", task.title),
        format!(
            "labels: {}",
            if task.labels.is_empty() {
                "-".to_owned()
            } else {
                task.labels
                    .iter()
                    .map(|label| label.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            }
        ),
    ];
    push_multiline_field(&mut lines, "description", task.description.as_deref());
    lines.extend([
        format!(
            "status_reason: {}",
            option_display(task.status_reason.as_deref())
        ),
        format!("assignee: {}", option_display(task.assignee.as_deref())),
        format!("priority: P{}", task.priority),
        format!("position: {}", task.position),
        format!("scheduled_at: {}", option_i64(task.scheduled_at)),
        format!("due_at: {}", option_i64(task.due_at)),
        format!("created_by: {}", task.created_by),
        format!("created_at: {}", task.created_at),
        format!("updated_at: {}", task.updated_at),
        format!("started_at: {}", option_i64(task.started_at)),
        format!("completed_at: {}", option_i64(task.completed_at)),
        format!("archived_at: {}", option_i64(task.archived_at)),
        format!(
            "claim_token: {}",
            option_display(task.claim_token.as_deref())
        ),
        format!(
            "claim_owner: {}",
            option_display(task.claim_owner.as_deref())
        ),
        format!("claim_expires_at: {}", option_i64(task.claim_expires_at)),
        format!("last_heartbeat_at: {}", option_i64(task.last_heartbeat_at)),
        format!(
            "current_run_id: {}",
            option_display(task.current_run_id.as_deref())
        ),
        format!("retry_count: {}", task.retry_count),
        format!("max_retries: {}", option_i64(task.max_retries)),
        format!(
            "result_summary: {}",
            option_display(task.result_summary.as_deref())
        ),
        format!(
            "result_json: {}",
            option_display(task.result_json.as_deref())
        ),
        format!("metadata_json: {}", task.metadata_json),
        format!("lock_version: {}", task.lock_version),
    ]);
    lines.join("\n")
}

pub(crate) fn label_line(label: &kanban_sqlite::LabelRecord) -> String {
    let color = label.color.as_deref().unwrap_or("-");
    format!("{} {} color={}", label.id, label.name, color)
}

fn task_label_suffix(task: &kanban_sqlite::TaskRecord) -> String {
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

fn option_display(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn option_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn push_multiline_field(lines: &mut Vec<String>, label: &str, value: Option<&str>) {
    match value {
        Some(value) if !value.is_empty() => {
            lines.push(format!("{label}:"));
            lines.extend(value.lines().map(|line| format!("  {line}")));
        }
        _ => lines.push(format!("{label}: -")),
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
        "#{} {} [{}] score={:.1} {}{}",
        hit.seq,
        hit.task_id,
        hit.task.status.as_str(),
        hit.score,
        hit.task.title,
        snippet
    )
}
