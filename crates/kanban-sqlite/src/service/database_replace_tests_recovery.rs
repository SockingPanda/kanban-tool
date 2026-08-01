#[test]
fn restart_phase_matrix_converges_for_existing_and_missing_targets() {
    for missing_target in [false, true] {
        for failpoint in [
            PublishFailpoint::JournalInitial,
            PublishFailpoint::StagedFenced,
            PublishFailpoint::PreviousAnchored,
            PublishFailpoint::CanonicalPublished,
            PublishFailpoint::Rebound,
        ] {
            restart_phase_matrix_case(missing_target, failpoint);
        }
    }
}

#[test]
fn missing_target_failure_after_previous_publish_retains_placeholder_for_recovery() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&staged, "tester").unwrap();

    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    let error = publish_staged_database_with_hook(
        &mut guard,
        &canonical,
        &staged,
        &previous,
        &journal,
        DatabaseReplaceOptions::default(),
        |point| {
            if point == PublishFailpoint::StagedDurable {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("simulated process loss"));
    drop(guard);

    assert!(!canonical.exists());
    assert!(previous.is_file());
    assert!(staged.is_file());
    assert!(
        fs::read_to_string(&journal)
            .unwrap()
            .contains("previous_published")
    );

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert!(canonical.is_file());
    assert!(!staged.exists());
    assert!(!previous.exists());
    drop(restarted);
}

#[test]
fn completed_journal_rejects_replaced_previous_evidence() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
    init_database(&staged, "tester").unwrap();
    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    publish_staged_database(&mut guard, &canonical, &staged, &previous, &journal).unwrap();
    drop(guard);
    fs::remove_file(&previous).unwrap();
    fs::write(&previous, b"untrusted replacement evidence").unwrap();

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    let error = resume_staged_database_replace(&mut restarted, &journal).unwrap_err();
    assert!(error.to_string().contains("previous identity"));
    drop(restarted);
}

#[test]
fn completed_journal_rejects_retained_staged_evidence() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
    init_database(&staged, "tester").unwrap();
    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    publish_staged_database(&mut guard, &canonical, &staged, &previous, &journal).unwrap();
    drop(guard);
    fs::write(&staged, b"untrusted retained staged evidence").unwrap();

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    let error = resume_staged_database_replace(&mut restarted, &journal).unwrap_err();
    assert!(error.to_string().contains("staged database"));
    assert!(staged.is_file());
    assert!(journal.is_file());
    drop(restarted);
}

#[test]
fn completed_journal_allows_normal_canonical_mutation_before_recovery() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
    init_database(&staged, "tester").unwrap();

    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    publish_staged_database(&mut guard, &canonical, &staged, &previous, &journal).unwrap();
    drop(guard);

    // A completed journal is durable recovery evidence, not an immutable
    // content snapshot. Normal SQLite activity may change the canonical
    // inode's length/mtime before a later import quarantines the journal.
    let connection = crate::db::connect_file(&canonical).unwrap();
    connection
        .execute_batch("CREATE TABLE completed_journal_mutation(value TEXT);")
        .unwrap();
    drop(connection);

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    drop(restarted);
}

#[test]
fn read_journal_rejects_legacy_v1_format() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
    init_database(&staged, "tester").unwrap();

    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    publish_staged_database(&mut guard, &canonical, &staged, &previous, &journal).unwrap();
    drop(guard);

    let mut legacy: DatabaseReplaceJournal =
        serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
    legacy.format_version = 1;
    fs::write(&journal, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();

    let error = read_journal(&journal).unwrap_err();
    assert!(error.to_string().contains("unsupported database replacement journal format: 1"));
}

#[test]
fn resume_rejects_unknown_journal_format_before_recovery() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
    init_database(&staged, "tester").unwrap();

    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    publish_staged_database(&mut guard, &canonical, &staged, &previous, &journal).unwrap();
    drop(guard);

    let mut unknown: DatabaseReplaceJournal =
        serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
    unknown.format_version = 99;
    fs::write(&journal, serde_json::to_vec_pretty(&unknown).unwrap()).unwrap();

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    let error = resume_staged_database_replace(&mut restarted, &journal).unwrap_err();
    assert!(error.to_string().contains("unsupported database replacement journal format: 99"));
    assert!(journal.is_file());
    drop(restarted);
}

#[test]
fn legacy_file_identity_is_not_accepted_without_stable_native_id() {
    let legacy = FileIdentity {
        device: 7,
        inode: 11,
        file_id_high: None,
        length: 13,
        modified_ns: 17,
    };
    let stable = FileIdentity {
        device: 7,
        inode: 11,
        file_id_high: Some(0),
        length: 13,
        modified_ns: 17,
    };

    assert_eq!(same_file_identity(&legacy, &stable), cfg!(unix));
    assert_eq!(same_file_location_identity(&legacy, &stable), cfg!(unix));
}

#[cfg(any(unix, windows))]
#[test]
fn replacement_file_identity_uses_native_path_identity() {
    let tempdir = tempfile::tempdir().unwrap();
    let first = tempdir.path().join("first.db");
    let second = tempdir.path().join("second.db");
    init_database(&first, "tester").unwrap();
    init_database(&second, "tester").unwrap();

    let first_identity = file_identity(&first).unwrap();
    let second_identity = file_identity(&second).unwrap();
    assert!(first_identity.file_id_high.is_some());
    assert!(second_identity.file_id_high.is_some());
    assert!(!same_file_location_identity(&first_identity, &second_identity));
}

#[cfg(unix)]
#[test]
fn read_journal_rejects_symlink_entries_before_parsing() {
    use std::os::unix::fs::symlink;

    let tempdir = tempfile::tempdir().unwrap();
    let target = tempdir.path().join("journal-target");
    let journal = tempdir.path().join("replace.journal");
    fs::write(&target, br#"{"phase":"completed"}"#).unwrap();
    symlink(&target, &journal).unwrap();

    let error = read_journal(&journal).unwrap_err();
    assert!(error.to_string().contains("regular file"));
}

#[cfg(unix)]
#[test]
fn read_journal_rejects_regular_entry_replaced_after_open() {
    let tempdir = tempfile::tempdir().unwrap();
    let journal = tempdir.path().join("replace.journal");
    let replacement = tempdir.path().join("replacement.journal");
    fs::write(&journal, br#"{"phase":"completed"}"#).unwrap();
    fs::write(&replacement, br#"{"phase":"replaced"}"#).unwrap();

    let error = read_journal_with_hook(&journal, || {
        fs::remove_file(&journal).map_err(storage)?;
        fs::rename(&replacement, &journal).map_err(storage)?;
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("path changed"));
    assert_eq!(fs::read(&journal).unwrap(), br#"{"phase":"replaced"}"#);
}

#[cfg(unix)]
#[test]
fn restart_rebound_cleans_placeholder_retained_across_process_loss() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    let retained = tempdir.path().join("retained-placeholder.db");
    init_database(&staged, "tester").unwrap();

    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    let _ = publish_staged_database_with_hook(
        &mut guard,
        &canonical,
        &staged,
        &previous,
        &journal,
        DatabaseReplaceOptions::default(),
        |point| {
            if point == PublishFailpoint::Rebound {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    // Move the placeholder entry aside while the guard is held, then
    // restore it after Drop. The held inode has one link throughout, and
    // the missing path makes Drop skip cleanup just like process loss.
    fs::rename(&previous, &retained).unwrap();
    drop(guard);
    fs::rename(&retained, &previous).unwrap();
    assert!(previous.is_file());

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert!(canonical.is_file());
    assert!(!staged.exists());
    assert!(!previous.exists());
    drop(restarted);
}

#[cfg(unix)]
#[test]
fn restart_rebound_rejects_replaced_placeholder_evidence() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    let retained = tempdir.path().join("retained-placeholder.db");
    init_database(&staged, "tester").unwrap();

    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    let _ = publish_staged_database_with_hook(
        &mut guard,
        &canonical,
        &staged,
        &previous,
        &journal,
        DatabaseReplaceOptions::default(),
        |point| {
            if point == PublishFailpoint::Rebound {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    fs::rename(&previous, &retained).unwrap();
    drop(guard);
    fs::rename(&retained, &previous).unwrap();
    fs::write(&previous, b"untrusted placeholder replacement").unwrap();

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    let error = resume_staged_database_replace(&mut restarted, &journal).unwrap_err();
    assert!(error.to_string().contains("placeholder previous identity"));
    assert_eq!(
        fs::read(&previous).unwrap(),
        b"untrusted placeholder replacement"
    );
    drop(restarted);
}

fn restart_phase_matrix_case(missing_target: bool, failpoint: PublishFailpoint) {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    if !missing_target {
        init_database(&canonical, "tester").unwrap();
    }
    init_database(&staged, "tester").unwrap();

    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    let _ = publish_staged_database_with_hook(
        &mut guard,
        &canonical,
        &staged,
        &previous,
        &journal,
        DatabaseReplaceOptions::default(),
        |point| {
            if point == failpoint {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    drop(guard);

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert!(
        canonical.is_file(),
        "missing_target={missing_target:?} {failpoint:?}"
    );
    assert!(
        !staged.exists(),
        "missing_target={missing_target:?} {failpoint:?}"
    );
    drop(restarted);
    assert_eq!(
        previous.exists(),
        !missing_target,
        "missing_target={missing_target:?} {failpoint:?}"
    );
    let journal_text = fs::read_to_string(journal).unwrap();
    assert!(journal_text.contains("\"phase\": \"completed\""));
}
