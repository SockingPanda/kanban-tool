use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use kanban_core::KanbanError;

pub(crate) fn invalid_input(message: impl Into<String>) -> anyhow::Error {
    KanbanError::InvalidInput(message.into()).into()
}

const MAX_TEXT_INPUT_BYTES: usize = 1_048_576;

pub(crate) fn is_stdio_path(path: &Path) -> bool {
    path.as_os_str() == "-"
}

pub(crate) fn resolve_optional_text_input(
    inline: Option<String>,
    file: Option<PathBuf>,
    inline_name: &str,
    file_name: &str,
) -> Result<Option<String>> {
    if inline.is_some() && file.is_some() {
        return Err(invalid_input(format!(
            "{inline_name} and {file_name} are mutually exclusive"
        )));
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
    resolve_optional_text_input(inline, file, inline_name, file_name)?.ok_or_else(|| {
        invalid_input(format!(
            "{value_name} is required; pass either {inline_name} or {file_name}"
        ))
    })
}

pub(crate) fn read_text_input(path: &Path) -> Result<String> {
    if is_stdio_path(path) {
        let stdin = io::stdin();
        let mut handle = stdin.lock();
        return read_bounded_text(&mut handle, "stdin").with_context(|| "failed to read stdin");
    }
    let mut file = File::open(path).with_context(|| format!("failed to read {}", path.display()))?;
    read_bounded_text(&mut file, "input file")
        .with_context(|| format!("failed to read {}", path.display()))
}

fn read_bounded_text(reader: &mut impl Read, source: &str) -> Result<String> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_TEXT_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_TEXT_INPUT_BYTES {
        return Err(invalid_input(format!(
            "{source} size exceeds {MAX_TEXT_INPUT_BYTES} bytes"
        )));
    }
    String::from_utf8(bytes).map_err(|_| invalid_input(format!("{source} is not valid UTF-8")))
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

pub(crate) fn optional_clearable<T>(value: Option<T>, clear: bool) -> Option<Option<T>> {
    if clear { Some(None) } else { value.map(Some) }
}

pub(crate) fn resolved_db_path(explicit: Option<&Path>) -> Result<PathBuf> {
    kanban_local::resolved_db_path(explicit).with_context(|| "failed to resolve database path")
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
