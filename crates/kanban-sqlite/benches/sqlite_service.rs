use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use kanban_core::TaskStatus;
use kanban_sqlite::{CreateTask, TaskListOptions, TaskListSort};

fn task_list_options() -> TaskListOptions {
    TaskListOptions {
        statuses: vec![TaskStatus::Todo],
        priorities: vec![],
        labels: vec![],
        plan_filters: vec![],
        include_archived: false,
        assignee: None,
        search: None,
        sort: TaskListSort::Seq,
        limit: 25,
        offset: 0,
    }
}

fn seed_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("bench.db");
    kanban_sqlite::init_database(&db_path, "criterion").expect("init database");
    for index in 0..25 {
        kanban_sqlite::create_task(
            &db_path,
            "default",
            "criterion",
            CreateTask::ready(format!("bench task {index:02}")),
        )
        .expect("seed task");
    }
    (dir, db_path)
}

fn sqlite_list_tasks_page(c: &mut Criterion) {
    let (_dir, db_path) = seed_database();

    c.bench_function("sqlite_list_tasks_page_25", |b| {
        b.iter(|| {
            let page = kanban_sqlite::list_tasks_page(
                black_box(&db_path),
                black_box("default"),
                task_list_options(),
            )
            .expect("list tasks page");
            black_box(page.total);
        });
    });
}

criterion_group!(benches, sqlite_list_tasks_page);
criterion_main!(benches);
