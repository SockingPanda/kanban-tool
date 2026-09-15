use super::*;
fn card(id: &str, title: &str) -> Card {
    Card {
        id: id.into(),
        title: title.into(),
        status: Status::Todo,
        priority: 1,
        position: 0,
        seq: 1,
        lock_version: 0,
    }
}
#[tokio::test]
async fn initial_snapshot_is_not_visible_before_publish() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    assert!(matches!(hub.snapshot().await, Err(LiveError::Unavailable)));
    hub.publish(vec![card("t_a", "A")]).await.unwrap();
    let snapshot = hub.snapshot().await.unwrap();
    assert_eq!(snapshot.cursor.revision, 1);
    assert_eq!(snapshot.cards.len(), 1);
}
#[tokio::test]
async fn empty_board_still_has_a_committed_revision() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    hub.publish(vec![]).await.unwrap();
    assert_eq!(hub.snapshot().await.unwrap().cursor.revision, 1);
}
#[tokio::test]
async fn delta_upserts_and_removes_are_one_revision() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    hub.publish(vec![card("t_a", "A"), card("t_b", "B")])
        .await
        .unwrap();
    let cursor = hub.snapshot().await.unwrap().cursor;
    hub.publish(vec![card("t_a", "new"), card("t_c", "C")])
        .await
        .unwrap();
    let ReadNext::Delta(delta) = hub.next(Some(&cursor)).await.unwrap() else {
        panic!("expected delta")
    };
    assert_eq!((delta.base_revision, delta.revision), (1, 2));
    assert_eq!(delta.upserts.len(), 2);
    assert_eq!(delta.removed, vec!["t_b"]);
}
#[tokio::test]
async fn identical_projection_does_not_increment_revision() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    hub.publish(vec![card("t_a", "A")]).await.unwrap();
    assert!(!hub.publish(vec![card("t_a", "A")]).await.unwrap());
    assert_eq!(hub.snapshot().await.unwrap().cursor.revision, 1);
}
#[tokio::test]
async fn old_snapshot_survives_new_publish_unchanged() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    hub.publish(vec![card("t_a", "A")]).await.unwrap();
    let old = hub.snapshot().await.unwrap();
    hub.publish(vec![card("t_a", "B")]).await.unwrap();
    assert_eq!(old.cards["t_a"].title, "A");
    assert_eq!(old.cursor.revision, 1);
}
#[tokio::test]
async fn expired_history_resets_instead_of_skipping() {
    let limits = Limits {
        history_batches: 1,
        ..Limits::default()
    };
    let hub = Hub::new("b_x", limits).unwrap();
    hub.publish(vec![card("t_a", "A")]).await.unwrap();
    let cursor = hub.snapshot().await.unwrap().cursor;
    hub.publish(vec![card("t_a", "B")]).await.unwrap();
    hub.publish(vec![card("t_a", "C")]).await.unwrap();
    assert!(matches!(
        hub.next(Some(&cursor)).await.unwrap(),
        ReadNext::Reset {
            reason: "history_expired",
            ..
        }
    ));
}
#[tokio::test]
async fn future_cursor_is_reset() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    hub.publish(vec![]).await.unwrap();
    let mut cursor = hub.snapshot().await.unwrap().cursor;
    cursor.revision = 9;
    assert!(matches!(
        hub.next(Some(&cursor)).await.unwrap(),
        ReadNext::Reset {
            reason: "future_cursor",
            ..
        }
    ));
}
#[tokio::test]
async fn epoch_and_scope_are_part_of_identity() {
    let a = Hub::new("b_x", Limits::default()).unwrap();
    let b = Hub::new("b_x", Limits::default()).unwrap();
    a.publish(vec![]).await.unwrap();
    b.publish(vec![]).await.unwrap();
    let cursor = a.snapshot().await.unwrap().cursor;
    assert!(matches!(
        b.next(Some(&cursor)).await.unwrap(),
        ReadNext::Reset {
            reason: "identity_changed",
            ..
        }
    ));
    let mut other = b.snapshot().await.unwrap().cursor;
    other.scope = "board:b_y:cards:v1".into();
    assert!(matches!(
        b.next(Some(&other)).await.unwrap(),
        ReadNext::Reset {
            reason: "identity_changed",
            ..
        }
    ));
}
#[tokio::test]
async fn duplicate_task_is_rejected_without_mutating_projection() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    hub.publish(vec![]).await.unwrap();
    assert!(
        hub.publish(vec![card("t_a", "A"), card("t_a", "B")])
            .await
            .is_err()
    );
    assert!(hub.snapshot().await.unwrap().cards.is_empty());
}
#[tokio::test]
async fn a_large_delta_uses_snapshot_reset() {
    let hub = Hub::new(
        "b_x",
        Limits {
            delta_bytes: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    hub.publish(vec![]).await.unwrap();
    let cursor = hub.snapshot().await.unwrap().cursor;
    hub.publish(vec![card("t_a", "A")]).await.unwrap();
    assert!(matches!(
        hub.next(Some(&cursor)).await.unwrap(),
        ReadNext::Reset { .. }
    ));
}
#[tokio::test]
async fn source_failure_is_not_a_healthy_stale_snapshot() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    hub.publish(vec![]).await.unwrap();
    let cursor = hub.snapshot().await.unwrap().cursor;
    hub.unavailable().await;
    assert!(matches!(
        hub.next(Some(&cursor)).await,
        Err(LiveError::Unavailable)
    ));
    hub.publish(vec![]).await.unwrap();
    assert!(matches!(
        hub.next(Some(&cursor)).await.unwrap(),
        ReadNext::Idle
    ));
}
#[tokio::test]
async fn stopped_hub_cannot_be_revived() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    hub.stop().await;
    assert!(matches!(hub.publish(vec![]).await, Err(LiveError::Stopped)));
}
#[tokio::test]
async fn publish_wakeup_survives_wait_registration_race() {
    let hub = Hub::new("b_x", Limits::default()).unwrap();
    let mut receiver = hub.subscribe();
    hub.publish(vec![]).await.unwrap();
    assert!(receiver.has_changed().unwrap());
    receiver.changed().await.unwrap();
}
#[tokio::test]
async fn byte_budget_rejects_before_commit() {
    let hub = Hub::new(
        "b_x",
        Limits {
            snapshot_bytes: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(matches!(
        hub.publish(vec![card("t_a", "A")]).await,
        Err(LiveError::Budget(_))
    ));
    assert!(matches!(hub.snapshot().await, Err(LiveError::Unavailable)));
}
