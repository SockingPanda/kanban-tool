use anyhow::{Context, Result};
use clap::{CommandFactory, Parser};
use clap_complete::Shell;
use kanban_core::{Locale, set_current_locale};
use kanban_sqlite::{
    CompletionCandidateKind, begin_database_runtime, completion_candidates, dispatch_once,
    init_database, list_events, list_runs,
};
use std::{ffi::OsString, io::Write};

use crate::args::*;
use crate::commands::{
    board::handle_board,
    comment::handle_comment,
    common::{active_board, default_actor, default_db_path},
    dep::handle_dep,
    dispatch::{dispatch_loop, dispatch_options},
    index::handle_index,
    label::handle_label,
    maintenance::{
        handle_backup, handle_checkpoint, handle_doctor, handle_export, handle_import,
        handle_stats, handle_vacuum,
    },
    run::handle_run,
    search::handle_search,
    serve::serve,
    signal::handle_signal,
    substrate::{
        handle_context, handle_derived, handle_entity, handle_graph, handle_outbox, handle_vector,
    },
    task::handle_task,
};
use crate::output::print_or_json;

pub(crate) fn run() -> Result<()> {
    let cli = Cli::parse_from(normalize_agent_bool_flags(std::env::args_os()));
    if let Command::Completions { shell } = &cli.command {
        generate_completions(*shell)?;
        return Ok(());
    }
    if let Command::Complete(args) = &cli.command {
        let db_path = cli.db.clone().unwrap_or_else(default_db_path);
        let board = active_board(cli.board.as_deref()).unwrap_or_else(|_| "default".to_owned());
        let candidates = completion_candidates(
            &db_path,
            &board,
            completion_kind(args.kind),
            args.current.as_deref(),
        );
        for candidate in candidates {
            println!("{candidate}");
        }
        return Ok(());
    }

    let db_path = cli.db.clone().unwrap_or_else(default_db_path);
    let actor = cli.actor.clone().unwrap_or_else(default_actor);
    let board = active_board(cli.board.as_deref())?;
    let locale = cli_locale(cli.locale.as_deref())?;
    set_current_locale(locale);
    match cli.command {
        Command::Init { force: _ } => {
            let result = init_database(&db_path, &actor)
                .with_context(|| format!("failed to initialize {}", db_path.display()))?;
            print_or_json(cli.json, &result, || {
                format!(
                    "Initialized Kanban database at {}\nDefault board: {}",
                    result.db_path.display(),
                    result.board_slug
                )
            })?;
        }
        Command::Board { command } => handle_board(command, &db_path, &board, &actor, cli.json)?,
        Command::Task { command } => handle_task(command, &db_path, &board, &actor, cli.json)?,
        Command::Comment { command } => {
            handle_comment(command, &db_path, &board, &actor, cli.json)?
        }
        Command::Label { command } => handle_label(command, &db_path, &board, &actor, cli.json)?,
        Command::Dep { command } => handle_dep(command, &db_path, &board, &actor, cli.json)?,
        Command::Events { task_ref } => {
            let events = list_events(&db_path, &board, task_ref.as_deref())?;
            print_or_json(cli.json, &events, || {
                events
                    .iter()
                    .map(|e| format!("{} {} {:?}", e.id, e.kind, e.task_id))
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
        Command::Runs { task_ref } => {
            let runs = list_runs(&db_path, &board, task_ref.as_deref())?;
            print_or_json(cli.json, &runs, || {
                runs.iter()
                    .map(|r| {
                        format!(
                            "{} [{}] task={} exit={:?}",
                            r.id, r.status, r.task_id, r.exit_code
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })?;
        }
        Command::Run { command } => handle_run(command, &db_path, cli.json)?,
        Command::Search(args) => handle_search(args, &db_path, &board, cli.json)?,
        Command::Signal { command } => handle_signal(command, &db_path, &board, &actor, cli.json)?,
        Command::Index { command } => handle_index(command, &db_path, &board, cli.json)?,
        Command::Entity { command } => handle_entity(command, &db_path, cli.json)?,
        Command::Outbox { command } => handle_outbox(command, &db_path, cli.json)?,
        Command::Derived { command } => handle_derived(command, &db_path, cli.json)?,
        Command::Graph { command } => handle_graph(command, &db_path, &board, cli.json)?,
        Command::Vector { command } => handle_vector(command, &db_path, &board, cli.json)?,
        Command::Context { command } => handle_context(command, &db_path, &board, cli.json)?,
        Command::Dispatch(args) => {
            let options = dispatch_options(&args, actor.clone())?;
            if args.once {
                let result = dispatch_once(&db_path, &board, options)?;
                print_or_json(cli.json, &result, || {
                    format!(
                        "claimed={} task={:?} exit={:?}",
                        result.claimed, result.task_id, result.exit_code
                    )
                })?;
            } else {
                let _runtime_guard = begin_database_runtime(&db_path)?;
                let summary = dispatch_loop(
                    &db_path,
                    &board,
                    options,
                    args.poll_interval_ms,
                    args.max_iterations,
                )?;
                print_or_json(cli.json, &summary, || {
                    format!(
                        "iterations={} claimed={}",
                        summary.iterations, summary.claimed
                    )
                })?;
            }
        }
        Command::Serve(args) => serve(args, db_path, &board, actor)?,
        Command::Completions { .. } => unreachable!("handled before database initialization"),
        Command::Complete(..) => unreachable!("handled before database initialization"),
        Command::Doctor => handle_doctor(&db_path, cli.json)?,
        Command::Stats => handle_stats(&db_path, &board, cli.json)?,
        Command::Backup(args) => handle_backup(&db_path, args, cli.json)?,
        Command::Export(args) => handle_export(&db_path, &board, args, cli.json)?,
        Command::Import(args) => handle_import(&db_path, &actor, args, cli.json)?,
        Command::Checkpoint => handle_checkpoint(&db_path, cli.json)?,
        Command::Vacuum => handle_vacuum(&db_path, cli.json)?,
    }
    Ok(())
}

fn normalize_agent_bool_flags<I>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = OsString>,
{
    let mut normalized = Vec::new();
    let mut args = args.into_iter().peekable();
    while let Some(arg) = args.next() {
        if let Some(value) = arg
            .to_str()
            .and_then(|value| value.strip_prefix("--required="))
        {
            match parse_bool_literal(value) {
                Some(true) => normalized.push(OsString::from("--required")),
                Some(false) => normalized.push(OsString::from("--optional")),
                None => normalized.push(arg),
            }
            continue;
        }

        if arg.to_str() == Some("--required")
            && let Some(value) = args
                .peek()
                .and_then(|next| next.to_str())
                .and_then(parse_bool_literal)
        {
            normalized.push(OsString::from(if value {
                "--required"
            } else {
                "--optional"
            }));
            let _ = args.next();
            continue;
        }

        normalized.push(arg);
    }
    normalized
}

fn parse_bool_literal(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn cli_locale(flag: Option<&str>) -> Result<Locale> {
    let env_locale = std::env::var("KANBAN_LOCALE").ok();
    Locale::explicit_or_system(flag.or(env_locale.as_deref())).map_err(anyhow::Error::msg)
}

fn completion_kind(kind: CompleteKind) -> CompletionCandidateKind {
    match kind {
        CompleteKind::TaskRef => CompletionCandidateKind::TaskRef,
        CompleteKind::DependencyTaskRef => CompletionCandidateKind::DependencyTaskRef,
        CompleteKind::Board => CompletionCandidateKind::Board,
        CompleteKind::Status => CompletionCandidateKind::Status,
        CompleteKind::CommentKind => CompletionCandidateKind::CommentKind,
    }
}

fn generate_completions(shell: Shell) -> Result<()> {
    let mut command = Cli::command();
    let mut buffer = Vec::new();
    clap_complete::generate(shell, &mut command, "kanban", &mut buffer);
    match shell {
        Shell::Bash => buffer.extend_from_slice(BASH_DYNAMIC_COMPLETIONS.as_bytes()),
        Shell::Zsh => buffer.extend_from_slice(ZSH_DYNAMIC_COMPLETIONS.as_bytes()),
        _ => {}
    }
    std::io::stdout().write_all(&buffer)?;
    Ok(())
}

const BASH_DYNAMIC_COMPLETIONS: &str = r#"

# Dynamic kanban candidates. Static clap completions remain the fallback.
_kanban_dynamic_completions() {
    local cur prev words kind
    local -a cmd positional
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    words=" ${COMP_WORDS[*]} "
    kind=""
    cmd=(kanban)

    for ((i = 1; i < COMP_CWORD; i++)); do
        case "${COMP_WORDS[i]}" in
            --db|--board|--locale)
                if (( i + 1 < COMP_CWORD )); then
                    cmd+=("${COMP_WORDS[i]}" "${COMP_WORDS[i + 1]}")
                    ((i++))
                fi
                ;;
            --actor)
                if (( i + 1 < COMP_CWORD )); then
                    ((i++))
                fi
                ;;
            --json)
                ;;
            --*)
                ;;
            *)
                positional+=("${COMP_WORDS[i]}")
                ;;
        esac
    done

    if [[ "$cur" == -* ]]; then
        _kanban "$@"
        return
    fi

    case "$prev" in
        --board)
            kind="board"
            ;;
        --status)
            case "${positional[0]} ${positional[1]}" in
                "task create"|"task list"|search\ *)
                    kind="status"
                    ;;
            esac
            ;;
        --kind)
            if [[ "${positional[0]} ${positional[1]}" == "comment add" ]]; then
                kind="comment-kind"
            fi
            ;;
    esac

    if [[ -z "$kind" ]]; then
        case "${positional[0]} ${positional[1]} ${#positional[@]}" in
            "dep add 2"|"dep add 3"|"dep remove 2"|"dep remove 3"|"dep list 2")
                kind="dependency-task-ref"
                ;;
            "task show 2"|"task update 2"|"task promote 2"|"task start 2"|"task claim 2"|"task heartbeat 2"|"task done 2"|"task complete 2"|"task review 2"|"task block 2"|"task unblock 2"|"task archive 2"|"comment list 2"|"comment add 2"|"context build 2")
                kind="task-ref"
                ;;
            "board show 2"|"board use 2"|"board archive 2")
                kind="board"
                ;;
        esac
        case "${positional[0]} ${#positional[@]}" in
            "events 1"|"runs 1")
                kind="task-ref"
                ;;
        esac
    fi

    if [[ -n "$kind" ]]; then
        COMPREPLY=( $(compgen -W "$("${cmd[@]}" __complete "$kind" "$cur" 2>/dev/null)" -- "$cur") )
        return 0
    fi
    _kanban "$@"
}
complete -o default -F _kanban_dynamic_completions kanban
"#;

const ZSH_DYNAMIC_COMPLETIONS: &str = r#"

# Dynamic kanban candidates. Static clap completions remain above as documentation/fallback.
_kanban_dynamic_completions() {
  local -a candidates cmd positional
  local cur="${words[CURRENT]}" prev="${words[CURRENT-1]}"
  local kind="" output
  cmd=(kanban)
  for ((i = 2; i < CURRENT; i++)); do
    case "${words[i]}" in
      --db|--board|--locale)
        if (( i + 1 < CURRENT )); then
          cmd+=("${words[i]}" "${words[i + 1]}")
          ((i++))
        fi
        ;;
      --actor)
        if (( i + 1 < CURRENT )); then
          ((i++))
        fi
        ;;
      --json)
        ;;
      --*)
        ;;
      *)
        positional+=("${words[i]}")
        ;;
    esac
  done
  if [[ "$cur" == -* ]]; then
    _kanban "$@"
    return
  fi
  case "$prev" in
    --board)
      kind="board"
      ;;
    --status)
      case "${positional[1]} ${positional[2]}" in
        "task create"|"task list"|search\ *)
          kind="status"
          ;;
      esac
      ;;
    --kind)
      if [[ "${positional[1]} ${positional[2]}" == "comment add" ]]; then
        kind="comment-kind"
      fi
      ;;
  esac
  if [[ -z "$kind" ]]; then
    case "${positional[1]} ${positional[2]} ${#positional[@]}" in
      "dep add 2"|"dep add 3"|"dep remove 2"|"dep remove 3"|"dep list 2")
        kind="dependency-task-ref"
        ;;
      "task show 2"|"task update 2"|"task promote 2"|"task start 2"|"task claim 2"|"task heartbeat 2"|"task done 2"|"task complete 2"|"task review 2"|"task block 2"|"task unblock 2"|"task archive 2"|"comment list 2"|"comment add 2"|"context build 2")
        kind="task-ref"
        ;;
      "board show 2"|"board use 2"|"board archive 2")
        kind="board"
        ;;
    esac
    case "${positional[1]} ${#positional[@]}" in
      "events 1"|"runs 1")
        kind="task-ref"
        ;;
    esac
  fi
  if [[ -n "$kind" ]]; then
    output="$("${cmd[@]}" __complete "$kind" "$PREFIX" 2>/dev/null)"
    candidates=("${(@f)output}")
    compadd -- "$candidates[@]"
    return
  fi
  _kanban "$@"
}
compdef _kanban_dynamic_completions kanban
"#;
