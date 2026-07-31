use std::{
    fs::File,
    io::{self, BufReader, Read},
    path::{Component, Path, PathBuf},
};

#[cfg(target_os = "linux")]
use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    mem::MaybeUninit,
    os::unix::ffi::OsStringExt as _,
};

use fs_err as fs;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const CLEANUP_FORMAT_VERSION: u32 = 1;
const JOURNAL_FILE: &str = "journal.toml";
const MANIFEST_FILE: &str = "manifest.toml";
const ROOTS_DIR: &str = "roots";

/// The complete cleanup allowlist.
///
/// The DB-scoped `index/v2/databases` namespace is deliberately absent. New
/// entries require a source-backed format decision; callers must never discover
/// cleanup roots by walking `index` recursively.
pub const LEGACY_PROJECTION_ROOTS: [LegacyProjectionRootKind; 5] = [
    LegacyProjectionRootKind::TantivyV1,
    LegacyProjectionRootKind::OxigraphV1,
    LegacyProjectionRootKind::LanceDbV1,
    LegacyProjectionRootKind::TantivyUnscopedV2,
    LegacyProjectionRootKind::OxigraphUnscopedV2,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyProjectionRootKind {
    TantivyV1,
    OxigraphV1,
    LanceDbV1,
    TantivyUnscopedV2,
    OxigraphUnscopedV2,
}

impl LegacyProjectionRootKind {
    fn relative_path(self) -> &'static str {
        match self {
            Self::TantivyV1 => "index/v1/tasks",
            Self::OxigraphV1 => "index/v1/graph",
            Self::LanceDbV1 => "index/v1/vectors",
            Self::TantivyUnscopedV2 => "index/v2/tantivy_tasks",
            Self::OxigraphUnscopedV2 => "index/v2/oxigraph_relations",
        }
    }

    fn path_components(self) -> &'static [&'static str] {
        match self {
            Self::TantivyV1 => &["index", "v1", "tasks"],
            Self::OxigraphV1 => &["index", "v1", "graph"],
            Self::LanceDbV1 => &["index", "v1", "vectors"],
            Self::TantivyUnscopedV2 => &["index", "v2", "tantivy_tasks"],
            Self::OxigraphUnscopedV2 => &["index", "v2", "oxigraph_relations"],
        }
    }

    fn backup_name(self) -> &'static str {
        match self {
            Self::TantivyV1 => "tantivy_v1",
            Self::OxigraphV1 => "oxigraph_v1",
            Self::LanceDbV1 => "lancedb_v1",
            Self::TantivyUnscopedV2 => "tantivy_unscoped_v2",
            Self::OxigraphUnscopedV2 => "oxigraph_unscoped_v2",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyProjectionRootInventory {
    pub kind: LegacyProjectionRootKind,
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub present: bool,
    pub file_count: u64,
    pub directory_count: u64,
    pub byte_count: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyProjectionCleanupInventory {
    pub format_version: u32,
    pub database_instance_id: String,
    pub database_path: PathBuf,
    pub roots: Vec<LegacyProjectionRootInventory>,
    pub inventory_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyProjectionBackupManifest {
    pub format_version: u32,
    pub database_instance_id: String,
    pub database_path: PathBuf,
    pub inventory_digest: String,
    pub roots: Vec<LegacyProjectionRootInventory>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyProjectionCleanupOutcome {
    pub manifest: LegacyProjectionBackupManifest,
    pub resumed: bool,
}

fn noop_cleanup_before_initial_publish(
    _staging: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    Ok(())
}

fn noop_cleanup_before_move(
    _kind: LegacyProjectionRootKind,
    _source: &Path,
    _destination: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    Ok(())
}

fn noop_cleanup_after_move(
    _kind: LegacyProjectionRootKind,
) -> Result<(), LegacyProjectionCleanupError> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn noop_cleanup_atomic_hook() -> Result<(), LegacyProjectionCleanupError> {
    Ok(())
}

type CleanupBeforeInitialPublishHook =
    for<'path> fn(&'path Path) -> Result<(), LegacyProjectionCleanupError>;
type CleanupBeforeMoveHook =
    for<'source, 'destination> fn(
        LegacyProjectionRootKind,
        &'source Path,
        &'destination Path,
    ) -> Result<(), LegacyProjectionCleanupError>;
type CleanupFilesystemIdHook =
    for<'path> fn(&'path Path) -> Result<u64, LegacyProjectionCleanupError>;

/// Opaque proof that every legacy physical writer lock is held for one
/// canonical database file.
pub struct LegacyProjectionCleanupGuard {
    database_path: PathBuf,
    database_file: File,
    #[cfg(target_os = "linux")]
    database_parent_path: PathBuf,
    #[cfg(target_os = "linux")]
    database_parent: File,
    #[cfg(target_os = "linux")]
    database_name: OsString,
    #[cfg(target_os = "linux")]
    database_parent_identity: LinuxFileIdentity,
    #[cfg(target_os = "linux")]
    database_parent_mount_id: u64,
    _store_guards: Vec<super::DerivedStoreWriteGuard>,
}

#[derive(Debug, Error)]
pub enum LegacyProjectionCleanupError {
    #[error("legacy projection cleanup I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("legacy projection cleanup path is unsafe: {path}: {reason}")]
    UnsafePath { path: PathBuf, reason: String },
    #[error("legacy projection cleanup found an unsupported entry: {0}")]
    UnsupportedEntry(PathBuf),
    #[error(
        "legacy projection cleanup inventory digest mismatch: expected {expected}, got {actual}"
    )]
    DigestMismatch { expected: String, actual: String },
    #[error("legacy projection cleanup backup path overlaps managed data: {0}")]
    Overlap(PathBuf),
    #[error("legacy projection cleanup requires one filesystem: {source_path} and {backup}")]
    CrossFilesystem {
        source_path: PathBuf,
        backup: PathBuf,
    },
    #[error("legacy projection cleanup backup directory conflicts with existing state: {0}")]
    BackupConflict(PathBuf),
    #[error("legacy projection cleanup resume decision is invalid: {0}")]
    ResumeDecision(String),
    #[error("legacy projection cleanup journal is incompatible: {0}")]
    JournalConflict(String),
    #[error("legacy projection cleanup manifest is incompatible: {0}")]
    ManifestConflict(String),
    #[error("legacy projection cleanup journal encoding failed: {0}")]
    JournalEncode(#[from] toml::ser::Error),
    #[error("legacy projection cleanup journal decoding failed: {0}")]
    JournalDecode(#[from] toml::de::Error),
}

/// Inventories the fixed legacy allowlist without creating files, locks, or
/// SQLite connections.
pub fn inventory_legacy_projection_roots(
    db_path: impl AsRef<Path>,
    database_instance_id: &str,
) -> Result<LegacyProjectionCleanupInventory, LegacyProjectionCleanupError> {
    validate_database_instance_id(database_instance_id)?;
    let database_path = canonical_database_path(db_path.as_ref())?;
    inventory_for_canonical_database(&database_path, database_instance_id)
}

/// Acquires every physical writer guard used by the five legacy roots.
///
/// The order is fixed to make concurrent cleanup attempts fail without
/// deadlock. The SQLite service remains responsible for validating the supplied
/// database instance id and excluding the singleton maintenance owner before it
/// calls a destructive primitive.
pub fn acquire_legacy_projection_cleanup_guard(
    db_path: impl AsRef<Path>,
) -> Result<LegacyProjectionCleanupGuard, LegacyProjectionCleanupError> {
    let database_path = canonical_database_path(db_path.as_ref())?;
    let database_file = File::open(&database_path)?;
    let mut store_guards = Vec::with_capacity(4);
    for store_name in [
        "tantivy_tasks",
        "oxigraph_relations",
        "lancedb_chunks",
        "lancedb_label_atoms",
    ] {
        store_guards.push(super::DerivedStoreWriteGuard::acquire(
            &database_path,
            store_name,
        )?);
    }
    #[cfg(target_os = "linux")]
    let database_parent_path = database_path
        .parent()
        .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
            path: database_path.clone(),
            reason: "canonical database path has no parent".to_owned(),
        })?
        .to_path_buf();
    #[cfg(target_os = "linux")]
    let database_name = database_path
        .file_name()
        .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
            path: database_path.clone(),
            reason: "canonical database path has no final component".to_owned(),
        })?
        .to_os_string();
    #[cfg(target_os = "linux")]
    let database_parent = linux_open_stable_directory(&database_parent_path)?;
    #[cfg(target_os = "linux")]
    let database_parent_snapshot = linux_snapshot_fd(&database_parent)?;
    let guard = LegacyProjectionCleanupGuard {
        database_path,
        database_file,
        #[cfg(target_os = "linux")]
        database_parent_path,
        #[cfg(target_os = "linux")]
        database_parent,
        #[cfg(target_os = "linux")]
        database_name,
        #[cfg(target_os = "linux")]
        database_parent_identity: database_parent_snapshot.identity,
        #[cfg(target_os = "linux")]
        database_parent_mount_id: database_parent_snapshot.mount_id,
        _store_guards: store_guards,
    };
    guard.validate(db_path.as_ref())?;
    Ok(guard)
}

/// Moves every present legacy root into a same-filesystem backup.
///
/// The caller must exclude maintenance and hold the relevant physical-store
/// write guards for the whole call. This filesystem primitive intentionally
/// does not acquire persistent lock files because inventory/dry-run callers
/// must remain byte-for-byte read-only.
pub fn apply_legacy_projection_cleanup(
    guard: &LegacyProjectionCleanupGuard,
    db_path: impl AsRef<Path>,
    database_instance_id: &str,
    expected_inventory_digest: &str,
    backup_dir: impl AsRef<Path>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    guard.validate(db_path.as_ref())?;
    apply_legacy_projection_cleanup_inner(
        guard,
        db_path.as_ref(),
        database_instance_id,
        expected_inventory_digest,
        backup_dir.as_ref(),
        None,
        ApplyHooks {
            before_initial_publish: noop_cleanup_before_initial_publish
                as CleanupBeforeInitialPublishHook,
            before_move: noop_cleanup_before_move as CleanupBeforeMoveHook,
            after_move: noop_cleanup_after_move,
        },
        filesystem_id as CleanupFilesystemIdHook,
    )
}

/// Applies cleanup only when the backup namespace matches the caller's
/// explicit fresh/resume decision.
///
/// This check is performed inside the same fail-closed path that observes and
/// opens the backup namespace, so a preflight-to-apply race cannot silently
/// turn a fresh operation into journal resume.
pub fn apply_legacy_projection_cleanup_with_resume_decision(
    guard: &LegacyProjectionCleanupGuard,
    db_path: impl AsRef<Path>,
    database_instance_id: &str,
    expected_inventory_digest: &str,
    backup_dir: impl AsRef<Path>,
    resume: bool,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    guard.validate(db_path.as_ref())?;
    apply_legacy_projection_cleanup_inner(
        guard,
        db_path.as_ref(),
        database_instance_id,
        expected_inventory_digest,
        backup_dir.as_ref(),
        Some(resume),
        ApplyHooks {
            before_initial_publish: noop_cleanup_before_initial_publish
                as CleanupBeforeInitialPublishHook,
            before_move: noop_cleanup_before_move as CleanupBeforeMoveHook,
            after_move: noop_cleanup_after_move,
        },
        filesystem_id as CleanupFilesystemIdHook,
    )
}

/// Re-hashes every backed-up root and checks the durable journal and manifest.
pub fn verify_legacy_projection_backup(
    db_path: impl AsRef<Path>,
    database_instance_id: &str,
    backup_dir: impl AsRef<Path>,
) -> Result<LegacyProjectionBackupManifest, LegacyProjectionCleanupError> {
    validate_database_instance_id(database_instance_id)?;
    let database_path = canonical_database_path(db_path.as_ref())?;
    let backup_dir = validate_backup_path(&database_path, backup_dir.as_ref(), true)?;
    let journal =
        load_and_validate_journal_read_only(&database_path, database_instance_id, &backup_dir)?;
    if journal.phase != CleanupPhase::Completed {
        return Err(LegacyProjectionCleanupError::JournalConflict(format!(
            "backup is not complete: {:?}",
            journal.phase
        )));
    }
    validate_completed_backup(&journal, &backup_dir)?;
    load_and_validate_manifest(&journal, &backup_dir)
}

/// Restores a completed backup using the same crash-resumable journal.
///
/// As with apply, the caller must hold maintenance exclusion and all relevant
/// physical-store write guards.
pub fn restore_legacy_projection_backup(
    guard: &LegacyProjectionCleanupGuard,
    db_path: impl AsRef<Path>,
    database_instance_id: &str,
    backup_dir: impl AsRef<Path>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    guard.validate(db_path.as_ref())?;
    restore_legacy_projection_backup_inner(
        guard,
        db_path.as_ref(),
        database_instance_id,
        backup_dir.as_ref(),
        noop_cleanup_before_move as CleanupBeforeMoveHook,
        noop_cleanup_after_move,
        filesystem_id as CleanupFilesystemIdHook,
    )
}

#[cfg(all(test, target_os = "linux"))]
fn apply_legacy_projection_cleanup_with_after_move(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    expected_inventory_digest: &str,
    backup_dir: &Path,
    after_move: impl FnMut(LegacyProjectionRootKind) -> Result<(), LegacyProjectionCleanupError>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    apply_legacy_projection_cleanup_inner(
        guard,
        db_path,
        database_instance_id,
        expected_inventory_digest,
        backup_dir,
        None,
        ApplyHooks {
            before_initial_publish: noop_cleanup_before_initial_publish
                as CleanupBeforeInitialPublishHook,
            before_move: noop_cleanup_before_move as CleanupBeforeMoveHook,
            after_move,
        },
        filesystem_id as CleanupFilesystemIdHook,
    )
}

#[cfg(all(test, target_os = "linux"))]
fn apply_legacy_projection_cleanup_with_before_initial_publish(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    expected_inventory_digest: &str,
    backup_dir: &Path,
    before_initial_publish: impl for<'path> FnMut(
        &'path Path,
    ) -> Result<(), LegacyProjectionCleanupError>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    apply_legacy_projection_cleanup_inner(
        guard,
        db_path,
        database_instance_id,
        expected_inventory_digest,
        backup_dir,
        None,
        ApplyHooks {
            before_initial_publish,
            before_move: noop_cleanup_before_move as CleanupBeforeMoveHook,
            after_move: noop_cleanup_after_move,
        },
        filesystem_id as CleanupFilesystemIdHook,
    )
}

#[cfg(all(test, target_os = "linux"))]
fn apply_legacy_projection_cleanup_with_before_move(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    expected_inventory_digest: &str,
    backup_dir: &Path,
    before_move: impl for<'source, 'destination> FnMut(
        LegacyProjectionRootKind,
        &'source Path,
        &'destination Path,
    ) -> Result<(), LegacyProjectionCleanupError>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    apply_legacy_projection_cleanup_inner(
        guard,
        db_path,
        database_instance_id,
        expected_inventory_digest,
        backup_dir,
        None,
        ApplyHooks {
            before_initial_publish: noop_cleanup_before_initial_publish
                as CleanupBeforeInitialPublishHook,
            before_move,
            after_move: noop_cleanup_after_move,
        },
        filesystem_id as CleanupFilesystemIdHook,
    )
}

#[cfg(all(test, target_os = "linux"))]
fn apply_legacy_projection_cleanup_with_filesystem_id(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    expected_inventory_digest: &str,
    backup_dir: &Path,
    filesystem_id: impl for<'path> FnMut(&'path Path) -> Result<u64, LegacyProjectionCleanupError>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    apply_legacy_projection_cleanup_inner(
        guard,
        db_path,
        database_instance_id,
        expected_inventory_digest,
        backup_dir,
        None,
        ApplyHooks {
            before_initial_publish: noop_cleanup_before_initial_publish
                as CleanupBeforeInitialPublishHook,
            before_move: noop_cleanup_before_move as CleanupBeforeMoveHook,
            after_move: noop_cleanup_after_move,
        },
        filesystem_id,
    )
}

#[cfg(all(test, target_os = "linux"))]
fn restore_legacy_projection_backup_with_after_move(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    backup_dir: &Path,
    after_move: impl FnMut(LegacyProjectionRootKind) -> Result<(), LegacyProjectionCleanupError>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    restore_legacy_projection_backup_inner(
        guard,
        db_path,
        database_instance_id,
        backup_dir,
        noop_cleanup_before_move as CleanupBeforeMoveHook,
        after_move,
        filesystem_id as CleanupFilesystemIdHook,
    )
}

#[cfg(all(test, target_os = "linux"))]
fn restore_legacy_projection_backup_with_before_move(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    backup_dir: &Path,
    before_move: impl for<'source, 'destination> FnMut(
        LegacyProjectionRootKind,
        &'source Path,
        &'destination Path,
    ) -> Result<(), LegacyProjectionCleanupError>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    restore_legacy_projection_backup_inner(
        guard,
        db_path,
        database_instance_id,
        backup_dir,
        before_move,
        noop_cleanup_after_move,
        filesystem_id as CleanupFilesystemIdHook,
    )
}

#[cfg(all(test, target_os = "linux"))]
fn restore_legacy_projection_backup_with_filesystem_id(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    backup_dir: &Path,
    filesystem_id: impl for<'path> FnMut(&'path Path) -> Result<u64, LegacyProjectionCleanupError>,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError> {
    restore_legacy_projection_backup_inner(
        guard,
        db_path,
        database_instance_id,
        backup_dir,
        noop_cleanup_before_move as CleanupBeforeMoveHook,
        noop_cleanup_after_move,
        filesystem_id,
    )
}

fn require_same_filesystem_id(
    source_id: u64,
    backup_id: u64,
    source: &Path,
    backup: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    if source_id != backup_id {
        return Err(LegacyProjectionCleanupError::CrossFilesystem {
            source_path: source.to_path_buf(),
            backup: backup.to_path_buf(),
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CleanupPhase {
    Applying,
    Completed,
    Restoring,
    Restored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CleanupRootState {
    Absent,
    Pending,
    Moved,
    Restored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CleanupJournalRoot {
    inventory: LegacyProjectionRootInventory,
    state: CleanupRootState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CleanupJournal {
    format_version: u32,
    database_instance_id: String,
    database_path: PathBuf,
    inventory_digest: String,
    phase: CleanupPhase,
    roots: Vec<CleanupJournalRoot>,
}

fn validate_database_instance_id(
    database_instance_id: &str,
) -> Result<(), LegacyProjectionCleanupError> {
    if !database_instance_id.starts_with("db_")
        || !database_instance_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(LegacyProjectionCleanupError::JournalConflict(
            "database instance id is not path-safe".to_owned(),
        ));
    }
    Ok(())
}

fn canonical_database_path(db_path: &Path) -> Result<PathBuf, LegacyProjectionCleanupError> {
    let canonical = fs::canonicalize(db_path)?;
    let metadata = fs::symlink_metadata(&canonical)?;
    if !metadata.is_file() {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: canonical,
            reason: "canonical database path is not a regular file".to_owned(),
        });
    }
    require_utf8_path(&canonical)?;
    if canonical.parent().is_none() {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: canonical,
            reason: "canonical database path has no parent".to_owned(),
        });
    }
    Ok(canonical)
}

impl LegacyProjectionCleanupGuard {
    fn validate(&self, db_path: &Path) -> Result<(), LegacyProjectionCleanupError> {
        let canonical = canonical_database_path(db_path)?;
        #[cfg(not(windows))]
        let held = self.database_file.metadata()?;
        #[cfg(not(windows))]
        let current = fs::symlink_metadata(&canonical)?;
        #[cfg(not(windows))]
        let identity_matches = same_file_identity(&held, &current);
        #[cfg(windows)]
        let identity_matches = windows_snapshot_handle(&self.database_file)?.identity
            == windows_snapshot_path(&canonical, false)?.identity;
        if canonical != self.database_path || !identity_matches {
            return Err(LegacyProjectionCleanupError::JournalConflict(
                "database file identity changed after cleanup guards were acquired".to_owned(),
            ));
        }
        #[cfg(target_os = "linux")]
        {
            let canonical_parent =
                canonical
                    .parent()
                    .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                        path: canonical.clone(),
                        reason: "canonical database path has no parent".to_owned(),
                    })?;
            if canonical_parent != self.database_parent_path {
                return Err(LegacyProjectionCleanupError::JournalConflict(
                    "database parent path changed after cleanup guards were acquired".to_owned(),
                ));
            }
            linux_validate_directory_handle_path(
                &self.database_parent_path,
                &self.database_parent,
            )?;
            let parent_snapshot = linux_snapshot_fd(&self.database_parent)?;
            let database_snapshot =
                linux_snapshot_at(&self.database_parent, self.database_name.as_os_str())?;
            let held_database = linux_snapshot_fd(&self.database_file)?;
            if parent_snapshot.identity != self.database_parent_identity
                || parent_snapshot.mount_id != self.database_parent_mount_id
                || database_snapshot.identity != held_database.identity
                || database_snapshot.mount_id != self.database_parent_mount_id
            {
                return Err(LegacyProjectionCleanupError::JournalConflict(
                    "database parent binding changed after cleanup guards were acquired".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

fn inventory_for_canonical_database(
    database_path: &Path,
    database_instance_id: &str,
) -> Result<LegacyProjectionCleanupInventory, LegacyProjectionCleanupError> {
    let database_parent =
        database_path
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: database_path.to_path_buf(),
                reason: "canonical database path has no parent".to_owned(),
            })?;
    let mut roots = Vec::with_capacity(LEGACY_PROJECTION_ROOTS.len());
    for kind in LEGACY_PROJECTION_ROOTS {
        roots.push(inventory_root(database_parent, kind)?);
    }
    let inventory_digest = inventory_digest(database_path, database_instance_id, &roots)?;
    Ok(LegacyProjectionCleanupInventory {
        format_version: CLEANUP_FORMAT_VERSION,
        database_instance_id: database_instance_id.to_owned(),
        database_path: database_path.to_path_buf(),
        roots,
        inventory_digest,
    })
}

fn inventory_root(
    database_parent: &Path,
    kind: LegacyProjectionRootKind,
) -> Result<LegacyProjectionRootInventory, LegacyProjectionCleanupError> {
    let absolute_path = database_parent.join(kind.relative_path());
    let mut current = database_parent.to_path_buf();
    let mut present = true;
    #[cfg(windows)]
    let mut allowlist_directory_guards = Vec::new();
    for component in kind.path_components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                #[cfg(windows)]
                {
                    let guard = windows_open_inventory_directory(&current)?;
                    let snapshot = windows_snapshot_handle(&guard)?;
                    if !metadata.is_dir()
                        || !windows_metadata_matches_snapshot(&metadata, &snapshot)
                    {
                        return Err(LegacyProjectionCleanupError::UnsafePath {
                            path: current,
                            reason: "legacy managed path component changed while opening"
                                .to_owned(),
                        });
                    }
                    allowlist_directory_guards.push(guard);
                }
                #[cfg(not(windows))]
                if !metadata.is_dir() {
                    return Err(LegacyProjectionCleanupError::UnsafePath {
                        path: current,
                        reason: "legacy managed path component is not a real directory".to_owned(),
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                present = false;
                break;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let inventory = if present {
        scan_present_root(kind, &absolute_path, &absolute_path)
    } else {
        Ok(absent_root_inventory(kind, absolute_path))
    };
    #[cfg(windows)]
    drop(allowlist_directory_guards);
    inventory
}

fn absent_root_inventory(
    kind: LegacyProjectionRootKind,
    absolute_path: PathBuf,
) -> LegacyProjectionRootInventory {
    let mut digest = Sha256::new();
    hash_field(&mut digest, b"kanban-legacy-projection-root-v1");
    hash_field(&mut digest, kind.backup_name().as_bytes());
    hash_field(&mut digest, b"absent");
    LegacyProjectionRootInventory {
        kind,
        relative_path: kind.relative_path().to_owned(),
        absolute_path,
        present: false,
        file_count: 0,
        directory_count: 0,
        byte_count: 0,
        digest: finish_digest(digest),
    }
}

#[cfg(not(target_os = "linux"))]
fn scan_present_root(
    kind: LegacyProjectionRootKind,
    scan_path: &Path,
    source_absolute_path: &Path,
) -> Result<LegacyProjectionRootInventory, LegacyProjectionCleanupError> {
    let metadata = fs::symlink_metadata(scan_path)?;
    if !metadata.is_dir() {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: scan_path.to_path_buf(),
            reason: "legacy root is not a real directory".to_owned(),
        });
    }
    let mut digest = Sha256::new();
    hash_field(&mut digest, b"kanban-legacy-projection-root-v1");
    hash_field(&mut digest, kind.backup_name().as_bytes());
    hash_field(&mut digest, b"present");
    let mut counts = TreeCounts {
        files: 0,
        directories: 1,
        bytes: 0,
    };
    hash_field(&mut digest, b"directory");
    hash_field(&mut digest, b"");
    scan_tree(scan_path, scan_path, &mut digest, &mut counts)?;
    Ok(LegacyProjectionRootInventory {
        kind,
        relative_path: kind.relative_path().to_owned(),
        absolute_path: source_absolute_path.to_path_buf(),
        present: true,
        file_count: counts.files,
        directory_count: counts.directories,
        byte_count: counts.bytes,
        digest: finish_digest(digest),
    })
}

#[cfg(target_os = "linux")]
fn scan_present_root(
    kind: LegacyProjectionRootKind,
    scan_path: &Path,
    source_absolute_path: &Path,
) -> Result<LegacyProjectionRootInventory, LegacyProjectionCleanupError> {
    let (inventory, _ticket) =
        linux_scan_present_root_with_ticket(kind, scan_path, source_absolute_path, false)?;
    Ok(inventory)
}

#[derive(Debug, Clone, Copy)]
struct TreeCounts {
    files: u64,
    directories: u64,
    bytes: u64,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct LinuxFileIdentity {
    dev_major: u32,
    dev_minor: u32,
    ino: u64,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LinuxFileSnapshot {
    identity: LinuxFileIdentity,
    mode: u32,
    len: u64,
    mtime: i64,
    mtime_nsec: u64,
    ctime: i64,
    ctime_nsec: u64,
    mount_id: u64,
}

#[cfg(target_os = "linux")]
struct RootScanTicket {
    source_path: PathBuf,
    source_parent_path: PathBuf,
    source_name: OsString,
    source_parent: File,
    source_directory: File,
    source_parent_snapshot: LinuxFileSnapshot,
    source_snapshot: LinuxFileSnapshot,
}

#[cfg(target_os = "linux")]
struct LinuxScanContext<'scan> {
    display_root: &'scan Path,
    root_mount_id: u64,
    durable: bool,
    visited: &'scan mut HashSet<LinuxFileIdentity>,
    digest: &'scan mut Sha256,
    counts: &'scan mut TreeCounts,
}

#[cfg(target_os = "linux")]
fn linux_scan_present_root_with_ticket(
    kind: LegacyProjectionRootKind,
    scan_path: &Path,
    source_absolute_path: &Path,
    durable: bool,
) -> Result<(LegacyProjectionRootInventory, RootScanTicket), LegacyProjectionCleanupError> {
    let ticket = linux_open_root_scan_ticket(scan_path)?;
    let mut digest = Sha256::new();
    hash_field(&mut digest, b"kanban-legacy-projection-root-v1");
    hash_field(&mut digest, kind.backup_name().as_bytes());
    hash_field(&mut digest, b"present");
    hash_field(&mut digest, b"directory");
    hash_field(&mut digest, b"");
    let mut counts = TreeCounts {
        files: 0,
        directories: 1,
        bytes: 0,
    };
    let mut visited = HashSet::new();
    visited.insert(ticket.source_snapshot.identity);
    let mut context = LinuxScanContext {
        display_root: &ticket.source_path,
        root_mount_id: ticket.source_snapshot.mount_id,
        durable,
        visited: &mut visited,
        digest: &mut digest,
        counts: &mut counts,
    };
    linux_scan_directory(&ticket.source_directory, "", &mut context)?;
    linux_validate_root_ticket(&ticket)?;
    Ok((
        LegacyProjectionRootInventory {
            kind,
            relative_path: kind.relative_path().to_owned(),
            absolute_path: source_absolute_path.to_path_buf(),
            present: true,
            file_count: counts.files,
            directory_count: counts.directories,
            byte_count: counts.bytes,
            digest: finish_digest(digest),
        },
        ticket,
    ))
}

#[cfg(target_os = "linux")]
fn linux_open_root_scan_ticket(
    source: &Path,
) -> Result<RootScanTicket, LegacyProjectionCleanupError> {
    use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};

    let source_parent_path =
        source
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: source.to_path_buf(),
                reason: "legacy root has no parent".to_owned(),
            })?;
    let source_name = source
        .file_name()
        .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
            path: source.to_path_buf(),
            reason: "legacy root has no final component".to_owned(),
        })?
        .to_os_string();
    let source_parent = linux_open_stable_directory(source_parent_path)?;
    let source_parent_snapshot = linux_snapshot_fd(&source_parent)?;
    let source_directory: File = match openat2(
        &source_parent,
        source_name.as_os_str(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
    ) {
        Ok(directory) => directory.into(),
        Err(rustix::io::Errno::XDEV) => {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: source.to_path_buf(),
                reason: "legacy root crosses a mount boundary".to_owned(),
            });
        }
        Err(error) => return Err(linux_errno(error)),
    };
    let source_snapshot = linux_snapshot_fd(&source_directory)?;
    if rustix::fs::FileType::from_raw_mode(source_snapshot.mode) != rustix::fs::FileType::Directory
        || source_snapshot.mount_id != source_parent_snapshot.mount_id
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: source.to_path_buf(),
            reason: "legacy root crosses a mount boundary or is not a directory".to_owned(),
        });
    }
    let ticket = RootScanTicket {
        source_path: source.to_path_buf(),
        source_parent_path: source_parent_path.to_path_buf(),
        source_name,
        source_parent,
        source_directory,
        source_parent_snapshot,
        source_snapshot,
    };
    linux_validate_root_ticket(&ticket)?;
    Ok(ticket)
}

#[cfg(target_os = "linux")]
fn linux_open_stable_directory(path: &Path) -> Result<File, LegacyProjectionCleanupError> {
    use rustix::fs::{CWD, Mode, OFlags, openat};

    let before = fs::symlink_metadata(path)?;
    if !before.is_dir() {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "directory handle path is not a real directory".to_owned(),
        });
    }
    let directory: File = openat(
        CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(linux_errno)?
    .into();
    let opened = directory.metadata()?;
    let after = fs::symlink_metadata(path)?;
    if !opened.is_dir()
        || !after.is_dir()
        || !same_file_identity(&before, &opened)
        || !same_file_identity(&before, &after)
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "directory identity changed while opening".to_owned(),
        });
    }
    Ok(directory)
}

#[cfg(target_os = "linux")]
fn linux_validate_directory_handle_path(
    path: &Path,
    directory: &File,
) -> Result<(), LegacyProjectionCleanupError> {
    validate_real_directory_chain(path)?;
    let reopened = linux_open_stable_directory(path)?;
    let expected = linux_snapshot_fd(directory)?;
    let actual = linux_snapshot_fd(&reopened)?;
    if expected.identity != actual.identity || expected.mount_id != actual.mount_id {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "directory path no longer resolves to the held descriptor".to_owned(),
        });
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_open_managed_root_parent(
    guard: &LegacyProjectionCleanupGuard,
    database_path: &Path,
    kind: LegacyProjectionRootKind,
) -> Result<File, LegacyProjectionCleanupError> {
    use rustix::fs::{Mode, OFlags, ResolveFlags, openat2};

    guard.validate(database_path)?;
    let database_parent = &guard.database_parent_path;
    let database_parent_snapshot = linux_snapshot_fd(&guard.database_parent)?;

    let mut relative_parent = PathBuf::new();
    let components = kind.path_components();
    for component in &components[..components.len() - 1] {
        relative_parent.push(component);
    }
    let managed_parent_path = database_parent.join(&relative_parent);
    let managed_parent: File = match openat2(
        &guard.database_parent,
        &relative_parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
    ) {
        Ok(directory) => directory.into(),
        Err(rustix::io::Errno::XDEV | rustix::io::Errno::LOOP) => {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: managed_parent_path,
                reason: "managed restore ancestry crosses a mount or symbolic link".to_owned(),
            });
        }
        Err(error) => return Err(linux_errno(error)),
    };
    let managed_snapshot = linux_snapshot_fd(&managed_parent)?;
    if rustix::fs::FileType::from_raw_mode(managed_snapshot.mode) != rustix::fs::FileType::Directory
        || managed_snapshot.mount_id != database_parent_snapshot.mount_id
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: managed_parent_path,
            reason: "managed restore parent is outside the database mount".to_owned(),
        });
    }
    guard.validate(database_path)?;
    Ok(managed_parent)
}

#[cfg(target_os = "linux")]
fn linux_scan_directory(
    directory: &File,
    relative_parent: &str,
    context: &mut LinuxScanContext<'_>,
) -> Result<(), LegacyProjectionCleanupError> {
    use rustix::fs::{FileType, Mode, OFlags, RawDir, ResolveFlags, openat2};

    let before = linux_snapshot_fd(directory)?;
    if FileType::from_raw_mode(before.mode) != FileType::Directory
        || before.mount_id != context.root_mount_id
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: context.display_root.join(relative_parent),
            reason: "legacy directory crossed a mount boundary".to_owned(),
        });
    }
    let mut buffer = [MaybeUninit::uninit(); 16 * 1024];
    let mut iterator = RawDir::new(directory, &mut buffer);
    let mut names = Vec::<Vec<u8>>::new();
    while let Some(entry) = iterator.next() {
        let entry = entry.map_err(linux_errno)?;
        let name = entry.file_name().to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        if name.is_empty() || name.contains(&b'/') {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: context.display_root.join(relative_parent),
                reason: "legacy directory contains an invalid entry name".to_owned(),
            });
        }
        names.push(name.to_vec());
    }
    names.sort();

    for name in names {
        let name = OsString::from_vec(name);
        let relative = linux_join_relative(relative_parent, &name, context.display_root)?;
        let display_path = context.display_root.join(&relative);
        let path_snapshot = linux_snapshot_at(directory, name.as_os_str())?;
        let file_type = FileType::from_raw_mode(path_snapshot.mode);
        if !matches!(file_type, FileType::Directory | FileType::RegularFile) {
            return Err(LegacyProjectionCleanupError::UnsupportedEntry(display_path));
        }
        let flags = OFlags::RDONLY
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | if file_type == FileType::Directory {
                OFlags::DIRECTORY
            } else {
                OFlags::empty()
            };
        let opened: File = match openat2(
            directory,
            name.as_os_str(),
            flags,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        ) {
            Ok(opened) => opened.into(),
            Err(rustix::io::Errno::XDEV) => {
                return Err(LegacyProjectionCleanupError::UnsafePath {
                    path: display_path,
                    reason: "legacy entry crosses a nested mount boundary".to_owned(),
                });
            }
            Err(error) => return Err(linux_errno(error)),
        };
        let opened_before = linux_snapshot_fd(&opened)?;
        if opened_before != path_snapshot || opened_before.mount_id != context.root_mount_id {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: display_path,
                reason: "legacy entry changed while opening or crossed a mount boundary".to_owned(),
            });
        }
        if !context.visited.insert(opened_before.identity) {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: display_path,
                reason: "legacy tree contains a repeated inode identity".to_owned(),
            });
        }

        if file_type == FileType::Directory {
            context.counts.directories =
                context.counts.directories.checked_add(1).ok_or_else(|| {
                    LegacyProjectionCleanupError::JournalConflict(
                        "legacy directory count overflow".to_owned(),
                    )
                })?;
            hash_field(context.digest, b"directory");
            hash_field(context.digest, relative.as_bytes());
            linux_scan_directory(&opened, &relative, context)?;
            if context.durable {
                opened.sync_all()?;
            }
        } else {
            context.counts.files = context.counts.files.checked_add(1).ok_or_else(|| {
                LegacyProjectionCleanupError::JournalConflict(
                    "legacy file count overflow".to_owned(),
                )
            })?;
            let len = opened_before.len;
            context.counts.bytes = context.counts.bytes.checked_add(len).ok_or_else(|| {
                LegacyProjectionCleanupError::JournalConflict(
                    "legacy byte count overflow".to_owned(),
                )
            })?;
            hash_field(context.digest, b"file");
            hash_field(context.digest, relative.as_bytes());
            hash_field(context.digest, &len.to_be_bytes());
            linux_hash_regular_file(
                opened,
                directory,
                name.as_os_str(),
                &display_path,
                opened_before,
                context.durable,
                context.digest,
            )?;
        }
        if linux_snapshot_at(directory, name.as_os_str())? != opened_before {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: display_path,
                reason: "legacy entry changed during inventory".to_owned(),
            });
        }
    }
    let after = linux_snapshot_fd(directory)?;
    if before != after {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: context.display_root.join(relative_parent),
            reason: "directory changed during legacy inventory".to_owned(),
        });
    }
    if context.durable {
        directory.sync_all()?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_hash_regular_file(
    file: File,
    parent: &File,
    name: &OsStr,
    display_path: &Path,
    before: LinuxFileSnapshot,
    durable: bool,
    digest: &mut Sha256,
) -> Result<(), LegacyProjectionCleanupError> {
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 64 * 1024];
    let mut read_bytes = 0_u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        read_bytes = read_bytes.checked_add(read as u64).ok_or_else(|| {
            LegacyProjectionCleanupError::JournalConflict(
                "legacy file read count overflow".to_owned(),
            )
        })?;
    }
    if durable {
        reader.get_ref().sync_all()?;
    }
    let opened_after = linux_snapshot_fd(reader.get_ref())?;
    if read_bytes != before.len
        || opened_after != before
        || linux_snapshot_at(parent, name)? != before
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: display_path.to_path_buf(),
            reason: "file changed during legacy inventory".to_owned(),
        });
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_join_relative(
    parent: &str,
    name: &OsStr,
    display_root: &Path,
) -> Result<String, LegacyProjectionCleanupError> {
    let name = name
        .to_str()
        .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
            path: display_root.join(parent),
            reason: "legacy entry path is not UTF-8".to_owned(),
        })?;
    if name.is_empty() || matches!(name, "." | "..") || name.contains('/') {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: display_root.join(parent),
            reason: "legacy entry has a non-normal path component".to_owned(),
        });
    }
    Ok(if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    })
}

#[cfg(target_os = "linux")]
fn linux_snapshot_fd(file: &File) -> Result<LinuxFileSnapshot, LegacyProjectionCleanupError> {
    use rustix::fs::AtFlags;

    linux_snapshot_at_with_flags(
        file,
        OsStr::new(""),
        AtFlags::EMPTY_PATH | AtFlags::NO_AUTOMOUNT,
    )
}

#[cfg(target_os = "linux")]
fn linux_snapshot_at(
    directory: &File,
    path: &OsStr,
) -> Result<LinuxFileSnapshot, LegacyProjectionCleanupError> {
    use rustix::fs::AtFlags;

    linux_snapshot_at_with_flags(
        directory,
        path,
        AtFlags::SYMLINK_NOFOLLOW | AtFlags::NO_AUTOMOUNT,
    )
}

#[cfg(target_os = "linux")]
fn linux_snapshot_at_with_flags(
    directory: &File,
    path: &OsStr,
    flags: rustix::fs::AtFlags,
) -> Result<LinuxFileSnapshot, LegacyProjectionCleanupError> {
    use rustix::fs::{StatxFlags, statx};

    let mask = StatxFlags::BASIC_STATS | StatxFlags::MNT_ID;
    let stat = statx(directory, path, flags, mask).map_err(linux_errno)?;
    let required = StatxFlags::TYPE
        | StatxFlags::MODE
        | StatxFlags::INO
        | StatxFlags::SIZE
        | StatxFlags::MTIME
        | StatxFlags::CTIME
        | StatxFlags::MNT_ID;
    if stat.stx_mask & required.bits() != required.bits() {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: PathBuf::from("<fd>"),
            reason: "Linux statx did not return the complete stable identity mask".to_owned(),
        });
    }
    Ok(LinuxFileSnapshot {
        identity: LinuxFileIdentity {
            dev_major: stat.stx_dev_major,
            dev_minor: stat.stx_dev_minor,
            ino: stat.stx_ino,
        },
        mode: u32::from(stat.stx_mode),
        len: stat.stx_size,
        mtime: stat.stx_mtime.tv_sec,
        mtime_nsec: u64::from(stat.stx_mtime.tv_nsec),
        ctime: stat.stx_ctime.tv_sec,
        ctime_nsec: u64::from(stat.stx_ctime.tv_nsec),
        mount_id: stat.stx_mnt_id,
    })
}

#[cfg(target_os = "linux")]
fn linux_snapshot_matches_after_rename(
    before: LinuxFileSnapshot,
    after: LinuxFileSnapshot,
) -> bool {
    before.identity == after.identity
        && before.mode == after.mode
        && before.len == after.len
        && before.mtime == after.mtime
        && before.mtime_nsec == after.mtime_nsec
        && before.mount_id == after.mount_id
}

#[cfg(target_os = "linux")]
fn linux_validate_root_ticket(ticket: &RootScanTicket) -> Result<(), LegacyProjectionCleanupError> {
    let current_parent_path = fs::symlink_metadata(&ticket.source_parent_path)?;
    let held_parent = ticket.source_parent.metadata()?;
    if !current_parent_path.is_dir()
        || !held_parent.is_dir()
        || !same_file_identity(&current_parent_path, &held_parent)
        || linux_snapshot_fd(&ticket.source_parent)? != ticket.source_parent_snapshot
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: ticket.source_parent_path.clone(),
            reason: "legacy root parent identity changed after scan".to_owned(),
        });
    }
    let current = linux_snapshot_at(&ticket.source_parent, ticket.source_name.as_os_str())?;
    let held = linux_snapshot_fd(&ticket.source_directory)?;
    let absolute = fs::symlink_metadata(&ticket.source_path)?;
    if held != ticket.source_snapshot
        || current != ticket.source_snapshot
        || !absolute.is_dir()
        || !same_file_identity(&absolute, &ticket.source_directory.metadata()?)
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: ticket.source_path.clone(),
            reason: "legacy root identity changed after scan".to_owned(),
        });
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_errno(error: rustix::io::Errno) -> LegacyProjectionCleanupError {
    LegacyProjectionCleanupError::Io(error.into())
}

#[cfg(not(target_os = "linux"))]
fn scan_tree(
    root: &Path,
    directory: &Path,
    digest: &mut Sha256,
    counts: &mut TreeCounts,
) -> Result<(), LegacyProjectionCleanupError> {
    let before = fs::symlink_metadata(directory)?;
    if !before.is_dir() {
        return Err(LegacyProjectionCleanupError::UnsupportedEntry(
            directory.to_path_buf(),
        ));
    }
    #[cfg(windows)]
    let directory_guard = windows_open_inventory_directory(directory)?;
    #[cfg(windows)]
    let before_snapshot = windows_snapshot_handle(&directory_guard)?;
    #[cfg(windows)]
    if !windows_metadata_matches_snapshot(&before, &before_snapshot) {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: directory.to_path_buf(),
            reason: "directory changed while opening legacy inventory".to_owned(),
        });
    }
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let relative = normalized_relative_path(root, &path)?;
        if metadata.is_dir() {
            counts.directories = counts.directories.checked_add(1).ok_or_else(|| {
                LegacyProjectionCleanupError::JournalConflict(
                    "legacy directory count overflow".to_owned(),
                )
            })?;
            hash_field(digest, b"directory");
            hash_field(digest, relative.as_bytes());
            scan_tree(root, &path, digest, counts)?;
        } else if metadata.is_file() {
            counts.files = counts.files.checked_add(1).ok_or_else(|| {
                LegacyProjectionCleanupError::JournalConflict(
                    "legacy file count overflow".to_owned(),
                )
            })?;
            counts.bytes = counts.bytes.checked_add(metadata.len()).ok_or_else(|| {
                LegacyProjectionCleanupError::JournalConflict(
                    "legacy byte count overflow".to_owned(),
                )
            })?;
            hash_field(digest, b"file");
            hash_field(digest, relative.as_bytes());
            hash_field(digest, &metadata.len().to_be_bytes());
            hash_regular_file(&path, &metadata, digest)?;
        } else {
            return Err(LegacyProjectionCleanupError::UnsupportedEntry(path));
        }
    }
    let after = fs::symlink_metadata(directory)?;
    #[cfg(windows)]
    let snapshot_matches = windows_metadata_matches_snapshot(&after, &before_snapshot)
        && windows_snapshot_handle(&directory_guard)? == before_snapshot
        && windows_snapshot_path(directory, true)? == before_snapshot;
    #[cfg(not(windows))]
    let snapshot_matches = same_file_snapshot(&before, &after);
    if !after.is_dir() || !snapshot_matches {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: directory.to_path_buf(),
            reason: "directory changed during legacy inventory".to_owned(),
        });
    }
    Ok(())
}

#[cfg(all(not(target_os = "linux"), not(windows)))]
fn hash_regular_file(
    path: &Path,
    before: &std::fs::Metadata,
    digest: &mut Sha256,
) -> Result<(), LegacyProjectionCleanupError> {
    let file = File::open(path)?;
    let opened_before = file.metadata()?;
    let path_after_open = fs::symlink_metadata(path)?;
    if !opened_before.is_file()
        || !path_after_open.is_file()
        || !same_file_snapshot(before, &opened_before)
        || !same_file_snapshot(before, &path_after_open)
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "file changed while opening legacy inventory".to_owned(),
        });
    }
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 64 * 1024];
    let mut read_bytes = 0_u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        read_bytes = read_bytes.checked_add(read as u64).ok_or_else(|| {
            LegacyProjectionCleanupError::JournalConflict(
                "legacy file read count overflow".to_owned(),
            )
        })?;
    }
    let opened_after = reader.get_ref().metadata()?;
    let path_after_read = fs::symlink_metadata(path)?;
    if read_bytes != before.len()
        || !path_after_read.is_file()
        || !same_file_snapshot(before, &opened_after)
        || !same_file_snapshot(before, &path_after_read)
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "file changed during legacy inventory".to_owned(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn hash_regular_file(
    path: &Path,
    before: &std::fs::Metadata,
    digest: &mut Sha256,
) -> Result<(), LegacyProjectionCleanupError> {
    let path_before = windows_snapshot_path(path, false)?;
    if !before.is_file() || !windows_metadata_matches_snapshot(before, &path_before) {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "file changed while opening legacy inventory".to_owned(),
        });
    }
    let file = windows_open_identity_path(path, false)?;
    let opened_before = windows_snapshot_handle(&file)?;
    let path_after_open = windows_snapshot_path(path, false)?;
    if opened_before != path_before || path_after_open != path_before {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "file changed while opening legacy inventory".to_owned(),
        });
    }

    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 64 * 1024];
    let mut read_bytes = 0_u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        read_bytes = read_bytes.checked_add(read as u64).ok_or_else(|| {
            LegacyProjectionCleanupError::JournalConflict(
                "legacy file read count overflow".to_owned(),
            )
        })?;
    }
    let opened_after = windows_snapshot_handle(reader.get_ref())?;
    let path_after_read = windows_snapshot_path(path, false)?;
    if read_bytes != before.len() || opened_after != path_before || path_after_read != path_before {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "file changed during legacy inventory".to_owned(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(all(unix, not(target_os = "linux")))]
fn same_file_snapshot(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    same_file_identity(left, right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowsFileIdentity {
    volume: u64,
    file_id: [u8; 16],
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowsFileSnapshot {
    identity: WindowsFileIdentity,
    attributes: u32,
    link_count: u32,
    length: u64,
    creation_time: u64,
    last_write_time: u64,
}

#[cfg(windows)]
fn windows_open_inventory_directory(path: &Path) -> Result<File, LegacyProjectionCleanupError> {
    use std::os::windows::fs::OpenOptionsExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let snapshot = windows_snapshot_handle(&file)?;
    if snapshot.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "legacy inventory directory is a reparse point".to_owned(),
        });
    }
    if snapshot.attributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "legacy inventory path is not a directory".to_owned(),
        });
    }
    if windows_snapshot_path(path, true)? != snapshot {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "legacy inventory directory changed while opening".to_owned(),
        });
    }
    Ok(file)
}

#[cfg(windows)]
fn windows_open_identity_path(
    path: &Path,
    directory: bool,
) -> Result<File, LegacyProjectionCleanupError> {
    use std::os::windows::fs::OpenOptionsExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let mut options = std::fs::OpenOptions::new();
    let flags = FILE_FLAG_OPEN_REPARSE_POINT
        | if directory {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            0
        };
    options
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(flags);
    Ok(options.open(path)?)
}

#[cfg(windows)]
fn windows_snapshot_path(
    path: &Path,
    directory: bool,
) -> Result<WindowsFileSnapshot, LegacyProjectionCleanupError> {
    windows_snapshot_handle(&windows_open_identity_path(path, directory)?)
}

#[cfg(windows)]
fn windows_snapshot_handle(
    file: &File,
) -> Result<WindowsFileSnapshot, LegacyProjectionCleanupError> {
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ID_INFO, FileIdInfo, GetFileInformationByHandle,
        GetFileInformationByHandleEx,
    };

    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: File owns a valid HANDLE and information is correctly sized
    // writable storage for this synchronous call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) } == 0 {
        return Err(io::Error::last_os_error().into());
    }
    let mut identity = FILE_ID_INFO::default();
    let identity_size = u32::try_from(std::mem::size_of::<FILE_ID_INFO>()).map_err(|_| {
        LegacyProjectionCleanupError::UnsafePath {
            path: PathBuf::from("<handle>"),
            reason: "FILE_ID_INFO size does not fit the Win32 API".to_owned(),
        }
    })?;
    // SAFETY: File owns a valid HANDLE, identity is correctly sized writable
    // storage, and the call is synchronous. Unsupported identity information
    // is returned fail-closed rather than falling back to a truncated file ID.
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut identity as *mut FILE_ID_INFO).cast(),
            identity_size,
        )
    } == 0
    {
        return Err(io::Error::last_os_error().into());
    }

    let length = (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow);
    let creation_time = (u64::from(information.ftCreationTime.dwHighDateTime) << 32)
        | u64::from(information.ftCreationTime.dwLowDateTime);
    let last_write_time = (u64::from(information.ftLastWriteTime.dwHighDateTime) << 32)
        | u64::from(information.ftLastWriteTime.dwLowDateTime);
    Ok(WindowsFileSnapshot {
        identity: WindowsFileIdentity {
            volume: identity.VolumeSerialNumber,
            file_id: identity.FileId.Identifier,
        },
        attributes: information.dwFileAttributes,
        link_count: information.nNumberOfLinks,
        length,
        creation_time,
        last_write_time,
    })
}

#[cfg(windows)]
fn windows_metadata_matches_snapshot(
    metadata: &std::fs::Metadata,
    snapshot: &WindowsFileSnapshot,
) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    metadata.file_attributes() == snapshot.attributes
        && metadata.file_size() == snapshot.length
        && metadata.creation_time() == snapshot.creation_time
        && metadata.last_write_time() == snapshot.last_write_time
}

#[cfg(not(any(unix, windows)))]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    left.len() == right.len()
        && left.created().ok() == right.created().ok()
        && left.modified().ok() == right.modified().ok()
}

#[cfg(not(any(unix, windows)))]
fn same_file_snapshot(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    same_file_identity(left, right)
        && left.modified().ok() == right.modified().ok()
        && left.created().ok() == right.created().ok()
}

#[cfg(not(target_os = "linux"))]
fn normalized_relative_path(
    root: &Path,
    path: &Path,
) -> Result<String, LegacyProjectionCleanupError> {
    let relative =
        path.strip_prefix(root)
            .map_err(|_| LegacyProjectionCleanupError::UnsafePath {
                path: path.to_path_buf(),
                reason: "legacy entry escaped its exact root".to_owned(),
            })?;
    let mut normalized = String::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: path.to_path_buf(),
                reason: "legacy entry has a non-normal path component".to_owned(),
            });
        };
        let component =
            component
                .to_str()
                .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                    path: path.to_path_buf(),
                    reason: "legacy entry path is not UTF-8".to_owned(),
                })?;
        if !normalized.is_empty() {
            normalized.push('/');
        }
        normalized.push_str(component);
    }
    Ok(normalized)
}

fn inventory_digest(
    database_path: &Path,
    database_instance_id: &str,
    roots: &[LegacyProjectionRootInventory],
) -> Result<String, LegacyProjectionCleanupError> {
    let mut digest = Sha256::new();
    hash_field(&mut digest, b"kanban-legacy-projection-inventory-v1");
    hash_field(&mut digest, database_instance_id.as_bytes());
    hash_field(&mut digest, require_utf8_path(database_path)?.as_bytes());
    for root in roots {
        hash_field(&mut digest, root.kind.backup_name().as_bytes());
        hash_field(&mut digest, root.relative_path.as_bytes());
        hash_field(
            &mut digest,
            if root.present { b"present" } else { b"absent" },
        );
        hash_field(&mut digest, &root.file_count.to_be_bytes());
        hash_field(&mut digest, &root.directory_count.to_be_bytes());
        hash_field(&mut digest, &root.byte_count.to_be_bytes());
        hash_field(&mut digest, root.digest.as_bytes());
    }
    Ok(finish_digest(digest))
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn finish_digest(digest: Sha256) -> String {
    format!("sha256:{:x}", digest.finalize())
}

fn require_utf8_path(path: &Path) -> Result<&str, LegacyProjectionCleanupError> {
    path.to_str()
        .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
            path: path.to_path_buf(),
            reason: "path is not UTF-8".to_owned(),
        })
}

struct ApplyHooks<BeforeInitialPublish, BeforeMove, AfterMove> {
    before_initial_publish: BeforeInitialPublish,
    before_move: BeforeMove,
    after_move: AfterMove,
}

fn apply_legacy_projection_cleanup_inner<
    BeforeInitialPublish,
    BeforeMove,
    AfterMove,
    FilesystemId,
>(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    expected_inventory_digest: &str,
    backup_dir: &Path,
    resume_decision: Option<bool>,
    mut hooks: ApplyHooks<BeforeInitialPublish, BeforeMove, AfterMove>,
    mut filesystem_id: FilesystemId,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError>
where
    BeforeInitialPublish: for<'path> FnMut(&'path Path) -> Result<(), LegacyProjectionCleanupError>,
    BeforeMove: for<'source, 'destination> FnMut(
        LegacyProjectionRootKind,
        &'source Path,
        &'destination Path,
    ) -> Result<(), LegacyProjectionCleanupError>,
    AfterMove: FnMut(LegacyProjectionRootKind) -> Result<(), LegacyProjectionCleanupError>,
    FilesystemId: for<'path> FnMut(&'path Path) -> Result<u64, LegacyProjectionCleanupError>,
{
    guard.validate(db_path)?;
    validate_database_instance_id(database_instance_id)?;
    let database_path = canonical_database_path(db_path)?;
    let backup_exists = match fs::symlink_metadata(backup_dir) {
        Ok(_) => true,
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    match (resume_decision, backup_exists) {
        (Some(false), true) => {
            return Err(LegacyProjectionCleanupError::ResumeDecision(format!(
                "legacy cleanup backup already exists at {}; use --resume only after verifying its binding",
                backup_dir.display()
            )));
        }
        (Some(true), false) => {
            return Err(LegacyProjectionCleanupError::ResumeDecision(format!(
                "legacy cleanup has no backup state to resume at {}",
                backup_dir.display()
            )));
        }
        _ => {}
    }
    let backup_dir = validate_backup_path(&database_path, backup_dir, backup_exists)?;
    ensure_atomic_cleanup_supported()?;
    let (mut journal, resumed) = if backup_exists {
        let journal = load_and_validate_journal(&database_path, database_instance_id, &backup_dir)?;
        if journal.inventory_digest != expected_inventory_digest {
            return Err(LegacyProjectionCleanupError::DigestMismatch {
                expected: expected_inventory_digest.to_owned(),
                actual: journal.inventory_digest,
            });
        }
        match journal.phase {
            CleanupPhase::Applying | CleanupPhase::Completed => {}
            CleanupPhase::Restoring | CleanupPhase::Restored => {
                return Err(LegacyProjectionCleanupError::JournalConflict(
                    "cannot apply cleanup after restore started".to_owned(),
                ));
            }
        }
        (journal, true)
    } else {
        let inventory = inventory_for_canonical_database(&database_path, database_instance_id)?;
        if inventory.inventory_digest != expected_inventory_digest {
            return Err(LegacyProjectionCleanupError::DigestMismatch {
                expected: expected_inventory_digest.to_owned(),
                actual: inventory.inventory_digest,
            });
        }
        preflight_backup_filesystems(&inventory, &backup_dir, &mut filesystem_id)?;
        let journal = CleanupJournal {
            format_version: CLEANUP_FORMAT_VERSION,
            database_instance_id: database_instance_id.to_owned(),
            database_path: database_path.clone(),
            inventory_digest: inventory.inventory_digest,
            phase: CleanupPhase::Applying,
            roots: inventory
                .roots
                .into_iter()
                .map(|inventory| CleanupJournalRoot {
                    state: if inventory.present {
                        CleanupRootState::Pending
                    } else {
                        CleanupRootState::Absent
                    },
                    inventory,
                })
                .collect(),
        };
        publish_initial_backup_layout(&backup_dir, &journal, &mut hooks.before_initial_publish)?;
        (journal, false)
    };

    if journal.phase == CleanupPhase::Completed {
        validate_completed_backup(&journal, &backup_dir)?;
        let manifest = load_and_validate_manifest(&journal, &backup_dir)?;
        return Ok(LegacyProjectionCleanupOutcome { manifest, resumed });
    }

    for index in 0..journal.roots.len() {
        guard.validate(&database_path)?;
        apply_one_root(
            guard,
            &database_path,
            &mut journal,
            index,
            &backup_dir,
            &mut hooks.before_move,
            &mut hooks.after_move,
        )?;
    }
    guard.validate(&database_path)?;
    let manifest = manifest_from_journal(&journal);
    write_or_validate_manifest(&backup_dir, &manifest)?;
    guard.validate(&database_path)?;
    journal.phase = CleanupPhase::Completed;
    write_journal(&backup_dir, &journal)?;
    validate_completed_backup(&journal, &backup_dir)?;
    Ok(LegacyProjectionCleanupOutcome { manifest, resumed })
}

fn apply_one_root(
    guard: &LegacyProjectionCleanupGuard,
    database_path: &Path,
    journal: &mut CleanupJournal,
    index: usize,
    backup_dir: &Path,
    before_move: &mut impl for<'source, 'destination> FnMut(
        LegacyProjectionRootKind,
        &'source Path,
        &'destination Path,
    )
        -> Result<(), LegacyProjectionCleanupError>,
    after_move: &mut impl FnMut(LegacyProjectionRootKind) -> Result<(), LegacyProjectionCleanupError>,
) -> Result<(), LegacyProjectionCleanupError> {
    let inventory = journal.roots[index].inventory.clone();
    let state = journal.roots[index].state;
    let source = &inventory.absolute_path;
    let destination = backup_root_path(backup_dir, inventory.kind);
    if !inventory.present {
        if state != CleanupRootState::Absent
            || path_entry_exists(source)?
            || path_entry_exists(&destination)?
        {
            return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                "absent root changed before apply: {}",
                source.display()
            )));
        }
        return Ok(());
    }

    match state {
        CleanupRootState::Pending => {
            let source_exists = path_entry_exists(source)?;
            let destination_exists = path_entry_exists(&destination)?;
            match (source_exists, destination_exists) {
                (true, false) => {
                    let prepared = prepare_inventory_root_move(source, &destination, &inventory)?;
                    before_move(inventory.kind, source, &destination)?;
                    commit_inventory_root_move(prepared, &destination, guard, database_path)?;
                    after_move(inventory.kind)?;
                    require_root_matches(&destination, &inventory)?;
                    journal.roots[index].state = CleanupRootState::Moved;
                    write_journal(backup_dir, journal)?;
                }
                (false, true) => {
                    require_root_matches(&destination, &inventory)?;
                    journal.roots[index].state = CleanupRootState::Moved;
                    write_journal(backup_dir, journal)?;
                }
                (true, true) => {
                    return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                        "legacy root exists in source and backup: {}",
                        source.display()
                    )));
                }
                (false, false) => {
                    return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                        "legacy root is missing from source and backup: {}",
                        source.display()
                    )));
                }
            }
        }
        CleanupRootState::Moved => {
            if path_entry_exists(source)? || !path_entry_exists(&destination)? {
                return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                    "moved root state does not match filesystem: {}",
                    source.display()
                )));
            }
            require_root_matches(&destination, &inventory)?;
        }
        CleanupRootState::Absent | CleanupRootState::Restored => {
            return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                "present root has invalid apply state: {:?}",
                state
            )));
        }
    }
    Ok(())
}

fn restore_legacy_projection_backup_inner<FilesystemId>(
    guard: &LegacyProjectionCleanupGuard,
    db_path: &Path,
    database_instance_id: &str,
    backup_dir: &Path,
    mut before_move: impl for<'source, 'destination> FnMut(
        LegacyProjectionRootKind,
        &'source Path,
        &'destination Path,
    )
        -> Result<(), LegacyProjectionCleanupError>,
    mut after_move: impl FnMut(LegacyProjectionRootKind) -> Result<(), LegacyProjectionCleanupError>,
    mut filesystem_id: FilesystemId,
) -> Result<LegacyProjectionCleanupOutcome, LegacyProjectionCleanupError>
where
    FilesystemId: for<'path> FnMut(&'path Path) -> Result<u64, LegacyProjectionCleanupError>,
{
    guard.validate(db_path)?;
    validate_database_instance_id(database_instance_id)?;
    let database_path = canonical_database_path(db_path)?;
    let backup_dir = validate_backup_path(&database_path, backup_dir, true)?;
    ensure_atomic_cleanup_supported()?;
    let mut journal =
        load_and_validate_journal_read_only(&database_path, database_instance_id, &backup_dir)?;
    if journal.phase == CleanupPhase::Applying {
        return Err(LegacyProjectionCleanupError::JournalConflict(
            "cannot restore an incomplete cleanup backup".to_owned(),
        ));
    }
    let manifest = load_and_validate_manifest(&journal, &backup_dir)?;
    let resumed = matches!(
        journal.phase,
        CleanupPhase::Restoring | CleanupPhase::Restored
    );
    match journal.phase {
        CleanupPhase::Completed => {
            validate_restore_preflight(&journal, &backup_dir)?;
            preflight_restore_filesystems(guard, &journal, &backup_dir, &mut filesystem_id)?;
            journal.phase = CleanupPhase::Restoring;
            write_journal(&backup_dir, &journal)?;
        }
        CleanupPhase::Restoring => {
            validate_restore_preflight(&journal, &backup_dir)?;
            preflight_restore_filesystems(guard, &journal, &backup_dir, &mut filesystem_id)?;
        }
        CleanupPhase::Restored => {
            validate_restored_state(&journal, &backup_dir)?;
            return Ok(LegacyProjectionCleanupOutcome { manifest, resumed });
        }
        CleanupPhase::Applying => unreachable!("applying phase rejected before manifest load"),
    }

    for index in 0..journal.roots.len() {
        guard.validate(&database_path)?;
        restore_one_root(
            guard,
            &database_path,
            &mut journal,
            index,
            &backup_dir,
            &mut before_move,
            &mut after_move,
        )?;
    }
    guard.validate(&database_path)?;
    journal.phase = CleanupPhase::Restored;
    write_journal(&backup_dir, &journal)?;
    validate_restored_state(&journal, &backup_dir)?;
    Ok(LegacyProjectionCleanupOutcome { manifest, resumed })
}

fn restore_one_root(
    guard: &LegacyProjectionCleanupGuard,
    database_path: &Path,
    journal: &mut CleanupJournal,
    index: usize,
    backup_dir: &Path,
    before_move: &mut impl for<'source, 'destination> FnMut(
        LegacyProjectionRootKind,
        &'source Path,
        &'destination Path,
    )
        -> Result<(), LegacyProjectionCleanupError>,
    after_move: &mut impl FnMut(LegacyProjectionRootKind) -> Result<(), LegacyProjectionCleanupError>,
) -> Result<(), LegacyProjectionCleanupError> {
    let inventory = journal.roots[index].inventory.clone();
    let state = journal.roots[index].state;
    let source = &inventory.absolute_path;
    let backup = backup_root_path(backup_dir, inventory.kind);
    if !inventory.present {
        if state != CleanupRootState::Absent
            || path_entry_exists(source)?
            || path_entry_exists(&backup)?
        {
            return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                "absent root changed before restore: {}",
                source.display()
            )));
        }
        return Ok(());
    }
    match state {
        CleanupRootState::Moved => {
            let source_exists = path_entry_exists(source)?;
            let backup_exists = path_entry_exists(&backup)?;
            match (source_exists, backup_exists) {
                (false, true) => {
                    let prepared = prepare_inventory_root_move(&backup, source, &inventory)?;
                    before_move(inventory.kind, &backup, source)?;
                    commit_inventory_root_move(prepared, source, guard, database_path)?;
                    after_move(inventory.kind)?;
                    require_root_matches(source, &inventory)?;
                    journal.roots[index].state = CleanupRootState::Restored;
                    write_journal(backup_dir, journal)?;
                }
                (true, false) => {
                    require_root_matches(source, &inventory)?;
                    journal.roots[index].state = CleanupRootState::Restored;
                    write_journal(backup_dir, journal)?;
                }
                (true, true) => {
                    return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                        "restore root exists in source and backup: {}",
                        source.display()
                    )));
                }
                (false, false) => {
                    return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                        "restore root is missing from source and backup: {}",
                        source.display()
                    )));
                }
            }
        }
        CleanupRootState::Restored => {
            if !path_entry_exists(source)? || path_entry_exists(&backup)? {
                return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                    "restored root state does not match filesystem: {}",
                    source.display()
                )));
            }
            require_root_matches(source, &inventory)?;
        }
        CleanupRootState::Absent | CleanupRootState::Pending => {
            return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                "present root has invalid restore state: {:?}",
                state
            )));
        }
    }
    Ok(())
}

fn validate_backup_path(
    database_path: &Path,
    backup_dir: &Path,
    expect_existing: bool,
) -> Result<PathBuf, LegacyProjectionCleanupError> {
    if !backup_dir.is_absolute() || !has_only_normal_absolute_components(backup_dir) {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: backup_dir.to_path_buf(),
            reason: "backup directory must be an absolute path without . or ..".to_owned(),
        });
    }
    require_utf8_path(backup_dir)?;
    let parent = backup_dir
        .parent()
        .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
            path: backup_dir.to_path_buf(),
            reason: "backup directory has no parent".to_owned(),
        })?;
    validate_real_directory_chain(parent)?;
    let canonical_parent = fs::canonicalize(parent)?;
    let backup_name =
        backup_dir
            .file_name()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: backup_dir.to_path_buf(),
                reason: "backup directory has no final component".to_owned(),
            })?;
    let candidate = canonical_parent.join(backup_name);
    let database_parent =
        database_path
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: database_path.to_path_buf(),
                reason: "canonical database path has no parent".to_owned(),
            })?;
    if paths_overlap(&candidate, database_parent) {
        return Err(LegacyProjectionCleanupError::Overlap(
            backup_dir.to_path_buf(),
        ));
    }
    let exists = match fs::symlink_metadata(&candidate) {
        Ok(metadata) if expect_existing && metadata.is_dir() => true,
        Ok(_) => {
            return Err(LegacyProjectionCleanupError::BackupConflict(
                backup_dir.to_path_buf(),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound && !expect_existing => false,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(LegacyProjectionCleanupError::BackupConflict(
                backup_dir.to_path_buf(),
            ));
        }
        Err(error) => return Err(error.into()),
    };
    validate_physical_backup_overlap(database_parent, &candidate, exists, backup_dir)?;
    Ok(candidate)
}

fn has_only_normal_absolute_components(path: &Path) -> bool {
    path.components().all(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::Normal(_)
        )
    })
}

#[cfg(not(windows))]
fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

#[cfg(windows)]
fn paths_overlap(left: &Path, right: &Path) -> bool {
    fn key(path: &Path) -> Vec<String> {
        path.components()
            .filter_map(|component| match component {
                Component::Prefix(prefix) => {
                    let normalized = prefix
                        .as_os_str()
                        .to_string_lossy()
                        .trim_start_matches(r"\\?\")
                        .replace('/', "\\")
                        .to_lowercase();
                    Some(normalized)
                }
                Component::RootDir => Some("\\".to_owned()),
                Component::Normal(component) => Some(component.to_string_lossy().to_lowercase()),
                Component::CurDir | Component::ParentDir => None,
            })
            .collect()
    }
    let left = key(left);
    let right = key(right);
    left.starts_with(&right) || right.starts_with(&left)
}

fn validate_real_directory_chain(path: &Path) -> Result<(), LegacyProjectionCleanupError> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = fs::symlink_metadata(&current)?;
        if !metadata.is_dir() {
            return Err(LegacyProjectionCleanupError::BackupConflict(current));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_physical_backup_overlap(
    database_parent: &Path,
    candidate: &Path,
    candidate_exists: bool,
    original: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    let anchor = if candidate_exists {
        candidate
    } else {
        candidate
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: candidate.to_path_buf(),
                reason: "backup candidate has no parent".to_owned(),
            })?
    };
    let anchor_file = linux_open_stable_directory(anchor)?;
    let anchor_snapshot = linux_snapshot_fd(&anchor_file)?;
    let needle = anchor_snapshot.identity;
    let database = linux_open_stable_directory(database_parent)?;
    let database_snapshot = linux_snapshot_fd(&database)?;
    let mut visited = HashSet::from([database_snapshot.identity]);
    if database_snapshot.identity == needle
        || linux_directory_contains_identity(
            &database,
            database_parent,
            database_snapshot.mount_id,
            needle,
            &mut visited,
        )?
    {
        return Err(LegacyProjectionCleanupError::Overlap(
            original.to_path_buf(),
        ));
    }
    if candidate_exists {
        let mut candidate_visited = HashSet::from([anchor_snapshot.identity]);
        if anchor_snapshot.identity == database_snapshot.identity
            || linux_directory_contains_identity(
                &anchor_file,
                candidate,
                anchor_snapshot.mount_id,
                database_snapshot.identity,
                &mut candidate_visited,
            )?
        {
            return Err(LegacyProjectionCleanupError::Overlap(
                original.to_path_buf(),
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_directory_contains_identity(
    directory: &File,
    display_path: &Path,
    root_mount_id: u64,
    needle: LinuxFileIdentity,
    visited: &mut HashSet<LinuxFileIdentity>,
) -> Result<bool, LegacyProjectionCleanupError> {
    use rustix::fs::{FileType, Mode, OFlags, RawDir, ResolveFlags, openat2};

    let mut buffer = [MaybeUninit::uninit(); 16 * 1024];
    let mut iterator = RawDir::new(directory, &mut buffer);
    let mut names = Vec::<Vec<u8>>::new();
    while let Some(entry) = iterator.next() {
        let entry = entry.map_err(linux_errno)?;
        let name = entry.file_name().to_bytes();
        if !matches!(name, b"." | b"..") {
            names.push(name.to_vec());
        }
    }
    names.sort();
    for name in names {
        let name = OsString::from_vec(name);
        let opened = match openat2(
            directory,
            name.as_os_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        ) {
            Ok(opened) => File::from(opened),
            Err(rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP) => {
                continue;
            }
            Err(rustix::io::Errno::XDEV) => {
                return Err(LegacyProjectionCleanupError::UnsafePath {
                    path: display_path.join(&name),
                    reason: "database directory contains a nested mount".to_owned(),
                });
            }
            Err(error) => return Err(linux_errno(error)),
        };
        let snapshot = linux_snapshot_fd(&opened)?;
        if FileType::from_raw_mode(snapshot.mode) != FileType::Directory {
            continue;
        }
        if snapshot.mount_id != root_mount_id {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: display_path.join(&name),
                reason: "database directory contains a nested mount".to_owned(),
            });
        }
        if snapshot.identity == needle {
            return Ok(true);
        }
        if !visited.insert(snapshot.identity) {
            return Err(LegacyProjectionCleanupError::UnsafePath {
                path: display_path.join(&name),
                reason: "database directory contains a repeated inode identity".to_owned(),
            });
        }
        if linux_directory_contains_identity(
            &opened,
            &display_path.join(&name),
            root_mount_id,
            needle,
            visited,
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(not(target_os = "linux"))]
fn validate_physical_backup_overlap(
    _database_parent: &Path,
    _candidate: &Path,
    _candidate_exists: bool,
    _original: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    Ok(())
}

fn preflight_backup_filesystems(
    inventory: &LegacyProjectionCleanupInventory,
    backup_dir: &Path,
    filesystem_id: &mut impl for<'path> FnMut(&'path Path) -> Result<u64, LegacyProjectionCleanupError>,
) -> Result<(), LegacyProjectionCleanupError> {
    let backup_parent =
        backup_dir
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: backup_dir.to_path_buf(),
                reason: "backup directory has no parent".to_owned(),
            })?;
    let backup_id = filesystem_id(backup_parent)?;
    let database_parent = inventory.database_path.parent().ok_or_else(|| {
        LegacyProjectionCleanupError::UnsafePath {
            path: inventory.database_path.clone(),
            reason: "canonical database path has no parent".to_owned(),
        }
    })?;
    require_same_filesystem_id(
        filesystem_id(database_parent)?,
        backup_id,
        database_parent,
        backup_parent,
    )?;
    for root in inventory.roots.iter().filter(|root| root.present) {
        require_same_filesystem_id(
            filesystem_id(&root.absolute_path)?,
            backup_id,
            &root.absolute_path,
            backup_parent,
        )?;
    }
    Ok(())
}

fn preflight_restore_filesystems(
    _guard: &LegacyProjectionCleanupGuard,
    journal: &CleanupJournal,
    backup_dir: &Path,
    filesystem_id: &mut impl for<'path> FnMut(&'path Path) -> Result<u64, LegacyProjectionCleanupError>,
) -> Result<(), LegacyProjectionCleanupError> {
    let backup_parent =
        backup_dir
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: backup_dir.to_path_buf(),
                reason: "backup directory has no parent".to_owned(),
            })?;
    let database_parent =
        journal
            .database_path
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: journal.database_path.clone(),
                reason: "canonical database path has no parent".to_owned(),
            })?;
    let backup_id = filesystem_id(backup_parent)?;
    require_same_filesystem_id(
        filesystem_id(database_parent)?,
        backup_id,
        database_parent,
        backup_parent,
    )?;
    for root in journal.roots.iter().filter(|root| root.inventory.present) {
        let backup = backup_root_path(backup_dir, root.inventory.kind);
        let source = &root.inventory.absolute_path;
        let source_parent =
            source
                .parent()
                .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                    path: source.to_path_buf(),
                    reason: "restore destination has no parent".to_owned(),
                })?;
        #[cfg(target_os = "linux")]
        {
            let managed_parent = linux_open_managed_root_parent(
                _guard,
                &journal.database_path,
                root.inventory.kind,
            )?;
            let managed_snapshot = linux_snapshot_fd(&managed_parent)?;
            if managed_snapshot.mount_id != _guard.database_parent_mount_id {
                return Err(LegacyProjectionCleanupError::CrossFilesystem {
                    source_path: source_parent.to_path_buf(),
                    backup: backup_parent.to_path_buf(),
                });
            }
        }
        require_same_filesystem_id(
            filesystem_id(source_parent)?,
            backup_id,
            source_parent,
            backup_parent,
        )?;
        let resident = if path_entry_exists(&backup)? {
            backup.as_path()
        } else {
            source.as_path()
        };
        require_same_filesystem_id(filesystem_id(resident)?, backup_id, resident, backup_parent)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn ensure_atomic_cleanup_supported() -> Result<(), LegacyProjectionCleanupError> {
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn ensure_atomic_cleanup_supported() -> Result<(), LegacyProjectionCleanupError> {
    Err(LegacyProjectionCleanupError::UnsafePath {
        path: PathBuf::from("<platform>"),
        reason: "legacy projection cleanup mutation requires Linux fd-bound renameat2 support"
            .to_owned(),
    })
}

#[cfg(target_os = "linux")]
fn ensure_path_same_filesystem(
    source: &Path,
    destination: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    let destination_parent =
        destination
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: destination.to_path_buf(),
                reason: "move destination has no parent".to_owned(),
            })?;
    require_same_filesystem_id(
        filesystem_id(source)?,
        filesystem_id(destination_parent)?,
        source,
        destination_parent,
    )
}

#[cfg(target_os = "linux")]
fn filesystem_id(path: &Path) -> Result<u64, LegacyProjectionCleanupError> {
    let directory = linux_open_stable_directory(path)?;
    Ok(linux_snapshot_fd(&directory)?.mount_id)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn filesystem_id(path: &Path) -> Result<u64, LegacyProjectionCleanupError> {
    use std::os::unix::fs::MetadataExt as _;

    Ok(fs::symlink_metadata(path)?.dev())
}

#[cfg(windows)]
fn filesystem_id(path: &Path) -> Result<u64, LegacyProjectionCleanupError> {
    let metadata = fs::symlink_metadata(path)?;
    windows_snapshot_path(path, metadata.is_dir()).map(|snapshot| snapshot.identity.volume)
}

#[cfg(not(any(unix, windows)))]
fn filesystem_id(path: &Path) -> Result<u64, LegacyProjectionCleanupError> {
    Err(LegacyProjectionCleanupError::UnsafePath {
        path: path.to_path_buf(),
        reason: "same-filesystem cleanup is unsupported on this platform".to_owned(),
    })
}

fn publish_initial_backup_layout(
    backup_dir: &Path,
    journal: &CleanupJournal,
    before_publish: &mut impl FnMut(&Path) -> Result<(), LegacyProjectionCleanupError>,
) -> Result<(), LegacyProjectionCleanupError> {
    let parent = backup_dir
        .parent()
        .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
            path: backup_dir.to_path_buf(),
            reason: "backup directory has no parent".to_owned(),
        })?;
    let staging = super::unique_sibling_path(backup_dir, "cleanup-stage")?;
    fs::create_dir(&staging)?;
    let roots = staging.join(ROOTS_DIR);
    fs::create_dir(&roots)?;
    write_journal(&staging, journal)?;
    super::durable_sync_directory(&roots)?;
    super::durable_sync_directory_tree(&staging)?;
    before_publish(&staging)?;
    atomic_move_directory_no_replace(&staging, backup_dir)?;
    super::durable_sync_directory(parent)?;
    Ok(())
}

#[cfg(target_os = "linux")]
struct PreparedInventoryRootMove {
    kind: LegacyProjectionRootKind,
    ticket: RootScanTicket,
}

#[cfg(not(target_os = "linux"))]
struct PreparedInventoryRootMove;

#[cfg(target_os = "linux")]
fn prepare_inventory_root_move(
    source: &Path,
    destination: &Path,
    expected: &LegacyProjectionRootInventory,
) -> Result<PreparedInventoryRootMove, LegacyProjectionCleanupError> {
    let (actual, ticket) =
        linux_scan_present_root_with_ticket(expected.kind, source, &expected.absolute_path, true)?;
    if actual != *expected {
        return Err(LegacyProjectionCleanupError::JournalConflict(format!(
            "legacy root content does not match inventory: {}",
            source.display()
        )));
    }
    ensure_path_same_filesystem(source, destination)?;
    Ok(PreparedInventoryRootMove {
        kind: expected.kind,
        ticket,
    })
}

#[cfg(not(target_os = "linux"))]
fn prepare_inventory_root_move(
    source: &Path,
    destination: &Path,
    _expected: &LegacyProjectionRootInventory,
) -> Result<PreparedInventoryRootMove, LegacyProjectionCleanupError> {
    Err(LegacyProjectionCleanupError::UnsafePath {
        path: source.to_path_buf(),
        reason: format!(
            "fd-bound directory move is unsupported for {}",
            destination.display()
        ),
    })
}

#[cfg(target_os = "linux")]
fn commit_inventory_root_move(
    prepared: PreparedInventoryRootMove,
    destination: &Path,
    guard: &LegacyProjectionCleanupGuard,
    database_path: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    guard.validate(database_path)?;
    let database_parent =
        database_path
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: database_path.to_path_buf(),
                reason: "canonical database path has no parent".to_owned(),
            })?;
    let managed_destination = database_parent.join(prepared.kind.relative_path()) == destination;
    let destination_parent =
        destination
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: destination.to_path_buf(),
                reason: "move destination has no parent".to_owned(),
            })?;
    let destination_parent_file = if managed_destination {
        linux_open_managed_root_parent(guard, database_path, prepared.kind)?
    } else {
        linux_open_stable_directory(destination_parent)?
    };
    let destination_parent_snapshot = linux_snapshot_fd(&destination_parent_file)?;
    linux_atomic_move_ticket_to_parent_no_replace(
        &prepared.ticket,
        destination,
        &destination_parent_file,
        noop_cleanup_atomic_hook,
        || {
            guard.validate(database_path)?;
            if managed_destination {
                let reopened = linux_open_managed_root_parent(guard, database_path, prepared.kind)?;
                let current = linux_snapshot_fd(&reopened)?;
                if current.identity != destination_parent_snapshot.identity
                    || current.mount_id != destination_parent_snapshot.mount_id
                {
                    return Err(LegacyProjectionCleanupError::UnsafePath {
                        path: destination_parent.to_path_buf(),
                        reason: "managed restore parent changed during atomic move".to_owned(),
                    });
                }
            }
            Ok(())
        },
    )
}

#[cfg(not(target_os = "linux"))]
fn commit_inventory_root_move(
    _prepared: PreparedInventoryRootMove,
    destination: &Path,
    _guard: &LegacyProjectionCleanupGuard,
    _database_path: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    Err(LegacyProjectionCleanupError::UnsafePath {
        path: destination.to_path_buf(),
        reason: "fd-bound directory move is unsupported on this platform".to_owned(),
    })
}

#[cfg(target_os = "linux")]
fn atomic_move_directory_no_replace(
    source: &Path,
    destination: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    super::durable_sync_directory_tree(source)?;
    let ticket = linux_open_root_scan_ticket(source)?;
    linux_atomic_move_ticket_no_replace(&ticket, destination)
}

#[cfg(target_os = "linux")]
fn linux_atomic_move_ticket_no_replace(
    ticket: &RootScanTicket,
    destination: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    let destination_parent =
        destination
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: destination.to_path_buf(),
                reason: "move destination has no parent".to_owned(),
            })?;
    let destination_parent_file = linux_open_stable_directory(destination_parent)?;
    linux_atomic_move_ticket_to_parent_no_replace(
        ticket,
        destination,
        &destination_parent_file,
        noop_cleanup_atomic_hook,
        noop_cleanup_atomic_hook,
    )
}

#[cfg(target_os = "linux")]
fn linux_atomic_move_ticket_to_parent_no_replace<BeforeRename, PostRename>(
    ticket: &RootScanTicket,
    destination: &Path,
    destination_parent_file: &File,
    before_rename: BeforeRename,
    post_rename: PostRename,
) -> Result<(), LegacyProjectionCleanupError>
where
    BeforeRename: FnOnce() -> Result<(), LegacyProjectionCleanupError>,
    PostRename: FnOnce() -> Result<(), LegacyProjectionCleanupError>,
{
    use rustix::fs::{AtFlags, RenameFlags, renameat_with, statat};

    let destination_parent =
        destination
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: destination.to_path_buf(),
                reason: "move destination has no parent".to_owned(),
            })?;
    let destination_name =
        destination
            .file_name()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: destination.to_path_buf(),
                reason: "move destination has no final component".to_owned(),
            })?;
    let destination_parent_snapshot = linux_snapshot_fd(destination_parent_file)?;
    linux_validate_root_ticket(ticket)?;
    require_same_filesystem_id(
        ticket.source_snapshot.mount_id,
        destination_parent_snapshot.mount_id,
        &ticket.source_path,
        destination_parent,
    )?;
    match statat(
        destination_parent_file,
        destination_name,
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Err(rustix::io::Errno::NOENT) => {}
        Ok(_) => {
            return Err(LegacyProjectionCleanupError::BackupConflict(
                destination.to_path_buf(),
            ));
        }
        Err(error) => return Err(linux_errno(error)),
    }
    let destination_parent_path = fs::symlink_metadata(destination_parent)?;
    if !destination_parent_path.is_dir()
        || !same_file_identity(
            &destination_parent_path,
            &destination_parent_file.metadata()?,
        )
        || linux_snapshot_fd(destination_parent_file)? != destination_parent_snapshot
    {
        return Err(LegacyProjectionCleanupError::UnsafePath {
            path: destination_parent.to_path_buf(),
            reason: "move destination parent changed before rename".to_owned(),
        });
    }
    linux_validate_root_ticket(ticket)?;
    before_rename()?;
    if let Err(error) = renameat_with(
        &ticket.source_parent,
        ticket.source_name.as_os_str(),
        destination_parent_file,
        destination_name,
        RenameFlags::NOREPLACE,
    ) {
        if matches!(
            error,
            rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY
        ) {
            return Err(LegacyProjectionCleanupError::BackupConflict(
                destination.to_path_buf(),
            ));
        }
        return Err(linux_errno(error));
    }

    let held_after = linux_snapshot_fd(&ticket.source_directory)?;
    let moved_snapshot = linux_snapshot_at(destination_parent_file, destination_name)?;
    let held_identity = ticket.source_directory.metadata()?;
    let destination_identity_matches = fs::symlink_metadata(destination)
        .is_ok_and(|metadata| metadata.is_dir() && same_file_identity(&held_identity, &metadata));
    let source_is_gone = matches!(
        statat(
            &ticket.source_parent,
            ticket.source_name.as_os_str(),
            AtFlags::SYMLINK_NOFOLLOW
        ),
        Err(rustix::io::Errno::NOENT)
    );
    let postcondition = if moved_snapshot != held_after
        || !linux_snapshot_matches_after_rename(ticket.source_snapshot, moved_snapshot)
        || !destination_identity_matches
        || !source_is_gone
        || linux_validate_directory_handle_path(&ticket.source_parent_path, &ticket.source_parent)
            .is_err()
        || linux_validate_directory_handle_path(destination_parent, destination_parent_file)
            .is_err()
    {
        Err(LegacyProjectionCleanupError::JournalConflict(format!(
            "atomic move identity check failed: {} -> {}",
            ticket.source_path.display(),
            destination.display()
        )))
    } else {
        post_rename()
    };
    if let Err(error) = postcondition {
        if let Err(rollback) = linux_rollback_ticket_move(
            ticket,
            destination,
            destination_parent_file,
            destination_name,
            moved_snapshot,
        ) {
            return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                "atomic move postcondition failed ({error}); held-fd rollback failed ({rollback})"
            )));
        }
        return Err(error);
    }
    ticket.source_parent.sync_all()?;
    destination_parent_file.sync_all()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_rollback_ticket_move(
    ticket: &RootScanTicket,
    destination: &Path,
    destination_parent_file: &File,
    destination_name: &OsStr,
    expected_destination_snapshot: LinuxFileSnapshot,
) -> Result<(), LegacyProjectionCleanupError> {
    use rustix::fs::{AtFlags, RenameFlags, renameat_with, statat};

    let destination_snapshot = linux_snapshot_at(destination_parent_file, destination_name)?;
    if destination_snapshot != expected_destination_snapshot {
        return Err(LegacyProjectionCleanupError::JournalConflict(
            "rollback destination is not the held source directory".to_owned(),
        ));
    }
    match statat(
        &ticket.source_parent,
        ticket.source_name.as_os_str(),
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Err(rustix::io::Errno::NOENT) => {}
        Ok(_) => {
            return Err(LegacyProjectionCleanupError::BackupConflict(
                ticket.source_path.clone(),
            ));
        }
        Err(error) => return Err(linux_errno(error)),
    }
    renameat_with(
        destination_parent_file,
        destination_name,
        &ticket.source_parent,
        ticket.source_name.as_os_str(),
        RenameFlags::NOREPLACE,
    )
    .map_err(linux_errno)?;
    let restored_snapshot =
        linux_snapshot_at(&ticket.source_parent, ticket.source_name.as_os_str())?;
    let destination_is_gone = matches!(
        statat(
            destination_parent_file,
            destination_name,
            AtFlags::SYMLINK_NOFOLLOW
        ),
        Err(rustix::io::Errno::NOENT)
    );
    if !linux_snapshot_matches_after_rename(expected_destination_snapshot, restored_snapshot)
        || !destination_is_gone
        || linux_validate_directory_handle_path(&ticket.source_parent_path, &ticket.source_parent)
            .is_err()
    {
        return Err(LegacyProjectionCleanupError::JournalConflict(format!(
            "held-fd rollback identity check failed: {} <- {}",
            ticket.source_path.display(),
            destination.display()
        )));
    }
    ticket.source_parent.sync_all()?;
    destination_parent_file.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn atomic_move_directory_no_replace(
    source: &Path,
    destination: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    let source_file = windows_open_identity_path(source, true)?;
    let source_snapshot = windows_snapshot_handle(&source_file)?;
    super::durable_move_entry_no_replace_platform(source, destination)?;
    let destination_metadata = fs::symlink_metadata(destination)?;
    let destination_snapshot = windows_snapshot_path(destination, true)?;
    let held_after = windows_snapshot_handle(&source_file)?;
    if !destination_metadata.is_dir()
        || source_snapshot.identity != destination_snapshot.identity
        || source_snapshot.identity != held_after.identity
        || path_entry_exists(source)?
    {
        return Err(LegacyProjectionCleanupError::JournalConflict(format!(
            "atomic move identity check failed: {} -> {}",
            source.display(),
            destination.display()
        )));
    }
    super::durable_sync_directory(source.parent().ok_or_else(|| {
        LegacyProjectionCleanupError::UnsafePath {
            path: source.to_path_buf(),
            reason: "move source has no parent".to_owned(),
        }
    })?)?;
    super::durable_sync_directory(destination.parent().ok_or_else(|| {
        LegacyProjectionCleanupError::UnsafePath {
            path: destination.to_path_buf(),
            reason: "move destination has no parent".to_owned(),
        }
    })?)?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", windows)))]
fn atomic_move_directory_no_replace(
    source: &Path,
    destination: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    Err(LegacyProjectionCleanupError::UnsafePath {
        path: source.to_path_buf(),
        reason: format!(
            "atomic no-replace directory move is unsupported for {}",
            destination.display()
        ),
    })
}

fn path_entry_exists(path: &Path) -> Result<bool, LegacyProjectionCleanupError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn backup_root_path(backup_dir: &Path, kind: LegacyProjectionRootKind) -> PathBuf {
    backup_dir.join(ROOTS_DIR).join(kind.backup_name())
}

fn require_root_matches(
    path: &Path,
    expected: &LegacyProjectionRootInventory,
) -> Result<(), LegacyProjectionCleanupError> {
    let actual = scan_present_root(expected.kind, path, &expected.absolute_path)?;
    if actual != *expected {
        return Err(LegacyProjectionCleanupError::JournalConflict(format!(
            "legacy root content does not match inventory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn write_journal(
    backup_dir: &Path,
    journal: &CleanupJournal,
) -> Result<(), LegacyProjectionCleanupError> {
    let encoded = toml::to_string_pretty(journal)?;
    let path = backup_dir.join(JOURNAL_FILE);
    if path_entry_exists(&path)? {
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_file() {
            return Err(LegacyProjectionCleanupError::BackupConflict(path));
        }
        super::durable_replace_file_contents(&path, encoded.as_bytes())?;
    } else {
        super::durable_create_new_file(&path, encoded.as_bytes())?;
    }
    Ok(())
}

fn load_and_validate_journal(
    database_path: &Path,
    database_instance_id: &str,
    backup_dir: &Path,
) -> Result<CleanupJournal, LegacyProjectionCleanupError> {
    validate_backup_layout(backup_dir)?;
    let path = backup_dir.join(JOURNAL_FILE);
    let journal = match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            toml::from_str::<CleanupJournal>(&fs::read_to_string(&path)?)?
        }
        Ok(_) => return Err(LegacyProjectionCleanupError::BackupConflict(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            recover_unpublished_initial_journal(
                database_path,
                database_instance_id,
                backup_dir,
                &path,
            )?
        }
        Err(error) => return Err(error.into()),
    };
    validate_journal_binding(&journal, database_path, database_instance_id)?;
    Ok(journal)
}

fn load_and_validate_journal_read_only(
    database_path: &Path,
    database_instance_id: &str,
    backup_dir: &Path,
) -> Result<CleanupJournal, LegacyProjectionCleanupError> {
    validate_backup_layout(backup_dir)?;
    let path = backup_dir.join(JOURNAL_FILE);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(LegacyProjectionCleanupError::BackupConflict(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(LegacyProjectionCleanupError::JournalConflict(
                "backup directory has no published cleanup journal".to_owned(),
            ));
        }
        Err(error) => return Err(error.into()),
    }
    let journal = toml::from_str::<CleanupJournal>(&fs::read_to_string(&path)?)?;
    validate_journal_binding(&journal, database_path, database_instance_id)?;
    Ok(journal)
}

fn recover_unpublished_initial_journal(
    database_path: &Path,
    database_instance_id: &str,
    backup_dir: &Path,
    journal_path: &Path,
) -> Result<CleanupJournal, LegacyProjectionCleanupError> {
    let mut recovered = None;
    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_durable_candidate(&name, JOURNAL_FILE, "new") || !entry.file_type()?.is_file() {
            continue;
        }
        let Ok(candidate) = toml::from_str::<CleanupJournal>(&fs::read_to_string(entry.path())?)
        else {
            continue;
        };
        if validate_journal_binding(&candidate, database_path, database_instance_id).is_err()
            || candidate.phase != CleanupPhase::Applying
            || candidate.roots.iter().any(|root| {
                !matches!(
                    root.state,
                    CleanupRootState::Absent | CleanupRootState::Pending
                )
            })
        {
            continue;
        }
        match &recovered {
            Some(existing) if existing != &candidate => {
                return Err(LegacyProjectionCleanupError::JournalConflict(
                    "multiple incompatible unpublished cleanup journals exist".to_owned(),
                ));
            }
            Some(_) => {}
            None => recovered = Some(candidate),
        }
    }
    let recovered = recovered.ok_or_else(|| {
        LegacyProjectionCleanupError::JournalConflict(
            "backup directory has no recoverable cleanup journal".to_owned(),
        )
    })?;
    let encoded = toml::to_string_pretty(&recovered)?;
    super::durable_create_new_file(journal_path, encoded.as_bytes())?;
    Ok(recovered)
}

fn validate_journal_binding(
    journal: &CleanupJournal,
    database_path: &Path,
    database_instance_id: &str,
) -> Result<(), LegacyProjectionCleanupError> {
    if journal.format_version != CLEANUP_FORMAT_VERSION
        || journal.database_path != database_path
        || journal.database_instance_id != database_instance_id
        || !journal.inventory_digest.starts_with("sha256:")
        || journal.inventory_digest.len() != 71
        || journal.roots.len() != LEGACY_PROJECTION_ROOTS.len()
    {
        return Err(LegacyProjectionCleanupError::JournalConflict(
            "journal binding does not match the selected database".to_owned(),
        ));
    }
    let database_parent =
        database_path
            .parent()
            .ok_or_else(|| LegacyProjectionCleanupError::UnsafePath {
                path: database_path.to_path_buf(),
                reason: "canonical database path has no parent".to_owned(),
            })?;
    for (expected_kind, root) in LEGACY_PROJECTION_ROOTS.iter().zip(&journal.roots) {
        let expected_path = database_parent.join(expected_kind.relative_path());
        if root.inventory.kind != *expected_kind
            || root.inventory.relative_path != expected_kind.relative_path()
            || root.inventory.absolute_path != expected_path
            || root.inventory.digest.len() != 71
            || !root.inventory.digest.starts_with("sha256:")
            || (root.inventory.present && root.state == CleanupRootState::Absent)
            || (!root.inventory.present && root.state != CleanupRootState::Absent)
        {
            return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                "journal root binding is invalid for {}",
                expected_kind.relative_path()
            )));
        }
    }
    let roots = journal
        .roots
        .iter()
        .map(|root| root.inventory.clone())
        .collect::<Vec<_>>();
    let recomputed = inventory_digest(database_path, database_instance_id, &roots)?;
    if recomputed != journal.inventory_digest {
        return Err(LegacyProjectionCleanupError::JournalConflict(
            "journal inventory digest does not match its root records".to_owned(),
        ));
    }
    Ok(())
}

fn validate_backup_layout(backup_dir: &Path) -> Result<(), LegacyProjectionCleanupError> {
    let mut entries = fs::read_dir(backup_dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let metadata = fs::symlink_metadata(entry.path())?;
        match name.to_str() {
            Some(JOURNAL_FILE | MANIFEST_FILE) if metadata.is_file() => {}
            Some(ROOTS_DIR) if metadata.is_dir() => validate_backup_roots_layout(&entry.path())?,
            Some(name)
                if metadata.is_file()
                    && (is_any_durable_candidate(name, JOURNAL_FILE)
                        || is_any_durable_candidate(name, MANIFEST_FILE)) => {}
            _ => {
                return Err(LegacyProjectionCleanupError::BackupConflict(entry.path()));
            }
        }
    }
    let roots = backup_dir.join(ROOTS_DIR);
    if !fs::symlink_metadata(&roots).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(LegacyProjectionCleanupError::BackupConflict(roots));
    }
    Ok(())
}

fn is_any_durable_candidate(name: &str, destination: &str) -> bool {
    is_durable_candidate(name, destination, "new")
        || is_durable_candidate(name, destination, "replace")
}

fn is_durable_candidate(name: &str, destination: &str, purpose: &str) -> bool {
    let prefix = format!(".{destination}.{purpose}.");
    let Some(suffix) = name.strip_prefix(&prefix) else {
        return false;
    };
    let mut parts = suffix.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(pid), Some(nonce), None)
            if !pid.is_empty()
                && !nonce.is_empty()
                && pid.bytes().all(|byte| byte.is_ascii_digit())
                && nonce.bytes().all(|byte| byte.is_ascii_digit())
    )
}

fn validate_backup_roots_layout(roots_dir: &Path) -> Result<(), LegacyProjectionCleanupError> {
    let mut entries = fs::read_dir(roots_dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(LegacyProjectionCleanupError::BackupConflict(entry.path()));
        };
        let known = LEGACY_PROJECTION_ROOTS
            .iter()
            .any(|kind| kind.backup_name() == name);
        let metadata = entry.file_type()?;
        if !known || !metadata.is_dir() {
            return Err(LegacyProjectionCleanupError::BackupConflict(entry.path()));
        }
    }
    Ok(())
}

fn manifest_from_journal(journal: &CleanupJournal) -> LegacyProjectionBackupManifest {
    LegacyProjectionBackupManifest {
        format_version: journal.format_version,
        database_instance_id: journal.database_instance_id.clone(),
        database_path: journal.database_path.clone(),
        inventory_digest: journal.inventory_digest.clone(),
        roots: journal
            .roots
            .iter()
            .map(|root| root.inventory.clone())
            .collect(),
    }
}

fn write_or_validate_manifest(
    backup_dir: &Path,
    manifest: &LegacyProjectionBackupManifest,
) -> Result<(), LegacyProjectionCleanupError> {
    let path = backup_dir.join(MANIFEST_FILE);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {
            let existing: LegacyProjectionBackupManifest =
                toml::from_str(&fs::read_to_string(&path)?)?;
            if existing != *manifest {
                return Err(LegacyProjectionCleanupError::ManifestConflict(
                    "existing backup manifest does not match the cleanup journal".to_owned(),
                ));
            }
        }
        Ok(_) => return Err(LegacyProjectionCleanupError::BackupConflict(path)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let encoded = toml::to_string_pretty(manifest)?;
            super::durable_create_new_file(&path, encoded.as_bytes())?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn load_and_validate_manifest(
    journal: &CleanupJournal,
    backup_dir: &Path,
) -> Result<LegacyProjectionBackupManifest, LegacyProjectionCleanupError> {
    let path = backup_dir.join(MANIFEST_FILE);
    let metadata = fs::symlink_metadata(&path).map_err(|_| {
        LegacyProjectionCleanupError::ManifestConflict("manifest missing".to_owned())
    })?;
    if !metadata.is_file() {
        return Err(LegacyProjectionCleanupError::BackupConflict(path));
    }
    let manifest: LegacyProjectionBackupManifest = toml::from_str(&fs::read_to_string(&path)?)?;
    if manifest != manifest_from_journal(journal) {
        return Err(LegacyProjectionCleanupError::ManifestConflict(
            "backup manifest does not match the cleanup journal".to_owned(),
        ));
    }
    Ok(manifest)
}

fn validate_completed_backup(
    journal: &CleanupJournal,
    backup_dir: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    for root in &journal.roots {
        let source = &root.inventory.absolute_path;
        let backup = backup_root_path(backup_dir, root.inventory.kind);
        if root.inventory.present {
            if root.state != CleanupRootState::Moved
                || path_entry_exists(source)?
                || !path_entry_exists(&backup)?
            {
                return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                    "completed backup state does not match filesystem: {}",
                    source.display()
                )));
            }
            require_root_matches(&backup, &root.inventory)?;
        } else if root.state != CleanupRootState::Absent
            || path_entry_exists(source)?
            || path_entry_exists(&backup)?
        {
            return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                "absent root changed after cleanup: {}",
                source.display()
            )));
        }
    }
    Ok(())
}

fn validate_restore_preflight(
    journal: &CleanupJournal,
    backup_dir: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    for root in &journal.roots {
        let source = &root.inventory.absolute_path;
        let backup = backup_root_path(backup_dir, root.inventory.kind);
        if !root.inventory.present {
            if root.state != CleanupRootState::Absent
                || path_entry_exists(source)?
                || path_entry_exists(&backup)?
            {
                return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                    "absent root changed before restore: {}",
                    source.display()
                )));
            }
            continue;
        }
        match root.state {
            CleanupRootState::Moved => {
                match (path_entry_exists(source)?, path_entry_exists(&backup)?) {
                    (false, true) => require_root_matches(&backup, &root.inventory)?,
                    (true, false) if journal.phase == CleanupPhase::Restoring => {
                        require_root_matches(source, &root.inventory)?
                    }
                    _ => {
                        return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                            "restore preflight is ambiguous: {}",
                            source.display()
                        )));
                    }
                }
            }
            CleanupRootState::Restored if journal.phase == CleanupPhase::Restoring => {
                if !path_entry_exists(source)? || path_entry_exists(&backup)? {
                    return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                        "restored preflight state is invalid: {}",
                        source.display()
                    )));
                }
                require_root_matches(source, &root.inventory)?;
            }
            _ => {
                return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                    "root has invalid restore state: {}",
                    source.display()
                )));
            }
        }
    }
    Ok(())
}

fn validate_restored_state(
    journal: &CleanupJournal,
    backup_dir: &Path,
) -> Result<(), LegacyProjectionCleanupError> {
    for root in &journal.roots {
        let source = &root.inventory.absolute_path;
        let backup = backup_root_path(backup_dir, root.inventory.kind);
        if root.inventory.present {
            if root.state != CleanupRootState::Restored
                || !path_entry_exists(source)?
                || path_entry_exists(&backup)?
            {
                return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                    "restored state does not match filesystem: {}",
                    source.display()
                )));
            }
            require_root_matches(source, &root.inventory)?;
        } else if root.state != CleanupRootState::Absent
            || path_entry_exists(source)?
            || path_entry_exists(&backup)?
        {
            return Err(LegacyProjectionCleanupError::JournalConflict(format!(
                "absent root changed during restore: {}",
                source.display()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temp = tempdir().expect("temporary cleanup fixture");
        let data = temp.path().join("data");
        let backup = temp.path().join("backup");
        fs::create_dir(&data).expect("data directory");
        let db = data.join("kanban.db");
        fs::write(&db, b"sqlite-fixture").expect("database fixture");
        (temp, db, backup)
    }

    fn root_path(db: &Path, kind: LegacyProjectionRootKind) -> PathBuf {
        let parent = db.parent().expect("database parent");
        match kind {
            LegacyProjectionRootKind::TantivyV1 => parent.join("index/v1/tasks"),
            LegacyProjectionRootKind::OxigraphV1 => parent.join("index/v1/graph"),
            LegacyProjectionRootKind::LanceDbV1 => parent.join("index/v1/vectors"),
            LegacyProjectionRootKind::TantivyUnscopedV2 => parent.join("index/v2/tantivy_tasks"),
            LegacyProjectionRootKind::OxigraphUnscopedV2 => {
                parent.join("index/v2/oxigraph_relations")
            }
        }
    }

    fn seed_all_roots(db: &Path) {
        for (index, kind) in LEGACY_PROJECTION_ROOTS.iter().enumerate().rev() {
            let root = root_path(db, *kind);
            fs::create_dir_all(root.join("nested")).expect("legacy root");
            fs::write(
                root.join(format!("entry-{index}.bin")),
                format!("legacy-{index}"),
            )
            .expect("legacy file");
            fs::write(root.join("nested/empty-proof"), b"").expect("nested file");
        }
    }

    #[test]
    fn legacy_cleanup_inventory_is_fixed_deterministic_and_excludes_database_scoped_v2() {
        let (_temp, db, _backup) = fixture();
        seed_all_roots(&db);
        let canonical_v2 = db
            .parent()
            .unwrap()
            .join("index/v2/databases/db_fixture/tantivy_tasks/generations/gen_active");
        fs::create_dir_all(&canonical_v2).expect("canonical v2 fixture");
        let sentinel = canonical_v2.join("sentinel");
        fs::write(&sentinel, b"canonical-a").expect("canonical sentinel");

        let first = inventory_legacy_projection_roots(&db, "db_fixture").expect("first inventory");
        let second =
            inventory_legacy_projection_roots(&db, "db_fixture").expect("second inventory");

        assert_eq!(first, second);
        assert_eq!(first.format_version, 1);
        assert_eq!(
            first
                .roots
                .iter()
                .map(|root| root.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "index/v1/tasks",
                "index/v1/graph",
                "index/v1/vectors",
                "index/v2/tantivy_tasks",
                "index/v2/oxigraph_relations",
            ]
        );
        assert!(first.roots.iter().all(|root| root.present));
        assert!(first.inventory_digest.starts_with("sha256:"));
        assert_eq!(first.inventory_digest.len(), 71);

        fs::write(&sentinel, b"canonical-b").expect("change canonical sentinel");
        let canonical_changed =
            inventory_legacy_projection_roots(&db, "db_fixture").expect("third inventory");
        assert_eq!(canonical_changed.inventory_digest, first.inventory_digest);

        fs::write(
            root_path(&db, LegacyProjectionRootKind::TantivyV1).join("entry-0.bin"),
            b"legacy-mutated",
        )
        .expect("change legacy file");
        let legacy_changed =
            inventory_legacy_projection_roots(&db, "db_fixture").expect("fourth inventory");
        assert_ne!(legacy_changed.inventory_digest, first.inventory_digest);
    }

    #[cfg(unix)]
    #[test]
    fn legacy_cleanup_inventory_rejects_symlinks_and_special_entries() {
        use std::os::unix::{fs::symlink, net::UnixListener};

        let (_temp, db, _backup) = fixture();
        let root = root_path(&db, LegacyProjectionRootKind::TantivyV1);
        fs::create_dir_all(&root).expect("legacy root");
        let external = db.parent().unwrap().parent().unwrap().join("external");
        fs::write(&external, b"do-not-follow").expect("external file");
        symlink(&external, root.join("link")).expect("legacy symlink");

        let error =
            inventory_legacy_projection_roots(&db, "db_fixture").expect_err("symlink rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsupportedEntry(path) if path.ends_with("link")
        ));
        assert_eq!(fs::read(&external).unwrap(), b"do-not-follow");

        fs::remove_file(root.join("link")).expect("remove symlink");
        let socket = root.join("socket");
        let _listener = UnixListener::bind(&socket).expect("unix socket");
        let error =
            inventory_legacy_projection_roots(&db, "db_fixture").expect_err("socket rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsupportedEntry(path) if path.ends_with("socket")
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_apply_rejects_stale_digest_overlap_and_cross_filesystem() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        fs::write(
            root_path(&db, LegacyProjectionRootKind::OxigraphV1).join("entry-1.bin"),
            b"changed",
        )
        .expect("stale inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let error = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect_err("stale digest rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::DigestMismatch { .. }
        ));
        assert!(!backup.exists());

        let refreshed =
            inventory_legacy_projection_roots(&db, "db_fixture").expect("fresh inventory");
        let overlap = db.parent().unwrap().join("backup");
        let error = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &refreshed.inventory_digest,
            &overlap,
        )
        .expect_err("overlap rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::Overlap(path) if path == overlap
        ));

        let error =
            require_same_filesystem_id(1, 2, db.parent().unwrap(), backup.parent().unwrap())
                .expect_err("cross-filesystem move rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::CrossFilesystem { .. }
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_apply_enforces_the_explicit_resume_decision_inside_the_guarded_path() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");

        let missing = apply_legacy_projection_cleanup_with_resume_decision(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            true,
        )
        .expect_err("resume cannot create a fresh backup");
        assert!(
            matches!(missing, LegacyProjectionCleanupError::ResumeDecision(message)
                if message.contains("no backup state to resume"))
        );
        assert!(!backup.exists());
        assert!(
            inventory
                .roots
                .iter()
                .filter(|root| root.present)
                .all(|root| root.absolute_path.is_dir())
        );

        let applied = apply_legacy_projection_cleanup_with_resume_decision(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            false,
        )
        .expect("fresh cleanup");
        assert!(!applied.resumed);

        let implicit = apply_legacy_projection_cleanup_with_resume_decision(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            false,
        )
        .expect_err("existing backup requires explicit resume");
        assert!(
            matches!(implicit, LegacyProjectionCleanupError::ResumeDecision(message)
                if message.contains("use --resume"))
        );

        let resumed = apply_legacy_projection_cleanup_with_resume_decision(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            true,
        )
        .expect("explicit completed-journal resume");
        assert!(resumed.resumed);
        assert_eq!(resumed.manifest, applied.manifest);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_same_device_different_mount_preflight_never_mutates_journal() {
        use std::os::unix::fs::MetadataExt as _;

        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let database_parent = db.parent().expect("database parent").to_path_buf();
        let backup_parent = backup.parent().expect("backup parent").to_path_buf();
        assert_eq!(
            fs::symlink_metadata(&database_parent).unwrap().dev(),
            fs::symlink_metadata(&backup_parent).unwrap().dev(),
            "fixture must prove st_dev alone cannot distinguish the injected mount domains"
        );

        let database_mount = database_parent.clone();
        let error = apply_legacy_projection_cleanup_with_filesystem_id(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            move |path| {
                Ok(if path.starts_with(&database_mount) {
                    41
                } else {
                    42
                })
            },
        )
        .expect_err("different mount ids on one st_dev must fail before journal publish");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::CrossFilesystem { .. }
        ));
        assert!(!backup.exists());
        assert!(
            inventory
                .roots
                .iter()
                .all(|root| root.absolute_path.is_dir())
        );

        apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("same-mount cleanup apply");
        let journal_path = backup.join(JOURNAL_FILE);
        let journal_before = fs::read(&journal_path).expect("completed journal before restore");
        let database_mount = database_parent.clone();
        let error = restore_legacy_projection_backup_with_filesystem_id(
            &guard,
            &db,
            "db_fixture",
            &backup,
            move |path| {
                Ok(if path.starts_with(&database_mount) {
                    41
                } else {
                    42
                })
            },
        )
        .expect_err("different mount ids on one st_dev must fail before restoring phase");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::CrossFilesystem { .. }
        ));
        assert_eq!(
            fs::read(&journal_path).expect("journal after failed restore preflight"),
            journal_before
        );
        let canonical_db = canonical_database_path(&db).unwrap();
        let journal =
            load_and_validate_journal(&canonical_db, "db_fixture", &backup).expect("journal");
        assert_eq!(journal.phase, CleanupPhase::Completed);
        assert!(inventory.roots.iter().all(|root| {
            !root.absolute_path.exists()
                && backup
                    .join(ROOTS_DIR)
                    .join(root.kind.backup_name())
                    .is_dir()
        }));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_apply_moves_only_exact_roots_and_restore_rehearsal_succeeds() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let canonical_v2 = db
            .parent()
            .unwrap()
            .join("index/v2/databases/db_fixture/tantivy_tasks/generations/gen_active");
        fs::create_dir_all(&canonical_v2).expect("canonical v2 fixture");
        let sentinel = canonical_v2.join("sentinel");
        fs::write(&sentinel, b"canonical").expect("canonical sentinel");
        let database_before = fs::read(&db).expect("database bytes before cleanup");
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");

        let outcome = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("cleanup apply");
        assert!(!outcome.resumed);
        for root in &inventory.roots {
            assert!(!root.absolute_path.exists());
            assert!(backup.join("roots").join(root.kind.backup_name()).is_dir());
        }
        assert_eq!(fs::read(&sentinel).unwrap(), b"canonical");
        assert_eq!(fs::read(&db).unwrap(), database_before);
        let verified =
            verify_legacy_projection_backup(&db, "db_fixture", &backup).expect("backup verifies");
        assert_eq!(verified, outcome.manifest);

        let restored = restore_legacy_projection_backup(&guard, &db, "db_fixture", &backup)
            .expect("restore rehearsal");
        assert!(!restored.resumed);
        let after =
            inventory_legacy_projection_roots(&db, "db_fixture").expect("restored inventory");
        assert_eq!(after.inventory_digest, inventory.inventory_digest);
        assert_eq!(fs::read(&sentinel).unwrap(), b"canonical");
        assert_eq!(fs::read(&db).unwrap(), database_before);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_restore_rejects_symlinked_managed_ancestry_before_phase_change() {
        use std::os::unix::fs::symlink;

        let (temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("cleanup apply");
        let journal_path = backup.join(JOURNAL_FILE);
        let journal_before = fs::read(&journal_path).expect("completed journal");
        let index = db.parent().unwrap().join("index");
        let held_index = db.parent().unwrap().join("index-held");
        fs::rename(&index, &held_index).expect("displace managed index ancestry");
        let external_index = temp.path().join("external-index");
        fs::create_dir_all(external_index.join("v1")).expect("external v1 parent");
        fs::create_dir_all(external_index.join("v2")).expect("external v2 parent");
        symlink(&external_index, &index).expect("replace index with same-filesystem symlink");

        let error = restore_legacy_projection_backup(&guard, &db, "db_fixture", &backup)
            .expect_err("managed restore must not follow an intermediate symlink");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsafePath { .. } | LegacyProjectionCleanupError::Io(_)
        ));
        assert_eq!(fs::read(&journal_path).unwrap(), journal_before);
        let canonical_db = canonical_database_path(&db).unwrap();
        let journal =
            load_and_validate_journal(&canonical_db, "db_fixture", &backup).expect("journal");
        assert_eq!(journal.phase, CleanupPhase::Completed);
        assert!(!external_index.join("v1/tasks").exists());
        assert!(!external_index.join("v1/graph").exists());
        assert!(!external_index.join("v1/vectors").exists());
        assert!(!external_index.join("v2/tantivy_tasks").exists());
        assert!(!external_index.join("v2/oxigraph_relations").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_apply_resumes_rename_that_crashed_before_journal_update() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let mut moves = 0;
        let error = apply_legacy_projection_cleanup_with_after_move(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            |_| {
                moves += 1;
                if moves == 1 {
                    return Err(LegacyProjectionCleanupError::JournalConflict(
                        "injected crash after rename".to_owned(),
                    ));
                }
                Ok(())
            },
        )
        .expect_err("injected crash");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(message)
                if message == "injected crash after rename"
        ));

        let resumed = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("resume cleanup");
        assert!(resumed.resumed);
        verify_legacy_projection_backup(&db, "db_fixture", &backup)
            .expect("resumed backup verifies");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_restore_resumes_rename_that_crashed_before_journal_update() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("cleanup apply");

        let mut moves = 0;
        let error = restore_legacy_projection_backup_with_after_move(
            &guard,
            &db,
            "db_fixture",
            &backup,
            |_| {
                moves += 1;
                if moves == 1 {
                    return Err(LegacyProjectionCleanupError::JournalConflict(
                        "injected restore crash after rename".to_owned(),
                    ));
                }
                Ok(())
            },
        )
        .expect_err("injected restore crash");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(message)
                if message == "injected restore crash after rename"
        ));

        let restored = restore_legacy_projection_backup(&guard, &db, "db_fixture", &backup)
            .expect("resume restore");
        assert!(restored.resumed);
        let after =
            inventory_legacy_projection_roots(&db, "db_fixture").expect("restored inventory");
        assert_eq!(after.inventory_digest, inventory.inventory_digest);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_retries_after_crash_before_initial_layout_publish() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");

        let error = apply_legacy_projection_cleanup_with_before_initial_publish(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            |staging| {
                assert!(staging.join(JOURNAL_FILE).is_file());
                assert!(staging.join(ROOTS_DIR).is_dir());
                Err(LegacyProjectionCleanupError::JournalConflict(
                    "injected crash before initial publish".to_owned(),
                ))
            },
        )
        .expect_err("injected pre-publish crash");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(message)
                if message == "injected crash before initial publish"
        ));
        assert!(!backup.exists());
        assert!(
            inventory
                .roots
                .iter()
                .all(|root| root.absolute_path.is_dir())
        );

        let resumed = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("fresh retry after unpublished staging");
        assert!(!resumed.resumed);
        verify_legacy_projection_backup(&db, "db_fixture", &backup)
            .expect("retried backup verifies");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_restore_missing_journal_is_strictly_read_only() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("cleanup apply");
        let journal = backup.join(JOURNAL_FILE);
        let unpublished = backup.join(format!(".{JOURNAL_FILE}.new.123.456"));
        fs::rename(&journal, &unpublished).expect("simulate unpublished journal candidate");
        let before = fs::read(&unpublished).expect("candidate bytes");
        let backup_entries = || {
            let mut entries = fs::read_dir(&backup)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>();
            entries.sort();
            entries
        };
        let entries_before = backup_entries();

        let error = restore_legacy_projection_backup(&guard, &db, "db_fixture", &backup)
            .expect_err("restore must not recover or publish a missing journal");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(message)
                if message.contains("no published cleanup journal")
        ));
        assert!(!journal.exists());
        assert_eq!(fs::read(&unpublished).unwrap(), before);
        assert_eq!(backup_entries(), entries_before);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_never_replaces_a_racing_backup_destination() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let mut moves = 0;
        apply_legacy_projection_cleanup_with_after_move(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            |_| {
                moves += 1;
                if moves == 1 {
                    return Err(LegacyProjectionCleanupError::JournalConflict(
                        "stop before second root".to_owned(),
                    ));
                }
                Ok(())
            },
        )
        .expect_err("stop after first root");

        let racing_destination = backup
            .join(ROOTS_DIR)
            .join(LegacyProjectionRootKind::OxigraphV1.backup_name());
        fs::create_dir(&racing_destination).expect("racing destination");
        fs::write(racing_destination.join("sentinel"), b"must-not-replace")
            .expect("racing sentinel");
        let source = root_path(&db, LegacyProjectionRootKind::OxigraphV1);
        let source_before = fs::read(source.join("entry-1.bin")).expect("source before retry");

        let error = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect_err("racing destination rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(_)
        ));
        assert_eq!(
            fs::read(racing_destination.join("sentinel")).unwrap(),
            b"must-not-replace"
        );
        assert_eq!(fs::read(source.join("entry-1.bin")).unwrap(), source_before);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_scan_ticket_rejects_source_inode_swap_without_moving_either_tree() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let verified_original = db.parent().unwrap().join("verified-original-tantivy");
        let replacement_sentinel = b"replacement-must-stay-at-source";
        let mut injected = false;

        let error = apply_legacy_projection_cleanup_with_before_move(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            |kind, source, _destination| {
                if !injected {
                    assert_eq!(kind, LegacyProjectionRootKind::TantivyV1);
                    fs::rename(source, &verified_original)?;
                    fs::create_dir(source)?;
                    fs::write(source.join("replacement"), replacement_sentinel)?;
                    injected = true;
                }
                Ok(())
            },
        )
        .expect_err("scan-to-rename source replacement must fail closed");

        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsafePath { .. }
        ));
        let source = root_path(&db, LegacyProjectionRootKind::TantivyV1);
        let destination = backup
            .join(ROOTS_DIR)
            .join(LegacyProjectionRootKind::TantivyV1.backup_name());
        assert_eq!(
            fs::read(source.join("replacement")).unwrap(),
            replacement_sentinel
        );
        assert!(verified_original.join("entry-0.bin").is_file());
        assert!(!destination.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_scan_ticket_rejects_parent_swap_without_moving_verified_root() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let mut verified_parent = None;

        let error = apply_legacy_projection_cleanup_with_before_move(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            |_kind, source, _destination| {
                if verified_parent.is_none() {
                    let parent = source.parent().expect("source parent");
                    let displaced = parent.with_file_name("v1-verified-parent");
                    fs::rename(parent, &displaced)?;
                    fs::create_dir_all(source)?;
                    fs::write(source.join("replacement"), b"replacement-parent")?;
                    verified_parent = Some(displaced);
                }
                Ok(())
            },
        )
        .expect_err("scan-to-rename parent replacement must fail closed");

        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsafePath { .. }
        ));
        let displaced = verified_parent.expect("parent swap injected");
        assert!(displaced.join("tasks/entry-0.bin").is_file());
        assert_eq!(
            fs::read(root_path(&db, LegacyProjectionRootKind::TantivyV1).join("replacement"))
                .unwrap(),
            b"replacement-parent"
        );
        assert!(
            !backup
                .join(ROOTS_DIR)
                .join(LegacyProjectionRootKind::TantivyV1.backup_name())
                .exists()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_final_source_name_swap_rolls_back_the_unverified_tree() {
        let temp = tempdir().unwrap();
        let source_parent = temp.path().join("source-parent");
        let destination_parent = temp.path().join("destination-parent");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("root");
        let destination = destination_parent.join("root");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("verified"), b"verified").unwrap();
        let ticket = linux_open_root_scan_ticket(&source).expect("verified ticket");
        let destination_parent_file =
            linux_open_stable_directory(&destination_parent).expect("destination parent");
        let displaced_verified = source_parent.join("verified-root");

        let error = linux_atomic_move_ticket_to_parent_no_replace(
            &ticket,
            &destination,
            &destination_parent_file,
            || {
                fs::rename(&source, &displaced_verified)?;
                fs::create_dir(&source)?;
                fs::write(source.join("replacement"), b"unverified")?;
                Ok(())
            },
            noop_cleanup_atomic_hook,
        )
        .expect_err("final source-name replacement must fail and roll back");

        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(_)
        ));
        assert_eq!(fs::read(source.join("replacement")).unwrap(), b"unverified");
        assert_eq!(
            fs::read(displaced_verified.join("verified")).unwrap(),
            b"verified"
        );
        assert!(!destination.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_final_destination_parent_swap_rolls_back_to_source() {
        let temp = tempdir().unwrap();
        let source_parent = temp.path().join("source-parent");
        let destination_parent = temp.path().join("destination-parent");
        fs::create_dir(&source_parent).unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let source = source_parent.join("root");
        let destination = destination_parent.join("root");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("verified"), b"verified").unwrap();
        let ticket = linux_open_root_scan_ticket(&source).expect("verified ticket");
        let destination_parent_file =
            linux_open_stable_directory(&destination_parent).expect("destination parent");
        let displaced_parent = temp.path().join("destination-parent-held");

        let error = linux_atomic_move_ticket_to_parent_no_replace(
            &ticket,
            &destination,
            &destination_parent_file,
            || {
                fs::rename(&destination_parent, &displaced_parent)?;
                fs::create_dir(&destination_parent)?;
                Ok(())
            },
            noop_cleanup_atomic_hook,
        )
        .expect_err("displaced destination parent must trigger held-fd rollback");

        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(_)
        ));
        assert_eq!(fs::read(source.join("verified")).unwrap(), b"verified");
        assert!(!destination.exists());
        assert!(!displaced_parent.join("root").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_post_rename_database_replacement_rolls_back_source() {
        let (temp, db, _backup) = fixture();
        let source = db.parent().unwrap().join("move-source");
        let destination_parent = temp.path().join("move-destination");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("verified"), b"verified").unwrap();
        fs::create_dir(&destination_parent).unwrap();
        let destination = destination_parent.join("root");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let ticket = linux_open_root_scan_ticket(&source).expect("verified ticket");
        let destination_parent_file =
            linux_open_stable_directory(&destination_parent).expect("destination parent");
        let held_database = db.with_extension("held");

        let error = linux_atomic_move_ticket_to_parent_no_replace(
            &ticket,
            &destination,
            &destination_parent_file,
            noop_cleanup_atomic_hook,
            || {
                fs::rename(&db, &held_database)?;
                fs::write(&db, b"replacement-database")?;
                guard.validate(&db)
            },
        )
        .expect_err("post-rename database replacement must roll back the source");

        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(_)
        ));
        assert_eq!(fs::read(source.join("verified")).unwrap(), b"verified");
        assert!(!destination.exists());
        assert_eq!(fs::read(&db).unwrap(), b"replacement-database");
        assert_eq!(fs::read(&held_database).unwrap(), b"sqlite-fixture");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_revalidates_database_identity_after_scan_before_first_rename() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let held_database = db.with_extension("held");
        let mut injected = false;

        let error = apply_legacy_projection_cleanup_with_before_move(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            |_kind, _source, _destination| {
                if !injected {
                    fs::rename(&db, &held_database)?;
                    fs::write(&db, b"replacement-database")?;
                    injected = true;
                }
                Ok(())
            },
        )
        .expect_err("database replacement must stop the destructive rename");

        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(message)
                if message.contains("database file identity changed")
        ));
        assert_eq!(fs::read(&db).unwrap(), b"replacement-database");
        assert_eq!(fs::read(&held_database).unwrap(), b"sqlite-fixture");
        assert!(
            inventory
                .roots
                .iter()
                .all(|root| root.absolute_path.is_dir())
        );
        assert!(inventory.roots.iter().all(|root| {
            !backup
                .join(ROOTS_DIR)
                .join(root.kind.backup_name())
                .exists()
        }));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_database_replacement_after_last_root_blocks_completed_phase() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let held_database = db.with_extension("held");
        let mut moves = 0;

        let error = apply_legacy_projection_cleanup_with_after_move(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            |_| {
                moves += 1;
                if moves == LEGACY_PROJECTION_ROOTS.len() {
                    fs::rename(&db, &held_database)?;
                    fs::write(&db, b"replacement-database")?;
                }
                Ok(())
            },
        )
        .expect_err("final database guard must reject replacement");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(message)
                if message.contains("database file identity changed")
                    || message.contains("database parent binding changed")
        ));
        let canonical_db = canonical_database_path(&db).unwrap();
        let journal =
            load_and_validate_journal(&canonical_db, "db_fixture", &backup).expect("journal");
        assert_eq!(journal.phase, CleanupPhase::Applying);
        assert!(
            journal
                .roots
                .iter()
                .all(|root| root.state == CleanupRootState::Moved)
        );

        fs::remove_file(&db).expect("remove replacement database");
        fs::rename(&held_database, &db).expect("restore guarded database identity");
        let resumed = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("resume final phase after database identity is restored");
        assert!(resumed.resumed);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_database_replacement_after_last_restore_blocks_restored_phase() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("cleanup apply");
        let held_database = db.with_extension("held");
        let mut moves = 0;

        let error = restore_legacy_projection_backup_with_after_move(
            &guard,
            &db,
            "db_fixture",
            &backup,
            |_| {
                moves += 1;
                if moves == LEGACY_PROJECTION_ROOTS.len() {
                    fs::rename(&db, &held_database)?;
                    fs::write(&db, b"replacement-database")?;
                }
                Ok(())
            },
        )
        .expect_err("final database guard must reject replacement during restore");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::JournalConflict(message)
                if message.contains("database file identity changed")
                    || message.contains("database parent binding changed")
        ));
        let canonical_db = canonical_database_path(&db).unwrap();
        let journal =
            load_and_validate_journal(&canonical_db, "db_fixture", &backup).expect("journal");
        assert_eq!(journal.phase, CleanupPhase::Restoring);
        assert!(
            journal
                .roots
                .iter()
                .all(|root| root.state == CleanupRootState::Restored)
        );

        fs::remove_file(&db).expect("remove replacement database");
        fs::rename(&held_database, &db).expect("restore guarded database identity");
        let resumed = restore_legacy_projection_backup(&guard, &db, "db_fixture", &backup)
            .expect("resume final restore phase after database identity is restored");
        assert!(resumed.resumed);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_initial_destination_race_preserves_source_and_pending_journal() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        let mut raced_destination = None;

        let error = apply_legacy_projection_cleanup_with_before_move(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
            |_kind, _source, destination| {
                if raced_destination.is_none() {
                    fs::create_dir(destination)?;
                    fs::write(destination.join("sentinel"), b"never-replace")?;
                    raced_destination = Some(destination.to_path_buf());
                }
                Ok(())
            },
        )
        .expect_err("destination created after scan must fail closed");

        assert!(matches!(
            error,
            LegacyProjectionCleanupError::BackupConflict(_)
        ));
        let destination = raced_destination.expect("destination race injected");
        assert_eq!(
            fs::read(destination.join("sentinel")).unwrap(),
            b"never-replace"
        );
        assert!(root_path(&db, LegacyProjectionRootKind::TantivyV1).is_dir());
        let canonical_db = canonical_database_path(&db).unwrap();
        let journal =
            load_and_validate_journal(&canonical_db, "db_fixture", &backup).expect("journal");
        assert_eq!(journal.phase, CleanupPhase::Applying);
        assert_eq!(journal.roots[0].state, CleanupRootState::Pending);

        fs::remove_dir_all(&destination).expect("remove injected destination");
        apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("cleanup resumes after destination race");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_restore_destination_race_keeps_backup_and_root_state_moved() {
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");
        apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect("cleanup apply");
        let mut raced_source = None;

        let error = restore_legacy_projection_backup_with_before_move(
            &guard,
            &db,
            "db_fixture",
            &backup,
            |_kind, _backup_source, restore_destination| {
                if raced_source.is_none() {
                    fs::create_dir(restore_destination)?;
                    fs::write(restore_destination.join("sentinel"), b"restore-race")?;
                    raced_source = Some(restore_destination.to_path_buf());
                }
                Ok(())
            },
        )
        .expect_err("restore destination race must fail closed");

        assert!(matches!(
            error,
            LegacyProjectionCleanupError::BackupConflict(_)
        ));
        let source = raced_source.expect("restore race injected");
        assert_eq!(fs::read(source.join("sentinel")).unwrap(), b"restore-race");
        assert!(
            backup
                .join(ROOTS_DIR)
                .join(LegacyProjectionRootKind::TantivyV1.backup_name())
                .is_dir()
        );
        let canonical_db = canonical_database_path(&db).unwrap();
        let journal =
            load_and_validate_journal(&canonical_db, "db_fixture", &backup).expect("journal");
        assert_eq!(journal.phase, CleanupPhase::Restoring);
        assert_eq!(journal.roots[0].state, CleanupRootState::Moved);

        fs::remove_dir_all(&source).expect("remove injected restore destination");
        restore_legacy_projection_backup(&guard, &db, "db_fixture", &backup)
            .expect("restore resumes after destination race");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_inventory_rejects_revisited_inode_identity() {
        let (_temp, db, _backup) = fixture();
        let root = root_path(&db, LegacyProjectionRootKind::TantivyV1);
        fs::create_dir_all(&root).expect("legacy root");
        let first = root.join("first");
        let second = root.join("second");
        fs::write(&first, b"hard-link-content").expect("first hardlink");
        fs::hard_link(&first, &second).expect("second hardlink");

        let error = inventory_legacy_projection_roots(&db, "db_fixture")
            .expect_err("repeated inode identity must fail closed");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsafePath { reason, .. }
                if reason.contains("repeated inode")
        ));
    }

    #[cfg(target_os = "linux")]
    struct BindMount {
        target: PathBuf,
    }

    #[cfg(target_os = "linux")]
    impl BindMount {
        fn try_new(source: &Path, target: &Path) -> Option<Self> {
            let status = std::process::Command::new("mount")
                .arg("--bind")
                .arg(source)
                .arg(target)
                .status()
                .ok()?;
            status.success().then(|| Self {
                target: target.to_path_buf(),
            })
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for BindMount {
        fn drop(&mut self) {
            let _ = std::process::Command::new("umount")
                .arg(&self.target)
                .status();
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_rejects_bind_alias_backup_path_when_mounts_are_available() {
        let (temp, db, _backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let alias = temp.path().join("data-alias");
        fs::create_dir(&alias).expect("alias mountpoint");
        let Some(_mount) = BindMount::try_new(db.parent().unwrap(), &alias) else {
            return;
        };
        let aliased_backup = alias.join("backup");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");

        let error = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &aliased_backup,
        )
        .expect_err("physical alias into database parent must be rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::Overlap(path) if path == aliased_backup
        ));
        assert!(
            inventory
                .roots
                .iter()
                .all(|root| root.absolute_path.is_dir())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_rejects_database_alias_inside_existing_backup_tree_when_mounts_are_available()
    {
        let (temp, db, _backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let candidate = temp.path().join("existing-backup");
        let database_alias = candidate.join("database-alias");
        fs::create_dir(&candidate).expect("existing backup candidate");
        fs::create_dir(&database_alias).expect("database alias mountpoint");
        let Some(_mount) = BindMount::try_new(db.parent().unwrap(), &database_alias) else {
            return;
        };
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");

        let error = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &candidate,
        )
        .expect_err("reverse physical overlap must be rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::Overlap(_)
                | LegacyProjectionCleanupError::UnsafePath { .. }
                | LegacyProjectionCleanupError::Io(_)
        ));
        assert!(
            inventory
                .roots
                .iter()
                .all(|root| root.absolute_path.is_dir())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_inventory_rejects_nested_bind_mount_when_mounts_are_available() {
        let (temp, db, _backup) = fixture();
        let root = root_path(&db, LegacyProjectionRootKind::TantivyV1);
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("nested mountpoint");
        let external = temp.path().join("external-tree");
        fs::create_dir(&external).expect("external tree");
        fs::write(external.join("sentinel"), b"outside").expect("external sentinel");
        let Some(_mount) = BindMount::try_new(&external, &nested) else {
            return;
        };

        let error = inventory_legacy_projection_roots(&db, "db_fixture")
            .expect_err("nested mount must fail closed");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::Io(_) | LegacyProjectionCleanupError::UnsafePath { .. }
        ));
    }

    #[cfg(windows)]
    #[test]
    fn legacy_cleanup_windows_aliases_overlap_and_mutation_is_disabled_before_journal_write() {
        assert!(paths_overlap(
            Path::new(r"\\?\C:\DATA\backup"),
            Path::new(r"c:\data")
        ));
        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");

        let error = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect_err("Windows mutation must fail closed without stable change identity");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsafePath { reason, .. }
                if reason.contains("requires Linux")
        ));
        assert!(!backup.exists());
        assert!(
            inventory
                .roots
                .iter()
                .all(|root| root.absolute_path.is_dir())
        );
    }

    #[test]
    fn durable_legacy_cleanup_windows_inventory_source_contract_holds_non_reparse_directory_handles()
     {
        let source = include_str!("legacy_cleanup.rs");
        let production = source
            .split_once("#[cfg(test)]\nmod tests")
            .expect("legacy cleanup unit-test boundary")
            .0;

        assert!(
            production.contains("let guard = windows_open_inventory_directory(&current)?;")
                && production.contains("allowlist_directory_guards.push(guard);"),
            "every present allowlist component must retain a Windows directory handle"
        );
        assert!(
            production
                .contains("let directory_guard = windows_open_inventory_directory(directory)?;"),
            "each recursive inventory must retain its Windows directory handle"
        );
        assert!(
            production.contains(".share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)"),
            "inventory directory handles must not share delete access"
        );
        assert!(
            production.contains("snapshot.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0"),
            "inventory directory handles must reject reparse points explicitly"
        );
    }

    #[cfg(windows)]
    #[test]
    fn durable_legacy_cleanup_windows_inventory_directory_handle_blocks_path_replacement() {
        let temp = tempdir().expect("temporary Windows directory guard fixture");
        let directory = temp.path().join("inventory");
        let parked = temp.path().join("parked");
        fs::create_dir(&directory).expect("inventory directory");

        let directory_guard =
            windows_open_inventory_directory(&directory).expect("inventory directory guard");
        fs::rename(&directory, &parked)
            .expect_err("non-delete-shared directory handle must block path replacement");
        drop(directory_guard);
        fs::rename(&directory, &parked)
            .expect("directory rename must succeed after inventory guard release");
    }

    #[cfg(windows)]
    #[test]
    fn durable_legacy_cleanup_windows_inventory_directory_rejects_reparse_points() {
        use std::os::windows::fs::symlink_dir;

        let temp = tempdir().expect("temporary Windows reparse fixture");
        let target = temp.path().join("target");
        let reparse = temp.path().join("reparse");
        fs::create_dir(&target).expect("reparse target");
        match symlink_dir(&target, &reparse) {
            Ok(()) => {}
            Err(error) if error.raw_os_error() == Some(1314) => {
                // ERROR_PRIVILEGE_NOT_HELD: this host cannot create the native
                // fixture. windows-latest CI retains the real reparse gate.
                return;
            }
            Err(error) => panic!("create Windows directory reparse fixture: {error}"),
        }

        let error = windows_open_inventory_directory(&reparse)
            .expect_err("directory reparse point must fail closed");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::UnsafePath { path, reason }
                if path == reparse && reason.contains("reparse")
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_cleanup_rejects_backup_symlink_without_touching_target() {
        use std::os::unix::fs::symlink;

        let (_temp, db, backup) = fixture();
        seed_all_roots(&db);
        let inventory = inventory_legacy_projection_roots(&db, "db_fixture").expect("inventory");
        let target = backup.parent().unwrap().join("backup-target");
        fs::create_dir(&target).expect("backup target");
        symlink(&target, &backup).expect("backup symlink");
        let guard = acquire_legacy_projection_cleanup_guard(&db).expect("cleanup physical guards");

        let error = apply_legacy_projection_cleanup(
            &guard,
            &db,
            "db_fixture",
            &inventory.inventory_digest,
            &backup,
        )
        .expect_err("backup symlink rejected");
        assert!(matches!(
            error,
            LegacyProjectionCleanupError::BackupConflict(path) if path == backup
        ));
        assert!(fs::read_dir(&target).unwrap().next().is_none());
        assert!(
            inventory
                .roots
                .iter()
                .all(|root| root.absolute_path.is_dir())
        );
    }
}
