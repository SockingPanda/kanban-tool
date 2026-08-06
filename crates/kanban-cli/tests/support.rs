use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
    path::Path,
    process::{Command, Output},
    time::Duration,
};

use kanban_server::{
    AppState, DispatcherConfig, ShutdownSignal, serve_with_dispatcher_shutdown, serve_with_shutdown,
};
use serde_json::Value;
use tempfile::TempDir;
use tokio::{
    sync::{oneshot, watch},
    task::JoinHandle,
    time::sleep,
};

#[allow(dead_code)]
enum Shutdown {
    Http(oneshot::Sender<()>),
    Dispatcher(watch::Sender<ShutdownSignal>),
}

/// 启动真实 localhost host，并把测试命令固定到同一个临时项目和数据库。
pub struct TestHost {
    temp: TempDir,
    server_url: String,
    shutdown: Option<Shutdown>,
    server: Option<JoinHandle<std::io::Result<()>>>,
}

#[allow(dead_code)]
impl TestHost {
    pub async fn start() -> Self {
        let temp = tempfile::tempdir().expect("创建 CLI host 临时目录");
        let db_path = temp.path().join("kanban.db");
        let state = AppState::open(&db_path, "fixture")
            .await
            .expect("打开测试 canonical 数据库");
        let port = free_loopback_port();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let server = tokio::spawn(serve_with_shutdown(addr, state, async move {
            let _ = shutdown_rx.await;
        }));
        wait_for_server(addr).await;
        Self {
            temp,
            server_url: format!("http://{addr}"),
            shutdown: Some(Shutdown::Http(shutdown_tx)),
            server: Some(server),
        }
    }

    /// 启动带有真实 dispatcher worker 的 host，供 runs/run log adoption 使用。
    pub async fn start_with_dispatcher() -> Self {
        let temp = tempfile::tempdir().expect("创建 dispatcher host 临时目录");
        let db_path = temp.path().join("kanban.db");
        let profile = temp.path().join("dispatcher.toml");
        std::fs::write(
            &profile,
            r#"
command = "printf 'fixture log\\n'"
poll_interval_ms = 10
claim_ttl_ms = 1000
heartbeat_interval_ms = 100
on_success = "done"
on_failure = "ready"
log_dir = "worker-logs"
"#,
        )
        .expect("写入 dispatcher profile");
        let dispatcher = DispatcherConfig::load(&profile)
            .await
            .expect("加载 dispatcher profile");
        let state = AppState::open_with_run_log_root(
            &db_path,
            "fixture",
            Some(temp.path().join("worker-logs")),
        )
        .await
        .expect("打开 dispatcher canonical 数据库");
        let port = free_loopback_port();
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
        let server = tokio::spawn(serve_with_dispatcher_shutdown(
            addr,
            state,
            Some(dispatcher),
            shutdown_rx,
        ));
        wait_for_server(addr).await;
        Self {
            temp,
            server_url: format!("http://{addr}"),
            shutdown: Some(Shutdown::Dispatcher(shutdown_tx)),
            server: Some(server),
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kanban"));
        command
            .current_dir(self.temp.path())
            .env("KANBAN_SERVER_URL", &self.server_url)
            .env("KANBAN_ACTOR", "fixture")
            .env_remove("KB_BOARD")
            .env_remove("KANBAN_LOCALE")
            .env("XDG_CONFIG_HOME", self.temp.path().join("xdg-config"))
            .env("XDG_DATA_HOME", self.temp.path().join("xdg-data"));
        command
    }

    pub fn project_path(&self) -> &Path {
        self.temp.path()
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().expect("运行 kanban CLI")
    }

    pub fn json(&self, args: &[&str]) -> Value {
        let output = self.run(args);
        assert_success(args, &output);
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "kanban JSON 输出无法解析，args={args:?}, stdout={}, stderr={}: {error}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }
}

impl Drop for TestHost {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            match shutdown {
                Shutdown::Http(shutdown) => {
                    let _ = shutdown.send(());
                }
                Shutdown::Dispatcher(shutdown) => {
                    let _ = shutdown.send(ShutdownSignal::Force);
                }
            }
        }
        if let Some(server) = self.server.take() {
            server.abort();
        }
    }
}

async fn wait_for_server(addr: SocketAddr) {
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("测试 host 未能在预期时间内监听 {addr}");
}

fn free_loopback_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("分配 localhost 测试端口")
        .local_addr()
        .expect("读取 localhost 测试端口")
        .port()
}

pub fn assert_success(args: &[&str], output: &Output) {
    assert!(
        output.status.success(),
        "kanban 命令失败，args={args:?}, code={:?}, stdout={}, stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 读取 schemas/fixtures/cli 下的已提交 JSON fixture。
pub fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/fixtures/cli")
        .join(format!("{name}-output.v1.valid.json"));
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取 fixture {} 失败：{error}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("解析 fixture {} 失败：{error}", path.display()))
}

/// 归一化数据库生成的字段，再比较 fixture 的 JSON 结构。
///
/// 归一化集合只覆盖不可由 CLI 输入稳定控制的标识、时间、位置、版本和文件路径；
/// title、status、reason、metadata 等业务值仍由测试断言保留。
pub fn normalize_dynamic_fields(value: &mut Value) {
    const DYNAMIC_KEYS: &[&str] = &[
        "id",
        "task_id",
        "board_id",
        "run_id",
        "step_id",
        "event_id",
        "claim_token",
        "created_at",
        "updated_at",
        "started_at",
        "completed_at",
        "archived_at",
        "scheduled_at",
        "due_at",
        "claim_expires_at",
        "last_heartbeat_at",
        "resolved_at",
        "position",
        "lock_version",
        "seq",
        "retry_count",
        "generated_at",
        "tail_bytes",
        "db_path",
        "config_path",
        "rel_path",
        "sha256",
    ];
    match value {
        Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if DYNAMIC_KEYS.contains(&key.as_str()) {
                    *child = Value::String("<dynamic>".to_owned());
                } else {
                    normalize_dynamic_fields(child);
                }
            }
        }
        Value::Array(array) => {
            for child in array {
                normalize_dynamic_fields(child);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// 校验 stdout 与 fixture 使用相同的 envelope、字段和 JSON 类型。
///
/// 列表数量由实际 host 状态决定，因此 fixture 数组作为元素形状模板；测试另行断言
/// 关键业务值和是否至少产生了一条记录。
pub fn assert_fixture_shape(actual: &Value, fixture_name: &str) {
    let mut actual = actual.clone();
    let mut expected = fixture(fixture_name);
    normalize_dynamic_fields(&mut actual);
    normalize_dynamic_fields(&mut expected);
    assert_json_shape(&actual, &expected, "$", fixture_name);
}

fn assert_json_shape(actual: &Value, expected: &Value, path: &str, fixture_name: &str) {
    if (path.ends_with(".payload") || path.ends_with(".metadata"))
        && actual.is_object()
        && expected.is_object()
    {
        return;
    }
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            let actual_keys = actual.keys().collect::<BTreeSet<_>>();
            let expected_keys = expected.keys().collect::<BTreeSet<_>>();
            let missing_keys = expected_keys
                .difference(&actual_keys)
                .filter(|key| !optional_fixture_field(path, key))
                .collect::<Vec<_>>();
            let extra_keys = actual_keys
                .difference(&expected_keys)
                .filter(|key| !optional_fixture_field(path, key))
                .collect::<Vec<_>>();
            assert!(
                missing_keys.is_empty(),
                "fixture {fixture_name} 在 {path} 缺少字段：期望={expected_keys:?}, 实际={actual_keys:?}"
            );
            assert!(
                extra_keys.is_empty(),
                "fixture {fixture_name} 在 {path} 出现未声明字段：期望={expected_keys:?}, 实际={actual_keys:?}"
            );
            for (key, expected_child) in expected {
                if let Some(actual_child) = actual.get(key) {
                    assert_json_shape(
                        actual_child,
                        expected_child,
                        &format!("{path}.{key}"),
                        fixture_name,
                    );
                }
            }
        }
        (Value::Array(actual), Value::Array(expected)) => {
            if let Some(expected_first) = expected.first() {
                for (index, actual_child) in actual.iter().enumerate() {
                    let expected_child = expected.get(index).unwrap_or(expected_first);
                    assert_json_shape(
                        actual_child,
                        expected_child,
                        &format!("{path}[{index}]"),
                        fixture_name,
                    );
                }
            }
        }
        (Value::Null, _)
        | (_, Value::Null)
        | (Value::Bool(_), Value::Bool(_))
        | (Value::Number(_), Value::Number(_))
        | (Value::String(_), Value::String(_)) => {}
        _ => panic!(
            "fixture {fixture_name} 在 {path} 的 JSON 类型不一致：actual={actual:?}, expected={expected:?}"
        ),
    }
}

fn optional_fixture_field(path: &str, key: &str) -> bool {
    matches!(
        (path, key),
        ("$", "meta") | ("$.data", "created") | ("$.data", "config_path")
    )
}

/// 让测试名称直接对应 protocol catalog 中的 CLI contract ID。
pub fn assert_contract(operation: &str, fixture_name: &str) {
    let expected = format!("cli.{fixture_name}.output");
    let descriptor = kanban_protocol::cli_operation_catalog()
        .into_iter()
        .find(|descriptor| descriptor.key == operation)
        .unwrap_or_else(|| panic!("protocol catalog 缺少 CLI operation {operation}"));
    match descriptor.machine_output {
        kanban_protocol::CliMachineOutput::Contract { id } => assert_eq!(id, expected),
        other => panic!("CLI operation {operation} 未采用 JSON contract：{other:?}"),
    }
}
