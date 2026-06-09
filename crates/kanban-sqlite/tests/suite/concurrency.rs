use crate::common::*;

#[test]
fn concurrent_claim_attempts_on_one_ready_task_have_exactly_one_success() {
    let temp = TempDb::new("concurrent_claim_attempts_on_one_ready_task_have_exactly_one_success");
    init_database(&temp.path, "tester").unwrap();
    let task = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("只允许一个 claim"),
    )
    .unwrap();

    let path = Arc::new(temp.path.clone());
    let task_id = Arc::new(task.id.clone());
    let barrier = Arc::new(Barrier::new(2));
    let mut handles = Vec::new();
    for actor in ["worker-a", "worker-b"] {
        let path = Arc::clone(&path);
        let task_id = Arc::clone(&task_id);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            claim_task(&*path, "default", actor, &task_id, 300_000)
        }));
    }

    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("claim thread should not panic"))
        .collect::<Vec<_>>();
    let successes = results.iter().filter(|result| result.is_ok()).count();
    let failures = results.iter().filter(|result| result.is_err()).count();
    assert_eq!(successes, 1, "results: {results:?}");
    assert_eq!(failures, 1, "results: {results:?}");

    let claimed = get_task(&temp.path, "default", &task.id).unwrap();
    assert_eq!(claimed.status, TaskStatus::Running);
    assert!(claimed.claim_token.is_some());
    assert_eq!(
        list_runs(&temp.path, "default", Some(&task.id))
            .unwrap()
            .iter()
            .filter(|run| run.status == "running")
            .count(),
        1
    );
}
