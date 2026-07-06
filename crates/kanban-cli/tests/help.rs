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
        ],
    )?;

    let vector_query_label_atoms = kanban_help(&["vector", "query-label-atoms"])?;
    assert_contains_all(
        &vector_query_label_atoms,
        &["--text-file <PATH|->", "--vector-json-file <PATH|->"],
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
            "--replace",
            "Clear existing importable records before loading input",
            "use only with an intentional backup/restore flow",
        ],
    )?;
    assert_no_line(&import, "  kanban import --input backup.jsonl")?;
    assert_contains_all(&import, &["  kanban import --input backup.jsonl --replace"])?;

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
