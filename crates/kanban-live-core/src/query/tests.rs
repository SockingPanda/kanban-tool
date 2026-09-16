use super::*;

fn hub(limits: QueryLimits, budget: Arc<ByteBudget>) -> QueryHub {
    QueryHub::new("host-epoch".into(), "query-scope".into(), limits, budget)
}

fn snapshot(hub: &QueryHub) -> QuerySnapshot {
    match hub.next(None).unwrap() {
        QueryNext::Snapshot(value) => value,
        _ => panic!("snapshot"),
    }
}

#[test]
fn splice_exactly_reconstructs_every_short_binary_projection() {
    let values: Vec<Vec<u8>> = (0..64)
        .map(|n| {
            (0..6)
                .filter(|bit| n & (1 << bit) != 0)
                .map(|bit| bit as u8)
                .collect()
        })
        .collect();
    for old in &values {
        for next in &values {
            let (offset, delete, patch) = splice(old, next);
            let mut rebuilt = old.clone();
            rebuilt.splice(offset..offset + delete, patch.iter().copied());
            assert_eq!(&rebuilt, next);
        }
    }
}

#[test]
fn identical_projection_is_silent_and_delta_preserves_full_bytes() {
    let hub = hub(QueryLimits::default(), ByteBudget::new(1024));
    hub.publish(b"prefix-deep-field-suffix".to_vec(), [1; 32])
        .unwrap();
    let first = snapshot(&hub);
    let changes = hub.subscribe();
    assert!(
        !hub.publish(first.bytes.as_ref().as_ref().to_vec(), [1; 32])
            .unwrap()
    );
    assert!(!changes.has_changed().unwrap());
    hub.publish(b"prefix-complete-other-field-suffix".to_vec(), [2; 32])
        .unwrap();
    let QueryNext::Delta(delta) = hub.next(Some(&first.cursor)).unwrap() else {
        panic!("delta")
    };
    let mut rebuilt = first.bytes.as_ref().as_ref().to_vec();
    rebuilt.splice(
        delta.offset..delta.offset + delta.delete_length,
        delta.patch.as_ref().as_ref().iter().copied(),
    );
    assert_eq!(rebuilt, snapshot(&hub).bytes.as_ref().as_ref());
    assert_eq!(delta.result_size, rebuilt.len());
    assert_eq!(delta.result_sha256, [2; 32]);
}

#[test]
fn history_count_and_bytes_eviction_unknown_future_and_epoch_reset() {
    let limits = QueryLimits {
        history_batches: 2,
        history_bytes: 260,
        ..QueryLimits::default()
    };
    let hub = hub(limits, ByteBudget::new(1024));
    hub.publish(vec![0], [0; 32]).unwrap();
    let first = snapshot(&hub).cursor;
    hub.publish(vec![1], [0; 32]).unwrap();
    let second = snapshot(&hub).cursor;
    hub.publish(vec![2], [0; 32]).unwrap();
    hub.publish(vec![3], [0; 32]).unwrap();
    assert!(matches!(
        hub.next(Some(&first)).unwrap(),
        QueryNext::Snapshot(_)
    ));
    assert!(matches!(
        hub.next(Some(&second)).unwrap(),
        QueryNext::Delta(_)
    ));
    for cursor in [
        Resume {
            revision: 900,
            ..first.clone()
        },
        Resume {
            epoch: "old-host".into(),
            ..first.clone()
        },
        Resume {
            scope: "other-query".into(),
            ..first
        },
    ] {
        assert!(matches!(
            hub.next(Some(&cursor)).unwrap(),
            QueryNext::Snapshot(_)
        ));
    }
    assert!(matches!(
        hub.next(Some(&snapshot(&hub).cursor)).unwrap(),
        QueryNext::Idle
    ));
}

#[test]
fn shared_budget_counts_in_flight_frames_after_eviction_and_releases_on_cancel() {
    let budget = ByteBudget::new(40);
    let hub = hub(
        QueryLimits {
            history_bytes: 0,
            ..QueryLimits::default()
        },
        budget.clone(),
    );
    hub.publish(vec![1; 20], [0; 32]).unwrap();
    let in_flight = snapshot(&hub);
    hub.publish(vec![2; 20], [0; 32]).unwrap();
    assert_eq!(budget.used(), 40);
    assert!(matches!(
        hub.publish(vec![3; 20], [0; 32]),
        Err(LiveError::Budget(_))
    ));
    drop(in_flight);
    assert_eq!(budget.used(), 20);
    hub.publish(vec![3; 20], [0; 32]).unwrap();
    hub.stop();
    assert_eq!(budget.used(), 0);
}

#[test]
fn failed_publish_keeps_committed_projection_and_oversized_patch_resets() {
    let hub = hub(
        QueryLimits {
            snapshot_bytes: 8,
            history_bytes: 128,
            ..QueryLimits::default()
        },
        ByteBudget::new(100),
    );
    hub.publish(vec![1; 8], [0; 32]).unwrap();
    let first = snapshot(&hub);
    assert!(hub.publish(vec![2; 9], [0; 32]).is_err());
    assert_eq!(snapshot(&hub).cursor, first.cursor);
    hub.publish(vec![2; 8], [0; 32]).unwrap();
    assert!(matches!(
        hub.next(Some(&first.cursor)).unwrap(),
        QueryNext::Snapshot(_)
    ));
    hub.unavailable();
    assert!(matches!(hub.next(None), Err(LiveError::Unavailable)));
    hub.publish(vec![2; 8], [0; 32]).unwrap();
    assert_eq!(snapshot(&hub).cursor.revision, 2);
}
