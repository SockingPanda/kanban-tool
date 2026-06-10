use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use kanban_core::TaskStatus;
use kanban_entity::Predicate;
use kanban_sqlite::TaskListSort;

use crate::commands::dispatch::unquote;

pub(crate) fn parse_predicate(value: &str) -> Result<Predicate> {
    match value {
        "belongs_to_board" => Ok(Predicate::BelongsToBoard),
        "belongs_to_task" => Ok(Predicate::BelongsToTask),
        "depends_on" => Ok(Predicate::DependsOn),
        "produced_by" => Ok(Predicate::ProducedBy),
        "generated_by" => Ok(Predicate::GeneratedBy),
        "references_artifact" => Ok(Predicate::ReferencesArtifact),
        "related_to" => Ok(Predicate::RelatedTo),
        "uses_skill" => Ok(Predicate::UsesSkill),
        "uses_context" => Ok(Predicate::UsesContext),
        "derived_from" => Ok(Predicate::DerivedFrom),
        "supersedes" => Ok(Predicate::Supersedes),
        "similar_to" => Ok(Predicate::SimilarTo),
        "requires_review" => Ok(Predicate::RequiresReview),
        "waiting_for_user" => Ok(Predicate::WaitingForUser),
        _ => bail!("unsupported predicate: {value}"),
    }
}

pub(crate) fn validate_page_bounds(limit: usize, max_limit: usize, offset: usize) -> Result<()> {
    if limit > max_limit {
        bail!("limit must be <= {max_limit}");
    }
    if offset > i64::MAX as usize {
        bail!("offset must be <= {}", i64::MAX);
    }
    Ok(())
}

pub(crate) fn parse_status(value: &str) -> Result<TaskStatus> {
    TaskStatus::try_from(value).map_err(|err| anyhow::anyhow!(err))
}

pub(crate) fn parse_task_list_sort(value: &str) -> Result<TaskListSort> {
    match value {
        "position" => Ok(TaskListSort::Position),
        "position_desc" => Ok(TaskListSort::PositionDesc),
        "priority" => Ok(TaskListSort::Priority),
        "priority_desc" => Ok(TaskListSort::PriorityDesc),
        "created" | "created_at" => Ok(TaskListSort::CreatedAt),
        "created_desc" | "created_at_desc" => Ok(TaskListSort::CreatedAtDesc),
        "updated" | "updated_at" => Ok(TaskListSort::UpdatedAt),
        "updated_desc" | "updated_at_desc" => Ok(TaskListSort::UpdatedAtDesc),
        "due" | "due_at" => Ok(TaskListSort::DueAt),
        "due_desc" | "due_at_desc" => Ok(TaskListSort::DueAtDesc),
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
    if let Some(path) = nearest_board_config()?
        && let Some(board) = read_board_config(&path)?
    {
        return Ok(board);
    }
    Ok("default".to_owned())
}

pub(crate) fn nearest_board_config() -> Result<Option<PathBuf>> {
    let mut dir = std::env::current_dir().context("failed to resolve current directory")?;
    loop {
        let candidate = dir.join(".kb").join("config.toml");
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

pub(crate) fn write_board_config(board: &str) -> Result<()> {
    let path = nearest_board_config()?.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".kb")
            .join("config.toml")
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(
        &path,
        format!("board = \"{}\"\n", escape_toml_string(board)),
    )
    .with_context(|| format!("failed to write {}", path.display()))
}

pub(crate) fn read_board_config(path: &Path) -> Result<Option<String>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "board" {
            let board = unquote(value.trim()).trim();
            if !board.is_empty() {
                return Ok(Some(board.to_owned()));
            }
        }
    }
    Ok(None)
}

pub(crate) fn escape_toml_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
