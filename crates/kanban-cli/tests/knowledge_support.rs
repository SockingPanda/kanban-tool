//! CLI adoption 测试共用的真实 `kanban`/localhost host 驱动。

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::de::DeserializeOwned;

/// 一个隔离的 canonical host 进程和其临时数据库。
pub struct Host {
    child: Child,
    temp: tempfile::TempDir,
    server_url: String,
}

#[allow(dead_code)]
impl Host {
    pub fn new() -> Self {
        Self::start(None)
    }

    pub fn with_dispatcher(profile: &str) -> Self {
        Self::start(Some(profile))
    }

    fn start(dispatcher_profile: Option<&str>) -> Self {
        let temp = tempfile::tempdir().expect("create host tempdir");
        let db = temp.path().join("canonical.db");
        let port = free_port();
        let server_url = format!("http://127.0.0.1:{port}");
        let profile_path = dispatcher_profile.map(|profile| {
            let path = temp.path().join("dispatcher.toml");
            std::fs::write(&path, profile).expect("write dispatcher profile");
            path
        });
        let mut args = vec![
            "--db".to_owned(),
            db.to_str()
                .expect("temporary database path is UTF-8")
                .to_owned(),
            "--actor".to_owned(),
            "cli-adoption".to_owned(),
            "serve".to_owned(),
            "--host".to_owned(),
            "127.0.0.1".to_owned(),
            "--port".to_owned(),
            port.to_string(),
        ];
        if let Some(path) = profile_path.as_ref() {
            args.extend([
                "--dispatcher-profile".to_owned(),
                path.to_str()
                    .expect("dispatcher profile path is UTF-8")
                    .to_owned(),
            ]);
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_kanban"))
            .args(&args)
            .current_dir(temp.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start kanban serve host");
        wait_for_health(&mut child, port);
        Self {
            child,
            temp,
            server_url,
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kanban"));
        command
            .current_dir(self.temp.path())
            .args([
                "--json",
                "--server-url",
                &self.server_url,
                "--board",
                "default",
                "--actor",
                "cli-adoption",
            ])
            .env_remove("KANBAN_DB")
            .env_remove("KB_DB")
            .env_remove("KB_BOARD");
        command
    }

    pub fn run(&self, args: &[&str]) -> Output {
        let output = self
            .command()
            .args(args)
            .output()
            .expect("run kanban command");
        assert!(
            output.status.success(),
            "kanban {:?} failed (status={}): stdout={} stderr={}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    pub fn json<T: DeserializeOwned>(&self, args: &[&str]) -> T {
        let output = self.run(args);
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "kanban {:?} did not produce typed JSON: {error}; stdout={}",
                args,
                String::from_utf8_lossy(&output.stdout)
            )
        })
    }

    pub fn run_with_stdin(&self, args: &[&str], input: &str) -> Output {
        let mut child = self
            .command()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run kanban command with stdin");
        child
            .stdin
            .take()
            .expect("stdin pipe")
            .write_all(input.as_bytes())
            .expect("write command stdin");
        let output = child.wait_with_output().expect("collect kanban output");
        assert!(
            output.status.success(),
            "kanban {:?} failed (status={}): stdout={} stderr={}",
            args,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    pub fn temp_path(&self, name: &str) -> std::path::PathBuf {
        self.temp.path().join(name)
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("allocate loopback port")
        .local_addr()
        .expect("read loopback port")
        .port()
}

fn wait_for_health(child: &mut Child, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(15);
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("poll serve process") {
            panic!("kanban serve exited before becoming ready: {status}");
        }
        if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(200)) {
            stream
                .set_read_timeout(Some(Duration::from_millis(500)))
                .expect("set health read timeout");
            stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .expect("write health request");
            let mut response = Vec::new();
            let _ = stream.read_to_end(&mut response);
            if response.starts_with(b"HTTP/1.1 200") {
                return;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("kanban serve did not become ready on {address}");
}
