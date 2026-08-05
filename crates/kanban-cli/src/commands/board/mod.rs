pub(super) mod archive;
pub(super) mod columns;
pub(super) mod create;
pub(super) mod list;
pub(super) mod show;

use clap::Subcommand;
use kanban_contract::{
    CliBoardConfigSelection, CliBoardCurrentOutput, CliBoardUseOutput, CliConfigSource,
};

use crate::{context::CliContext, error::CliFailure};

#[derive(Debug, Subcommand)]
pub(crate) enum BoardCommand {
    /// 创建看板并初始化默认列。
    Create(create::CreateArgs),
    /// 从 canonical application service 列出看板。
    List(list::ListArgs),
    /// 查找当前有效看板。
    Show(show::ShowArgs),
    /// 归档看板。
    Archive(archive::ArchiveArgs),
    /// 列出看板的固定状态列。
    Columns(columns::ColumnsArgs),
    /// 只更新项目 `.kb/config.toml` 的 active board 选择，不校验 Turso 中的 board。
    Use { board: String },
    /// 显示项目配置解析出的 active board，不访问 host。
    Current,
}

pub(crate) fn run(ctx: &CliContext, command: &BoardCommand) -> Result<(), CliFailure> {
    match command {
        BoardCommand::Create(args) => create::run(ctx, args),
        BoardCommand::List(args) => list::run(ctx, args),
        BoardCommand::Show(args) => show::run(ctx, args),
        BoardCommand::Archive(args) => archive::run(ctx, args),
        BoardCommand::Columns(args) => columns::run(ctx, args),
        BoardCommand::Use { board } => {
            let write =
                crate::config::write_active_board(board).map_err(crate::error::CliFailure::from)?;
            let output = CliBoardUseOutput::new(CliBoardConfigSelection {
                board: board.clone(),
                config_path: write.path.display().to_string(),
                source: CliConfigSource::ProjectConfig {
                    path: write.path.display().to_string(),
                    key: "board".to_owned(),
                },
                created: write.created,
                updated: write.updated,
            });
            if ctx.json {
                crate::output::print_json(&output);
            } else {
                println!(
                    "当前 board：{}\n配置：{}\n{}",
                    output.data.board,
                    output.data.config_path,
                    if output.data.created {
                        "已创建配置"
                    } else if output.data.updated {
                        "已更新配置"
                    } else {
                        "配置未改变"
                    }
                );
            }
            Ok(())
        }
        BoardCommand::Current => {
            let config_path = match &ctx.board_source {
                crate::config::ConfigValueSource::ProjectConfig { path, .. } => path.clone(),
                _ => crate::config::project_config_path_for_write().map_err(|source| {
                    crate::error::CliFailure::from(crate::config::ConfigError::Io {
                        path: std::path::PathBuf::from(".kb/config.toml"),
                        source,
                    })
                })?,
            };
            let output = CliBoardCurrentOutput::new(CliBoardConfigSelection {
                board: ctx.board.clone(),
                config_path: config_path.display().to_string(),
                source: source(&ctx.board_source),
                created: false,
                updated: false,
            });
            if ctx.json {
                crate::output::print_json(&output);
            } else {
                println!("{}", output.data.board);
            }
            Ok(())
        }
    }
}

fn source(source: &crate::config::ConfigValueSource) -> CliConfigSource {
    match source {
        crate::config::ConfigValueSource::Flag { name } => CliConfigSource::Flag {
            name: (*name).to_owned(),
        },
        crate::config::ConfigValueSource::Env { name } => CliConfigSource::Env {
            name: (*name).to_owned(),
        },
        crate::config::ConfigValueSource::ProjectConfig { path, key } => {
            CliConfigSource::ProjectConfig {
                path: path.display().to_string(),
                key: (*key).to_owned(),
            }
        }
        crate::config::ConfigValueSource::GlobalConfig { path, key } => {
            CliConfigSource::GlobalConfig {
                path: path.display().to_string(),
                key: (*key).to_owned(),
            }
        }
        crate::config::ConfigValueSource::Default => CliConfigSource::Default,
    }
}
