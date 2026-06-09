mod common;

use common::{TempDb, kb};
#[test]
fn serve_help_includes_default_localhost_bind_options() -> anyhow::Result<()> {
    let temp = TempDb::new("serve_help_includes_default_localhost_bind_options")?;
    let output = kb(&temp.path, &["serve", "--help"])?.output;
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
fn serve_rejects_non_loopback_host_without_creating_database() -> anyhow::Result<()> {
    let temp = TempDb::new("serve_rejects_non_loopback_host_without_creating_database")?;

    kb(&temp.path, &["serve", "--host", "0.0.0.0", "--port", "0"])?
        .failure_containing("kb serve only supports loopback hosts")?;
    assert!(!temp.path.exists());
    Ok(())
}
