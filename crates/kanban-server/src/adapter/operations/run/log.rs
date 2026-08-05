use std::path::{Component, Path, PathBuf};

use kanban_service::{RunLog, RunLogRecord};
use kanban_core::{KanbanError, Result};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
};

use crate::adapter::{TursoApplicationStore, store_error};

const RUN_LOG_FILE_SUFFIX: &str = ".log";

impl RunLog for TursoApplicationStore {
    async fn read_run_log(&self, run_id: &str, max_bytes: usize) -> Result<RunLogRecord> {
        let run = self
            .store
            .get_run_log_source(run_id)
            .await
            .map_err(store_error)?;

        read_run_log_file(
            self.run_log_root(),
            &run.id,
            run.log_path.as_deref(),
            max_bytes,
        )
        .await
    }
}

async fn read_run_log_file(
    trusted_root: Option<&Path>,
    run_id: &str,
    log_path: Option<&str>,
    max_bytes: usize,
) -> Result<RunLogRecord> {
    let log_path = log_path
        .map(str::trim)
        .ok_or_else(|| KanbanError::NotFound(format!("run {run_id} does not have a log path")))?;
    if log_path.is_empty() {
        return Err(KanbanError::InvalidInput(
            "run log path must not be empty".to_owned(),
        ));
    }
    let trusted_root = trusted_root.ok_or_else(|| {
        KanbanError::NotFound(format!("run log root is not configured for run {run_id}"))
    })?;

    let candidate = resolve_run_log_path(trusted_root, run_id, log_path)?;
    let metadata = tokio::fs::metadata(&candidate)
        .await
        .map_err(|error| map_file_error(&candidate, error))?;
    if !metadata.is_file() {
        return Err(KanbanError::InvalidInput(format!(
            "run log path is not a regular file: {}",
            candidate.display()
        )));
    }

    // Open the canonical target and take a metadata snapshot from the opened
    // handle before seeking. This bounds the read to a suffix without loading
    // the complete log into memory.
    let mut file = File::open(&candidate)
        .await
        .map_err(|error| map_file_error(&candidate, error))?;
    let metadata = file
        .metadata()
        .await
        .map_err(|error| map_file_error(&candidate, error))?;
    if !metadata.is_file() {
        return Err(KanbanError::InvalidInput(format!(
            "run log path is not a regular file: {}",
            candidate.display()
        )));
    }

    let file_len = metadata.len();
    let max_bytes = max_bytes as u64;
    let start = file_len.saturating_sub(max_bytes);
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|error| {
            KanbanError::Storage(format!(
                "cannot seek run log {}: {error}",
                candidate.display()
            ))
        })?;
    let mut bytes = Vec::with_capacity((file_len - start).min(max_bytes) as usize);
    file.take(max_bytes)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            KanbanError::Storage(format!(
                "cannot read run log {}: {error}",
                candidate.display()
            ))
        })?;

    Ok(RunLogRecord {
        run_id: run_id.to_owned(),
        content: String::from_utf8_lossy(&bytes).into_owned(),
        truncated: file_len > max_bytes,
    })
}

fn resolve_run_log_path(trusted_root: &Path, run_id: &str, log_path: &str) -> Result<PathBuf> {
    let path = Path::new(log_path);
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(KanbanError::InvalidInput(
            "run log path must not contain '..'".to_owned(),
        ));
    }

    let expected_name = format!("{run_id}{RUN_LOG_FILE_SUFFIX}");
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        return Err(KanbanError::InvalidInput(format!(
            "run log filename must be {expected_name}"
        )));
    }

    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        trusted_root.join(path)
    };
    let lexical_parent = path.parent().unwrap_or_else(|| Path::new(""));
    if (path.is_absolute() && lexical_parent != trusted_root)
        || (!path.is_absolute() && !lexical_parent.as_os_str().is_empty())
    {
        return Err(KanbanError::InvalidInput(
            "run log path must be directly under the trusted root".to_owned(),
        ));
    }
    let link_metadata = std::fs::symlink_metadata(&candidate).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KanbanError::NotFound(format!("run log file not found: {}", candidate.display()))
        } else {
            KanbanError::Storage(format!(
                "cannot inspect run log path {}: {error}",
                candidate.display()
            ))
        }
    })?;
    if link_metadata.file_type().is_symlink() {
        return Err(KanbanError::InvalidInput(
            "run log path must not be a symlink".to_owned(),
        ));
    }
    if !link_metadata.file_type().is_file() {
        return Err(KanbanError::InvalidInput(format!(
            "run log path is not a regular file: {}",
            candidate.display()
        )));
    }
    let canonical_root = trusted_root;
    let canonical_candidate = std::fs::canonicalize(&candidate).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KanbanError::NotFound(format!("run log file not found: {}", candidate.display()))
        } else {
            KanbanError::Storage(format!(
                "cannot resolve run log path {}: {error}",
                candidate.display()
            ))
        }
    })?;
    if !canonical_candidate.starts_with(canonical_root) {
        return Err(KanbanError::InvalidInput(
            "run log path resolves outside the trusted root".to_owned(),
        ));
    }
    if canonical_candidate.parent() != Some(canonical_root) {
        return Err(KanbanError::InvalidInput(
            "run log path must be directly under the trusted root".to_owned(),
        ));
    }
    Ok(canonical_candidate)
}

fn map_file_error(path: &Path, error: std::io::Error) -> KanbanError {
    if error.kind() == std::io::ErrorKind::NotFound {
        KanbanError::NotFound(format!("run log file not found: {}", path.display()))
    } else {
        KanbanError::Storage(format!("cannot read run log {}: {error}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{read_run_log_file, resolve_run_log_path};
    use kanban_core::KanbanError;

    #[tokio::test]
    async fn reads_short_log_and_lossy_utf8() {
        let root = tempdir().unwrap();
        let path = root.path().join("r_short.log");
        fs::write(&path, [b'o', b'k', 0xff]).unwrap();

        let result = read_run_log_file(Some(root.path()), "r_short", Some("r_short.log"), 64)
            .await
            .unwrap();
        assert_eq!(result.run_id, "r_short");
        assert_eq!(result.content, "ok�");
        assert!(!result.truncated);
    }

    #[tokio::test]
    async fn reads_only_the_bounded_suffix() {
        let root = tempdir().unwrap();
        let path = root.path().join("r_large.log");
        fs::write(&path, vec![b'a'; 256 * 1024 + 17]).unwrap();

        let result = read_run_log_file(
            Some(root.path()),
            "r_large",
            Some("r_large.log"),
            256 * 1024,
        )
        .await
        .unwrap();
        assert_eq!(result.content.len(), 256 * 1024);
        assert!(result.truncated);
    }

    #[tokio::test]
    async fn missing_configuration_and_files_are_not_found() {
        let root = tempdir().unwrap();
        assert!(matches!(
            read_run_log_file(None, "r_missing", Some("r_missing.log"), 10).await,
            Err(KanbanError::NotFound(_))
        ));
        assert!(matches!(
            read_run_log_file(Some(root.path()), "r_missing", None, 10).await,
            Err(KanbanError::NotFound(_))
        ));
        assert!(matches!(
            read_run_log_file(Some(root.path()), "r_missing", Some("r_missing.log"), 10).await,
            Err(KanbanError::NotFound(_))
        ));
    }

    #[test]
    fn rejects_path_traversal_root_escape_and_wrong_filename() {
        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let outside_path = outside.path().join("r_path.log");
        fs::write(&outside_path, "outside").unwrap();

        for path in [
            "../r_path.log",
            outside_path.to_str().unwrap(),
            "nested/r_path.log",
            "r_other.log",
        ] {
            let result = resolve_run_log_path(root.path(), "r_path", path);
            assert!(
                matches!(result, Err(KanbanError::InvalidInput(_))),
                "{path}"
            );
        }
    }

    #[tokio::test]
    async fn rejects_non_regular_file_and_symlink_escape() {
        let root = tempdir().unwrap();
        let directory_path = root.path().join("r_directory.log");
        fs::create_dir(&directory_path).unwrap();
        assert!(matches!(
            read_run_log_file(
                Some(root.path()),
                "r_directory",
                Some("r_directory.log"),
                10
            )
            .await,
            Err(KanbanError::InvalidInput(_))
        ));

        #[cfg(unix)]
        {
            let outside = tempdir().unwrap();
            let outside_file = outside.path().join("outside.log");
            fs::write(&outside_file, "outside").unwrap();
            std::os::unix::fs::symlink(&outside_file, root.path().join("r_link.log")).unwrap();
            assert!(matches!(
                read_run_log_file(Some(root.path()), "r_link", Some("r_link.log"), 10).await,
                Err(KanbanError::InvalidInput(_))
            ));

            let inside_file = root.path().join("r_inside_target.log");
            fs::write(&inside_file, "inside").unwrap();
            std::os::unix::fs::symlink(&inside_file, root.path().join("r_inside.log")).unwrap();
            assert!(matches!(
                read_run_log_file(Some(root.path()), "r_inside", Some("r_inside.log"), 10).await,
                Err(KanbanError::InvalidInput(_))
            ));
        }
    }
}
