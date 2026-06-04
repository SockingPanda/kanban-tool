use std::path::Path;

use kanban_core::TaskStatus;
use kanban_sqlite::{
    CreateTask, DispatchOptions, FinishPolicy, TaskPatch, add_dependency, archive_task, block_task,
    claim_task, complete_task, create_task, dispatch_once, get_task, init_database, list_events,
    list_runs, list_tasks, unblock_task, update_task,
};

#[test]
fn task_crud_writes_events_and_hides_archived_by_default() {
    let temp = TempDb::new("task_crud_writes_events_and_hides_archived_by_default");
    init_database(&temp.path, "tester").unwrap();

    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "实现 Task CRUD".into(),
            description: Some("规格".into()),
            status: None,
            assignee: None,
            priority: 10,
            scheduled_at: None,
            due_at: None,
            metadata_json: "{}".into(),
        },
    )
    .unwrap();

    assert_eq!(task.seq, 1);
    assert_eq!(task.status, TaskStatus::Ready);
    assert_eq!(
        list_events(&temp.path, "default", Some(&task.id)).unwrap()[0].kind,
        "task.created"
    );

    let updated = update_task(
        &temp.path,
        "default",
        "tester",
        &task.id,
        TaskPatch {
            title: Some("实现 Task CRUD v0.5".into()),
            description: None,
            assignee: Some(Some("worker-a".into())),
            priority: Some(20),
            scheduled_at: None,
            due_at: None,
            metadata_json: None,
            expected_lock_version: Some(task.lock_version),
        },
    )
    .unwrap();
    assert_eq!(updated.title, "实现 Task CRUD v0.5");
    assert_eq!(updated.lock_version, task.lock_version + 1);

    archive_task(&temp.path, "default", "tester", &task.id, false).unwrap();
    assert!(
        list_tasks(&temp.path, "default", &[], false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        list_tasks(&temp.path, "default", &[], true).unwrap().len(),
        1
    );
}

#[test]
fn claim_complete_and_dependencies_promote_children() {
    let temp = TempDb::new("claim_complete_and_dependencies_promote_children");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("父任务")).unwrap();
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("子任务")).unwrap();

    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Todo
    );

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000).unwrap();
    assert_eq!(claim.task.status, TaskStatus::Running);
    assert!(!claim.claim_token.is_empty());
    assert!(claim.task.current_run_id.is_some());
    let heartbeat = kanban_sqlite::heartbeat_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        &claim.claim_token,
        600_000,
    )
    .unwrap();
    assert!(heartbeat.claim_expires_at > claim.task.claim_expires_at);

    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )
    .unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &parent.id).unwrap().status,
        TaskStatus::Done
    );
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Ready
    );
    assert_eq!(
        list_runs(&temp.path, "default", Some(&parent.id)).unwrap()[0].status,
        "succeeded"
    );
}

#[test]
fn block_unblock_recomputes_target_and_cycle_detection_rejects_cycles() {
    let temp = TempDb::new("block_unblock_recomputes_target_and_cycle_detection_rejects_cycles");
    init_database(&temp.path, "tester").unwrap();
    let parent = create_task(&temp.path, "default", "tester", CreateTask::ready("父任务")).unwrap();
    let child = create_task(&temp.path, "default", "tester", CreateTask::ready("子任务")).unwrap();
    add_dependency(&temp.path, "default", "tester", &parent.id, &child.id).unwrap();

    let err = add_dependency(&temp.path, "default", "tester", &child.id, &parent.id).unwrap_err();
    assert!(err.to_string().contains("cycle"));

    block_task(
        &temp.path,
        "default",
        "tester",
        &child.id,
        "等待输入",
        None,
        false,
    )
    .unwrap();
    let unblocked = unblock_task(&temp.path, "default", "tester", &child.id).unwrap();
    assert_eq!(unblocked.status, TaskStatus::Todo);

    let claim = claim_task(&temp.path, "default", "worker", &parent.id, 300_000).unwrap();
    complete_task(
        &temp.path,
        "default",
        "worker",
        &parent.id,
        Some(&claim.claim_token),
        false,
    )
    .unwrap();
    assert_eq!(
        get_task(&temp.path, "default", &child.id).unwrap().status,
        TaskStatus::Ready
    );
}

#[test]
fn dispatch_once_runs_ready_task_and_records_log() {
    let temp = TempDb::new("dispatch_once_runs_ready_task_and_records_log");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("跑 worker"),
    )
    .unwrap();
    let log_dir = temp.dir.join("logs");

    let result = dispatch_once(
        &temp.path,
        "default",
        DispatchOptions {
            actor: "dispatcher".into(),
            command: "sh -c 'echo task=$KB_TASK_ID; test -n \"$KB_CLAIM_TOKEN\"'".into(),
            worker_profile: "default".into(),
            claim_ttl_ms: 300_000,
            on_success: FinishPolicy::Done,
            on_failure: FinishPolicy::Blocked,
            log_dir: log_dir.clone(),
        },
    )
    .unwrap();

    assert_eq!(result.claimed, 1);
    assert_eq!(
        get_task(&temp.path, "default", &task.id).unwrap().status,
        TaskStatus::Done
    );
    let runs = list_runs(&temp.path, "default", Some(&task.id)).unwrap();
    assert_eq!(runs[0].status, "succeeded");
    let log_path = runs[0].log_path.as_ref().expect("run log path");
    assert!(std::fs::read_to_string(log_path).unwrap().contains("task="));
}

struct TempDb {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!("kb-v05-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kb.db");
        Self { dir, path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        if Path::new(&self.dir).exists() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}
