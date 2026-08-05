#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn update_task_persists_nullable_fields_metadata_null_and_event() {
        let (_directory, store, _path) = store("task-update-lifecycle").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_update_lifecycle", Some("update-lifecycle"), "Original"),
            )
            .await
            .expect("create task");

        let updated = store
            .update_task(
                &task.id,
                UpdateTaskInput {
                    expected_lock_version: 0,
                    actor: "editor".into(),
                    title: Some("Updated".into()),
                    description: Some(None),
                    assignee: None,
                    priority: Some(2),
                    scheduled_at: None,
                    due_at: Some(None),
                    max_retries: None,
                    metadata_json: Some("null".into()),
                    event_id: "e_task_updated".into(),
                    now: 200,
                },
            )
            .await
            .expect("update task");
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.description, None);
        assert_eq!(updated.due_at, None);
        assert_eq!(updated.priority, 2);
        assert_eq!(updated.metadata_json, "null");
        assert_eq!(updated.lock_version, 1);

        let connection = store.connection().await.expect("connection");
        let event = first_row(
            connection
                .query(
                    "SELECT kind, actor FROM task_events WHERE event_id = ?1",
                    ["e_task_updated"],
                )
                .await
                .expect("event query"),
        )
        .await
        .expect("event row");
        assert_eq!(
            text_value(event.get_value(0).expect("event kind"), "event.kind").expect("kind"),
            "task.updated"
        );
        assert_eq!(
            text_value(event.get_value(1).expect("event actor"), "event.actor").expect("actor"),
            "editor"
        );
    }

    #[tokio::test]
    async fn specify_task_recomputes_unplanned_task_to_todo() {
        let (_directory, store, _path) = store("task-specify-lifecycle").await;
        store.initialize().await.expect("initialize");
        let mut input = create_input("t_specify_lifecycle", Some("specify-lifecycle"), "Triage");
        input.status = "triage".into();
        input.description = None;
        input.scheduled_at = None;
        let task = store
            .create_task("default", input)
            .await
            .expect("create task");

        let specified = store
            .specify_task(
                &task.id,
                specify_input(
                    0,
                    "planner",
                    Some("Specified"),
                    None,
                    "e_task_specified",
                    200,
                ),
            )
            .await
            .expect("specify task");
        assert_eq!(specified.description.as_deref(), Some("Specified"));
        assert_eq!(specified.status, "todo");
        assert_eq!(specified.lock_version, 1);
        assert_eq!(specified.status_reason, None);
    }

    #[tokio::test]
    async fn unblock_task_recomputes_blocked_task_without_forcing_ready() {
        let (_directory, store, _path) = store("task-unblock-lifecycle").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_unblock_lifecycle", Some("unblock-lifecycle"), "Blocked"),
            )
            .await
            .expect("create task");
        let blocked = store
            .block_task(
                &task.id,
                block_input(0, "worker", None, false, "waiting", 200, "e_task_blocked"),
            )
            .await
            .expect("block task");
        assert_eq!(blocked.status, "blocked");
        let unblocked = store
            .unblock_task(
                &task.id,
                unblock_input(1, "worker", "e_task_unblocked", 300),
            )
            .await
            .expect("unblock task");
        assert_eq!(unblocked.status, "todo");
        assert_eq!(unblocked.status_reason, None);
        assert_eq!(unblocked.lock_version, 2);
    }

    #[tokio::test]
    async fn reopen_task_clears_completion_but_preserves_result_and_recomputes_children() {
        let (_directory, store, _path) = store("task-reopen-lifecycle").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_reopen_lifecycle", "reopen-lifecycle", "Done").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim-reopen",
                    "r_reopen",
                    "e_reopen_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim task");
        let completed = store
            .complete_task(
                &claimed.task.id,
                complete_input(
                    2,
                    "worker",
                    Some("claim-reopen"),
                    false,
                    Some("result"),
                    Some(r#"{"ok":true}"#),
                    350,
                    "e_reopen_done",
                ),
            )
            .await
            .expect("complete task");
        assert_eq!(completed.status, "done");
        assert_eq!(completed.completed_at, Some(350));
        let reopened = store
            .reopen_task(
                &task.id,
                reopen_input(3, "operator", "retry", "e_task_reopened", 400),
            )
            .await
            .expect("reopen task");
        assert_eq!(reopened.status, "ready");
        assert_eq!(reopened.completed_at, None);
        assert_eq!(reopened.result_summary.as_deref(), Some("result"));
        assert_eq!(reopened.result_json.as_deref(), Some(r#"{"ok":true}"#));
        assert_eq!(reopened.lock_version, 4);
    }

    #[tokio::test]
    async fn explicit_reclaim_expires_run_in_one_transaction_and_increments_retry() {
        let (_directory, store, _path) = store("task-reclaim-lifecycle").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_reclaim_lifecycle",
            "reclaim-lifecycle",
            "Running",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim-reclaim",
                    "r_reclaim",
                    "e_reclaim_claim",
                    "{}",
                    300,
                    10,
                ),
            )
            .await
            .expect("claim task");
        let reclaimed = store
            .reclaim_task(
                &task.id,
                explicit_reclaim_input(
                    2,
                    "operator",
                    false,
                    "ready",
                    1,
                    "expired",
                    "e_task_reclaimed",
                    400,
                ),
            )
            .await
            .expect("reclaim task");
        assert_eq!(reclaimed.status, "ready");
        assert_eq!(reclaimed.retry_count, 1);
        assert_eq!(reclaimed.current_run_id, None);
        assert_eq!(reclaimed.lock_version, 3);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status FROM task_runs WHERE id = ?1",
                    [claimed.run.id.as_str()],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status").expect("status"),
            "expired"
        );
    }

    #[tokio::test]
    async fn archive_task_sets_archived_state_and_event() {
        let (_directory, store, _path) = store("task-archive-lifecycle").await;
        store.initialize().await.expect("initialize");
        let task = store
            .create_task(
                "default",
                create_input("t_archive_lifecycle", Some("archive-lifecycle"), "Archive"),
            )
            .await
            .expect("create task");
        let archived = store
            .archive_task(
                &task.id,
                archive_input(0, "operator", false, "e_task_archived", 200),
            )
            .await
            .expect("archive task");
        assert_eq!(archived.status, "archived");
        assert_eq!(archived.archived_at, Some(200));
        assert_eq!(archived.lock_version, 1);
        let connection = store.connection().await.expect("connection");
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE event_id = ?1 AND kind = 'task.archived'",
                    ["e_task_archived"],
                )
                .await
                .expect("event count query"),
        )
        .await
        .expect("event count row");
        assert_eq!(
            integer_value(
                event_count.get_value(0).expect("event count"),
                "event.count"
            )
            .expect("count"),
            1
        );
    }
}
