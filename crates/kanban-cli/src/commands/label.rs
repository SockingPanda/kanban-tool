use std::path::{Path, PathBuf};
#[cfg(any())]
use std::sync::Arc;

use anyhow::{Result, bail};
use kanban_sqlite::{
    BootstrapTaskLabel, CreateLabel, LabelOntologyActionInput, LabelOntologyActionRecord,
    LabelOntologyActionType, LabelOntologyActor, LabelOntologyAtomApplyInput,
    LabelOntologyCandidateAtomInput, LabelOntologyProposedAction, LabelOntologyQualityOptions,
    LabelOntologyRecordInput, LabelOntologyRetargetOptions, LabelOntologyRevertInput,
    LabelOntologyReviewGroupBy, LabelOntologyReviewOptions, LabelOntologySignalInput,
    LabelOntologySignalKind, LabelOntologySuggestState, LabelOntologyValidationInput,
    LabelOntologyValidationStatus, LabelProposalCandidate, LabelProposalCreateOptions,
    LabelProposalDecisionOptions, LabelProposalListOptions, LabelProposalStatus,
    LabelSemanticProposalRecord, LabelSemanticsMutationOptions, LabelSuggestionOptions,
    LabelSuggestionResult, MAX_TASK_LIST_LIMIT, ManualLabelProposalProvider, UpsertLabelSemantics,
    accept_label_proposal_with_options, add_task_labels_with_options,
    apply_label_ontology_atom_with_options, bootstrap_task_label,
    clear_label_semantics_with_options, create_label_ontology_action, delete_label,
    explain_label_atom, get_label_ontology_signal, get_label_proposal, get_label_semantics,
    label_atom_index_status, label_ontology_quality_report, list_label_atoms,
    list_label_ontology_signals, list_label_proposals, list_label_semantics, list_labels,
    propose_task_label_with_create_options, record_label_ontology_observation,
    reject_label_proposal, remove_task_label, revert_label_ontology_mutation,
    review_label_ontology, suggest_task_labels, upsert_label_semantics_with_options,
    validate_label_ontology_action,
};
#[cfg(any())]
use kanban_sqlite::{
    LabelOntologyTrustedValidationInput, validate_label_ontology_action_with_trusted_suggestions,
};
#[cfg(any())]
use kanban_sqlite::{
    bootstrap_task_label_with_staged_verification, label_atom_index_status_with,
    query_label_atom_index_with, rebuild_label_atom_index_with, suggest_task_labels_with,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{fs, io::Read, str::FromStr};

use crate::args::{
    LabelAtomPolarityArg, LabelCommand, LabelOntologyActorArgs, LabelOntologyActorTypeArg,
    LabelOntologyAtomKindArg, LabelOntologyReviewGroupByArg, LabelOntologyValidationStatusArg,
};
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
            let label = kanban_sqlite::create_label_with_actor(
                db_path,
                board,
                actor,
                CreateLabel {
                    name: args.name,
                    color: args.color,
                },
            )?;
            print_or_json(json, &label, || label_line(&label))?;
        }
        LabelCommand::Delete(args) => {
            let result = delete_label(db_path, board, actor, &args.label, args.force)?;
            print_or_json(json, &result, || {
                format!(
                    "Deleted label {} removed_task_bindings={} removed_semantics={} removed_atoms={}",
                    result.label.name,
                    result.removed_task_bindings,
                    result.removed_semantics,
                    result.removed_atoms
                )
            })?;
        }
        LabelCommand::Bootstrap(args) => {
            let verify = args.verify || args.vector_config.is_some();
            if verify {
                validate_label_bootstrap_verification_score(args.min_verify_score)?;
                ensure_label_bootstrap_verification_available(
                    db_path,
                    args.vector_config.as_deref(),
                )?;
            }
            let bootstrap_input = BootstrapTaskLabel {
                name: args.label,
                description: args.description,
                applies_when: args.applies_when,
                excludes_when: args.excludes_when,
                positive_examples: args.positive_examples,
                negative_examples: args.negative_examples,
            };
            let output = if verify {
                #[cfg(any())]
                {
                    let Some(store) =
                        configured_lancedb_store(db_path, args.vector_config.as_deref())?
                    else {
                        bail!(
                            "{LABEL_VECTOR_HELPER_ADAPTER_UNAVAILABLE}; omit --verify to bootstrap without vector verification"
                        );
                    };
                    let result = bootstrap_task_label_with_staged_verification(
                        db_path,
                        board,
                        actor,
                        &args.task_ref,
                        bootstrap_input,
                        &store,
                        args.min_verify_score,
                    )?;
                    LabelBootstrapCommandOutput {
                        task: result.task,
                        semantics: result.semantics,
                        verification: Some(result.verification),
                    }
                }
                {
                    let _ = (db_path, board, actor, bootstrap_input);
                    bail!(
                        "{LABEL_VECTOR_HELPER_ADAPTER_UNAVAILABLE}; omit --verify to bootstrap without vector verification"
                    );
                }
            } else {
                let result =
                    bootstrap_task_label(db_path, board, actor, &args.task_ref, bootstrap_input)?;
                LabelBootstrapCommandOutput {
                    task: result.task,
                    semantics: result.semantics,
                    verification: None,
                }
            };
            print_or_json(json, &output, || label_bootstrap_lines(&output))?;
        }
        LabelCommand::Add(args) => {
            let result = add_task_labels_with_options(
                db_path,
                board,
                actor,
                &args.task_ref,
                &args.labels,
                args.create_missing,
            )?;
            if args.create_missing {
                let output = LabelAddCommandOutput {
                    task: result.task,
                    created_labels: result.created_labels,
                };
                print_or_json(json, &output, || label_add_lines(&output))?;
            } else {
                print_task(json, &result.task)?;
            }
        }
        LabelCommand::Remove(args) => {
            let task = remove_task_label(db_path, board, actor, &args.task_ref, &args.label)?;
            print_task(json, &task)?;
        }
        LabelCommand::Semantics { command } => {
            handle_label_semantics(command, db_path, board, actor, json)?
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
            crate::args::LabelAtomsCommand::Explain { atom_ref } => {
                let explain = explain_label_atom(db_path, board, &atom_ref)?;
                print_or_json(json, &explain, || label_atom_explain_lines(&explain))?;
            }
        },
        LabelCommand::AtomIndex { command } => {
            handle_label_atom_index(command, db_path, board, json)?
        }
        LabelCommand::Suggest(args) => {
            validate_label_suggest_bounds(
                args.limit,
                args.candidate_limit,
                args.atom_limit,
                args.max_selected_labels,
            )?;
            let options = LabelSuggestionOptions {
                output_limit: args.limit,
                candidate_limit: args.candidate_limit,
                atom_limit: args.atom_limit,
                max_selected_labels: args.max_selected_labels,
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
            validate_label_suggest_bounds(
                args.limit,
                args.candidate_limit,
                args.atom_limit,
                args.max_selected_labels,
            )?;
            let options = LabelSuggestionOptions {
                output_limit: args.limit,
                candidate_limit: args.candidate_limit,
                atom_limit: args.atom_limit,
                max_selected_labels: args.max_selected_labels,
                min_score: args.min_score,
            };
            let create_options = LabelProposalCreateOptions {
                source_signal_ids: args.source_signal_ids,
                ontology_actor: Some(label_ontology_cli_actor(actor, &args.ontology_actor)),
                allow_retarget: args.allow_retarget,
                retarget_reason: args.retarget_reason,
            };
            let propose_options = kanban_sqlite::LabelProposalProposeOptions {
                suggestion: options,
                create: create_options,
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
                    propose_options,
                    args.vector_config.as_deref(),
                )?
            } else {
                propose_with_optional_vector_config(
                    db_path,
                    board,
                    actor,
                    &args.task_ref,
                    &kanban_sqlite::DisabledLabelProposalProvider,
                    propose_options,
                    args.vector_config.as_deref(),
                )?
            };
            print_or_json(json, &attempt, || {
                if let Some(proposal) = &attempt.proposal {
                    format!(
                        "{} {} [{}] coverage={:.3} coverage_cosine={:.3} residual_norm={:.3}",
                        proposal.id,
                        proposal.name,
                        proposal.status,
                        attempt.heuristic_coverage,
                        attempt.heuristic_coverage_cosine,
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
                let proposal = accept_label_proposal_with_options(
                    db_path,
                    actor,
                    &args.proposal_id,
                    args.reason,
                    LabelProposalDecisionOptions {
                        source_signal_ids: args.source_signal_ids,
                        ontology_actor: Some(label_ontology_cli_actor(actor, &args.ontology_actor)),
                        allow_retarget: args.allow_retarget,
                        retarget_reason: args.retarget_reason,
                    },
                )?;
                print_or_json(json, &proposal, || proposal_line(&proposal))?;
            }
            crate::args::LabelProposalsCommand::Reject(args) => {
                let proposal =
                    reject_label_proposal(db_path, actor, &args.proposal_id, args.reason)?;
                print_or_json(json, &proposal, || proposal_line(&proposal))?;
            }
        },
        LabelCommand::Ontology { command } => {
            handle_label_ontology(command, db_path, board, actor, json)?
        }
    }
    Ok(())
}

fn handle_label_semantics(
    command: crate::args::LabelSemanticsCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
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
            let args = *args;
            let mut options = LabelSemanticsMutationOptions::manual_actor(actor);
            options.reason = args.reason;
            options.source_signal_ids = args.source_signal_ids;
            let semantics = upsert_label_semantics_with_options(
                db_path,
                board,
                UpsertLabelSemantics {
                    label_ref: args.label,
                    expected_semantics_hash: args.expected_semantics_hash,
                    replace: args.replace,
                    description: args.description,
                    applies_when: args.applies_when,
                    excludes_when: args.excludes_when,
                    positive_examples: args.positive_examples,
                    negative_examples: args.negative_examples,
                    remove_applies_when: args.remove_applies_when,
                    remove_excludes_when: args.remove_excludes_when,
                    remove_positive_examples: args.remove_positive_examples,
                    remove_negative_examples: args.remove_negative_examples,
                },
                options,
            )?;
            print_or_json(json, &semantics, || label_semantics_line(&semantics))?;
        }
        crate::args::LabelSemanticsCommand::Delete(args) => {
            let mut options = LabelSemanticsMutationOptions::manual_actor(actor);
            options.reason = Some(args.reason.clone());
            clear_label_semantics_with_options(
                db_path,
                board,
                &args.label,
                args.expected_semantics_hash,
                options,
            )?;
            let deleted = serde_json::json!({ "deleted": true });
            print_or_json(json, &deleted, || {
                format!("Deleted label semantics for {}", args.label)
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

fn handle_label_ontology(
    command: crate::args::LabelOntologyCommand,
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    json: bool,
) -> Result<()> {
    match command {
        crate::args::LabelOntologyCommand::Record(args) => {
            if args.capture_suggest && args.suggestion_snapshot.is_some() {
                bail!("--capture-suggest cannot be used with --suggestion-snapshot");
            }
            let input = read_label_ontology_record_input(db_path, board, &args)?;
            let observation =
                record_label_ontology_observation(db_path, board, &args.task_ref, input)?;
            print_or_json(json, &observation, || {
                label_ontology_observation_line(&observation)
            })?;
        }
        crate::args::LabelOntologyCommand::List(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let signals = list_label_ontology_signals(
                db_path,
                board,
                label_ontology_list_options(
                    args.status,
                    args.kind,
                    args.task,
                    args.label,
                    args.proposed_label,
                    args.include_all,
                    args.limit,
                )?,
            )?;
            print_or_json(json, &signals, || label_ontology_signal_lines(&signals))?;
        }
        crate::args::LabelOntologyCommand::Show { signal_id } => {
            let detail = get_label_ontology_signal(db_path, &signal_id)?;
            print_or_json(json, &detail, || {
                format!(
                    "{}\nobservation={} task={} actions={}",
                    label_ontology_signal_line(&detail.signal),
                    detail.observation.id,
                    detail.observation.task_ref_snapshot,
                    detail.actions.len()
                )
            })?;
        }
        crate::args::LabelOntologyCommand::Review(args) => {
            validate_page_bounds(args.limit, MAX_TASK_LIST_LIMIT, 0)?;
            let groups = review_label_ontology(
                db_path,
                board,
                LabelOntologyReviewOptions {
                    group_by: label_ontology_review_group_by_value(args.group_by),
                    include_all: args.include_all,
                    limit: args.limit,
                },
            )?;
            print_or_json(json, &groups, || label_ontology_review_group_lines(&groups))?;
        }
        crate::args::LabelOntologyCommand::Quality(args) => {
            validate_page_bounds(args.sample_limit, MAX_TASK_LIST_LIMIT, 0)?;
            let report = label_ontology_quality_report(
                db_path,
                board,
                LabelOntologyQualityOptions {
                    sample_limit: args.sample_limit,
                },
            )?;
            print_or_json(json, &report, || label_ontology_quality_line(&report))?;
        }
        crate::args::LabelOntologyCommand::Confirm(args) => {
            let ontology_actor = label_ontology_cli_actor(actor, &args.actor);
            let action = create_label_ontology_action(
                db_path,
                board,
                label_ontology_action_input(
                    ontology_actor,
                    LabelOntologyActionType::Confirm,
                    args.signal_ids,
                    args.reason,
                    None,
                ),
            )?;
            print_or_json(json, &action, || label_ontology_action_line(&action))?;
        }
        crate::args::LabelOntologyCommand::Reject(args) => {
            let ontology_actor = label_ontology_cli_actor(actor, &args.actor);
            let action = create_label_ontology_action(
                db_path,
                board,
                label_ontology_action_input(
                    ontology_actor,
                    LabelOntologyActionType::Reject,
                    args.signal_ids,
                    args.reason,
                    None,
                ),
            )?;
            print_or_json(json, &action, || label_ontology_action_line(&action))?;
        }
        crate::args::LabelOntologyCommand::Supersede(args) => {
            let ontology_actor = label_ontology_cli_actor(actor, &args.actor);
            let action = create_label_ontology_action(
                db_path,
                board,
                label_ontology_action_input(
                    ontology_actor,
                    LabelOntologyActionType::Supersede,
                    args.signal_ids,
                    args.reason,
                    Some(args.superseded_by_signal_id),
                ),
            )?;
            print_or_json(json, &action, || label_ontology_action_line(&action))?;
        }
        crate::args::LabelOntologyCommand::Resolve(args) => {
            if !args.no_change {
                bail!("resolve currently requires --no-change");
            }
            let ontology_actor = label_ontology_cli_actor(actor, &args.actor);
            let action = create_label_ontology_action(
                db_path,
                board,
                label_ontology_action_input(
                    ontology_actor,
                    LabelOntologyActionType::ResolveNoChange,
                    args.signal_ids,
                    args.reason,
                    None,
                ),
            )?;
            print_or_json(json, &action, || label_ontology_action_line(&action))?;
        }
        crate::args::LabelOntologyCommand::Apply { command } => match command {
            crate::args::LabelOntologyApplyCommand::Atom(args) => {
                let action = apply_label_ontology_atom_with_options(
                    db_path,
                    board,
                    LabelOntologyAtomApplyInput {
                        actor: label_ontology_cli_actor(actor, &args.actor),
                        signal_ids: args.signal_ids,
                        label_ref: args.label,
                        kind: label_ontology_atom_kind_value(args.kind).to_owned(),
                        text: args.text,
                        reason: args.reason,
                    },
                    LabelOntologyRetargetOptions {
                        allow_retarget: args.allow_retarget,
                        retarget_reason: args.retarget_reason,
                    },
                )?;
                print_or_json(json, &action, || label_ontology_action_line(&action))?;
            }
        },
        crate::args::LabelOntologyCommand::Revert(args) => {
            let action = revert_label_ontology_mutation(
                db_path,
                board,
                LabelOntologyRevertInput {
                    actor: label_ontology_cli_actor(actor, &args.actor),
                    target_action_id: args.action_id,
                    expected_current_hash: args.expected_current_hash,
                    reason: args.reason,
                },
            )?;
            print_or_json(json, &action, || label_ontology_action_line(&action))?;
        }
        crate::args::LabelOntologyCommand::Validate(args) => {
            let validation_status = label_ontology_validation_status(args.status);
            let action = if args.trusted {
                validate_label_suggest_bounds(
                    args.limit,
                    args.candidate_limit,
                    args.atom_limit,
                    args.max_selected_labels,
                )?;
                if args.input.is_some() {
                    bail!(
                        "--trusted collects validation evidence from label suggest; do not pass --input"
                    );
                }
                let options = LabelSuggestionOptions {
                    output_limit: args.limit,
                    candidate_limit: args.candidate_limit,
                    atom_limit: args.atom_limit,
                    max_selected_labels: args.max_selected_labels,
                    min_score: args.min_score,
                };
                validate_label_ontology_action_with_trusted_cli_evidence(
                    db_path,
                    board,
                    actor,
                    &args.actor,
                    args.action_id,
                    args.signal_ids,
                    args.reason,
                    validation_status,
                    args.positive_controls,
                    args.positive_control_waiver,
                    args.vector_config.as_deref(),
                    options,
                )?
            } else {
                if !args.positive_controls.is_empty() || args.positive_control_waiver.is_some() {
                    bail!("--positive-control and --positive-control-waiver require --trusted");
                }
                let Some(input) = args.input.as_deref() else {
                    bail!("label ontology validate requires --input unless --trusted is used");
                };
                let validation_json = read_json_input_string(input)?;
                validate_label_ontology_action(
                    db_path,
                    board,
                    LabelOntologyValidationInput {
                        actor: label_ontology_cli_actor(actor, &args.actor),
                        parent_action_id: args.action_id,
                        signal_ids: args.signal_ids,
                        reason: args.reason,
                        validation_status,
                        validation_json,
                    },
                )?
            };
            print_or_json(json, &action, || label_ontology_action_line(&action))?;
        }
    }
    Ok(())
}

fn label_ontology_list_options(
    statuses: Vec<String>,
    kinds: Vec<String>,
    task_ref: Option<String>,
    target_label_ref: Option<String>,
    proposed_label_name: Option<String>,
    include_all: bool,
    limit: usize,
) -> Result<kanban_sqlite::LabelOntologySignalListOptions> {
    Ok(kanban_sqlite::LabelOntologySignalListOptions {
        statuses: statuses
            .iter()
            .map(|status| kanban_sqlite::LabelOntologySignalStatus::from_str(status))
            .collect::<kanban_core::Result<Vec<_>>>()?,
        kinds: kinds
            .iter()
            .map(|kind| kanban_sqlite::LabelOntologySignalKind::from_str(kind))
            .collect::<kanban_core::Result<Vec<_>>>()?,
        task_ref,
        target_label_ref,
        proposed_label_name,
        include_all,
        limit,
    })
}

fn read_label_ontology_record_input(
    db_path: &PathBuf,
    board: &str,
    args: &crate::args::LabelOntologyRecordArgs,
) -> Result<LabelOntologyRecordInput> {
    let raw = read_json_input_string(&args.input)?;
    if !args.capture_suggest
        && args.suggestion_snapshot.is_none()
        && let Ok(input) = serde_json::from_str::<LabelOntologyRecordInput>(&raw)
    {
        return Ok(input);
    }

    let capture = serde_json::from_str::<LabelOntologyRecordCaptureInput>(&raw)?;
    capture.into_record_input(captured_or_supplied_suggestion_snapshot(
        db_path, board, args,
    )?)
}

#[derive(Debug, Deserialize)]
struct LabelOntologyRecordCaptureInput {
    actor: LabelOntologyActor,
    #[serde(default)]
    agent_candidates: Option<JsonValue>,
    #[serde(default)]
    agent_candidates_json: Option<String>,
    #[serde(default)]
    suggestion_snapshot: Option<JsonValue>,
    #[serde(default)]
    suggestion_snapshot_json: Option<String>,
    #[serde(default)]
    final_decision: Option<JsonValue>,
    #[serde(default)]
    final_decision_json: Option<String>,
    #[serde(default)]
    diagnostics: Option<JsonValue>,
    #[serde(default)]
    diagnostics_json: Option<String>,
    #[serde(default)]
    capture_fingerprint: Option<String>,
    #[serde(default)]
    signals: Vec<LabelOntologyCaptureSignalInput>,
}

#[derive(Debug, Deserialize)]
struct LabelOntologyCaptureSignalInput {
    kind: LabelOntologySignalKind,
    #[serde(default)]
    target_label_ref: Option<String>,
    #[serde(default)]
    related_labels: Option<JsonValue>,
    #[serde(default)]
    related_labels_json: Option<String>,
    proposed_action: LabelOntologyProposedAction,
    #[serde(default)]
    candidate_atom: Option<LabelOntologyCandidateAtomInput>,
    #[serde(default)]
    proposed_label_name: Option<String>,
    #[serde(default)]
    proposal: Option<JsonValue>,
    #[serde(default)]
    proposal_json: Option<String>,
    #[serde(default)]
    agent_selected: Option<bool>,
    #[serde(default)]
    suggest_state: Option<LabelOntologySuggestState>,
    #[serde(default)]
    suggest_score: Option<f64>,
    #[serde(default)]
    suggest_rank: Option<i64>,
    #[serde(default)]
    final_selected: Option<bool>,
    rationale: String,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    signal_key: Option<String>,
}

impl LabelOntologyRecordCaptureInput {
    fn into_record_input(
        self,
        supplied_snapshot: Option<JsonValue>,
    ) -> Result<LabelOntologyRecordInput> {
        let input_snapshot = coalesce_json_value(
            "suggestion_snapshot",
            self.suggestion_snapshot,
            "suggestion_snapshot_json",
            self.suggestion_snapshot_json,
            None,
        )?;
        let suggestion_snapshot = match (input_snapshot, supplied_snapshot) {
            (Some(input), Some(supplied)) if input != supplied => {
                bail!(
                    "--suggestion-snapshot/--capture-suggest conflicts with input suggestion_snapshot"
                )
            }
            (Some(input), _) => input,
            (_, Some(supplied)) => supplied,
            (None, None) => {
                bail!(
                    "simplified ontology record input requires suggestion_snapshot, --suggestion-snapshot, or --capture-suggest"
                )
            }
        };
        let diagnostics = coalesce_json_value(
            "diagnostics",
            self.diagnostics,
            "diagnostics_json",
            self.diagnostics_json,
            suggestion_snapshot
                .get("diagnostics")
                .cloned()
                .or_else(|| Some(serde_json::json!([]))),
        )?
        .unwrap_or_else(|| serde_json::json!([]));
        Ok(LabelOntologyRecordInput {
            actor: self.actor,
            agent_candidates_json: json_value_to_string(
                coalesce_json_value(
                    "agent_candidates",
                    self.agent_candidates,
                    "agent_candidates_json",
                    self.agent_candidates_json,
                    Some(serde_json::json!([])),
                )?
                .unwrap_or_else(|| serde_json::json!([])),
            )?,
            suggestion_snapshot_json: json_value_to_string(suggestion_snapshot.clone())?,
            final_decision_json: json_value_to_string(
                coalesce_json_value(
                    "final_decision",
                    self.final_decision,
                    "final_decision_json",
                    self.final_decision_json,
                    Some(serde_json::json!({})),
                )?
                .unwrap_or_else(|| serde_json::json!({})),
            )?,
            suggest_coverage: None,
            suggest_coverage_cosine: None,
            suggest_residual_norm: None,
            suggest_needs_new_label: suggestion_snapshot
                .get("needs_new_label")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false),
            suggest_degraded: suggestion_snapshot
                .get("degraded")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false),
            diagnostics_json: json_value_to_string(diagnostics)?,
            capture_fingerprint: self.capture_fingerprint,
            signals: self
                .signals
                .into_iter()
                .map(LabelOntologyCaptureSignalInput::into_signal_input)
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

impl LabelOntologyCaptureSignalInput {
    fn into_signal_input(self) -> Result<LabelOntologySignalInput> {
        Ok(LabelOntologySignalInput {
            kind: self.kind,
            target_label_ref: self.target_label_ref,
            related_labels_json: json_value_to_string(
                coalesce_json_value(
                    "related_labels",
                    self.related_labels,
                    "related_labels_json",
                    self.related_labels_json,
                    Some(serde_json::json!([])),
                )?
                .unwrap_or_else(|| serde_json::json!([])),
            )?,
            proposed_action: self.proposed_action,
            candidate_atom: self.candidate_atom,
            proposed_label_name: self.proposed_label_name,
            proposal_json: json_value_to_string(
                coalesce_json_value(
                    "proposal",
                    self.proposal,
                    "proposal_json",
                    self.proposal_json,
                    Some(serde_json::json!({})),
                )?
                .unwrap_or_else(|| serde_json::json!({})),
            )?,
            agent_selected: self.agent_selected.unwrap_or(false),
            suggest_state: self.suggest_state,
            suggest_score: self.suggest_score,
            suggest_rank: self.suggest_rank,
            final_selected: self.final_selected.unwrap_or(false),
            rationale: self.rationale,
            confidence: self.confidence,
            signal_key: self.signal_key,
        })
    }
}

fn captured_or_supplied_suggestion_snapshot(
    db_path: &PathBuf,
    board: &str,
    args: &crate::args::LabelOntologyRecordArgs,
) -> Result<Option<JsonValue>> {
    if args.capture_suggest {
        let options = LabelSuggestionOptions {
            output_limit: args.limit,
            candidate_limit: args.candidate_limit,
            atom_limit: args.atom_limit,
            max_selected_labels: args.max_selected_labels,
            min_score: args.min_score,
        };
        let suggestions = suggest_with_optional_vector_config(
            db_path,
            board,
            &args.task_ref,
            options,
            args.vector_config.as_deref(),
        )?;
        return Ok(Some(serde_json::to_value(suggestions)?));
    }
    args.suggestion_snapshot
        .as_deref()
        .map(read_suggestion_snapshot_value)
        .transpose()
}

fn read_suggestion_snapshot_value(path: &str) -> Result<JsonValue> {
    let raw = read_json_input_string(path)?;
    normalize_suggestion_snapshot_value(serde_json::from_str(&raw)?)
}

fn normalize_suggestion_snapshot_value(value: JsonValue) -> Result<JsonValue> {
    if let Some(data) = value.get("data")
        && data.is_object()
    {
        return Ok(data.clone());
    }
    if value.is_object() {
        Ok(value)
    } else {
        bail!("suggestion snapshot must be a JSON object or an envelope with object data")
    }
}

fn coalesce_json_value(
    natural_field: &str,
    natural: Option<JsonValue>,
    legacy_field: &str,
    legacy_json: Option<String>,
    default: Option<JsonValue>,
) -> Result<Option<JsonValue>> {
    let legacy = legacy_json
        .map(|raw| serde_json::from_str::<JsonValue>(&raw))
        .transpose()?;
    if let (Some(natural), Some(legacy)) = (&natural, &legacy)
        && natural != legacy
    {
        bail!("{natural_field} conflicts with {legacy_field}");
    }
    Ok(natural.or(legacy).or(default))
}

fn json_value_to_string(value: JsonValue) -> Result<String> {
    serde_json::to_string(&value).map_err(Into::into)
}

fn read_json_input_string(path: &str) -> Result<String> {
    if path == "-" {
        let mut raw = String::new();
        std::io::stdin().read_to_string(&mut raw)?;
        Ok(raw)
    } else {
        fs::read_to_string(path).map_err(Into::into)
    }
}

fn label_ontology_cli_actor(actor: &str, args: &LabelOntologyActorArgs) -> LabelOntologyActor {
    LabelOntologyActor {
        name: actor.to_owned(),
        actor_type: match args.actor_type {
            LabelOntologyActorTypeArg::User => "user",
            LabelOntologyActorTypeArg::Agent => "agent",
        }
        .to_owned(),
        agent_type: args.agent_type.clone(),
    }
}

fn label_ontology_action_input(
    actor: LabelOntologyActor,
    action_type: LabelOntologyActionType,
    signal_ids: Vec<String>,
    reason: String,
    superseded_by_signal_id: Option<String>,
) -> LabelOntologyActionInput {
    LabelOntologyActionInput {
        actor,
        action_type,
        signal_ids,
        reason,
        superseded_by_signal_id,
        parent_action_id: None,
        target_label_ref: None,
        result_label_ref: None,
        result_atom_id: None,
        result_atom_content_hash: None,
        result_proposal_id: None,
        canonical_before_hash: None,
        canonical_after_hash: None,
        change_json: None,
        validation_status: None,
        validation_json: None,
    }
}

fn label_ontology_atom_kind_value(kind: LabelOntologyAtomKindArg) -> &'static str {
    match kind {
        LabelOntologyAtomKindArg::AppliesWhen => "applies_when",
        LabelOntologyAtomKindArg::PositiveExample => "positive_example",
        LabelOntologyAtomKindArg::ExcludesWhen => "excludes_when",
        LabelOntologyAtomKindArg::NegativeExample => "negative_example",
    }
}

fn label_ontology_validation_status(
    status: LabelOntologyValidationStatusArg,
) -> LabelOntologyValidationStatus {
    match status {
        LabelOntologyValidationStatusArg::Passed => LabelOntologyValidationStatus::Passed,
        LabelOntologyValidationStatusArg::Failed => LabelOntologyValidationStatus::Failed,
        LabelOntologyValidationStatusArg::Partial => LabelOntologyValidationStatus::Partial,
    }
}

fn label_ontology_observation_line(
    observation: &kanban_sqlite::LabelOntologyObservationRecord,
) -> String {
    format!(
        "{} task={} signals={} fingerprint={}",
        observation.id,
        observation.task_ref_snapshot,
        observation.signals.len(),
        observation.capture_fingerprint
    )
}

fn label_ontology_signal_lines(signals: &[kanban_sqlite::LabelOntologySignalRecord]) -> String {
    if signals.is_empty() {
        return "No label ontology signals.".to_owned();
    }
    signals
        .iter()
        .map(label_ontology_signal_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn label_ontology_review_group_by_value(
    value: LabelOntologyReviewGroupByArg,
) -> LabelOntologyReviewGroupBy {
    match value {
        LabelOntologyReviewGroupByArg::Label => LabelOntologyReviewGroupBy::Label,
        LabelOntologyReviewGroupByArg::CandidateAtom => LabelOntologyReviewGroupBy::CandidateAtom,
        LabelOntologyReviewGroupByArg::ProposedLabel => LabelOntologyReviewGroupBy::ProposedLabel,
        LabelOntologyReviewGroupByArg::Cluster => LabelOntologyReviewGroupBy::Cluster,
    }
}

fn label_ontology_review_group_lines(groups: &[kanban_sqlite::LabelOntologyReviewGroup]) -> String {
    if groups.is_empty() {
        return "No label ontology review groups.".to_owned();
    }
    groups
        .iter()
        .map(label_ontology_review_group_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn label_ontology_review_group_line(group: &kanban_sqlite::LabelOntologyReviewGroup) -> String {
    let title = match group.group_by {
        LabelOntologyReviewGroupBy::Label => group.label_name.as_deref(),
        LabelOntologyReviewGroupBy::CandidateAtom => group.candidate_text.as_deref(),
        LabelOntologyReviewGroupBy::ProposedLabel => group.proposed_label_name.as_deref(),
        LabelOntologyReviewGroupBy::Cluster => group.cluster_key.as_deref(),
    }
    .or(group.label_name.as_deref())
    .or(group.proposed_label_name.as_deref())
    .or(group.candidate_text.as_deref())
    .unwrap_or(group.key.as_str());
    let avg = group
        .average_score
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "-".to_owned());
    let median = group
        .median_score
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "-".to_owned());
    format!(
        "{} key={} title={} tasks={} signals={} open={} confirmed={} degraded={} avg_score={} median_score={} samples=[{}] signals=[{}] actions=[{}]",
        group.group_by,
        group.key,
        title,
        group.task_count,
        group.signal_count,
        group.open_count,
        group.confirmed_count,
        group.degraded_count,
        avg,
        median,
        group.sample_task_refs.join(","),
        group.signal_ids.join(","),
        group.action_ids.join(",")
    )
}

fn label_ontology_quality_line(report: &kanban_sqlite::LabelOntologyQualityReport) -> String {
    let rate = report
        .rates
        .disagreement_task_rate
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "unavailable".to_owned());
    let warnings = if report.warnings.is_empty() {
        "-".to_owned()
    } else {
        report.warnings.join("; ")
    };
    format!(
        "ontology quality observations={} observed_tasks={} agreement_observations={} raw_signals={} disagreement_tasks={} disagreement_task_rate={} precision_recall={} samples=[{}] warnings={}",
        report.denominator.observation_count,
        report.denominator.distinct_task_count,
        report.denominator.agreement_observation_count,
        report.disagreement.signal_count,
        report.disagreement.distinct_task_count,
        rate,
        if report.precision_recall.available {
            "available"
        } else {
            "unavailable"
        },
        report.denominator.sample_task_refs.join(","),
        warnings
    )
}

fn label_ontology_signal_line(signal: &kanban_sqlite::LabelOntologySignalRecord) -> String {
    let target = signal
        .target_label_name_snapshot
        .as_deref()
        .or(signal.proposed_label_name.as_deref())
        .unwrap_or("-");
    let confidence = signal
        .confidence
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "-".to_owned());
    format!(
        "{} {} [{}] action={} target={} confidence={} rationale={}",
        signal.id,
        signal.kind,
        signal.status,
        signal.proposed_action,
        target,
        confidence,
        signal.rationale
    )
}

fn label_ontology_action_line(action: &LabelOntologyActionRecord) -> String {
    let signals = if action.signal_ids.is_empty() {
        "-".to_owned()
    } else {
        action.signal_ids.join(",")
    };
    let result_atom = action
        .result_atom_id
        .as_deref()
        .map(|id| format!(" result_atom={id}"))
        .unwrap_or_default();
    format!(
        "{} {} signals={} validation={}{} reason={}",
        action.id,
        action.action_type,
        signals,
        action.validation_status,
        result_atom,
        action.reason
    )
}

fn label_atom_explain_lines(explain: &kanban_sqlite::LabelAtomExplainRecord) -> String {
    let mut lines = Vec::new();
    if let Some(atom) = &explain.atom {
        lines.push(format!(
            "{} {} {} [{}] content_hash={} text={}",
            atom.label_name, atom.polarity, atom.kind, atom.id, atom.content_hash, atom.text
        ));
    } else {
        lines.push(format!("No current atom for {}", explain.query));
    }
    if let Some(semantics) = &explain.current_semantics {
        lines.push(format!(
            "semantics label={} atoms={}",
            semantics.label_name,
            semantics.atoms.len()
        ));
    }
    if explain.legacy_untracked {
        lines.push(format!(
            "legacy_untracked: {}",
            explain.legacy_reason.as_deref().unwrap_or("unknown")
        ));
    }
    for provenance in &explain.provenance_actions {
        lines.push(format!(
            "provenance {} {} matched_by={} validation={}",
            provenance.action.id,
            provenance.action.action_type,
            provenance.matched_by,
            provenance.action.validation_status
        ));
    }
    for source in &explain.supporting_signals {
        lines.push(format!(
            "signal {} {} task={} stale={} degraded={}",
            source.signal.id,
            source.signal.kind,
            source.task_ref_snapshot,
            source.suggest_input_stale,
            source.suggest_degraded
        ));
    }
    for validation in &explain.validation_history {
        lines.push(format!(
            "validation {} status={} parent={}",
            validation.action.id, validation.validation_status, validation.parent_action_id
        ));
    }
    lines.join("\n")
}

fn validate_label_suggest_bounds(
    limit: usize,
    candidate_limit: usize,
    atom_limit: usize,
    max_selected_labels: usize,
) -> Result<()> {
    if limit == 0 {
        bail!("limit must be >= 1");
    }
    if candidate_limit == 0 {
        bail!("candidate_limit must be >= 1");
    }
    if atom_limit == 0 {
        bail!("atom_limit must be >= 1");
    }
    if max_selected_labels == 0 {
        bail!("max_selected_labels must be >= 1");
    }
    validate_page_bounds(limit, MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(candidate_limit, MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(atom_limit, MAX_TASK_LIST_LIMIT, 0)?;
    validate_page_bounds(max_selected_labels, MAX_TASK_LIST_LIMIT, 0)?;
    Ok(())
}

fn validate_label_bootstrap_verification_score(min_score: f32) -> Result<()> {
    if !(0.0..=1.0).contains(&min_score) {
        bail!("min_verify_score must be between 0 and 1");
    }
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

#[derive(Debug, Serialize)]
struct LabelAddCommandOutput {
    task: kanban_sqlite::TaskRecord,
    created_labels: Vec<kanban_sqlite::LabelRecord>,
}

fn label_add_lines(result: &LabelAddCommandOutput) -> String {
    let mut lines = crate::output::task_line(&result.task);
    if !result.created_labels.is_empty() {
        let labels = result
            .created_labels
            .iter()
            .map(label_line)
            .collect::<Vec<_>>()
            .join(", ");
        lines.push('\n');
        lines.push_str(&format!("created_labels: {labels}"));
    }
    lines
}

#[derive(Debug, Serialize)]
struct LabelBootstrapCommandOutput {
    task: kanban_sqlite::TaskRecord,
    semantics: kanban_sqlite::LabelSemanticsRecord,
    verification: Option<kanban_sqlite::BootstrapTaskLabelVerification>,
}

fn label_bootstrap_lines(result: &LabelBootstrapCommandOutput) -> String {
    let mut lines = format!(
        "{}\n{}",
        label_semantics_line(&result.semantics),
        crate::output::task_line(&result.task)
    );
    if let Some(verification) = &result.verification {
        lines.push('\n');
        lines.push_str(&format!(
            "verification label={} score={:.3} min_score={:.3} source={}",
            verification.label_name,
            verification.score,
            verification.min_score,
            verification.source
        ));
    }
    lines
}

#[allow(clippy::too_many_arguments)]
fn validate_label_ontology_action_with_trusted_cli_evidence(
    db_path: &PathBuf,
    board: &str,
    actor: &str,
    actor_args: &LabelOntologyActorArgs,
    parent_action_id: String,
    signal_ids: Vec<String>,
    reason: String,
    validation_status: LabelOntologyValidationStatus,
    positive_control_task_refs: Vec<String>,
    positive_control_waiver_reason: Option<String>,
    vector_config_path: Option<&std::path::Path>,
    options: LabelSuggestionOptions,
) -> Result<LabelOntologyActionRecord> {
    #[cfg(any())]
    {
        let Some(store) = configured_lancedb_store(db_path, vector_config_path)? else {
            bail!(
                "trusted ontology validation requires a configured label atom vector store; pass --vector-config <path>"
            );
        };
        rebuild_label_atom_index_with(db_path, board, &store)?;
        validate_label_ontology_action_with_trusted_suggestions(
            db_path,
            board,
            LabelOntologyTrustedValidationInput {
                actor: label_ontology_cli_actor(actor, actor_args),
                parent_action_id,
                signal_ids,
                reason,
                validation_status,
                positive_control_task_refs,
                positive_control_waiver_reason,
            },
            &store,
            options,
        )
        .map_err(Into::into)
    }
    {
        let _ = (
            db_path,
            board,
            actor,
            actor_args,
            parent_action_id,
            signal_ids,
            reason,
            validation_status,
            positive_control_task_refs,
            positive_control_waiver_reason,
            vector_config_path,
            options,
        );
        bail!(
            "{LABEL_VECTOR_HELPER_ADAPTER_UNAVAILABLE}; use external attestation via --input for this CLI build"
        )
    }
}

fn label_atom_index_status_optional_config(
    db_path: &PathBuf,
    board: &str,
    vector_config_path: Option<&std::path::Path>,
) -> Result<kanban_vector::VectorStoreStatus> {
    let _ = vector_config_path;
    #[cfg(any())]
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
    rebuild_configured_label_atom_index_optional(db_path, board, Some(vector_config_path))
}

fn rebuild_configured_label_atom_index_optional(
    db_path: &PathBuf,
    board: &str,
    vector_config_path: Option<&std::path::Path>,
) -> Result<kanban_vector::VectorStoreStatus> {
    #[cfg(any())]
    {
        if let Some(store) = configured_lancedb_store(db_path, vector_config_path)? {
            return rebuild_label_atom_index_with(db_path, board, &store).map_err(Into::into);
        }
    }
    let _ = (db_path, board, vector_config_path);
    bail!(
        "{LABEL_VECTOR_HELPER_ADAPTER_UNAVAILABLE}; use `kanban vector query-label-atoms` for raw helper queries after a separately rebuilt helper index"
    )
}

fn ensure_label_bootstrap_verification_available(
    db_path: &Path,
    vector_config_path: Option<&Path>,
) -> Result<()> {
    #[cfg(any())]
    {
        if configured_lancedb_store(db_path, vector_config_path)?.is_some() {
            return Ok(());
        }
    }
    let _ = (db_path, vector_config_path);
    bail!(
        "{LABEL_VECTOR_HELPER_ADAPTER_UNAVAILABLE}; omit --verify to bootstrap without vector verification"
    )
}

fn query_configured_label_atom_index(
    db_path: &PathBuf,
    board: &str,
    text: &str,
    polarity: Option<LabelAtomPolarityArg>,
    limit: usize,
    vector_config_path: &std::path::Path,
) -> Result<Vec<kanban_vector::LabelAtomHit>> {
    #[cfg(any())]
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
    let _ = (db_path, board, text, polarity, limit, vector_config_path);
    bail!(
        "{LABEL_VECTOR_HELPER_ADAPTER_UNAVAILABLE}; use `kanban vector query-label-atoms` for raw helper queries"
    )
}

const LABEL_VECTOR_HELPER_ADAPTER_UNAVAILABLE: &str =
    "label vector helper adapter is not available in this CLI build";

#[cfg(any())]
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
    let reason_codes = if result.reason_codes.is_empty() {
        "none".to_owned()
    } else {
        result.reason_codes.join(",")
    };
    lines.push(format!(
        "coverage={:.3} coverage_cosine={:.3} residual_norm={:.3} label_coverage_review={} reason_codes={}",
        result.coverage, result.coverage_cosine, result.residual_norm, result.needs_new_label, reason_codes
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
        "{} {} [{}] task={} coverage={:.3} coverage_cosine={:.3} residual_norm={:.3}{}",
        proposal.id,
        proposal.name,
        proposal.status,
        proposal.task_id,
        proposal.heuristic_coverage,
        proposal.heuristic_coverage_cosine,
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
    let _ = vector_config_path;
    #[cfg(any())]
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
    propose_options: kanban_sqlite::LabelProposalProposeOptions,
    vector_config_path: Option<&std::path::Path>,
) -> Result<kanban_sqlite::LabelProposalAttempt> {
    let _ = vector_config_path;
    #[cfg(any())]
    {
        if let Some(store) = configured_lancedb_store(db_path, vector_config_path)? {
            return kanban_sqlite::propose_task_label_with_store_and_create_options(
                db_path,
                board,
                actor,
                task_ref,
                provider,
                &store,
                propose_options,
            )
            .map_err(Into::into);
        }
    }
    let kanban_sqlite::LabelProposalProposeOptions { suggestion, create } = propose_options;
    propose_task_label_with_create_options(
        db_path, board, actor, task_ref, provider, suggestion, create,
    )
    .map_err(Into::into)
}

#[cfg(any())]
fn configured_lancedb_store(
    db_path: &Path,
    vector_config_path: Option<&Path>,
) -> Result<Option<kanban_vector_lancedb::LanceDbStore>> {
    let Some(config) = kanban_local::resolved_vector_config(vector_config_path)? else {
        return Ok(None);
    };
    if config.provider != "ollama" {
        return Err(anyhow::anyhow!(
            "unsupported vector provider in config: {}",
            config.provider
        ));
    }
    let provider = Arc::new(kanban_vector_lancedb::OllamaEmbeddingProvider::new(
        config.endpoint.clone(),
        config.model.clone(),
        config.dimensions,
    )?);
    kanban_vector_lancedb::LanceDbStore::connect(kanban_vector_lancedb::LanceDbConfig::new(
        kanban_local::vector_store_path(db_path.to_path_buf()),
        provider,
    ))
    .map(Some)
    .map_err(Into::into)
}
