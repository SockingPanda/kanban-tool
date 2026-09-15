use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
    time::Duration,
};

use serde_json::json;
use tokio::sync::{oneshot, watch};

use crate::{
    AddTaskLabelsCommand, CommentAuthorType, CommentKind, CreateAttachmentCommand,
    CreateBoardLabelCommand, DeleteAttachmentCommand, DeleteBoardLabelCommand, KanbanError,
    KanbanService, RemoveTaskLabelCommand, TaskRecord, TaskStatus, UpdateTaskCommand,
    operations::{
        CreateCommentCommand, CreateStepCommand, CreateTaskCommand, RemoveStepCommand,
        UpdateStepCommand,
    },
    test_support,
};

mod measurement;

const TASK: &str = "t_realtime_fixture";

async fn service(name: &str) -> (tempfile::TempDir, KanbanService) {
    let (directory, store, _) = test_support::store(name).await;
    store.initialize().await.expect("初始化隔离 Turso");
    let mut service = KanbanService::new(store);
    let attachment_root = directory.path().join("attachments");
    std::fs::create_dir(&attachment_root).unwrap();
    service.attachment_root = Some(Arc::new(attachment_root));
    service.create_task(task_command(TASK)).await.unwrap();
    (directory, service)
}

fn task_command(task_id: &str) -> CreateTaskCommand {
    CreateTaskCommand {
        task_id: task_id.to_owned(),
        board: "default".to_owned(),
        idempotency_key: None,
        title: task_id.to_owned(),
        description: Some("真实 service 通知与一致读取".to_owned()),
        requested_status: Some(TaskStatus::Todo),
        assignee: None,
        priority: 1,
        scheduled_at: None,
        due_at: None,
        max_retries: None,
        metadata: Default::default(),
        labels: Vec::new(),
        depends_on: Vec::new(),
        actor: "realtime-test".to_owned(),
    }
}

fn comment_command(key: &str) -> CreateCommentCommand {
    CreateCommentCommand {
        task_id: TASK.to_owned(),
        idempotency_key: Some(key.to_owned()),
        author: "realtime-test".to_owned(),
        author_type: CommentAuthorType::User,
        agent_type: None,
        body: key.to_owned(),
        kind: CommentKind::Note,
        metadata: Default::default(),
    }
}

fn update_command(task: &TaskRecord, title: &str) -> UpdateTaskCommand {
    UpdateTaskCommand {
        task_id: task.id.clone(),
        actor: "realtime-test".to_owned(),
        expected_lock_version: Some(task.lock_version),
        title: Some(title.to_owned()),
        description: None,
        assignee: None,
        priority: None,
        scheduled_at: None,
        due_at: None,
        max_retries: None,
        metadata: None,
    }
}

async fn notified(changes: &mut watch::Receiver<u64>) {
    tokio::time::timeout(Duration::from_secs(5), changes.changed())
        .await
        .expect("退出写临界区后必须可重读")
        .expect("service 仍存活");
    changes.borrow_and_update();
    assert!(!changes.has_changed().unwrap());
}

fn assert_pending<F: Future>(future: Pin<&mut F>) {
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(future.poll(&mut context), Poll::Pending));
}

async fn canonical_counts(service: &KanbanService) -> Vec<i64> {
    let connection = service.store.connection().await.unwrap();
    let mut counts = Vec::new();
    for table in [
        "tasks",
        "task_comments",
        "task_steps",
        "task_events",
        "projection_jobs",
    ] {
        counts.push(test_support::count_rows(&connection, table).await);
    }
    counts
}

#[tokio::test]
async fn comments_notify_even_when_realtime_cards_are_identical() {
    let (_directory, service) = service("realtime-comments").await;
    let before = service.load_realtime_board("default").await.unwrap();
    let mut changes = service.subscribe_realtime_changes();
    let comment = service
        .create_comment(comment_command("new-comment"))
        .await
        .unwrap();
    notified(&mut changes).await;
    assert_eq!(service.list_comments(TASK).await.unwrap(), vec![comment]);
    assert_eq!(
        service.load_realtime_board("default").await.unwrap().tasks,
        before.tasks
    );
    assert!(!changes.has_changed().unwrap(), "快照重读不得触发自身刷新");
}

#[tokio::test]
async fn label_catalog_and_task_bindings_each_notify_after_commit() {
    let (_directory, service) = service("realtime-labels").await;
    let before = service.load_realtime_board("default").await.unwrap();
    let mut changes = service.subscribe_realtime_changes();
    let label = service
        .create_board_label(CreateBoardLabelCommand {
            board: "default".to_owned(),
            name: "刷新标签".to_owned(),
            color: None,
        })
        .await
        .unwrap();
    notified(&mut changes).await;
    assert_eq!(
        service.list_board_labels("default").await.unwrap(),
        vec![label.clone()]
    );
    assert_eq!(
        service.load_realtime_board("default").await.unwrap().tasks,
        before.tasks
    );
    service
        .add_task_labels(AddTaskLabelsCommand {
            task_id: TASK.to_owned(),
            names: vec![label.name.clone()],
            create_missing: false,
            actor: "realtime-test".to_owned(),
        })
        .await
        .unwrap();
    notified(&mut changes).await;
    assert_eq!(
        service.list_task_labels(TASK).await.unwrap(),
        vec![label.clone()]
    );
    service
        .remove_task_label(RemoveTaskLabelCommand {
            task_id: TASK.to_owned(),
            label_ref: label.id.clone(),
            actor: "realtime-test".to_owned(),
        })
        .await
        .unwrap();
    notified(&mut changes).await;
    assert!(service.list_task_labels(TASK).await.unwrap().is_empty());
    service
        .delete_board_label(DeleteBoardLabelCommand {
            board: "default".to_owned(),
            label_ref: label.id,
            force: false,
            actor: "realtime-test".to_owned(),
        })
        .await
        .unwrap();
    notified(&mut changes).await;
    assert!(
        service
            .list_board_labels("default")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn step_create_update_and_remove_each_notify_with_committed_plan() {
    let (_directory, service) = service("realtime-steps").await;
    let mut changes = service.subscribe_realtime_changes();
    let plan = service
        .create_step(CreateStepCommand {
            task_id: TASK.to_owned(),
            idempotency_key: None,
            title: "检查刷新".to_owned(),
            body: None,
            linked_task_id: None,
            position: None,
            required: true,
            actor: "realtime-test".to_owned(),
        })
        .await
        .unwrap();
    notified(&mut changes).await;
    assert_eq!(service.list_steps(TASK).await.unwrap(), plan);
    let step_id = plan.steps[0].id.clone();
    let updated = service
        .update_step(UpdateStepCommand {
            task_id: TASK.to_owned(),
            step_id: step_id.clone(),
            title: Some("检查最终事实".to_owned()),
            body: None,
            linked_task_id: None,
            unlink_task: false,
            position: None,
            required: None,
            actor: "realtime-test".to_owned(),
        })
        .await
        .unwrap();
    notified(&mut changes).await;
    assert_eq!(service.list_steps(TASK).await.unwrap(), updated);
    let removed = service
        .remove_step(RemoveStepCommand {
            task_id: TASK.to_owned(),
            step_id,
            actor: "realtime-test".to_owned(),
        })
        .await
        .unwrap();
    notified(&mut changes).await;
    assert_eq!(service.list_steps(TASK).await.unwrap(), removed);
    assert!(removed.steps.is_empty());
}

#[tokio::test]
async fn attachment_create_and_delete_notify_with_file_and_canonical_metadata() {
    let (_directory, service) = service("realtime-attachments").await;
    let before = service.load_realtime_board("default").await.unwrap();
    let mut changes = service.subscribe_realtime_changes();
    let content = b"G06 committed attachment".to_vec();
    let attachment = service
        .create_attachment(CreateAttachmentCommand {
            task_id: TASK.to_owned(),
            id: None,
            filename: "proof.txt".to_owned(),
            rel_path: None,
            content_type: Some("text/plain".to_owned()),
            content: content.clone(),
            sha256: None,
            created_by: "realtime-test".to_owned(),
        })
        .await
        .unwrap();
    notified(&mut changes).await;
    assert_eq!(
        service.list_attachments(TASK).await.unwrap(),
        vec![attachment.clone()]
    );
    assert_eq!(
        service
            .read_attachment(TASK, &attachment.id)
            .await
            .unwrap()
            .content,
        content
    );
    assert_eq!(
        service.load_realtime_board("default").await.unwrap().tasks,
        before.tasks
    );
    assert!(
        service
            .delete_attachment(DeleteAttachmentCommand {
                task_id: TASK.to_owned(),
                attachment_id: attachment.id.clone(),
                actor: "realtime-test".to_owned(),
            })
            .await
            .unwrap()
    );
    notified(&mut changes).await;
    assert!(service.list_attachments(TASK).await.unwrap().is_empty());
    assert!(service.read_attachment(TASK, &attachment.id).await.is_err());
}

#[tokio::test]
async fn cas_and_idempotency_outcomes_are_hints_without_extra_facts() {
    let (_directory, service) = service("realtime-rejected-writes").await;
    let task = service.get_task(TASK).await.unwrap();
    let command = comment_command("replay-once");
    let first = service.create_comment(command.clone()).await.unwrap();
    let before = canonical_counts(&service).await;
    let mut changes = service.subscribe_realtime_changes();
    assert_eq!(
        service.create_comment(command.clone()).await.unwrap(),
        first
    );
    notified(&mut changes).await;
    assert_eq!(canonical_counts(&service).await, before);
    let mut conflict = command;
    conflict.body = "改变幂等请求".to_owned();
    assert!(matches!(
        service.create_comment(conflict).await,
        Err(KanbanError::IdempotencyConflict { .. })
    ));
    notified(&mut changes).await;
    assert_eq!(canonical_counts(&service).await, before);
    let updated = service
        .update_task(update_command(&task, "已提交标题"))
        .await
        .unwrap();
    notified(&mut changes).await;
    let before = canonical_counts(&service).await;
    assert!(
        service
            .update_task(update_command(&task, "过期标题"))
            .await
            .is_err()
    );
    notified(&mut changes).await;
    assert_eq!(canonical_counts(&service).await, before);
    assert_eq!(service.get_task(TASK).await.unwrap(), updated);
    let mut invalid = comment_command("invalid");
    invalid.body.clear();
    assert!(service.create_comment(invalid).await.is_err());
    assert!(!changes.has_changed().unwrap(), "锁前拒绝不声称进入临界区");
}

#[tokio::test]
async fn event_conflict_rolls_back_comment_before_gate_exit_hint() {
    let (_directory, service) = service("realtime-rollback").await;
    let events = service
        .list_events(
            "default",
            crate::EventListOptions {
                task_id: Some(TASK.to_owned()),
                after: 0,
                limit: 100,
            },
        )
        .await
        .unwrap();
    let before = canonical_counts(&service).await;
    let cards = service.load_realtime_board("default").await.unwrap().tasks;
    let mut changes = service.subscribe_realtime_changes();
    {
        // 复用 store 的 event-id seam，令已插入的 comment 在事件唯一约束失败时回滚。
        let _mutation = service.mutation_gate.lock().await;
        let result = service
            .store
            .create_comment(
                TASK,
                test_support::comment_input(
                    "c_rollback",
                    None,
                    "realtime-test",
                    "user",
                    None,
                    "必须回滚",
                    "note",
                    "{}",
                    &events.events[0].event_id,
                    500,
                ),
            )
            .await;
        assert!(result.is_err());
        assert!(!changes.has_changed().unwrap());
    }
    notified(&mut changes).await;
    assert_eq!(canonical_counts(&service).await, before);
    assert!(service.list_comments(TASK).await.unwrap().is_empty());
    assert_eq!(
        service.load_realtime_board("default").await.unwrap().tasks,
        cards
    );
}

#[tokio::test]
async fn cancelled_waiter_does_not_write_or_notify_and_store_wrappers_share_gate() {
    let (_directory, first) = service("realtime-shared-store").await;
    let second = KanbanService::new(first.store.clone());
    let clone = second.clone();
    assert!(Arc::ptr_eq(&first.mutation_gate, &second.mutation_gate));
    assert!(Arc::ptr_eq(&first.mutation_gate, &clone.mutation_gate));
    let mut changes = first.subscribe_realtime_changes();
    let mut second_changes = second.subscribe_realtime_changes();
    let fence = first.mutation_gate.read().await;
    let mut waiter = Box::pin(clone.create_comment(comment_command("cancel-waiter")));
    assert_pending(waiter.as_mut());
    drop(waiter);
    assert!(!changes.has_changed().unwrap());
    drop(fence);
    assert!(first.list_comments(TASK).await.unwrap().is_empty());
    second
        .create_comment(comment_command("after-cancel"))
        .await
        .unwrap();
    notified(&mut changes).await;
    notified(&mut second_changes).await;
    assert_eq!(first.list_comments(TASK).await.unwrap().len(), 1);
    first.load_realtime_board("default").await.unwrap();
    first.check_realtime_board("b_default").await.unwrap();
    assert!(first.check_realtime_board("default").await.is_err());
    assert!(first.load_realtime_board("missing").await.is_err());
    assert!(
        !changes.has_changed().unwrap(),
        "成功或失败的 read fence 都静默"
    );
}

#[tokio::test]
async fn cancelled_transaction_owner_rolls_back_before_consistent_reader_continues() {
    let (_directory, service) = service("realtime-cancel-owner").await;
    let cards = service.load_realtime_board("default").await.unwrap().tasks;
    let before = canonical_counts(&service).await;
    let writer_service = service.clone();
    let (staged, staged_rx) = oneshot::channel();
    let mut changes = service.subscribe_realtime_changes();
    let writer = tokio::spawn(async move {
        let _mutation = writer_service.mutation_gate.lock().await;
        let mut connection = writer_service.store.connection().await.unwrap();
        let transaction = connection
            .transaction_with_behavior(turso::transaction::TransactionBehavior::Immediate)
            .await
            .unwrap();
        transaction
            .execute("UPDATE tasks SET title='未提交标题' WHERE id=?1", [TASK])
            .await
            .unwrap();
        staged.send(()).unwrap();
        std::future::pending::<()>().await;
        transaction.commit().await.unwrap();
    });
    staged_rx.await.unwrap();
    let mut snapshot = Box::pin(service.load_realtime_board("default"));
    assert_pending(snapshot.as_mut());
    assert!(!changes.has_changed().unwrap());
    writer.abort();
    assert!(writer.await.unwrap_err().is_cancelled());
    notified(&mut changes).await;
    let snapshot = tokio::time::timeout(Duration::from_secs(5), snapshot)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.tasks, cards);
    assert_eq!(canonical_counts(&service).await, before);
    service
        .create_comment(comment_command("after-owner-cancel"))
        .await
        .unwrap();
    notified(&mut changes).await;
}

#[tokio::test]
async fn subscribing_before_snapshot_covers_both_writer_orderings() {
    let (_directory, service) = service("realtime-subscribe-first").await;
    let mut changes = service.subscribe_realtime_changes();
    let fence = service.mutation_gate.read().await;
    let mut writer = Box::pin(service.create_task(task_command("t_before_snapshot")));
    assert_pending(writer.as_mut());
    let mut snapshot = Box::pin(service.load_realtime_board("default"));
    assert_pending(snapshot.as_mut());
    drop(fence);
    writer.await.unwrap();
    let snapshot = snapshot.await.unwrap();
    assert_eq!(snapshot.tasks.len(), 2);
    notified(&mut changes).await;
    let fence = service.mutation_gate.read().await;
    let mut snapshot = Box::pin(service.load_realtime_board("default"));
    assert_pending(snapshot.as_mut());
    let mut writer = Box::pin(service.create_task(task_command("t_after_snapshot")));
    assert_pending(writer.as_mut());
    drop(fence);
    assert_eq!(snapshot.await.unwrap().tasks.len(), 2);
    writer.await.unwrap();
    notified(&mut changes).await;
    assert_eq!(
        service
            .load_realtime_board("default")
            .await
            .unwrap()
            .tasks
            .len(),
        3
    );
    assert!(!changes.has_changed().unwrap());
}

#[tokio::test]
async fn legacy_ontology_and_maintenance_are_blocked_by_the_same_read_fence() {
    let (_directory, service) = service("realtime-legacy-maintenance").await;
    let mut changes = service.subscribe_realtime_changes();
    let fence = service.mutation_gate.read().await;
    let mut ontology = Box::pin(service.label_ontology("rebuild_atom_index", "default", json!({})));
    assert_pending(ontology.as_mut());
    let mut maintenance = Box::pin(service.checkpoint());
    assert_pending(maintenance.as_mut());
    assert!(!changes.has_changed().unwrap());
    drop(fence);
    ontology.await.unwrap();
    notified(&mut changes).await;
    maintenance.await.unwrap();
    notified(&mut changes).await;
    assert!(
        service
            .maintenance_status()
            .await
            .unwrap()
            .owner
            .owner
            .is_none()
    );
    service.load_realtime_board("default").await.unwrap();
    assert!(!changes.has_changed().unwrap());
}

#[tokio::test]
async fn every_legacy_ontology_mutation_waits_and_rejected_payloads_leave_no_facts() {
    let (_directory, service) = service("realtime-legacy-all-mutations").await;
    let before = canonical_counts(&service).await;
    let mut changes = service.subscribe_realtime_changes();
    for operation in [
        "upsert_semantics",
        "delete_semantics",
        "rebuild_atom_index",
        "propose_label",
        "decide_proposal",
        "record_observation",
        "create_action",
        "apply_atom",
        "revert_mutation",
        "validate_action",
    ] {
        let fence = service.mutation_gate.read().await;
        let mut mutation = Box::pin(service.label_ontology(operation, "missing", json!({})));
        assert_pending(mutation.as_mut());
        assert!(!changes.has_changed().unwrap());
        drop(fence);
        assert!(mutation.await.is_err(), "{operation}");
        notified(&mut changes).await;
        assert_eq!(canonical_counts(&service).await, before, "{operation}");
    }
}

async fn projection_rows(service: &KanbanService) -> Vec<String> {
    let connection = service.store.connection().await.unwrap();
    let mut result = Vec::new();
    for table in [
        "projection_state",
        "projection_jobs",
        "label_atom_index_boards",
    ] {
        let mut columns = connection
            .query(format!("PRAGMA table_info({table})"), ())
            .await
            .unwrap();
        let mut names = Vec::new();
        while let Some(column) = columns.next().await.unwrap() {
            names.push(test_support::text_value(column.get_value(1).unwrap(), "name").unwrap());
        }
        let mut rows = connection
            .query(
                format!(
                    "SELECT json_array({}) FROM {table} ORDER BY rowid",
                    names.join(",")
                ),
                (),
            )
            .await
            .unwrap();
        while let Some(row) = rows.next().await.unwrap() {
            result.push(format!(
                "{table}:{}",
                test_support::text_value(row.get_value(0).unwrap(), "row").unwrap()
            ));
        }
    }
    result
}

#[tokio::test]
async fn projection_queries_and_consistent_reads_do_not_write_or_notify() {
    let (_directory, service) = service("realtime-read-projections").await;
    service.rebuild_search_index("default").await.unwrap();
    let before = projection_rows(&service).await;
    let counts = canonical_counts(&service).await;
    let changes = service.subscribe_realtime_changes();
    service.load_realtime_board("default").await.unwrap();
    service.check_realtime_board("b_default").await.unwrap();
    service.get_task_details(TASK).await.unwrap();
    service
        .search_tasks(crate::SearchQuery {
            board: "default".to_owned(),
            q: Some("真实".to_owned()),
            statuses: Vec::new(),
            labels: Vec::new(),
            assignee: None,
            include_archived: false,
            limit: 100,
            offset: 0,
        })
        .await
        .unwrap();
    service.search_index_status("default").await.unwrap();
    service.graph_status("default").await.unwrap();
    service.vector_status("default").await.unwrap();
    service.label_atom_index_status("default").await.unwrap();
    service
        .query_label_atom_index(
            "default",
            crate::LabelAtomIndexQuery {
                query: None,
                polarity: None,
                limit: 10,
            },
        )
        .await
        .unwrap();
    service
        .label_ontology("index_status", "default", json!({}))
        .await
        .unwrap();
    service.maintenance_status().await.unwrap();
    assert_eq!(projection_rows(&service).await, before);
    assert_eq!(canonical_counts(&service).await, counts);
    assert!(!changes.has_changed().unwrap());
}
