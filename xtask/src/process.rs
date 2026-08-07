use std::{
    path::Path,
    process::{Command, ExitStatus},
};

use xtask::ToolResult;

pub(crate) fn run_checked<I, S>(root: &Path, program: &str, arguments: I) -> ToolResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let status = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .status()?;
    if status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "命令失败（{}）: {program}",
        status_description(status)
    ))
    .into())
}

pub(crate) fn status_description(status: ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| "signal".to_owned(), |code| format!("exit={code}"))
}
