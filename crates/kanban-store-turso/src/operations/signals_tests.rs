#[cfg(test)]
mod tests {
    use crate::test_support::*;
    use crate::{CreateSignalInput, ReviewSignalsInput, SignalLifecycleInput, StoreError};

    fn signal_input(
        id: &str,
        observation_id: &str,
        event_id: &str,
        title: &str,
    ) -> CreateSignalInput {
        CreateSignalInput {
            id: id.to_owned(),
            observation_id: observation_id.to_owned(),
            event_id: event_id.to_owned(),
            board: "default".to_owned(),
            kind: "failure".to_owned(),
            title: title.to_owned(),
            summary: "worker failed".to_owned(),
            severity: "error".to_owned(),
            task_ref: Some("default#1".to_owned()),
            task_id: None,
            run_id: None,
            comment_id: None,
            actor: "agent".to_owned(),
            agent_type: Some("executor".to_owned()),
            dedupe_key: Some("failure:one".to_owned()),
            source: Some("test".to_owned()),
            evidence_json: r#"{"exit_code":1}"#.to_owned(),
            comment_body: Some("Investigate this failure".to_owned()),
            created_at: 100,
        }
    }

    #[tokio::test]
    async fn record_signal_writes_ledger_backlink_and_events_atomically() {
        let (_directory, store, _path) = store("signal-record").await;
        store.initialize().await.expect("initialize");
        store
            .create_task("default", create_input("t_signal", None, "Signal task"))
            .await
            .expect("create task");

        let result = store
            .record_signal(signal_input(
                "sig_record",
                "obs_record",
                "e_signal_record",
                "Observed failure",
            ))
            .await
            .expect("record signal");
        assert_eq!(result.signal.id, "sig_record");
        assert_eq!(result.signal.status, "open");
        let backlink = result.backlink_comment.expect("signal backlink");
        assert_eq!(backlink.kind, "signal");
        assert!(backlink.metadata_json.contains("sig_record"));

        let connection = store.connection().await.expect("connection");
        let count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE board_id = 'b_default' AND kind IN ('task.comment.created', 'signal.recorded')",
                    (),
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event count row");
        assert_eq!(
            integer_value(count.get_value(0).expect("event count"), "event count")
                .expect("event count integer"),
            2
        );
    }

    #[tokio::test]
    async fn record_signal_replays_same_dedupe_payload_and_conflicts_on_change() {
        let (_directory, store, _path) = store("signal-idempotency").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_signal_replay", None, "Signal task"),
            )
            .await
            .expect("create task");
        let first_input = signal_input(
            "sig_first",
            "obs_first",
            "e_signal_first",
            "Observed failure",
        );
        let first = store
            .record_signal(first_input.clone())
            .await
            .expect("first signal");
        let replay = store
            .record_signal(CreateSignalInput {
                id: "sig_retry".to_owned(),
                observation_id: "obs_retry".to_owned(),
                event_id: "e_signal_retry".to_owned(),
                created_at: 900,
                ..first_input.clone()
            })
            .await
            .expect("dedupe replay");
        assert_eq!(replay.signal, first.signal);
        assert_eq!(replay.backlink_comment, first.backlink_comment);

        let conflict = store
            .record_signal(CreateSignalInput {
                title: "Changed payload".to_owned(),
                id: "sig_changed".to_owned(),
                observation_id: "obs_changed".to_owned(),
                event_id: "e_signal_changed".to_owned(),
                ..first_input
            })
            .await
            .expect_err("changed payload must conflict");
        assert!(matches!(
            conflict,
            StoreError::SignalIdempotencyConflict {
                board_id,
                key,
                existing_signal_id
            } if board_id == "b_default" && key == "failure:one" && existing_signal_id == "sig_first"
        ));
    }

    #[tokio::test]
    async fn review_signal_updates_backlink_and_rejects_invalid_transition() {
        let (_directory, store, _path) = store("signal-review").await;
        store.initialize().await.expect("initialize");
        store
            .create_task(
                "default",
                create_input("t_signal_review", None, "Signal task"),
            )
            .await
            .expect("create task");
        store
            .record_signal(signal_input(
                "sig_review",
                "obs_review",
                "e_signal_review",
                "Observed failure",
            ))
            .await
            .expect("record signal");

        let reviewed = store
            .review_signals(ReviewSignalsInput {
                board: Some("default".to_owned()),
                signal_ids: vec!["sig_review".to_owned()],
                lifecycle: SignalLifecycleInput::Confirm,
                replacement_signal_id: None,
                actor: "reviewer".to_owned(),
                reason: "confirmed".to_owned(),
                event_ids: vec!["e_signal_reviewed".to_owned()],
                now: 200,
            })
            .await
            .expect("confirm signal");
        assert_eq!(reviewed[0].status, "confirmed");

        let second = store
            .review_signals(ReviewSignalsInput {
                board: Some("default".to_owned()),
                signal_ids: vec!["sig_review".to_owned()],
                lifecycle: SignalLifecycleInput::Confirm,
                replacement_signal_id: None,
                actor: "reviewer".to_owned(),
                reason: "duplicate".to_owned(),
                event_ids: vec!["e_signal_reviewed_again".to_owned()],
                now: 300,
            })
            .await
            .expect_err("confirmed signal cannot be confirmed twice");
        assert!(matches!(second, StoreError::InvalidTransition(_)));
    }
}
