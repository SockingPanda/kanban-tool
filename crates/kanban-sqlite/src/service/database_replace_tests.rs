use super::*;
use crate::init::init_database;

#[test]
fn publish_failure_after_previous_publish_retains_recovery_evidence() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
    init_database(&staged, "tester").unwrap();
    let before = fs::read(&canonical).unwrap();
    let mut guard = super::super::begin_database_replace(&canonical).unwrap();
    let error = publish_staged_database_with_hook(
        &mut guard,
        &canonical,
        &staged,
        &previous,
        &journal,
        DatabaseReplaceOptions::default(),
        |point| {
            if point == PublishFailpoint::PreviousAnchored {
                Err(KanbanError::Storage(
                    "injected previous publish failure".to_owned(),
                ))
            } else {
                Ok(())
            }
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("injected previous"));
    assert!(!canonical.exists());
    assert!(previous.exists());
    assert!(staged.exists());
    assert_eq!(fs::read(&previous).unwrap(), before);
    assert!(
        fs::read_to_string(journal)
            .unwrap()
            .contains("previous_published")
    );
    drop(guard);
}

#[test]
fn resume_after_previous_publish_recovery_completes_without_rollback() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
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
            if point == PublishFailpoint::PreviousAnchored {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    drop(guard);

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    let report = resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert_eq!(
        report.canonical_path,
        fs::canonicalize(&canonical).unwrap()
    );
    assert!(canonical.is_file());
    assert!(previous.is_file());
    assert!(!staged.exists());
    assert!(fs::read_to_string(journal).unwrap().contains("completed"));
    drop(restarted);
}

#[test]
fn resume_after_canonical_publish_rebinds_fresh_guard() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
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
            if point == PublishFailpoint::CanonicalPublished {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    drop(guard);

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert!(canonical.is_file());
    assert!(previous.is_file());
    assert!(!staged.exists());
    drop(restarted);
}

#[test]
fn resume_rejects_a_journal_bound_to_another_database_guard() {
    let tempdir = tempfile::tempdir().unwrap();
    let database_a = tempdir.path().join("a.db");
    let database_b = tempdir.path().join("b.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&database_a, "tester").unwrap();
    init_database(&database_b, "tester").unwrap();
    init_database(&staged, "tester").unwrap();

    let mut guard_a = super::super::begin_database_replace(&database_a).unwrap();
    let _ = publish_staged_database_with_hook(
        &mut guard_a,
        &database_a,
        &staged,
        &previous,
        &journal,
        DatabaseReplaceOptions::default(),
        |point| {
            if point == PublishFailpoint::PreviousAnchored {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    drop(guard_a);

    let mut wrong_guard = super::super::begin_database_replace(&database_b).unwrap();
    let error = resume_staged_database_replace(&mut wrong_guard, &journal).unwrap_err();
    assert!(error.to_string().contains("canonical") || error.to_string().contains("bound"));
    assert!(database_b.is_file());
    assert!(previous.is_file());
    drop(wrong_guard);
}

#[test]
fn resume_reconciles_canonical_gap_when_journal_lags_previous_publish() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
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
            if point == PublishFailpoint::PreviousAnchored {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    drop(guard);
    let mut lagging: DatabaseReplaceJournal =
        serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
    lagging.phase = JournalPhase::StagedFenced;
    fs::write(&journal, serde_json::to_vec_pretty(&lagging).unwrap()).unwrap();

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert!(canonical.is_file());
    assert!(previous.is_file());
    assert!(!staged.exists());
    drop(restarted);
}

#[test]
fn resume_reconciles_canonical_publish_when_journal_lags_staged_move() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
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
            if point == PublishFailpoint::CanonicalPublished {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    drop(guard);
    let mut lagging: DatabaseReplaceJournal =
        serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
    lagging.phase = JournalPhase::PreviousPublished;
    fs::write(&journal, serde_json::to_vec_pretty(&lagging).unwrap()).unwrap();

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert!(canonical.is_file());
    assert!(previous.is_file());
    assert!(!staged.exists());
    drop(restarted);
}

#[test]
fn staged_sidecar_created_after_fence_stops_before_previous_move() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
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
            if point == PublishFailpoint::StagedFenced {
                fs::write(format!("{}-wal", staged.display()), b"sidecar").map_err(storage)?;
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("sidecar"));
    assert!(canonical.is_file());
    assert!(!previous.exists());
    assert!(staged.is_file());
    drop(guard);
}

#[test]
fn resume_prepared_missing_target_rebinds_restart_placeholder() {
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
            if point == PublishFailpoint::JournalInitial {
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
    assert!(!previous.exists());

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert!(canonical.is_file());
    assert!(!staged.exists());
    drop(restarted);
    assert!(!previous.exists());
}

#[test]
fn resume_staged_fenced_missing_target_rebinds_restart_placeholder() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
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
            if point == PublishFailpoint::StagedFenced {
                Err(KanbanError::Storage("simulated process loss".to_owned()))
            } else {
                Ok(())
            }
        },
    );
    drop(guard);
    assert!(!canonical.exists());
    assert!(!previous.exists());

    let mut restarted = super::super::begin_database_replace(&canonical).unwrap();
    resume_staged_database_replace(&mut restarted, &journal).unwrap();
    assert!(canonical.is_file());
    assert!(!staged.exists());
    drop(restarted);
    assert!(!previous.exists());
}

#[test]
fn canonical_sidecar_created_before_move_stops_without_previous_publish() {
    let tempdir = tempfile::tempdir().unwrap();
    let canonical = tempdir.path().join("canonical.db");
    let staged = tempdir.path().join("staged.db");
    let previous = tempdir.path().join("previous.db");
    let journal = tempdir.path().join("replace.journal");
    init_database(&canonical, "tester").unwrap();
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
            if point == PublishFailpoint::StagedFenced {
                fs::write(format!("{}-wal", canonical.display()), b"sidecar").map_err(storage)?;
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("sidecar"));
    assert!(canonical.is_file());
    assert!(!previous.exists());
    drop(guard);
}
