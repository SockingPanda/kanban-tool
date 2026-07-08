use std::path::Path;

use anyhow::Result;
use kanban_core::Locale;
use serde::Serialize;

use crate::{commands::common::invalid_input, output::print_or_json};

#[derive(Debug, Serialize)]
pub(crate) struct ConfigShowOutput {
    db: ResolvedValue<String>,
    board: ResolvedValue<String>,
    locale: ResolvedValue<String>,
}

#[derive(Debug, Serialize)]
struct ResolvedValue<T> {
    value: T,
    source: ConfigSourceDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ConfigSourceDto {
    Flag { name: String },
    Env { name: String },
    ProjectConfig { path: String, key: String },
    GlobalConfig { path: String, key: String },
    Default,
}

pub(crate) fn show_config(
    db: Option<&Path>,
    board: Option<&str>,
    locale: Option<&str>,
    json: bool,
) -> Result<()> {
    let output = resolve_config(db, board, locale)?;
    print_or_json(json, &output, || human_config_summary(&output))
}

fn resolve_config(
    db: Option<&Path>,
    board: Option<&str>,
    locale: Option<&str>,
) -> Result<ConfigShowOutput> {
    let db = kanban_local::resolved_db_path_with_source(db)?;
    let board = kanban_local::resolved_active_board_with_source(board)?;
    let locale = resolved_locale_with_source(locale)?;

    Ok(ConfigShowOutput {
        db: ResolvedValue {
            value: db.value.display().to_string(),
            source: db.source.into(),
            input: None,
        },
        board: ResolvedValue {
            value: board.value,
            source: board.source.into(),
            input: None,
        },
        locale,
    })
}

fn resolved_locale_with_source(locale: Option<&str>) -> Result<ResolvedValue<String>> {
    let (input, source) = if let Some(flag) = locale {
        (
            Some(flag.trim().to_owned()),
            ConfigSourceDto::Flag {
                name: "--locale".to_owned(),
            },
        )
    } else if let Ok(env) = std::env::var("KANBAN_LOCALE") {
        (
            Some(env.trim().to_owned()),
            ConfigSourceDto::Env {
                name: "KANBAN_LOCALE".to_owned(),
            },
        )
    } else {
        (None, ConfigSourceDto::Default)
    };

    let value = Locale::explicit_or_system(input.as_deref())
        .map_err(invalid_input)?
        .as_str()
        .to_owned();

    Ok(ResolvedValue {
        value,
        source,
        input,
    })
}

fn human_config_summary(output: &ConfigShowOutput) -> String {
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

fn source_summary(source: &ConfigSourceDto) -> String {
    match source {
        ConfigSourceDto::Flag { name } => format!("flag {name}"),
        ConfigSourceDto::Env { name } => format!("env {name}"),
        ConfigSourceDto::ProjectConfig { path, key } => format!("project config {path}:{key}"),
        ConfigSourceDto::GlobalConfig { path, key } => format!("global config {path}:{key}"),
        ConfigSourceDto::Default => "default".to_owned(),
    }
}

impl From<kanban_local::ConfigValueSource> for ConfigSourceDto {
    fn from(source: kanban_local::ConfigValueSource) -> Self {
        match source {
            kanban_local::ConfigValueSource::Flag { name } => Self::Flag {
                name: name.to_owned(),
            },
            kanban_local::ConfigValueSource::Env { name } => Self::Env {
                name: name.to_owned(),
            },
            kanban_local::ConfigValueSource::ProjectConfig { path, key } => Self::ProjectConfig {
                path: path.display().to_string(),
                key: key.to_owned(),
            },
            kanban_local::ConfigValueSource::GlobalConfig { path, key } => Self::GlobalConfig {
                path: path.display().to_string(),
                key: key.to_owned(),
            },
            kanban_local::ConfigValueSource::Default => Self::Default,
        }
    }
}
