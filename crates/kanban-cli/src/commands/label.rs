use std::path::PathBuf;
#[cfg(feature = "vector-lancedb")]
use std::{path::Path, sync::Arc};

use anyhow::{Result, bail};
use kanban_sqlite::{
    CreateLabel, LabelProposalCandidate, LabelProposalListOptions, LabelProposalStatus,
    LabelSemanticProposalRecord, LabelSuggestionOptions, LabelSuggestionResult,
    MAX_TASK_LIST_LIMIT, ManualLabelProposalProvider, UpsertLabelSemantics, accept_label_proposal,
    add_task_label, create_label, delete_label_semantics, get_label_proposal, get_label_semantics,
    label_atom_index_status, list_label_atoms, list_label_proposals, list_label_semantics,
    list_labels, propose_task_label_with, reject_label_proposal, remove_task_label,
    suggest_task_labels, upsert_label_semantics,
};
#[cfg(feature = "vector-lancedb")]
use kanban_sqlite::{
    label_atom_index_status_with, propose_task_label_with_store, query_label_atom_index_with,
    rebuild_label_atom_index_with, suggest_task_labels_with,
};
use std::{fs, str::FromStr};

use crate::args::{LabelAtomPolarityArg, LabelCommand};
use crate::commands::common::validate_page_bounds;
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
        LabelCommand::Semantics { command } => {
            handle_label_semantics(command, db_path, board, json)?
        }
        LabelCommand::Atoms { command } => match command {
            crate::args::LabelAtomsCommand::List => {
                let atoms = list_label_atoms(db_path, board)?;
                print_or_json(json, &atoms, || {
                    atoms
                        .iter()
                        .map(|atom| {
                            format!(
                                "{} {} {} [{}] score_source={}",
                                atom.label_name, atom.polarity, atom.kind, atom.id, atom.text
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })?;
            }
        },
        LabelCommand::AtomIndex { command } => {
            handle_label_atom_index(command, db_path, board, json)?
        }
        LabelCommand::Suggest(args) => {
            validate_label_suggest_bounds(args.limit, args.atom_limit)?;
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
        LabelCommand::Propose(args) => {
            validate_label_suggest_bounds(args.limit, args.atom_limit)?;
            let options = LabelSuggestionOptions {
                limit: args.limit,
                atom_limit: args.atom_limit,
                min_score: args.min_score,
            };
            let attempt = if let Some(path) = args.proposal_json {
                let candidate = read_proposal_candidate(&path)?;
                let provider = ManualLabelProposalProvider::new(candidate);
                propose_with_optional_vector_config(
                    db_path,
                    board,
                    actor,
                    &args.task_ref,
                    &provider,
                    options,
                    args.vector_config.as_deref(),
                )?
            } else {
                propose_with_optional_vector_config(
                    db_path,
                    board,
                    actor,
                    &args.task_ref,
                    &kanban_sqlite::DisabledLabelProposalProvider,
                    options,
                    args.vector_config.as_deref(),
                )?
            };
            print_or_json(json, &attempt, || {
                if let Some(proposal) = &attempt.proposal {
                    format!(
                        "{} {} [{}] coverage={:.3} residual_norm={:.3}",
                        proposal.id,
                        proposal.name,
                        proposal.status,
                        attempt.heuristic_coverage,
                        attempt.heuristic_residual_norm
                    )
                } else {
                    format!(
                        "No label proposal. degraded={} diagnostics={}",
                        attempt.degraded,
                        attempt.diagnostics.join(",")
                    )
                }
            })?;
        }
        LabelCommand::Proposals { command } => match command {
            crate::args::LabelProposalsCommand::List(args) => {
                let status = args
                    .status
                    .as_deref()
                    .map(LabelProposalStatus::from_str)
                    .transpose()?;
                let proposals = list_label_proposals(
                    db_path,
                    board,
                    LabelProposalListOptions {
                        task_ref: args.task,
                        status,
                    },
                )?;
                print_or_json(json, &proposals, || proposal_lines(&proposals))?;
            }
            crate::args::LabelProposalsCommand::Show { proposal_id } => {
                let proposal = get_label_proposal(db_path, &proposal_id)?;
                print_or_json(json, &proposal, || proposal_line(&proposal))?;
            }
            crate::args::LabelProposalsCommand::Accept(args) => {
                let proposal =
                    accept_label_proposal(db_path, actor, &args.proposal_id, args.reason)?;
                print_or_json(json, &proposal, || proposal_line(&proposal))?;
            }
            crate::args::LabelProposalsCommand::Reject(args) => {
                let proposal =
                    reject_label_proposal(db_path, actor, &args.proposal_id, args.reason)?;
                print_or_json(json, &proposal, || proposal_line(&proposal))?;
            }
        },
    }
    Ok(())
}

fn handle_label_semantics(
    command: crate::args::LabelSemanticsCommand,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        crate::args::LabelSemanticsCommand::List => {
            let semantics = list_label_semantics(db_path, board)?;
            print_or_json(json, &semantics, || label_semantics_lines(&semantics))?;
        }
        crate::args::LabelSemanticsCommand::Show { label } => {
            let semantics = get_label_semantics(db_path, board, &label)?;
            print_or_json(json, &semantics, || label_semantics_line(&semantics))?;
        }
        crate::args::LabelSemanticsCommand::Upsert(args) => {
            let semantics = upsert_label_semantics(
                db_path,
                board,
                UpsertLabelSemantics {
                    label_ref: args.label,
                    description: args.description,
                    applies_when: args.applies_when,
                    excludes_when: args.excludes_when,
                    positive_examples: args.positive_examples,
                    negative_examples: args.negative_examples,
                },
            )?;
            print_or_json(json, &semantics, || label_semantics_line(&semantics))?;
        }
        crate::args::LabelSemanticsCommand::Delete { label } => {
            delete_label_semantics(db_path, board, &label)?;
            let deleted = serde_json::json!({ "deleted": true });
            print_or_json(json, &deleted, || {
                format!("Deleted label semantics for {label}")
            })?;
        }
    }
    Ok(())
}

fn handle_label_atom_index(
    command: crate::args::LabelAtomIndexCommand,
    db_path: &PathBuf,
    board: &str,
    json: bool,
) -> Result<()> {
    match command {
        crate::args::LabelAtomIndexCommand::Status(args) => {
            let status = label_atom_index_status_optional_config(
                db_path,
                board,
                args.vector_config.as_deref(),
            )?;
            print_or_json(json, &status, || {
                format!(
                    "label atom index backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        crate::args::LabelAtomIndexCommand::Rebuild(args) => {
            let status =
                rebuild_configured_label_atom_index(db_path, board, args.vector_config.as_path())?;
            print_or_json(json, &status, || {
                format!(
                    "label atom index backend={} enabled={}: {}",
                    status.backend, status.enabled, status.message
                )
            })?;
        }
        crate::args::LabelAtomIndexCommand::Query(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let hits = query_configured_label_atom_index(
                db_path,
                board,
                &args.text,
                args.polarity,
                args.limit,
                args.vector_config.as_path(),
            )?;
            print_or_json(json, &hits, || {
                if hits.is_empty() {
                    "No label atom hits.".to_owned()
                } else {
                    hits.iter()
                        .map(|hit| {
                            format!(
                                "{} {} {} distance={:.3} {}",
                                hit.label_name, hit.polarity, hit.kind, hit.distance, hit.text
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            })?;
        }
    }
    Ok(())
}

fn validate_label_suggest_bounds(limit: usize, atom_limit: usize) -> Result<()> {
    if limit == 0 {
        bail!("limit must be >= 1");
    }
    if atom_limit == 0 {
        bail!("atom_limit must be >= 1");
    }
    validate_page_bounds(limit, MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(atom_limit, MAX_TASK_LIST_LIMIT, 0)?;
    Ok(())
}

fn label_semantics_lines(records: &[kanban_sqlite::LabelSemanticsRecord]) -> String {
    if records.is_empty() {
        return "No label semantics.".to_owned();
    }
    records
        .iter()
        .map(label_semantics_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn label_semantics_line(record: &kanban_sqlite::LabelSemanticsRecord) -> String {
    format!(
        "{} description={} applies={} excludes={} examples=+{}/-{} atoms={}",
        record.label_name,
        record.description.as_deref().unwrap_or("-"),
        record.applies_when.len(),
        record.excludes_when.len(),
        record.positive_examples.len(),
        record.negative_examples.len(),
        record.atoms.len()
    )
}

fn label_atom_index_status_optional_config(
    db_path: &PathBuf,
    board: &str,
    vector_config_path: Option<&std::path::Path>,
) -> Result<kanban_vector::VectorStoreStatus> {
    #[cfg(not(feature = "vector-lancedb"))]
    let _ = vector_config_path;
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = configured_lancedb_store(db_path, vector_config_path)? {
            return label_atom_index_status_with(db_path, board, &store).map_err(Into::into);
        }
    }
    label_atom_index_status(db_path, board).map_err(Into::into)
}

fn rebuild_configured_label_atom_index(
    db_path: &PathBuf,
    board: &str,
    vector_config_path: &std::path::Path,
) -> Result<kanban_vector::VectorStoreStatus> {
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = configured_lancedb_store(db_path, Some(vector_config_path))? {
            return rebuild_label_atom_index_with(db_path, board, &store).map_err(Into::into);
        }
    }
    #[cfg(not(feature = "vector-lancedb"))]
    let _ = (db_path, board, vector_config_path);
    bail!("label atom index rebuild requires a configured label atom vector store")
}

fn query_configured_label_atom_index(
    db_path: &PathBuf,
    board: &str,
    text: &str,
    polarity: Option<LabelAtomPolarityArg>,
    limit: usize,
    vector_config_path: &std::path::Path,
) -> Result<Vec<kanban_vector::LabelAtomHit>> {
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = configured_lancedb_store(db_path, Some(vector_config_path))? {
            return query_label_atom_index_with(
                db_path,
                board,
                &store,
                kanban_vector::LabelAtomQuery {
                    text: text.to_owned(),
                    limit,
                    board_id: None,
                    embedding_model: None,
                    polarity: polarity.map(label_atom_polarity_value),
                },
            )
            .map_err(Into::into);
        }
    }
    #[cfg(not(feature = "vector-lancedb"))]
    let _ = (db_path, board, text, polarity, limit, vector_config_path);
    bail!("label atom index query requires a configured label atom vector store")
}

#[cfg(feature = "vector-lancedb")]
fn label_atom_polarity_value(polarity: LabelAtomPolarityArg) -> String {
    match polarity {
        LabelAtomPolarityArg::Positive => "positive".to_owned(),
        LabelAtomPolarityArg::Negative => "negative".to_owned(),
    }
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

fn read_proposal_candidate(path: &std::path::Path) -> Result<LabelProposalCandidate> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(Into::into)
}

fn proposal_lines(proposals: &[LabelSemanticProposalRecord]) -> String {
    if proposals.is_empty() {
        return "No label proposals.".to_owned();
    }
    proposals
        .iter()
        .map(proposal_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn proposal_line(proposal: &LabelSemanticProposalRecord) -> String {
    let resolved = proposal
        .resolved_label_id
        .as_deref()
        .map(|id| format!(" resolved_label_id={id}"))
        .unwrap_or_default();
    format!(
        "{} {} [{}] task={} coverage={:.3} residual_norm={:.3}{}",
        proposal.id,
        proposal.name,
        proposal.status,
        proposal.task_id,
        proposal.heuristic_coverage,
        proposal.heuristic_residual_norm,
        resolved
    )
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

fn propose_with_optional_vector_config(
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    task_ref: &str,
    provider: &dyn kanban_sqlite::LabelProposalProvider,
    options: LabelSuggestionOptions,
    vector_config_path: Option<&std::path::Path>,
) -> Result<kanban_sqlite::LabelProposalAttempt> {
    #[cfg(not(feature = "vector-lancedb"))]
    let _ = vector_config_path;
    #[cfg(feature = "vector-lancedb")]
    {
        if let Some(store) = configured_lancedb_store(db_path, vector_config_path)? {
            return propose_task_label_with_store(
                db_path, board, actor, task_ref, provider, &store, options,
            )
            .map_err(Into::into);
        }
    }
    propose_task_label_with(db_path, board, actor, task_ref, provider, options).map_err(Into::into)
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
