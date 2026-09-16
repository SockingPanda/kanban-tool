use super::*;
use crate::AppState;
use futures_util::StreamExt;
use kanban_protocol::{self as dto, rpc::v1::query_definition::Query};
use prost::Message;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

async fn fixture() -> (tempfile::TempDir, AppState, QueryRuntime) {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("query.db"), "query-test")
        .await
        .unwrap();
    let runtime = QueryRuntime::new(state.clone());
    (directory, state, runtime)
}

async fn create(state: &AppState, title: &str) -> dto::ApiTask {
    crate::application::tasks::create::create_task(
        state.clone(),
        dto::CreateTaskPath {
            board: "default".into(),
        },
        Default::default(),
        serde_json::from_value(
            serde_json::json!({"title": title, "description": "完整字段", "status": "todo"}),
        )
        .unwrap(),
    )
    .await
    .unwrap()
    .data
}

async fn comment(state: &AppState, task: &str, body: &str) {
    state
        .application()
        .create_comment(kanban_service::operations::CreateCommentCommand {
            task_id: task.into(),
            idempotency_key: None,
            author: "query-test".into(),
            author_type: kanban_service::CommentAuthorType::User,
            agent_type: None,
            body: body.into(),
            kind: kanban_service::CommentKind::Note,
            metadata: Default::default(),
        })
        .await
        .unwrap();
}

fn definition(id: &str, query: Query) -> pb::QueryDefinition {
    pb::QueryDefinition {
        client_query_id: id.into(),
        projection_version: 1,
        query: Some(query),
        resume: None,
        refresh: false,
    }
}

fn list(board: &str) -> Query {
    Query::ListTasks(pb::ListTasksRequest {
        board: Some(board.into()),
        ..Default::default()
    })
}

async fn watch(
    runtime: &QueryRuntime,
    queries: Vec<pb::QueryDefinition>,
) -> <QueryRuntime as QueryService>::WatchQueriesStream {
    runtime
        .watch_queries(Request::new(pb::WatchQueriesRequest {
            protocol_version: 1,
            queries,
        }))
        .await
        .unwrap()
        .into_inner()
}

#[derive(Default)]
struct Rebuild {
    bytes: BTreeMap<String, Vec<u8>>,
    cursors: BTreeMap<String, pb::QueryCursor>,
    pending: BTreeMap<String, (pb::QueryBegin, Vec<u8>, u32)>,
}

impl Rebuild {
    fn apply(&mut self, frame: pb::QueryFrame) -> bool {
        let id = frame.client_query_id;
        match frame.body.unwrap() {
            Body::Begin(begin) => {
                if !begin.snapshot {
                    assert_eq!(begin.base.as_ref(), self.cursors.get(&id));
                }
                assert_eq!(begin.result_sha256.len(), 32);
                self.pending.insert(id, (begin, Vec::new(), 0));
            }
            Body::Chunk(chunk) => {
                let (_, bytes, count) = self.pending.get_mut(&id).unwrap();
                assert_eq!(chunk.index, *count);
                assert!(chunk.data.len() <= kanban_protocol::rpc::query::QUERY_CHUNK_BYTES);
                *count += 1;
                bytes.extend(chunk.data);
            }
            Body::End(end) => {
                let (begin, patch, count) = self.pending.remove(&id).unwrap();
                assert_eq!(count, begin.chunk_count);
                assert_eq!(patch.len(), begin.patch_size as usize);
                assert_eq!(begin.cursor, end.cursor);
                let bytes = if begin.snapshot {
                    patch
                } else {
                    let mut bytes = self.bytes.get(&id).unwrap().clone();
                    let start = begin.offset as usize;
                    bytes.splice(start..start + begin.delete_length as usize, patch);
                    bytes
                };
                assert_eq!(bytes.len(), begin.result_size as usize);
                assert_eq!(Sha256::digest(&bytes).as_slice(), begin.result_sha256);
                pb::QueryResult::decode(bytes.as_slice()).unwrap();
                self.bytes.insert(id.clone(), bytes);
                self.cursors.insert(id, end.cursor.unwrap());
                return true;
            }
            Body::Failure(error) => panic!("unexpected failure: {error:?}"),
            Body::Ready(_) | Body::Heartbeat(_) => {}
        }
        false
    }

    async fn receive(
        &mut self,
        stream: &mut <QueryRuntime as QueryService>::WatchQueriesStream,
        count: usize,
    ) {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut committed = 0;
            while committed < count {
                committed += usize::from(self.apply(stream.next().await.unwrap().unwrap()));
            }
        })
        .await
        .unwrap();
    }

    async fn ready(
        &mut self,
        stream: &mut <QueryRuntime as QueryService>::WatchQueriesStream,
        id: &str,
    ) -> pb::QueryCursor {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let frame = stream.next().await.unwrap().unwrap();
                if frame.client_query_id == id
                    && let Some(Body::Ready(ready)) = &frame.body
                {
                    return ready.cursor.clone().unwrap();
                }
                self.apply(frame);
            }
        })
        .await
        .unwrap()
    }
}

#[tokio::test]
async fn canonical_defaults_share_one_read_and_unmount_releases_every_hub() {
    let (_directory, state, runtime) = fixture().await;
    let board = state.application().get_board("default").await.unwrap().id;
    let first = source::normalize(&state, list(" default ")).await.unwrap();
    let second = Query::ListTasks(
        pb::ListTasksRequest::from_parts(
            dto::ListTasksPath { board },
            dto::ListTasksQuery::default(),
            (),
        )
        .unwrap(),
    );
    let second = source::normalize(&state, second).await.unwrap();
    let a = runtime.attach(first).unwrap();
    let b = runtime.attach(second).unwrap();
    assert!(Arc::ptr_eq(&a.shared, &b.shared));
    let mut changes = a.shared.hub.subscribe();
    tokio::time::timeout(Duration::from_secs(5), async {
        while a.shared.hub.next(None).is_err() {
            changes.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(a.shared.reads.load(Ordering::Relaxed), 1);
    assert_eq!(state.grpc_probe.queries().values().sum::<usize>(), 1);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(a.shared.reads.load(Ordering::Relaxed), 1);
    drop(a);
    assert_eq!(runtime.counts().0, 1);
    drop(b);
    assert_eq!(runtime.counts().0, 0);
    runtime.stop().await;
    assert_eq!(runtime.counts(), (0, 0, 0));
}

#[tokio::test]
async fn idle_heartbeats_leave_query_unchanged_and_cancel_reclaims_resources() {
    let (_directory, state, runtime) = fixture().await;
    let query = list("default");
    let mut stream = watch(&runtime, vec![definition("list", query.clone())]).await;
    let mut rebuilt = Rebuild::default();
    let cursor = rebuilt.ready(&mut stream, "list").await;
    assert_eq!(rebuilt.cursors["list"], cursor);
    let lease = runtime
        .attach(source::normalize(&state, query).await.unwrap())
        .unwrap();
    let shared = lease.shared.clone();
    drop(lease);
    let resume = from_cursor(cursor.clone());
    let reads = shared.reads.load(Ordering::Relaxed);
    let application_reads = state.grpc_probe.queries();
    let resources = runtime.counts();
    assert_eq!(reads, 1);
    assert_eq!(resources.0, 1);
    assert!(resources.2 > 0);
    assert_eq!(
        runtime.0.connections.available_permits(),
        MAX_CONNECTIONS - 1
    );

    let started = Instant::now();
    for _ in 0..3 {
        let waiting = Instant::now();
        let frame = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("静默查询必须及时发送周期心跳")
            .unwrap()
            .unwrap();
        assert!(waiting.elapsed() >= Duration::from_millis(100));
        assert!(frame.client_query_id.is_empty());
        assert!(matches!(frame.body, Some(Body::Heartbeat(_))));
        assert!(matches!(
            shared.hub.next(Some(&resume)),
            Ok(QueryNext::Idle)
        ));
        assert_eq!(shared.reads.load(Ordering::Relaxed), reads);
        assert_eq!(state.grpc_probe.queries(), application_reads);
        assert_eq!(runtime.counts(), resources);
    }
    let elapsed = started.elapsed();

    // 在下一次 idle 等待已经开始后取消，确认计时器没有成为投影资源的独立 owner。
    assert!(futures_util::poll!(stream.next()).is_pending());
    drop(stream);
    assert_eq!(runtime.counts().0, 0);
    assert_eq!(runtime.counts().2, 0);
    assert_eq!(runtime.0.connections.available_permits(), MAX_CONNECTIONS);
    runtime.stop().await;
    assert_eq!(runtime.counts(), (0, 0, 0));
    assert_eq!(shared.reads.load(Ordering::Relaxed), reads);
    assert_eq!(state.grpc_probe.queries(), application_reads);
    eprintln!(
        "G08_QUERY_IDLE_HEARTBEAT count=3 elapsed_ms={:.3} application_reads={reads} revision={} hubs_after_cancel=0 available_permits={MAX_CONNECTIONS} retained_bytes=0",
        elapsed.as_secs_f64() * 1000.0,
        cursor.revision
    );
}

#[tokio::test]
async fn full_details_comments_and_page_boundary_rebuild_authoritative_query() {
    let (_directory, state, runtime) = fixture().await;
    let task = create(&state, "beta").await;
    create(&state, "gamma").await;
    let page = Query::ListTasks(
        pb::ListTasksRequest::from_parts(
            dto::ListTasksPath {
                board: "default".into(),
            },
            dto::ListTasksQuery {
                limit: 1,
                offset: 1,
                sort: dto::TaskReadSort::Title,
                ..Default::default()
            },
            (),
        )
        .unwrap(),
    );
    let details = Query::GetTaskDetails(pb::GetTaskDetailsRequest {
        task_id: Some(task.id.clone()),
    });
    let comments = Query::ListComments(pb::ListCommentsRequest {
        task_id: Some(task.id.clone()),
    });
    let mut stream = watch(
        &runtime,
        vec![
            definition("page", page.clone()),
            definition("details", details.clone()),
            definition("comments", comments.clone()),
        ],
    )
    .await;
    let mut rebuilt = Rebuild::default();
    rebuilt.receive(&mut stream, 3).await;
    for (id, query) in [
        ("page", page.clone()),
        ("details", details.clone()),
        ("comments", comments.clone()),
    ] {
        assert_eq!(
            rebuilt.bytes[id],
            source::read(state.clone(), query).await.unwrap()
        );
    }
    comment(&state, &task.id, "不改变七字段卡片的评论").await;
    rebuilt.receive(&mut stream, 2).await;
    assert_eq!(
        rebuilt.bytes["details"],
        source::read(state.clone(), details).await.unwrap()
    );
    assert_eq!(
        rebuilt.bytes["comments"],
        source::read(state.clone(), comments).await.unwrap()
    );
    create(&state, "alpha").await;
    rebuilt.receive(&mut stream, 1).await;
    assert_eq!(
        rebuilt.bytes["page"],
        source::read(state.clone(), page).await.unwrap()
    );
    let pb::query_result::Result::ListTasks(result) =
        pb::QueryResult::decode(rebuilt.bytes["page"].as_slice())
            .unwrap()
            .result
            .unwrap()
    else {
        panic!("page")
    };
    let page: dto::ListTasksResponse = result.try_into().unwrap();
    assert_eq!(page.meta.total, 3);
    assert_eq!(page.data[0].title, "beta");
    drop(stream);
    runtime.stop().await;
    assert_eq!(runtime.counts(), (0, 0, 0));
}

#[tokio::test]
async fn resume_current_refresh_confirms_after_new_read_without_revision_change() {
    let (_directory, _state, runtime) = fixture().await;
    let query = list("default");
    let mut original = watch(&runtime, vec![definition("list", query.clone())]).await;
    let mut rebuilt = Rebuild::default();
    let cursor = rebuilt.ready(&mut original, "list").await;
    let lease = runtime
        .attach(
            source::normalize(&runtime.0.state, query.clone())
                .await
                .unwrap(),
        )
        .unwrap();
    let before = lease.shared.reads.load(Ordering::Relaxed);
    let mut refresh = definition("list", query);
    refresh.resume = Some(cursor.clone());
    refresh.refresh = true;
    let mut refreshed = watch(&runtime, vec![refresh]).await;
    let after = rebuilt.ready(&mut refreshed, "list").await;
    assert_eq!(cursor, after);
    assert_eq!(lease.shared.reads.load(Ordering::Relaxed), before + 1);
    drop(refreshed);
    drop(original);
    drop(lease);
    runtime.stop().await;
}

#[tokio::test]
async fn one_query_failure_preserves_other_queries_and_deadline_releases_resources() {
    let (_directory, _state, runtime) = fixture().await;
    let mut request = Request::new(pb::WatchQueriesRequest {
        protocol_version: 1,
        queries: vec![
            definition(
                "missing",
                Query::GetTask(pb::GetTaskRequest {
                    task_id: Some("t_missing".into()),
                    include: None,
                }),
            ),
            definition("good", list("default")),
        ],
    });
    request.set_timeout(Duration::from_millis(250));
    let mut stream = runtime.watch_queries(request).await.unwrap().into_inner();
    let mut failure = false;
    let mut ready = false;
    while let Some(frame) = stream.next().await {
        match frame {
            Ok(frame) => match frame.body.unwrap() {
                Body::Failure(error) => {
                    assert_eq!(frame.client_query_id, "missing");
                    assert_eq!(
                        error.error.unwrap().code,
                        pb::DtoApiErrorCode::NotFound as i32
                    );
                    failure = true;
                }
                Body::Ready(_) => {
                    assert_eq!(frame.client_query_id, "good");
                    ready = true;
                }
                _ => {}
            },
            Err(error) => {
                assert_eq!(error.code(), tonic::Code::DeadlineExceeded);
                break;
            }
        }
    }
    assert!(failure && ready);
    assert_eq!(runtime.counts().0, 0);
    drop(stream);
    runtime.stop().await;
}

#[tokio::test]
async fn connection_query_limits_and_active_shutdown_are_bounded() {
    let (_directory, _state, runtime) = fixture().await;
    let mut streams = Vec::new();
    for _ in 0..MAX_CONNECTIONS {
        streams.push(watch(&runtime, vec![definition("list", list("default"))]).await);
    }
    let rejected = runtime
        .watch_queries(Request::new(pb::WatchQueriesRequest {
            protocol_version: 1,
            queries: vec![definition("list", list("default"))],
        }))
        .await;
    assert_eq!(
        rejected.err().unwrap().code(),
        tonic::Code::ResourceExhausted
    );
    let invalid = runtime
        .watch_queries(Request::new(pb::WatchQueriesRequest {
            protocol_version: 1,
            queries: (0..65)
                .map(|n| definition(&n.to_string(), list("default")))
                .collect(),
        }))
        .await;
    assert_eq!(invalid.err().unwrap().code(), tonic::Code::InvalidArgument);
    runtime.stop().await;
    for stream in &mut streams {
        assert!(stream.next().await.is_none());
    }
    drop(streams);
    assert_eq!(runtime.counts(), (0, 0, 0));
}

fn sampled_at(bytes: &[u8]) -> i64 {
    match pb::QueryResult::decode(bytes).unwrap().result.unwrap() {
        pb::query_result::Result::GetStats(value) => value.data.unwrap().generated_at.unwrap(),
        pb::query_result::Result::BoardTaskMap(value) => {
            value.data.unwrap().meta.unwrap().generated_at.unwrap()
        }
        pb::query_result::Result::TaskNeighborhood(value) => {
            value.data.unwrap().meta.unwrap().generated_at.unwrap()
        }
        _ => panic!("sample timestamp"),
    }
}

async fn wait_read(lease: &runtime::Lease, after: u64) {
    let mut changed = lease.shared.hub.subscribe();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if lease.shared.reads.load(Ordering::Relaxed) > after
                && lease.shared.hub.next(None).is_ok()
            {
                break;
            }
            changed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn volatile_sample_time_is_silent_but_business_change_preserves_actual_timestamp() {
    let (_directory, state, runtime) = fixture().await;
    let task = create(&state, "timestamp-task").await;
    crate::application::boards::create::create_board(
        state.clone(),
        Default::default(),
        serde_json::from_value(serde_json::json!({"slug":"unrelated", "name":"Unrelated"}))
            .unwrap(),
    )
    .await
    .unwrap();
    let definitions = vec![
        Query::GetStats(pb::GetStatsRequest {
            board: Some("default".into()),
        }),
        Query::BoardTaskMap(pb::BoardTaskMapRequest {
            board: Some("default".into()),
            ..Default::default()
        }),
        Query::TaskNeighborhood(pb::TaskNeighborhoodRequest {
            task_id: Some(task.id.clone()),
            ..Default::default()
        }),
    ];
    let mut leases = Vec::new();
    let mut snapshots = Vec::new();
    for query in definitions {
        let lease = runtime
            .attach(source::normalize(&state, query).await.unwrap())
            .unwrap();
        wait_read(&lease, 0).await;
        let QueryNext::Snapshot(snapshot) = lease.shared.hub.next(None).unwrap() else {
            panic!("snapshot")
        };
        assert!(sampled_at(snapshot.bytes.as_ref().as_ref()) > 0);
        snapshots.push(snapshot);
        leases.push(lease);
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
    let before: Vec<_> = leases
        .iter()
        .map(|lease| lease.shared.reads.load(Ordering::Relaxed))
        .collect();
    crate::application::tasks::create::create_task(state.clone(), dto::CreateTaskPath { board: "unrelated".into() }, Default::default(), serde_json::from_value(serde_json::json!({"title":"unrelated-write", "description":"other board", "status":"todo"})).unwrap()).await.unwrap();
    for ((lease, snapshot), before) in leases.iter().zip(&snapshots).zip(before) {
        wait_read(lease, before).await;
        assert!(matches!(
            lease.shared.hub.next(Some(&snapshot.cursor)).unwrap(),
            QueryNext::Idle
        ));
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
    let before: Vec<_> = leases
        .iter()
        .map(|lease| lease.shared.reads.load(Ordering::Relaxed))
        .collect();
    state
        .application()
        .update_task(kanban_service::UpdateTaskCommand {
            task_id: task.id,
            actor: "query-test".into(),
            expected_lock_version: Some(task.lock_version),
            title: Some("new title".into()),
            description: None,
            assignee: None,
            priority: None,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata: None,
        })
        .await
        .unwrap();
    for (index, ((lease, snapshot), before)) in
        leases.iter().zip(&snapshots).zip(before).enumerate()
    {
        wait_read(lease, before).await;
        let QueryNext::Snapshot(current) = lease.shared.hub.next(None).unwrap() else {
            panic!("snapshot")
        };
        if index == 0 {
            // 改标题没有改变 Stats 的业务计数；原样本时间保持不变。
            assert_eq!(current.cursor, snapshot.cursor);
        } else {
            assert!(current.cursor.revision > snapshot.cursor.revision);
            assert!(
                sampled_at(current.bytes.as_ref().as_ref())
                    > sampled_at(snapshot.bytes.as_ref().as_ref())
            );
        }
    }
    drop(snapshots);
    drop(leases);
    runtime.stop().await;
}

#[tokio::test]
async fn cancelled_chunk_and_evicted_hub_resume_reset_without_reusing_revision_identity() {
    let (_directory, state, runtime) = fixture().await;
    let task = create(&state, "chunk-cancel").await;
    let query = Query::ListComments(pb::ListCommentsRequest {
        task_id: Some(task.id.clone()),
    });
    let mut stream = watch(&runtime, vec![definition("comments", query.clone())]).await;
    let mut rebuilt = Rebuild::default();
    let committed = rebuilt.ready(&mut stream, "comments").await;
    let old_bytes = rebuilt.bytes["comments"].clone();
    comment(&state, &task.id, &"多字节日志内容".repeat(12_000)).await;
    let begin = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Some(Body::Begin(value)) = &begin.body else {
        panic!("begin")
    };
    assert!(value.chunk_count > 1);
    rebuilt.apply(begin);
    let chunk = stream.next().await.unwrap().unwrap();
    assert!(chunk.encoded_len() <= kanban_protocol::rpc::query::MAX_QUERY_FRAME_BYTES);
    assert!(matches!(chunk.body, Some(Body::Chunk(_))));
    rebuilt.apply(chunk);
    assert_eq!(rebuilt.bytes["comments"], old_bytes);
    assert_eq!(rebuilt.cursors["comments"], committed);
    drop(stream);
    assert_eq!(runtime.counts().0, 0);
    let mut resume = definition("comments", query.clone());
    resume.resume = Some(committed.clone());
    let mut resumed = watch(&runtime, vec![resume]).await;
    let reset = tokio::time::timeout(Duration::from_secs(5), resumed.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Some(Body::Begin(value)) = &reset.body else {
        panic!("reset")
    };
    assert!(value.snapshot);
    assert_ne!(value.cursor.as_ref().unwrap().scope, committed.scope);
    rebuilt.pending.clear();
    rebuilt.apply(reset);
    rebuilt.receive(&mut resumed, 1).await;
    assert_eq!(
        rebuilt.bytes["comments"],
        source::read(state, query).await.unwrap()
    );
    drop(resumed);
    runtime.stop().await;
    assert_eq!(runtime.counts(), (0, 0, 0));
}

#[tokio::test]
async fn active_run_log_and_stats_observe_file_and_clock_changes_without_mutation_hint() {
    let directory = tempfile::tempdir().unwrap();
    let logs = directory.path().join("run-logs");
    let state = AppState::open_with_run_log_root(
        directory.path().join("run-query.db"),
        "query-test",
        Some(logs.clone()),
    )
    .await
    .unwrap();
    let task = crate::application::tasks::create::create_task(state.clone(), dto::CreateTaskPath { board: "default".into() }, Default::default(),
        serde_json::from_value(serde_json::json!({"title":"run-log", "description":"execute log check", "status":"ready"})).unwrap()).await.unwrap().data;
    state
        .application()
        .mark_execution_plan_not_required(
            kanban_service::operations::MarkExecutionPlanNotRequiredCommand {
                task_id: task.id.clone(),
                reason: "single operation".into(),
                actor: "query-test".into(),
            },
        )
        .await
        .unwrap();
    // 新任务的 unplanned 计划会令请求的 ready 保持 todo；计划豁免后仍需显式 promote。
    state
        .application()
        .promote_task(kanban_service::operations::PromoteTaskCommand {
            task_id: task.id.clone(),
            actor: "query-test".into(),
        })
        .await
        .unwrap();
    let claim = state
        .application()
        .claim_task_with_run_log_dir(
            kanban_service::operations::ClaimTaskCommand {
                task_id: task.id,
                actor: "query-test".into(),
                ttl_ms: 1500,
                worker_profile: None,
                metadata: serde_json::json!({}),
            },
            &logs,
        )
        .await
        .unwrap();
    let path = logs.join(format!("{}.log", claim.run.id));
    tokio::fs::write(&path, "first line\n").await.unwrap();
    let query = Query::GetRunLog(pb::GetRunLogRequest {
        run_id: Some(claim.run.id),
    });
    let runtime = QueryRuntime::new(state.clone());
    let mut stream = watch(
        &runtime,
        vec![
            definition("log", query.clone()),
            definition(
                "stats",
                Query::GetStats(pb::GetStatsRequest {
                    board: Some("default".into()),
                }),
            ),
        ],
    )
    .await;
    let mut rebuilt = Rebuild::default();
    rebuilt.receive(&mut stream, 2).await;
    let Some(pb::query_result::Result::GetStats(initial)) =
        pb::QueryResult::decode(rebuilt.bytes["stats"].as_slice())
            .unwrap()
            .result
    else {
        panic!("stats")
    };
    assert!(initial.data.unwrap().stale_claims.is_empty());
    let changes = state.application().subscribe_realtime_changes();
    tokio::fs::write(&path, "first line\nappended without DB writes\n")
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            rebuilt.apply(stream.next().await.unwrap().unwrap());
            let log = pb::QueryResult::decode(rebuilt.bytes["log"].as_slice()).unwrap();
            let stats = pb::QueryResult::decode(rebuilt.bytes["stats"].as_slice()).unwrap();
            let Some(pb::query_result::Result::GetRunLog(log)) = log.result else {
                panic!("log")
            };
            let Some(pb::query_result::Result::GetStats(stats)) = stats.result else {
                panic!("stats")
            };
            if log.data.unwrap().content.unwrap().contains("appended")
                && !stats.data.unwrap().stale_claims.is_empty()
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(!changes.has_changed().unwrap());
    assert_eq!(
        rebuilt.bytes["log"],
        source::read(state.clone(), query).await.unwrap()
    );
    drop(stream);
    runtime.stop().await;
    let reads = state.grpc_probe.queries();
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert_eq!(state.grpc_probe.queries(), reads);
    assert_eq!(runtime.counts(), (0, 0, 0));
}
