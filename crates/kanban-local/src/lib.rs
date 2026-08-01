use std::{
    collections::BTreeMap,
    ffi::OsStr,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use fs_err as fs;
use serde::{Deserialize, Serialize, de::IntoDeserializer};
use sha2::{Digest as _, Sha256};

mod legacy_cleanup;

pub use legacy_cleanup::{
    LEGACY_PROJECTION_ROOTS, LegacyProjectionBackupManifest, LegacyProjectionCleanupError,
    LegacyProjectionCleanupGuard, LegacyProjectionCleanupInventory, LegacyProjectionCleanupOutcome,
    LegacyProjectionRootInventory, LegacyProjectionRootKind,
    acquire_legacy_projection_cleanup_guard, apply_legacy_projection_cleanup,
    apply_legacy_projection_cleanup_with_resume_decision, inventory_legacy_projection_roots,
    restore_legacy_projection_backup, verify_legacy_projection_backup,
};

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

/// Shared physical authority for the lifetime of one canonical SQLite opener.
///
/// The guard is acquired before SQLite opens the path and must outlive the
/// SQLite connection's successful close. Replacement code takes the exclusive
/// counterpart on the same inode byte.
#[derive(Debug)]
pub struct DatabaseLifecycleSharedGuard {
    guard: DatabaseLifecyclePhysicalGuard,
}

/// Exclusive physical authority for a future canonical database replacement.
///
/// This phase exposes only acquisition. Atomic replacement and recovery remain
/// owned by the higher-level lifecycle API.
#[derive(Debug)]
pub struct DatabaseLifecycleExclusiveGuard {
    guard: DatabaseLifecyclePhysicalGuard,
}

/// Exclusive database authority plus every legacy derived-store lock.
///
/// The exclusive lifecycle guard is consumed when this authority is built, so
/// callers cannot accidentally release it before the legacy database-range and
/// sentinel guards. Fields are ordered so legacy guards unlock first and the
/// lifecycle authority unlocks last.
#[derive(Debug)]
pub struct DatabaseLifecycleExclusiveAuthority {
    _store_locks: Vec<DerivedStoreLockSet>,
    lifecycle: DatabaseLifecycleExclusiveGuard,
}

#[derive(Debug)]
struct DatabaseLifecyclePhysicalGuard {
    file: std::fs::File,
    normalized_path: PathBuf,
    lifecycle_locked: bool,
    created_authority_file: bool,
    remove_created_file_on_drop: bool,
}

#[derive(Debug)]
pub struct DerivedStoreWriteGuard {
    _guard: DerivedStorePhysicalGuard,
}

#[derive(Debug)]
pub struct DerivedStoreReadGuard {
    _guard: DerivedStorePhysicalGuard,
}

/// Open-handle identity guard for an existing real directory.
///
/// This does not lock the directory against replacement. Callers use
/// [`validate_path_identity`](Self::validate_path_identity) immediately before
/// and after path-based third-party opens, and retain the guard for as long as
/// those third-party handles may reopen path-relative data.
#[derive(Debug)]
pub struct DirectoryIdentityGuard {
    file: std::fs::File,
    path: PathBuf,
    canonical_path: PathBuf,
    identity: DerivedLockFileIdentity,
}

#[derive(Debug)]
struct DerivedStorePhysicalGuard {
    _store_locks: DerivedStoreLockSet,
    _lifecycle_guard: DatabaseLifecycleSharedGuard,
}

#[derive(Debug)]
struct DerivedStoreLockSet {
    _sentinel_lock: DerivedStoreSentinelLockGuard,
    _database_lock: DerivedStoreDatabaseRangeGuard,
}

#[derive(Debug)]
struct DerivedStoreDatabaseRangeGuard {
    file: std::fs::File,
    offset: u64,
}

#[derive(Debug)]
struct DerivedStoreSentinelLockGuard {
    file: std::fs::File,
}

#[derive(Debug, Clone, Copy)]
enum DerivedStoreLockMode {
    Shared,
    Exclusive,
}

const DERIVED_STORE_SENTINEL_MAGIC: &str = "kanban-derived-store-lock-v2";
const SQLITE_MAXIMUM_FILE_SIZE: u64 = 65_536 * 4_294_967_294;
/// The single database-lifecycle byte sits immediately above SQLite's exact
/// maximum file size (maximum page size × maximum page count).
///
/// Database connection/replacement lifecycle guards own this byte. Public
/// derived-store guards only use offsets beginning at
/// [`DERIVED_STORE_DATABASE_LOCK_BASE`], so they can be nested after a shared
/// lifecycle guard without overlapping its authority.
pub const DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET: u64 = 1 << 48;
const DERIVED_STORE_DATABASE_LOCK_BASE: u64 = DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET + 1;
const _: () = assert!(DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET > SQLITE_MAXIMUM_FILE_SIZE);
const DERIVED_STORE_LOCK_CONTRACT_NAMES: [&str; 8] = [
    "tantivy_tasks",
    "oxigraph_relations",
    "lancedb_chunks",
    "lancedb_label_atoms",
    "tantivy_tasks-projection-helper",
    "oxigraph_relations-projection-helper",
    "lancedb_chunks-projection-helper",
    "lancedb_label_atoms-projection-helper",
];
static DERIVED_STORE_NAMES_BY_OFFSET: OnceLock<Mutex<BTreeMap<u64, String>>> = OnceLock::new();
static DURABLE_ENTRY_NONCE: AtomicU64 = AtomicU64::new(1);

impl DatabaseLifecycleSharedGuard {
    /// Acquires the shared lifecycle byte for an existing canonical database.
    pub fn acquire_existing(path: &Path) -> io::Result<Self> {
        Ok(Self {
            guard: acquire_database_lifecycle_guard(
                path,
                DerivedStoreLockMode::Shared,
                false,
                false,
            )?,
        })
    }

    /// Safely creates a missing database authority, or opens the existing one,
    /// then acquires its shared lifecycle byte.
    pub fn acquire_or_create(path: &Path) -> io::Result<Self> {
        Ok(Self {
            guard: acquire_database_lifecycle_guard(
                path,
                DerivedStoreLockMode::Shared,
                true,
                false,
            )?,
        })
    }

    /// Returns the canonical path whose inode is held by this guard.
    pub fn path(&self) -> &Path {
        &self.guard.normalized_path
    }

    /// Revalidates that the public path still resolves to the held inode.
    pub fn validate_path_identity(&self) -> io::Result<()> {
        self.guard.validate_path_identity()
    }
}

impl DirectoryIdentityGuard {
    pub fn acquire(path: &Path) -> io::Result<Self> {
        let file = open_existing_directory(path)?;
        let identity = snapshot_open_directory(path, &file)?;
        let canonical_path = fs::canonicalize(path)?;
        let guard = Self {
            file,
            path: path.to_path_buf(),
            canonical_path,
            identity,
        };
        guard.validate_path_identity()?;
        Ok(guard)
    }

    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    pub fn validate_path_identity(&self) -> io::Result<()> {
        let handle_identity = snapshot_open_directory(&self.path, &self.file)?;
        let current = open_existing_directory(&self.path)?;
        let path_identity = snapshot_open_directory(&self.path, &current)?;
        let canonical_path = fs::canonicalize(&self.path)?;
        if handle_identity != self.identity
            || path_identity != self.identity
            || canonical_path != self.canonical_path
        {
            return Err(unsafe_directory_path(
                &self.path,
                "directory path no longer identifies the opened authority",
            ));
        }
        Ok(())
    }
}

impl DatabaseLifecycleExclusiveGuard {
    /// Acquires the exclusive lifecycle byte for an existing replacement
    /// target. The handle is writable and delete-share compatible on Windows.
    pub fn acquire_existing_for_replace(path: &Path) -> io::Result<Self> {
        Ok(Self {
            guard: acquire_database_lifecycle_guard(
                path,
                DerivedStoreLockMode::Exclusive,
                false,
                false,
            )?,
        })
    }

    /// Acquires or creates the exclusive replacement authority.
    ///
    /// The caller must already own the stable maintenance namespace fence for
    /// `path`. If this call creates a placeholder, dropping the guard before it
    /// is published removes that placeholder only while it still resolves to
    /// the held inode.
    pub fn acquire_or_create_for_replace(path: &Path) -> io::Result<Self> {
        Ok(Self {
            guard: acquire_database_lifecycle_guard(
                path,
                DerivedStoreLockMode::Exclusive,
                true,
                true,
            )?,
        })
    }

    /// Reports whether this acquisition created its database authority file.
    pub fn created_authority_file(&self) -> bool {
        self.guard.created_authority_file
    }

    /// Marks the held namespace witness for identity-checked removal on drop.
    pub fn mark_remove_file_on_drop(&mut self) {
        self.guard.mark_remove_file_on_drop();
    }

    /// Removes the held replacement authority immediately after revalidating
    /// that `path` still identifies the same inode, then flushes its parent
    /// directory entry. This closes the mark-for-drop window used by
    /// crash-safe placeholder cleanup.
    pub fn remove_file_now_if_identity(&mut self, path: &Path) -> io::Result<()> {
        self.guard.remove_file_now_if_identity(path)
    }

    /// Returns the canonical path whose inode is held by this guard.
    pub fn path(&self) -> &Path {
        &self.guard.normalized_path
    }

    /// Revalidates that the replacement path still resolves to the held inode.
    pub fn validate_path_identity(&self) -> io::Result<()> {
        self.guard.validate_path_identity()
    }

    /// Validates that another namespace path resolves to the held inode.
    pub fn validate_identity_at(&self, path: &Path) -> io::Result<()> {
        self.guard.validate_identity_at(path).map(|_| ())
    }

    /// Rebinds this authority after a caller-controlled atomic rename.
    ///
    /// The new path is validated against the open handle before the stored
    /// namespace identity changes.
    pub fn rebind_after_rename(&mut self, path: &Path) -> io::Result<()> {
        self.guard.rebind_after_rename(path)
    }

    /// Consumes the lifecycle guard and acquires legacy store range/sentinel
    /// guards without recursively taking a shared lifecycle lock.
    pub fn into_derived_store_authority(
        self,
        store_names: &[&str],
    ) -> io::Result<DatabaseLifecycleExclusiveAuthority> {
        let mut store_locks = Vec::with_capacity(store_names.len());
        for store_name in store_names {
            store_locks.push(acquire_derived_store_lock_set(
                self.path(),
                store_name,
                DerivedStoreLockMode::Exclusive,
                true,
            )?);
        }
        self.validate_path_identity()?;
        Ok(DatabaseLifecycleExclusiveAuthority {
            _store_locks: store_locks,
            lifecycle: self,
        })
    }
}

impl DatabaseLifecycleExclusiveAuthority {
    /// Returns the canonical current-database path owned by this authority.
    pub fn path(&self) -> &Path {
        self.lifecycle.path()
    }

    /// Revalidates the current path against the held lifecycle inode.
    pub fn validate_path_identity(&self) -> io::Result<()> {
        self.lifecycle.validate_path_identity()
    }

    /// Validates that another namespace path resolves to the held inode.
    pub fn validate_identity_at(&self, path: &Path) -> io::Result<()> {
        self.lifecycle.validate_identity_at(path)
    }

    /// Rebinds the lifecycle side of this composite authority after rename.
    ///
    /// Legacy range and sentinel guards remain held until this composite is
    /// dropped; only the lifecycle inode's namespace witness changes.
    pub fn rebind_after_rename(&mut self, path: &Path) -> io::Result<()> {
        self.lifecycle.rebind_after_rename(path)
    }

    /// Reports whether the lifecycle acquisition created its authority file.
    pub fn created_authority_file(&self) -> bool {
        self.lifecycle.created_authority_file()
    }

    /// Removes the held namespace witness on drop if it still identifies the
    /// same inode. Used only for a missing-target placeholder retained under
    /// the previous path after a successful replacement recovery.
    pub fn remove_file_on_drop_if_identity(&mut self) {
        self.lifecycle.mark_remove_file_on_drop();
    }

    /// Removes the held replacement authority immediately after revalidating
    /// that `path` still identifies the same inode, then flushes its parent
    /// directory entry.
    pub fn remove_file_now_if_identity(&mut self, path: &Path) -> io::Result<()> {
        self.lifecycle.remove_file_now_if_identity(path)
    }

    /// Drops derived-store locks and returns the lifecycle guard after a
    /// caller has finished an exclusive database inspection.
    pub fn into_lifecycle_guard(self) -> DatabaseLifecycleExclusiveGuard {
        let Self {
            _store_locks,
            lifecycle,
        } = self;
        drop(_store_locks);
        lifecycle
    }
}

impl DatabaseLifecyclePhysicalGuard {
    fn mark_remove_file_on_drop(&mut self) {
        self.remove_created_file_on_drop = true;
    }

    fn remove_file_now_if_identity(&mut self, path: &Path) -> io::Result<()> {
        let normalized_path = self.validate_identity_at(path)?;
        fs::remove_file(&normalized_path)?;
        // Do not leave a delayed drop cleanup armed after the namespace entry
        // has been removed; a later replacement must never be deleted by the
        // old authority's destructor.
        self.remove_created_file_on_drop = false;
        durable_sync_directory(parent_directory(&normalized_path)?)
    }

    fn validate_path_identity(&self) -> io::Result<()> {
        validate_database_lock_file(&self.normalized_path, &self.file)
    }

    fn validate_identity_at(&self, path: &Path) -> io::Result<PathBuf> {
        let normalized_path = normalized_file_path(path);
        validate_database_lock_file(&normalized_path, &self.file)?;
        Ok(normalized_path)
    }

    fn rebind_after_rename(&mut self, path: &Path) -> io::Result<()> {
        let normalized_path = self.validate_identity_at(path)?;
        self.normalized_path = normalized_path;
        // Rebinding after a caller-controlled rename transfers namespace
        // ownership to the new path. Placeholder cleanup is explicit and
        // identity-checked; never let the destructor remove the rebound path
        // after a later publication failure.
        self.remove_created_file_on_drop = false;
        Ok(())
    }
}

impl Drop for DatabaseLifecyclePhysicalGuard {
    fn drop(&mut self) {
        let remove_created_file = self.remove_created_file_on_drop
            && validate_database_lock_file(&self.normalized_path, &self.file).is_ok();
        if self.lifecycle_locked {
            let _ = platform_unlock_database_lifecycle(&self.file);
        }
        if remove_created_file {
            let _ = fs::remove_file(&self.normalized_path);
        }
    }
}

impl DerivedStoreWriteGuard {
    pub fn acquire(db_path: &Path, store_name: &str) -> io::Result<Self> {
        Ok(Self {
            _guard: acquire_derived_store_guard(
                db_path,
                store_name,
                DerivedStoreLockMode::Exclusive,
                true,
            )?,
        })
    }
}

impl DerivedStoreReadGuard {
    pub fn acquire(db_path: &Path, store_name: &str) -> io::Result<Self> {
        Ok(Self {
            _guard: acquire_derived_store_guard(
                db_path,
                store_name,
                DerivedStoreLockMode::Shared,
                false,
            )?,
        })
    }
}

impl Drop for DerivedStoreDatabaseRangeGuard {
    fn drop(&mut self) {
        let _ = platform_unlock_database_range(&self.file, self.offset);
    }
}

impl Drop for DerivedStoreSentinelLockGuard {
    fn drop(&mut self) {
        let _ = fs4::fs_std::FileExt::unlock(&self.file);
    }
}

fn acquire_derived_store_guard(
    db_path: &Path,
    store_name: &str,
    mode: DerivedStoreLockMode,
    create_sentinel: bool,
) -> io::Result<DerivedStorePhysicalGuard> {
    validate_derived_store_name(store_name)?;
    ensure_database_range_lock_supported()?;

    let lifecycle_guard = DatabaseLifecycleSharedGuard::acquire_existing(db_path)?;
    let normalized_db_path = lifecycle_guard.path().to_path_buf();
    ensure_database_maintenance_fence_absent(&normalized_db_path)?;
    let store_locks =
        acquire_derived_store_lock_set(&normalized_db_path, store_name, mode, create_sentinel)?;
    lifecycle_guard.validate_path_identity()?;

    Ok(DerivedStorePhysicalGuard {
        _store_locks: store_locks,
        _lifecycle_guard: lifecycle_guard,
    })
}

fn acquire_derived_store_lock_set(
    normalized_db_path: &Path,
    store_name: &str,
    mode: DerivedStoreLockMode,
    create_sentinel: bool,
) -> io::Result<DerivedStoreLockSet> {
    validate_derived_store_name(store_name)?;
    ensure_database_range_lock_supported()?;

    let lock_path = derived_store_write_lock_path_from_normalized(normalized_db_path, store_name);
    let lock_offset = derived_store_database_lock_offset(store_name);
    validate_derived_store_lock_offset(store_name, lock_offset)?;
    let expected_sentinel =
        derived_store_sentinel_bytes(normalized_db_path, store_name, lock_offset);

    let database_file = open_database_lock_file(normalized_db_path, mode)?;
    validate_database_lock_file(normalized_db_path, &database_file)?;
    if !platform_try_lock_database_range(&database_file, lock_offset, mode)? {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "derived store {store_name} has an active physical writer: {}",
                lock_path.display()
            ),
        ));
    }
    let database_lock = DerivedStoreDatabaseRangeGuard {
        file: database_file,
        offset: lock_offset,
    };
    validate_database_lock_file(normalized_db_path, &database_lock.file)?;

    let existing_sentinel =
        match open_and_validate_derived_store_sentinel(&lock_path, &expected_sentinel) {
            Ok(file) => Some(file),
            Err(error) if is_lock_contention_error(&error) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "derived store {store_name} has an active physical writer: {}",
                        lock_path.display()
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
    let sentinel_file = match existing_sentinel {
        Some(file) => {
            validate_derived_store_sentinel(&lock_path, &file, &expected_sentinel)?;
            file
        }
        None if create_sentinel => {
            match durable_create_new_file(&lock_path, &expected_sentinel) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            open_and_validate_derived_store_sentinel(&lock_path, &expected_sentinel)?
        }
        None => {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "derived store {store_name} has no persistent lock sentinel: {}",
                    lock_path.display()
                ),
            ));
        }
    };
    validate_database_lock_file(normalized_db_path, &database_lock.file)?;
    validate_derived_store_sentinel(&lock_path, &sentinel_file, &expected_sentinel)?;
    if !try_lock_derived_store_sentinel(&sentinel_file, mode)? {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "derived store {store_name} has an active physical writer: {}",
                lock_path.display()
            ),
        ));
    }
    let sentinel_lock = DerivedStoreSentinelLockGuard {
        file: sentinel_file,
    };
    validate_derived_store_sentinel(&lock_path, &sentinel_lock.file, &expected_sentinel)?;
    validate_database_lock_file(normalized_db_path, &database_lock.file)?;

    Ok(DerivedStoreLockSet {
        _sentinel_lock: sentinel_lock,
        _database_lock: database_lock,
    })
}

fn acquire_database_lifecycle_guard(
    path: &Path,
    mode: DerivedStoreLockMode,
    create_if_missing: bool,
    remove_created_file_on_drop: bool,
) -> io::Result<DatabaseLifecyclePhysicalGuard> {
    ensure_database_lifecycle_lock_supported()?;
    let normalized_path = normalized_file_path(path);
    let (file, created_authority_file) = if create_if_missing {
        open_or_create_database_lock_file(&normalized_path, mode)?
    } else {
        (open_database_lock_file(&normalized_path, mode)?, false)
    };
    let mut guard = DatabaseLifecyclePhysicalGuard {
        file,
        normalized_path,
        lifecycle_locked: false,
        created_authority_file,
        remove_created_file_on_drop: created_authority_file && remove_created_file_on_drop,
    };
    validate_database_lock_file(&guard.normalized_path, &guard.file)?;
    if !platform_try_lock_database_lifecycle(&guard.file, mode)? {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "database lifecycle has an active physical writer or connection: {}",
                guard.normalized_path.display()
            ),
        ));
    }
    guard.lifecycle_locked = true;
    validate_database_lock_file(&guard.normalized_path, &guard.file)?;
    Ok(guard)
}

fn validate_derived_store_name(store_name: &str) -> io::Result<()> {
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
    Ok(())
}

fn derived_store_database_lock_offset(store_name: &str) -> u64 {
    let digest = Sha256::digest(store_name.as_bytes());
    let bucket = u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    let available_offsets = (i64::MAX as u64) - DERIVED_STORE_DATABASE_LOCK_BASE + 1;
    DERIVED_STORE_DATABASE_LOCK_BASE + (bucket % available_offsets)
}

fn validate_derived_store_lock_offset(store_name: &str, offset: u64) -> io::Result<()> {
    debug_assert!(offset >= DERIVED_STORE_DATABASE_LOCK_BASE);
    debug_assert!(offset <= i64::MAX as u64);

    for contract_name in DERIVED_STORE_LOCK_CONTRACT_NAMES {
        if contract_name != store_name
            && derived_store_database_lock_offset(contract_name) == offset
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "derived store lock offset collision between {store_name} and {contract_name}"
                ),
            ));
        }
    }

    // Cross-process offset collisions remain fail-safe because they alias the
    // database byte and conservatively serialize the two stores. Within one
    // process, retain every observed name so a collision is reported instead
    // of appearing as unrelated WouldBlock contention.
    let offsets = DERIVED_STORE_NAMES_BY_OFFSET.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut offsets = offsets
        .lock()
        .map_err(|_| io::Error::other("derived store lock offset registry is poisoned"))?;
    if let Some(existing) = offsets.get(&offset)
        && existing != store_name
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("derived store lock offset collision between {store_name} and {existing}"),
        ));
    }
    offsets
        .entry(offset)
        .or_insert_with(|| store_name.to_owned());
    Ok(())
}

fn derived_store_sentinel_bytes(
    normalized_db_path: &Path,
    store_name: &str,
    lock_offset: u64,
) -> Vec<u8> {
    let (path_encoding, path_bytes) = normalized_path_identity_bytes(normalized_db_path);
    format!(
        "{DERIVED_STORE_SENTINEL_MAGIC}\npath_encoding={path_encoding}\ndatabase_path_hex={}\nstore={store_name}\nlock_offset={lock_offset}\n",
        lowercase_hex(&path_bytes)
    )
    .into_bytes()
}

#[cfg(unix)]
fn normalized_path_identity_bytes(path: &Path) -> (&'static str, Vec<u8>) {
    use std::os::unix::ffi::OsStrExt as _;

    ("unix-bytes", path.as_os_str().as_bytes().to_vec())
}

#[cfg(windows)]
fn normalized_path_identity_bytes(path: &Path) -> (&'static str, Vec<u8>) {
    use std::os::windows::ffi::OsStrExt as _;

    let bytes = path
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect();
    ("windows-utf16le", bytes)
}

#[cfg(not(any(unix, windows)))]
fn normalized_path_identity_bytes(path: &Path) -> (&'static str, Vec<u8>) {
    (
        "unsupported-display",
        path.as_os_str().to_string_lossy().as_bytes().to_vec(),
    )
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn open_and_validate_derived_store_sentinel(
    path: &Path,
    expected: &[u8],
) -> io::Result<std::fs::File> {
    let file = open_existing_lock_file(path, false)?;
    validate_derived_store_sentinel(path, &file, expected)?;
    Ok(file)
}

fn validate_derived_store_sentinel(
    path: &Path,
    file: &std::fs::File,
    expected: &[u8],
) -> io::Result<()> {
    let handle_before = snapshot_open_lock_file(file)?;
    let path_before = snapshot_lock_path(path)?;
    validate_sentinel_snapshot(path, &handle_before)?;
    validate_sentinel_snapshot(path, &path_before)?;
    if handle_before.identity != path_before.identity {
        return Err(unsafe_derived_lock_path(
            path,
            "sentinel path does not identify the opened file",
        ));
    }

    let mut reader = file.try_clone()?;
    reader.seek(SeekFrom::Start(0))?;
    let mut contents = Vec::with_capacity(expected.len().saturating_add(1));
    let read_limit = u64::try_from(expected.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    reader.take(read_limit).read_to_end(&mut contents)?;
    if contents != expected {
        return Err(unsafe_derived_lock_path(
            path,
            "sentinel identity does not match the database and store",
        ));
    }

    let handle_after = snapshot_open_lock_file(file)?;
    let path_after = snapshot_lock_path(path)?;
    validate_sentinel_snapshot(path, &handle_after)?;
    validate_sentinel_snapshot(path, &path_after)?;
    if handle_before != handle_after || handle_before != path_after {
        return Err(unsafe_derived_lock_path(
            path,
            "sentinel changed while it was being validated",
        ));
    }
    Ok(())
}

fn validate_sentinel_snapshot(path: &Path, snapshot: &DerivedLockFileSnapshot) -> io::Result<()> {
    if !snapshot.regular_non_reparse {
        return Err(unsafe_derived_lock_path(
            path,
            "sentinel is not a regular non-reparse file",
        ));
    }
    if snapshot.link_count != 1 {
        return Err(unsafe_derived_lock_path(
            path,
            "sentinel must have exactly one filesystem link",
        ));
    }
    Ok(())
}

fn validate_database_lock_file(path: &Path, file: &std::fs::File) -> io::Result<()> {
    let handle_snapshot = snapshot_open_lock_file(file)?;
    let path_snapshot = snapshot_lock_path(path)?;
    if !handle_snapshot.regular_non_reparse || !path_snapshot.regular_non_reparse {
        return Err(unsafe_derived_lock_path(
            path,
            "database authority is not a regular non-reparse file",
        ));
    }
    if handle_snapshot.link_count != 1 || path_snapshot.link_count != 1 {
        return Err(unsafe_derived_lock_path(
            path,
            "database authority must have exactly one filesystem link",
        ));
    }
    if handle_snapshot.identity != path_snapshot.identity {
        return Err(unsafe_derived_lock_path(
            path,
            "database path does not identify the opened lock authority",
        ));
    }
    Ok(())
}

fn unsafe_derived_lock_path(path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unsafe derived lock path {}: {reason}", path.display()),
    )
}

fn try_lock_derived_store_sentinel(
    file: &std::fs::File,
    mode: DerivedStoreLockMode,
) -> io::Result<bool> {
    let result = match mode {
        DerivedStoreLockMode::Shared => fs4::fs_std::FileExt::try_lock_shared(file),
        DerivedStoreLockMode::Exclusive => fs4::fs_std::FileExt::try_lock_exclusive(file),
    };
    match result {
        Ok(locked) => Ok(locked),
        Err(error) if is_lock_contention_error(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

/// The Windows sentinel path can report the underlying `LockFileEx` contention
/// codes or `ERROR_SHARING_VIOLATION` for an unavailable fixed authority. All
/// three mean the sentinel is currently unavailable; access-denied and
/// unrelated I/O failures remain fail-closed.
#[cfg(windows)]
fn is_lock_contention_error(error: &io::Error) -> bool {
    use windows_sys::Win32::Foundation::{
        ERROR_IO_PENDING, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION,
    };

    matches!(
        error.raw_os_error(),
        Some(code)
            if code == ERROR_LOCK_VIOLATION as i32
                || code == ERROR_IO_PENDING as i32
                || code == ERROR_SHARING_VIOLATION as i32
    )
}

#[cfg(not(windows))]
fn is_lock_contention_error(_error: &io::Error) -> bool {
    false
}

#[cfg(any(target_os = "linux", windows))]
fn ensure_database_range_lock_supported() -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(target_os = "linux", windows)))]
fn ensure_database_range_lock_supported() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "derived store locks require Linux OFD locks or Windows LockFileEx",
    ))
}

#[cfg(any(unix, windows))]
fn ensure_database_lifecycle_lock_supported() -> io::Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn ensure_database_lifecycle_lock_supported() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "database lifecycle locks require Unix flock, Linux OFD locks, or Windows LockFileEx",
    ))
}

fn open_database_lock_file(path: &Path, mode: DerivedStoreLockMode) -> io::Result<std::fs::File> {
    open_existing_lock_file(path, matches!(mode, DerivedStoreLockMode::Exclusive))
}

fn open_or_create_database_lock_file(
    path: &Path,
    mode: DerivedStoreLockMode,
) -> io::Result<(std::fs::File, bool)> {
    for _ in 0..16 {
        match open_existing_lock_file(path, matches!(mode, DerivedStoreLockMode::Exclusive)) {
            Ok(file) => return Ok((file, false)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        match create_new_database_lock_file(path) {
            Ok(file) => return Ok((file, true)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        format!(
            "database lifecycle authority changed repeatedly while opening {}",
            path.display()
        ),
    ))
}

#[cfg(unix)]
fn open_existing_lock_file(path: &Path, writable: bool) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut options = std::fs::OpenOptions::new();
    options
        .read(true)
        .write(writable)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    options.open(path)
}

#[cfg(unix)]
fn create_new_database_lock_file(path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut options = std::fs::OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    options.open(path)
}

#[cfg(windows)]
fn open_existing_lock_file(path: &Path, writable: bool) -> io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let mut options = std::fs::OpenOptions::new();
    options
        .read(true)
        .write(writable)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

#[cfg(windows)]
fn create_new_database_lock_file(path: &Path) -> io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let mut options = std::fs::OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create_new(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_existing_lock_file(_path: &Path, _writable: bool) -> io::Result<std::fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "derived store locks require Linux OFD locks or Windows LockFileEx",
    ))
}

#[cfg(not(any(unix, windows)))]
fn create_new_database_lock_file(_path: &Path) -> io::Result<std::fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "database lifecycle locks require Linux OFD locks or Windows LockFileEx",
    ))
}

#[cfg(unix)]
fn open_existing_directory(path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut options = std::fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    options.open(path)
}

#[cfg(windows)]
fn open_existing_directory(path: &Path) -> io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let mut options = std::fs::OpenOptions::new();
    options
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_existing_directory(_path: &Path) -> io::Result<std::fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "directory identity guards require Unix or Windows file identities",
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedLockFileIdentity {
    volume: u64,
    file_id: [u8; 16],
}

fn snapshot_open_directory(
    path: &Path,
    file: &std::fs::File,
) -> io::Result<DerivedLockFileIdentity> {
    if !file.metadata()?.is_dir() {
        return Err(unsafe_directory_path(
            path,
            "opened authority is not a directory",
        ));
    }
    let snapshot = snapshot_open_lock_file(file)?;
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

        if snapshot.attributes & u64::from(FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Err(unsafe_directory_path(
                path,
                "opened authority is a reparse directory",
            ));
        }
    }
    Ok(snapshot.identity)
}

fn unsafe_directory_path(path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unsafe directory path {}: {reason}", path.display()),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedLockFileSnapshot {
    identity: DerivedLockFileIdentity,
    length: u64,
    link_count: u64,
    regular_non_reparse: bool,
    attributes: u64,
    created_or_changed: (i64, i64),
    modified: (i64, i64),
}

fn snapshot_lock_path(path: &Path) -> io::Result<DerivedLockFileSnapshot> {
    let current = open_existing_lock_file(path, false)?;
    snapshot_open_lock_file(&current)
}

#[cfg(unix)]
fn snapshot_open_lock_file(file: &std::fs::File) -> io::Result<DerivedLockFileSnapshot> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = file.metadata()?;
    let mut file_id = [0_u8; 16];
    file_id[..8].copy_from_slice(&metadata.ino().to_be_bytes());
    Ok(DerivedLockFileSnapshot {
        identity: DerivedLockFileIdentity {
            volume: metadata.dev(),
            file_id,
        },
        length: metadata.len(),
        link_count: metadata.nlink(),
        regular_non_reparse: metadata.file_type().is_file(),
        attributes: u64::from(metadata.mode()),
        created_or_changed: (metadata.ctime(), metadata.ctime_nsec()),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
    })
}

#[cfg(windows)]
fn snapshot_open_lock_file(file: &std::fs::File) -> io::Result<DerivedLockFileSnapshot> {
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_ID_INFO, FileIdInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: File owns a valid HANDLE and information points to writable,
    // correctly sized storage for the duration of the synchronous call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut identity = FILE_ID_INFO::default();
    let identity_size = u32::try_from(std::mem::size_of::<FILE_ID_INFO>())
        .map_err(|_| io::Error::other("FILE_ID_INFO size does not fit the Win32 API"))?;
    // SAFETY: File owns a valid HANDLE, identity is correctly sized writable
    // storage, and the call is synchronous. Failure (including an unsupported
    // filesystem information class) is deliberately returned fail-closed.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut identity as *mut FILE_ID_INFO).cast(),
            identity_size,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let file_size =
        (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow);
    let creation_time = (u64::from(information.ftCreationTime.dwHighDateTime) << 32)
        | u64::from(information.ftCreationTime.dwLowDateTime);
    let last_write_time = (u64::from(information.ftLastWriteTime.dwHighDateTime) << 32)
        | u64::from(information.ftLastWriteTime.dwLowDateTime);
    let forbidden_attributes = FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT;
    Ok(DerivedLockFileSnapshot {
        identity: DerivedLockFileIdentity {
            volume: identity.VolumeSerialNumber,
            file_id: identity.FileId.Identifier,
        },
        length: file_size,
        link_count: u64::from(information.nNumberOfLinks),
        regular_non_reparse: information.dwFileAttributes & forbidden_attributes == 0,
        attributes: u64::from(information.dwFileAttributes),
        created_or_changed: (creation_time as i64, 0),
        modified: (last_write_time as i64, 0),
    })
}

#[cfg(not(any(unix, windows)))]
fn snapshot_open_lock_file(file: &std::fs::File) -> io::Result<DerivedLockFileSnapshot> {
    let metadata = file.metadata()?;
    let created = metadata
        .created()
        .ok()
        .and_then(|value| value.elapsed().ok())
        .map(|value| value.as_nanos() as i64)
        .unwrap_or_default();
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.elapsed().ok())
        .map(|value| value.as_nanos() as i64)
        .unwrap_or_default();
    Ok(DerivedLockFileSnapshot {
        identity: DerivedLockFileIdentity {
            volume: metadata.len(),
            file_id: {
                let mut file_id = [0_u8; 16];
                file_id[..8].copy_from_slice(&(created as u64).to_be_bytes());
                file_id
            },
        },
        length: metadata.len(),
        link_count: 0,
        regular_non_reparse: metadata.file_type().is_file(),
        attributes: 0,
        created_or_changed: (created, 0),
        modified: (modified, 0),
    })
}

#[cfg(target_os = "linux")]
fn platform_try_lock_database_range(
    file: &std::fs::File,
    offset: u64,
    mode: DerivedStoreLockMode,
) -> io::Result<bool> {
    use std::os::fd::AsRawFd as _;

    let start = libc::off_t::try_from(offset).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "derived store database lock offset exceeds off_t",
        )
    })?;
    // SAFETY: zero is a valid initial value for every field of libc::flock.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = match mode {
        DerivedStoreLockMode::Shared => libc::F_RDLCK as _,
        DerivedStoreLockMode::Exclusive => libc::F_WRLCK as _,
    };
    lock.l_whence = libc::SEEK_SET as _;
    lock.l_start = start;
    lock.l_len = 1;
    lock.l_pid = 0;
    loop {
        // SAFETY: file owns a valid descriptor and lock points to an initialized
        // flock with a one-byte range representable by off_t.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => continue,
            Some(code) if code == libc::EACCES || code == libc::EAGAIN => return Ok(false),
            Some(code)
                if code == libc::EINVAL || code == libc::ENOSYS || code == libc::EOPNOTSUPP =>
            {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Linux kernel or filesystem does not support OFD range locks",
                ));
            }
            _ => return Err(error),
        }
    }
}

#[cfg(target_os = "linux")]
fn platform_unlock_database_range(file: &std::fs::File, offset: u64) -> io::Result<()> {
    use std::os::fd::AsRawFd as _;

    let start = libc::off_t::try_from(offset).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "derived store database lock offset exceeds off_t",
        )
    })?;
    // SAFETY: zero is a valid initial value for every field of libc::flock.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_UNLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    lock.l_start = start;
    lock.l_len = 1;
    lock.l_pid = 0;
    loop {
        // SAFETY: file owns a valid descriptor and lock describes the exact
        // one-byte OFD range acquired by this guard.
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_OFD_SETLK, &lock) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EINTR) {
            return Err(error);
        }
    }
}

#[cfg(windows)]
fn platform_try_lock_database_range(
    file: &std::fs::File,
    offset: u64,
    mode: DerivedStoreLockMode,
) -> io::Result<bool> {
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::{
        Storage::FileSystem::{LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx},
        System::IO::OVERLAPPED,
    };

    let mut overlapped = OVERLAPPED::default();
    overlapped.Anonymous.Anonymous.Offset = offset as u32;
    overlapped.Anonymous.Anonymous.OffsetHigh = (offset >> 32) as u32;
    let flags = LOCKFILE_FAIL_IMMEDIATELY
        | if matches!(mode, DerivedStoreLockMode::Exclusive) {
            LOCKFILE_EXCLUSIVE_LOCK
        } else {
            0
        };
    // SAFETY: the File owns a valid HANDLE and OVERLAPPED lives for the
    // synchronous call. The requested range is exactly one byte.
    if unsafe { LockFileEx(file.as_raw_handle(), flags, 0, 1, 0, &mut overlapped) } != 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    if is_lock_contention_error(&error) {
        Ok(false)
    } else {
        Err(error)
    }
}

#[cfg(windows)]
fn platform_unlock_database_range(file: &std::fs::File, offset: u64) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::{Storage::FileSystem::UnlockFileEx, System::IO::OVERLAPPED};

    let mut overlapped = OVERLAPPED::default();
    overlapped.Anonymous.Anonymous.Offset = offset as u32;
    overlapped.Anonymous.Anonymous.OffsetHigh = (offset >> 32) as u32;
    // SAFETY: the File owns the HANDLE used to acquire this exact range and
    // OVERLAPPED lives for the synchronous call.
    if unsafe { UnlockFileEx(file.as_raw_handle(), 0, 1, 0, &mut overlapped) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(any(target_os = "linux", windows)))]
fn platform_try_lock_database_range(
    _file: &std::fs::File,
    _offset: u64,
    _mode: DerivedStoreLockMode,
) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "derived store locks require Linux OFD locks or Windows LockFileEx",
    ))
}

#[cfg(not(any(target_os = "linux", windows)))]
fn platform_unlock_database_range(_file: &std::fs::File, _offset: u64) -> io::Result<()> {
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
fn platform_try_lock_database_lifecycle(
    file: &std::fs::File,
    mode: DerivedStoreLockMode,
) -> io::Result<bool> {
    platform_try_lock_database_range(file, DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET, mode)
}

#[cfg(any(target_os = "linux", windows))]
fn platform_unlock_database_lifecycle(file: &std::fs::File) -> io::Result<()> {
    platform_unlock_database_range(file, DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn platform_try_lock_database_lifecycle(
    file: &std::fs::File,
    mode: DerivedStoreLockMode,
) -> io::Result<bool> {
    match mode {
        DerivedStoreLockMode::Shared => fs4::fs_std::FileExt::try_lock_shared(file),
        DerivedStoreLockMode::Exclusive => fs4::fs_std::FileExt::try_lock_exclusive(file),
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn platform_unlock_database_lifecycle(file: &std::fs::File) -> io::Result<()> {
    fs4::fs_std::FileExt::unlock(file)
}

#[cfg(not(any(unix, windows)))]
fn platform_try_lock_database_lifecycle(
    _file: &std::fs::File,
    _mode: DerivedStoreLockMode,
) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "database lifecycle locks require Unix flock, Linux OFD locks, or Windows LockFileEx",
    ))
}

#[cfg(not(any(unix, windows)))]
fn platform_unlock_database_lifecycle(_file: &std::fs::File) -> io::Result<()> {
    Ok(())
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

/// Moves one regular sibling file without replacing an existing destination.
///
/// The source is flushed before the namespace operation and the parent
/// directory is flushed after it.  Platforms without an atomic no-replace
/// primitive fail closed rather than emulating the move with copy/remove.
pub fn durable_move_file_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    require_sibling_paths(source, destination)?;
    let source_metadata = fs::symlink_metadata(source)?;
    if !source_metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "durable move source is not a regular file: {}",
                source.display()
            ),
        ));
    }
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "durable move destination already exists: {}",
                    destination.display()
                ),
            ));
        }
        Err(error) => return Err(error),
    }
    durable_sync_file(source)?;
    durable_move_file_no_replace_platform(source, destination)?;
    durable_sync_directory(parent_directory(source)?)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn durable_move_file_no_replace_platform(source: &Path, destination: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE).map_err(Into::into)
}

#[cfg(windows)]
fn durable_move_file_no_replace_platform(source: &Path, destination: &Path) -> io::Result<()> {
    windows_move_file(source, destination, false)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn durable_move_file_no_replace_platform(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "durable no-replace file move requires Linux renameat2 or Windows MoveFileEx",
    ))
}

#[cfg(not(any(unix, windows)))]
fn durable_move_file_no_replace_platform(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "durable no-replace file move is unsupported on this platform",
    ))
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

#[cfg(target_os = "linux")]
fn durable_publish_new_file_platform(staged: &Path, destination: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, staged, CWD, destination, RenameFlags::NOREPLACE).map_err(Into::into)
}

#[cfg(all(not(windows), not(target_os = "linux")))]
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

/// Returns the stable maintenance marker path for one canonical database.
///
/// Replacement creates this namespace fence before taking inode authorities.
/// Shared openers recheck it only after acquiring their lifecycle guard, which
/// closes the check-before-lock race across a database rename.
pub fn database_maintenance_lock_path(db_path: &Path) -> PathBuf {
    let normalized = normalized_file_path(db_path);
    let mut marker = normalized.into_os_string();
    marker.push(".maintenance.lock");
    PathBuf::from(marker)
}

fn ensure_database_maintenance_fence_absent(normalized_db_path: &Path) -> io::Result<()> {
    let marker = database_maintenance_lock_path(normalized_db_path);
    match fs::symlink_metadata(&marker) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "database is fenced for maintenance: {}",
                normalized_db_path.display()
            ),
        )),
    }
}

pub fn derived_store_write_lock_path(db_path: &Path, store_name: &str) -> PathBuf {
    let normalized = normalized_file_path(db_path);
    derived_store_write_lock_path_from_normalized(&normalized, store_name)
}

fn derived_store_write_lock_path_from_normalized(
    normalized_db_path: &Path,
    store_name: &str,
) -> PathBuf {
    let mut lock_path = normalized_db_path.as_os_str().to_os_string();
    lock_path.push(format!(".derived.{store_name}.lock"));
    PathBuf::from(lock_path)
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

pub mod sqlite_connection;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn directory_identity_guard_detects_a_deterministic_path_swap() {
        let tempdir = tempfile::tempdir().unwrap();
        let guarded = tempdir.path().join("generation");
        let displaced = tempdir.path().join("generation.displaced");
        let replacement = tempdir.path().join("replacement");
        fs::create_dir(&guarded).unwrap();
        fs::create_dir(&replacement).unwrap();
        let guard = DirectoryIdentityGuard::acquire(&guarded).unwrap();

        fs::rename(&guarded, &displaced).unwrap();
        fs::rename(&replacement, &guarded).unwrap();

        let error = guard.validate_path_identity().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        fs::rename(&guarded, &replacement).unwrap();
        fs::rename(&displaced, &guarded).unwrap();
        guard.validate_path_identity().unwrap();
    }

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

    #[cfg(target_os = "linux")]
    #[test]
    fn durable_derived_store_new_file_publish_is_a_single_entry_atomic_rename() {
        use std::os::unix::fs::MetadataExt as _;

        let tempdir = tempfile::tempdir().unwrap();
        let staged = tempdir.path().join("sentinel.new");
        let destination = tempdir.path().join("sentinel");
        let expected = b"kanban-derived-store-lock-v2\nexact=true\n";
        fs::write(&staged, expected).unwrap();
        let staged_before = fs::metadata(&staged).unwrap();

        durable_publish_new_file_platform(&staged, &destination).unwrap();

        assert!(
            !staged.exists(),
            "the publish primitive must never leave a second hard-link entry"
        );
        assert_eq!(fs::read(&destination).unwrap(), expected);
        let published = fs::metadata(&destination).unwrap();
        assert_eq!(published.dev(), staged_before.dev());
        assert_eq!(published.ino(), staged_before.ino());
        assert_eq!(published.nlink(), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn durable_derived_store_first_create_race_publishes_one_exact_single_link_inode() {
        use std::{
            os::unix::fs::MetadataExt as _,
            sync::{Arc, Barrier},
        };

        let tempdir = tempfile::tempdir().unwrap();
        let marker = tempdir.path().join("published");
        let barrier = Arc::new(Barrier::new(17));
        let contenders = (0..16_u8)
            .map(|contender| {
                let marker = marker.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let contents = format!("winner={contender:02}\n").into_bytes();
                    barrier.wait();
                    (
                        contents.clone(),
                        durable_create_new_file(&marker, &contents),
                    )
                })
            })
            .collect::<Vec<_>>();
        barrier.wait();

        let mut winner = None;
        for contender in contenders {
            let (contents, result) = contender.join().unwrap();
            match result {
                Ok(()) => assert!(winner.replace(contents).is_none()),
                Err(error) => assert_eq!(error.kind(), io::ErrorKind::AlreadyExists),
            }
        }

        let winner = winner.expect("exactly one create-new contender must publish");
        assert_eq!(fs::read(&marker).unwrap(), winner);
        assert_eq!(fs::metadata(&marker).unwrap().nlink(), 1);
        assert_eq!(
            fs::read_dir(tempdir.path()).unwrap().count(),
            1,
            "all losing and renamed staged entries must be removed"
        );
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

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_lock_file_is_persistent_and_candidate_artifacts_are_ignored() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        let abandoned_candidate =
            PathBuf::from(format!("{}.candidate.reused", lock_path.display()));

        drop(DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap());
        let sentinel_before = fs::read(&lock_path).unwrap();
        let sentinel_text = std::str::from_utf8(&sentinel_before).unwrap();
        assert!(sentinel_text.starts_with("kanban-derived-store-lock-v2\n"));
        assert!(sentinel_text.contains("\nstore=tantivy_tasks\n"));
        assert!(sentinel_text.contains("\nlock_offset="));
        assert!(!sentinel_text.contains("pid="));
        fs::write(&abandoned_candidate, "abandoned").unwrap();
        let guard = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();
        let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        drop(guard);

        assert!(lock_path.exists());
        assert_eq!(fs::read(&lock_path).unwrap(), sentinel_before);
        assert_eq!(
            fs::read_to_string(&abandoned_candidate).unwrap(),
            "abandoned"
        );
        drop(DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap());
        assert!(lock_path.exists());
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_lock_serializes_concurrent_contenders() {
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

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_read_guard_fails_closed_without_a_persistent_lock_file() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");

        let error = DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(!lock_path.exists());
    }

    #[test]
    fn durable_derived_store_read_guard_rejects_unsafe_store_names() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();

        for store_name in ["", "../tantivy_tasks", "tantivy/tasks", "tantivy tasks"] {
            let error = DerivedStoreReadGuard::acquire(&db_path, store_name).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
    }

    #[cfg(not(any(target_os = "linux", windows)))]
    #[test]
    fn durable_derived_store_guards_fail_closed_when_stable_range_locks_are_unsupported() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");

        let write_error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();
        let read_error = DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();

        assert_eq!(write_error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(read_error.kind(), io::ErrorKind::Unsupported);
        assert!(!lock_path.exists());
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_writer_rejects_legacy_mutable_sentinel_without_rewriting_it() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        let legacy = b"pid=123\nowner=legacy\nstore=tantivy_tasks\n";
        fs::write(&lock_path, legacy).unwrap();

        let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(fs::read(&lock_path).unwrap(), legacy);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_read_guards_coexist_without_changing_lock_evidence() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        drop(DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap());
        let bytes_before = fs::read(&lock_path).unwrap();
        let metadata_before = fs::metadata(&lock_path).unwrap();

        let first = DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks").unwrap();
        let second = DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks").unwrap();

        assert_eq!(fs::read(&lock_path).unwrap(), bytes_before);
        let metadata_during = fs::metadata(&lock_path).unwrap();
        assert_eq!(metadata_during.len(), metadata_before.len());
        assert_eq!(
            metadata_during.modified().unwrap(),
            metadata_before.modified().unwrap()
        );
        assert_eq!(
            metadata_during.permissions().readonly(),
            metadata_before.permissions().readonly()
        );
        drop(second);
        drop(first);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_read_guard_blocks_writer_without_changing_lock_evidence() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        drop(DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap());
        let bytes_before = fs::read(&lock_path).unwrap();
        let modified_before = fs::metadata(&lock_path).unwrap().modified().unwrap();
        let reader = DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks").unwrap();

        let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(fs::read(&lock_path).unwrap(), bytes_before);
        assert_eq!(
            fs::metadata(&lock_path).unwrap().modified().unwrap(),
            modified_before
        );
        drop(reader);
        drop(DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap());
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_writer_blocks_read_guard() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let writer = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();

        let error = DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        drop(writer);
        drop(DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks").unwrap());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn durable_derived_store_guards_reject_symlink_lock_without_changing_its_target() {
        use std::os::unix::fs::symlink;

        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        let sentinel = b"canonical-sqlite-must-not-change";
        fs::write(&db_path, sentinel).unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        symlink(&db_path, &lock_path).unwrap();

        let write_result = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks");
        assert!(
            write_result.is_err(),
            "a derived writer must never follow a lock-path symlink"
        );
        let read_result = DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks");
        assert!(
            read_result.is_err(),
            "a derived reader must never follow a lock-path symlink"
        );
        assert_eq!(fs::read(&db_path).unwrap(), sentinel);
        assert!(
            fs::symlink_metadata(&lock_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_guards_reject_hardlinked_lock_without_changing_its_target() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        let sentinel = b"canonical-sqlite-must-not-change";
        fs::write(&db_path, sentinel).unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        fs::hard_link(&db_path, &lock_path).unwrap();

        let write_result = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks");
        assert!(
            write_result.is_err(),
            "a derived writer must reject a multiply-linked lock inode"
        );
        let read_result = DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks");
        assert!(
            read_result.is_err(),
            "a derived reader must reject a multiply-linked lock inode"
        );
        assert_eq!(fs::read(&db_path).unwrap(), sentinel);
    }

    #[cfg(target_os = "linux")]
    fn run_linux_non_regular_guard_probe_with_deadline(mode: &str, path: &Path) {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("tests::durable_derived_store_linux_non_regular_probe_child")
            .arg("--nocapture")
            .env("KANBAN_LOCAL_DERIVED_GUARD_PROBE", mode)
            .env("KANBAN_LOCAL_DERIVED_GUARD_PATH", path)
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "{mode} probe failed with {status}");
                return;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{mode} probe blocked while opening {}", path.display());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn durable_derived_store_linux_non_regular_probe_child() {
        let Ok(mode) = std::env::var("KANBAN_LOCAL_DERIVED_GUARD_PROBE") else {
            return;
        };
        let path = PathBuf::from(
            std::env::var_os("KANBAN_LOCAL_DERIVED_GUARD_PATH")
                .expect("guard probe path must be supplied"),
        );
        let result = match mode.as_str() {
            "sentinel-fifo" | "sentinel-device" => {
                open_and_validate_derived_store_sentinel(&path, b"expected-sentinel").map(|_| ())
            }
            "database-fifo" | "database-device" => {
                open_database_lock_file(&path, DerivedStoreLockMode::Shared)
                    .and_then(|file| validate_database_lock_file(&path, &file))
            }
            other => panic!("unknown guard probe mode {other}"),
        };
        let error = result.expect_err("non-regular lock authority must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn durable_derived_store_linux_fifo_and_device_openers_fail_before_deadline() {
        use std::os::unix::{ffi::OsStrExt as _, fs::FileTypeExt as _};

        let tempdir = tempfile::tempdir().unwrap();
        let fifo = tempdir.path().join("adversarial-fifo");
        let fifo_bytes = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: fifo_bytes is a NUL-terminated path and the mode is a valid
        // permission mask. The path is owned by this test's private directory.
        assert_eq!(unsafe { libc::mkfifo(fifo_bytes.as_ptr(), 0o600) }, 0);

        run_linux_non_regular_guard_probe_with_deadline("sentinel-fifo", &fifo);
        run_linux_non_regular_guard_probe_with_deadline("database-fifo", &fifo);
        run_linux_non_regular_guard_probe_with_deadline("sentinel-device", Path::new("/dev/zero"));
        run_linux_non_regular_guard_probe_with_deadline("database-device", Path::new("/dev/zero"));
        assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    }

    #[cfg(windows)]
    #[test]
    fn durable_derived_store_windows_snapshot_uses_full_file_id_identity() {
        let tempdir = tempfile::tempdir().unwrap();
        let first = tempdir.path().join("first");
        let alias = tempdir.path().join("alias");
        let distinct = tempdir.path().join("distinct");
        fs::write(&first, b"first").unwrap();
        fs::hard_link(&first, &alias).unwrap();
        fs::write(&distinct, b"distinct").unwrap();

        let first = snapshot_lock_path(&first).unwrap();
        let alias = snapshot_lock_path(&alias).unwrap();
        let distinct = snapshot_lock_path(&distinct).unwrap();

        assert_eq!(first.identity, alias.identity);
        assert_ne!(first.identity, distinct.identity);
        assert!(
            std::mem::size_of_val(&first.identity) >= 24,
            "Windows identity must retain the u64 volume and complete FILE_ID_128"
        );
    }

    #[cfg(windows)]
    #[test]
    fn durable_derived_store_windows_reparse_sentinel_is_rejected_without_target_mutation() {
        use std::os::windows::fs::symlink_file;

        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        let target = tempdir.path().join("target");
        let sentinel = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        fs::write(&db_path, b"canonical").unwrap();
        fs::write(&target, b"must-not-change").unwrap();
        if let Err(error) = symlink_file(&target, &sentinel) {
            assert_eq!(
                error.raw_os_error(),
                Some(1314),
                "only ERROR_PRIVILEGE_NOT_HELD may prevent the native reparse fixture"
            );
            let mut synthetic_reparse =
                snapshot_open_lock_file(&open_existing_lock_file(&target, false).unwrap()).unwrap();
            synthetic_reparse.regular_non_reparse = false;
            let error = validate_sentinel_snapshot(&sentinel, &synthetic_reparse)
                .expect_err("reparse attributes must fail closed");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(fs::read(&target).unwrap(), b"must-not-change");
            return;
        }

        assert!(DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").is_err());
        assert!(DerivedStoreReadGuard::acquire(&db_path, "tantivy_tasks").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"must-not-change");
        assert!(fs::symlink_metadata(&sentinel).unwrap().is_symlink());
    }

    #[cfg(windows)]
    #[test]
    fn windows_lock_contention_codes_are_normalized_for_sentinel_authority() {
        use windows_sys::Win32::Foundation::{
            ERROR_ACCESS_DENIED, ERROR_IO_PENDING, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION,
        };

        for code in [
            ERROR_LOCK_VIOLATION,
            ERROR_IO_PENDING,
            ERROR_SHARING_VIOLATION,
        ] {
            assert!(is_lock_contention_error(&io::Error::from_raw_os_error(
                code as i32
            )));
        }
        assert!(!is_lock_contention_error(&io::Error::from_raw_os_error(
            ERROR_ACCESS_DENIED as i32
        )));
    }

    #[cfg(windows)]
    #[test]
    fn windows_exclusive_sentinel_blocks_validation_through_second_handle() {
        use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;

        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "canonical-database").unwrap();
        let guard = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();
        let normalized_db_path = normalized_file_path(&db_path);
        let lock_path =
            derived_store_write_lock_path_from_normalized(&normalized_db_path, "tantivy_tasks");
        let expected_sentinel = derived_store_sentinel_bytes(
            &normalized_db_path,
            "tantivy_tasks",
            derived_store_database_lock_offset("tantivy_tasks"),
        );

        let error =
            open_and_validate_derived_store_sentinel(&lock_path, &expected_sentinel).unwrap_err();

        assert_eq!(error.raw_os_error(), Some(ERROR_LOCK_VIOLATION as i32));
        drop(guard);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_lock_path_cannot_fork_while_writer_is_held() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        fs::write(&db_path, "").unwrap();
        let lock_path = derived_store_write_lock_path(&db_path, "tantivy_tasks");
        let displaced_lock = tempdir.path().join("displaced-derived.lock");
        let first = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();

        match fs::rename(&lock_path, &displaced_lock) {
            Ok(()) => {
                let second = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks");
                assert!(
                    second.is_err(),
                    "renaming a held lock must not let a second writer lock a new inode"
                );
                assert!(
                    !lock_path.exists(),
                    "a failed second acquire must not publish a replacement lock inode"
                );
            }
            Err(_) => {
                assert!(
                    lock_path.exists(),
                    "a platform rename denial must leave the held lock path intact"
                );
                let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            }
        }
        drop(first);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_database_hardlink_alias_is_rejected_before_any_store_path_changes() {
        let tempdir = tempfile::tempdir().unwrap();
        let first_dir = tempdir.path().join("first");
        let second_dir = tempdir.path().join("second");
        fs::create_dir_all(&first_dir).unwrap();
        fs::create_dir_all(&second_dir).unwrap();
        let db_path = first_dir.join("kanban.db");
        let db_alias = second_dir.join("kanban.db");
        let canonical_bytes = b"canonical-sqlite-must-not-change";
        fs::write(&db_path, canonical_bytes).unwrap();
        fs::hard_link(&db_path, &db_alias).unwrap();

        let first_root =
            projection_store_root_path(&db_path, "db_hardlink", "tantivy_tasks").unwrap();
        let second_root =
            projection_store_root_path(&db_alias, "db_hardlink", "tantivy_tasks").unwrap();
        fs::create_dir_all(&first_root).unwrap();
        fs::create_dir_all(&second_root).unwrap();
        fs::write(first_root.join("sentinel"), b"first-root").unwrap();
        fs::write(second_root.join("sentinel"), b"second-root").unwrap();

        for database_path in [&db_path, &db_alias] {
            let write_error =
                DerivedStoreWriteGuard::acquire(database_path, "tantivy_tasks").unwrap_err();
            let read_error =
                DerivedStoreReadGuard::acquire(database_path, "tantivy_tasks").unwrap_err();
            assert_eq!(write_error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(read_error.kind(), io::ErrorKind::InvalidData);
            assert!(
                !derived_store_write_lock_path(database_path, "tantivy_tasks").exists(),
                "hard-linked database rejection must happen before sentinel publication"
            );
        }

        assert_eq!(fs::read(&db_path).unwrap(), canonical_bytes);
        assert_eq!(fs::read(&db_alias).unwrap(), canonical_bytes);
        assert_eq!(
            fs::read(first_root.join("sentinel")).unwrap(),
            b"first-root"
        );
        assert_eq!(
            fs::read(second_root.join("sentinel")).unwrap(),
            b"second-root"
        );
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_database_rename_does_not_fork_lock_authority() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        let renamed_db = tempdir.path().join("renamed.db");
        fs::write(&db_path, "").unwrap();
        let first = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();

        match fs::rename(&db_path, &renamed_db) {
            Ok(()) => {
                let error =
                    DerivedStoreWriteGuard::acquire(&renamed_db, "tantivy_tasks").unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            }
            Err(_) => {
                assert!(db_path.exists());
                let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            }
        }
        drop(first);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_database_replacement_does_not_fork_unmoved_sentinel_authority() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        let displaced_db = tempdir.path().join("displaced.db");
        fs::write(&db_path, "old-database").unwrap();
        let first = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();

        match fs::rename(&db_path, &displaced_db) {
            Ok(()) => {
                fs::write(&db_path, "replacement-database").unwrap();
                let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            }
            Err(_) => {
                assert!(db_path.exists());
                let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            }
        }
        drop(first);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn durable_derived_store_unrelated_sqlite_close_does_not_release_lock() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        let connection = rusqlite::Connection::open(&db_path).unwrap();
        connection
            .execute_batch("CREATE TABLE canonical(value INTEGER);")
            .unwrap();
        drop(connection);
        let writer = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();

        let unrelated = rusqlite::Connection::open(&db_path).unwrap();
        unrelated
            .query_row("SELECT COUNT(*) FROM canonical", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        drop(unrelated);
        let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        drop(writer);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn durable_derived_store_high_range_lock_does_not_block_sqlite_wal_vacuum_or_backup() {
        let tempdir = tempfile::tempdir().unwrap();
        let db_path = tempdir.path().join("kanban.db");
        let backup_path = tempdir.path().join("kanban-backup.db");
        let connection = rusqlite::Connection::open(&db_path).unwrap();
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = FULL;
                 CREATE TABLE canonical(id INTEGER PRIMARY KEY, payload BLOB NOT NULL);
                 WITH RECURSIVE rows(id) AS (
                   VALUES(1)
                   UNION ALL
                   SELECT id + 1 FROM rows WHERE id < 512
                 )
                 INSERT INTO canonical(id, payload)
                 SELECT id, zeroblob(4096) FROM rows;
                 PRAGMA wal_checkpoint(TRUNCATE);",
            )
            .unwrap();
        let size_before = fs::metadata(&db_path).unwrap().len();
        let writer = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap();

        connection.execute("DELETE FROM canonical", []).unwrap();
        connection
            .execute_batch(
                "PRAGMA wal_checkpoint(TRUNCATE);
                 VACUUM;",
            )
            .unwrap();
        connection
            .execute("VACUUM INTO ?1", [backup_path.to_string_lossy().as_ref()])
            .unwrap();
        connection
            .execute(
                "INSERT INTO canonical(id, payload) VALUES (1, zeroblob(64))",
                [],
            )
            .unwrap();
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();

        assert!(fs::metadata(&db_path).unwrap().len() < size_before);
        assert!(fs::metadata(&backup_path).unwrap().len() > 0);
        drop(connection);
        let error = DerivedStoreWriteGuard::acquire(&db_path, "tantivy_tasks").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        drop(writer);
    }

    #[test]
    fn durable_derived_store_lock_offset_contract_is_stable_unique_and_above_sqlite_maximum() {
        const SQLITE_MAX_PAGE_SIZE: u64 = 65_536;
        const SQLITE_MAX_PAGE_COUNT: u64 = 4_294_967_294;
        const SQLITE_MAXIMUM_FILE_SIZE: u64 = SQLITE_MAX_PAGE_SIZE * SQLITE_MAX_PAGE_COUNT;
        const CONTRACT_NAMES: [&str; 8] = [
            "tantivy_tasks",
            "oxigraph_relations",
            "lancedb_chunks",
            "lancedb_label_atoms",
            "tantivy_tasks-projection-helper",
            "oxigraph_relations-projection-helper",
            "lancedb_chunks-projection-helper",
            "lancedb_label_atoms-projection-helper",
        ];

        assert_eq!(DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET, 1_u64 << 48);
        assert!(
            DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET > std::hint::black_box(SQLITE_MAXIMUM_FILE_SIZE)
        );
        assert_eq!(
            DERIVED_STORE_DATABASE_LOCK_BASE,
            DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET + 1
        );

        let offsets = CONTRACT_NAMES.map(derived_store_database_lock_offset);
        assert!(offsets.iter().all(|offset| {
            *offset > DERIVED_DATABASE_LIFECYCLE_LOCK_OFFSET && *offset <= i64::MAX as u64
        }));
        assert_eq!(
            offsets
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            CONTRACT_NAMES.len(),
            "canonical stores and helper mutation locks must have unique offsets"
        );
        assert!(
            offsets
                .iter()
                .any(|offset| *offset - DERIVED_STORE_DATABASE_LOCK_BASE > u64::from(u32::MAX)),
            "the lock offset must consume more than the previous 32-bit hash bucket"
        );
        assert_eq!(
            offsets[0],
            derived_store_database_lock_offset(CONTRACT_NAMES[0])
        );
        let collision =
            validate_derived_store_lock_offset("custom-collision-witness", offsets[0]).unwrap_err();
        assert_eq!(collision.kind(), io::ErrorKind::InvalidData);
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
