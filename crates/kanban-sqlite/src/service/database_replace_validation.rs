fn validate_fenced_staged(
    guard: &mut DatabaseReplaceGuard,
    journal: &DatabaseReplaceJournal,
    options: &DatabaseReplaceOptions,
) -> Result<()> {
    reject_sqlite_sidecars(&journal.staged_path)?;
    let identity = file_identity(&journal.staged_path)?;
    if !same_file_identity(&journal.staged_identity, &identity) {
        return Err(KanbanError::Conflict(
            "staged database identity changed after lifecycle fencing".to_owned(),
        ));
    }
    let hash = sha256_file(&journal.staged_path)?;
    if hash != journal.staged_sha256 {
        return Err(KanbanError::Conflict(
            "staged database contents changed after lifecycle fencing".to_owned(),
        ));
    }
    // Reopen through the exclusive authority seam. This re-reads projection
    // singleton/protocol/schema while the staged inode is fenced and avoids a
    // shared lifecycle lock gap.
    guard.inspect_staged_database(|connection| {
        validate_staged_binding_connection(connection, options)
    })
}

fn validate_staged_binding(path: &Path, options: &DatabaseReplaceOptions) -> Result<()> {
    if let Some(expected) = &options.expected_sha256 {
        let actual = sha256_file(path)?;
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(KanbanError::Conflict(format!(
                "staged database SHA-256 mismatch: expected {expected}, got {actual}"
            )));
        }
    }
    let conn = crate::db::connect_existing_quiescent_read_only(path)?;
    validate_staged_binding_connection(&conn, options)
}

fn validate_staged_binding_connection(
    conn: &Connection,
    options: &DatabaseReplaceOptions,
) -> Result<()> {
    let (database_instance_id, protocol_version): (String, i64) = conn
        .query_row(
            "SELECT database_instance_id,protocol_version
             FROM projection_database WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    if let Some(expected) = &options.expected_database_instance_id
        && &database_instance_id != expected
    {
        return Err(KanbanError::Conflict(format!(
            "staged database instance id mismatch: expected {expected}, got {database_instance_id}"
        )));
    }
    if let Some(expected) = options.expected_protocol_version
        && protocol_version != expected
    {
        return Err(KanbanError::Conflict(format!(
            "staged database protocol version mismatch: expected {expected}, got {protocol_version}"
        )));
    }
    let (schema_min, schema_max, schema_count): (Option<i64>, Option<i64>, i64) = conn
        .query_row(
            "SELECT MIN(schema_version),MAX(schema_version),COUNT(*)
             FROM projection_store_state",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|error| KanbanError::Storage(error.to_string()))?;
    if schema_count == 0 || schema_min != schema_max {
        return Err(KanbanError::Conflict(
            "staged database projection schema binding is not singular".to_owned(),
        ));
    }
    if let Some(expected) = options.expected_schema_version
        && schema_min != Some(expected)
    {
        return Err(KanbanError::Conflict(format!(
            "staged database projection schema version mismatch: expected {expected}, got {:?}",
            schema_min
        )));
    }
    Ok(())
}

fn validate_completed_journal(journal: &DatabaseReplaceJournal) -> Result<()> {
    require_regular_file(&journal.canonical_path, "canonical database")?;
    require_absent(&journal.staged_path, "staged database")?;
    if !same_file_identity(
        &journal.staged_identity,
        &file_identity(&journal.canonical_path)?,
    ) {
        return Err(KanbanError::Conflict(
            "completed replacement canonical identity does not match staged identity".to_owned(),
        ));
    }
    if journal.placeholder_previous {
        let previous_identity = journal.previous_identity.as_ref().ok_or_else(|| {
            KanbanError::Conflict(
                "completed replacement journal is missing placeholder previous identity".to_owned(),
            )
        })?;
        match fs::symlink_metadata(&journal.previous_path) {
            Ok(metadata) if metadata.is_file() => {
                let actual = identity_from_metadata(&metadata);
                if !same_file_identity(previous_identity, &actual) {
                    return Err(KanbanError::Conflict(
                        "completed replacement placeholder previous identity no longer matches retained evidence"
                            .to_owned(),
                    ));
                }
            }
            Ok(_) => {
                return Err(KanbanError::Conflict(
                    "completed replacement placeholder previous path is not a regular file"
                        .to_owned(),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(storage(error)),
        }
        return Ok(());
    }
    require_regular_file(&journal.previous_path, "previous database")?;
    let previous_identity = journal.previous_identity.as_ref().ok_or_else(|| {
        KanbanError::Conflict(
            "completed replacement journal is missing previous database identity".to_owned(),
        )
    })?;
    if !same_file_identity(previous_identity, &file_identity(&journal.previous_path)?) {
        return Err(KanbanError::Conflict(
            "completed replacement previous identity no longer matches retained evidence"
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_rebound_journal(journal: &DatabaseReplaceJournal) -> Result<()> {
    validate_completed_journal(journal)?;
    require_absent(&journal.staged_path, "staged database")?;
    reject_sqlite_sidecars(&journal.canonical_path)
}

fn cleanup_placeholder_previous(
    guard: &mut DatabaseReplaceGuard,
    journal: &DatabaseReplaceJournal,
) -> Result<()> {
    if !journal.placeholder_previous {
        return Ok(());
    }
    let previous_identity = journal.previous_identity.as_ref().ok_or_else(|| {
        KanbanError::Conflict(
            "replacement journal is missing placeholder previous identity".to_owned(),
        )
    })?;
    // In the uninterrupted path the current authority was rebound to the
    // previous inode, so keep the existing identity-checked drop behavior.
    // After an abrupt process loss a fresh guard binds the canonical inode;
    // reacquire the retained previous inode without following symlinks and
    // durably remove it only when the journal identity still matches.
    if guard
        .validate_current_database_identity_at(&journal.previous_path)
        .is_ok()
    {
        guard.mark_current_database_for_drop_if_identity_at(&journal.previous_path)?;
        return Ok(());
    }
    guard.remove_placeholder_previous_if_identity(&journal.previous_path, previous_identity)
}
