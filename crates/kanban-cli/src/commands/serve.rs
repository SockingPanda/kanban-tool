use std::{net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use kanban_sqlite::api::lifecycle::begin_database_runtime;
use kanban_sqlite::init::init_database;

use crate::args::{ServeArgs, ServeLogLevel};
use crate::commands::common::invalid_input;

pub(crate) fn serve(args: ServeArgs, db_path: PathBuf, board: &str, actor: String) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;
    if !addr.ip().is_loopback() {
        return Err(invalid_input(
            "kanban serve only supports loopback hosts; use 127.0.0.1 or ::1",
        ));
    }
    init_serve_tracing(&args);
    let _runtime_guard = begin_database_runtime(&db_path)?;
    let _init = init_database(&db_path, &actor)
        .with_context(|| format!("failed to initialize/open {}", db_path.display()))?;
    tracing::info!(
        "Serving Kanban API on http://{} using {}",
        addr,
        db_path.display()
    );
    let runtime = tokio::runtime::Runtime::new().context("failed to start tokio runtime")?;
    runtime
        .block_on(kanban_server::serve_with_search_sync_shutdown(
            addr,
            kanban_server::AppState::new(db_path, actor),
            kanban_server::SearchSyncConfig::new(
                board,
                Duration::from_millis(args.search_sync_interval_ms),
            ),
            serve_shutdown_signal(),
        ))
        .context("kanban server failed")
}

async fn serve_shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!("received Ctrl-C; shutting down Kanban API gracefully");
            tokio::spawn(async {
                if tokio::signal::ctrl_c().await.is_ok() {
                    std::process::exit(130);
                }
            });
        }
        Err(error) => {
            tracing::warn!(%error, "failed to listen for Ctrl-C shutdown signal");
        }
    }
}

fn init_serve_tracing(args: &ServeArgs) {
    if args.quiet {
        kanban_server::init_tracing_with_filter_spec("off");
        return;
    }

    if let Some(level) = args.log_level {
        kanban_server::init_tracing_with_filter_spec(&serve_log_filter(level));
        return;
    }

    kanban_server::init_tracing();
}

fn serve_log_filter(level: ServeLogLevel) -> String {
    let level = level.as_filter_level();
    if level == "off" {
        return "off".to_owned();
    }
    format!(
        "kanban={level},kanban_cli={level},kanban_server={level},tower_http={level},kanban_desktop={level}"
    )
}
