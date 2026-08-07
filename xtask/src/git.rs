use std::{path::Path, process::Command};

use xtask::ToolResult;

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub(crate) struct Sources {
    pub(crate) base: Vec<String>,
    pub(crate) staged: Vec<String>,
    pub(crate) working_tree: Vec<String>,
    pub(crate) untracked: Vec<String>,
}

impl Sources {
    pub(crate) fn merged(&self) -> Vec<String> {
        let mut merged = Vec::new();
        for path in self
            .base
            .iter()
            .chain(&self.staged)
            .chain(&self.working_tree)
            .chain(&self.untracked)
        {
            if !merged.iter().any(|seen| seen == path) {
                merged.push(path.clone());
            }
        }
        merged
    }
}

pub(crate) fn changed_sources(root: &Path, base: &str) -> ToolResult<Sources> {
    let base = base.strip_prefix("base=").unwrap_or(base);
    if base.is_empty() {
        return Err(std::io::Error::other("base 不能为空").into());
    }
    Ok(Sources {
        base: git_names(root, &["diff", "--name-only", &format!("{base}...HEAD")])?,
        staged: git_names(root, &["diff", "--name-only", "--cached"])?,
        working_tree: git_names(root, &["diff", "--name-only"])?,
        untracked: git_names(root, &["ls-files", "--others", "--exclude-standard"])?,
    })
}

fn git_names(root: &Path, arguments: &[&str]) -> ToolResult<Vec<String>> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| std::io::Error::other(format!("执行 git 失败: {error}")))?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr);
        let details = details.trim();
        let message = if details.is_empty() {
            format!("git {} 失败", arguments.join(" "))
        } else {
            format!("git {} 失败: {details}", arguments.join(" "))
        };
        return Err(std::io::Error::other(message).into());
    }

    let mut paths = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();
    Ok(paths)
}
