use std::io::{self, Write};

use clap::{Args, CommandFactory, ValueEnum};
pub(crate) use clap_complete::Shell;

use crate::{Cli, config, error::CliFailure};

#[derive(Debug, Args)]
pub(crate) struct CompleteArgs {
    pub(crate) kind: CompleteKind,
    pub(crate) current: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum CompleteKind {
    #[value(name = "task-ref")]
    TaskRef,
    #[value(name = "dependency-task-ref")]
    DependencyTaskRef,
    Board,
    Status,
    #[value(name = "comment-kind")]
    CommentKind,
}

/// 生成静态 clap completion，并仅为 Bash/Zsh 附加无数据库动态候选 helper。
pub(crate) fn generate(shell: Shell) -> io::Result<()> {
    let mut command = Cli::command();
    let mut buffer = Vec::new();
    clap_complete::generate(shell, &mut command, "kanban", &mut buffer);
    match shell {
        Shell::Bash => buffer.extend_from_slice(BASH_DYNAMIC.as_bytes()),
        Shell::Zsh => buffer.extend_from_slice(ZSH_DYNAMIC.as_bytes()),
        _ => {}
    }
    io::stdout().write_all(&buffer)
}

pub(crate) fn complete(args: &CompleteArgs, board_flag: Option<&str>) -> Result<(), CliFailure> {
    let current = args.current.as_deref().unwrap_or_default();
    let candidates: Vec<String> = match args.kind {
        // Canonical task refs belong to the host. Completion never opens or probes Turso;
        // returning no candidates is a quiet, successful fallback while the host is absent.
        CompleteKind::TaskRef | CompleteKind::DependencyTaskRef => Vec::new(),
        CompleteKind::Board => config::resolve_board(board_flag)
            .ok()
            .map(|resolved| vec![resolved.value])
            .unwrap_or_default(),
        CompleteKind::Status => vec![
            "triage",
            "todo",
            "scheduled",
            "ready",
            "running",
            "blocked",
            "review",
            "done",
            "archived",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        CompleteKind::CommentKind => vec!["note", "decision", "signal"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
    };
    for candidate in candidates {
        if current.is_empty() || candidate.starts_with(current) {
            println!("{candidate}");
        }
    }
    Ok(())
}

const BASH_DYNAMIC: &str = r#"

# Dynamic candidates are intentionally host-independent. They never open a database.
_kanban_dynamic_completions() {
    local cur prev kind
    local -a cmd positional
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    cmd=(kanban)
    for ((i = 1; i < COMP_CWORD; i++)); do
      case "${COMP_WORDS[i]}" in
        --db|--board|--locale)
          if (( i + 1 < COMP_CWORD )); then
            cmd+=("${COMP_WORDS[i]}" "${COMP_WORDS[i + 1]}")
            ((i++))
          fi
          ;;
        --actor) ((i++)) ;;
        --json|--*) ;;
        *) positional+=("${COMP_WORDS[i]}") ;;
      esac
    done
    if [[ "$cur" == -* ]]; then
      _kanban "$@"
      return
    fi
    case "$prev" in
      --board) kind="board" ;;
      --status) kind="status" ;;
      --kind) kind="comment-kind" ;;
    esac
    if [[ -z "$kind" ]]; then
      case "${positional[0]} ${positional[1]} ${#positional[@]}" in
        "dep add 2"|"dep add 3"|"dep remove 2"|"dep remove 3"|"dep list 2")
          kind="dependency-task-ref" ;;
        "task show 2"|"task update 2"|"task promote 2"|"task start 2"|"task claim 2"|"task heartbeat 2"|"task done 2"|"task complete 2"|"task review 2"|"task block 2"|"task unblock 2"|"task archive 2"|"comment list 2"|"comment add 2"|"context build 2")
          kind="task-ref" ;;
        "board show 2"|"board use 2"|"board archive 2")
          kind="board" ;;
      esac
    fi
    if [[ -n "$kind" ]]; then
      # Keep this exact argv shape stable for shell integrations: "${cmd[@]} __complete".
      COMPREPLY=( $("${cmd[@]}" __complete "$kind" "$cur" 2>/dev/null) )
      return 0
    fi
    _kanban "$@"
}
complete -o default -F _kanban_dynamic_completions kanban
"#;

const ZSH_DYNAMIC: &str = r#"

# Dynamic candidates are intentionally host-independent. They never open a database.
_kanban_dynamic_completions() {
  local cur="${words[CURRENT]}" prev="${words[CURRENT-1]}" kind="" output
  local -a cmd candidates positional
  cmd=(kanban)
  for ((i = 2; i < CURRENT; i++)); do
    case "${words[i]}" in
      --db|--board|--locale)
        if (( i + 1 < CURRENT )); then
          cmd+=("${words[i]}" "${words[i + 1]}")
          ((i++))
        fi
        ;;
      --actor) ((i++)) ;;
      --json|--*) ;;
      *) positional+=("${words[i]}") ;;
    esac
  done
  if [[ "$cur" == -* ]]; then
    _kanban "$@"
    return
  fi
  case "$prev" in
    --board) kind="board" ;;
    --status) kind="status" ;;
    --kind) kind="comment-kind" ;;
  esac
  if [[ -z "$kind" ]]; then
    case "${positional[1]} ${positional[2]} ${#positional[@]}" in
      "dep add 2"|"dep add 3"|"dep remove 2"|"dep remove 3"|"dep list 2")
        kind="dependency-task-ref" ;;
      "task show 2"|"task update 2"|"task promote 2"|"task start 2"|"task claim 2"|"task heartbeat 2"|"task done 2"|"task complete 2"|"task review 2"|"task block 2"|"task unblock 2"|"task archive 2"|"comment list 2"|"comment add 2"|"context build 2")
        kind="task-ref" ;;
      "board show 2"|"board use 2"|"board archive 2")
        kind="board" ;;
    esac
  fi
  if [[ -n "$kind" ]]; then
    output="$("${cmd[@]}" __complete "$kind" "$cur" 2>/dev/null)"
    candidates=("${(@f)output}")
    compadd -- "$candidates[@]"
    return
  fi
  _kanban "$@"
}
compdef _kanban_dynamic_completions kanban
"#;
