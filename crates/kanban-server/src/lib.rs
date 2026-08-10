#![doc = include_str!("../README.md")]

mod dispatcher;
mod error;
mod http;
mod router;
mod state;
pub(crate) mod vector;
mod web;

#[cfg(test)]
mod knowledge_adoption;
#[cfg(test)]
mod suite;

pub use dispatcher::{DispatcherConfig, ShutdownSignal};
pub use router::{
    build_production_router, build_router, serve, serve_with_dispatcher_shutdown,
    serve_with_dispatcher_shutdown_and_web, serve_with_shutdown, serve_with_shutdown_and_web,
};
pub use state::AppState;
pub use web::WebHostConfig;
