use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use kanban_core::{KanbanError, TaskStatus};
use kanban_sqlite::TaskListSort;

pub(crate) fn resolve_optional_text_input(
    inline: Option<String>,
    file: Option<PathBuf>,
    inline_name: &str,
    file_name: &str,
) -> Result<Option<String>> {
    if inline.is_some() && file.is_some() {
        bail!("{inline_name} and {file_name} are mutually exclusive");
    }
    if let Some(path) = file {
        return Ok(Some(read_text_input(&path)?));
    }
    Ok(inline)
}

pub(crate) fn resolve_required_text_input(
    inline: Option<String>,
    file: Option<PathBuf>,
    inline_name: &str,
    file_name: &str,
    value_name: &str,
) -> Result<String> {
    resolve_optional_text_input(inline, file, inline_name, file_name)?
        .ok_or_else(|| anyhow::anyhow!("{value_name} requires either {inline_name} or {file_name}"))
}

fn read_text_input(path: &PathBuf) -> Result<String> {
    if path.as_os_str() == "-" {
        let mut value = String::new();
        io::stdin()
            .read_to_string(&mut value)
            .with_context(|| "failed to read stdin")?;
        return Ok(value);
    }
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

pub(crate) fn validate_page_bounds(limit: usize, max_limit: usize, offset: usize) -> Result<()> {
    if limit > max_limit {
        return Err(KanbanError::InvalidInput(format!("limit must be <= {max_limit}")).into());
    }
    if offset > i64::MAX as usize {
        return Err(KanbanError::InvalidInput(format!("offset must be <= {}", i64::MAX)).into());
    }
    Ok(())
}

pub(crate) fn parse_status(value: &str) -> Result<TaskStatus> {
    TaskStatus::try_from(value).map_err(|err| anyhow::anyhow!(err))
}

pub(crate) fn parse_task_list_sort(value: &str) -> Result<TaskListSort> {
    match value {
        "seq" => Ok(TaskListSort::Seq),
        "seq_desc" | "-seq" => Ok(TaskListSort::SeqDesc),
        "title" => Ok(TaskListSort::Title),
        "title_desc" | "-title" => Ok(TaskListSort::TitleDesc),
        "status" => Ok(TaskListSort::Status),
        "status_desc" | "-status" => Ok(TaskListSort::StatusDesc),
        "position" => Ok(TaskListSort::Position),
        "position_desc" | "-position" => Ok(TaskListSort::PositionDesc),
        "priority" => Ok(TaskListSort::Priority),
        "priority_desc" | "-priority" => Ok(TaskListSort::PriorityDesc),
        "assignee" => Ok(TaskListSort::Assignee),
        "assignee_desc" | "-assignee" => Ok(TaskListSort::AssigneeDesc),
        "scheduled" | "scheduled_at" => Ok(TaskListSort::ScheduledAt),
        "scheduled_desc" | "scheduled_at_desc" | "-scheduled_at" => {
            Ok(TaskListSort::ScheduledAtDesc)
        }
        "created" | "created_at" => Ok(TaskListSort::CreatedAt),
        "created_desc" | "created_at_desc" | "-created_at" => Ok(TaskListSort::CreatedAtDesc),
        "updated" | "updated_at" => Ok(TaskListSort::UpdatedAt),
        "updated_desc" | "updated_at_desc" | "-updated_at" => Ok(TaskListSort::UpdatedAtDesc),
        "due" | "due_at" => Ok(TaskListSort::DueAt),
        "due_desc" | "due_at_desc" | "-due_at" => Ok(TaskListSort::DueAtDesc),
        _ => bail!("unsupported task list sort: {value}"),
    }
}

pub(crate) fn optional_clearable<T>(value: Option<T>, clear: bool) -> Option<Option<T>> {
    if clear { Some(None) } else { value.map(Some) }
}

pub(crate) fn default_db_path() -> PathBuf {
    kanban_local::default_db_path()
}

pub(crate) fn default_actor() -> String {
    kanban_local::default_actor()
}

pub(crate) fn active_board(flag: Option<&str>) -> Result<String> {
    if let Some(board) = flag.map(str::trim).filter(|board| !board.is_empty()) {
        return Ok(board.to_owned());
    }
    if let Ok(board) = std::env::var("KB_BOARD") {
        let board = board.trim();
        if !board.is_empty() {
            return Ok(board.to_owned());
        }
    }
    if let Some(board) = kanban_local::nearest_active_board_config()?
        .map(|board| board.trim().to_owned())
        .filter(|board| !board.is_empty())
    {
        return Ok(board);
    }
    Ok("default".to_owned())
}

pub(crate) fn write_board_config(board: &str) -> Result<()> {
    kanban_local::write_active_board_config(board)
        .with_context(|| "failed to write project board config")?;
    Ok(())
}
