mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_with_stdin};
use serde_json::json;
use std::fs;

#[test]
fn signal_lifecycle_reason_file_and_stdin_preserve_shell_sensitive_text() -> anyhow::Result<()> {
    let temp = TempDb::new("signal_lifecycle_reason_file_and_stdin_preserve_shell_sensitive_text")?;
    kanban(&temp.path, &["init"])?.success()?;
    let input_path = temp.dir.join("signal.json");
    fs::write(
        &input_path,
        json!({
            "kind": "agent_cli_failure",
            "title": "agent-facing CLI failure",
            "summary": "A command failed while testing file input.",
            "severity": "medium",
            "actor": "codex",
            "agent_type": "codex",
            "source": "cli-test",
            "evidence": {"command": "kanban test"}
        })
        .to_string(),
    )?;
    let recorded = kanban(
        &temp.path,
        &[
            "--json",
            "signal",
            "record",
            "--input",
            input_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;
    let signal_id = recorded["data"]["signal"]["id"]
        .as_str()
        .context("signal id")?;

    let reason = "confirmed with `cmd` $VAR $(date)\nsecond line";
    let reason_path = temp.dir.join("signal-reason.md");
    fs::write(&reason_path, reason)?;
    let confirmed = kanban(
        &temp.path,
        &[
            "--json",
            "signal",
            "confirm",
            signal_id,
            "--reason-file",
            reason_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;
    assert_eq!(confirmed["data"][0]["review_reason"], reason);

    let source = kanban(
        &temp.path,
        &[
            "--json",
            "signal",
            "record",
            "--input",
            input_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;
    let source_id = source["data"]["signal"]["id"]
        .as_str()
        .context("source signal id")?;
    let replacement = kanban(
        &temp.path,
        &[
            "--json",
            "signal",
            "record",
            "--input",
            input_path.to_str().context("utf-8 path")?,
        ],
    )?
    .success_json()?;
    let replacement_id = replacement["data"]["signal"]["id"]
        .as_str()
        .context("replacement signal id")?;
    let supersede_reason = "superseded from stdin {\"json\":true}\n$VAR remains literal";
    let superseded = kanban_with_stdin(
        &temp.path,
        &[
            "--json",
            "signal",
            "supersede",
            source_id,
            "--by",
            replacement_id,
            "--reason-file",
            "-",
        ],
        supersede_reason,
    )?
    .success_json()?;
    assert_eq!(superseded["data"][0]["review_reason"], supersede_reason);
    Ok(())
}

#[test]
fn signal_lifecycle_rejects_inline_reason_with_reason_file() -> anyhow::Result<()> {
    let temp = TempDb::new("signal_lifecycle_rejects_inline_reason_with_reason_file")?;
    kanban(&temp.path, &["init"])?.success()?;
    let reason_path = temp.dir.join("signal-reason.md");
    fs::write(&reason_path, "from file")?;

    kanban(
        &temp.path,
        &[
            "signal",
            "confirm",
            "sig_missing",
            "--reason",
            "inline",
            "--reason-file",
            reason_path.to_str().context("utf-8 path")?,
        ],
    )?
    .failure_containing("mutually exclusive")?;
    Ok(())
}
