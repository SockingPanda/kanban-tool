mod common;

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command as ProcessCommand, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::Context;
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
    assert!(stdout.contains("--quiet"), "{stdout}");
    assert!(stdout.contains("--log-level"), "{stdout}");
    assert!(stdout.contains("stderr"), "{stdout}");
    assert!(stdout.contains("RUST_LOG"), "{stdout}");
    assert!(stdout.contains("Ctrl-C"), "{stdout}");
    assert!(stdout.contains("graceful shutdown"), "{stdout}");
    assert!(stdout.contains("stdout"), "{stdout}");
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

#[test]
fn serve_default_tracing_logs_request_to_stderr_without_stdout() -> anyhow::Result<()> {
    let temp = TempDb::new("serve_default_tracing_logs_request_to_stderr_without_stdout")?;
    let port = reserve_loopback_port()?;
    let port_arg = port.to_string();
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_kanban"));
    command
        .current_dir(&temp.dir)
        .arg("--db")
        .arg(&temp.path)
        .args(["serve", "--host", "127.0.0.1", "--port", &port_arg])
        .env_remove("KB_BOARD")
        .env_remove("RUST_LOG")
        .env("XDG_CONFIG_HOME", temp.dir.join(".xdg-config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().context("spawn kanban serve")?;
    let health_result = wait_for_health(port);
    let _ = child.kill();
    let output = child.wait_with_output().context("wait for kanban serve")?;
    health_result.with_context(|| {
        format!(
            "serve did not answer health before timeout\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    })?;

    assert!(
        output.stdout.is_empty(),
        "serve tracing must not write stdout, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Serving Kanban API"), "{stderr}");
    assert!(
        stderr.contains("started processing request"),
        "expected request trace in stderr, got:\n{stderr}"
    );
    assert!(
        stderr.contains("finished processing request"),
        "expected response trace in stderr, got:\n{stderr}"
    );
    assert!(stderr.contains("status=200"), "{stderr}");
    Ok(())
}

#[test]
fn serve_quiet_suppresses_startup_and_request_logs_without_stdout() -> anyhow::Result<()> {
    let temp = TempDb::new("serve_quiet_suppresses_startup_and_request_logs_without_stdout")?;
    let port = reserve_loopback_port()?;
    let port_arg = port.to_string();
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_kanban"));
    command
        .current_dir(&temp.dir)
        .arg("--db")
        .arg(&temp.path)
        .args([
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            &port_arg,
            "--quiet",
        ])
        .env_remove("KB_BOARD")
        .env_remove("RUST_LOG")
        .env("XDG_CONFIG_HOME", temp.dir.join(".xdg-config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().context("spawn kanban serve")?;
    let health_result = wait_for_health(port);
    let _ = child.kill();
    let output = child.wait_with_output().context("wait for kanban serve")?;
    health_result.with_context(|| {
        format!(
            "serve did not answer health before timeout\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    })?;

    assert!(
        output.stdout.is_empty(),
        "serve quiet must not write stdout, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("Serving Kanban API"), "{stderr}");
    assert!(!stderr.contains("started processing request"), "{stderr}");
    assert!(!stderr.contains("finished processing request"), "{stderr}");
    Ok(())
}

#[test]
fn serve_log_level_warn_suppresses_info_logs_without_stdout() -> anyhow::Result<()> {
    let temp = TempDb::new("serve_log_level_warn_suppresses_info_logs_without_stdout")?;
    let port = reserve_loopback_port()?;
    let port_arg = port.to_string();
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_kanban"));
    command
        .current_dir(&temp.dir)
        .arg("--db")
        .arg(&temp.path)
        .args([
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            &port_arg,
            "--log-level",
            "warn",
        ])
        .env_remove("KB_BOARD")
        .env_remove("RUST_LOG")
        .env("XDG_CONFIG_HOME", temp.dir.join(".xdg-config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().context("spawn kanban serve")?;
    let health_result = wait_for_health(port);
    let _ = child.kill();
    let output = child.wait_with_output().context("wait for kanban serve")?;
    health_result.with_context(|| {
        format!(
            "serve did not answer health before timeout\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    })?;

    assert!(
        output.stdout.is_empty(),
        "serve log-level must not write stdout, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("Serving Kanban API"), "{stderr}");
    assert!(!stderr.contains("started processing request"), "{stderr}");
    assert!(!stderr.contains("finished processing request"), "{stderr}");
    Ok(())
}

#[cfg(unix)]
#[test]
fn serve_sigint_gracefully_exits_without_stdout_and_releases_runtime_lock() -> anyhow::Result<()> {
    let temp = TempDb::new("serve_sigint_gracefully_exits_without_stdout_and_releases_lock")?;
    let port = reserve_loopback_port()?;
    let port_arg = port.to_string();
    let mut command = ProcessCommand::new(env!("CARGO_BIN_EXE_kanban"));
    command
        .current_dir(&temp.dir)
        .arg("--db")
        .arg(&temp.path)
        .args(["serve", "--host", "127.0.0.1", "--port", &port_arg])
        .env_remove("KB_BOARD")
        .env_remove("RUST_LOG")
        .env("XDG_CONFIG_HOME", temp.dir.join(".xdg-config"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = command.spawn().context("spawn kanban serve")?;
    let health_result = wait_for_health(port);
    send_sigint(child.id())?;
    let output = child.wait_with_output().context("wait for kanban serve")?;
    health_result.with_context(|| {
        format!(
            "serve did not answer health before timeout\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    })?;

    assert!(
        output.status.success(),
        "serve SIGINT should exit 0\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "serve SIGINT must not write stdout, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let _runtime_guard = kanban_sqlite::api::lifecycle::begin_database_runtime(&temp.path)
        .context("serve SIGINT should release runtime lock")?;
    Ok(())
}

fn reserve_loopback_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).context("reserve loopback port")?;
    Ok(listener.local_addr()?.port())
}

fn wait_for_health(port: u16) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last_error = None;
    while Instant::now() < deadline {
        match request_health(port) {
            Ok(response) if response.contains("200 OK") => return Ok(()),
            Ok(response) => last_error = Some(anyhow::anyhow!("unexpected response: {response}")),
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("health check timed out")))
}

fn request_health(port: u16) -> anyhow::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).context("connect health")?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

#[cfg(unix)]
fn send_sigint(pid: u32) -> anyhow::Result<()> {
    let status = ProcessCommand::new("/bin/kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .context("send SIGINT")?;
    anyhow::ensure!(status.success(), "kill -INT failed with {status}");
    Ok(())
}
