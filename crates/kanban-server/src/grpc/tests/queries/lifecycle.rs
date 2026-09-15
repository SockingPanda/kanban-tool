//! 完整查询正在传输大快照时，Host 退出仍拥有 source、连接和所有编码字节。
use super::{Host, common};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
enum Stop {
    Graceful,
    Force,
}

async fn assert_query_reclaimed(stop: Stop) {
    let host = Host::start().await;
    let mut business = common::client(&host).await;
    let task = common::task(&mut business, &host.board, "退出中的完整查询").await;
    common::comment(&mut business, &task, &"x".repeat(2 * 1024 * 1024)).await;
    let client = reqwest::Client::builder()
        .http2_prior_knowledge()
        .http2_initial_stream_window_size(1024)
        .http2_initial_connection_window_size(1024)
        .build()
        .unwrap();
    let mut response = client
        .post(format!("{}/kanban.v1.QueryService/WatchQueries", host.url))
        .header("content-type", "application/grpc")
        .body(super::super::frame(common::request(
            common::comments_query(&task),
            None,
        )))
        .send()
        .await
        .unwrap();
    assert_eq!(response.version(), reqwest::Version::HTTP_2);
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    // 至少接收一次 DATA，确认发送已开始；随后保持小窗口背压，不继续读取。
    let first = tokio::time::timeout(Duration::from_secs(2), response.chunk())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!first.is_empty());
    let retained = tokio::time::timeout(Duration::from_secs(2), async {
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
    assert_eq!((retained.0, retained.1), (1, 15));
    let probe = host.state.grpc_probe.clone();
    assert!(Arc::strong_count(&probe) > 2);
    let started = Instant::now();
    host.stop.send_replace(match stop {
        Stop::Graceful => crate::ShutdownSignal::Graceful,
        Stop::Force => crate::ShutdownSignal::Force,
    });
    let outcome = tokio::time::timeout(Duration::from_secs(7), host.server)
        .await
        .unwrap()
        .unwrap();
    let expected = match stop {
        Stop::Graceful => std::io::ErrorKind::TimedOut,
        Stop::Force => std::io::ErrorKind::Interrupted,
    };
    assert_eq!(outcome.unwrap_err().kind(), expected);
    tokio::time::timeout(Duration::from_secs(2), async {
        while Arc::strong_count(&probe) != 2
            || probe
                .query_resources()
                .is_some_and(|metrics| metrics != (0, 16, 0))
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("客户端仍持有未读响应时，Host 必须回收 query source 和完整字节");
    assert!(*host.state.event_stream_shutdown_receiver().borrow());
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    println!(
        "G08_QUERY_SHUTDOWN mode={stop:?} h2_window_bytes=1024 retained_before={} released_ms={:.3} remaining_application_refs=2 client_response_still_held=true",
        retained.2,
        started.elapsed().as_secs_f64() * 1000.0
    );
    drop(response);
    drop(client);
}

#[tokio::test]
async fn backpressured_query_releases_all_resources_after_graceful_timeout() {
    assert_query_reclaimed(Stop::Graceful).await;
}

#[tokio::test]
async fn backpressured_query_releases_all_resources_after_force_shutdown() {
    assert_query_reclaimed(Stop::Force).await;
}
