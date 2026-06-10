#![allow(dead_code)]

use std::{path::Path, process::Output};

use anyhow::Context;
use assert_cmd::Command;
use predicates::{Predicate, str::contains};

pub fn kb(db_path: &Path, args: &[&str]) -> anyhow::Result<CmdResult> {
    kb_in_dir(db_path, args, Path::new(env!("CARGO_MANIFEST_DIR")))
}

pub fn kb_in_dir(db_path: &Path, args: &[&str], current_dir: &Path) -> anyhow::Result<CmdResult> {
    kb_in_dir_env(db_path, args, current_dir, None)
}

pub fn kb_in_dir_env(
    db_path: &Path,
    args: &[&str],
    current_dir: &Path,
    board_env: Option<&str>,
) -> anyhow::Result<CmdResult> {
    let mut command = Command::cargo_bin("kb").context("failed to locate kb test binary")?;
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
    let output = command.output().context("failed to execute kb command")?;
    Ok(CmdResult { output })
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
