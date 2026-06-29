mod common;

use common::{TempDb, kanban};
#[test]
fn serve_help_shows_localhost_bind_defaults() -> anyhow::Result<()> {
    let temp = TempDb::new("serve_help_includes_default_localhost_bind_options")?;
    let output = kanban(&temp.path, &["serve", "--help"])?.output;
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--host"), "{stdout}");
    assert!(stdout.contains("127.0.0.1"), "{stdout}");
    assert!(stdout.contains("--port"), "{stdout}");
    assert!(stdout.contains("8721"), "{stdout}");
    assert!(stdout.contains("--search-sync-interval-ms"), "{stdout}");
    assert!(stdout.contains("5000"), "{stdout}");
    Ok(())
}

#[test]
fn serve_rejects_non_loopback_host_before_opening_database() -> anyhow::Result<()> {
    let temp = TempDb::new("serve_rejects_non_loopback_host_without_creating_database")?;

    let result = kanban(&temp.path, &["serve", "--host", "0.0.0.0", "--port", "0"])?;
    assert!(
        result.output.stdout.is_empty(),
        "serve failure must not write logging to stdout, got: {}",
        String::from_utf8_lossy(&result.output.stdout)
    );
    result.failure_containing("kanban serve only supports loopback hosts")?;
    assert!(!temp.path.exists());
    Ok(())
}
