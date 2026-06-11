use assert_cmd::Command;

mod common;

fn kanban() -> anyhow::Result<Command> {
    Command::cargo_bin("kanban").map_err(Into::into)
}

#[test]
fn completions_are_generated_for_all_documented_shells() -> anyhow::Result<()> {
    for (shell, expected) in [
        ("bash", "_kanban"),
        ("fish", "complete -c kanban"),
        ("zsh", "#compdef kanban"),
        ("powershell", "Register-ArgumentCompleter"),
        ("elvish", "edit:completion:arg-completer[kanban]"),
    ] {
        let output = kanban()?.args(["completions", shell]).output()?;

        anyhow::ensure!(
            output.status.success(),
            "shell: {shell}\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout)?;
        for expected in [expected, "task", "show"] {
            anyhow::ensure!(
                stdout.contains(expected),
                "expected {shell} completions to contain {expected:?}, got:\n{stdout}"
            );
        }
    }
    Ok(())
}

#[test]
fn invalid_completion_shell_is_rejected_by_clap() -> anyhow::Result<()> {
    let output = kanban()?.args(["completions", "invalid-shell"]).output()?;

    anyhow::ensure!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8(output.stderr)?;
    anyhow::ensure!(
        stderr.contains("invalid value 'invalid-shell'"),
        "expected clap invalid value error, got:\n{stderr}"
    );
    Ok(())
}

#[test]
fn completions_do_not_open_db_or_resolve_board() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let db_path = temp.path().join("missing-parent").join("kb.db");
    let output = kanban()?
        .current_dir(temp.path())
        .env_remove("KB_BOARD")
        .arg("--db")
        .arg(&db_path)
        .args([
            "--board",
            "definitely-not-a-real-board",
            "completions",
            "bash",
        ])
        .output()?;

    anyhow::ensure!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    anyhow::ensure!(
        !db_path.exists(),
        "completions unexpectedly created database at {}",
        db_path.display()
    );
    anyhow::ensure!(
        !temp.path().join(".kb").exists(),
        "completions unexpectedly created board config under {}",
        temp.path().display()
    );

    let entries = std::fs::read_dir(temp.path())?.collect::<Result<Vec<_>, _>>()?;
    anyhow::ensure!(
        entries.is_empty(),
        "completions unexpectedly created files under {}: {:?}",
        temp.path().display(),
        entries
            .iter()
            .map(|entry| entry.file_name())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn help_lists_completions_command() -> anyhow::Result<()> {
    let output = kanban()?.arg("--help").output()?;

    anyhow::ensure!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout)?;
    anyhow::ensure!(
        stdout.contains("completions"),
        "expected help to list completions command, got:\n{stdout}"
    );
    Ok(())
}

#[test]
fn dynamic_completion_returns_task_refs_from_temp_db() -> anyhow::Result<()> {
    let temp = common::TempDb::new("completion-task-refs")?;
    common::kanban(&temp.path, &["init"])?.success()?;
    let task = common::kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "Wire dynamic completions",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = task["data"]["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing task id in {task}"))?;

    let stdout = common::kanban(&temp.path, &["__complete", "task-ref"])?.success_stdout()?;
    assert_candidates_include(&stdout, &["default#1", "#1", task_id])?;
    Ok(())
}

#[test]
fn dynamic_completion_returns_board_slugs() -> anyhow::Result<()> {
    let temp = common::TempDb::new("completion-board-slugs")?;
    common::kanban(&temp.path, &["init"])?.success()?;
    common::kanban(
        &temp.path,
        &["board", "create", "agent-work", "--name", "Agent Work"],
    )?
    .success()?;

    let stdout = common::kanban(&temp.path, &["__complete", "board"])?.success_stdout()?;
    assert_candidates_include(&stdout, &["default", "agent-work"])?;
    Ok(())
}

#[test]
fn dynamic_completion_returns_enum_candidates_without_db() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let db_path = temp.path().join("missing").join("kb.db");

    let statuses = common::kanban(&db_path, &["__complete", "status"])?.success_stdout()?;
    assert_candidates_include(
        &statuses,
        &[
            "triage",
            "todo",
            "scheduled",
            "ready",
            "running",
            "blocked",
            "review",
            "done",
            "archived",
        ],
    )?;

    let kinds = common::kanban(&db_path, &["__complete", "comment-kind"])?.success_stdout()?;
    assert_candidates_include(&kinds, &["text", "system", "worker"])?;
    Ok(())
}

#[test]
fn dynamic_completion_missing_db_is_quiet_success() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let db_path = temp.path().join("missing-parent").join("kb.db");
    let output = kanban()?
        .current_dir(temp.path())
        .env_remove("KB_BOARD")
        .arg("--db")
        .arg(&db_path)
        .args(["__complete", "task-ref"])
        .output()?;

    anyhow::ensure!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    anyhow::ensure!(
        output.stdout.is_empty(),
        "expected empty stdout, got:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    anyhow::ensure!(
        output.stderr.is_empty(),
        "expected empty stderr, got:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    anyhow::ensure!(
        !db_path.exists(),
        "completion unexpectedly created database at {}",
        db_path.display()
    );
    Ok(())
}

#[test]
fn generated_bash_and_zsh_completions_include_dynamic_helper_hook() -> anyhow::Result<()> {
    for shell in ["bash", "zsh"] {
        let output = kanban()?.args(["completions", shell]).output()?;
        anyhow::ensure!(
            output.status.success(),
            "shell: {shell}\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout)?;
        for expected in [
            "kanban __complete",
            "task-ref",
            "task update",
            "dependency-task-ref",
            "dep list",
            "\"$cur\" == -*",
            "comment-kind",
        ] {
            anyhow::ensure!(
                stdout.contains(expected),
                "expected {shell} completion script to contain {expected:?}, got:\n{stdout}"
            );
        }
    }
    Ok(())
}

fn assert_candidates_include(stdout: &str, expected: &[&str]) -> anyhow::Result<()> {
    let candidates = stdout.lines().collect::<Vec<_>>();
    for expected in expected {
        anyhow::ensure!(
            candidates.contains(expected),
            "expected candidates to include {expected:?}, got:\n{stdout}"
        );
    }
    Ok(())
}
