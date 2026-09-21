use super::super::{ObjectCommand, ObjectMutation, ObjectPatch, ObjectTarget};
use super::*;
use crate::{
    CreateTaskCommand, KanbanService, TaskListOptions, TaskPlanningInput, UpdateTaskCommand,
};
use std::sync::Arc;

async fn fixture() -> (tempfile::TempDir, KanbanService) {
    let dir = tempfile::tempdir().unwrap();
    let service = KanbanService::open_with_roots(
        dir.path().join("planning.db"),
        None,
        Arc::new(dir.path().join("attachments")),
    )
    .await
    .unwrap();
    (dir, service)
}
fn command(kind: PlanningKind, request: &str, mutation: PlanningMutation) -> PlanningCommand {
    PlanningCommand {
        board_id: "b_default".into(),
        kind,
        actor: "planning-test".into(),
        request_id: request.into(),
        versions: PlanningVersions::default(),
        mutation,
    }
}
async fn container(s: &KanbanService, kind: PlanningKind, title: &str) -> PlanningRecord {
    s.planning_execute(command(
        kind,
        title,
        PlanningMutation::Create {
            title: title.into(),
            body: Some("原始正文".into()),
            parent_id: None,
            starts_at: (kind == PlanningKind::Cycle).then_some(1),
            ends_at: (kind == PlanningKind::Cycle).then_some(i64::MAX),
        },
    ))
    .await
    .unwrap()
    .object
}
fn create(id: &str, modules: Vec<String>, cycle: Option<String>) -> CreateTaskCommand {
    CreateTaskCommand {
        planning: TaskPlanningInput {
            module_ids: (!modules.is_empty()).then_some(modules),
            cycle_id: cycle.map(Some),
            ..Default::default()
        },
        request_fingerprint: None,
        task_id: id.into(),
        board: "default".into(),
        idempotency_key: Some(format!("create:{id}")),
        title: id.into(),
        description: Some("任务规格".into()),
        requested_status: None,
        assignee: None,
        priority: 2,
        scheduled_at: None,
        due_at: None,
        max_retries: None,
        metadata: BTreeMap::new(),
        labels: vec![],
        depends_on: vec![],
        actor: "planning-test".into(),
    }
}
fn update(id: &str, key: &str) -> UpdateTaskCommand {
    UpdateTaskCommand {
        planning: Default::default(),
        request_fingerprint: None,
        request_id: Some(key.into()),
        task_id: id.into(),
        actor: "planning-test".into(),
        expected_lock_version: None,
        title: None,
        description: None,
        assignee: None,
        priority: None,
        scheduled_at: None,
        due_at: None,
        max_retries: None,
        metadata: None,
    }
}
async fn counts(s: &KanbanService) -> Vec<i64> {
    let c = s.store.connection().await.unwrap();
    let mut result = Vec::new();
    for table in [
        "tasks",
        "task_execution_plans",
        "task_labels",
        "task_dependencies",
        "object_relation_edges",
        "task_events",
        "object_requests",
    ] {
        result.push(
            super::super::store::count(&c, &format!("SELECT COUNT(*) FROM {table}"), vec![])
                .await
                .unwrap(),
        );
    }
    result
}

#[tokio::test]
async fn create_and_update_membership_roll_back_every_fact_and_event() {
    let (_dir, s) = fixture().await;
    let m = container(&s, PlanningKind::Module, "module").await;
    let cycle = container(&s, PlanningKind::Cycle, "cycle").await;
    let dependency = s
        .create_task(create("t_parent", vec![], None))
        .await
        .unwrap();
    let label = s
        .create_board_label(crate::CreateBoardLabelCommand {
            board: "default".into(),
            name: "事务标签".into(),
            color: None,
        })
        .await
        .unwrap();
    let other = s
        .create_board(crate::CreateBoardCommand {
            slug: "other".into(),
            name: "other".into(),
            description: None,
            actor: "test".into(),
        })
        .await
        .unwrap();
    let mut elsewhere = command(
        PlanningKind::Module,
        "elsewhere",
        PlanningMutation::Create {
            title: "other".into(),
            body: None,
            parent_id: None,
            starts_at: None,
            ends_at: None,
        },
    );
    elsewhere.board_id = other.id;
    let foreign = s.planning_execute(elsewhere).await.unwrap().object;
    for (id, modules, cycle_id) in [
        ("t_bad_type", vec![m.id.clone()], Some(m.id.clone())),
        ("t_cross_board", vec![m.id.clone(), foreign.id], None),
        (
            "t_missing",
            vec![m.id.clone(), "obj_missing".into()],
            Some(cycle.id.clone()),
        ),
    ] {
        let before = counts(&s).await;
        let mut invalid = create(id, modules, cycle_id);
        invalid.labels = vec![label.id.clone()];
        invalid.depends_on = vec![dependency.id.clone()];
        assert!(s.create_task(invalid).await.is_err());
        assert_eq!(counts(&s).await, before, "失败创建必须整体回滚");
        assert!(s.get_task(id).await.is_err());
    }
    let task = s
        .create_task(create("t_atomic", vec![], None))
        .await
        .unwrap();
    let before = counts(&s).await;
    let mut bad = update(&task.id, "bad-update");
    bad.title = Some("不能留下".into());
    bad.planning.module_ids = Some(vec![m.id.clone()]);
    bad.planning.cycle_id = Some(Some(m.id.clone()));
    assert!(s.update_task(bad).await.is_err());
    assert_eq!(s.get_task(&task.id).await.unwrap(), task);
    assert_eq!(counts(&s).await, before);
    s.planning_execute(command(
        PlanningKind::Module,
        "archive-module",
        PlanningMutation::Archive { id: m.id.clone() },
    ))
    .await
    .unwrap();
    let before = counts(&s).await;
    assert!(
        s.create_task(create("t_archived", vec![m.id], None))
            .await
            .is_err()
    );
    assert_eq!(counts(&s).await, before);
}

#[tokio::test]
async fn original_task_requests_survive_later_membership_changes() {
    let (_dir, s) = fixture().await;
    let a = container(&s, PlanningKind::Module, "a").await;
    let b = container(&s, PlanningKind::Module, "b").await;
    let c1 = container(&s, PlanningKind::Cycle, "c1").await;
    let c2 = container(&s, PlanningKind::Cycle, "c2").await;
    let initial = create("t_retry", vec![a.id.clone()], Some(c1.id.clone()));
    let task = s.create_task(initial.clone()).await.unwrap();
    assert_eq!(task.module_ids, vec![a.id.clone()]);
    assert_eq!(task.cycle_id, Some(c1.id.clone()));
    let mut replace = update(&task.id, "move");
    replace.title = Some("新标题".into());
    replace.planning.module_ids = Some(vec![b.id.clone()]);
    replace.planning.cycle_id = Some(Some(c2.id.clone()));
    let moved = s.update_task(replace.clone()).await.unwrap();
    assert_eq!(moved.cycle_id, Some(c2.id.clone()));
    let mut clear = update(&task.id, "clear");
    clear.planning.module_ids = Some(vec![]);
    clear.planning.cycle_id = Some(None);
    let cleared = s.update_task(clear).await.unwrap();
    let before = counts(&s).await;
    assert_eq!(s.create_task(initial.clone()).await.unwrap(), cleared);
    assert_eq!(s.update_task(replace.clone()).await.unwrap(), cleared);
    assert_eq!(counts(&s).await, before);
    let mut different = initial;
    different.actor = "another".into();
    assert!(matches!(
        s.create_task(different).await,
        Err(crate::KanbanError::IdempotencyConflict(message)) if message.contains(&task.id)
    ));
    replace.planning.module_ids = Some(vec![a.id]);
    assert!(matches!(
        s.update_task(replace).await,
        Err(crate::KanbanError::IdempotencyConflict(message)) if message.contains(&task.id)
    ));
    assert_eq!(counts(&s).await, before);
}

#[tokio::test]
async fn object_only_task_and_old_new_endpoint_conflicts_are_rejected() {
    let (_dir, s) = fixture().await;
    let m = container(&s, PlanningKind::Module, "m").await;
    let c1 = container(&s, PlanningKind::Cycle, "c1").await;
    let c2 = container(&s, PlanningKind::Cycle, "c2").await;
    let task = s
        .create_task(create("t_cas", vec![], Some(c1.id.clone())))
        .await
        .unwrap();
    let before_object = task.object_version;
    s.planning_execute(command(
        PlanningKind::Module,
        "add",
        PlanningMutation::TaskAdd {
            id: m.id.clone(),
            task_id: task.id.clone(),
        },
    ))
    .await
    .unwrap();
    let current = s.get_task(&task.id).await.unwrap();
    assert_eq!(current.lock_version, task.lock_version);
    assert!(current.object_version > before_object);
    let mut stale = update(&task.id, "stale-object");
    stale.title = Some("stale".into());
    stale.planning.expected_object_version = Some(before_object);
    assert!(s.update_task(stale).await.is_err());
    let old = s.object_get("b_default", &c1.id).await.unwrap();
    s.planning_execute(command(
        PlanningKind::Cycle,
        "edit-old",
        PlanningMutation::Update {
            id: c1.id.clone(),
            title: Some("旧端点被编辑".into()),
            body: None,
            parent_id: None,
            starts_at: None,
            ends_at: None,
        },
    ))
    .await
    .unwrap();
    let mut move_task = update(&task.id, "stale-old");
    move_task.planning.cycle_id = Some(Some(c2.id.clone()));
    move_task
        .planning
        .expected_versions
        .insert(c1.id.clone(), old.version);
    assert!(s.update_task(move_task).await.is_err());
    let destination = s.object_get("b_default", &c2.id).await.unwrap();
    s.planning_execute(command(
        PlanningKind::Cycle,
        "edit-destination",
        PlanningMutation::Update {
            id: c2.id.clone(),
            title: Some("新端点被编辑".into()),
            body: None,
            parent_id: None,
            starts_at: None,
            ends_at: None,
        },
    ))
    .await
    .unwrap();
    let mut stale_destination = update(&task.id, "stale-destination");
    stale_destination.planning.cycle_id = Some(Some(c2.id.clone()));
    stale_destination
        .planning
        .expected_versions
        .insert(c2.id.clone(), destination.version);
    let before = counts(&s).await;
    assert!(s.update_task(stale_destination).await.is_err());
    let mut stale_catalog = update(&task.id, "stale-catalog");
    stale_catalog.planning.cycle_id = Some(Some(c2.id));
    stale_catalog.planning.expected_catalog_version =
        Some(s.object_catalog().await.unwrap().version - 1);
    assert!(s.update_task(stale_catalog).await.is_err());
    assert_eq!(s.get_task(&task.id).await.unwrap().cycle_id, Some(c1.id));
    assert_eq!(counts(&s).await, before);
    let mut competing = update(&task.id, "one");
    competing.title = Some("one".into());
    competing.expected_lock_version = Some(current.lock_version);
    competing.planning.expected_object_version = Some(current.object_version);
    let mut second = competing.clone();
    second.request_id = Some("two".into());
    second.title = Some("two".into());
    let (a, b) = tokio::join!(s.update_task(competing), s.update_task(second));
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(
        s.get_task(&task.id).await.unwrap().lock_version,
        current.lock_version + 1
    );
}

#[tokio::test]
async fn module_compound_updates_and_parent_creation_are_atomic_and_replayable() {
    let (_dir, s) = fixture().await;
    let parent = container(&s, PlanningKind::Module, "parent").await;
    let create = command(
        PlanningKind::Module,
        "child",
        PlanningMutation::Create {
            title: "child".into(),
            body: Some("body".into()),
            parent_id: Some(parent.id.clone()),
            starts_at: None,
            ends_at: None,
        },
    );
    let child = s.planning_execute(create.clone()).await.unwrap();
    assert_eq!(
        s.planning_execute(create).await.unwrap().object.id,
        child.object.id
    );
    let before = counts(&s).await;
    assert!(
        s.planning_execute(command(
            PlanningKind::Module,
            "cycle",
            PlanningMutation::Update {
                id: parent.id.clone(),
                title: Some("bad".into()),
                body: Some(Some("bad body".into())),
                parent_id: Some(Some(child.object.id.clone())),
                starts_at: None,
                ends_at: None
            }
        ))
        .await
        .is_err()
    );
    let current = s
        .planning_get("b_default", PlanningKind::Module, &parent.id)
        .await
        .unwrap();
    assert_eq!(current.title, parent.title);
    assert_eq!(current.body, parent.body);
    assert_eq!(counts(&s).await, before);
    let page = s
        .planning_list(
            "b_default",
            PlanningKind::Module,
            PlanningListOptions {
                parent_id: Some(parent.id),
                q: Some("child".into()),
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].id, child.object.id);
    assert!(page.items[0].body.is_none());
}

#[tokio::test]
async fn close_freezes_members_and_carries_without_touching_task_execution() {
    Box::pin(close_active_task_scenario()).await;
}

async fn close_active_task_scenario() {
    let (_dir, s) = fixture().await;
    let c1 = container(&s, PlanningKind::Cycle, "c1").await;
    let c2 = container(&s, PlanningKind::Cycle, "c2").await;
    let t1 = s
        .create_task(create("t_one", vec![], Some(c1.id.clone())))
        .await
        .unwrap();
    Box::pin(
        s.mark_execution_plan_not_required(crate::MarkExecutionPlanNotRequiredCommand {
            task_id: t1.id.clone(),
            reason: "测试持续运行的任务".into(),
            actor: "planning-test".into(),
        }),
    )
    .await
    .unwrap();
    Box::pin(s.promote_task(crate::PromoteTaskCommand {
        task_id: t1.id.clone(),
        actor: "planning-test".into(),
    }))
    .await
    .unwrap();
    let claim = Box::pin(s.claim_task(crate::ClaimTaskCommand {
        task_id: t1.id.clone(),
        actor: "planning-test".into(),
        ttl_ms: 60_000,
        worker_profile: None,
        metadata: serde_json::json!({}),
    }))
    .await
    .unwrap();
    let t1 = claim.task;
    assert!(t1.has_claim_token);
    assert_eq!(t1.cycle_id, Some(c1.id.clone()));
    s.create_task(create("t_two", vec![], Some(c1.id.clone())))
        .await
        .unwrap();
    let add = command(
        PlanningKind::Cycle,
        "exclusive",
        PlanningMutation::TaskAdd {
            id: c2.id.clone(),
            task_id: t1.id.clone(),
        },
    );
    assert!(s.planning_execute(add).await.is_err());
    s.planning_execute(command(
        PlanningKind::Cycle,
        "start",
        PlanningMutation::Start { id: c1.id.clone() },
    ))
    .await
    .unwrap();
    let close = command(
        PlanningKind::Cycle,
        "close",
        PlanningMutation::Close {
            id: c1.id.clone(),
            carry_to: Some(c2.id.clone()),
        },
    );
    s.planning_execute(close.clone()).await.unwrap();
    assert!(s.planning_execute(close).await.unwrap().receipt.replayed);
    let after = s.get_task(&t1.id).await.unwrap();
    assert_eq!(after.status, t1.status);
    assert_eq!(after.lock_version, t1.lock_version);
    assert_eq!(after.current_run_id, t1.current_run_id);
    assert_eq!(after.started_at, t1.started_at);
    assert_eq!(after.claim_owner, t1.claim_owner);
    assert_eq!(after.claim_expires_at, t1.claim_expires_at);
    assert!(after.has_claim_token);
    assert_eq!(s.get_run(&claim.run.id).await.unwrap(), claim.run);
    assert_eq!(after.cycle_id, Some(c2.id.clone()));
    let page = s
        .planning_members("b_default", PlanningKind::Cycle, &c1.id, 1, 1)
        .await
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.items.len(), 1);
    assert!(page.frozen);
    assert!(page.captured_at.is_some());
    let original = s
        .planning_members("b_default", PlanningKind::Cycle, &c1.id, 1, 0)
        .await
        .unwrap();
    let mut edit = update(&t1.id, "later");
    edit.title = Some("后来更名".into());
    s.update_task(edit).await.unwrap();
    let frozen = s
        .planning_members("b_default", PlanningKind::Cycle, &c1.id, 1, 0)
        .await
        .unwrap();
    assert_eq!(frozen.items[0].title, original.items[0].title);
    assert_eq!(
        s.planning_overview("b_default", PlanningKind::Cycle, &c1.id)
            .await
            .unwrap()
            .progress
            .total,
        2
    );
    assert_eq!(
        s.list_tasks(
            "default",
            TaskListOptions {
                cycle_id: Some(c1.id.clone()),
                ..Default::default()
            }
        )
        .await
        .unwrap()
        .total,
        0
    );
    assert!(
        s.create_task(create("t_frozen", vec![], Some(c1.id.clone())))
            .await
            .is_err()
    );
    s.planning_execute(command(
        PlanningKind::Cycle,
        "archive",
        PlanningMutation::Archive { id: c1.id.clone() },
    ))
    .await
    .unwrap();
    let restored = s
        .planning_execute(command(
            PlanningKind::Cycle,
            "restore",
            PlanningMutation::Restore { id: c1.id },
        ))
        .await
        .unwrap();
    assert_eq!(restored.object.status.as_deref(), Some("completed"));
    assert!(restored.object.archived_at.is_none());
    s.planning_execute(command(
        PlanningKind::Cycle,
        "cancel",
        PlanningMutation::Cancel { id: c2.id.clone() },
    ))
    .await
    .unwrap();
    assert!(s.get_task(&t1.id).await.unwrap().cycle_id.is_none());
    assert_eq!(s.get_run(&claim.run.id).await.unwrap(), claim.run);
}

#[tokio::test]
async fn canonical_filters_intersect_before_order_limit_and_total() {
    let (_dir, s) = fixture().await;
    let a = container(&s, PlanningKind::Module, "a").await;
    let b = container(&s, PlanningKind::Module, "b").await;
    let c = container(&s, PlanningKind::Cycle, "c").await;
    s.create_task(create("t_a", vec![a.id.clone()], Some(c.id.clone())))
        .await
        .unwrap();
    s.create_task(create("t_b", vec![b.id.clone()], Some(c.id.clone())))
        .await
        .unwrap();
    for id in ["t_ab1", "t_ab2"] {
        s.create_task(create(
            id,
            vec![a.id.clone(), b.id.clone()],
            Some(c.id.clone()),
        ))
        .await
        .unwrap();
    }
    let page = s
        .list_tasks(
            "default",
            TaskListOptions {
                module_ids: vec![a.id.clone(), b.id],
                cycle_id: Some(c.id),
                query: Some("t_ab2".into()),
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.tasks[0].id, "t_ab2");
    assert_eq!(page.tasks[0].module_ids.len(), 2);
    let overview = s
        .planning_overview("b_default", PlanningKind::Module, &a.id)
        .await
        .unwrap();
    assert_eq!(overview.progress.total, 3);
    let before = s.object_get("b_default", &a.id).await.unwrap();
    s.object_execute(ObjectCommand {
        board_id: "b_default".into(),
        request_id: "web-edit".into(),
        actor: "web".into(),
        mutation: ObjectMutation::Patch {
            patch: ObjectPatch {
                target: ObjectTarget {
                    id: a.id.clone(),
                    expected: before.version,
                },
                title: Some("Web 编辑".into()),
                edits: vec![],
                expected_catalog_version: None,
            },
        },
    })
    .await
    .unwrap();
    assert_eq!(
        s.planning_get("b_default", PlanningKind::Module, &a.id)
            .await
            .unwrap()
            .title,
        "Web 编辑"
    );
}
