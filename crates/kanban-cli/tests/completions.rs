use assert_cmd::Command;

fn kanban() -> anyhow::Result<Command> {
    Command::cargo_bin("kanban").map_err(Into::into)
}

#[test]
fn bash_completions_include_core_commands() -> anyhow::Result<()> {
    let output = kanban()?.args(["completions", "bash"]).output()?;

    anyhow::ensure!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout)?;
    for expected in ["_kanban", "task", "show"] {
        anyhow::ensure!(
            stdout.contains(expected),
            "expected bash completions to contain {expected:?}, got:\n{stdout}"
        );
    }
    Ok(())
}

#[test]
fn fish_completions_are_generated() -> anyhow::Result<()> {
    let output = kanban()?.args(["completions", "fish"]).output()?;

    anyhow::ensure!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout)?;
    anyhow::ensure!(
        stdout.contains("complete -c kanban"),
        "expected fish completion directives, got:\n{stdout}"
    );
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
