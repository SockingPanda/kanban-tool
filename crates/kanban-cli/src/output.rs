use anyhow::Result;

use crate::args::SearchOutputHit;

pub(crate) fn print_task(json: bool, task: &kanban_sqlite::TaskRecord) -> Result<()> {
    print_or_json(json, task, || task_line(task))
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
    format!(
        "{} {} [{}] {}",
        task.task_ref,
        task.id,
        task.status.as_str(),
        task.title
    )
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
