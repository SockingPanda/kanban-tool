//! CLI 与 `serve` 共用的本地配置解析和安全文件写入。
//!
//! 配置只决定路径和选择，不拥有 Turso 连接。数据库的打开、初始化和迁移仍由
//! `kanban serve` 独占；普通 CLI 命令最多读取 `.kb/config.toml`。

use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use kanban_core::Locale;
use kanban_protocol::ProjectConfigInput;
use serde::{Deserialize, Serialize};

pub(crate) const USER_CONFIG_DIR_NAME: &str = "kanban";
pub(crate) const DEFAULT_BOARD: &str = "default";

/// 保留已知配置字段，同时保留尚未被当前 CLI 理解的用户字段。
///
/// `vector` 使用 contract 中的严格 shape，使坏配置在 config inspection 阶段尽早失败；
/// 其它扩展字段通过 `flatten` 原样 round-trip，`board use` 不会抹掉用户设置。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ProjectConfig {
    pub(crate) board: Option<String>,
    pub(crate) db: Option<PathBuf>,
    pub(crate) vector: Option<ProjectVectorConfig>,
    #[serde(flatten)]
    pub(crate) extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectVectorConfig {
    pub(crate) provider: String,
    pub(crate) endpoint: String,
    pub(crate) model: String,
    pub(crate) dimensions: usize,
}

impl From<ProjectConfigInput> for ProjectConfig {
    fn from(value: ProjectConfigInput) -> Self {
        Self {
            board: value.board,
            db: value.db,
            vector: value.vector.map(|vector| ProjectVectorConfig {
                provider: vector.provider,
                endpoint: vector.endpoint,
                model: vector.model,
                dimensions: vector.dimensions,
            }),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved<T> {
    pub(crate) value: T,
    pub(crate) source: ConfigValueSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConfigValueSource {
    Flag { name: &'static str },
    Env { name: &'static str },
    ProjectConfig { path: PathBuf, key: &'static str },
    GlobalConfig { path: PathBuf, key: &'static str },
    Default,
}

#[derive(Debug)]
pub(crate) enum ConfigError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        field_path: String,
        message: String,
    },
    Serialize {
        path: PathBuf,
        message: String,
    },
}

impl From<io::Error> for ConfigError {
    fn from(source: io::Error) -> Self {
        Self::Io {
            path: PathBuf::from(".kb/config.toml"),
            source,
        }
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "访问配置 {} 失败：{source}", path.display())
            }
            Self::Parse {
                path,
                field_path,
                message,
            } => write!(
                formatter,
                "解析配置 {} 的 {field_path} 失败：{message}",
                path.display()
            ),
            Self::Serialize { path, message } => {
                write!(formatter, "序列化配置 {} 失败：{message}", path.display())
            }
        }
    }
}

impl std::error::Error for ConfigError {}

pub(crate) fn resolve_db_path(explicit: Option<&Path>) -> Result<Resolved<PathBuf>, ConfigError> {
    if let Some(path) = explicit {
        return Ok(Resolved {
            value: path.to_path_buf(),
            source: ConfigValueSource::Flag { name: "--db" },
        });
    }
    for name in ["KANBAN_DB", "KB_DB"] {
        if let Some(path) = non_empty_env_path(name) {
            return Ok(Resolved {
                value: path,
                source: ConfigValueSource::Env { name },
            });
        }
    }

    if let Some(path) = nearest_project_config()? {
        let config = read_project_config(&path)?;
        if let Some(db) = non_empty_path(config.db) {
            return Ok(Resolved {
                value: path_relative_to_config(&path, db),
                source: ConfigValueSource::ProjectConfig { path, key: "db" },
            });
        }
    }

    let global = global_config_path();
    if global.is_file() {
        let config = read_project_config(&global)?;
        if let Some(db) = non_empty_path(config.db) {
            return Ok(Resolved {
                value: path_relative_to_config(&global, db),
                source: ConfigValueSource::GlobalConfig {
                    path: global,
                    key: "db",
                },
            });
        }
    }

    Ok(Resolved {
        value: default_db_path(),
        source: ConfigValueSource::Default,
    })
}

pub(crate) fn resolve_board(explicit: Option<&str>) -> Result<Resolved<String>, ConfigError> {
    if let Some(board) = non_empty_string(explicit) {
        return Ok(Resolved {
            value: board,
            source: ConfigValueSource::Flag { name: "--board" },
        });
    }
    if let Some(board) = env::var_os("KB_BOARD").and_then(|value| value.into_string().ok())
        && let Some(board) = non_empty_string(Some(&board))
    {
        return Ok(Resolved {
            value: board,
            source: ConfigValueSource::Env { name: "KB_BOARD" },
        });
    }
    if let Some(path) = nearest_project_config()? {
        let config = read_project_config(&path)?;
        if let Some(board) = non_empty_string(config.board.as_deref()) {
            return Ok(Resolved {
                value: board,
                source: ConfigValueSource::ProjectConfig { path, key: "board" },
            });
        }
    }
    Ok(Resolved {
        value: DEFAULT_BOARD.to_owned(),
        source: ConfigValueSource::Default,
    })
}

pub(crate) fn resolve_locale(explicit: Option<&str>) -> Result<ResolvedLocale, String> {
    let (input, source) = if let Some(flag) = explicit {
        (
            Some(flag.trim().to_owned()),
            ConfigValueSource::Flag { name: "--locale" },
        )
    } else if let Ok(value) = env::var("KANBAN_LOCALE") {
        (
            Some(value.trim().to_owned()),
            ConfigValueSource::Env {
                name: "KANBAN_LOCALE",
            },
        )
    } else {
        (None, ConfigValueSource::Default)
    };
    let value = Locale::explicit_or_system(input.as_deref())
        .map_err(|error| error.to_owned())?
        .as_str()
        .to_owned();
    Ok(ResolvedLocale {
        value,
        input,
        source,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedLocale {
    pub(crate) value: String,
    pub(crate) input: Option<String>,
    pub(crate) source: ConfigValueSource,
}

pub(crate) fn default_db_path() -> PathBuf {
    data_local_dir().join("kb").join("kanban.db")
}

pub(crate) fn global_config_path() -> PathBuf {
    config_dir().join(USER_CONFIG_DIR_NAME).join("config.toml")
}

pub(crate) fn prompt_config_path() -> PathBuf {
    config_dir()
        .join(USER_CONFIG_DIR_NAME)
        .join("codex-hooks.json")
}

pub(crate) fn nearest_project_config() -> io::Result<Option<PathBuf>> {
    let mut directory = env::current_dir()?;
    loop {
        let candidate = directory.join(".kb").join("config.toml");
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
        if !directory.pop() {
            return Ok(None);
        }
    }
}

pub(crate) fn project_config_path_for_write() -> io::Result<PathBuf> {
    Ok(nearest_project_config()?.unwrap_or_else(|| {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".kb")
            .join("config.toml")
    }))
}

pub(crate) fn read_project_config(path: &Path) -> Result<ProjectConfig, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let deserializer = toml::Deserializer::new(&text);
    serde_path_to_error::deserialize(deserializer).map_err(|error| ConfigError::Parse {
        path: path.to_path_buf(),
        field_path: error.path().to_string(),
        message: error.into_inner().to_string(),
    })
}

pub(crate) fn write_project_config_atomic(
    path: &Path,
    config: &ProjectConfig,
) -> Result<bool, ConfigError> {
    let text = toml::to_string_pretty(config).map_err(|error| ConfigError::Serialize {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    atomic_write(path, format!("{text}\n").as_bytes()).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// 初始化项目选择文件，不触碰数据库。重复执行只会复用已有内容。
pub(crate) fn init_project_config() -> Result<(PathBuf, bool, String), ConfigError> {
    let path = project_config_path_for_write().map_err(|source| ConfigError::Io {
        path: PathBuf::from(".kb/config.toml"),
        source,
    })?;
    let mut config = if path.is_file() {
        read_project_config(&path)?
    } else {
        ProjectConfig::default()
    };
    let Some(board) = non_empty_string(config.board.as_deref()) else {
        config.board = Some(DEFAULT_BOARD.to_owned());
        let created = write_project_config_atomic(&path, &config)?;
        return Ok((path, created, DEFAULT_BOARD.to_owned()));
    };
    if path.is_file() {
        return Ok((path, false, board));
    }
    let created = write_project_config_atomic(&path, &config)?;
    Ok((path, created, board))
}

pub(crate) fn write_active_board(board: &str) -> Result<ActiveBoardWrite, ConfigError> {
    let board = non_empty_string(Some(board)).ok_or_else(|| ConfigError::Parse {
        path: PathBuf::from(".kb/config.toml"),
        field_path: "board".to_owned(),
        message: "board 不能为空".to_owned(),
    })?;
    let path = project_config_path_for_write().map_err(|source| ConfigError::Io {
        path: PathBuf::from(".kb/config.toml"),
        source,
    })?;
    write_active_board_at(&path, &board)
}

pub(crate) fn write_active_board_at(
    path: &Path,
    board: &str,
) -> Result<ActiveBoardWrite, ConfigError> {
    let mut config = if path.is_file() {
        read_project_config(path)?
    } else {
        ProjectConfig::default()
    };
    let created = !path.exists();
    let previous = config
        .board
        .as_deref()
        .and_then(|value| non_empty_string(Some(value)));
    if !created && previous.as_deref() == Some(board) {
        return Ok(ActiveBoardWrite {
            path: path.to_path_buf(),
            created: false,
            updated: false,
        });
    }
    config.board = Some(board.to_owned());
    let changed = write_project_config_atomic(path, &config)?;
    Ok(ActiveBoardWrite {
        path: path.to_path_buf(),
        created,
        updated: changed && previous.as_deref() != Some(board),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveBoardWrite {
    pub(crate) path: PathBuf,
    pub(crate) created: bool,
    pub(crate) updated: bool,
}

pub(crate) fn atomic_write(path: &Path, content: &[u8]) -> io::Result<bool> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && metadata.file_type().is_symlink()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("拒绝替换符号链接配置路径 {}", path.display()),
        ));
    }
    if let Ok(existing) = fs::read(path)
        && existing == content
    {
        return Ok(false);
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent_existed = fs::symlink_metadata(parent).is_ok();
    fs::create_dir_all(parent)?;
    if !parent_existed {
        set_private_directory(parent)?;
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config"),
        std::process::id(),
        nonce
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options.open(&temp_path)?;
        set_private_file(&file)?;
        file.write_all(content)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)?;
        sync_directory(parent)?;
        Ok(true)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn set_private_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn set_private_file(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = file.metadata()?.permissions();
        permissions.set_mode(0o600);
        file.set_permissions(permissions)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()?;
    }
    Ok(())
}

fn config_dir() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(dirs_next::config_dir)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn data_local_dir() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(dirs_next::data_local_dir)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn non_empty_env_path(name: &'static str) -> Option<PathBuf> {
    env::var_os(name)
        .map(PathBuf::from)
        .and_then(|path| non_empty_path(Some(path)))
}

fn non_empty_path(path: Option<PathBuf>) -> Option<PathBuf> {
    path.filter(|path| !path.as_os_str().is_empty() && !path.to_string_lossy().trim().is_empty())
}

fn non_empty_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn path_relative_to_config(config_path: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        config_path
            .parent()
            .map(|parent| parent.join(&path))
            .unwrap_or(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_relative_to_config_anchors_relative_values() {
        let path = PathBuf::from("/tmp/project/.kb/config.toml");
        assert_eq!(
            path_relative_to_config(&path, PathBuf::from("db.turso")),
            PathBuf::from("/tmp/project/.kb/db.turso")
        );
    }

    #[test]
    fn source_round_trip_keeps_unknown_config_fields() {
        let value: ProjectConfig = toml::from_str("board = \"default\"\ncustom = true\n").unwrap();
        assert_eq!(value.extra.get("custom"), Some(&toml::Value::Boolean(true)));
    }

    #[test]
    fn active_board_write_is_atomic_idempotent_and_preserves_fields() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".kb/config.toml");
        let existing = "db = \"./project.db\"\nboard = \"old\"\ncustom = true\n";
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, existing).unwrap();

        let result = write_active_board_at(&path, "new").unwrap();
        assert!(!result.created);
        assert!(result.updated);
        let text = fs::read_to_string(&path).unwrap();
        let parsed = read_project_config(&path).unwrap();
        assert_eq!(parsed.board.as_deref(), Some("new"));
        assert_eq!(parsed.db, Some(PathBuf::from("./project.db")));
        assert_eq!(
            parsed.extra.get("custom"),
            Some(&toml::Value::Boolean(true))
        );
        assert!(!text.contains("tmp-"));

        let bytes = fs::read(&path).unwrap();
        let repeat = write_active_board_at(&path, "new").unwrap();
        assert!(!repeat.created);
        assert!(!repeat.updated);
        assert_eq!(bytes, fs::read(&path).unwrap());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
