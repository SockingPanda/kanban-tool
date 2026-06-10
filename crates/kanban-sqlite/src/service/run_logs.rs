use std::{
    fs,
    path::{Path, PathBuf},
};

use kanban_core::{KanbanError, Result};

use super::RunLogPathStatus;

pub fn resolve_run_log_path(
    db_path: impl AsRef<Path>,
    run_id: &str,
    log_path: &str,
) -> Result<PathBuf> {
    match run_log_path_status(db_path, run_id, log_path) {
        RunLogPathStatus::Present(path) => Ok(path),
        RunLogPathStatus::Missing(_) => Err(KanbanError::NotFound(format!("run log {run_id}"))),
        RunLogPathStatus::Suspicious { reason } => Err(KanbanError::InvalidInput(format!(
            "suspicious run log path for {run_id}: {reason}"
        ))),
    }
}

pub fn run_log_path_status(
    db_path: impl AsRef<Path>,
    run_id: &str,
    log_path: &str,
) -> RunLogPathStatus {
    run_log_path_status_for_db_dir(db_path.as_ref().parent(), run_id, log_path)
}

pub(crate) fn run_log_path_status_for_db_dir(
    db_dir: Option<&Path>,
    run_id: &str,
    log_path: &str,
) -> RunLogPathStatus {
    let expected_name = format!("{run_id}.log");
    let stored_path = Path::new(log_path);
    if stored_path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        return RunLogPathStatus::Suspicious {
            reason: format!("expected log file name {expected_name}"),
        };
    }

    let Some(db_dir) = db_dir else {
        return RunLogPathStatus::Suspicious {
            reason: "database path has no parent directory".to_owned(),
        };
    };
    let candidate = if stored_path.is_absolute() {
        stored_path.to_path_buf()
    } else {
        db_dir.join(stored_path)
    };
    let normalized_candidate = normalize_existing_aware(&candidate);
    let allowed = allowed_run_log_roots(db_dir)
        .iter()
        .map(|root| normalize_existing_aware(root))
        .any(|root| normalized_candidate.starts_with(root));
    if !allowed {
        return RunLogPathStatus::Suspicious {
            reason: "path is outside allowed run log roots".to_owned(),
        };
    }
    if normalized_candidate.exists() {
        RunLogPathStatus::Present(normalized_candidate)
    } else {
        RunLogPathStatus::Missing(normalized_candidate)
    }
}

pub(crate) fn allowed_run_log_roots(db_dir: &Path) -> [PathBuf; 3] {
    [
        kanban_local::default_log_dir().join("runs"),
        db_dir.join("logs"),
        db_dir.join(".kb").join("logs"),
    ]
}

pub(crate) fn normalize_existing_aware(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    let mut missing = Vec::new();
    let mut ancestor = path;
    while let Some(parent) = ancestor.parent() {
        if let Some(name) = ancestor.file_name() {
            missing.push(name.to_owned());
        }
        if let Ok(canonical_parent) = fs::canonicalize(parent) {
            let mut normalized = canonical_parent;
            for component in missing.iter().rev() {
                normalized.push(component);
            }
            return lexical_normalize(&normalized);
        }
        ancestor = parent;
    }
    lexical_normalize(path)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
