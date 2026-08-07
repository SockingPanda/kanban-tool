use std::path::Path;

use xtask::ToolResult;

use crate::process::run_checked;

pub(crate) fn run(root: &Path) -> ToolResult<()> {
    run_checked(root, "python3", ["-B", "scripts/test_dependency_owners.py"])?;
    run_checked(
        root,
        "python3",
        ["-B", "scripts/schema_dependency_policy.py"],
    )?;
    run_checked(
        root,
        "python3",
        ["-B", "scripts/check-dependency-owners.py"],
    )?;
    run_checked(
        root,
        "scripts/test-schema-cargo-tree.sh",
        std::iter::empty::<&str>(),
    )?;
    run_checked(
        root,
        "python3",
        ["-B", "scripts/check-single-host-dependencies.py"],
    )?;
    Ok(())
}
