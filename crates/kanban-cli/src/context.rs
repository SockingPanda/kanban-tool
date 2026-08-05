use kanban_client::{ClientError, KanbanClient};

use crate::{Cli, config, error::CliFailure};

/// Runtime values shared by command handlers after clap has parsed the CLI.
///
/// Keeping this small context separate from the parser lets each operation
/// module depend on the same client construction and output flags without
/// depending on another operation module.
#[derive(Debug, Clone)]
pub(crate) struct CliContext {
    pub(crate) server_url: String,
    pub(crate) board: String,
    pub(crate) board_source: config::ConfigValueSource,
    pub(crate) db: Option<std::path::PathBuf>,
    pub(crate) actor: Option<String>,
    pub(crate) json: bool,
}

impl CliContext {
    pub(crate) fn from_cli(cli: &Cli) -> Result<Self, CliFailure> {
        let resolved_board =
            config::resolve_board(cli.board.as_deref()).map_err(CliFailure::from)?;
        Ok(Self {
            server_url: cli.server_url.clone(),
            board: resolved_board.value,
            board_source: resolved_board.source,
            db: cli.db.clone(),
            actor: cli.actor.clone(),
            json: cli.json,
        })
    }

    pub(crate) fn client(&self) -> Result<KanbanClient, CliFailure> {
        KanbanClient::new(&self.server_url, self.actor()).map_err(|error: ClientError| error.into())
    }

    pub(crate) fn actor(&self) -> String {
        self.actor
            .clone()
            .or_else(|| std::env::var("USER").ok())
            .or_else(|| std::env::var("USERNAME").ok())
            .unwrap_or_else(|| "local".to_owned())
    }
}
