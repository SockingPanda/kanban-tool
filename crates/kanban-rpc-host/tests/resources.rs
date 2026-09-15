//! G06 的 Board Hub 测量仅覆盖内存框架/demo；正式 Host 未装配 WatchBoard。
use kanban_live_core::*;
use kanban_rpc_host::{RpcApp, serve};
use kanban_rpc_proto::v1 as pb;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::watch;

struct Source {
    cards: Mutex<BTreeMap<String, Vec<Card>>>,
    reads: Mutex<BTreeMap<String, usize>>,
    changed: watch::Sender<u64>,
}

impl BoardSource for Source {
    fn subscribe_changes(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }
    fn load_board<'a>(&'a self, board: &'a str) -> BoxFuture<'a, Result<Vec<Card>>> {
        Box::pin(async move {
            *self.reads.lock().unwrap().entry(board.into()).or_default() += 1;
            self.cards
                .lock()
                .unwrap()
                .get(board)
                .cloned()
                .ok_or_else(|| LiveError::NotFound(board.into()))
        })
    }
}

impl TaskCommands for Source {
    fn update_title(&self, input: UpdateTitle) -> BoxFuture<'_, Result<Card>> {
        Box::pin(async move {
            let mut boards = self.cards.lock().unwrap();
            let cards = boards
                .get_mut(&input.board_id)
                .ok_or_else(|| LiveError::NotFound(input.board_id.clone()))?;
            let card = cards
                .iter_mut()
                .find(|card| card.id == input.task_id)
                .ok_or_else(|| LiveError::NotFound(input.task_id.clone()))?;
            if card.lock_version != input.expected_version {
                return Err(LiveError::Conflict(input.task_id));
            }
            card.title = input.title;
            card.lock_version += 1;
            let result = card.clone();
            drop(boards);
            self.changed.send_modify(|value| *value += 1);
            Ok(result)
        })
    }
}

type Stream = tonic::Streaming<pb::BoardFrame>;
type Client = pb::board_service_client::BoardServiceClient<tonic::transport::Channel>;

fn request(board: &str) -> pb::WatchBoardRequest {
    pb::WatchBoardRequest {
        board_id: board.into(),
        resume: None,
        protocol_version: 1,
    }
}

async fn next(stream: &mut Stream) -> pb::BoardFrame {
    tokio::time::timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

async fn snapshot(stream: &mut Stream) {
    let mut count = 0;
    loop {
        match next(stream).await.body.unwrap() {
            pb::board_frame::Body::SnapshotChunk(chunk) => count += chunk.tasks.len(),
            pb::board_frame::Body::SnapshotCommit(commit) => {
                assert_eq!(count, 256);
                assert_eq!(commit.count, 256);
                return;
            }
            _ => {}
        }
    }
}

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
        .unwrap()
}

fn report_samples(label: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let pick = |percent: usize| samples[(samples.len() * percent).div_ceil(100).saturating_sub(1)];
    println!(
        "G06_HUB {label} count={} p50_ms={:.3} p95_ms={:.3} max_ms={:.3}",
        samples.len(),
        pick(50),
        pick(95),
        samples.last().unwrap()
    );
}

async fn subscribe_retry(client: &mut Client, board: &str) -> (Stream, f64) {
    let started = Instant::now();
    let stream = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match client.watch_board(request(board)).await {
                Ok(response) => break response.into_inner(),
                Err(error) if error.code() == tonic::Code::ResourceExhausted => {
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

#[tokio::test]
async fn cancelling_demo_host_releases_a_backpressured_stream_with_client_still_alive() {
    let source = Arc::new(Source {
        cards: Mutex::new(BTreeMap::new()),
        reads: Mutex::new(BTreeMap::new()),
        changed: watch::channel(0).0,
    });
    let hub = Arc::new(Hub::new("b_a", Limits::default()).unwrap());
    hub.publish(
        (0..256)
            .map(|index| Card {
                id: format!("t_{index:04}"),
                title: "x".repeat(1024),
                status: Status::Todo,
                priority: 1,
                position: index,
                seq: index as u64 + 1,
                lock_version: 0,
            })
            .collect(),
    )
    .await
    .unwrap();
    let app = RpcApp::new(vec![hub.clone()], source, vec![]).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (_stop, receiver) = watch::channel(false);
    let server = tokio::spawn(serve(listener, app, receiver));
    let channel = tonic::transport::Endpoint::from_shared(url)
        .unwrap()
        .initial_stream_window_size(1024)
        .initial_connection_window_size(1024)
        .connect()
        .await
        .unwrap();
    let mut client = Client::new(channel);
    let stream = client
        .watch_board(request("b_a"))
        .await
        .unwrap()
        .into_inner();
    assert!(Arc::strong_count(&hub) > 1);
    let started = Instant::now();
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(1), async {
        while Arc::strong_count(&hub) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("Host future 取消必须回收连接，与客户端是否 drop 无关");
    println!(
        "G06_HUB_CANCEL released_ms={:.3} hub_refs=1 client_stream_still_held=true",
        started.elapsed().as_secs_f64() * 1000.0
    );
    drop(stream);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "G06 独立资源测量，使用 --exact --ignored --nocapture"]
async fn shared_board_hub_load_and_resource_measurement() {
    let cards = (0..256)
        .map(|index| Card {
            id: format!("t_{index:04}"),
            title: "a".repeat(1024),
            status: Status::Todo,
            priority: 1,
            position: index,
            seq: index as u64 + 1,
            lock_version: 0,
        })
        .collect::<Vec<_>>();
    let source = Arc::new(Source {
        cards: Mutex::new(BTreeMap::from([
            ("b_a".into(), cards.clone()),
            ("b_b".into(), cards),
        ])),
        reads: Mutex::new(BTreeMap::new()),
        changed: watch::channel(0).0,
    });
    let a = Arc::new(Hub::new("b_a", Limits::default()).unwrap());
    let b = Arc::new(Hub::new("b_b", Limits::default()).unwrap());
    let (stop, receiver) = watch::channel(false);
    let mut pumps = Vec::new();
    for hub in [&a, &b] {
        let mut changed = hub.subscribe();
        pumps.push(tokio::spawn(run_projection(
            source.clone(),
            hub.clone(),
            receiver.clone(),
        )));
        tokio::time::timeout(Duration::from_secs(5), changed.changed())
            .await
            .unwrap()
            .unwrap();
        hub.snapshot().await.unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = RpcApp::new(vec![a.clone(), b.clone()], source.clone(), vec![]).unwrap();
    let server = tokio::spawn(serve(listener, app, receiver));
    let baseline_rss = rss_kib();
    let mut client = Client::connect(url.clone()).await.unwrap();
    let mut streams = Vec::new();
    for board in ["b_a", "b_b"] {
        for _ in 0..8 {
            let mut stream = client
                .watch_board(request(board))
                .await
                .unwrap()
                .into_inner();
            snapshot(&mut stream).await;
            streams.push(stream);
        }
    }
    assert_eq!(
        source.changed.receiver_count(),
        2,
        "每看板一个 pump，客户端不增加 source 订阅"
    );
    let initial_reads = source.reads.lock().unwrap().clone();
    assert_eq!(
        initial_reads,
        BTreeMap::from([("b_a".into(), 1), ("b_b".into(), 1)])
    );
    let subscribed_rss = rss_kib();
    assert_eq!(
        client.watch_board(request("b_a")).await.unwrap_err().code(),
        tonic::Code::ResourceExhausted
    );
    let writer = pb::task_service_client::TaskServiceClient::connect(url.clone())
        .await
        .unwrap();
    let mut delays = Vec::new();
    let mut ack_delays = Vec::new();
    let mut after_ack = Vec::new();
    let mut frames = 0usize;
    for round in 0..8 {
        let started = Instant::now();
        let mut writes = tokio::task::JoinSet::new();
        for index in 0..8 {
            let mut writer = writer.clone();
            writes.spawn(async move {
                writer
                    .update_task_title(pb::UpdateTaskTitleRequest {
                        board_id: "b_a".into(),
                        task_id: format!("t_{index:04}"),
                        title: format!("round {round} index {index}"),
                        actor: "measurement".into(),
                        expected: Some(pb::ExpectedVersion { value: round }),
                    })
                    .await
                    .unwrap();
                Instant::now()
            });
        }
        let mut last_ack = started;
        while let Some(result) = writes.join_next().await {
            last_ack = last_ack.max(result.unwrap());
        }
        ack_delays.push(last_ack.duration_since(started).as_secs_f64() * 1000.0);
        for stream in &mut streams[..8] {
            let mut applied = BTreeMap::new();
            loop {
                let frame = next(stream).await;
                let Some(pb::board_frame::Body::Delta(delta)) = frame.body else {
                    panic!("需要增量")
                };
                frames += 1;
                for card in delta.upserts {
                    applied.insert(card.id, card.lock_version);
                }
                if (0..8).all(|index| applied.get(&format!("t_{index:04}")) == Some(&(round + 1))) {
                    let observed = Instant::now();
                    delays.push(observed.duration_since(started).as_secs_f64() * 1000.0);
                    after_ack.push(observed.duration_since(last_ack).as_secs_f64() * 1000.0);
                    break;
                }
            }
        }
    }
    assert_eq!(
        b.snapshot().await.unwrap().cursor.revision,
        1,
        "跨板全局提示只重读，不伪造 delta"
    );
    let loaded_rss = rss_kib();
    let reads_after = source.reads.lock().unwrap().clone();
    assert!(reads_after.values().sum::<usize>() < 2 * 64);
    let mut cancel = Vec::new();
    for _ in 0..16 {
        drop(streams.pop());
        let (mut replacement, elapsed) = subscribe_retry(&mut client, "b_a").await;
        snapshot(&mut replacement).await;
        streams.insert(0, replacement);
        cancel.push(elapsed);
    }
    drop(streams);
    let channel = tonic::transport::Endpoint::from_shared(url)
        .unwrap()
        .initial_stream_window_size(1024)
        .initial_connection_window_size(1024)
        .connect()
        .await
        .unwrap();
    let mut slow_client = Client::new(channel);
    let mut slow_streams = Vec::new();
    for _ in 0..16 {
        let (stream, _) = subscribe_retry(&mut slow_client, "b_a").await;
        slow_streams.push(stream);
    }
    let slow_rss = rss_kib();
    let started = Instant::now();
    drop(slow_streams.pop());
    let (replacement, slow_cancel_ms) = subscribe_retry(&mut slow_client, "b_a").await;
    slow_streams.push(replacement);
    let cancel_wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    let stopped = Instant::now();
    stop.send_replace(true);
    // 先保持客户端不读取，测真实 HTTP/2 背压下 server 的有界退出。
    let shutdown = tokio::time::timeout(Duration::from_secs(7), server)
        .await
        .unwrap()
        .unwrap();
    let shutdown_ms = stopped.elapsed().as_secs_f64() * 1000.0;
    for pump in pumps {
        pump.await.unwrap();
    }
    let exit_receivers = source.changed.receiver_count();
    assert_eq!(exit_receivers, 0);
    let held_clients_released = tokio::time::timeout(Duration::from_millis(250), async {
        while Arc::strong_count(&a) != 1 || Arc::strong_count(&b) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .is_ok();
    let held_refs = (Arc::strong_count(&a), Arc::strong_count(&b));
    drop(slow_streams);
    let release_started = Instant::now();
    tokio::time::timeout(Duration::from_secs(2), async {
        while Arc::strong_count(&a) != 1 || Arc::strong_count(&b) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let after_client_drop_ms = release_started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "G06_HUB scope=framework_demo_only boards=2 cards_per_board=256 title_bytes=1024 subscriptions_per_board=8 rounds=8 concurrent_writers=8 writes=64 source_receivers_active=2 source_receivers_after_stop={exit_receivers} initial_reads={initial_reads:?} reads_after={reads_after:?} frames={frames}"
    );
    report_samples("burst_start_to_applied_delta", delays);
    report_samples("write_ack_latency", ack_delays);
    report_samples("post_ack_delta_receive_latency", after_ack);
    report_samples("cancel_to_replacement_accepted", cancel);
    println!(
        "G06_HUB rss_kib baseline={baseline_rss} subscribed={subscribed_rss} loaded={loaded_rss} slow={slow_rss} stopped={} slow_streams=16 h2_stream_window_bytes=1024 slow_cancel_ms={slow_cancel_ms:.3} cancel_wall_ms={cancel_wall_ms:.3} shutdown_ms={shutdown_ms:.3} shutdown={shutdown:?} held_clients_released={held_clients_released} held_refs={held_refs:?} after_client_drop_ms={after_client_drop_ms:.3}",
        rss_kib()
    );
    assert!(
        held_clients_released,
        "Host 退出必须回收连接，不依赖客户端 drop"
    );
    assert_eq!(held_refs, (1, 1));
    assert!(shutdown_ms < 6000.0);
    if let Err(error) = shutdown {
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::TimedOut
        );
    }
}
