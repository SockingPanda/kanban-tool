use std::hint::black_box;
use std::path::PathBuf;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use kanban_application::api::{
    self, CreateTask, DispatchOptions, EventListOptions, FinishPolicy, TaskListOptions,
    TaskListSort,
};
use kanban_core::TaskStatus;
use kanban_sqlite::application::SqliteApplication;

const ACTOR: &str = "criterion";
const BOARD: &str = "default";

fn task_list_options(limit: usize, offset: usize) -> TaskListOptions {
    TaskListOptions {
        statuses: vec![TaskStatus::Ready],
        priorities: vec![],
        labels: vec![],
        plan_filters: vec![],
        include_archived: false,
        assignee: None,
        search: None,
        sort: TaskListSort::Seq,
        limit,
        offset,
    }
}

fn seed_database(name: &str) -> (tempfile::TempDir, PathBuf, SqliteApplication) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join(format!("{name}.db"));
    kanban_sqlite::init::init_database(&db_path, ACTOR).expect("init database");
    let app = SqliteApplication::new(db_path.clone());
    (dir, db_path, app)
}

fn create_ready_task(app: &SqliteApplication, title: impl Into<String>) -> api::TaskRecord {
    api::create_task(app, BOARD, ACTOR, CreateTask::ready(title)).expect("create task")
}

fn create_claimable_task(app: &SqliteApplication, title: impl Into<String>) -> api::TaskRecord {
    let task = create_ready_task(app, title);
    api::mark_execution_plan_not_required(app, BOARD, ACTOR, &task.id, "benchmark fixture")
        .expect("mark execution plan not required");
    task
}

fn seed_ready_tasks(app: &SqliteApplication, count: usize) {
    for index in 0..count {
        create_ready_task(app, format!("bench task {index:04}"));
    }
}

fn dispatch_options(log_dir: PathBuf) -> DispatchOptions {
    DispatchOptions {
        actor: "criterion-dispatcher".to_owned(),
        command: "true".to_owned(),
        worker_profile: "criterion-true".to_owned(),
        claim_ttl_ms: 300_000,
        heartbeat_interval_ms: 30_000,
        on_success: FinishPolicy::Done,
        on_failure: FinishPolicy::Blocked,
        log_dir,
    }
}

fn sqlite_create_task_ready(c: &mut Criterion) {
    let mut next_seq = 0usize;

    c.bench_function("sqlite_create_task_ready", |b| {
        b.iter_batched(
            || {
                next_seq += 1;
                let (dir, _db_path, app) =
                    seed_database(&format!("create_task_ready_{next_seq:06}"));
                (dir, app, next_seq)
            },
            |(_dir, app, seq)| {
                let task = api::create_task(
                    black_box(&app),
                    black_box(BOARD),
                    black_box(ACTOR),
                    CreateTask::ready(format!("created task {seq:06}")),
                )
                .expect("create ready task");
                black_box(task.id);
            },
            BatchSize::SmallInput,
        );
    });
}

fn sqlite_list_tasks_page_25_of_1000(c: &mut Criterion) {
    let (_dir, _db_path, app) = seed_database("list_tasks_page");
    seed_ready_tasks(&app, 1000);

    c.bench_function("sqlite_list_tasks_page_25_of_1000", |b| {
        b.iter(|| {
            let page = api::list_tasks_page(
                black_box(&app),
                black_box(BOARD),
                black_box(task_list_options(25, 0)),
            )
            .expect("list tasks page");
            black_box((page.tasks.len(), page.total));
        });
    });
}

fn sqlite_claim_task_ready(c: &mut Criterion) {
    let mut next_seq = 0usize;

    c.bench_function("sqlite_claim_task_ready", |b| {
        b.iter_batched(
            || {
                next_seq += 1;
                let (dir, _db_path, app) =
                    seed_database(&format!("claim_task_ready_{next_seq:06}"));
                let task = create_claimable_task(&app, format!("claimable task {next_seq:06}"));
                (dir, app, task)
            },
            |(_dir, app, task)| {
                let claim = api::claim_task(
                    black_box(&app),
                    black_box(BOARD),
                    black_box("criterion-worker"),
                    black_box(&task.id),
                    black_box(300_000),
                )
                .expect("claim task");
                black_box(claim.run_id);
            },
            BatchSize::SmallInput,
        );
    });
}

fn sqlite_dispatch_once_true_worker(c: &mut Criterion) {
    let mut next_seq = 0usize;

    c.bench_function("sqlite_dispatch_once_true_worker", |b| {
        b.iter_batched(
            || {
                next_seq += 1;
                let (dir, _db_path, app) =
                    seed_database(&format!("dispatch_once_true_worker_{next_seq:06}"));
                create_claimable_task(&app, format!("dispatch task {next_seq:06}"));
                (dir, app)
            },
            |(dir, app)| {
                let result = api::dispatch_once(
                    black_box(&app),
                    black_box(BOARD),
                    black_box(dispatch_options(dir.path().join("logs"))),
                )
                .expect("dispatch once");
                black_box((result.claimed, result.exit_code));
            },
            BatchSize::SmallInput,
        );
    });
}

fn sqlite_list_events_after_100_of_1000(c: &mut Criterion) {
    let (_dir, _db_path, app) = seed_database("list_events_after");
    for index in 0..1000 {
        api::create_task(
            &app,
            BOARD,
            ACTOR,
            CreateTask::ready(format!("event source task {index:04}")),
        )
        .expect("seed event source task");
    }

    c.bench_function("sqlite_list_events_after_100_of_1000", |b| {
        b.iter(|| {
            let events = api::list_events_after(
                black_box(&app),
                black_box(BOARD),
                black_box(EventListOptions {
                    task_ref: None,
                    after: 0,
                    limit: 100,
                }),
            )
            .expect("list events after");
            black_box(events.len());
        });
    });
}

criterion_group!(
    benches,
    sqlite_create_task_ready,
    sqlite_list_tasks_page_25_of_1000,
    sqlite_claim_task_ready,
    sqlite_dispatch_once_true_worker,
    sqlite_list_events_after_100_of_1000
);
criterion_main!(benches);
