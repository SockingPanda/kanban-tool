//! 需单独运行的负载采样，避免把其他 libtest 的内存计入 Host 测量。
use super::*;
use futures_util::future::join_all;
use std::time::Instant;

fn rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn percentiles(mut samples: Vec<f64>) -> Value {
    samples.sort_by(f64::total_cmp);
    let pick = |percent: usize| samples[(samples.len() * percent).div_ceil(100).saturating_sub(1)];
    json!({"count":samples.len(), "p50_ms":pick(50), "p95_ms":pick(95), "max_ms":samples.last().unwrap()})
}

async fn subscribe_retry(host: &Host, board: &str) -> (Stream, f64) {
    let started = Instant::now();
    let mut workspace =
        pb::workspace_service_client::WorkspaceServiceClient::connect(host.url.clone())
            .await
            .unwrap();
    let stream = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match workspace
                .watch_changes(pb::WatchChangesRequest {
                    board_id: board.into(),
                    protocol_version: 1,
                })
                .await
            {
                Ok(response) => break response.into_inner(),
                Err(error) if error.code() == Code::ResourceExhausted => {
                    tokio::task::yield_now().await
                }
                Err(error) => panic!("重新订阅失败: {error}"),
            }
        }
    })
    .await
    .unwrap();
    (stream, started.elapsed().as_secs_f64() * 1000.0)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "G06 资源测量需用 --exact --ignored --nocapture 单独运行"]
async fn formal_watch_load_and_resource_measurement() {
    let host = Host::start().await;
    let mut writer = client(&host).await;
    let task = create(&mut writer, &host.board, "并发刷新测量").await;
    let other = create_board(&mut writer, "unrelated-measurement").await;
    let baseline_rss = rss_kib();
    let mut streams = Vec::new();
    for board in [&host.board, &other] {
        for _ in 0..8 {
            let mut stream = watch(&host, board).await;
            next(&mut stream).await;
            streams.push(stream);
        }
    }
    let initial_checks = host.state.grpc_probe.checks();
    assert_eq!(initial_checks.get(&host.board), Some(&8));
    assert_eq!(initial_checks.get(&other), Some(&8));
    let subscribed_rss = rss_kib();
    let mut workspace =
        pb::workspace_service_client::WorkspaceServiceClient::connect(host.url.clone())
            .await
            .unwrap();
    assert_eq!(
        workspace
            .watch_changes(pb::WatchChangesRequest {
                board_id: host.board.clone(),
                protocol_version: 1
            })
            .await
            .unwrap_err()
            .code(),
        Code::ResourceExhausted
    );

    let heartbeat_started = Instant::now();
    let heartbeat_ms = join_all(streams.iter_mut().map(|stream| async move {
        let frame = tokio::time::timeout(Duration::from_secs(17), stream.message())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(frame.sequence, 1);
        assert!(matches!(
            frame.body,
            Some(pb::workspace_change_frame::Body::Heartbeat(_))
        ));
        heartbeat_started.elapsed().as_secs_f64() * 1000.0
    }))
    .await;
    assert_eq!(
        host.state.grpc_probe.checks(),
        initial_checks,
        "心跳不重新读取 source"
    );

    let http = reqwest::Client::new();
    let mut complete_ms = Vec::new();
    let mut after_ack_ms = Vec::new();
    let mut ack_ms = Vec::new();
    let mut before_ack = 0;
    let mut frames = 0usize;
    for round in 0..8 {
        let expected = (round + 1) * 8;
        let started = Instant::now();
        let writes = join_all((0..8).map(|index| {
            let mut writer = writer.clone();
            let http = http.clone();
            let url = host.url.clone();
            let id = task.id.clone();
            async move {
                let body = format!("round {round} writer {index}");
                if index % 2 == 0 {
                    comment(&mut writer, &id, &body).await;
                } else {
                    let response = http.post(format!("{url}/api/v1/tasks/{id}/comments"))
                        .header("content-type", "application/json")
                        .body(json!({"author":"load", "author_type":"agent", "kind":"note", "body":body}).to_string())
                        .send().await.unwrap();
                    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
                }
                Instant::now()
            }
        }));
        let reads = join_all(streams.iter_mut().map(|stream| {
            let mut reader = writer.clone();
            let id = task.id.clone();
            async move {
                let mut seen = 0usize;
                loop {
                    let frame = next(stream).await;
                    assert!(matches!(
                        frame.body,
                        Some(pb::workspace_change_frame::Body::Invalidated(_))
                    ));
                    seen += 1;
                    // 全局提示没有 mutation ID；随后用真实 typed query 确认该轮已可见。
                    if details(&mut reader, &id).await.data.comments.len() >= expected {
                        break (Instant::now(), seen);
                    }
                }
            }
        }));
        let (acknowledgements, observed) = tokio::join!(writes, reads);
        let acknowledged = *acknowledgements.iter().max().unwrap();
        ack_ms.push(acknowledged.duration_since(started).as_secs_f64() * 1000.0);
        for (at, seen) in observed {
            frames += seen;
            complete_ms.push(at.duration_since(started).as_secs_f64() * 1000.0);
            if at >= acknowledged {
                after_ack_ms.push(at.duration_since(acknowledged).as_secs_f64() * 1000.0);
            } else {
                before_ack += 1;
            }
        }
    }
    assert_eq!(details(&mut writer, &task.id).await.data.comments.len(), 64);
    let loaded_rss = rss_kib();
    let after_load = host.state.grpc_probe.checks();
    let source_reads: usize =
        after_load.values().sum::<usize>() - initial_checks.values().sum::<usize>();
    assert!(source_reads < 16 * 64, "连续写入应合并而非每订阅每写读取");
    assert!(host.state.grpc_probe.loads().is_empty());

    let mut release_ms = Vec::new();
    for index in 0..16 {
        drop(streams.pop());
        let (mut replacement, elapsed) = subscribe_retry(&host, &host.board).await;
        next(&mut replacement).await;
        release_ms.push(elapsed);
        // 新流保持不消费，后续并发写用于验证慢消费者仍可取消和退出。
        streams.insert(0, replacement);
        assert_eq!(streams.len(), 16, "第 {index} 次替换后活跃订阅仍为 16");
    }
    let before_slow = host.state.grpc_probe.checks().values().sum::<usize>();
    for index in 0..32 {
        comment(&mut writer, &task.id, &format!("slow {index}")).await;
    }
    let slow_checks = host.state.grpc_probe.checks().values().sum::<usize>() - before_slow;
    let slow_rss = rss_kib();
    let shutdown_start = Instant::now();
    host.stop.send_replace(crate::ShutdownSignal::Graceful);
    let mut pending = 0usize;
    for stream in &mut streams {
        while tokio::time::timeout(Duration::from_secs(5), stream.message())
            .await
            .unwrap()
            .unwrap()
            .is_some()
        {
            pending += 1;
        }
    }
    tokio::time::timeout(Duration::from_secs(5), host.server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let shutdown_ms = shutdown_start.elapsed().as_secs_f64() * 1000.0;
    let stopped_checks = host.state.grpc_probe.checks();
    // 停机后写隔离 service，只用于证明已结束流不再持有 source 订阅。
    drop(streams);
    let probe = host.state.grpc_probe.clone();
    let service = host.state.application().clone();
    service
        .create_comment(kanban_service::CreateCommentCommand {
            task_id: task.id.clone(),
            idempotency_key: None,
            author: "after-stop".into(),
            author_type: kanban_service::CommentAuthorType::Agent,
            agent_type: None,
            body: "停机后仍可使用隔离 service".into(),
            kind: kanban_service::CommentKind::Note,
            metadata: Default::default(),
        })
        .await
        .unwrap();
    assert_eq!(probe.checks(), stopped_checks);
    println!(
        "G06_FORMAL_RESOURCE {}",
        json!({
            "scope":"正式 /kanban.v1.WorkspaceService/WatchChanges + 单进程真实 Turso Host",
            "load":{"boards":2,"subscriptions_per_board":8,"rounds":8,"concurrent_writers":8,"successful_writes":64,"native_writes":32,"rest_writes":32},
            "source":{"initial_checks":initial_checks,"after_load_checks":after_load,"refresh_checks":source_reads,"uncoalesced_upper_bound":16*64,"full_board_loads":probe.loads(),"received_frames":frames},
        "write_ack_latency":percentiles(ack_ms),
        "idle_heartbeat":percentiles(heartbeat_ms),
            "burst_start_to_validated_refresh":percentiles(complete_ms),
            "post_ack_validated_refresh_latency":percentiles(after_ack_ms),
            "refresh_observed_before_last_rpc_ack":before_ack,
            "cancel_to_replacement_accepted":percentiles(release_ms),
            "slow_consumer":{"unread_streams":16,"writes":32,"source_checks_before_shutdown":slow_checks,"buffered_frames_drained":pending,"shutdown_ms":shutdown_ms},
            "rss_kib":{"baseline":baseline_rss,"subscribed":subscribed_rss,"loaded":loaded_rss,"slow":slow_rss,"stopped":rss_kib()},
            "limits":"RSS 为单独 libtest 进程（含 Host、client 和测试数据），不是独立生产进程；刷新延迟含随后一次真实 typed query 的可见性确认；slow 阶段证明不读取客户端的取消和退出，未声称触发 HTTP/2 窗口耗尽。"
        })
    );
}
