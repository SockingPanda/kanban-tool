use std::path::PathBuf;
#[cfg(feature = "vector-lancedb")]
use std::{path::Path, sync::Arc};

use anyhow::Result;
#[cfg(feature = "vector-lancedb")]
use kanban_sqlite::suggest_task_labels_with;
use kanban_sqlite::{
    CreateLabel, LabelSuggestionOptions, LabelSuggestionResult, add_task_label, create_label,
    list_labels, remove_task_label, suggest_task_labels,
};

use crate::args::LabelCommand;
use crate::output::{label_line, print_or_json, print_task};

pub(crate) fn handle_label(
    command: LabelCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        LabelCommand::List => {
            let labels = list_labels(db_path, board)?;
            print_or_json(json, &labels, || {
                labels.iter().map(label_line).collect::<Vec<_>>().join("\n")
            })?;
        }
        LabelCommand::Create(args) => {
            let label = create_label(
                db_path,
                board,
                CreateLabel {
                    name: args.name,
                    color: args.color,
                },
            )?;
            print_or_json(json, &label, || label_line(&label))?;
        }
        LabelCommand::Add(args) => {
            let task = add_task_label(db_path, board, actor, &args.task_ref, &args.label)?;
            print_task(json, &task)?;
        }
        LabelCommand::Remove(args) => {
            let task = remove_task_label(db_path, board, actor, &args.task_ref, &args.label)?;
            print_task(json, &task)?;
        }
        LabelCommand::Suggest(args) => {
            let options = LabelSuggestionOptions {
                limit: args.limit,
                atom_limit: args.atom_limit,
                min_score: args.min_score,
            };
            let suggestions = suggest_with_optional_vector_config(
                db_path,
                board,
                &args.task_ref,
                options,
                args.vector_config.as_deref(),
            )?;
            print_or_json(json, &suggestions, || label_suggestion_lines(&suggestions))?;
        }
    }
    Ok(())
}

fn label_suggestion_lines(result: &LabelSuggestionResult) -> String {
    let mut lines = Vec::new();
    if result.selected_labels.is_empty() {
        lines.push("No label suggestions.".to_owned());
    } else {
        lines.extend(result.selected_labels.iter().map(|suggestion| {
            let applied = if suggestion.already_applied {
                " already_applied=true"
            } else {
                ""
            };
            format!(
                "{} score={:.3} weight={:.3}{}",
                suggestion.label_name, suggestion.score, suggestion.weight, applied
            )
        }));
    }
    if result.degraded {
        lines.push(format!("degraded: {}", result.diagnostics.join(",")));
    }
    lines.push(format!(
        "coverage={:.3} residual_norm={:.3} needs_new_label={}",
        result.coverage, result.residual_norm, result.needs_new_label
    ));
    lines.join("\n")
}

fn suggest_with_optional_vector_config(
    db_path: &PathBuf,
    board: &str,
    task_ref: &str,
    options: LabelSuggestionOptions,
    vector_config_path: Option<&std::path::Path>,
) -> Result<LabelSuggestionResult> {
    #[cfg(not(feature = "vector-lancedb"))]
    let _ = vector_config_path;
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = configured_lancedb_store(db_path, vector_config_path)? {
            return suggest_task_labels_with(db_path, board, task_ref, &store, options)
                .map_err(Into::into);
        }
    }
    suggest_task_labels(db_path, board, task_ref, options).map_err(Into::into)
}

#[cfg(feature = "vector-lancedb")]
fn configured_lancedb_store(
    db_path: &Path,
    vector_config_path: Option<&Path>,
) -> Result<Option<kanban_vector::LanceDbStore>> {
    let Some(config) = kanban_local::resolved_vector_config(vector_config_path)? else {
        return Ok(None);
    };
    if config.provider != "ollama" {
        return Err(anyhow::anyhow!(
            "unsupported vector provider in config: {}",
            config.provider
        ));
    }
    let provider = Arc::new(kanban_vector::OllamaEmbeddingProvider::new(
        config.endpoint.clone(),
        config.model.clone(),
        config.dimensions,
    )?);
    kanban_vector::LanceDbStore::connect(kanban_vector::LanceDbConfig::new(
        kanban_local::vector_store_path(db_path.to_path_buf()),
        provider,
    ))
    .map(Some)
    .map_err(Into::into)
}
