use std::{env, fmt::Display, future::Future, sync::Arc};

use kanban_client::{DEFAULT_SERVER_URL, KanbanClient};
use rmcp::ErrorData as McpError;

#[derive(Clone)]
pub(crate) struct KanbanMcp {
    pub(crate) client: KanbanClient,
    pub(crate) default_board: Arc<str>,
}

impl KanbanMcp {
    pub(crate) fn from_env() -> anyhow::Result<Self> {
        let server_url =
            env::var("KANBAN_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_owned());
        let actor = env::var("KANBAN_ACTOR").unwrap_or_else(|_| "mcp".to_owned());
        let default_board = env::var("KB_BOARD").unwrap_or_else(|_| "default".to_owned());

        Ok(Self {
            client: KanbanClient::new(server_url, actor)?,
            default_board: Arc::from(default_board),
        })
    }

    pub(crate) fn board(&self, board: Option<String>) -> String {
        board.unwrap_or_else(|| self.default_board.to_string())
    }
}

pub(crate) async fn call_client<T, E, F>(operation: F) -> Result<T, McpError>
where
    E: Display,
    F: Future<Output = Result<T, E>>,
{
    operation
        .await
        .map_err(|error| McpError::invalid_params(error.to_string(), None))
}

pub(crate) async fn call_client_internal<T, E, F>(operation: F) -> Result<T, McpError>
where
    E: Display,
    F: Future<Output = Result<T, E>>,
{
    operation
        .await
        .map_err(|error| McpError::internal_error(error.to_string(), None))
}
