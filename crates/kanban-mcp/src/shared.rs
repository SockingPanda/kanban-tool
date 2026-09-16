use std::{future::Future, sync::Arc};

use kanban_client::{ClientError, KanbanClient};
use rmcp::{ErrorData as McpError, handler::server::router::tool::ToolRouter};
use tokio::sync::Semaphore;

use crate::{config::Config, errors::Failure, policy::ToolPolicy};

#[derive(Clone)]
pub(crate) struct KanbanMcp {
    pub(crate) client: KanbanClient,
    pub(crate) default_board: Arc<str>,
    pub(crate) config: Arc<Config>,
    pub(crate) policy: Arc<ToolPolicy>,
    pub(crate) router: Arc<ToolRouter<Self>>,
    pub(crate) in_flight: Arc<Semaphore>,
}

impl KanbanMcp {
    pub(crate) fn from_env() -> anyhow::Result<Self> {
        Self::from_config(Config::load()?)
    }

    pub(crate) fn from_config(config: Config) -> anyhow::Result<Self> {
        config.validate()?;
        let router = Arc::new(Self::tool_router());
        let work = Self::work_tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        let policy = Arc::new(ToolPolicy::build(&config, router.list_all(), work)?);
        Ok(Self {
            client: KanbanClient::new(config.server_url.clone(), config.actor.clone())?,
            default_board: Arc::from(config.default_board.as_str()),
            in_flight: Arc::new(Semaphore::new(config.limits.max_in_flight)),
            config: Arc::new(config),
            policy,
            router,
        })
    }

    pub(crate) fn board(&self, board: Option<String>) -> String {
        board.unwrap_or_else(|| self.default_board.to_string())
    }
}

pub(crate) async fn call_client<T, F>(operation: F) -> Result<T, McpError>
where
    F: Future<Output = Result<T, ClientError>>,
{
    operation
        .await
        .map_err(|error| Failure::from_client(error).into_internal())
}

pub(crate) async fn call_client_internal<T, F>(operation: F) -> Result<T, McpError>
where
    F: Future<Output = Result<T, ClientError>>,
{
    call_client(operation).await
}
