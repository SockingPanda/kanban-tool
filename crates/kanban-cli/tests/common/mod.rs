#![allow(dead_code)]

use std::{path::Path, process::Output};

use anyhow::Context;
use assert_cmd::Command;
use predicates::{Predicate, str::contains};

pub fn kanban(db_path: &Path, args: &[&str]) -> anyhow::Result<CmdResult> {
    let current_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
    kanban_in_dir(db_path, args, current_dir)
}

pub fn kanban_with_stdin(db_path: &Path, args: &[&str], stdin: &str) -> anyhow::Result<CmdResult> {
    let current_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
    let mut command =
        Command::cargo_bin("kanban").context("failed to locate kanban test binary")?;
    command
        .current_dir(current_dir)
        .arg("--db")
        .arg(db_path)
        .args(args);
    command.env_remove("KB_BOARD");
    command.env("XDG_CONFIG_HOME", current_dir.join(".xdg-config"));
    let output = command
        .write_stdin(stdin)
        .output()
        .context("failed to execute kanban command")?;
    Ok(CmdResult { output })
}

pub fn kanban_in_dir_envs_with_stdin(
    db_path: &Path,
    args: &[&str],
    stdin: &str,
    current_dir: &Path,
    envs: &[(&str, &Path)],
) -> anyhow::Result<CmdResult> {
    let mut command =
        Command::cargo_bin("kanban").context("failed to locate kanban test binary")?;
    command
        .current_dir(current_dir)
        .arg("--db")
        .arg(db_path)
        .args(args)
        .env_remove("KB_BOARD");
    for (key, value) in envs {
        command.env(key, value);
    }
    if !envs.iter().any(|(key, _)| *key == "XDG_CONFIG_HOME") {
        command.env("XDG_CONFIG_HOME", current_dir.join(".xdg-config"));
    }
    let output = command
        .write_stdin(stdin)
        .output()
        .context("failed to execute kanban command")?;
    Ok(CmdResult { output })
}

pub fn kanban_in_dir(
    db_path: &Path,
    args: &[&str],
    current_dir: &Path,
) -> anyhow::Result<CmdResult> {
    kanban_in_dir_env(db_path, args, current_dir, None)
}

pub fn kanban_in_dir_env(
    db_path: &Path,
    args: &[&str],
    current_dir: &Path,
    board_env: Option<&str>,
) -> anyhow::Result<CmdResult> {
    let mut command =
        Command::cargo_bin("kanban").context("failed to locate kanban test binary")?;
    command
        .current_dir(current_dir)
        .arg("--db")
        .arg(db_path)
        .args(args);
    if let Some(board) = board_env {
        command.env("KB_BOARD", board);
    } else {
        command.env_remove("KB_BOARD");
    }
    command.env("XDG_CONFIG_HOME", current_dir.join(".xdg-config"));
    let output = command
        .output()
        .context("failed to execute kanban command")?;
    Ok(CmdResult { output })
}

pub fn kanban_in_dir_envs(
    db_path: &Path,
    args: &[&str],
    current_dir: &Path,
    envs: &[(&str, &Path)],
) -> anyhow::Result<CmdResult> {
    let mut command =
        Command::cargo_bin("kanban").context("failed to locate kanban test binary")?;
    command
        .current_dir(current_dir)
        .arg("--db")
        .arg(db_path)
        .args(args)
        .env_remove("KB_BOARD");
    for (key, value) in envs {
        command.env(key, value);
    }
    if !envs.iter().any(|(key, _)| *key == "XDG_CONFIG_HOME") {
        command.env("XDG_CONFIG_HOME", current_dir.join(".xdg-config"));
    }
    let output = command
        .output()
        .context("failed to execute kanban command")?;
    Ok(CmdResult { output })
}

pub fn kanban_in_dir_str_envs(
    db_path: &Path,
    args: &[&str],
    current_dir: &Path,
    envs: &[(&str, &str)],
) -> anyhow::Result<CmdResult> {
    let mut command =
        Command::cargo_bin("kanban").context("failed to locate kanban test binary")?;
    command
        .current_dir(current_dir)
        .arg("--db")
        .arg(db_path)
        .args(args)
        .env_remove("KB_BOARD");
    for (key, value) in envs {
        command.env(key, value);
    }
    if !envs.iter().any(|(key, _)| *key == "XDG_CONFIG_HOME") {
        command.env("XDG_CONFIG_HOME", current_dir.join(".xdg-config"));
    }
    let output = command
        .output()
        .context("failed to execute kanban command")?;
    Ok(CmdResult { output })
}

pub fn kanban_without_db_in_dir_str_envs(
    args: &[&str],
    current_dir: &Path,
    envs: &[(&str, &str)],
) -> anyhow::Result<CmdResult> {
    let mut command =
        Command::cargo_bin("kanban").context("failed to locate kanban test binary")?;
    command.current_dir(current_dir).args(args);
    command.env_remove("KB_BOARD");
    command.env_remove("KANBAN_DB");
    command.env_remove("KB_DB");
    for (key, value) in envs {
        command.env(key, value);
    }
    if !envs.iter().any(|(key, _)| *key == "XDG_CONFIG_HOME") {
        command.env("XDG_CONFIG_HOME", current_dir.join(".xdg-config"));
    }
    if !envs.iter().any(|(key, _)| *key == "XDG_DATA_HOME") {
        command.env("XDG_DATA_HOME", current_dir.join(".xdg-data"));
    }
    Ok(CmdResult {
        output: command
            .output()
            .context("failed to execute kanban command")?,
    })
}

pub struct CmdResult {
    pub output: Output,
}

impl CmdResult {
    pub fn success(self) -> anyhow::Result<()> {
        ensure_success(&self.output)?;
        Ok(())
    }

    pub fn success_json(self) -> anyhow::Result<serde_json::Value> {
        ensure_success(&self.output)?;
        serde_json::from_slice(&self.output.stdout)
            .context("failed to parse command stdout as JSON")
    }

    pub fn success_stdout(self) -> anyhow::Result<String> {
        ensure_success(&self.output)?;
        String::from_utf8(self.output.stdout).context("failed to parse command stdout as UTF-8")
    }

    pub fn failure_containing(self, expected: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.output.status.success(),
            "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        anyhow::ensure!(
            contains(expected).eval(&stderr),
            "expected stderr to contain {expected:?}, got:\n{stderr}"
        );
        Ok(())
    }

    pub fn failure_containing_any(self, expected: &[&str]) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.output.status.success(),
            "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        anyhow::ensure!(
            expected.iter().any(|value| contains(*value).eval(&stderr)),
            "expected stderr to contain one of {expected:?}, got:\n{stderr}"
        );
        Ok(())
    }

    pub fn json_failure_containing(self, expected: &str) -> anyhow::Result<()> {
        let json = self.json_failure()?;
        let message = json["error"]["message"].as_str().unwrap_or_default();
        anyhow::ensure!(
            contains(expected).eval(message),
            "expected JSON error message to contain {expected:?}, got:\n{json}"
        );
        Ok(())
    }

    pub fn json_failure_code_containing(
        self,
        expected_code: i32,
        expected: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.output.status.code() == Some(expected_code),
            "expected exit code {expected_code}, got {:?}",
            self.output.status.code()
        );
        self.json_failure_containing(expected)
    }

    pub fn json_failure_contract_containing(
        self,
        expected_exit_code: i32,
        expected_error_code: &str,
        expected: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.output.status.code() == Some(expected_exit_code),
            "expected exit code {expected_exit_code}, got {:?}",
            self.output.status.code()
        );
        let json = self.json_failure()?;
        anyhow::ensure!(
            json["error"]["exit_code"] == expected_exit_code,
            "expected JSON exit_code {expected_exit_code}, got:\n{json}"
        );
        anyhow::ensure!(
            json["error"]["code"] == expected_error_code,
            "expected JSON error code {expected_error_code:?}, got:\n{json}"
        );
        let message = json["error"]["message"].as_str().unwrap_or_default();
        anyhow::ensure!(
            contains(expected).eval(message),
            "expected JSON error message to contain {expected:?}, got:\n{json}"
        );
        Ok(())
    }

    pub fn json_failure_containing_any(self, expected: &[&str]) -> anyhow::Result<()> {
        let json = self.json_failure()?;
        let message = json["error"]["message"].as_str().unwrap_or_default();
        anyhow::ensure!(
            expected.iter().any(|value| contains(*value).eval(message)),
            "expected JSON error message to contain one of {expected:?}, got:\n{json}"
        );
        Ok(())
    }

    fn json_failure(self) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(
            !self.output.status.success(),
            "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        anyhow::ensure!(
            stderr.is_empty(),
            "runtime --json errors should not write stderr, got:\n{stderr}"
        );
        serde_json::from_slice(&self.output.stdout).context("failed to parse JSON error stdout")
    }
}

fn ensure_success(output: &Output) -> anyhow::Result<()> {
    anyhow::ensure!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

pub struct TempDb {
    pub dir: std::path::PathBuf,
    pub path: std::path::PathBuf,
}

impl TempDb {
    pub fn new(name: &str) -> anyhow::Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix(&format!("kb-cli-{name}-"))
            .tempdir()
            .context("failed to create temporary test directory")?
            .keep();
        let path = dir.join("kb.db");
        Ok(Self { dir, path })
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
