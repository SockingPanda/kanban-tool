#[cfg(test)]
mod tests {
    use crate::store_operations::RemoveStepInput;
    use crate::test_support::*;

    #[tokio::test]
    async fn remove_last_step_unplans_parent_recomputes_status_and_keeps_events_atomic() {
        let (_directory, store, _path) = store("step-remove").await;
        store.initialize().await.expect("初始化数据库");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_remove_parent", None, "Step remove parent"),
            )
            .await
            .expect("创建父任务");
        let created = store
            .create_step(
                &parent.id,
                step_input(
                    "step_remove",
                    None,
                    "Remove step",
                    Some(1024),
                    "planner",
                    parent.lock_version,
                    "unplanned",
                    "ready",
                    "e_step_remove_create",
                    "e_step_remove_plan",
                    "e_step_remove_recompute_create",
                    500,
                ),
            )
            .await
            .expect("创建 step");
        assert_eq!(created.status, "todo");
        let parent_after_create = store
            .get_task_global(&parent.id)
            .await
            .expect("读取创建后的父任务");
        assert_eq!(parent_after_create.status, "ready");
        assert_eq!(parent_after_create.lock_version, 1);

        let connection = store.connection().await.expect("获取连接");
        connection
            .execute(
                "UPDATE tasks SET status = 'todo' WHERE id = ?1",
                [parent.id.as_str()],
            )
            .await
            .expect("准备状态重算场景");

        let removed = store
            .remove_step(
                &parent.id,
                &created.id,
                RemoveStepInput {
                    actor: "operator".into(),
                    event_id: "e_step_remove".into(),
                    recompute_event_id: "e_step_remove_recompute".into(),
                    updated_at: 600,
                    expected_lock_version: 1,
                },
            )
            .await
            .expect("删除 step");
        assert_eq!(removed.id, created.id);

        let listed = store.list_steps(&parent.id).await.expect("列出剩余 step");
        assert!(listed.steps.is_empty());
        assert_eq!(listed.execution_plan.state, "unplanned");
        let parent_after_remove = store
            .get_task_global(&parent.id)
            .await
            .expect("读取删除后的父任务");
        assert_eq!(parent_after_remove.status, "ready");
        assert_eq!(parent_after_remove.lock_version, 2);

        let events = first_row(
            connection
                .query(
                    "SELECT kind, payload_json FROM task_events WHERE event_id IN ('e_step_remove', 'e_step_remove_recompute') ORDER BY event_id",
                    (),
                )
                .await
                .expect("查询删除事件"),
        )
        .await
        .expect("读取删除事件");
        assert_eq!(
            text_value(events.get_value(0).expect("删除事件类型"), "event.kind")
                .expect("删除事件 kind"),
            "task.step.removed"
        );
        let recompute = first_row(
            connection
                .query(
                    "SELECT kind, payload_json FROM task_events WHERE event_id = 'e_step_remove_recompute'",
                    (),
                )
                .await
                .expect("查询重算事件"),
        )
        .await
        .expect("读取重算事件");
        assert_eq!(
            text_value(recompute.get_value(0).expect("重算事件类型"), "event.kind")
                .expect("重算事件 kind"),
            "task.recomputed"
        );
        assert_eq!(
            text_value(
                recompute.get_value(1).expect("重算 payload"),
                "event.payload"
            )
            .expect("重算 payload 类型"),
            r#"{"to_status":"ready"}"#
        );
    }

    #[tokio::test]
    async fn remove_step_reports_missing_step_without_mutating_parent() {
        let (_directory, store, _path) = store("step-remove-missing").await;
        store.initialize().await.expect("初始化数据库");
        let parent = store
            .create_task(
                "default",
                create_input("t_step_remove_missing", None, "Step remove missing"),
            )
            .await
            .expect("创建父任务");
        let error = store
            .remove_step(
                &parent.id,
                "step_missing",
                RemoveStepInput {
                    actor: "operator".into(),
                    event_id: "e_step_remove_missing".into(),
                    recompute_event_id: "e_step_remove_missing_recompute".into(),
                    updated_at: 600,
                    expected_lock_version: parent.lock_version,
                },
            )
            .await
            .expect_err("不存在的 step 应返回 not found");
        assert!(matches!(error, StoreError::StepNotFound(id) if id == "step_missing"));
        let after = store
            .get_task_global(&parent.id)
            .await
            .expect("读取未变更的父任务");
        assert_eq!(after.lock_version, parent.lock_version);
    }
}
