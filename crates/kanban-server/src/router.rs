use std::{
    future::{Future, IntoFuture},
    net::SocketAddr,
};

use axum::{
    Router,
    http::{HeaderValue, Method, header},
};
use tokio::sync::{oneshot, watch};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    dispatcher::{DispatcherConfig, ShutdownSignal, run_dispatcher},
    http::operations,
    state::AppState,
};

pub fn build_router(state: AppState) -> Router {
    operations::router(state)
        .layer(desktop_cors_layer())
        .layer(TraceLayer::new_for_http())
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
    axum::serve(listener, build_router(state))
        .with_graceful_shutdown(shutdown)
        .await
}

pub async fn serve_with_dispatcher_shutdown(
    addr: SocketAddr,
    state: AppState,
    dispatcher: Option<DispatcherConfig>,
    shutdown: watch::Receiver<ShutdownSignal>,
) -> std::io::Result<()> {
    if !addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "kanban serve 只接受 loopback 地址",
        ));
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let (http_shutdown_tx, http_shutdown_rx) = oneshot::channel();
    let mut http = std::pin::pin!(
        axum::serve(listener, build_router(state.clone()))
            .with_graceful_shutdown(async move {
                let _ = http_shutdown_rx.await;
            })
            .into_future()
    );
    let dispatcher_shutdown = shutdown.clone();
    let mut dispatcher = std::pin::pin!(async move {
        if let Some(config) = dispatcher {
            run_dispatcher(state, config, addr, dispatcher_shutdown).await
        } else {
            wait_for_graceful(dispatcher_shutdown).await;
            Ok(())
        }
    });
    let mut force_shutdown = shutdown.clone();

    let dispatcher_result = tokio::select! {
        result = &mut http => return result,
        result = &mut dispatcher => result,
        () = wait_for_force(&mut force_shutdown) => {
            return Err(force_shutdown_error());
        }
    };
    if *shutdown.borrow() == ShutdownSignal::Force {
        return Err(force_shutdown_error());
    }
    http_shutdown_tx.send(()).ok();
    if let Err(error) = dispatcher_result {
        return Err(std::io::Error::other(error.to_string()));
    }

    tokio::select! {
        result = &mut http => result,
        () = wait_for_force(&mut force_shutdown) => Err(force_shutdown_error()),
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

fn desktop_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            HeaderValue::from_static("http://127.0.0.1:1420"),
            HeaderValue::from_static("http://localhost:1420"),
            HeaderValue::from_static("http://tauri.localhost"),
            HeaderValue::from_static("https://tauri.localhost"),
            HeaderValue::from_static("tauri://localhost"),
        ]))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::HeaderName::from_static("x-kb-actor"),
        ])
}

#[cfg(test)]
mod contract_catalog_tests {
    use std::collections::BTreeSet;

    use kanban_protocol::{HttpMethod, endpoint_catalog};

    use crate::http::operations::registered_api_routes;

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
}
