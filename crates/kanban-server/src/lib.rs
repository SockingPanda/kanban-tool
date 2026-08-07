#![doc = include_str!("../README.md")]

mod dispatcher;
mod error;
mod http;
mod router;
mod state;
pub(crate) mod vector;

#[cfg(test)]
mod knowledge_adoption;
#[cfg(test)]
mod suite;

pub use dispatcher::{DispatcherConfig, ShutdownSignal};
pub use router::{build_router, serve, serve_with_dispatcher_shutdown, serve_with_shutdown};
pub use state::AppState;
