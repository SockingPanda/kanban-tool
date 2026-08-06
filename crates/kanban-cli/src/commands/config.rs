use clap::Subcommand;
use kanban_protocol::{
    CliConfigShow, CliConfigShowOutput, CliConfigSource, CliResolvedConfigValue,
    CliResolvedLocaleValue,
};

use crate::{config, error::CliFailure, output};

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    /// 查看当前配置值及其来源；不会打开、初始化或创建 Turso 数据库。
    #[command(
        after_help = "说明：\n  此命令只解析配置，不会打开、初始化、迁移或创建 Turso 数据库。\n  使用 --json 时，请读取 data.db.source.kind、data.board.source.kind 和 data.locale.source.kind，\n  不要解析人类可读输出。\n\n示例：\n  kanban config show\n  kanban --json config show"
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
            "数据库：{}（{}）\n看板：{}（{}）\nlocale：{}（{}）",
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
        CliConfigSource::Flag { name } => format!("命令行 flag {name}"),
        CliConfigSource::Env { name } => format!("环境变量 {name}"),
        CliConfigSource::ProjectConfig { path, key } => {
            format!("项目配置 {path}:{key}")
        }
        CliConfigSource::GlobalConfig { path, key } => format!("全局配置 {path}:{key}"),
        CliConfigSource::Default => "默认值".to_owned(),
    }
}
