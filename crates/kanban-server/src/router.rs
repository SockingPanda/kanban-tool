use std::{future::Future, net::SocketAddr};

use axum::{Router, middleware};
use tokio::sync::{oneshot, watch};
use tower_http::trace::TraceLayer;

use crate::{
    dispatcher::{DispatcherConfig, ShutdownSignal, run_dispatcher},
    grpc::{RpcLifetime, WorkspaceRpcMount, workspace_rpc_mount},
    http::operations,
    state::AppState,
    web::WebHostConfig,
};

pub fn build_router(state: AppState) -> Router {
    let origins = crate::web::desktop_origins();
    application_router(state, origins.clone())
        .router
        .layer(crate::web::cors_layer(origins))
        .layer(TraceLayer::new_for_http())
}

fn application_router(state: AppState, origins: Vec<axum::http::HeaderValue>) -> WorkspaceRpcMount {
    let mut mount =
        workspace_rpc_mount(&state, origins).expect("Host Origin 来自经过验证的本地配置");
    mount.router = operations::router(state).merge(mount.router);
    mount
}

fn host_router(
    state: AppState,
    web: Option<WebHostConfig>,
    listener: SocketAddr,
) -> WorkspaceRpcMount {
    let policy = crate::web::HostOriginPolicy::for_listener(listener);
    let origins = policy.origins();
    let mut mount = application_router(state, origins.clone());
    if let Some(web) = web {
        mount.router = crate::web::with_web(mount.router, web);
    }
    mount.router = mount
        .router
        .layer(crate::web::cors_layer(origins))
        .layer(middleware::from_fn_with_state(
            policy,
            crate::web::enforce_host_origin,
        ))
        .layer(TraceLayer::new_for_http());
    mount
}

pub fn build_production_router(
    state: AppState,
    config: WebHostConfig,
    listener: SocketAddr,
) -> Router {
    host_router(state, Some(config), listener).router
}

pub async fn serve(addr: SocketAddr, state: AppState) -> std::io::Result<()> {
    serve_with_shutdown(addr, state, std::future::pending()).await
}

pub async fn serve_with_shutdown<S>(
    addr: SocketAddr,
    state: AppState,
    shutdown: S,
) -> std::io::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "kanban serve 只接受 loopback 地址",
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let mount = host_router(state.clone(), None, listener.local_addr()?);
    let rpc = RpcLifetime(mount.runtime);
    let shutdown_rpc = rpc.0.clone();
    let shutdown_state = state.clone();
    let result = crate::connections::serve(listener, mount.router, async move {
        shutdown.await;
        shutdown_state.begin_event_stream_shutdown();
        shutdown_rpc.stop().await;
    })
    .await;
    rpc.stop().await;
    result
}

pub async fn serve_with_shutdown_and_web<S>(
    addr: SocketAddr,
    state: AppState,
    web: WebHostConfig,
    shutdown: S,
) -> std::io::Result<()>
where
    S: Future<Output = ()> + Send + 'static,
{
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "kanban serve 只接受 loopback 地址",
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let listener_addr = listener.local_addr()?;
    let mount = host_router(state.clone(), Some(web), listener_addr);
    let rpc = RpcLifetime(mount.runtime);
    let shutdown_rpc = rpc.0.clone();
    let shutdown_state = state.clone();
    let result = crate::connections::serve(listener, mount.router, async move {
        shutdown.await;
        shutdown_state.begin_event_stream_shutdown();
        shutdown_rpc.stop().await;
    })
    .await;
    rpc.stop().await;
    result
}

pub async fn serve_with_dispatcher_shutdown(
    addr: SocketAddr,
    state: AppState,
    dispatcher: Option<DispatcherConfig>,
    shutdown: watch::Receiver<ShutdownSignal>,
) -> std::io::Result<()> {
    serve_with_dispatcher_shutdown_inner(addr, state, dispatcher, shutdown, None).await
}

pub async fn serve_with_dispatcher_shutdown_and_web(
    addr: SocketAddr,
    state: AppState,
    dispatcher: Option<DispatcherConfig>,
    shutdown: watch::Receiver<ShutdownSignal>,
    web: WebHostConfig,
) -> std::io::Result<()> {
    serve_with_dispatcher_shutdown_inner(addr, state, dispatcher, shutdown, Some(web)).await
}

async fn serve_with_dispatcher_shutdown_inner(
    addr: SocketAddr,
    state: AppState,
    dispatcher: Option<DispatcherConfig>,
    shutdown: watch::Receiver<ShutdownSignal>,
    web: Option<WebHostConfig>,
) -> std::io::Result<()> {
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "kanban serve 只接受 loopback 地址",
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let listener_addr = listener.local_addr()?;
    let shutdown_state = state.clone();
    let mount = host_router(state.clone(), web, listener_addr);
    let rpc = RpcLifetime(mount.runtime);
    let (http_shutdown_tx, http_shutdown_rx) = oneshot::channel();
    let mut http = std::pin::pin!(crate::connections::serve(
        listener,
        mount.router,
        async move {
            let _ = http_shutdown_rx.await;
        }
    ));
    let dispatcher_shutdown = shutdown.clone();
    let mut dispatcher = std::pin::pin!(async move {
        if let Some(config) = dispatcher {
            run_dispatcher(state, config, listener_addr, dispatcher_shutdown).await
        } else {
            wait_for_graceful(dispatcher_shutdown).await;
            Ok(())
        }
    });
    let mut force_shutdown = shutdown.clone();

    let dispatcher_result = tokio::select! {
        result = &mut http => {
            shutdown_state.begin_event_stream_shutdown();
            rpc.stop().await;
            return result;
        },
        result = &mut dispatcher => result,
        () = wait_for_force(&mut force_shutdown) => {
            shutdown_state.begin_event_stream_shutdown();
            return Err(force_shutdown_error());
        }
    };
    shutdown_state.begin_event_stream_shutdown();
    if *shutdown.borrow() == ShutdownSignal::Force {
        return Err(force_shutdown_error());
    }
    rpc.stop().await;
    http_shutdown_tx.send(()).ok();
    if let Err(error) = dispatcher_result {
        return Err(std::io::Error::other(error.to_string()));
    }

    tokio::select! {
        result = &mut http => result,
        () = wait_for_force(&mut force_shutdown) => {
            shutdown_state.begin_event_stream_shutdown();
            Err(force_shutdown_error())
        },
    }
}

async fn wait_for_graceful(mut shutdown: watch::Receiver<ShutdownSignal>) {
    loop {
        if *shutdown.borrow() != ShutdownSignal::Running {
            return;
        }
        if shutdown.changed().await.is_err() {
            return;
        }
    }
}

async fn wait_for_force(shutdown: &mut watch::Receiver<ShutdownSignal>) {
    loop {
        if *shutdown.borrow() == ShutdownSignal::Force {
            return;
        }
        if shutdown.changed().await.is_err() {
            return;
        }
    }
}

fn force_shutdown_error() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Interrupted, "kanban serve 被强制停止")
}

#[cfg(test)]
mod contract_catalog_tests {
    use std::{collections::BTreeSet, net::SocketAddr, time::Duration};

    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use kanban_protocol::{HttpMethod, endpoint_catalog};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::{oneshot, watch},
    };
    use tower::ServiceExt;

    use crate::{ShutdownSignal, http::operations::registered_api_routes, state::AppState};

    #[test]
    fn api_route_catalog_matches_exact_contract_catalog() {
        let routes = registered_api_routes();
        let actual = routes
            .iter()
            .map(|route| format!("{} {}", method_name(route.method), route.path))
            .collect::<BTreeSet<_>>();
        let expected = endpoint_catalog()
            .iter()
            .map(|endpoint| format!("{} {}", method_name(endpoint.method), endpoint.path))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            routes.len(),
            actual.len(),
            "同一个 method+path 不得重复注册；实际路由注册必须保持唯一"
        );
        assert_eq!(
            actual, expected,
            "Axum 实际 route 注册必须与精确 API contract catalog 完全一致"
        );
    }

    fn method_name(method: HttpMethod) -> &'static str {
        match method {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
        }
    }

    #[tokio::test]
    async fn desktop_cors_preflight_allows_last_event_id() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let response = super::build_router(state)
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/api/v1/stream/events")
                    .header(header::ORIGIN, "http://127.0.0.1:1421")
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                    .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "last-event-id")
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
        assert!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS]
                .to_str()
                .unwrap()
                .split(',')
                .any(|value| value.trim().eq_ignore_ascii_case("last-event-id"))
        );
    }

    async fn free_loopback_addr() -> SocketAddr {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        addr
    }

    async fn connect_sse(addr: SocketAddr) -> TcpStream {
        let mut stream = loop {
            match TcpStream::connect(addr).await {
                Ok(stream) => break stream,
                Err(_) => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        };
        stream
            .write_all(
                format!(
                    "GET /api/v1/stream/events?after=0 HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut response = Vec::new();
        while !response.windows(4).any(|window| window == b"\r\n\r\n") {
            let mut chunk = [0_u8; 1024];
            let read = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut chunk))
                .await
                .expect("SSE headers timeout")
                .unwrap();
            assert!(read > 0, "SSE closed before headers");
            response.extend_from_slice(&chunk[..read]);
        }
        assert!(response.starts_with(b"HTTP/1.1 200"));
        stream
    }

    async fn assert_sse_closes(mut stream: TcpStream) {
        let mut tail = Vec::new();
        tokio::time::timeout(Duration::from_secs(3), stream.read_to_end(&mut tail))
            .await
            .expect("SSE shutdown timeout")
            .unwrap();
    }

    #[tokio::test]
    async fn no_web_serve_shutdown_closes_active_sse() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let addr = free_loopback_addr().await;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server = tokio::spawn(crate::serve_with_shutdown(addr, state, async move {
            let _ = shutdown_rx.await;
        }));
        let stream = connect_sse(addr).await;
        shutdown_tx.send(()).unwrap();
        assert_sse_closes(stream).await;
        assert!(server.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn dispatcher_common_shutdown_closes_active_sse() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("kanban.db"), "test")
            .await
            .unwrap();
        let addr = free_loopback_addr().await;
        let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
        let server = tokio::spawn(crate::serve_with_dispatcher_shutdown(
            addr,
            state,
            None,
            shutdown_rx,
        ));
        let stream = connect_sse(addr).await;
        shutdown_tx.send(ShutdownSignal::Graceful).unwrap();
        assert_sse_closes(stream).await;
        assert!(server.await.unwrap().is_ok());
    }
}
