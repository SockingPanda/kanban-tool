#[cfg(test)]
mod tests {
    use crate::test_support::*;

    #[tokio::test]
    async fn reclaim_expired_task_returns_ready_and_closes_run_atomically() {
        let (_directory, store, _path) = store("reclaim-expired-success").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_reclaim_expired_success",
            "reclaim-expired-success",
            "Reclaim expired",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_success",
                    "r_reclaim_success",
                    "e_reclaim_success_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim task");

        let expired = store
            .list_expired_claims(" default ", 500)
            .await
            .expect("list expired claims");
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, task.id);

        let reclaimed = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    claimed.task.lock_version,
                    "dispatcher",
                    "e_reclaim_success",
                    "ready",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim expired task")
            .expect("expired task must be reclaimed");
        assert_eq!(reclaimed.status, "ready");
        assert_eq!(reclaimed.retry_count, 1);
        assert_eq!(reclaimed.lock_version, claimed.task.lock_version + 1);
        assert_eq!(reclaimed.claim_token, None);
        assert_eq!(reclaimed.claim_owner, None);
        assert_eq!(reclaimed.claim_expires_at, None);
        assert_eq!(reclaimed.last_heartbeat_at, None);
        assert_eq!(reclaimed.current_run_id, None);

        let connection = store.connection().await.expect("connection");
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, error FROM task_runs WHERE id = ?1",
                    ["r_reclaim_success"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "expired"
        );
        assert_eq!(
            integer_value(run.get_value(1).expect("run finished"), "run.finished_at")
                .expect("run finished integer"),
            500
        );
        assert_eq!(
            text_value(run.get_value(2).expect("run error"), "run.error").expect("run error text"),
            "claim expired"
        );
        let event = first_row(
            connection
                .query(
                    "SELECT board_id, task_id, run_id, kind, actor, payload_json, created_at FROM task_events WHERE event_id = ?1",
                    ["e_reclaim_success"],
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
            text_value(event.get_value(2).expect("event run"), "event.run_id")
                .expect("event run text"),
            "r_reclaim_success"
        );
        assert_eq!(
            text_value(event.get_value(3).expect("event kind"), "event.kind")
                .expect("event kind text"),
            "task.reclaimed"
        );
        assert_eq!(
            text_value(event.get_value(4).expect("event actor"), "event.actor")
                .expect("event actor text"),
            "dispatcher"
        );
        assert_eq!(
            text_value(event.get_value(5).expect("event payload"), "event.payload")
                .expect("event payload text"),
            r#"{"retry_count":1,"max_retries":2,"to_status":"ready","reason":"claim expired"}"#
        );
        assert_eq!(
            integer_value(
                event.get_value(6).expect("event created"),
                "event.created_at"
            )
            .expect("event created integer"),
            500
        );
    }

    #[tokio::test]
    async fn reclaim_expired_task_recomputes_retry_target_from_canonical_facts() {
        let (_directory, store, _path) = store("reclaim-expired-targets").await;
        store.initialize().await.expect("initialize");
        let connection = store.connection().await.expect("connection");

        let maxed =
            ready_task_for_claim(&store, "t_reclaim_maxed", "reclaim-maxed", "Maxed retry").await;
        connection
            .execute(
                "UPDATE tasks SET max_retries = 1 WHERE id = ?1",
                [maxed.id.as_str()],
            )
            .await
            .expect("set max retries");
        let maxed_claim = store
            .claim_task(
                &maxed.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_maxed",
                    "r_reclaim_maxed",
                    "e_reclaim_maxed_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim maxed task");
        let maxed_reclaimed = store
            .reclaim_expired_task(
                &maxed.id,
                reclaim_input(
                    maxed_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_maxed",
                    "blocked",
                    1,
                    "max retries reached",
                    500,
                ),
            )
            .await
            .expect("reclaim maxed task")
            .expect("maxed task reclaimed");
        assert_eq!(maxed_reclaimed.status, "blocked");
        assert_eq!(
            maxed_reclaimed.status_reason.as_deref(),
            Some("max retries reached")
        );
        assert_eq!(maxed_reclaimed.retry_count, 1);

        let dependency = ready_task_for_claim(
            &store,
            "t_reclaim_dependency",
            "reclaim-dependency",
            "Dependency retry",
        )
        .await;
        let parent = store
            .create_task(
                "default",
                create_input(
                    "t_reclaim_dependency_parent",
                    Some("reclaim-dependency-parent"),
                    "Unfinished parent",
                ),
            )
            .await
            .expect("create dependency parent");
        let dependency_claim = store
            .claim_task(
                &dependency.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_dependency",
                    "r_reclaim_dependency",
                    "e_reclaim_dependency_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim dependency task");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 400)",
                (parent.id.as_str(), dependency.id.as_str()),
            )
            .await
            .expect("insert dependency");
        let dependency_reclaimed = store
            .reclaim_expired_task(
                &dependency.id,
                reclaim_input(
                    dependency_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_dependency",
                    "todo",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim dependency task")
            .expect("dependency task reclaimed");
        assert_eq!(dependency_reclaimed.status, "todo");

        let unplanned = ready_task_for_claim(
            &store,
            "t_reclaim_unplanned",
            "reclaim-unplanned",
            "Unplanned retry",
        )
        .await;
        let unplanned_claim = store
            .claim_task(
                &unplanned.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_unplanned",
                    "r_reclaim_unplanned",
                    "e_reclaim_unplanned_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim unplanned task");
        connection
            .execute(
                "UPDATE task_execution_plans SET state = 'planned' WHERE task_id = ?1",
                [unplanned.id.as_str()],
            )
            .await
            .expect("make plan non-ready");
        let unplanned_reclaimed = store
            .reclaim_expired_task(
                &unplanned.id,
                reclaim_input(
                    unplanned_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_unplanned",
                    "todo",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim unplanned task")
            .expect("unplanned task reclaimed");
        assert_eq!(unplanned_reclaimed.status, "todo");

        let scheduled = ready_task_for_claim(
            &store,
            "t_reclaim_scheduled",
            "reclaim-scheduled",
            "Scheduled retry",
        )
        .await;
        let scheduled_claim = store
            .claim_task(
                &scheduled.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_scheduled",
                    "r_reclaim_scheduled",
                    "e_reclaim_scheduled_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim scheduled task");
        connection
            .execute(
                "UPDATE tasks SET scheduled_at = 1_000 WHERE id = ?1",
                [scheduled.id.as_str()],
            )
            .await
            .expect("schedule task");
        let scheduled_reclaimed = store
            .reclaim_expired_task(
                &scheduled.id,
                reclaim_input(
                    scheduled_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_scheduled",
                    "scheduled",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim scheduled task")
            .expect("scheduled task reclaimed");
        assert_eq!(scheduled_reclaimed.status, "scheduled");

        let triage =
            ready_task_for_claim(&store, "t_reclaim_triage", "reclaim-triage", "Triage retry")
                .await;
        let triage_claim = store
            .claim_task(
                &triage.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_triage",
                    "r_reclaim_triage",
                    "e_reclaim_triage_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim triage task");
        connection
            .execute(
                "UPDATE tasks SET description = NULL WHERE id = ?1",
                [triage.id.as_str()],
            )
            .await
            .expect("remove task description");
        let triage_reclaimed = store
            .reclaim_expired_task(
                &triage.id,
                reclaim_input(
                    triage_claim.task.lock_version,
                    "dispatcher",
                    "e_reclaim_triage",
                    "triage",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect("reclaim triage task")
            .expect("triage task reclaimed");
        assert_eq!(triage_reclaimed.status, "triage");
    }

    #[tokio::test]
    async fn reclaim_expired_task_skips_fresh_heartbeat_and_lock_races_without_writes() {
        let (_directory, store, _path) = store("reclaim-expired-races").await;
        store.initialize().await.expect("initialize");
        let task =
            ready_task_for_claim(&store, "t_reclaim_races", "reclaim-races", "Reclaim races").await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_races",
                    "r_reclaim_races",
                    "e_reclaim_races_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim task");

        assert!(
            store
                .list_expired_claims("default", 350)
                .await
                .expect("list fresh claims")
                .is_empty()
        );
        let fresh = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    claimed.task.lock_version,
                    "dispatcher",
                    "e_reclaim_fresh",
                    "ready",
                    1,
                    "claim expired",
                    350,
                ),
            )
            .await
            .expect("fresh claim must be skipped");
        assert_eq!(fresh, None);

        let heartbeated = store
            .heartbeat_task(
                &task.id,
                heartbeat_input(
                    claimed.task.lock_version,
                    "worker",
                    "claim_reclaim_races",
                    "e_reclaim_races_heartbeat",
                    None,
                    400,
                    2_000,
                ),
            )
            .await
            .expect("heartbeat task");
        let heartbeat_race = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    heartbeated.lock_version,
                    "dispatcher",
                    "e_reclaim_heartbeat_race",
                    "ready",
                    1,
                    "claim expired",
                    1_000,
                ),
            )
            .await
            .expect("heartbeated claim must be skipped");
        assert_eq!(heartbeat_race, None);

        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET lock_version = lock_version + 1 WHERE id = ?1",
                [task.id.as_str()],
            )
            .await
            .expect("advance task lock");
        let lock_race = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    heartbeated.lock_version,
                    "dispatcher",
                    "e_reclaim_lock_race",
                    "ready",
                    1,
                    "claim expired",
                    2_500,
                ),
            )
            .await
            .expect("lock race must be skipped");
        assert_eq!(lock_race, None);
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get unchanged race task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.retry_count, 0);
        assert_eq!(unchanged.current_run_id.as_deref(), Some("r_reclaim_races"));
        let reclaimed_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.reclaimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("reclaimed event count"),
        )
        .await
        .expect("reclaimed event count row");
        assert_eq!(
            integer_value(
                reclaimed_events.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }

    #[tokio::test]
    async fn reclaim_expired_task_rejects_inconsistent_run_and_rolls_back_event_conflict() {
        let (_directory, store, _path) = store("reclaim-expired-rollback").await;
        store.initialize().await.expect("initialize");
        let task = ready_task_for_claim(
            &store,
            "t_reclaim_rollback",
            "reclaim-rollback",
            "Reclaim rollback",
        )
        .await;
        let claimed = store
            .claim_task(
                &task.id,
                claim_input(
                    1,
                    "worker",
                    "claim_reclaim_rollback",
                    "r_reclaim_rollback",
                    "e_reclaim_rollback_claim",
                    "{}",
                    300,
                    100,
                ),
            )
            .await
            .expect("claim task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE task_runs SET claim_owner = 'different-worker' WHERE id = ?1",
                ["r_reclaim_rollback"],
            )
            .await
            .expect("corrupt run owner");
        let inconsistent = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    claimed.task.lock_version,
                    "dispatcher",
                    "e_reclaim_inconsistent",
                    "ready",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect_err("inconsistent run must fail");
        assert!(matches!(
            inconsistent,
            StoreError::InvalidTransition(message) if message.contains("inconsistent")
        ));
        let unchanged = store
            .get_task_global(&task.id)
            .await
            .expect("get inconsistent task");
        assert_eq!(unchanged.status, "running");
        assert_eq!(unchanged.lock_version, claimed.task.lock_version);

        connection
            .execute(
                "UPDATE task_runs SET claim_owner = ?1 WHERE id = ?2",
                ("worker", "r_reclaim_rollback"),
            )
            .await
            .expect("restore run owner");
        connection
            .execute(
                "INSERT INTO task_events(event_id, board_id, task_id, run_id, kind, actor, payload_json, created_at) VALUES (?1, 'b_default', ?2, NULL, 'other.event', 'tester', '{}', 1)",
                ("e_reclaim_event_conflict", task.id.as_str()),
            )
            .await
            .expect("insert conflicting event");
        let event_error = store
            .reclaim_expired_task(
                &task.id,
                reclaim_input(
                    claimed.task.lock_version,
                    "dispatcher",
                    "e_reclaim_event_conflict",
                    "ready",
                    1,
                    "claim expired",
                    500,
                ),
            )
            .await
            .expect_err("event conflict must fail");
        assert!(matches!(event_error, StoreError::Turso(_)));
        let rolled_back = store
            .get_task_global(&task.id)
            .await
            .expect("get rolled back task");
        assert_eq!(rolled_back.status, "running");
        assert_eq!(rolled_back.lock_version, claimed.task.lock_version);
        let run = first_row(
            connection
                .query(
                    "SELECT status, finished_at, error FROM task_runs WHERE id = ?1",
                    ["r_reclaim_rollback"],
                )
                .await
                .expect("run query"),
        )
        .await
        .expect("run row");
        assert_eq!(
            text_value(run.get_value(0).expect("run status"), "run.status")
                .expect("run status text"),
            "running"
        );
        assert!(matches!(
            run.get_value(1).expect("run finished"),
            Value::Null
        ));
        assert!(matches!(run.get_value(2).expect("run error"), Value::Null));
        let reclaimed_events = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind = 'task.reclaimed'",
                    [task.id.as_str()],
                )
                .await
                .expect("reclaimed event count"),
        )
        .await
        .expect("reclaimed event count row");
        assert_eq!(
            integer_value(
                reclaimed_events.get_value(0).expect("event count"),
                "event.count",
            )
            .expect("event count integer"),
            0
        );
    }
}
