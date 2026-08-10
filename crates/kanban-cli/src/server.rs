use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::Args;

use crate::{config, context::CliContext, error::CliFailure};

#[derive(Debug, Args, Clone)]
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
    /// 禁用同源 Web host，仅启动 API host。
    #[arg(long, conflicts_with = "web_dir")]
    pub(crate) no_web: bool,
    /// Web artifact dist 目录；优先级高于 `KANBAN_WEB_DIR` 与系统默认目录。
    #[arg(long, value_name = "PATH")]
    pub(crate) web_dir: Option<PathBuf>,
}

pub(crate) async fn run(ctx: &CliContext, args: &ServeArgs) -> Result<(), CliFailure> {
    if !args.host.is_loopback() {
        return Err(CliFailure {
            code: "invalid_input",
            message: "kanban serve 的 --host 只能是 loopback 地址".to_owned(),
            exit_code: 2,
        });
    }
    let web = discover_web(ctx, args)?;
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
    let result = match web {
        Some(web) => {
            kanban_server::serve_with_dispatcher_shutdown_and_web(
                addr,
                state,
                dispatcher,
                shutdown_rx,
                web,
            )
            .await
        }
        None => {
            kanban_server::serve_with_dispatcher_shutdown(addr, state, dispatcher, shutdown_rx)
                .await
        }
    };
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

const DEFAULT_WEB_DIR: &str = "/usr/share/kanban-tool/web";

fn discover_web(
    ctx: &CliContext,
    args: &ServeArgs,
) -> Result<Option<kanban_server::WebHostConfig>, CliFailure> {
    if args.no_web {
        if args.web_dir.is_some() {
            return Err(invalid_web("--no-web 不能与 --web-dir 同时使用"));
        }
        return Ok(None);
    }
    let (raw, source) = if let Some(path) = args.web_dir.as_deref() {
        (path.to_owned(), "--web-dir")
    } else if let Some(path) = std::env::var_os("KANBAN_WEB_DIR") {
        (PathBuf::from(path), "KANBAN_WEB_DIR")
    } else {
        (PathBuf::from(DEFAULT_WEB_DIR), "默认 Web artifact 目录")
    };
    let path = lexical_absolute(&raw).map_err(invalid_web)?;
    let artifact = kanban_web_artifact::verify_directory(&path, env!("CARGO_PKG_VERSION"))
        .map_err(|error| {
            invalid_web(format!(
                "无法加载 {source} Web artifact {}：{error}",
                path.display()
            ))
        })?;
    Ok(Some(kanban_server::WebHostConfig::new(
        Arc::new(artifact),
        ctx.actor(),
        ctx.board.clone(),
    )))
}

fn lexical_absolute(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("Web artifact 路径不能为空".to_owned());
    }
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|error| format!("无法解析 Web artifact 当前目录：{error}"))
}

fn invalid_web(message: impl Into<String>) -> CliFailure {
    CliFailure {
        code: "invalid_input",
        message: message.into(),
        exit_code: 2,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        sync::{Mutex, OnceLock},
    };

    use crate::{Cli, Command};
    use clap::Parser;
    use kanban_protocol::{
        WEB_ARTIFACT_BASE_PATH, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
        WEB_PROTOCOL_VERSION, WebArtifactManifest, web_artifact_build_id_for,
        web_artifact_file_from_bytes,
    };

    use super::{ServeArgs, discover_web};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn write_artifact(root: &Path) {
        let bytes = b"<main />";
        fs::write(root.join(WEB_ARTIFACT_ENTRYPOINT), bytes).expect("write index");
        let file = web_artifact_file_from_bytes(WEB_ARTIFACT_ENTRYPOINT, bytes).expect("file");
        let build_id = web_artifact_build_id_for(
            WEB_ARTIFACT_FORMAT_VERSION,
            WEB_ARTIFACT_BASE_PATH,
            WEB_ARTIFACT_ENTRYPOINT,
            env!("CARGO_PKG_VERSION"),
            WEB_PROTOCOL_VERSION,
            std::slice::from_ref(&file),
        )
        .expect("build id");
        let manifest = WebArtifactManifest {
            format_version: WEB_ARTIFACT_FORMAT_VERSION,
            base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
            entrypoint: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
            server_version: env!("CARGO_PKG_VERSION").to_owned(),
            protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
            build_id,
            files: vec![file],
        };
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&manifest).expect("manifest json"),
        )
        .expect("write manifest");
    }

    #[test]
    fn serve_dispatcher_is_opt_in_by_profile_path() {
        let disabled = Cli::try_parse_from(["kanban", "serve"]).expect("serve args");
        let Command::Serve(disabled) = disabled.command else {
            panic!("expected serve command");
        };
        assert_eq!(disabled.dispatcher_profile, None);
        assert!(!disabled.no_web);
        assert_eq!(disabled.web_dir, None);

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

    #[test]
    fn serve_no_web_conflicts_with_explicit_web_dir() {
        let error = Cli::try_parse_from(["kanban", "serve", "--no-web", "--web-dir", "dist"])
            .expect_err("no-web and web-dir must conflict");
        assert!(error.to_string().contains("cannot be used with"));
    }

    #[test]
    fn web_discovery_prefers_explicit_and_fails_before_host_open() {
        let root = tempfile::tempdir().expect("artifact root");
        write_artifact(root.path());
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let old = std::env::var_os("KANBAN_WEB_DIR");
        // Rust 2024 将进程环境写操作标为 unsafe；测试持有锁，避免同进程并发污染。
        unsafe {
            std::env::set_var("KANBAN_WEB_DIR", root.path().join("missing-env"));
        }
        let ctx = crate::context::CliContext {
            server_url: String::new(),
            board: "default".to_owned(),
            board_source: crate::config::ConfigValueSource::Default,
            db: None,
            actor: Some("test".to_owned()),
            json: false,
        };
        let args = ServeArgs {
            dispatcher_profile: None,
            host: "127.0.0.1".parse().expect("host"),
            port: 8721,
            no_web: false,
            web_dir: Some(root.path().to_owned()),
        };
        assert!(
            discover_web(&ctx, &args)
                .expect("explicit artifact")
                .is_some()
        );

        unsafe {
            std::env::set_var("KANBAN_WEB_DIR", root.path());
        }
        let env_args = ServeArgs {
            web_dir: None,
            ..args.clone()
        };
        assert!(
            discover_web(&ctx, &env_args)
                .expect("env artifact")
                .is_some()
        );

        let missing = ServeArgs {
            web_dir: Some(root.path().join("missing-explicit")),
            ..args
        };
        let error = match discover_web(&ctx, &missing) {
            Ok(_) => panic!("missing artifact must fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, "invalid_input");

        let no_web = ServeArgs {
            no_web: true,
            web_dir: None,
            ..missing
        };
        assert!(
            discover_web(&ctx, &no_web)
                .expect("no-web skips discovery")
                .is_none()
        );
        if let Some(value) = old {
            unsafe { std::env::set_var("KANBAN_WEB_DIR", value) };
        } else {
            unsafe { std::env::remove_var("KANBAN_WEB_DIR") };
        }
    }
}
