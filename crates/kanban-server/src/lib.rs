mod dto;
mod error;
mod handlers;
mod helper;
mod i18n;
mod observability;
mod router;
mod state;

pub use observability::{DEFAULT_TRACING_FILTER, init_tracing, init_tracing_with_filter};
pub use router::{
    build_desktop_router, build_router, build_serve_router, serve, serve_with_search_sync,
};
pub use state::{AppState, SearchSyncConfig, search_sync_task_enabled, spawn_search_sync_task};
