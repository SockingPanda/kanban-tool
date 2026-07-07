#[allow(dead_code)]
#[path = "../src/args.rs"]
mod args;

use clap::CommandFactory;

fn kanban_help(args: &[&str]) -> anyhow::Result<String> {
    let mut root = args::Cli::command();
    let mut command = &mut root;

    for arg in args {
        command = command
            .find_subcommand_mut(arg)
            .ok_or_else(|| anyhow::anyhow!("missing subcommand {arg:?} in help path {args:?}"))?;
    }

    let mut output = Vec::new();
    command.write_long_help(&mut output)?;
    String::from_utf8(output).map_err(Into::into)
}

#[test]
fn public_command_groups_have_subcommand_descriptions() -> anyhow::Result<()> {
    for args in [
        &[][..],
        &["task"],
        &["task", "step"],
        &["board"],
        &["comment"],
        &["signal"],
        &["hook"],
        &["hook", "codex"],
        &["hook", "codex", "handle"],
        &["label"],
        &["label", "semantics"],
        &["label", "atoms"],
        &["label", "atom-index"],
        &["label", "proposals"],
        &["label", "ontology"],
        &["label", "ontology", "apply"],
        &["dep"],
        &["index"],
        &["entity"],
        &["outbox"],
        &["derived"],
        &["graph"],
        &["vector"],
        &["context"],
        &["run"],
    ] {
        let stdout = kanban_help(args)?;
        assert_command_descriptions(args, &stdout)?;
    }
    Ok(())
}

#[test]
fn key_agent_facing_help_includes_examples_and_safe_input_guidance() -> anyhow::Result<()> {
    let root = kanban_help(&[])?;
    assert_contains_all(
        &root,
        &[
            "Examples:",
            "kanban task create \"Write spec\" --description-file -",
            "kanban comment add default#1 --body-file - --kind note",
        ],
    )?;

    let task_create = kanban_help(&["task", "create"])?;
    assert_contains_all(
        &task_create,
        &[
            "--description-file <PATH|->",
            "recommended for multiline or shell-sensitive text",
            "Use --description-file - for multiline or shell-sensitive text containing $, backticks, or JSON.",
        ],
    )?;

    let comment_add = kanban_help(&["comment", "add"])?;
    assert_contains_all(
        &comment_add,
        &[
            "--body-file <PATH|->",
            "--metadata-json-file <PATH|->",
            "avoids shell quoting issues",
            "kanban comment add default#1 --body-file - --kind note",
        ],
    )?;

    let signal_record = kanban_help(&["signal", "record"])?;
    assert_contains_all(
        &signal_record,
        &[
            "kanban signal record --input signal.json --json",
            "kanban signal record --input - --json < signal.json",
            "source must be a string",
            "put structured command, cwd, exit_code, and stderr details under evidence",
        ],
    )?;

    let vector_query_label_atoms = kanban_help(&["vector", "query-label-atoms"])?;
    assert_contains_all(
        &vector_query_label_atoms,
        &["--text-file <PATH|->", "--vector-json-file <PATH|->"],
    )?;

    let label_ontology_validate = kanban_help(&["label", "ontology", "validate"])?;
    assert_contains_all(
        &label_ontology_validate,
        &[
            "--input <PATH|->",
            "Read external validation JSON from PATH, or stdin with -",
            "--trusted",
            "Run the trusted automated collector; cannot be combined with --input",
            "--positive-control <TASK_REF>",
            "negative atom trusted validation",
            "--positive-control-waiver <REASON>",
            "--positive-control-waiver-file <PATH|->",
            "kanban label ontology validate act_01 --input - --status failed --json",
            "kanban label ontology validate act_01 --trusted --status passed --positive-control default#1 --json",
        ],
    )?;

    Ok(())
}

#[test]
fn dangerous_flags_explain_their_semantics() -> anyhow::Result<()> {
    let init = kanban_help(&["init"])?;
    assert_contains_all(&init, &["--force", "Deprecated compatibility no-op"])?;

    let archive = kanban_help(&["task", "archive"])?;
    assert_contains_all(
        &archive,
        &[
            "--force",
            "Archive even when normal archive guards would reject the task",
        ],
    )?;

    let import = kanban_help(&["import"])?;
    assert_contains_all(
        &import,
        &[
            "--dry-run",
            "Validate the import in a temporary database without replacing the selected database",
            "--replace",
            "Clear existing importable records before loading input",
            "use only with an intentional backup/restore flow",
        ],
    )?;
    assert_no_line(&import, "  kanban import --input backup.jsonl")?;
    assert_contains_all(&import, &["  kanban import --input backup.jsonl --dry-run"])?;
    assert_contains_all(&import, &["  kanban import --input backup.jsonl --replace"])?;

    Ok(())
}

#[test]
fn dangerous_label_and_vector_flags_explain_boundaries() -> anyhow::Result<()> {
    struct HelpCase<'a> {
        path: &'a [&'a str],
        needles: &'a [&'a str],
    }

    let cases = [
        HelpCase {
            path: &["label", "delete"],
            needles: &[
                "--force",
                "Remove task bindings before deleting the label identity",
                "does not delete semantics or ontology atoms",
            ],
        },
        HelpCase {
            path: &["label", "semantics", "upsert"],
            needles: &[
                "--replace",
                "Replace existing semantics text and atom lists instead of merging edits",
                "use for intentional full rewrites with review evidence",
            ],
        },
        HelpCase {
            path: &["label", "propose"],
            needles: &[
                "--allow-retarget",
                "Allow proposal input or source signals to target a task other than TASK_REF",
                "requires a retarget reason when the target changes",
            ],
        },
        HelpCase {
            path: &["label", "proposals", "accept"],
            needles: &[
                "--allow-retarget",
                "Allow accepting a proposal whose stored target is being retargeted",
                "requires a retarget reason when the target changes",
            ],
        },
        HelpCase {
            path: &["label", "ontology", "apply", "atom"],
            needles: &[
                "--allow-retarget",
                "Allow applying atom changes to a label different from the signal target",
                "requires a retarget reason when the target changes",
            ],
        },
        HelpCase {
            path: &["label", "ontology", "resolve"],
            needles: &[
                "--no-change",
                "Resolve signals as handled without applying ontology changes",
                "use when the reason explains why no label or atom change is needed",
            ],
        },
        HelpCase {
            path: &["vector", "configure"],
            needles: &[
                "--skip-check",
                "Write vector config without probing the provider endpoint and model",
                "use only when the provider is intentionally unavailable or checked elsewhere",
            ],
        },
    ];

    for case in cases {
        let stdout = kanban_help(case.path)?;
        assert_contains_all(&stdout, case.needles)?;
    }

    Ok(())
}

#[test]
fn rich_file_stdin_leaf_commands_advertise_safe_input_examples() -> anyhow::Result<()> {
    let step_done = kanban_help(&["task", "step", "done"])?;
    assert_contains_all(
        &step_done,
        &[
            "--note-file <PATH|->",
            "Read note text from PATH, or stdin with -",
            "kanban task step done default#1 step_01 --note-file -",
        ],
    )?;

    let step_skip = kanban_help(&["task", "step", "skip"])?;
    assert_contains_all(
        &step_skip,
        &[
            "--reason-file <PATH|->",
            "Read reason text from PATH, or stdin with -",
            "kanban task step skip default#1 step_01 --reason-file -",
        ],
    )?;
    assert_no_line(
        &step_skip,
        "  kanban task step reopen default#1 step_01 --reason-file reason.md",
    )?;

    let step_reopen = kanban_help(&["task", "step", "reopen"])?;
    assert_contains_all(
        &step_reopen,
        &[
            "--reason-file <PATH|->",
            "Read reason text from PATH, or stdin with -",
            "kanban task step reopen default#1 step_01 --reason-file -",
        ],
    )?;
    assert_no_line(
        &step_reopen,
        "  kanban task step skip default#1 step_01 --reason-file reason.md",
    )?;

    let step_not_required = kanban_help(&["task", "step", "not-required"])?;
    assert_contains_all(
        &step_not_required,
        &[
            "--reason-file <PATH|->",
            "Read reason text from PATH, or stdin with -",
            "kanban task step not-required default#1 --reason-file reason.md",
        ],
    )?;

    let signal_confirm = kanban_help(&["signal", "confirm"])?;
    assert_contains_all(
        &signal_confirm,
        &[
            "--reason-file <PATH|->",
            "Read reason text from PATH, or stdin with -",
            "kanban signal confirm sig_01 --reason-file -",
        ],
    )?;

    let signal_resolve = kanban_help(&["signal", "resolve"])?;
    assert_contains_all(
        &signal_resolve,
        &[
            "--reason-file <PATH|->",
            "Read reason text from PATH, or stdin with -",
            "kanban signal resolve sig_01 --reason-file reason.md",
        ],
    )?;

    let graph_query = kanban_help(&["graph", "query"])?;
    assert_contains_all(
        &graph_query,
        &[
            "--sparql-file <PATH|->",
            "Read SPARQL query from PATH, or stdin with -",
            "kanban graph query --sparql-file query.rq --limit 100",
            "kanban graph query --sparql-file -",
        ],
    )?;

    Ok(())
}

#[test]
fn claim_and_force_leaf_help_explains_guard_boundaries() -> anyhow::Result<()> {
    let claim = kanban_help(&["task", "claim"])?;
    assert_contains_all(&claim, &["--ttl-ms <TTL_MS>", "[default: 300000]"])?;

    let done = kanban_help(&["task", "done"])?;
    assert_contains_all(
        &done,
        &[
            "--claim-token <CLAIM_TOKEN>",
            "--force",
            "Bypass normal finish guards when intentionally closing without an active claim",
        ],
    )?;

    let block = kanban_help(&["task", "block"])?;
    assert_contains_all(
        &block,
        &[
            "--reason-file <PATH|->",
            "--claim-token <CLAIM_TOKEN>",
            "Bypass normal block guards when intentionally blocking without an active claim",
        ],
    )?;

    Ok(())
}

fn assert_command_descriptions(args: &[&str], stdout: &str) -> anyhow::Result<()> {
    let mut in_commands = false;
    let mut checked = 0usize;
    for line in stdout.lines() {
        if line.trim() == "Commands:" {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        if line.trim().is_empty() {
            break;
        }
        if !line.starts_with("  ") {
            continue;
        }
        let trimmed = line.trim();
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let command = parts.next().unwrap_or_default();
        if command == "help" {
            continue;
        }
        let description = parts.next().unwrap_or_default().trim();
        anyhow::ensure!(
            !description.is_empty(),
            "missing help description for command {command:?} in args {args:?}:\n{stdout}"
        );
        checked += 1;
    }
    anyhow::ensure!(
        checked > 0,
        "no command rows found in help for args {args:?}:\n{stdout}"
    );
    Ok(())
}

fn assert_contains_all(haystack: &str, needles: &[&str]) -> anyhow::Result<()> {
    for needle in needles {
        anyhow::ensure!(
            haystack.contains(needle),
            "expected help to contain {needle:?}, got:\n{haystack}"
        );
    }
    Ok(())
}

fn assert_no_line(haystack: &str, unexpected_line: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !haystack.lines().any(|line| line == unexpected_line),
        "expected help not to contain line {unexpected_line:?}, got:\n{haystack}"
    );
    Ok(())
}
