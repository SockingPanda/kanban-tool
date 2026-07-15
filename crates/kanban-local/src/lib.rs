use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};

use fs_err as fs;
use serde::{Deserialize, Serialize, de::IntoDeserializer};

pub use kanban_contract::{
    ProjectConfigInput as ProjectConfig, ProjectVectorConfigInput as VectorConfig,
    WorkerFinishPolicy, WorkerProfileInput, WorkerProfilesInput,
};

pub const INDEX_LAYOUT_VERSION: &str = "v1";
pub const TASK_INDEX_NAME: &str = "tasks";
pub const GRAPH_STORE_NAME: &str = "graph";
pub const VECTOR_STORE_NAME: &str = "vectors";
pub const BLOBS_DIR_NAME: &str = "blobs";
pub const ATTACHMENTS_DIR_NAME: &str = "attachments";
pub const USER_CONFIG_DIR_NAME: &str = "kanban";

pub const DEFAULT_VECTOR_PROVIDER: &str = "ollama";
pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
pub const DEFAULT_OLLAMA_EMBEDDING_MODEL: &str = "qwen3-embedding:0.6b";
pub const DEFAULT_OLLAMA_EMBEDDING_DIMENSIONS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigValueSource {
    Flag { name: &'static str },
    Env { name: &'static str },
    ProjectConfig { path: PathBuf, key: &'static str },
    GlobalConfig { path: PathBuf, key: &'static str },
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedConfigValue<T> {
    pub value: T,
    pub source: ConfigValueSource,
}

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

pub fn default_config_dir() -> Option<PathBuf> {
    dirs_next::config_dir()
}

pub fn global_config_dir() -> PathBuf {
    global_config_dir_from_root(default_config_dir().unwrap_or_else(|| PathBuf::from(".")))
}

pub fn global_config_path() -> PathBuf {
    global_config_dir().join("config.toml")
}

fn global_config_dir_from_root(root: impl Into<PathBuf>) -> PathBuf {
    root.into().join(USER_CONFIG_DIR_NAME)
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

pub fn resolved_db_path(explicit_path: Option<&Path>) -> Result<PathBuf, ConfigError> {
    Ok(resolved_db_path_with_source(explicit_path)?.value)
}

pub fn resolved_db_path_with_source(
    explicit_path: Option<&Path>,
) -> Result<ResolvedConfigValue<PathBuf>, ConfigError> {
    if let Some(path) = explicit_path {
        return Ok(ResolvedConfigValue {
            value: path.to_path_buf(),
            source: ConfigValueSource::Flag { name: "--db" },
        });
    }

    if let Some(path) = env_db_path("KANBAN_DB") {
        return Ok(ResolvedConfigValue {
            value: path,
            source: ConfigValueSource::Env { name: "KANBAN_DB" },
        });
    }

    if let Some(path) = env_db_path("KB_DB") {
        return Ok(ResolvedConfigValue {
            value: path,
            source: ConfigValueSource::Env { name: "KB_DB" },
        });
    }

    if let Some(path) = nearest_project_config()? {
        let config = read_project_config(&path)?;
        if let Some(db) = non_empty_path(config.db) {
            return Ok(ResolvedConfigValue {
                value: path_relative_to_config(&path, db),
                source: ConfigValueSource::ProjectConfig { path, key: "db" },
            });
        }
    }

    let global = global_config_path();
    if global.is_file() {
        let config = read_project_config(&global)?;
        if let Some(db) = non_empty_path(config.db) {
            return Ok(ResolvedConfigValue {
                value: path_relative_to_config(&global, db),
                source: ConfigValueSource::GlobalConfig {
                    path: global,
                    key: "db",
                },
            });
        }
    }

    Ok(ResolvedConfigValue {
        value: default_db_path(),
        source: ConfigValueSource::Default,
    })
}

fn env_db_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .map(PathBuf::from)
        .and_then(|path| non_empty_path(Some(path)))
}

fn non_empty_path(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| {
        !path.as_os_str().is_empty() && !path.as_os_str().to_string_lossy().trim().is_empty()
    })
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

pub fn nearest_project_config() -> io::Result<Option<PathBuf>> {
    let mut dir = std::env::current_dir()?;
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

pub fn project_config_path_for_write() -> io::Result<PathBuf> {
    nearest_project_config().map(|path| {
        path.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(".kb")
                .join("config.toml")
        })
    })
}

pub fn read_project_config(path: &Path) -> Result<ProjectConfig, ConfigError> {
    let text = fs::read_to_string(path)?;
    let deserializer = toml::Deserializer::new(&text);
    serde_path_to_error::deserialize(deserializer).map_err(|err| ConfigError::FileParse {
        path: path.to_path_buf(),
        field_path: err.path().to_string(),
        source: Box::new(err.into_inner()),
    })
}

pub fn read_worker_profiles(path: &Path) -> Result<WorkerProfilesInput, ConfigError> {
    let text = fs::read_to_string(path)?;
    let deserializer = toml::Deserializer::new(&text);
    serde_path_to_error::deserialize(deserializer).map_err(|err| ConfigError::FileParse {
        path: path.to_path_buf(),
        field_path: err.path().to_string(),
        source: Box::new(err.into_inner()),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerProfileSections {
    workers: BTreeMap<String, toml::Value>,
}

pub fn read_worker_profile(
    path: &Path,
    profile_name: &str,
) -> Result<Option<WorkerProfileInput>, ConfigError> {
    let text = fs::read_to_string(path)?;
    let deserializer = toml::Deserializer::new(&text);
    let mut document: WorkerProfileSections = serde_path_to_error::deserialize(deserializer)
        .map_err(|err| ConfigError::FileParse {
            path: path.to_path_buf(),
            field_path: err.path().to_string(),
            source: Box::new(err.into_inner()),
        })?;
    let Some(profile) = document.workers.remove(profile_name) else {
        return Ok(None);
    };
    serde_path_to_error::deserialize(profile.into_deserializer())
        .map(Some)
        .map_err(|err| ConfigError::FileParse {
            path: path.to_path_buf(),
            field_path: format!("workers.{profile_name}.{}", err.path()),
            source: Box::new(err.into_inner()),
        })
}

pub fn write_project_config(path: &Path, config: &ProjectConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(config)?;
    fs::write(path, text)?;
    Ok(())
}

pub fn write_active_board_config_at(path: &Path, board: &str) -> Result<(), ConfigError> {
    let mut config = if path.is_file() {
        read_project_config(path)?
    } else {
        ProjectConfig::default()
    };
    config.board = Some(board.to_owned());
    write_project_config(path, &config)
}

pub fn write_active_board_config(board: &str) -> Result<PathBuf, ConfigError> {
    let path = project_config_path_for_write()?;
    write_active_board_config_at(&path, board)?;
    Ok(path)
}

pub fn write_vector_config_at(path: &Path, vector: VectorConfig) -> Result<(), ConfigError> {
    let mut config = if path.is_file() {
        read_project_config(path)?
    } else {
        ProjectConfig::default()
    };
    config.vector = Some(vector);
    write_project_config(path, &config)
}

pub fn write_vector_config(vector: VectorConfig) -> Result<PathBuf, ConfigError> {
    let path = global_config_path();
    write_vector_config_at(&path, vector)?;
    Ok(path)
}

pub fn resolved_vector_config(
    explicit_path: Option<&Path>,
) -> Result<Option<VectorConfig>, ConfigError> {
    if let Some(path) = explicit_path {
        return Ok(read_project_config(path)?.vector);
    }
    if let Some(path) = nearest_project_config()?
        && let Some(vector) = read_project_config(&path)?.vector
    {
        return Ok(Some(vector));
    }
    resolved_global_vector_config(&global_config_path())
}

fn resolved_global_vector_config(global_path: &Path) -> Result<Option<VectorConfig>, ConfigError> {
    if global_path.is_file() {
        return Ok(read_project_config(global_path)?.vector);
    }
    Ok(None)
}

pub fn nearest_active_board_config() -> Result<Option<String>, ConfigError> {
    let Some(path) = nearest_project_config()? else {
        return Ok(None);
    };
    Ok(read_project_config(&path)?.board)
}

pub fn resolved_active_board_with_source(
    explicit_board: Option<&str>,
) -> Result<ResolvedConfigValue<String>, ConfigError> {
    if let Some(board) = explicit_board
        .map(str::trim)
        .filter(|board| !board.is_empty())
    {
        return Ok(ResolvedConfigValue {
            value: board.to_owned(),
            source: ConfigValueSource::Flag { name: "--board" },
        });
    }

    if let Ok(board) = std::env::var("KB_BOARD") {
        let board = board.trim();
        if !board.is_empty() {
            return Ok(ResolvedConfigValue {
                value: board.to_owned(),
                source: ConfigValueSource::Env { name: "KB_BOARD" },
            });
        }
    }

    if let Some(path) = nearest_project_config()? {
        let config = read_project_config(&path)?;
        if let Some(board) = config
            .board
            .map(|board| board.trim().to_owned())
            .filter(|board| !board.is_empty())
        {
            return Ok(ResolvedConfigValue {
                value: board,
                source: ConfigValueSource::ProjectConfig { path, key: "board" },
            });
        }
    }

    Ok(ResolvedConfigValue {
        value: "default".to_owned(),
        source: ConfigValueSource::Default,
    })
}

fn path_relative_to_config(config_path: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    config_path
        .parent()
        .map(|parent| parent.join(path.clone()))
        .unwrap_or(path)
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("failed to parse config {path} at {field_path}: {source}")]
    FileParse {
        path: PathBuf,
        field_path: String,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("{0}")]
    Parse(#[source] Box<toml::de::Error>),
    #[error("{0}")]
    Serialize(#[from] toml::ser::Error),
}

impl From<toml::de::Error> for ConfigError {
    fn from(source: toml::de::Error) -> Self {
        Self::Parse(Box::new(source))
    }
}

pub fn default_actor() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "local".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract_fixture(relative: &str) -> serde_json::Value {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        serde_json::from_str(
            &fs::read_to_string(root.join("schemas/fixtures/config").join(relative)).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn project_config_input_fixture_is_produced_by_runtime_config_dto() {
        let config = ProjectConfig {
            board: Some("kanban-tool".to_owned()),
            db: Some(PathBuf::from(".kb/kb.db")),
            vector: Some(VectorConfig::default()),
        };

        assert_eq!(
            serde_json::to_value(config).unwrap(),
            contract_fixture("project-input.v1.valid.json")
        );
    }

    #[test]
    fn project_config_input_fixture_is_consumed_by_real_toml_decoder() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("config.toml");
        fs::write(
            &path,
            r#"board = "kanban-tool"
db = ".kb/kb.db"

[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = 1024
"#,
        )
        .unwrap();

        let decoded = read_project_config(&path).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            contract_fixture("project-input.v1.valid.json")
        );

        fs::write(&path, "unknown = true\n").unwrap();
        assert!(read_project_config(&path).is_err());
    }

    #[test]
    fn worker_profiles_input_fixture_is_produced_by_runtime_config_dto() {
        let profiles = WorkerProfilesInput {
            workers: [(
                "default".to_owned(),
                WorkerProfileInput {
                    command: Some("echo $KB_TASK_ID".to_owned()),
                    claim_ttl_ms: Some(300_000),
                    heartbeat_interval_ms: Some(30_000),
                    on_success: Some(WorkerFinishPolicy::Done),
                    on_failure: Some(WorkerFinishPolicy::Blocked),
                    log_dir: Some(PathBuf::from(".kb/logs")),
                },
            )]
            .into_iter()
            .collect(),
        };

        assert_eq!(
            serde_json::to_value(profiles).unwrap(),
            contract_fixture("worker-profiles-input.v1.valid.json")
        );
    }

    #[test]
    fn worker_profiles_input_fixture_is_consumed_by_real_toml_decoder() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("workers.toml");
        fs::write(
            &path,
            r#"[workers.default]
command = "echo $KB_TASK_ID"
claim_ttl_ms = 300000
heartbeat_interval_ms = 30000
on_success = "done"
on_failure = "blocked"
log_dir = ".kb/logs"
"#,
        )
        .unwrap();

        let decoded = read_worker_profiles(&path).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            contract_fixture("worker-profiles-input.v1.valid.json")
        );

        fs::write(&path, "[workers.default]\nunknown = true\n").unwrap();
        assert!(read_worker_profiles(&path).is_err());
        fs::write(&path, "[not_workers]\nunknown = true\n").unwrap();
        assert!(read_worker_profiles(&path).is_err());
    }

    #[test]
    fn selected_worker_profile_ignores_unselected_profile_extensions() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("workers.toml");
        fs::write(
            &path,
            r#"[workers.backend]
command = "echo backend"
claim_ttl_ms = 300000

[workers.future]
concurrency = 2
max_runtime_ms = 3600000
"#,
        )
        .unwrap();

        let profile = read_worker_profile(&path, "backend").unwrap().unwrap();
        assert_eq!(profile.command.as_deref(), Some("echo backend"));
        assert_eq!(profile.claim_ttl_ms, Some(300_000));
    }

    #[test]
    fn selected_worker_profile_preserves_selected_field_path() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("workers.toml");
        fs::write(
            &path,
            r#"[workers.backend]
concurrency = 2
"#,
        )
        .unwrap();

        let error = read_worker_profile(&path, "backend").unwrap_err();
        assert!(
            error.to_string().contains("workers.backend.concurrency"),
            "{error}"
        );

        fs::write(
            &path,
            r#"[workers.backend]
on_success = "future"
"#,
        )
        .unwrap();
        let error = read_worker_profile(&path, "backend").unwrap_err();
        assert!(
            error.to_string().contains("workers.backend.on_success"),
            "{error}"
        );
    }

    #[test]
    fn project_config_round_trips_vector_settings_and_preserves_them_on_board_update() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join(".kb").join("config.toml");
        let vector = VectorConfig {
            provider: "ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            model: "qwen3-embedding:0.6b".to_owned(),
            dimensions: 1024,
        };

        write_project_config(
            &path,
            &ProjectConfig {
                board: Some("kanban-tool".to_owned()),
                db: Some(PathBuf::from("kb.db")),
                vector: Some(vector.clone()),
            },
        )
        .unwrap();
        write_active_board_config_at(&path, "next-board").unwrap();

        let config = read_project_config(&path).unwrap();
        assert_eq!(config.board.as_deref(), Some("next-board"));
        assert_eq!(config.db, Some(PathBuf::from("kb.db")));
        assert_eq!(config.vector, Some(vector));
    }

    #[test]
    fn explicit_project_vector_config_overrides_global_config() {
        let tempdir = tempfile::tempdir().unwrap();
        let global = tempdir.path().join("global.toml");
        let project = tempdir.path().join("project.toml");
        let explicit = tempdir.path().join("explicit.toml");
        write_vector_config_at(
            &global,
            VectorConfig {
                model: "global".to_owned(),
                ..VectorConfig::default()
            },
        )
        .unwrap();
        write_vector_config_at(
            &project,
            VectorConfig {
                model: "project".to_owned(),
                ..VectorConfig::default()
            },
        )
        .unwrap();
        write_vector_config_at(
            &explicit,
            VectorConfig {
                model: "explicit".to_owned(),
                ..VectorConfig::default()
            },
        )
        .unwrap();

        assert_eq!(
            read_project_config(&explicit)
                .unwrap()
                .vector
                .unwrap()
                .model,
            "explicit"
        );
        assert_eq!(
            read_project_config(&project).unwrap().vector.unwrap().model,
            "project"
        );
        assert_eq!(
            read_project_config(&global).unwrap().vector.unwrap().model,
            "global"
        );
    }

    #[test]
    fn project_config_parse_error_includes_file_and_field_path() {
        use assert_fs::prelude::*;

        let tempdir = assert_fs::TempDir::new().unwrap();
        let config_file = tempdir.child(".kb/config.toml");
        config_file
            .write_str(
                r#"
[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "qwen3-embedding:0.6b"
dimensions = "large"
"#,
            )
            .unwrap();
        let path = config_file.path();

        let error = read_project_config(path).unwrap_err().to_string();

        assert!(error.contains(path.to_string_lossy().as_ref()), "{error}");
        assert!(error.contains("vector.dimensions"), "{error}");
    }

    #[test]
    fn default_paths_match_kb_data_layout() {
        let db_path = default_db_path();
        assert!(db_path.ends_with("kb/kb.db"));

        let log_dir = default_log_dir();
        assert!(log_dir.ends_with("kb/logs"));
    }

    #[test]
    fn global_config_path_uses_kanban_dir() {
        let root = PathBuf::from("/home/alice/.config");

        assert_eq!(
            global_config_dir_from_root(root.clone()),
            PathBuf::from("/home/alice/.config/kanban")
        );
    }

    #[test]
    fn resolved_global_vector_config_reads_kanban_path() {
        let tempdir = tempfile::tempdir().unwrap();
        let global = tempdir.path().join("kanban").join("config.toml");
        write_vector_config_at(
            &global,
            VectorConfig {
                model: "new-global".to_owned(),
                ..VectorConfig::default()
            },
        )
        .unwrap();

        let config = resolved_global_vector_config(&global).unwrap().unwrap();

        assert_eq!(config.model, "new-global");
    }

    #[test]
    fn resolved_global_vector_config_ignores_legacy_kb_path() {
        let tempdir = tempfile::tempdir().unwrap();
        let global = tempdir.path().join("kanban").join("config.toml");
        let legacy = tempdir.path().join("kb").join("config.toml");
        write_vector_config_at(
            &legacy,
            VectorConfig {
                model: "legacy-global".to_owned(),
                ..VectorConfig::default()
            },
        )
        .unwrap();

        let config = resolved_global_vector_config(&global).unwrap();

        assert_eq!(config, None);
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
