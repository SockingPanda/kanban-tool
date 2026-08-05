#[cfg(test)]
mod tests {
    use crate::test_support::*;
    use crate::{AddTaskLabelsInput, CreateLabelInput};

    #[tokio::test]
    async fn list_tasks_excludes_archived_by_default_and_supports_status_priority_and_assignee_filters()
     {
        let (_directory, store, _path) = store("list-filters").await;
        store.initialize().await.expect("initialize");
        let first = store
            .create_task(
                "default",
                create_input("t_filter_1", Some("filter-1"), "First"),
            )
            .await
            .expect("first task");
        let mut second_input = create_input("t_filter_2", Some("filter-2"), "Second");
        second_input.status = "scheduled".to_owned();
        second_input.priority = 2;
        second_input.assignee = Some("other".to_owned());
        let second = store
            .create_task("default", second_input)
            .await
            .expect("second task");
        let archived = store
            .create_task(
                "default",
                create_input("t_filter_archived", Some("filter-archived"), "Archived"),
            )
            .await
            .expect("archived task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "UPDATE tasks SET status = 'archived', archived_at = 10 WHERE id = ?1",
                [archived.id.as_str()],
            )
            .await
            .expect("archive task");
        store
            .create_board_label(
                "default",
                CreateLabelInput {
                    id: "l_filter_bug".to_owned(),
                    name: "bug".to_owned(),
                    color: None,
                    created_at: 100,
                },
            )
            .await
            .expect("create label");
        store
            .add_task_labels(
                &first.id,
                AddTaskLabelsInput {
                    names: vec!["bug".to_owned()],
                    label_ids: vec!["l_filter_bug".to_owned()],
                    event_ids: vec!["e_filter_label".to_owned()],
                    create_missing: false,
                    actor: "tester".to_owned(),
                    now: 100,
                },
            )
            .await
            .expect("attach label");

        let default_page = store
            .list_tasks("default", TaskListOptions::default())
            .await
            .expect("default task list");
        assert_eq!(default_page.total, 2);
        assert_eq!(
            default_page
                .tasks
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            vec![first.id.as_str(), second.id.as_str()]
        );

        let filtered = store
            .list_tasks(
                "b_default",
                TaskListOptions {
                    statuses: vec!["todo".to_owned()],
                    priorities: vec![1],
                    assignee: Some("agent".to_owned()),
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("filtered task list");
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.tasks[0].id, first.id);

        let label_filtered = store
            .list_tasks(
                "default",
                TaskListOptions {
                    labels: vec!["bug".to_owned()],
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("label filtered task list");
        assert_eq!(label_filtered.total, 1);
        assert_eq!(label_filtered.tasks[0].id, first.id);

        let with_archived = store
            .list_tasks(
                "default",
                TaskListOptions {
                    include_archived: true,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("archived task list");
        assert_eq!(with_archived.total, 3);
        assert_eq!(with_archived.tasks[2].id, archived.id);
    }

    #[tokio::test]
    async fn list_tasks_supports_id_seq_board_seq_and_escaped_text_search() {
        let (_directory, store, _path) = store("list-search").await;
        store.initialize().await.expect("initialize");
        let first = store
            .create_task(
                "default",
                create_input("t_search", Some("search-1"), "Literal %_\\ Marker"),
            )
            .await
            .expect("search task");
        let mut second_input = create_input("t_other", Some("search-2"), "Different");
        second_input.description = Some("Needle in description".to_owned());
        let second = store
            .create_task("default", second_input)
            .await
            .expect("second search task");

        for (query, expected) in [
            ("t_search", vec![first.id.as_str()]),
            ("default#1", vec![first.id.as_str()]),
            ("#1", vec![first.id.as_str()]),
            ("1", vec![first.id.as_str()]),
            ("%_\\", vec![first.id.as_str()]),
            ("needle", vec![second.id.as_str()]),
        ] {
            let page = store
                .list_tasks(
                    "default",
                    TaskListOptions {
                        q: Some(query.to_owned()),
                        ..TaskListOptions::default()
                    },
                )
                .await
                .expect("search task list");
            assert_eq!(
                page.tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "query {query}"
            );
        }

        let mismatch = store
            .list_tasks(
                "default",
                TaskListOptions {
                    q: Some("other#1".to_owned()),
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("mismatched board search");
        assert!(mismatch.tasks.is_empty());
        assert_eq!(mismatch.total, 0);
    }

    #[tokio::test]
    async fn list_tasks_plan_filters_and_derived_fields_are_consistent() {
        let (_directory, store, _path) = store("list-plan").await;
        store.initialize().await.expect("initialize");
        let plain = store
            .create_task(
                "default",
                create_input("t_plan_plain", Some("plan-plain"), "Plain"),
            )
            .await
            .expect("plain task");
        let with_steps = store
            .create_task(
                "default",
                create_input("t_plan_steps", Some("plan-steps"), "With steps"),
            )
            .await
            .expect("steps task");
        let not_required = store
            .create_task(
                "default",
                create_input(
                    "t_plan_not_required",
                    Some("plan-not-required"),
                    "Not required",
                ),
            )
            .await
            .expect("not required task");
        let done = store
            .create_task(
                "default",
                create_input("t_plan_done", Some("plan-done"), "Done"),
            )
            .await
            .expect("done task");
        let parent = store
            .create_task(
                "default",
                create_input("t_plan_parent", Some("plan-parent"), "Parent"),
            )
            .await
            .expect("parent task");
        let child = store
            .create_task(
                "default",
                create_input("t_plan_child", Some("plan-child"), "Child"),
            )
            .await
            .expect("child task");
        let connection = store.connection().await.expect("connection");
        connection
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, position, title, required, status, created_by, created_at, updated_by, updated_at) VALUES (?1, 'b_default', ?2, 1, 'required', 1, 'todo', 'tester', 1, 'tester', 1)",
                ("step_incomplete", with_steps.id.as_str()),
            )
            .await
            .expect("insert incomplete step");
        connection
            .execute(
                "INSERT INTO task_steps(id, board_id, parent_task_id, position, title, required, status, created_by, created_at, updated_by, updated_at) VALUES (?1, 'b_default', ?2, 2, 'optional', 0, 'done', 'tester', 1, 'tester', 1)",
                ("step_optional", with_steps.id.as_str()),
            )
            .await
            .expect("insert optional step");
        connection
            .execute(
                "UPDATE task_execution_plans SET state = 'not_required' WHERE task_id = ?1",
                [not_required.id.as_str()],
            )
            .await
            .expect("set plan not required");
        connection
            .execute(
                "UPDATE tasks SET status = 'done' WHERE id = ?1",
                [done.id.as_str()],
            )
            .await
            .expect("finish task");
        connection
            .execute(
                "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', ?1, ?2, 1)",
                (parent.id.as_str(), child.id.as_str()),
            )
            .await
            .expect("insert dependency");
        connection
            .execute(
                "UPDATE tasks SET status = 'blocked' WHERE id = ?1",
                [parent.id.as_str()],
            )
            .await
            .expect("block parent");

        let all = store
            .list_tasks(
                "default",
                TaskListOptions {
                    include_archived: true,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("all tasks");
        let child_record = all.tasks.iter().find(|task| task.id == child.id).unwrap();
        assert!(child_record.dependency_blocked);
        assert_eq!(child_record.unfinished_parent_count, 1);
        let steps_record = all
            .tasks
            .iter()
            .find(|task| task.id == with_steps.id)
            .unwrap();
        assert_eq!(steps_record.execution_plan_state, "planned");
        assert_eq!(steps_record.required_step_count, 1);
        assert_eq!(steps_record.completed_required_step_count, 0);
        assert_eq!(steps_record.optional_step_count, 1);
        assert!(all.tasks.iter().all(|task| task.labels.is_empty()));

        for (filter, expected) in [
            (
                TaskPlanFilter::PlanNeeded,
                vec![plain.id.as_str(), parent.id.as_str(), child.id.as_str()],
            ),
            (TaskPlanFilter::HasSteps, vec![with_steps.id.as_str()]),
            (
                TaskPlanFilter::IncompleteRequiredSteps,
                vec![with_steps.id.as_str()],
            ),
        ] {
            let page = store
                .list_tasks(
                    "default",
                    TaskListOptions {
                        include_archived: true,
                        plan_filters: vec![filter],
                        ..TaskListOptions::default()
                    },
                )
                .await
                .expect("plan filter list");
            assert_eq!(
                page.tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    #[tokio::test]
    async fn list_tasks_paginates_with_total_and_all_sort_mappings_are_deterministic() {
        let (_directory, store, _path) = store("list-pagination-sort").await;
        store.initialize().await.expect("initialize");
        for (id, title) in [
            ("t_sort_1", "Zeta"),
            ("t_sort_2", "Alpha"),
            ("t_sort_3", "Beta"),
        ] {
            store
                .create_task("default", create_input(id, Some(id), title))
                .await
                .expect("sort task");
        }
        let page = store
            .list_tasks(
                "default",
                TaskListOptions {
                    limit: 1,
                    offset: 1,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("paged task list");
        assert_eq!(page.total, 3);
        assert_eq!(page.tasks.len(), 1);
        assert_eq!(page.tasks[0].seq, 2);
        let empty_page = store
            .list_tasks(
                "default",
                TaskListOptions {
                    limit: 0,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect("zero limit task list");
        assert_eq!(empty_page.total, 3);
        assert!(empty_page.tasks.is_empty());

        let sorts = [
            TaskListSort::Seq,
            TaskListSort::SeqDesc,
            TaskListSort::Title,
            TaskListSort::TitleDesc,
            TaskListSort::Status,
            TaskListSort::StatusDesc,
            TaskListSort::Position,
            TaskListSort::PositionDesc,
            TaskListSort::Priority,
            TaskListSort::PriorityDesc,
            TaskListSort::Assignee,
            TaskListSort::AssigneeDesc,
            TaskListSort::ScheduledAt,
            TaskListSort::ScheduledAtDesc,
            TaskListSort::CreatedAt,
            TaskListSort::CreatedAtDesc,
            TaskListSort::UpdatedAt,
            TaskListSort::UpdatedAtDesc,
            TaskListSort::DueAt,
            TaskListSort::DueAtDesc,
        ];
        for sort in sorts {
            let options = TaskListOptions {
                sort,
                ..TaskListOptions::default()
            };
            let first = store
                .list_tasks("default", options.clone())
                .await
                .expect("sort task list");
            let second = store
                .list_tasks("default", options)
                .await
                .expect("repeat sort task list");
            assert_eq!(
                first
                    .tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>(),
                second
                    .tasks
                    .iter()
                    .map(|task| task.id.as_str())
                    .collect::<Vec<_>>()
            );
        }

        let error = store
            .list_tasks(
                "default",
                TaskListOptions {
                    limit: 1001,
                    ..TaskListOptions::default()
                },
            )
            .await
            .expect_err("limit above maximum must fail");
        assert!(matches!(error, StoreError::InvalidInput(message) if message.contains("limit")));
    }

    #[tokio::test]
    async fn list_tasks_reports_missing_board() {
        let (_directory, store, _path) = store("list-missing-board").await;
        store.initialize().await.expect("initialize");
        let error = store
            .list_tasks("missing", TaskListOptions::default())
            .await
            .expect_err("missing board must fail");
        assert!(matches!(error, StoreError::BoardNotFound(selector) if selector == "missing"));
    }
}
