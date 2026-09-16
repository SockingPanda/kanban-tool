use std::{collections::BTreeSet, net::SocketAddr, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode, Version, header},
};
use kanban_protocol::rpc::{self, v1 as pb};
use prost::Message;
use tokio::sync::{oneshot, watch};
use tower::ServiceExt;

use crate::{AppState, ShutdownSignal};

#[tokio::test]
async fn rpc_routes_match_method_manifest_and_descriptor() {
    let methods = rpc::catalog::methods();
    let expected = methods
        .iter()
        .map(|method| method.path())
        .collect::<BTreeSet<_>>();
    let descriptor = prost_types::FileDescriptorSet::decode(rpc::FILE_DESCRIPTOR_SET).unwrap();
    let actual = descriptor
        .file
        .into_iter()
        .flat_map(|file| {
            let package = file.package.unwrap_or_default();
            file.service.into_iter().flat_map(move |service| {
                let name = format!("{package}.{}", service.name.unwrap());
                service
                    .method
                    .into_iter()
                    .map(move |method| format!("/{name}/{}", method.name.unwrap()))
            })
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(methods.len(), expected.len());
    let routes = methods
        .iter()
        .map(|method| {
            if method.service == "kanban.v1.QueryService" {
                method.path()
            } else {
                format!("/{}/*method", method.service)
            }
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        routes,
        crate::grpc::RPC_ROUTES
            .iter()
            .map(|route| (*route).to_owned())
            .collect()
    );
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("catalog.db"), "catalog-test")
        .await
        .unwrap();
    let router = super::build_router(state.clone());
    for path in expected {
        // 无效 Protobuf 使所有方法停在各自 decoder，验证实际注册且不执行 mutation。
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .version(Version::HTTP_2)
                    .method("POST")
                    .uri(&path)
                    .header("content-type", "application/grpc")
                    .body(Body::from(vec![0, 0, 0, 0, 1, 0xff]))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = crate::test_support::wire_response::<pb::Empty>(response)
            .await
            .unwrap_err();
        assert_eq!(
            status.code(),
            tonic::Code::Internal,
            "{path} 未到达对应 Protobuf decoder: {status}"
        );
    }
    assert_eq!(state.grpc_probe.business_calls(), 0);
    assert!(state.grpc_probe.queries().is_empty());
    for path in [
        "/kanban.v1.KanbanService/Unknown",
        "/kanban.v1.QueryService/Unknown",
        "/kanban.extensions.v1.ObjectService/Unknown",
        "/kanban.extensions.v1.FileService/Unknown",
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .version(Version::HTTP_2)
                    .method("POST")
                    .uri(path)
                    .header("content-type", "application/grpc")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        if !path.contains("QueryService") {
            assert_eq!(
                tonic::Status::from_header_map(response.headers())
                    .unwrap()
                    .code(),
                tonic::Code::Unimplemented
            );
        } else {
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }
}

#[tokio::test]
async fn desktop_cors_preflight_allows_rpc_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("kanban.db"), "test")
        .await
        .unwrap();
    let response = super::build_router(state)
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/kanban.v1.QueryService/WatchQueries")
                .header(header::ORIGIN, "http://127.0.0.1:1421")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(
                    header::ACCESS_CONTROL_REQUEST_HEADERS,
                    "grpc-timeout,x-kb-actor-bin",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "http://127.0.0.1:1421"
    );
    let headers = response.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS]
        .to_str()
        .unwrap();
    assert!(headers.contains("grpc-timeout"));
    assert!(headers.contains("x-kb-actor-bin"));
}

async fn free_loopback_addr() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    listener.local_addr().unwrap()
}

async fn connect_query(addr: SocketAddr) -> tonic::Streaming<pb::QueryFrame> {
    let mut client = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(client) =
                pb::query_service_client::QueryServiceClient::connect(format!("http://{addr}"))
                    .await
            {
                break client;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut stream = client
        .watch_queries(pb::WatchQueriesRequest {
            protocol_version: 1,
            queries: vec![pb::QueryDefinition {
                client_query_id: "boards".into(),
                projection_version: 1,
                query: Some(pb::query_definition::Query::ListBoards(
                    pb::ListBoardsRequest::default(),
                )),
                resume: None,
                refresh: false,
            }],
        })
        .await
        .unwrap()
        .into_inner();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if matches!(
                stream.message().await.unwrap().unwrap().body,
                Some(pb::query_frame::Body::Ready(_))
            ) {
                break;
            }
        }
    })
    .await
    .unwrap();
    stream
}

async fn assert_query_closes(mut stream: tonic::Streaming<pb::QueryFrame>) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while stream.message().await.unwrap().is_some() {}
    })
    .await
    .expect("QueryService shutdown timeout");
}

#[tokio::test]
async fn no_web_serve_shutdown_closes_active_query() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("kanban.db"), "test")
        .await
        .unwrap();
    let addr = free_loopback_addr().await;
    let (tx, rx) = oneshot::channel();
    let server = tokio::spawn(crate::serve_with_shutdown(
        addr,
        state.clone(),
        async move {
            let _ = rx.await;
        },
    ));
    let stream = connect_query(addr).await;
    tx.send(()).unwrap();
    assert_query_closes(stream).await;
    assert!(server.await.unwrap().is_ok());
    assert!(
        state
            .grpc_probe
            .query_resources()
            .is_none_or(|counts| counts == (0, 16, 0))
    );
}

#[tokio::test]
async fn dispatcher_common_shutdown_closes_active_query() {
    let directory = tempfile::tempdir().unwrap();
    let state = AppState::open(directory.path().join("kanban.db"), "test")
        .await
        .unwrap();
    let addr = free_loopback_addr().await;
    let (tx, rx) = watch::channel(ShutdownSignal::Running);
    let server = tokio::spawn(crate::serve_with_dispatcher_shutdown(
        addr,
        state.clone(),
        None,
        rx,
    ));
    let stream = connect_query(addr).await;
    tx.send(ShutdownSignal::Graceful).unwrap();
    assert_query_closes(stream).await;
    assert!(server.await.unwrap().is_ok());
    assert!(
        state
            .grpc_probe
            .query_resources()
            .is_none_or(|counts| counts == (0, 16, 0))
    );
}
