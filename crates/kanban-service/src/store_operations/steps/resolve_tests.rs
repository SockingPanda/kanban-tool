#[cfg(test)]
mod tests {
    use crate::store_operations::{CompleteStepInput, ReopenStepInput, SkipStepInput};
    use crate::test_support::*;

    #[tokio::test]
    async fn resolve_step_lifecycle_persists_resolution_fields_and_events_atomically() {
        let (_directory, store, _path) = store("step-resolve").await;
        store.initialize().await.expect("初始化数据库");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_resolve_parent", None, "Step resolve parent"),
            )
            .await
            .expect("创建父任务");
        let created = store
            .create_step(
                &parent.id,
                step_input(
                    "step_resolve",
                    None,
                    "Resolve step",
                    Some(1024),
                    "planner",
                    parent.lock_version,
                    "unplanned",
                    "ready",
                    "e_step_resolve_create",
                    "e_step_resolve_plan",
                    "e_step_resolve_recompute",
                    500,
                ),
            )
            .await
            .expect("创建 step");
        assert_eq!(created.status, "todo");

        let done = store
            .complete_step(
                &parent.id,
                &created.id,
                CompleteStepInput {
                    note: "finished".into(),
                    actor: "operator".into(),
                    event_id: "e_step_resolve_done".into(),
                    updated_at: 600,
                    expected_lock_version: 1,
                },
            )
            .await
            .expect("完成 step");
        assert_eq!(done.status, "done");
        assert_eq!(done.resolution_note.as_deref(), Some("finished"));
        assert_eq!(done.resolved_by.as_deref(), Some("operator"));
        assert_eq!(done.resolved_at, Some(600));
        assert_eq!(
            store
                .get_task_global(&parent.id)
                .await
                .expect("读取完成后的父任务")
                .lock_version,
            2
        );

        let skipped = store
            .skip_step(
                &parent.id,
                &created.id,
                SkipStepInput {
                    reason: "not needed".into(),
                    actor: "operator".into(),
                    event_id: "e_step_resolve_skip".into(),
                    updated_at: 700,
                    expected_lock_version: 2,
                },
            )
            .await
            .expect("跳过 step");
        assert_eq!(skipped.status, "skipped");
        assert_eq!(skipped.resolution_note.as_deref(), Some("not needed"));

        let reopened = store
            .reopen_step(
                &parent.id,
                &created.id,
                ReopenStepInput {
                    reason: "needs revision".into(),
                    actor: "operator".into(),
                    event_id: "e_step_resolve_reopen".into(),
                    updated_at: 800,
                    expected_lock_version: 3,
                },
            )
            .await
            .expect("重新打开 step");
        assert_eq!(reopened.status, "todo");
        assert_eq!(reopened.resolution_note, None);
        assert_eq!(reopened.resolved_by, None);
        assert_eq!(reopened.resolved_at, None);

        let connection = store.connection().await.expect("获取连接");
        let event_count = first_row(
            connection
                .query(
                    "SELECT COUNT(*) FROM task_events WHERE task_id = ?1 AND kind IN ('task.step.done', 'task.step.skipped', 'task.step.reopened')",
                    [parent.id.as_str()],
                )
                .await
                .expect("查询 step 事件"),
        )
        .await
        .expect("读取事件计数");
        assert_eq!(
            integer_value(event_count.get_value(0).expect("事件计数"), "events")
                .expect("事件计数类型"),
            3
        );
        let payload = first_row(
            connection
                .query(
                    "SELECT payload_json FROM task_events WHERE event_id = 'e_step_resolve_done'",
                    (),
                )
                .await
                .expect("查询完成事件"),
        )
        .await
        .expect("读取完成事件");
        assert_eq!(
            text_value(payload.get_value(0).expect("事件 payload"), "event.payload")
                .expect("事件 payload 类型"),
            r#"{"step_id":"step_resolve","linked_task_id":null,"position":1024,"required":true,"status":"done"}"#
        );
    }
}
