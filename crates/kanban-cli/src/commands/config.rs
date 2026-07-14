use std::path::Path;

use anyhow::Result;
use kanban_contract::{
    CliConfigShow, CliConfigShowOutput, CliConfigSource, CliResolvedConfigValue,
    CliResolvedLocaleValue,
};
use kanban_core::Locale;

use crate::{commands::common::invalid_input, output::print_contract_or_human};

pub(crate) fn show_config(
    db: Option<&Path>,
    board: Option<&str>,
    locale: Option<&str>,
    json: bool,
) -> Result<()> {
    let output = resolve_config(db, board, locale)?;
    let human = human_config_summary(&output);
    let output = CliConfigShowOutput::new(output);
    print_contract_or_human(json, &output, || human)
}

fn resolve_config(
    db: Option<&Path>,
    board: Option<&str>,
    locale: Option<&str>,
) -> Result<CliConfigShow> {
    let db = kanban_local::resolved_db_path_with_source(db)?;
    let board = kanban_local::resolved_active_board_with_source(board)?;
    let locale = resolved_locale_with_source(locale)?;

    Ok(CliConfigShow {
        db: CliResolvedConfigValue {
            value: db.value.display().to_string(),
            source: config_source(db.source),
        },
        board: CliResolvedConfigValue {
            value: board.value,
            source: config_source(board.source),
        },
        locale,
    })
}

fn resolved_locale_with_source(locale: Option<&str>) -> Result<CliResolvedLocaleValue> {
    let (input, source) = if let Some(flag) = locale {
        (
            Some(flag.trim().to_owned()),
            CliConfigSource::Flag {
                name: "--locale".to_owned(),
            },
        )
    } else if let Ok(env) = std::env::var("KANBAN_LOCALE") {
        (
            Some(env.trim().to_owned()),
            CliConfigSource::Env {
                name: "KANBAN_LOCALE".to_owned(),
            },
        )
    } else {
        (None, CliConfigSource::Default)
    };

    let value = Locale::explicit_or_system(input.as_deref())
        .map_err(invalid_input)?
        .as_str()
        .to_owned();

    Ok(CliResolvedLocaleValue {
        value,
        source,
        input,
    })
}

fn human_config_summary(output: &CliConfigShow) -> String {
    [
        format!(
            "db: {} ({})",
            output.db.value,
            source_summary(&output.db.source)
        ),
        format!(
            "board: {} ({})",
            output.board.value,
            source_summary(&output.board.source)
        ),
        format!(
            "locale: {} ({})",
            output.locale.value,
            source_summary(&output.locale.source)
        ),
    ]
    .join("\n")
}

fn source_summary(source: &CliConfigSource) -> String {
    match source {
        CliConfigSource::Flag { name } => format!("flag {name}"),
        CliConfigSource::Env { name } => format!("env {name}"),
        CliConfigSource::ProjectConfig { path, key } => format!("project config {path}:{key}"),
        CliConfigSource::GlobalConfig { path, key } => format!("global config {path}:{key}"),
        CliConfigSource::Default => "default".to_owned(),
    }
}

fn config_source(source: kanban_local::ConfigValueSource) -> CliConfigSource {
    match source {
        kanban_local::ConfigValueSource::Flag { name } => CliConfigSource::Flag {
            name: name.to_owned(),
        },
        kanban_local::ConfigValueSource::Env { name } => CliConfigSource::Env {
            name: name.to_owned(),
        },
        kanban_local::ConfigValueSource::ProjectConfig { path, key } => {
            CliConfigSource::ProjectConfig {
                path: path.display().to_string(),
                key: key.to_owned(),
            }
        }
        kanban_local::ConfigValueSource::GlobalConfig { path, key } => {
            CliConfigSource::GlobalConfig {
                path: path.display().to_string(),
                key: key.to_owned(),
            }
        }
        kanban_local::ConfigValueSource::Default => CliConfigSource::Default,
    }
}
