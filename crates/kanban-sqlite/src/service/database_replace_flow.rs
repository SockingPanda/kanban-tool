/// Publishes a closed staged SQLite file under the canonical path.
pub fn publish_staged_database(
    guard: &mut DatabaseReplaceGuard,
    canonical_path: impl AsRef<Path>,
    staged_path: impl AsRef<Path>,
    previous_path: impl AsRef<Path>,
    journal_path: impl AsRef<Path>,
) -> Result<DatabaseReplaceReport> {
    publish_staged_database_with_options(
        guard,
        canonical_path,
        staged_path,
        previous_path,
        journal_path,
        DatabaseReplaceOptions::default(),
    )
}

/// Publishes a staged database after checking an optional immutable binding.
pub fn publish_staged_database_with_options(
    guard: &mut DatabaseReplaceGuard,
    canonical_path: impl AsRef<Path>,
    staged_path: impl AsRef<Path>,
    previous_path: impl AsRef<Path>,
    journal_path: impl AsRef<Path>,
    options: DatabaseReplaceOptions,
) -> Result<DatabaseReplaceReport> {
    publish_staged_database_with_hook(
        guard,
        canonical_path.as_ref(),
        staged_path.as_ref(),
        previous_path.as_ref(),
        journal_path.as_ref(),
        options,
        |_| Ok(()),
    )
}

/// Resumes an interrupted publication from its durable journal.
///
/// The caller must hold a fresh `DatabaseReplaceGuard` for the journal's
/// canonical path. The journal is never deleted or silently overwritten.
pub fn resume_staged_database_replace(
    guard: &mut DatabaseReplaceGuard,
    journal_path: impl AsRef<Path>,
) -> Result<DatabaseReplaceReport> {
    resume_staged_database_replace_with_options(
        guard,
        journal_path,
        DatabaseReplaceOptions::default(),
    )
}

/// Resumes an interrupted publication while rechecking an optional binding.
pub fn resume_staged_database_replace_with_options(
    guard: &mut DatabaseReplaceGuard,
    journal_path: impl AsRef<Path>,
    options: DatabaseReplaceOptions,
) -> Result<DatabaseReplaceReport> {
    let journal_path = normalized_path(journal_path.as_ref())?;
    let mut journal = read_journal(&journal_path)?;
    validate_journal_paths(&journal, &journal_path)?;
    guard.validate_current_database_path_binding(&journal.canonical_path)?;
    reconcile_physical_phase(guard, &mut journal)?;
    continue_publish(guard, &mut journal, &options, |_| Ok(()))
}

fn publish_staged_database_with_hook<Hook>(
    guard: &mut DatabaseReplaceGuard,
    canonical_path: &Path,
    staged_path: &Path,
    previous_path: &Path,
    journal_path: &Path,
    options: DatabaseReplaceOptions,
    mut hook: Hook,
) -> Result<DatabaseReplaceReport>
where
    Hook: FnMut(PublishFailpoint) -> Result<()>,
{
    let canonical_path = normalized_path(canonical_path)?;
    let staged_path = normalized_path(staged_path)?;
    let previous_path = normalized_path(previous_path)?;
    let journal_path = normalized_path(journal_path)?;

    require_same_parent(
        &canonical_path,
        [&staged_path, &previous_path, &journal_path],
    )?;
    require_distinct_paths([&canonical_path, &staged_path, &previous_path, &journal_path])?;
    // This binds the caller's canonical argument to the inode already held by
    // the guard before any path-based inspection or namespace mutation.
    guard.validate_current_database_identity_at(&canonical_path)?;
    require_regular_file(&canonical_path, "canonical database")?;
    require_regular_file(&staged_path, "staged database")?;
    require_absent(&previous_path, "previous database destination")?;
    require_absent(&journal_path, "replacement journal")?;
    reject_sqlite_sidecars(&canonical_path)?;
    reject_sqlite_sidecars(&staged_path)?;
    validate_staged_binding(&staged_path, &options)?;

    guard.validate_database_identities()?;
    let canonical_identity = file_identity(&canonical_path)?;
    let staged_identity = file_identity(&staged_path)?;
    if canonical_identity.device != staged_identity.device {
        return Err(KanbanError::InvalidInput(
            "canonical and staged databases must be on one device".to_owned(),
        ));
    }
    let staged_sha256 = sha256_file(&staged_path)?;
    let mut journal = DatabaseReplaceJournal {
        format_version: JOURNAL_FORMAT_VERSION,
        canonical_path,
        staged_path,
        previous_path,
        journal_path,
        canonical_identity,
        staged_identity,
        staged_sha256,
        previous_identity: None,
        placeholder_previous: guard.current_database_was_created_for_replace()?,
        phase: JournalPhase::Prepared,
    };
    write_new_journal(&journal)?;
    hook(PublishFailpoint::JournalInitial)?;
    continue_publish(guard, &mut journal, &options, hook)
}

fn reconcile_physical_phase(
    guard: &mut DatabaseReplaceGuard,
    journal: &mut DatabaseReplaceJournal,
) -> Result<()> {
    if matches!(
        &journal.phase,
        JournalPhase::Completed | JournalPhase::Rebound
    ) {
        return Ok(());
    }

    let previous_identity = match fs::symlink_metadata(&journal.previous_path) {
        Ok(metadata) if metadata.is_file() => Some(file_identity(&journal.previous_path)?),
        Ok(_) => {
            return Err(KanbanError::Conflict(
                "replacement previous path is not a regular file".to_owned(),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(storage(error)),
    };
    let canonical_identity = match fs::symlink_metadata(&journal.canonical_path) {
        Ok(metadata) if metadata.is_file() => Some(file_identity(&journal.canonical_path)?),
        Ok(_) => {
            return Err(KanbanError::Conflict(
                "replacement canonical path is not a regular file".to_owned(),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(storage(error)),
    };
    let staged_identity = match fs::symlink_metadata(&journal.staged_path) {
        Ok(metadata) if metadata.is_file() => Some(file_identity(&journal.staged_path)?),
        Ok(_) => {
            return Err(KanbanError::Conflict(
                "replacement staged path is not a regular file".to_owned(),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(storage(error)),
    };

    // A missing-target crash before the first fence leaves only a Prepared
    // journal. Drop removed the original placeholder; the restart guard
    // safely created a fresh placeholder inode. Because the guard proves both
    // that it created this path and that it still holds the inode, rebinding
    // the journal's placeholder identity is safe and does not admit an
    // existing-target substitution.
    if matches!(
        &journal.phase,
        JournalPhase::Prepared | JournalPhase::StagedFenced
    ) && journal.placeholder_previous
        && previous_identity.is_none()
        && guard.current_database_was_created_for_replace()?
    {
        guard.validate_current_database_identity_at(&journal.canonical_path)?;
        let replacement_placeholder = canonical_identity.as_ref().ok_or_else(|| {
            KanbanError::Conflict(
                "missing-target replacement journal has no restart placeholder".to_owned(),
            )
        })?;
        if !same_file_identity(&journal.canonical_identity, replacement_placeholder) {
            journal.canonical_identity = replacement_placeholder.clone();
            write_journal(journal)?;
        }
    }

    // A crash after canonical→previous but before its journal write leaves
    // StagedFenced (or even Prepared) on disk. The old inode and staged inode
    // make that physical state unambiguous; advance the journal before
    // continuing, without attempting a rollback.
    if matches!(
        &journal.phase,
        JournalPhase::Prepared | JournalPhase::StagedFenced
    ) && previous_identity
        .as_ref()
        .is_some_and(|identity| same_file_identity(identity, &journal.canonical_identity))
        && staged_identity
            .as_ref()
            .is_some_and(|identity| same_file_identity(identity, &journal.staged_identity))
        && canonical_identity
            .as_ref()
            .is_none_or(|identity| !same_file_identity(identity, &journal.canonical_identity))
    {
        if canonical_identity.is_some()
            && (!guard.current_database_was_created_for_replace()?
                || guard
                    .validate_current_database_identity_at(&journal.canonical_path)
                    .is_err())
        {
            return Err(KanbanError::Conflict(
                "replacement canonical path contains an unknown inode while previous evidence exists"
                    .to_owned(),
            ));
        }
        if guard
            .validate_current_database_identity_at(&journal.previous_path)
            .is_err()
        {
            if guard.current_database_was_created_for_replace()? {
                guard.rebind_current_authority_from_previous(&journal.previous_path)?;
            } else {
                guard.rebind_current_database_after_previous_publish(&journal.previous_path)?;
            }
        }
        journal.previous_identity = previous_identity;
        journal.phase = JournalPhase::PreviousPublished;
        write_journal(journal)?;
    }

    // A crash after staged→canonical but before its journal write leaves
    // PreviousPublished while the new canonical inode is already complete.
    if matches!(&journal.phase, JournalPhase::PreviousPublished)
        && canonical_identity
            .as_ref()
            .is_some_and(|identity| same_file_identity(identity, &journal.staged_identity))
        && staged_identity.is_none()
    {
        journal.phase = JournalPhase::CanonicalPublished;
        write_journal(journal)?;
    }
    Ok(())
}

fn continue_publish<Hook>(
    guard: &mut DatabaseReplaceGuard,
    journal: &mut DatabaseReplaceJournal,
    options: &DatabaseReplaceOptions,
    mut hook: Hook,
) -> Result<DatabaseReplaceReport>
where
    Hook: FnMut(PublishFailpoint) -> Result<()>,
{
    validate_journal_paths(journal, &journal.journal_path)?;
    validate_journal_format_version(journal)?;

    if matches!(&journal.phase, JournalPhase::Completed) {
        guard.validate_database_identities()?;
        validate_completed_journal(journal)?;
        validate_completed_canonical_location(journal)?;
        cleanup_placeholder_previous(guard, journal)?;
        return report_for(journal);
    }

    if matches!(&journal.phase, JournalPhase::Prepared) {
        guard.validate_current_database_identity_at(&journal.canonical_path)?;
        guard.fence_staged_database_for_replace(&journal.staged_path)?;
        journal.phase = JournalPhase::StagedFenced;
        write_journal(journal)?;
        hook(PublishFailpoint::StagedFenced)?;
    } else if matches!(&journal.phase, JournalPhase::StagedFenced) {
        // A restart has no staged authority; a retry in the same process may
        // still hold it. The guard seam rejects duplicate fencing.
        if !guard.has_staged_database_authority() {
            guard.fence_staged_database_for_replace(&journal.staged_path)?;
        }
    } else if matches!(&journal.phase, JournalPhase::PreviousPublished)
        && !guard.has_staged_database_authority()
    {
        guard.fence_staged_database_for_replace(&journal.staged_path)?;
    }

    match &journal.phase {
        JournalPhase::StagedFenced => {
            validate_fenced_staged(guard, journal, options)?;
            guard.validate_current_database_identity_at(&journal.canonical_path)?;
            require_regular_file(&journal.canonical_path, "canonical database")?;
            if !same_file_identity(
                &journal.canonical_identity,
                &file_identity(&journal.canonical_path)?,
            ) {
                return Err(KanbanError::Conflict(
                    "canonical database identity changed after journal preparation".to_owned(),
                ));
            }
            reject_sqlite_sidecars(&journal.canonical_path)?;
            durable_move_file_no_replace(&journal.canonical_path, &journal.previous_path)
                .map_err(storage)?;
            journal.previous_identity = Some(file_identity(&journal.previous_path)?);
            if !same_file_identity(
                &journal.canonical_identity,
                journal.previous_identity.as_ref().expect("just assigned"),
            ) {
                return Err(KanbanError::Conflict(
                    "previous database identity does not match the anchored current inode"
                        .to_owned(),
                ));
            }
            journal.phase = JournalPhase::PreviousPublished;
            write_journal(journal)?;
            hook(PublishFailpoint::PreviousAnchored)?;
            // The old canonical path is now absent. Rebind this held inode
            // before moving the staged inode into the canonical namespace.
            guard.rebind_current_database_after_previous_publish(&journal.previous_path)?;
        }
        JournalPhase::PreviousPublished
        | JournalPhase::CanonicalPublished
        | JournalPhase::Rebound => {}
        JournalPhase::Prepared | JournalPhase::Completed => unreachable!(),
    }

    if matches!(&journal.phase, JournalPhase::PreviousPublished) {
        require_regular_file(&journal.previous_path, "previous database")?;
        if let Some(previous_identity) = &journal.previous_identity
            && !same_file_identity(previous_identity, &file_identity(&journal.previous_path)?)
        {
            return Err(KanbanError::Conflict(
                "previous database identity changed during replacement recovery".to_owned(),
            ));
        }
        if guard
            .validate_current_database_identity_at(&journal.previous_path)
            .is_err()
        {
            guard.rebind_current_authority_from_previous(&journal.previous_path)?;
        }
        require_absent(&journal.canonical_path, "canonical database destination")?;
        validate_fenced_staged(guard, journal, options)?;
        hook(PublishFailpoint::StagedDurable)?;
        durable_move_file_no_replace(&journal.staged_path, &journal.canonical_path)
            .map_err(storage)?;
        journal.phase = JournalPhase::CanonicalPublished;
        write_journal(journal)?;
        hook(PublishFailpoint::CanonicalPublished)?;
    }

    if matches!(&journal.phase, JournalPhase::CanonicalPublished) {
        require_regular_file(&journal.canonical_path, "canonical database")?;
        let canonical_identity = file_identity(&journal.canonical_path)?;
        if !same_file_identity(&journal.staged_identity, &canonical_identity)
            || journal.staged_path.exists()
        {
            hook(PublishFailpoint::PostPublishIdentity)?;
            return Err(KanbanError::Conflict(
                "canonical database identity does not match the fenced staged inode".to_owned(),
            ));
        }
        reject_sqlite_sidecars(&journal.canonical_path)?;
        if guard.has_staged_database_authority() {
            guard
                .rebind_after_namespace_publish(&journal.previous_path, &journal.canonical_path)?;
        } else {
            // A process restart released the staged authority. The fresh
            // current authority already binds the newly published canonical
            // inode, so only the journal transition remains.
            guard.validate_current_database_identity_at(&journal.canonical_path)?;
        }
        guard.validate_database_identities()?;
        journal.phase = JournalPhase::Rebound;
        write_journal(journal)?;
        hook(PublishFailpoint::Rebound)?;
    }

    if matches!(&journal.phase, JournalPhase::Rebound) {
        validate_rebound_journal(journal)?;
        guard.validate_database_identities()?;
        hook(PublishFailpoint::ParentFsync)?;
        durable_sync_directory(parent_directory(&journal.canonical_path)?).map_err(storage)?;
        journal.phase = JournalPhase::Completed;
        write_journal(journal)?;
        hook(PublishFailpoint::JournalCompleted)?;
    }
    if matches!(&journal.phase, JournalPhase::Completed) {
        cleanup_placeholder_previous(guard, journal)?;
    }
    report_for(journal)
}
