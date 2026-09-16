use std::{future::Future, net::SocketAddr};

use axum::{Router, middleware};
use tokio::sync::{oneshot, watch};
use tower_http::trace::TraceLayer;

use crate::{
    dispatcher::{DispatcherConfig, ShutdownSignal, run_dispatcher},
    grpc::{RpcLifetime, RpcMount, rpc_mount},
    state::AppState,
    web::WebHostConfig,
};

pub fn build_router(state: AppState) -> Router {
    let origins = crate::web::desktop_origins();
    application_router(state)
        .router
        .layer(crate::web::cors_layer(origins))
        .layer(TraceLayer::new_for_http())
}

fn application_router(state: AppState) -> RpcMount {
    let mut mount = rpc_mount(&state);
    mount.router = crate::http::router(state).merge(mount.router);
    mount
}

fn host_router(state: AppState, web: Option<WebHostConfig>, listener: SocketAddr) -> RpcMount {
    let policy = crate::web::HostOriginPolicy::for_listener(listener);
    let origins = policy.origins();
    let mut mount = application_router(state);
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
        shutdown_state.begin_stream_shutdown();
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
        shutdown_state.begin_stream_shutdown();
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
            shutdown_state.begin_stream_shutdown();
            rpc.stop().await;
            return result;
        },
        result = &mut dispatcher => result,
        () = wait_for_force(&mut force_shutdown) => {
            shutdown_state.begin_stream_shutdown();
            return Err(force_shutdown_error());
        }
    };
    shutdown_state.begin_stream_shutdown();
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
            shutdown_state.begin_stream_shutdown();
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
mod contract_catalog_tests;
