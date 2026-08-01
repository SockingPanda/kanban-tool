const JOURNAL_FORMAT_VERSION: u32 = 2;

/// Durable evidence returned after a staged database is published.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseReplaceReport {
    pub canonical_path: PathBuf,
    pub previous_path: PathBuf,
    pub journal_path: PathBuf,
}

/// Optional immutable binding expected from a staged database.
///
/// The hash and projection fields are also the seam used by a future
/// operator-facing backup restore adapter.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseReplaceOptions {
    pub expected_sha256: Option<String>,
    pub expected_database_instance_id: Option<String>,
    pub expected_protocol_version: Option<i64>,
    pub expected_schema_version: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PublishFailpoint {
    JournalInitial,
    StagedFenced,
    PreviousAnchored,
    StagedDurable,
    CanonicalPublished,
    PostPublishIdentity,
    Rebound,
    ParentFsync,
    JournalCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalPhase {
    Prepared,
    StagedFenced,
    PreviousPublished,
    CanonicalPublished,
    Rebound,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatabaseReplaceJournal {
    format_version: u32,
    canonical_path: PathBuf,
    staged_path: PathBuf,
    previous_path: PathBuf,
    journal_path: PathBuf,
    canonical_identity: FileIdentity,
    staged_identity: FileIdentity,
    staged_sha256: String,
    previous_identity: Option<FileIdentity>,
    placeholder_previous: bool,
    phase: JournalPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileIdentity {
    /// Stable device/volume identity.
    device: u64,
    /// Unix inode, or the low 64 bits of the native Windows file identifier.
    inode: u64,
    /// High 64 bits of the native Windows file identifier. `None` marks a
    /// legacy journal written before stable non-Unix identities were stored.
    #[serde(default)]
    file_id_high: Option<u64>,
    length: u64,
    modified_ns: u128,
}

fn validate_journal_paths(journal: &DatabaseReplaceJournal, journal_path: &Path) -> Result<()> {
    validate_journal_format_version(journal)?;
    let canonical = normalized_path(&journal.canonical_path)?;
    let staged = normalized_path(&journal.staged_path)?;
    let previous = normalized_path(&journal.previous_path)?;
    let journal_name = normalized_path(journal_path)?;
    let recorded_journal = normalized_path(&journal.journal_path)?;
    require_same_parent(&canonical, [&staged, &previous, &journal_name])?;
    require_distinct_paths([&canonical, &staged, &previous, &journal_name])?;
    if canonical != journal.canonical_path
        || staged != journal.staged_path
        || previous != journal.previous_path
        || journal_name != recorded_journal
    {
        return Err(KanbanError::Conflict(
            "replacement journal contains non-canonical or mismatched paths".to_owned(),
        ));
    }
    Ok(())
}

fn validate_journal_format_version(journal: &DatabaseReplaceJournal) -> Result<()> {
    if journal.format_version != JOURNAL_FORMAT_VERSION {
        return Err(KanbanError::InvalidInput(format!(
            "unsupported database replacement journal format: {}",
            journal.format_version
        )));
    }
    Ok(())
}

fn report_for(journal: &DatabaseReplaceJournal) -> Result<DatabaseReplaceReport> {
    Ok(DatabaseReplaceReport {
        canonical_path: journal.canonical_path.clone(),
        previous_path: journal.previous_path.clone(),
        journal_path: journal.journal_path.clone(),
    })
}

fn normalized_path(path: &Path) -> Result<PathBuf> {
    let parent = fs::canonicalize(parent_directory(path)?).map_err(storage)?;
    let name = path.file_name().ok_or_else(|| {
        KanbanError::InvalidInput(format!(
            "replacement path has no final component: {}",
            path.display()
        ))
    })?;
    Ok(parent.join(name))
}

fn parent_directory(path: &Path) -> Result<&Path> {
    Ok(path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new(".")))
}

fn require_same_parent<const N: usize>(canonical: &Path, others: [&Path; N]) -> Result<()> {
    let parent = parent_directory(canonical)?;
    for other in others {
        if parent_directory(other)? != parent {
            return Err(KanbanError::InvalidInput(
                "database replacement paths must share one parent directory".to_owned(),
            ));
        }
    }
    Ok(())
}

fn require_distinct_paths(paths: [&Path; 4]) -> Result<()> {
    for (index, left) in paths.iter().enumerate() {
        for right in paths.iter().skip(index + 1) {
            if left == right {
                return Err(KanbanError::InvalidInput(
                    "database replacement paths must be distinct".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

fn require_regular_file(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            KanbanError::InvalidInput(format!("{label} does not exist: {}", path.display()))
        } else {
            storage(error)
        }
    })?;
    if !metadata.is_file() {
        return Err(KanbanError::InvalidInput(format!(
            "{label} is not a regular file: {}",
            path.display()
        )));
    }
    Ok(())
}

fn require_absent(path: &Path, label: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(KanbanError::Conflict(format!(
            "{label} already exists: {}",
            path.display()
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(storage(error)),
    }
}

fn reject_sqlite_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", path.display()));
        match fs::symlink_metadata(&sidecar) {
            Ok(_) => {
                return Err(KanbanError::Conflict(format!(
                    "database replacement requires no SQLite sidecars: {}",
                    sidecar.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(storage(error)),
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(storage)?;
    let mut digest = Sha256::new();
    io::copy(&mut file, &mut Sha256Writer(&mut digest)).map_err(storage)?;
    Ok(format!("{:x}", digest.finalize()))
}

struct Sha256Writer<'a>(&'a mut Sha256);

impl io::Write for Sha256Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn file_identity(path: &Path) -> Result<FileIdentity> {
    let metadata = fs::metadata(path).map_err(storage)?;
    let stable = database_file_identity(path).map_err(storage)?;
    Ok(identity_from_stable_metadata(&metadata, stable))
}

fn file_identity_from_file(file: &File) -> Result<FileIdentity> {
    let metadata = file.metadata().map_err(storage)?;
    let stable = database_file_identity_from_file(file).map_err(storage)?;
    Ok(identity_from_stable_metadata(&metadata, stable))
}

fn identity_from_stable_metadata(
    metadata: &Metadata,
    stable: DatabaseFileIdentity,
) -> FileIdentity {
    let mut low_bytes = [0_u8; 8];
    let mut high_bytes = [0_u8; 8];
    low_bytes.copy_from_slice(&stable.file_id[..8]);
    high_bytes.copy_from_slice(&stable.file_id[8..]);
    FileIdentity {
        device: stable.volume,
        inode: u64::from_be_bytes(low_bytes),
        file_id_high: Some(u64::from_be_bytes(high_bytes)),
        length: metadata.len(),
        modified_ns: modified_ns(metadata),
    }
}

fn modified_ns(metadata: &Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

pub(super) fn same_file_identity(left: &FileIdentity, right: &FileIdentity) -> bool {
    if left.device != right.device
        || left.inode != right.inode
        || left.length != right.length
        || left.modified_ns != right.modified_ns
    {
        return false;
    }
    match (left.file_id_high, right.file_id_high) {
        (Some(left), Some(right)) => left == right,
        _ => cfg!(unix),
    }
}

/// Compares the retained namespace identity without treating normal writes to
/// the same canonical SQLite inode as a replacement. Native stable file IDs
/// are required; legacy records without one fail closed off Unix.
pub(super) fn same_file_location_identity(left: &FileIdentity, right: &FileIdentity) -> bool {
    left.device == right.device
        && left.inode == right.inode
        && match (left.file_id_high, right.file_id_high) {
            (Some(left), Some(right)) => left == right,
            _ => cfg!(unix),
        }
}

fn read_journal(path: &Path) -> Result<DatabaseReplaceJournal> {
    read_journal_with_hook(path, || Ok(()))
}

fn read_journal_with_hook<Hook>(
    path: &Path,
    mut after_open: Hook,
) -> Result<DatabaseReplaceJournal>
where
    Hook: FnMut() -> Result<()>,
{
    // Read through an opened file descriptor and bind it to the directory
    // entry observed through the same no-follow handle. This rejects a
    // journal symlink and detects a path swap between opening and each path
    // identity check; the subsequent path/guard validation still binds the
    // parsed journal to the caller's canonical database authority.
    let mut file = open_database_file_for_identity(path).map_err(|error| {
        match fs::symlink_metadata(path) {
            Ok(metadata) if !metadata.is_file() => KanbanError::InvalidInput(format!(
                "replacement journal is not a regular file: {}",
                path.display()
            )),
            _ => storage(error),
        }
    })?;
    let opened_identity = file_identity_from_file(&file)?;
    after_open()?;
    let before_identity = file_identity(path)?;
    if !same_file_identity(&opened_identity, &before_identity) {
        return Err(KanbanError::Conflict(
            "replacement journal path changed while being opened".to_owned(),
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(storage)?;
    let after_identity = file_identity(path)?;
    if !same_file_identity(&opened_identity, &after_identity) {
        return Err(KanbanError::Conflict(
            "replacement journal path changed while being read".to_owned(),
        ));
    }
    let journal = serde_json::from_slice(&bytes)
        .map_err(|error| KanbanError::InvalidInput(format!("invalid replacement journal: {error}")))?;
    validate_journal_format_version(&journal)?;
    Ok(journal)
}

fn write_new_journal(journal: &DatabaseReplaceJournal) -> Result<()> {
    let encoded = serde_json::to_vec_pretty(journal).map_err(|error| {
        KanbanError::Storage(format!("failed to encode replacement journal: {error}"))
    })?;
    durable_create_new_file(&journal.journal_path, &encoded).map_err(storage)
}

fn write_journal(journal: &DatabaseReplaceJournal) -> Result<()> {
    let encoded = serde_json::to_vec_pretty(journal).map_err(|error| {
        KanbanError::Storage(format!("failed to encode replacement journal: {error}"))
    })?;
    durable_replace_file_contents(&journal.journal_path, &encoded).map_err(storage)
}

fn storage(error: io::Error) -> KanbanError {
    KanbanError::Storage(error.to_string())
}
