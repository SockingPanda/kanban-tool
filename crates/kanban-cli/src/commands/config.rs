use clap::Subcommand;
use kanban_contract::{
    CliConfigShow, CliConfigShowOutput, CliConfigSource, CliResolvedConfigValue,
    CliResolvedLocaleValue,
};

use crate::{config, error::CliFailure, output};

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    /// 查看当前配置值及其来源；不会打开、初始化或创建 Turso 数据库。
    #[command(
        after_help = "Notes:\n  This command only resolves configuration; it does not open, initialize, migrate, or create the Turso database.\n  With --json, read data.db.source.kind, data.board.source.kind, and data.locale.source.kind instead of parsing human output.\n\nExamples:\n  kanban config show\n  kanban --json config show"
    )]
    Show,
}

pub(crate) fn run(
    command: &ConfigCommand,
    db: Option<&std::path::Path>,
    board: Option<&str>,
    locale: Option<&str>,
    json: bool,
) -> Result<(), CliFailure> {
    match command {
        ConfigCommand::Show => show(db, board, locale, json),
    }
}

fn show(
    db: Option<&std::path::Path>,
    board: Option<&str>,
    locale: Option<&str>,
    json: bool,
) -> Result<(), CliFailure> {
    let resolved_db = config::resolve_db_path(db).map_err(CliFailure::from)?;
    let resolved_board = config::resolve_board(board).map_err(CliFailure::from)?;
    let resolved_locale = config::resolve_locale(locale).map_err(|message| CliFailure {
        code: "invalid_input",
        message,
        exit_code: 2,
    })?;
    let value = CliConfigShow {
        db: CliResolvedConfigValue {
            value: resolved_db.value.display().to_string(),
            source: source(resolved_db.source),
        },
        board: CliResolvedConfigValue {
            value: resolved_board.value,
            source: source(resolved_board.source),
        },
        locale: CliResolvedLocaleValue {
            value: resolved_locale.value,
            input: resolved_locale.input,
            source: source(resolved_locale.source),
        },
    };
    let envelope = CliConfigShowOutput::new(value.clone());
    if json {
        output::print_json(&envelope);
    } else {
        println!(
            "db: {} ({})\nboard: {} ({})\nlocale: {} ({})",
            value.db.value,
            source_summary(&value.db.source),
            value.board.value,
            source_summary(&value.board.source),
            value.locale.value,
            source_summary(&value.locale.source),
        );
    }
    Ok(())
}

fn source(source: config::ConfigValueSource) -> CliConfigSource {
    match source {
        config::ConfigValueSource::Flag { name } => CliConfigSource::Flag {
            name: name.to_owned(),
        },
        config::ConfigValueSource::Env { name } => CliConfigSource::Env {
            name: name.to_owned(),
        },
        config::ConfigValueSource::ProjectConfig { path, key } => CliConfigSource::ProjectConfig {
            path: path.display().to_string(),
            key: key.to_owned(),
        },
        config::ConfigValueSource::GlobalConfig { path, key } => CliConfigSource::GlobalConfig {
            path: path.display().to_string(),
            key: key.to_owned(),
        },
        config::ConfigValueSource::Default => CliConfigSource::Default,
    }
}

fn source_summary(source: &CliConfigSource) -> String {
    match source {
        CliConfigSource::Flag { name } => format!("flag {name}"),
        CliConfigSource::Env { name } => format!("env {name}"),
        CliConfigSource::ProjectConfig { path, key } => {
            format!("project config {path}:{key}")
        }
        CliConfigSource::GlobalConfig { path, key } => format!("global config {path}:{key}"),
        CliConfigSource::Default => "default".to_owned(),
    }
}
