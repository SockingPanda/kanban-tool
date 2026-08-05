#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn create_comment_writes_comment_and_event_atomically() {
        let (_directory, store, _path) = store("comment-create-success").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_comment_success", None, "Comment task"),
            )
            .await
            .expect("create task");

        let comment = store
            .create_comment(
                &task.id,
                comment_input(
                    "c_comment_success",
                    Some("comment-key"),
                    " operator ",
                    "user",
                    None,
                    " handoff note ",
                    "note",
                    " {} ",
                    "e_comment_success",
                    500,
                ),
            )
            .await
            .expect("create comment");
        assert_eq!(comment.id, "c_comment_success");
        assert_eq!(comment.board_id, "b_default");
        assert_eq!(comment.task_id, task.id);
        assert_eq!(comment.idempotency_key.as_deref(), Some("comment-key"));
        assert_eq!(comment.author, "operator");
        assert_eq!(comment.author_type, "user");
        assert_eq!(comment.agent_type, None);
        assert_eq!(comment.body, "handoff note");
        assert_eq!(comment.kind, "note");
        assert_eq!(comment.metadata_json, "{}");
        assert_eq!(comment.created_at, 500);

        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_comment_success"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event board"), "event.board_id")
                .expect("event board text"),
            "b_default"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event task"), "event.task_id")
                .expect("event task text"),
            task.id
        );
        assert_eq!(
            text_value(event.get_value(2).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.comment.created"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "operator"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"comment_id":"c_comment_success","kind":"note","author_type":"user","agent_type":null}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(5).expect("event created"),
                "event.created_at"
            )
            .expect("event created integer"),
            500
        );
        let comment_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.comment.created'",
                    [task.id.as_str()],
                )
                .await
                .expect("comment event count"),
        )
        .await
        .expect("comment event count row");
        assert_eq!(
            integer_value(
                comment_events.get_value(0).expect("comment event count"),
                "event.count",
            )
            .expect("comment event count integer"),
            1
        );
    }

    #[tokio::test]
    async fn create_comment_replays_same_payload_and_conflicts_on_changed_payload() {
        let (_directory, store, _path) = store("comment-create-idempotency").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_comment_idempotency", None, "Comment idempotency"),
            )
            .await
            .expect("create task");
        let first_input = comment_input(
            "c_comment_idempotency",
            Some("comment-replay"),
            "operator",
            "agent",
            Some("executor"),
            "same body",
            "decision",
            r#"{"options":[{"slug":"keep","title":"Keep","detail":"Keep the existing path"}],"selected":"keep","reason":"Test the idempotency path"}"#,
            "e_comment_idempotency_first",
            500,
        );
        let first = store
            .create_comment(&task.id, first_input.clone())
            .await
            .expect("first comment");
        let mut replay_input = first_input;
        replay_input.id = "c_comment_idempotency_retry".to_owned();
        replay_input.event_id = "e_comment_idempotency_retry".to_owned();
        replay_input.created_at = 900;
        let replay = store
            .create_comment(&task.id, replay_input)
            .await
            .expect("replay comment");
        assert_eq!(replay, first);

        let mut changed_input = comment_input(
            "c_comment_idempotency_changed",
            Some("comment-replay"),
            "operator",
            "agent",
            Some("executor"),
            "changed body",
            "decision",
            r#"{"options":[{"slug":"keep","title":"Keep","detail":"Keep the existing path"}],"selected":"keep","reason":"Test the idempotency path"}"#,
            "e_comment_idempotency_changed",
            1_000,
        );
        changed_input.body = "different body".to_owned();
        let conflict = store
            .create_comment(&task.id, changed_input)
            .await
            .expect_err("changed payload must conflict");
        assert!(matches!(
            conflict,
            StoreError::IdempotencyConflict {
                board_id,
                key,
                existing_task_id
            } if board_id == "b_default" && key == "comment-replay" && existing_task_id == task.id
        ));

        let connection = store.connection().await.expect("connection");
        let comments = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_comments WHERE board_id = ?1 AND task_id = ?2",
                    ("b_default", task.id.as_str()),
                )
                .await
                .expect("comment count"),
        )
        .await
        .expect("comment count row");
        assert_eq!(
            integer_value(
                comments.get_value(0).expect("comment count"),
                "comment.count"
            )
            .expect("comment count integer"),
            1
        );
        let events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.comment.created'",
                    [task.id.as_str()],
                )
                .await
                .expect("comment event count"),
        )
        .await
        .expect("comment event count row");
        assert_eq!(
            integer_value(events.get_value(0).expect("event count"), "event.count")
                .expect("event count integer"),
            1
        );
    }
    #[tokio::test]
    async fn create_comment_enforces_task_board_isolation_and_rolls_back_event_conflicts() {
        let (_directory, store, _path) = store("comment-create-isolation").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO boards(id, slug, name, created_at, updated_at) VALUES ('b_comment_other', 'comment-other', 'Comment other', 1, 1)",
                (),
            )
            .await
            .expect("insert other board");
        let other = store
            .create_task(
                "comment-other",
                create_input("t_comment_other", None, "Other comment task"),
            )
            .await
            .expect("create other task");
        let other_comment = store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_other",
                    None,
                    "operator",
                    "user",
                    None,
                    "other board",
                    "note",
                    "{}",
                    "e_comment_other",
                    500,
                ),
            )
            .await
            .expect("create other-board comment");
        assert_eq!(other_comment.board_id, "b_comment_other");
        let other_event = first_row(
            connection
                .query(
                    "SELECT board_id FROM task_events WHERE event_id = ?1",
                    ["e_comment_other"],
                )
                .await
                .expect("other event query"),
        )
        .await
        .expect("other event row");
        assert_eq!(
            text_value(
                other_event.get_value(0).expect("event board"),
                "event.board_id"
            )
            .expect("event board text"),
            "b_comment_other"
        );

        let invalid_decision = store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_invalid_decision",
                    None,
                    "operator",
                    "user",
                    None,
                    "invalid decision",
                    "decision",
                    "{}",
                    "e_comment_invalid_decision",
                    500,
                ),
            )
            .await
            .expect_err("decision metadata must be validated");
        assert!(matches!(
            invalid_decision,
            StoreError::InvalidInput(message) if message.contains("decision")
        ));

        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_comment_other', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_comment_conflict", other.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let event_error = store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_conflict",
                    None,
                    "operator",
                    "user",
                    None,
                    "must roll back",
                    "note",
                    "{}",
                    "e_comment_conflict",
                    500,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(event_error, StoreError::Turso(_)));
        let rolled_back_comments = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_comments WHERE id = ?1",
                    ["c_comment_conflict"],
                )
                .await
                .expect("rolled-back comment count"),
        )
        .await
        .expect("rolled-back comment count row");
        assert_eq!(
            integer_value(
                rolled_back_comments
                    .get_value(0)
                    .expect("rolled-back comment count"),
                "comment.count",
            )
            .expect("rolled-back comment count integer"),
            0
        );

        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 600 WHERE id = ?1",
                [other.id.as_str()],
            )
            .await
            .expect("archive task");
        let archived_error = store
            .create_comment(
                &other.id,
                comment_input(
                    "c_comment_archived",
                    None,
                    "operator",
                    "user",
                    None,
                    "archived task",
                    "note",
                    "{}",
                    "e_comment_archived",
                    700,
                ),
            )
            .await
            .expect_err("archived task must reject comments");
        assert!(matches!(
            archived_error,
            StoreError::InvalidTransition(message) if message.contains("archived")
        ));
    }
}
