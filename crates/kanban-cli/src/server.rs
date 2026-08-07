use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use clap::Args;

use crate::{config, context::CliContext, error::CliFailure};

#[derive(Debug, Args)]
pub(crate) struct ServeArgs {
    /// 使用严格 TOML profile 启用进程内单 worker dispatcher。
    #[arg(long)]
    pub(crate) dispatcher_profile: Option<PathBuf>,
    /// 要监听的 loopback 地址。
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::LOCALHOST))]
    pub(crate) host: IpAddr,
    /// 本地 HTTP 端口。
    #[arg(long, default_value_t = 8721)]
    pub(crate) port: u16,
}

pub(crate) async fn run(ctx: &CliContext, args: &ServeArgs) -> Result<(), CliFailure> {
    if !args.host.is_loopback() {
        return Err(CliFailure {
            code: "invalid_input",
            message: "kanban serve 的 --host 只能是 loopback 地址".to_owned(),
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
    let db_path = config::resolve_db_path(ctx.db.as_deref())
        .map_err(CliFailure::from)?
        .value;
    let state = kanban_server::AppState::open_with_run_log_root(
        &db_path,
        ctx.actor(),
        dispatcher
            .as_ref()
            .map(|config| config.log_dir().to_path_buf()),
    )
    .await
    .map_err(|error| CliFailure {
        code: "storage_error",
        message: error.to_string(),
        exit_code: 1,
    })?;
    let addr = SocketAddr::new(args.host, args.port);
    eprintln!(
        "kanban serve 正在监听 http://{addr}；数据库={}；dispatcher={}",
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

#[cfg(test)]
mod tests {
    use crate::{Cli, Command};
    use clap::Parser;

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
