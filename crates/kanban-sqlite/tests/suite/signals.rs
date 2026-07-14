use kanban_sqlite::api::{
    SignalLifecycle, SignalListOptions, SignalRecordInput, SignalReviewInput, SignalStatus,
    record_signal, review_signals, update_signal_status,
};

use crate::common::{TempDb, init_database};

struct SignalFixture {
    open_id: String,
    confirmed_id: String,
    resolved_id: String,
}

fn signal_input(title: &str) -> SignalRecordInput {
    SignalRecordInput {
        kind: "test".to_owned(),
        title: title.to_owned(),
        summary: format!("{title} summary"),
        severity: None,
        task_ref: None,
        task_id: None,
        run_id: None,
        comment_id: None,
        actor: Some("tester".to_owned()),
        agent_type: None,
        dedupe_key: None,
        source: Some("signals-contract-test".to_owned()),
        evidence: None,
        comment: None,
    }
}

fn seeded_signal_statuses(name: &str) -> anyhow::Result<(TempDb, SignalFixture)> {
    let temp = TempDb::new(name)?;
    init_database(&temp.path, "tester")?;
    let open_id = record_signal(&temp.path, "default", "tester", signal_input("open signal"))?
        .signal
        .id;
    let confirmed_id = record_signal(
        &temp.path,
        "default",
        "tester",
        signal_input("confirmed signal"),
    )?
    .signal
    .id;
    let resolved_id = record_signal(
        &temp.path,
        "default",
        "tester",
        signal_input("resolved signal"),
    )?
    .signal
    .id;

    update_signal_status(
        &temp.path,
        "default",
        "tester",
        SignalReviewInput {
            signal_ids: vec![confirmed_id.clone()],
            lifecycle: SignalLifecycle::Confirm,
            replacement_signal_id: None,
            reason: "confirmed for review".to_owned(),
        },
    )?;
    update_signal_status(
        &temp.path,
        "default",
        "tester",
        SignalReviewInput {
            signal_ids: vec![resolved_id.clone()],
            lifecycle: SignalLifecycle::Resolve,
            replacement_signal_id: None,
            reason: "resolved history".to_owned(),
        },
    )?;

    Ok((
        temp,
        SignalFixture {
            open_id,
            confirmed_id,
            resolved_id,
        },
    ))
}

fn signal_ids(signals: &[kanban_sqlite::api::SignalRecord]) -> Vec<String> {
    let mut ids = signals
        .iter()
        .map(|signal| signal.id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

#[test]
fn review_signals_defaults_to_reviewable_statuses() -> anyhow::Result<()> {
    let (temp, fixture) = seeded_signal_statuses("review_signals_defaults_to_reviewable")?;

    let signals = review_signals(&temp.path, "default", SignalListOptions::default())?;

    let mut expected = vec![fixture.open_id, fixture.confirmed_id];
    expected.sort();
    assert_eq!(signal_ids(&signals), expected);
    Ok(())
}

#[test]
fn review_signals_include_all_returns_reviewable_and_historical_statuses() -> anyhow::Result<()> {
    let (temp, fixture) = seeded_signal_statuses("review_signals_include_all")?;

    let signals = review_signals(
        &temp.path,
        "default",
        SignalListOptions {
            include_all: true,
            ..SignalListOptions::default()
        },
    )?;

    let mut expected = vec![fixture.open_id, fixture.confirmed_id, fixture.resolved_id];
    expected.sort();
    assert_eq!(signal_ids(&signals), expected);
    Ok(())
}

#[test]
fn review_signals_explicit_statuses_take_priority_over_include_all() -> anyhow::Result<()> {
    let (temp, fixture) = seeded_signal_statuses("review_signals_explicit_status")?;

    let signals = review_signals(
        &temp.path,
        "default",
        SignalListOptions {
            statuses: vec![SignalStatus::Resolved],
            include_all: true,
            ..SignalListOptions::default()
        },
    )?;

    assert_eq!(signal_ids(&signals), vec![fixture.resolved_id]);
    Ok(())
}
