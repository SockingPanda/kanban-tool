use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use clap::Args;

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    /// Canonical Turso database owned by this host.
    #[arg(long, env = "KANBAN_DB")]
    pub(crate) db: Option<PathBuf>,
    /// Enable the in-process single-worker dispatcher with a strict TOML profile.
    #[arg(long)]
    pub(crate) dispatcher_profile: Option<PathBuf>,
    /// Loopback address to listen on.
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    pub(crate) host: IpAddr,
    /// Local HTTP port.
    #[arg(long, default_value_t = 8721)]
    pub(crate) port: u16,
}

pub(crate) async fn run(ctx: &CliContext, args: &ServeArgs) -> Result<(), CliFailure> {
    if !args.host.is_loopback() {
        return Err(CliFailure {
            code: "invalid_input",
            message: "kanban serve only accepts a loopback --host".to_owned(),
            exit_code: 2,
        });
    }
    let dispatcher =
        match args.dispatcher_profile.as_deref() {
            Some(path) => Some(kanban_server::DispatcherConfig::load(path).await.map_err(
                |error| CliFailure {
                    code: "invalid_input",
                    message: error.to_string(),
                    exit_code: 2,
                },
            )?),
            None => None,
        };
    let db_path = args.db.clone().unwrap_or_else(default_db_path);
    let state = kanban_server::AppState::open(&db_path, ctx.actor())
        .await
        .map_err(|error| CliFailure {
            code: "storage_error",
            message: error.to_string(),
            exit_code: 1,
        })?;
    let addr = SocketAddr::new(args.host, args.port);
    eprintln!(
        "kanban serve listening on http://{addr}; database={}; dispatcher={}",
        db_path.display(),
        dispatcher
            .as_ref()
            .map(|config| config.board())
            .unwrap_or("disabled")
    );
    let (shutdown_tx, shutdown_rx) =
        tokio::sync::watch::channel(kanban_server::ShutdownSignal::Running);
    let signal_task = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        shutdown_tx
            .send(kanban_server::ShutdownSignal::Graceful)
            .ok();
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        shutdown_tx.send(kanban_server::ShutdownSignal::Force).ok();
    });
    let result =
        kanban_server::serve_with_dispatcher_shutdown(addr, state, dispatcher, shutdown_rx).await;
    signal_task.abort();
    result.map_err(|error| {
        if error.kind() == std::io::ErrorKind::Interrupted {
            CliFailure {
                code: "interrupted",
                message: error.to_string(),
                exit_code: 130,
            }
        } else {
            CliFailure {
                code: "server_error",
                message: error.to_string(),
                exit_code: 1,
            }
        }
    })
}

pub(crate) fn default_db_path() -> PathBuf {
    dirs_next::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("kb")
        .join("kanban.db")
}

#[cfg(test)]
mod tests {
    use super::default_db_path;
    use crate::{Cli, Command};
    use clap::Parser;

    #[test]
    fn default_database_uses_new_filename() {
        assert_eq!(
            default_db_path().file_name().and_then(|name| name.to_str()),
            Some("kanban.db")
        );
    }

    #[test]
    fn serve_dispatcher_is_opt_in_by_profile_path() {
        let disabled = Cli::try_parse_from(["kanban", "serve"]).expect("serve args");
        let Command::Serve(disabled) = disabled.command else {
            panic!("expected serve command");
        };
        assert_eq!(disabled.dispatcher_profile, None);

        let enabled =
            Cli::try_parse_from(["kanban", "serve", "--dispatcher-profile", "dispatcher.toml"])
                .expect("dispatcher serve args");
        let Command::Serve(enabled) = enabled.command else {
            panic!("expected serve command");
        };
        assert_eq!(
            enabled.dispatcher_profile.as_deref(),
            Some(std::path::Path::new("dispatcher.toml"))
        );
    }
}
