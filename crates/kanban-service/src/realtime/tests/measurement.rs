use std::{collections::BTreeSet, time::Instant};

use serde_json::{Value, json};
use tokio::sync::Barrier;

use super::*;

fn distribution(mut samples: Vec<Duration>) -> Value {
    samples.sort_unstable();
    let percentile = |percent: usize| {
        samples[(samples.len() * percent).div_ceil(100).saturating_sub(1)].as_secs_f64() * 1_000.0
    };
    json!({
        "samples": samples.len(),
        "unit": "ms",
        "p50": percentile(50),
        "p95": percentile(95),
        "max": samples.last().unwrap().as_secs_f64() * 1_000.0,
    })
}

fn process_memory() -> Value {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return Value::Null;
    };
    let kib = |name: &str| {
        status.lines().find_map(|line| {
            line.strip_prefix(name)?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        })
    };
    json!({"rss_kib": kib("VmRSS:"), "peak_rss_kib": kib("VmHWM:")})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore = "显式运行 G06 真 Turso 负载，输出 gate 等待与完整分页读取的当前机器测量"]
async fn actual_service_gate_wait_and_consistent_read_measurement() {
    const INITIAL_TASKS: usize = 1_001;
    const WRITERS: usize = 4;
    const READERS: usize = 4;
    const WRITES_PER_WORKER: usize = 32;
    const READS_PER_WORKER: usize = 8;
    let (_directory, service) = service("realtime-measurement").await;
    for index in 1..INITIAL_TASKS {
        service
            .create_task(task_command(&format!("t_seed_{index}")))
            .await
            .unwrap();
    }
    // 单独报告无竞争读取，避免把排队时间误称为 SQL/构造快照的耗时。
    let mut uncontended = Vec::new();
    for _ in 0..8 {
        let started = Instant::now();
        assert_eq!(
            service
                .read_test_task_summary("default")
                .await
                .unwrap()
                .tasks
                .len(),
            INITIAL_TASKS
        );
        uncontended.push(started.elapsed());
    }
    let memory_before = process_memory();
    let barrier = Arc::new(Barrier::new(WRITERS + READERS));
    let mut workers = tokio::task::JoinSet::new();
    service.mutation_gate.start_wait_measurement();
    let started = Instant::now();
    for worker in 0..WRITERS {
        let service = service.clone();
        let barrier = barrier.clone();
        workers.spawn(async move {
            barrier.wait().await;
            let mut samples = Vec::new();
            for index in 0..WRITES_PER_WORKER {
                let started = Instant::now();
                if index % 8 == 0 {
                    service
                        .create_task(task_command(&format!("t_concurrent_{worker}_{index}")))
                        .await
                        .unwrap();
                } else {
                    service
                        .create_comment(comment_command(&format!("load-{worker}-{index}")))
                        .await
                        .unwrap();
                }
                samples.push(started.elapsed());
            }
            (false, samples)
        });
    }
    for _ in 0..READERS {
        let service = service.clone();
        let barrier = barrier.clone();
        workers.spawn(async move {
            barrier.wait().await;
            let mut samples = Vec::new();
            for _ in 0..READS_PER_WORKER {
                let started = Instant::now();
                let snapshot = service.read_test_task_summary("default").await.unwrap();
                samples.push(started.elapsed());
                assert!(snapshot.tasks.len() >= INITIAL_TASKS);
                let ids = snapshot
                    .tasks
                    .iter()
                    .map(|task| &task.id)
                    .collect::<BTreeSet<_>>();
                assert_eq!(ids.len(), snapshot.tasks.len(), "分页不得重复或漏读");
                let mut sequences = snapshot
                    .tasks
                    .iter()
                    .map(|task| task.seq)
                    .collect::<Vec<_>>();
                sequences.sort_unstable();
                assert_eq!(
                    sequences,
                    (1..=snapshot.tasks.len() as i64).collect::<Vec<_>>()
                );
            }
            (true, samples)
        });
    }
    let mut read_durations = Vec::new();
    let mut write_durations = Vec::new();
    while let Some(result) = workers.join_next().await {
        let (read, samples) = result.unwrap();
        if read {
            read_durations.extend(samples);
        } else {
            write_durations.extend(samples);
        }
    }
    let elapsed = started.elapsed();
    let waits = service.mutation_gate.take_wait_measurement();
    let (read_waits, write_waits): (Vec<_>, Vec<_>) =
        waits.into_iter().partition(|sample| sample.read);
    assert_eq!(read_waits.len(), READERS * READS_PER_WORKER);
    assert_eq!(write_waits.len(), WRITERS * WRITES_PER_WORKER);
    let final_snapshot = service.read_test_task_summary("default").await.unwrap();
    assert_eq!(
        final_snapshot.tasks.len(),
        INITIAL_TASKS + WRITERS * WRITES_PER_WORKER / 8
    );
    assert_eq!(
        service.list_comments(TASK).await.unwrap().len(),
        WRITERS * WRITES_PER_WORKER * 7 / 8
    );
    let memory_after_full_snapshot = process_memory();
    println!(
        "G06_SERVICE_MEASUREMENT {}",
        json!({
            "profile": "cargo test dev, cfg(test) gate acquisition timer",
            "initial_tasks": INITIAL_TASKS,
            "final_tasks": final_snapshot.tasks.len(),
            "page_limit": 1_000,
            "concurrency": WRITERS + READERS,
            "writers": WRITERS,
            "readers": READERS,
            "task_creates": WRITERS * WRITES_PER_WORKER / 8,
            "comment_creates": WRITERS * WRITES_PER_WORKER * 7 / 8,
            "snapshots": READERS * READS_PER_WORKER,
            "wall_ms": elapsed.as_secs_f64() * 1_000.0,
            "uncontended_snapshot": distribution(uncontended),
            "concurrent_snapshot_including_gate_wait": distribution(read_durations),
            "write_including_gate_wait": distribution(write_durations),
            "read_gate_wait": distribution(read_waits.into_iter().map(|sample| sample.elapsed).collect()),
            "write_gate_wait": distribution(write_waits.into_iter().map(|sample| sample.elapsed).collect()),
            "memory_before": memory_before,
            "memory_after_full_snapshot": memory_after_full_snapshot,
        })
    );
}
