pub(super) mod common;
mod disconnect;
mod lifecycle;
mod recovery;

use super::Host;
use kanban_protocol::rpc::v1::{self as pb, query_definition::Query, query_frame::Body};
use prost::Message;
use std::time::Duration;

fn request() -> pb::WatchQueriesRequest {
    pb::WatchQueriesRequest {
        protocol_version: 1,
        queries: vec![pb::QueryDefinition {
            client_query_id: "boards".into(),
            projection_version: 1,
            query: Some(Query::ListBoards(pb::ListBoardsRequest {
                include_archived: None,
            })),
            resume: None,
            refresh: false,
        }],
    }
}

#[tokio::test]
async fn native_and_grpc_web_mount_complete_query_and_shutdown_together() {
    let host = Host::start().await;
    let mut native = pb::query_service_client::QueryServiceClient::connect(host.url.clone())
        .await
        .unwrap();
    let mut stream = native.watch_queries(request()).await.unwrap().into_inner();
    let mut bytes = Vec::new();
    let mut began = false;
    let mut ended = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let frame = stream.message().await.unwrap().unwrap();
            assert_eq!(frame.client_query_id, "boards");
            match frame.body.unwrap() {
                Body::Begin(begin) => {
                    assert!(begin.snapshot);
                    began = true;
                }
                Body::Chunk(chunk) => bytes.extend(chunk.data),
                Body::End(_) => {
                    assert!(began);
                    ended = true;
                }
                Body::Ready(_) => {
                    assert!(ended);
                    break;
                }
                _ => panic!("initial query body"),
            }
        }
    })
    .await
    .unwrap();
    let result = pb::QueryResult::decode(bytes.as_slice()).unwrap();
    assert!(matches!(
        result.result,
        Some(pb::query_result::Result::ListBoards(_))
    ));

    let mut web = reqwest::Client::new()
        .post(format!("{}/kanban.v1.QueryService/WatchQueries", host.url))
        .header("content-type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .header("origin", &host.url)
        .body(super::frame(request()))
        .send()
        .await
        .unwrap();
    assert_eq!(web.status(), reqwest::StatusCode::OK);
    assert_eq!(web.headers()["access-control-allow-origin"], host.url);
    let frame = tokio::time::timeout(Duration::from_secs(5), web.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(frame[0], 0);
    host.stop.send_replace(crate::ShutdownSignal::Graceful);
    tokio::time::timeout(Duration::from_secs(5), async {
        while stream.message().await.is_ok_and(|frame| frame.is_some()) {}
    })
    .await
    .unwrap();
    drop(web);
    drop(stream);
    host.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn h2_backpressure_deadline_reclaims_query_without_downstream_poll() {
    let host = Host::start().await;
    let task = crate::application::tasks::create::create_task(
        host.state.clone(),
        kanban_protocol::CreateTaskPath {
            board: host.board.clone(),
        },
        Default::default(),
        serde_json::from_value(serde_json::json!({
            "title": "query deadline",
            "description": "H2 backpressure",
            "status": "todo"
        }))
        .unwrap(),
    )
    .await
    .unwrap()
    .data;
    host.state
        .application()
        .create_comment(kanban_service::operations::CreateCommentCommand {
            task_id: task.id.clone(),
            idempotency_key: None,
            author: "query-test".into(),
            author_type: kanban_service::CommentAuthorType::User,
            agent_type: None,
            body: "x".repeat(2 * 1024 * 1024),
            kind: kanban_service::CommentKind::Note,
            metadata: Default::default(),
        })
        .await
        .unwrap();
    let request = pb::WatchQueriesRequest {
        protocol_version: 1,
        queries: vec![pb::QueryDefinition {
            client_query_id: "large-comments".into(),
            projection_version: 1,
            query: Some(Query::ListComments(pb::ListCommentsRequest {
                task_id: Some(task.id),
            })),
            resume: None,
            refresh: false,
        }],
    };
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .http2_initial_stream_window_size(1024)
        .http2_initial_connection_window_size(1024)
        .build()
        .unwrap();
    let started = std::time::Instant::now();
    let response = client
        .post(format!("{}/kanban.v1.QueryService/WatchQueries", host.url))
        .header("content-type", "application/grpc")
        .header("grpc-timeout", "1500m")
        .body(super::frame(request))
        .send()
        .await
        .unwrap();
    assert_eq!(response.version(), reqwest::Version::HTTP_2);
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let before = tokio::time::timeout(Duration::from_millis(1000), async {
        loop {
            let metrics = host.state.grpc_probe.query_resources().unwrap();
            if metrics.2 > 2 * 1024 * 1024 {
                break metrics;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!((before.0, before.1), (1, 15));
    // 始终持有未读取的 response 和 H2 client，直到核验 source/permit/发送字节全部回收。
    tokio::time::timeout(Duration::from_secs(3), async {
        while host.state.grpc_probe.query_resources() != Some((0, 16, 0)) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    eprintln!(
        "G07_QUERY_DEADLINE h2_window_bytes=1024 unread_result_bytes={} deadline_ms=1500 reclaimed_ms={:.3} hubs=0 available_permits=16 retained_bytes=0 client_response_still_held=true",
        before.2,
        started.elapsed().as_secs_f64() * 1000.0,
    );
    let reads = host.state.grpc_probe.queries();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(host.state.grpc_probe.queries(), reads);
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    drop(response);
    drop(client);
    host.finish().await;
}
