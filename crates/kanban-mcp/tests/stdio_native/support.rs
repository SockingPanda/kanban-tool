use std::{path::Path, process::Stdio, time::Duration};

use kanban_server::{AppState, ShutdownSignal, serve_with_dispatcher_shutdown};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    net::TcpStream,
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::watch,
    task::JoinHandle,
    time::{sleep, timeout},
};

pub const ACTOR: &str = "mcp-原生验收";
pub const WAIT: Duration = Duration::from_secs(15);

pub struct Host {
    _directory: tempfile::TempDir,
    pub url: String,
    stop: watch::Sender<ShutdownSignal>,
    server: JoinHandle<std::io::Result<()>>,
}

impl Host {
    pub async fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::open(directory.path().join("canonical.db"), "host-default")
            .await
            .unwrap();
        let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let (stop, receiver) = watch::channel(ShutdownSignal::Running);
        let server = tokio::spawn(serve_with_dispatcher_shutdown(
            address, state, None, receiver,
        ));
        timeout(WAIT, async {
            while TcpStream::connect(address).await.is_err() {
                assert!(!server.is_finished(), "候选 host 提前退出");
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("候选 host 监听超时");
        Self {
            _directory: directory,
            url: format!("http://{address}"),
            stop,
            server,
        }
    }

    pub async fn finish(mut self) {
        self.stop.send_replace(ShutdownSignal::Graceful);
        timeout(WAIT, &mut self.server)
            .await
            .expect("候选 host 退出超时")
            .unwrap()
            .unwrap();
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.server.abort();
    }
}

pub struct Mcp {
    process: Child,
    input: Option<ChildStdin>,
    output: Lines<BufReader<ChildStdout>>,
    next_id: u64,
    pub initialized: Value,
}

impl Mcp {
    pub async fn start(url: &str, board: &str) -> Self {
        Self::start_binary(Path::new(env!("CARGO_BIN_EXE_kanban-mcp")), url, board).await
    }

    async fn start_binary(binary: &Path, url: &str, board: &str) -> Self {
        let mut process = Command::new(binary)
            .env("KANBAN_SERVER_URL", url)
            .env("KANBAN_ACTOR", ACTOR)
            .env("KB_BOARD", board)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .expect("启动候选 MCP");
        let input = process.stdin.take().unwrap();
        let output = BufReader::new(process.stdout.take().unwrap()).lines();
        let mut session = Self {
            process,
            input: Some(input),
            output,
            next_id: 1,
            initialized: Value::Null,
        };
        let response = session
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "stdio-native-test", "version": "1"}
                }),
            )
            .await;
        assert!(response.get("error").is_none(), "{response}");
        session.initialized = response["result"].clone();
        assert_eq!(session.initialized["protocolVersion"], "2025-06-18");
        assert!(session.initialized["capabilities"]["tools"].is_object());
        session.notify("notifications/initialized", json!({})).await;
        session
    }

    async fn write(&mut self, value: Value) {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        self.input
            .as_mut()
            .expect("MCP stdin 已关闭")
            .write_all(&bytes)
            .await
            .unwrap();
    }

    pub async fn notify(&mut self, method: &str, params: Value) {
        self.write(json!({"jsonrpc": "2.0", "method": method, "params": params}))
            .await;
    }

    pub async fn send_request(&mut self, method: &str, params: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.write(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .await;
        id
    }

    pub async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.send_request(method, params).await;
        timeout(WAIT, async {
            loop {
                let line = self
                    .output
                    .next_line()
                    .await
                    .unwrap()
                    .expect("MCP stdout EOF");
                let response: Value =
                    serde_json::from_str(&line).expect("MCP stdout 只能输出 JSON-RPC");
                assert_eq!(response["jsonrpc"], "2.0");
                if response.get("id").is_some() {
                    assert_eq!(response["id"], id, "收到意外请求的响应：{response}");
                    return response;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("MCP {method} 响应超时"))
    }

    pub async fn call(&mut self, name: &str, arguments: Value) -> Value {
        let response = self
            .request("tools/call", json!({"name": name, "arguments": arguments}))
            .await;
        assert!(response.get("error").is_none(), "{name} 失败：{response}");
        let result = &response["result"];
        assert_ne!(result["isError"], true, "{name} 失败：{result}");
        let structured = result
            .get("structuredContent")
            .expect("缺少 typed tool output");
        let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap())
            .expect("JSON tool text output");
        assert_eq!(&text, structured, "文本与 structuredContent 不一致");
        structured.clone()
    }

    pub async fn call_error(&mut self, name: &str, arguments: Value) -> Value {
        let response = self
            .request("tools/call", json!({"name": name, "arguments": arguments}))
            .await;
        response
            .get("error")
            .unwrap_or_else(|| panic!("{name} 应失败：{response}"))
            .clone()
    }

    pub async fn finish(mut self) {
        self.input.take();
        let status = timeout(WAIT, self.process.wait())
            .await
            .expect("MCP 退出超时")
            .unwrap();
        assert!(status.success(), "MCP 退出失败：{status}");
    }
}
