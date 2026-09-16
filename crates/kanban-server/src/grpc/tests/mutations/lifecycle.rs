//! 正式具名 RPC 的大响应受 HTTP/2 背压时，Host 仍拥有并释放连接资源。
use super::*;
use std::{sync::Arc, time::Instant};

#[derive(Debug, Clone, Copy)]
enum Stop {
    Graceful,
    Force,
    Cancel,
}

async fn blocked_download(host: &Host) -> reqwest::Response {
    let mut client = client(host).await;
    let task = create(&mut client, &host.board, "背压中的正式附件响应").await;
    let attachment: dto::CreateAttachmentResponse = client
        .create_attachment(
            pb::CreateAttachmentRequest::from_parts(
                dto::CreateAttachmentPath {
                    task_id: task.id.clone(),
                },
                (),
                dto::CreateAttachmentRequest {
                    id: None,
                    filename: "backpressure.bin".into(),
                    content: vec![0xa5; 2 * 1024 * 1024],
                    content_type: None,
                    rel_path: None,
                    sha256: None,
                    actor: None,
                },
            )
            .unwrap(),
        )
        .await
        .unwrap()
        .into_inner()
        .try_into()
        .unwrap();
    let request = pb::DownloadAttachmentRequest::from_parts(
        dto::GetAttachmentPath {
            task_id: task.id,
            attachment_id: attachment.data.id,
        },
        (),
        (),
    )
    .unwrap();
    let response = reqwest::Client::builder()
        .http2_prior_knowledge()
        .http2_initial_stream_window_size(1024)
        .http2_initial_connection_window_size(1024)
        .build()
        .unwrap()
        .post(format!(
            "{}/kanban.v1.KanbanService/DownloadAttachment",
            host.url
        ))
        .header("content-type", "application/grpc")
        .body(super::super::frame(request))
        .send()
        .await
        .unwrap();
    assert_eq!(response.version(), reqwest::Version::HTTP_2);
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response
}

async fn assert_released(stop: Stop) {
    let host = Host::start().await;
    let response = blocked_download(&host).await;
    let mut stream = watch(&host, &host.board).await;
    super::super::ready(&mut stream).await;
    let probe = host.state.grpc_probe.clone();
    assert!(
        Arc::strong_count(&probe) > 2,
        "服务端仍持有 application adapter"
    );
    let started = Instant::now();
    match stop {
        Stop::Graceful => {
            host.stop.send_replace(crate::ShutdownSignal::Graceful);
        }
        Stop::Force => {
            host.stop.send_replace(crate::ShutdownSignal::Force);
        }
        Stop::Cancel => host.server.abort(),
    }
    let joined = tokio::time::timeout(Duration::from_secs(7), host.server)
        .await
        .unwrap();
    match stop {
        Stop::Graceful => assert_eq!(
            joined.unwrap().unwrap_err().kind(),
            std::io::ErrorKind::TimedOut
        ),
        Stop::Force => assert_eq!(
            joined.unwrap().unwrap_err().kind(),
            std::io::ErrorKind::Interrupted
        ),
        Stop::Cancel => assert!(joined.unwrap_err().is_cancelled()),
    }
    let returned_ms = started.elapsed().as_secs_f64() * 1000.0;
    tokio::time::timeout(Duration::from_secs(1), async {
        while Arc::strong_count(&probe) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("客户端继续持有大响应时，服务端必须释放所有 application 引用");
    assert!(*host.state.stream_shutdown_receiver().borrow());
    super::super::closed(&mut stream).await;
    println!(
        "G06_FORMAL_BACKPRESSURE mode={stop:?} h2_window_bytes=1024 unread_response_bytes=2097152 returned_ms={returned_ms:.3} released_ms={:.3} retained_probe_refs={} client_response_still_held=true",
        started.elapsed().as_secs_f64() * 1000.0,
        Arc::strong_count(&probe)
    );
    drop(response);
}

#[tokio::test]
async fn formal_large_response_releases_resources_after_graceful_timeout() {
    assert_released(Stop::Graceful).await;
}

#[tokio::test]
async fn formal_large_response_releases_resources_after_force() {
    assert_released(Stop::Force).await;
}

#[tokio::test]
async fn formal_large_response_releases_resources_after_host_future_cancel() {
    assert_released(Stop::Cancel).await;
}
