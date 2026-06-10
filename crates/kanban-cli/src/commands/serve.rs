use std::{net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail};
use kanban_sqlite::{begin_database_runtime, init_database};

use crate::args::ServeArgs;

pub(crate) fn serve(args: ServeArgs, db_path: PathBuf, board: &str, actor: String) -> Result<()> {
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;
    if !addr.ip().is_loopback() {
        bail!("kb serve only supports loopback hosts; use 127.0.0.1 or ::1");
    }
    let _runtime_guard = begin_database_runtime(&db_path)?;
    let _init = init_database(&db_path, &actor)
        .with_context(|| format!("failed to initialize/open {}", db_path.display()))?;
    eprintln!(
        "Serving kb API on http://{addr} using {}",
        db_path.display()
    );
    let runtime = tokio::runtime::Runtime::new().context("failed to start tokio runtime")?;
    runtime
        .block_on(kanban_server::serve_with_search_sync(
            addr,
            kanban_server::AppState::new(db_path, actor),
            kanban_server::SearchSyncConfig::new(
                board,
                Duration::from_millis(args.search_sync_interval_ms),
            ),
        ))
        .context("kb server failed")
}
