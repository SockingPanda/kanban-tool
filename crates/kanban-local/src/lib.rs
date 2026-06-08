use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

pub const INDEX_LAYOUT_VERSION: &str = "v1";
pub const TASK_INDEX_NAME: &str = "tasks";
pub const GRAPH_STORE_NAME: &str = "graph";
pub const VECTOR_STORE_NAME: &str = "vectors";
pub const BLOBS_DIR_NAME: &str = "blobs";
pub const ATTACHMENTS_DIR_NAME: &str = "attachments";

pub fn default_db_path() -> PathBuf {
    default_data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("kb.db")
}

pub fn default_log_dir() -> PathBuf {
    default_state_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("logs")
}

pub fn default_data_dir() -> Option<PathBuf> {
    dirs_next::data_dir()
}

pub fn default_state_dir() -> Option<PathBuf> {
    state_dir_from_parts(
        std::env::var_os("XDG_STATE_HOME").as_deref(),
        dirs_next::home_dir(),
        dirs_next::data_dir(),
    )
}

fn state_dir_from_parts(
    xdg_state_home: Option<&OsStr>,
    home_dir: Option<PathBuf>,
    fallback_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(state_home) = xdg_state_home
        .map(Path::new)
        .filter(|path| !path.as_os_str().is_empty())
    {
        return Some(state_home.to_path_buf());
    }

    if cfg!(target_os = "linux") || cfg!(target_os = "freebsd") || cfg!(target_os = "openbsd") {
        return home_dir
            .map(|home| home.join(".local").join("state"))
            .or(fallback_dir);
    }

    fallback_dir
}

pub fn kb_data_dir_for_db(db_path: impl Into<PathBuf>) -> PathBuf {
    let db_path = db_path.into();
    db_path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn index_root_path(db_path: impl Into<PathBuf>) -> PathBuf {
    kb_data_dir_for_db(db_path)
        .join("index")
        .join(INDEX_LAYOUT_VERSION)
}

pub fn task_index_path(db_path: impl Into<PathBuf>) -> PathBuf {
    index_root_path(db_path).join(TASK_INDEX_NAME)
}

pub fn graph_store_path(db_path: impl Into<PathBuf>) -> PathBuf {
    index_root_path(db_path).join(GRAPH_STORE_NAME)
}

pub fn vector_store_path(db_path: impl Into<PathBuf>) -> PathBuf {
    index_root_path(db_path).join(VECTOR_STORE_NAME)
}

pub fn blob_root_path(db_path: impl Into<PathBuf>) -> PathBuf {
    kb_data_dir_for_db(db_path).join(BLOBS_DIR_NAME)
}

pub fn attachments_root_path(db_path: impl Into<PathBuf>) -> PathBuf {
    kb_data_dir_for_db(db_path).join(ATTACHMENTS_DIR_NAME)
}

pub fn attachment_blob_path(
    db_path: impl Into<PathBuf>,
    board_id: &str,
    task_id: &str,
    attachment_id: &str,
    filename: &str,
) -> PathBuf {
    attachments_root_path(db_path)
        .join(board_id)
        .join(task_id)
        .join(attachment_id)
        .join(filename)
}

pub fn default_actor() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "local".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_match_kb_data_layout() {
        let db_path = default_db_path();
        assert!(db_path.ends_with("kb/kb.db"));

        let log_dir = default_log_dir();
        assert!(log_dir.ends_with("kb/logs"));
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
    #[test]
    fn default_state_dir_uses_state_root_not_data_root() {
        let state = state_dir_from_parts(
            None,
            Some(PathBuf::from("/home/alice")),
            Some(PathBuf::from("/home/alice/.local/share")),
        )
        .unwrap();
        let data = PathBuf::from("/home/alice/.local/share");

        assert_eq!(state, PathBuf::from("/home/alice/.local/state"));
        assert_ne!(state, data);
        assert_eq!(
            state.join("kb").join("logs"),
            PathBuf::from("/home/alice/.local/state/kb/logs")
        );
    }

    #[test]
    fn default_state_dir_honors_xdg_state_home_before_fallback() {
        let state = state_dir_from_parts(
            Some(OsStr::new("/tmp/xdg-state")),
            Some(PathBuf::from("/home/alice")),
            Some(PathBuf::from("/home/alice/.local/share")),
        )
        .unwrap();

        assert_eq!(state, PathBuf::from("/tmp/xdg-state"));
    }

    #[test]
    fn derived_and_blob_paths_are_stable_next_to_db() {
        let db_path = PathBuf::from("/tmp/project/.kb/kb.db");

        assert_eq!(
            task_index_path(db_path.clone()),
            PathBuf::from("/tmp/project/.kb/index/v1/tasks")
        );
        assert_eq!(
            graph_store_path(db_path.clone()),
            PathBuf::from("/tmp/project/.kb/index/v1/graph")
        );
        assert_eq!(
            vector_store_path(db_path.clone()),
            PathBuf::from("/tmp/project/.kb/index/v1/vectors")
        );
        assert_eq!(
            blob_root_path(db_path.clone()),
            PathBuf::from("/tmp/project/.kb/blobs")
        );
        assert_eq!(
            attachment_blob_path(db_path, "b1", "t1", "a1", "file.txt"),
            PathBuf::from("/tmp/project/.kb/attachments/b1/t1/a1/file.txt")
        );
    }
}
