use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use fs_err as fs;
use fs4::fs_std::FileExt as _;
use serde::{Deserialize, Serialize, de::IntoDeserializer};

pub use kanban_contract::{
    ProjectConfigInput as ProjectConfig, ProjectVectorConfigInput as VectorConfig,
    WorkerFinishPolicy, WorkerProfileInput,
};

pub const INDEX_LAYOUT_VERSION: &str = "v1";
pub const PROJECTION_INDEX_LAYOUT_VERSION: &str = "v2";
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

#[derive(Debug)]
pub struct DerivedStoreWriteGuard {
    lock_file: std::fs::File,
}

static DERIVED_LOCK_NONCE: AtomicU64 = AtomicU64::new(1);
static DURABLE_ENTRY_NONCE: AtomicU64 = AtomicU64::new(1);

impl DerivedStoreWriteGuard {
    pub fn acquire(db_path: &Path, store_name: &str) -> io::Result<Self> {
        if store_name.is_empty()
            || !store_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "derived store name is not lock-path safe",
            ));
        }
        let lock_path = derived_store_write_lock_path(db_path, store_name);
        let mut lock_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)?;
        if !lock_file.try_lock_exclusive()? {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "derived store {store_name} has an active physical writer: {}",
                    lock_path.display()
                ),
            ));
        }
        let identity = derived_store_lock_identity(store_name);
        lock_file.set_len(0)?;
        lock_file.write_all(identity.as_bytes())?;
        lock_file.sync_data()?;
        Ok(Self { lock_file })
    }
}

impl Drop for DerivedStoreWriteGuard {
    fn drop(&mut self) {
        let _ = self.lock_file.unlock();
    }
}

/// Flushes one regular file to its backing store.
///
/// Callers should use [`durable_sync_directory`] after creating, replacing, or
/// removing a directory entry. The two barriers deliberately remain separate
/// so a failed file flush can never be mistaken for a completed publish.
pub fn durable_sync_file(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "durability file path is not a regular file: {}",
                path.display()
            ),
        ));
    }
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?
        .sync_all()
}

/// Flushes directory-entry changes without hiding platform errors.
///
/// Unix exposes a real directory `fsync`, so failures are returned to the
/// caller. Rust's standard library does not expose a portable Windows
/// directory flush. On Windows this function therefore validates that the
/// directory exists and is accessible. Directory-entry mutations in this
/// module use `MoveFileExW(MOVEFILE_WRITE_THROUGH)` separately; this
/// validation-only helper must not be treated as their durability barrier.
pub fn durable_sync_directory(path: &Path) -> io::Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "durability directory path is not a directory: {}",
                path.display()
            ),
        ));
    }
    durable_sync_directory_platform(path)
}

#[cfg(unix)]
fn durable_sync_directory_platform(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

#[cfg(windows)]
fn durable_sync_directory_platform(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn durable_sync_directory_platform(path: &Path) -> io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

/// Flushes every regular file and directory in a staged artifact tree.
///
/// Symlinks and other special file types are rejected because following them
/// would make the physical generation's durability boundary ambiguous.
pub fn durable_sync_directory_tree(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "durability tree root is not a directory: {}",
                path.display()
            ),
        ));
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            durable_sync_directory_tree(&entry.path())?;
        } else if file_type.is_file() {
            durable_sync_file(&entry.path())?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "durability tree contains an unsupported entry: {}",
                    entry.path().display()
                ),
            ));
        }
    }
    durable_sync_directory(path)
}

/// Replaces a regular file from a sibling staged file and persists the parent
/// directory entry. The sibling requirement preserves atomic rename semantics.
pub fn durable_replace_file(staged: &Path, destination: &Path) -> io::Result<()> {
    require_sibling_paths(staged, destination)?;
    let metadata = fs::symlink_metadata(staged)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("staged durability path is not a file: {}", staged.display()),
        ));
    }
    match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "durability file destination is not a regular file: {}",
                    destination.display()
                ),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    durable_sync_file(staged)?;
    durable_replace_file_platform(staged, destination)?;
    durable_sync_directory(parent_directory(destination)?)
}

/// Writes and durably replaces one regular file through an unpredictable
/// sibling created with `create_new`.
///
/// Callers must not construct fixed `.tmp` names: an attacker or interrupted
/// process could leave a symlink at such a path and redirect the write before
/// [`durable_replace_file`] gets a chance to validate it.
pub fn durable_replace_file_contents(path: &Path, contents: &[u8]) -> io::Result<()> {
    let staged = unique_sibling_path(path, "replace")?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    let staged_write = (|| {
        file.write_all(contents)?;
        file.sync_all()
    })();
    drop(file);
    if let Err(error) = staged_write {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    if let Err(error) = durable_replace_file(&staged, path) {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    Ok(())
}

#[cfg(not(windows))]
fn durable_replace_file_platform(staged: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(staged, destination)
}

#[cfg(windows)]
fn durable_replace_file_platform(staged: &Path, destination: &Path) -> io::Result<()> {
    windows_move_file(staged, destination, true)
}

/// Publishes a complete staged directory as a new generation.
///
/// Existing destinations are refused: replacing an active directory is not a
/// portable atomic operation and generation publication must never overwrite
/// physical evidence.
pub fn durable_publish_directory(staged: &Path, destination: &Path) -> io::Result<()> {
    require_sibling_paths(staged, destination)?;
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "durable directory destination already exists: {}",
                    destination.display()
                ),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    durable_sync_directory_tree(staged)?;
    durable_publish_directory_platform(staged, destination)?;
    durable_sync_directory(parent_directory(destination)?)
}

#[cfg(not(windows))]
fn durable_publish_directory_platform(staged: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(staged, destination)
}

#[cfg(windows)]
fn durable_publish_directory_platform(staged: &Path, destination: &Path) -> io::Result<()> {
    windows_move_file(staged, destination, false)
}

/// Creates and flushes a file that must not already exist, then publishes it
/// from a sibling staged file. This prevents a short write from leaving a
/// truncated authoritative marker.
pub fn durable_create_new_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "durable file destination already exists: {}",
                    path.display()
                ),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let staged = unique_sibling_path(path, "new")?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)?;
    let staged_write = (|| {
        file.write_all(contents)?;
        file.sync_all()
    })();
    drop(file);
    if let Err(error) = staged_write {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    if let Err(error) = durable_publish_new_file_platform(&staged, path) {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    let parent = parent_directory(path)?;
    durable_sync_directory(parent)?;
    if staged.exists() {
        fs::remove_file(&staged)?;
        durable_sync_directory(parent)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn durable_publish_new_file_platform(staged: &Path, destination: &Path) -> io::Result<()> {
    fs::hard_link(staged, destination)
}

#[cfg(windows)]
fn durable_publish_new_file_platform(staged: &Path, destination: &Path) -> io::Result<()> {
    windows_move_file(staged, destination, false)
}

/// Creates every missing directory through a sibling staged-directory publish.
pub fn durable_create_dir_all(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => return Ok(()),
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "durable directory path is not a directory: {}",
                    path.display()
                ),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let parent = parent_directory(path)?;
    durable_create_dir_all(parent)?;
    let staged = unique_sibling_path(path, "mkdir")?;
    fs::create_dir(&staged)?;
    match durable_publish_directory(&staged, path) {
        Ok(()) => Ok(()),
        Err(error)
            if error.kind() == io::ErrorKind::AlreadyExists
                && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir()) =>
        {
            let _ = fs::remove_dir(&staged);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_dir(&staged);
            Err(error)
        }
    }
}

/// Moves one invalid directory entry aside without following it.
///
/// The returned sibling path preserves crash/corruption evidence for later
/// cleanup while removing the entry from the authoritative namespace.
pub fn durable_quarantine_entry(path: &Path) -> io::Result<PathBuf> {
    fs::symlink_metadata(path)?;
    let quarantined = unique_sibling_path(path, "quarantine")?;
    durable_move_entry_no_replace_platform(path, &quarantined)?;
    durable_sync_directory(parent_directory(path)?)?;
    Ok(quarantined)
}

/// Removes an unpublished directory after first durably moving it out of the
/// authoritative namespace.
pub fn durable_remove_directory(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "durable removal path is not a directory: {}",
                path.display()
            ),
        ));
    }
    let quarantined = durable_quarantine_entry(path)?;
    fs::remove_dir_all(&quarantined)?;
    durable_sync_directory(parent_directory(&quarantined)?)
}

#[cfg(not(windows))]
fn durable_move_entry_no_replace_platform(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn durable_move_entry_no_replace_platform(source: &Path, destination: &Path) -> io::Result<()> {
    windows_move_file(source, destination, false)
}

fn unique_sibling_path(path: &Path, purpose: &str) -> io::Result<PathBuf> {
    let parent = parent_directory(path)?;
    let name = path
        .file_name()
        .unwrap_or_else(|| OsStr::new("entry"))
        .to_string_lossy();
    for _ in 0..1_024 {
        let nonce = DURABLE_ENTRY_NONCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{name}.{purpose}.{}.{nonce}", std::process::id()));
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not allocate a durable sibling path for {}",
            path.display()
        ),
    ))
}

#[cfg(windows)]
fn windows_move_file(staged: &Path, destination: &Path, replace: bool) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
        let mut encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
        if encoded.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Windows durability path contains an interior NUL",
            ));
        }
        encoded.push(0);
        Ok(encoded)
    }

    let staged = wide_path(staged)?;
    let destination = wide_path(destination)?;
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    // SAFETY: both buffers are NUL-terminated UTF-16 paths and remain alive
    // for the call. All callers keep moves on one filesystem.
    let moved = unsafe { MoveFileExW(staged.as_ptr(), destination.as_ptr(), flags) };
    if moved == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn require_sibling_paths(left: &Path, right: &Path) -> io::Result<()> {
    if left == right || left.parent() != right.parent() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "durable rename requires distinct sibling paths",
        ));
    }
    Ok(())
}

fn parent_directory(path: &Path) -> io::Result<&Path> {
    path.parent()
        .map(|parent| {
            if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            }
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("durability path has no parent: {}", path.display()),
            )
        })
}

pub fn derived_store_write_lock_path(db_path: &Path, store_name: &str) -> PathBuf {
    let normalized = normalized_file_path(db_path);
    PathBuf::from(format!(
        "{}.derived.{store_name}.lock",
        normalized.display()
    ))
}

fn normalized_file_path(path: &Path) -> PathBuf {
    if path.exists()
        && let Ok(canonical) = std::fs::canonicalize(path)
    {
        return canonical;
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().unwrap_or_default();
    if let Ok(canonical_parent) = std::fs::canonicalize(parent) {
        return canonical_parent.join(file_name);
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn derived_store_lock_identity(store_name: &str) -> String {
    let pid = std::process::id();
    let nonce = DERIVED_LOCK_NONCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("pid={pid}\nowner={pid}-{timestamp}-{nonce}\nstore={store_name}\n")
}

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

pub fn projection_store_root_path(
    db_path: impl Into<PathBuf>,
    database_instance_id: &str,
    store_name: &str,
) -> io::Result<PathBuf> {
    validate_projection_database_instance_id(database_instance_id)?;
    validate_projection_path_component(store_name, "derived store name")?;
    Ok(projection_data_root(db_path.into())?
        .join("index")
        .join(PROJECTION_INDEX_LAYOUT_VERSION)
        .join("databases")
        .join(database_instance_id)
        .join(store_name))
}

/// Resolves and validates the complete managed path to a Projection v2
/// generations directory without following any managed symlink.
///
/// The canonical database parent is the trust anchor. Missing descendants are
/// allowed so read-only inspection can report an absent store, but every
/// existing managed component must be a real directory.
pub fn checked_projection_store_generations_path(
    db_path: impl Into<PathBuf>,
    database_instance_id: &str,
    store_name: &str,
) -> io::Result<PathBuf> {
    projection_store_generations_path(db_path.into(), database_instance_id, store_name, false)
}

/// Resolves, validates, and durably creates the complete managed path to a
/// Projection v2 generations directory.
pub fn ensure_projection_store_generations_path(
    db_path: impl Into<PathBuf>,
    database_instance_id: &str,
    store_name: &str,
) -> io::Result<PathBuf> {
    projection_store_generations_path(db_path.into(), database_instance_id, store_name, true)
}

/// Joins one validated Projection v2 generation id beneath an already checked
/// generations directory.
pub fn projection_generation_path(
    generations_path: &Path,
    generation: &str,
) -> io::Result<PathBuf> {
    if !generation.starts_with("gen_") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "projection generation id must start with gen_",
        ));
    }
    validate_projection_path_component(generation, "projection generation id")?;
    Ok(generations_path.join(generation))
}

fn projection_store_generations_path(
    db_path: PathBuf,
    database_instance_id: &str,
    store_name: &str,
    create_missing: bool,
) -> io::Result<PathBuf> {
    validate_projection_database_instance_id(database_instance_id)?;
    validate_projection_path_component(store_name, "derived store name")?;
    let data_root = projection_data_root(db_path)?;
    let mut current = data_root;
    let mut ancestor_missing = false;
    for component in [
        "index",
        PROJECTION_INDEX_LAYOUT_VERSION,
        "databases",
        database_instance_id,
        store_name,
        "generations",
    ] {
        current.push(component);
        if ancestor_missing {
            if create_missing {
                durable_create_dir_all(&current)?;
            }
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "managed projection path component is not a directory: {}",
                        current.display()
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                ancestor_missing = true;
                if create_missing {
                    durable_create_dir_all(&current)?;
                    ancestor_missing = false;
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(current)
}

fn projection_data_root(db_path: PathBuf) -> io::Result<PathBuf> {
    match fs::canonicalize(&db_path) {
        Ok(canonical_db) => canonical_db.parent().map(Path::to_path_buf).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "canonical database path has no parent: {}",
                    canonical_db.display()
                ),
            )
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::canonicalize(kb_data_dir_for_db(db_path))
        }
        Err(error) => Err(error),
    }
}

fn validate_projection_database_instance_id(value: &str) -> io::Result<()> {
    if !value.starts_with("db_") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "database instance id is not projection-path safe",
        ));
    }
    validate_projection_path_component(value, "database instance id")
}

fn validate_projection_path_component(value: &str, label: &str) -> io::Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is not projection-path safe"),
        ));
    }
    Ok(())
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

    #[test]
    fn durable_file_replace_syncs_and_atomically_replaces_contents() {
        let tempdir = tempfile::tempdir().unwrap();
        let destination = tempdir.path().join("metadata.json");
        let staged = tempdir.path().join("metadata.json.tmp");
        fs::write(&destination, "old").unwrap();
        fs::write(&staged, "new").unwrap();

        durable_replace_file(&staged, &destination).unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
        assert!(!staged.exists());
    }

    #[cfg(unix)]
    #[test]
    fn durable_file_contents_replace_ignores_fixed_temp_symlink() {
        use std::os::unix::fs::symlink;

        let tempdir = tempfile::tempdir().unwrap();
        let destination = tempdir.path().join("metadata.json");
        let external = tempdir.path().join("external-sentinel");
        let fixed_temp = tempdir.path().join("metadata.json.tmp");
        fs::write(&destination, "old").unwrap();
        fs::write(&external, "must-remain").unwrap();
        symlink(&external, &fixed_temp).unwrap();

        durable_replace_file_contents(&destination, b"new").unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
        assert_eq!(fs::read_to_string(&external).unwrap(), "must-remain");
        assert!(
            fs::symlink_metadata(&fixed_temp)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn durable_create_new_file_is_a_one_way_publish_marker() {
        let tempdir = tempfile::tempdir().unwrap();
        let marker = tempdir.path().join("published");

        durable_create_new_file(&marker, b"generation=one\n").unwrap();

        assert_eq!(fs::read(&marker).unwrap(), b"generation=one\n");
        let error = durable_create_new_file(&marker, b"generation=two\n").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&marker).unwrap(), b"generation=one\n");
    }

    #[test]
    fn durable_directory_publish_syncs_tree_and_refuses_existing_destination() {
        let tempdir = tempfile::tempdir().unwrap();
        let staged = tempdir.path().join("generation.tmp");
        let destination = tempdir.path().join("generation");
        fs::create_dir_all(staged.join("nested")).unwrap();
        fs::write(staged.join("nested").join("artifact"), "ready").unwrap();

        durable_publish_directory(&staged, &destination).unwrap();

        assert_eq!(
            fs::read_to_string(destination.join("nested").join("artifact")).unwrap(),
            "ready"
        );
        assert!(!staged.exists());

        let next = tempdir.path().join("next.tmp");
        fs::create_dir(&next).unwrap();
        let error = durable_publish_directory(&next, &destination).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(next.exists());
    }

    #[test]
    fn durable_create_dir_all_publishes_each_missing_directory() {
        let tempdir = tempfile::tempdir().unwrap();
        let nested = tempdir.path().join("index/v2/store/generations");

        durable_create_dir_all(&nested).unwrap();

        assert!(nested.is_dir());
        durable_create_dir_all(&nested).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn projection_generations_path_rejects_each_managed_symlink_component() {
        use std::os::unix::fs::symlink;

        for component_depth in 0..6 {
            let tempdir = tempfile::tempdir().unwrap();
            let external = tempfile::tempdir().unwrap();
            let sentinel = external.path().join("sentinel");
            fs::write(&sentinel, b"canonical-outside").unwrap();
            let components = [
                "index",
                "v2",
                "databases",
                "db_test",
                "tantivy_tasks",
                "generations",
            ];
            let mut parent = tempdir.path().to_path_buf();
            for component in &components[..component_depth] {
                parent.push(component);
                fs::create_dir(&parent).unwrap();
            }
            symlink(external.path(), parent.join(components[component_depth])).unwrap();

            let error = checked_projection_store_generations_path(
                tempdir.path().join("kanban.db"),
                "db_test",
                "tantivy_tasks",
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(fs::read(&sentinel).unwrap(), b"canonical-outside");

            let error = ensure_projection_store_generations_path(
                tempdir.path().join("kanban.db"),
                "db_test",
                "tantivy_tasks",
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(fs::read(&sentinel).unwrap(), b"canonical-outside");
        }
    }

    #[test]
    fn projection_generation_path_rejects_traversal_and_noncanonical_ids() {
        let generations = Path::new("/safe/generations");
        assert_eq!(
            projection_generation_path(generations, "../external")
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            projection_generation_path(generations, "pgen_legacy")
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            projection_generation_path(generations, "gen_valid-123").unwrap(),
            generations.join("gen_valid-123")
        );
    }

    #[cfg(unix)]
    #[test]
    fn projection_store_paths_share_namespace_through_database_file_symlink() {
        use std::os::unix::fs::symlink;

        let tempdir = tempfile::tempdir().unwrap();
        let real_parent = tempdir.path().join("real");
        let alias_parent = tempdir.path().join("alias");
        fs::create_dir_all(&real_parent).unwrap();
        fs::create_dir_all(&alias_parent).unwrap();
        let real_db = real_parent.join("kanban.db");
        let alias_db = alias_parent.join("kanban.db");
        fs::write(&real_db, b"sqlite-placeholder").unwrap();
        symlink(&real_db, &alias_db).unwrap();

        let real_root = projection_store_root_path(&real_db, "db_test", "tantivy_tasks").unwrap();
        let alias_root = projection_store_root_path(&alias_db, "db_test", "tantivy_tasks").unwrap();
        assert_eq!(alias_root, real_root);

        let real_generations =
            ensure_projection_store_generations_path(&real_db, "db_test", "tantivy_tasks").unwrap();
        fs::create_dir(real_generations.join("gen_active")).unwrap();
        assert_eq!(
            checked_projection_store_generations_path(&alias_db, "db_test", "tantivy_tasks")
                .unwrap(),
            real_generations
        );
        assert!(real_generations.join("gen_active").is_dir());
    }

    #[test]
    fn durable_quarantine_entry_preserves_corrupt_marker_evidence() {
        let tempdir = tempfile::tempdir().unwrap();
        let marker = tempdir.path().join("published");
        fs::write(&marker, "corrupt").unwrap();

        let quarantined = durable_quarantine_entry(&marker).unwrap();

        assert!(!marker.exists());
        assert_eq!(fs::read_to_string(quarantined).unwrap(), "corrupt");
    }

    #[cfg(unix)]
    #[test]
    fn durable_quarantine_entry_moves_symlink_without_following_target() {
        use std::os::unix::fs::symlink;

        let tempdir = tempfile::tempdir().unwrap();
        let target = tempdir.path().join("canonical");
        let marker = tempdir.path().join("published");
        fs::write(&target, "canonical-evidence").unwrap();
        symlink(&target, &marker).unwrap();

        let quarantined = durable_quarantine_entry(&marker).unwrap();

        assert!(!marker.exists());
        assert!(
            fs::symlink_metadata(&quarantined)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "canonical-evidence");
    }

    #[test]
    fn durable_directory_sync_does_not_swallow_missing_or_wrong_type_errors() {
        let tempdir = tempfile::tempdir().unwrap();
        let missing = tempdir.path().join("missing");
        let error = durable_sync_directory(&missing).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);

        let file = tempdir.path().join("file");
        fs::write(&file, "not a directory").unwrap();
        let error = durable_sync_directory(&file).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn derived_store_lock_file_is_persistent_and_candidate_artifacts_are_ignored() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        let abandoned_candidate =
            PathBuf::from(format!("{}.candidate.reused", lock_path.display()));

        fs::write(&lock_path, "pid=").unwrap();
        fs::write(&abandoned_candidate, "abandoned").unwrap();
        let guard = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();
        let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        drop(guard);

        assert!(lock_path.exists());
        assert_eq!(
            fs::read_to_string(&abandoned_candidate).unwrap(),
            "abandoned"
        );
        drop(DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap());
        assert!(lock_path.exists());
    }

    #[test]
    fn derived_store_lock_serializes_concurrent_contenders() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let guard = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();
        let contenders = (0..16)
            .map(|_| {
                let db_path = db_path.clone();
                std::thread::spawn(move || {
                    DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks")
                        .unwrap_err()
                        .kind()
                })
            })
            .collect::<Vec<_>>();
        for contender in contenders {
            assert_eq!(contender.join().unwrap(), io::ErrorKind::WouldBlock);
        }
        drop(guard);
        drop(DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap());
    }

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
    fn selected_worker_profile_input_fixture_is_produced_by_runtime_config_dto() {
        let profile = WorkerProfileInput {
            command: Some("echo $KB_TASK_ID".to_owned()),
            claim_ttl_ms: Some(300_000),
            heartbeat_interval_ms: Some(30_000),
            on_success: Some(WorkerFinishPolicy::Done),
            on_failure: Some(WorkerFinishPolicy::Blocked),
            log_dir: Some(PathBuf::from(".kb/logs")),
        };

        assert_eq!(
            serde_json::to_value(profile).unwrap(),
            contract_fixture("selected-worker-profile-input.v1.valid.json")
        );
    }

    #[test]
    fn selected_worker_profile_input_fixture_is_consumed_by_real_toml_decoder() {
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

[workers.future]
concurrency = 2
max_runtime_ms = 3600000
"#,
        )
        .unwrap();

        let decoded = read_worker_profile(&path, "default").unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            contract_fixture("selected-worker-profile-input.v1.valid.json")
        );
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
        let message = error.to_string();
        assert!(message.contains("workers.backend.concurrency"), "{message}");
        assert!(message.contains(&path.display().to_string()), "{message}");

        fs::write(
            &path,
            r#"[workers.backend]
on_success = "future"
"#,
        )
        .unwrap();
        let error = read_worker_profile(&path, "backend").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("workers.backend.on_success"), "{message}");
        assert!(message.contains(&path.display().to_string()), "{message}");
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
