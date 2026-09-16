//! 唯一 Host 的来源、安全 metadata、订阅预算及已退出 HTTP surface。
use super::*;
use pb::{query_definition::Query, query_frame::Body};

#[tokio::test]
async fn rejects_invalid_origin_board_version_and_exposes_rpc_cors_metadata() {
    let host = Host::start().await;
    let mut native = host.client().await;
    let mut request = host.request();
    request.protocol_version = 2;
    assert_eq!(
        native.watch_queries(request).await.unwrap_err().code(),
        Code::InvalidArgument
    );
    let mut missing = native
        .watch_queries(query_request(Query::ListTasks(pb::ListTasksRequest {
            board: Some("b_missing".into()),
            ..Default::default()
        })))
        .await
        .unwrap()
        .into_inner();
    let failure = tokio::time::timeout(Duration::from_secs(2), missing.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Some(Body::Failure(failure)) = failure.body else {
        panic!("缺失看板应返回单查询错误");
    };
    assert_eq!(
        failure.error.unwrap().code,
        pb::DtoApiErrorCode::NotFound as i32
    );
    drop(missing);
    let denied = reqwest::Client::new()
        .post(format!("{}/kanban.v1.QueryService/WatchQueries", host.url))
        .header("origin", "https://untrusted.invalid")
        .header("content-type", "application/grpc-web+proto")
        .body(frame(host.request()))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), reqwest::StatusCode::BAD_REQUEST);
    // 原生 H2 只有 :authority；同时提供冲突 Host 时必须拒绝。
    let conflict = reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .unwrap()
        .post(format!("{}/kanban.v1.KanbanService/GetHealth", host.url))
        .header("host", "localhost:1")
        .header("content-type", "application/grpc")
        .body(frame(pb::GetHealthRequest {}))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status(), reqwest::StatusCode::BAD_REQUEST);
    let options = reqwest::Client::new()
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/kanban.v1.QueryService/WatchQueries", host.url),
        )
        .header("origin", &host.url)
        .header("access-control-request-method", "POST")
        .header(
            "access-control-request-headers",
            "content-type,x-grpc-web,x-user-agent,grpc-timeout,x-kb-actor,x-kb-actor-bin",
        )
        .send()
        .await
        .unwrap();
    assert!(options.status().is_success());
    let allowed = options.headers()["access-control-allow-headers"]
        .to_str()
        .unwrap();
    for header in [
        "content-type",
        "x-grpc-web",
        "x-user-agent",
        "grpc-timeout",
        "x-kb-actor",
        "x-kb-actor-bin",
    ] {
        assert!(allowed.contains(header));
    }
    host.finish().await;
}

#[tokio::test]
async fn native_and_web_share_subscription_budget_and_cancellation_releases_it() {
    let host = Host::start().await;
    let mut client = host.client().await;
    let mut streams = Vec::new();
    for _ in 0..16 {
        let mut stream = client
            .watch_queries(host.request())
            .await
            .unwrap()
            .into_inner();
        ready(&mut stream).await;
        streams.push(stream);
    }
    assert_eq!(
        client
            .watch_queries(host.request())
            .await
            .unwrap_err()
            .code(),
        Code::ResourceExhausted
    );
    let web = reqwest::Client::new()
        .post(format!("{}/kanban.v1.QueryService/WatchQueries", host.url))
        .header("content-type", "application/grpc-web+proto")
        .body(frame(host.request()))
        .send()
        .await
        .unwrap();
    assert_eq!(web.headers()["grpc-status"], "8");
    drop(streams.pop());
    let mut replacement = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match client.watch_queries(host.request()).await {
                Ok(response) => break response.into_inner(),
                Err(error) if error.code() == Code::ResourceExhausted => {
                    tokio::time::sleep(Duration::from_millis(10)).await
                }
                Err(error) => panic!("unexpected status: {error}"),
            }
        }
    })
    .await
    .unwrap();
    ready(&mut replacement).await;
    drop((replacement, streams));
    queries::common::idle(&host).await;
    host.finish().await;
}

#[tokio::test]
async fn rpc_timeout_covers_stream_lifetime_and_rejects_invalid_metadata() {
    let host = Host::start().await;
    let mut native = host.client().await;
    let mut request = tonic::Request::new(host.request());
    request.set_timeout(Duration::from_millis(400));
    let mut stream = native.watch_queries(request).await.unwrap().into_inner();
    ready(&mut stream).await;
    let error = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match stream.message().await {
                Err(error) => break error,
                Ok(Some(_)) => {}
                Ok(None) => panic!("deadline 必须报告 terminal status"),
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(error.code(), Code::DeadlineExceeded);
    let web = reqwest::Client::new()
        .post(format!("{}/kanban.v1.QueryService/WatchQueries", host.url))
        .header("content-type", "application/grpc-web+proto")
        .header("grpc-timeout", "400m")
        .body(frame(host.request()))
        .send()
        .await
        .unwrap();
    let bytes = tokio::time::timeout(Duration::from_secs(2), web.bytes())
        .await
        .unwrap()
        .unwrap();
    let mut offset = 0;
    let mut terminal = false;
    while offset < bytes.len() {
        let length = u32::from_be_bytes(bytes[offset + 1..offset + 5].try_into().unwrap()) as usize;
        let end = offset + 5 + length;
        if bytes[offset] == 0x80 {
            assert_eq!(end, bytes.len());
            assert!(
                std::str::from_utf8(&bytes[offset + 5..end])
                    .unwrap()
                    .split("\r\n")
                    .any(|line| line
                        .split_once(':')
                        .is_some_and(|(key, value)| key == "grpc-status" && value.trim() == "4"))
            );
            terminal = true;
        }
        offset = end;
    }
    assert!(terminal);
    for bad in ["1x", "123456789m", "-1S", ""] {
        let mut request = tonic::Request::new(host.request());
        request
            .metadata_mut()
            .insert("grpc-timeout", bad.parse().unwrap());
        assert_eq!(
            native.watch_queries(request).await.unwrap_err().code(),
            Code::InvalidArgument
        );
    }
    queries::common::idle(&host).await;
    host.finish().await;
}

#[tokio::test]
async fn removed_rest_sse_and_watch_changes_return_404_without_application_calls() {
    let host = Host::start().await;
    let before = host.state.grpc_probe.business_calls();
    let http = reqwest::Client::new();
    // 有效的旧 mutation body 也只能命中 404，不能经 fallback 回调业务。
    let paths = [
        "/api/v1/boards",
        "/api/v1/boards/default/tasks",
        "/api/v1/boards/default/columns",
        "/api/v1/boards/default/labels",
        "/api/v1/tasks/t_retired",
        "/api/v1/tasks/t_retired/comments",
        "/api/v1/tasks/t_retired/attachments",
        "/api/v1/tasks/t_retired/steps",
        "/api/v1/tasks/t_retired/claim",
        "/api/v1/events",
        "/api/v1/stats",
        "/api/v1/search/tasks",
        "/api/v1/graph/status",
        "/api/v1/vector/status",
        "/api/v1/entities",
        "/api/v1/maintenance/doctor",
        "/api/v1/maintenance/import",
        "/api/v1/stream/events?after=0",
        "/kanban.v1.WorkspaceService/WatchChanges",
        "/kanban.framework.v1.WorkspaceService/WatchChanges",
    ];
    for path in paths {
        for method in [
            reqwest::Method::GET,
            reqwest::Method::POST,
            reqwest::Method::PUT,
            reqwest::Method::PATCH,
            reqwest::Method::DELETE,
        ] {
            let response=http.request(method.clone(),format!("{}{path}",host.url))
                .header("content-type","application/json").header("last-event-id","1")
                .body(r#"{"task_id":"t_retired","title":"must not commit","status":"todo","description":"retired surface"}"#)
                .send().await.unwrap();
            assert_eq!(
                response.status(),
                reqwest::StatusCode::NOT_FOUND,
                "{method} {path}"
            );
        }
    }
    assert_eq!(host.state.grpc_probe.business_calls(), before);
    assert!(host.state.grpc_probe.queries().is_empty());
    assert_eq!(host.state.grpc_probe.query_resources(), Some((0, 16, 0)));
    assert!(
        host.state
            .application()
            .get_task("t_retired")
            .await
            .is_err()
    );
    // 仍需存在的 bootstrap 与正式业务操作继续可用。
    assert!(
        http.get(format!("{}/health", host.url))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let mut business = pb::kanban_service_client::KanbanServiceClient::connect(host.url.clone())
        .await
        .unwrap();
    business.get_health(pb::GetHealthRequest {}).await.unwrap();
    assert_eq!(host.state.grpc_probe.business_calls(), before + 1);
    host.finish().await;
}
