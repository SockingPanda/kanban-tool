use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use kanban_protocol::{
    HealthResponse, WEB_ARTIFACT_BASE_PATH, WEB_PROTOCOL_VERSION, WebArtifactManifest,
    WebRuntimeConfig, validate_web_artifact_manifest,
};

/// Desktop 只连接这个固定的本机 host；不能因为冲突而随机改端口。
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8721;
pub const HOST_STARTUP_TIMEOUT: Duration = Duration::from_secs(8);
pub const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const HTTP_READ_TIMEOUT: Duration = Duration::from_millis(750);
const HOST_POLL_INTERVAL: Duration = Duration::from_millis(50);
const MAX_HTTP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_STDERR_DIAGNOSTIC_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy)]
struct ProbeTimeouts {
    connect: Duration,
    io: Duration,
    total: Duration,
}

impl Default for ProbeTimeouts {
    fn default() -> Self {
        Self {
            connect: HTTP_CONNECT_TIMEOUT,
            io: HTTP_READ_TIMEOUT,
            total: Duration::from_secs(2),
        }
    }
}

impl ProbeTimeouts {
    #[cfg(test)]
    fn with_single_timeout(timeout: Duration) -> Self {
        Self {
            connect: timeout,
            io: timeout,
            total: timeout,
        }
    }
}

pub fn default_endpoint() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], DEFAULT_PORT))
}

/// 从 host 返回的三份事实中构造已经兼容的连接描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCompatibility {
    runtime: WebRuntimeConfig,
    manifest: WebArtifactManifest,
}

impl HostCompatibility {
    pub fn verify(
        runtime: WebRuntimeConfig,
        manifest: WebArtifactManifest,
        expected_server_version: &str,
    ) -> Result<Self, CompatibilityError> {
        if !runtime.api_base_url.is_empty() {
            return Err(CompatibilityError::new(
                "host runtime apiBaseUrl 必须为空并使用同源 API",
            ));
        }
        if runtime.web_base_path != WEB_ARTIFACT_BASE_PATH {
            return Err(CompatibilityError::new(format!(
                "host runtime webBasePath 必须为 {WEB_ARTIFACT_BASE_PATH:?}"
            )));
        }
        if runtime.server_version != expected_server_version {
            return Err(CompatibilityError::new(format!(
                "host serverVersion 不匹配: {} != {expected_server_version}",
                runtime.server_version
            )));
        }
        if runtime.protocol_version != WEB_PROTOCOL_VERSION {
            return Err(CompatibilityError::new(format!(
                "host protocolVersion 不匹配: {} != {WEB_PROTOCOL_VERSION}",
                runtime.protocol_version
            )));
        }
        if manifest.server_version != expected_server_version {
            return Err(CompatibilityError::new(format!(
                "Web artifact serverVersion 不匹配: {} != {expected_server_version}",
                manifest.server_version
            )));
        }
        validate_web_artifact_manifest(&manifest)
            .map_err(|error| CompatibilityError::new(error.to_string()))?;
        if runtime.web_build_id != manifest.build_id {
            return Err(CompatibilityError::new(format!(
                "host webBuildId 与 manifest buildId 不匹配: {} != {}",
                runtime.web_build_id, manifest.build_id
            )));
        }
        Ok(Self { runtime, manifest })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityError(String);

impl CompatibilityError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CompatibilityError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    Unavailable(String),
    Incompatible(String),
}

impl ProbeError {
    fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable(message.into())
    }

    fn incompatible(message: impl Into<String>) -> Self {
        Self::Incompatible(message.into())
    }

    fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable(_))
    }
}

impl std::fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message) | Self::Incompatible(message) => {
                formatter.write_str(message)
            }
        }
    }
}

impl std::error::Error for ProbeError {}

/// 探测固定 loopback host 的 health、runtime 与 Web artifact manifest。
#[allow(dead_code)]
pub fn probe_host(endpoint: SocketAddr) -> Result<HostCompatibility, ProbeError> {
    probe_host_with_timeouts(endpoint, ProbeTimeouts::default())
}

fn probe_host_with_timeouts(
    endpoint: SocketAddr,
    timeouts: ProbeTimeouts,
) -> Result<HostCompatibility, ProbeError> {
    let deadline = Instant::now() + timeouts.total;
    probe_host_until(endpoint, timeouts, deadline)
}

fn probe_host_until(
    endpoint: SocketAddr,
    timeouts: ProbeTimeouts,
    deadline: Instant,
) -> Result<HostCompatibility, ProbeError> {
    if !endpoint.ip().is_loopback() {
        return Err(ProbeError::incompatible(
            "Desktop host probe 只允许 loopback 地址",
        ));
    }
    let health = get_json::<HealthResponse>(endpoint, "/health", timeouts, deadline)?;
    if !health.data.ok {
        return Err(ProbeError::incompatible("host health.ok=false"));
    }
    if health.data.version != env!("CARGO_PKG_VERSION") {
        return Err(ProbeError::incompatible(format!(
            "host health version 不匹配: {} != {}",
            health.data.version,
            env!("CARGO_PKG_VERSION")
        )));
    }

    let runtime = get_json::<WebRuntimeConfig>(endpoint, "/app/runtime.json", timeouts, deadline)?;
    let manifest =
        get_json::<WebArtifactManifest>(endpoint, "/app/manifest.json", timeouts, deadline)?;
    let compatibility = HostCompatibility::verify(runtime, manifest, env!("CARGO_PKG_VERSION"))
        .map_err(|error| ProbeError::incompatible(error.to_string()))?;
    if Instant::now() >= deadline {
        return Err(ProbeError::incompatible(
            "host probe total deadline exceeded",
        ));
    }
    Ok(compatibility)
}

fn get_json<T>(
    endpoint: SocketAddr,
    path: &str,
    timeouts: ProbeTimeouts,
    deadline: Instant,
) -> Result<T, ProbeError>
where
    T: serde::de::DeserializeOwned,
{
    let response = get(endpoint, path, timeouts, deadline)?;
    if response.status != 200 {
        return Err(ProbeError::incompatible(format!(
            "host {path} 返回 HTTP {}，需要 200",
            response.status
        )));
    }
    let media_type = response
        .content_type
        .split(';')
        .next()
        .map(str::trim)
        .unwrap_or_default();
    if !media_type.eq_ignore_ascii_case("application/json") {
        return Err(ProbeError::incompatible(format!(
            "host {path} Content-Type 必须为 application/json，实际为 {}",
            response.content_type
        )));
    }
    serde_json::from_slice(&response.body)
        .map_err(|error| ProbeError::incompatible(format!("host {path} JSON 无法解析: {error}")))
}

fn get(
    endpoint: SocketAddr,
    path: &str,
    timeouts: ProbeTimeouts,
    deadline: Instant,
) -> Result<HttpResponse, ProbeError> {
    let mut stream = TcpStream::connect_timeout(
        &endpoint,
        remaining_timeout(deadline, timeouts.connect).map_err(|error| {
            ProbeError::unavailable(format!(
                "无法连接固定 host http://{endpoint}{path}: {error}"
            ))
        })?,
    )
    .map_err(|error| {
        ProbeError::unavailable(format!(
            "无法连接固定 host http://{endpoint}{path}: {error}"
        ))
    })?;
    stream
        .set_read_timeout(Some(remaining_timeout(deadline, timeouts.io).map_err(
            |error| ProbeError::incompatible(format!("设置 host 读取超时失败: {error}")),
        )?))
        .map_err(|error| ProbeError::incompatible(format!("设置 host 读取超时失败: {error}")))?;
    stream
        .set_write_timeout(Some(remaining_timeout(deadline, timeouts.io).map_err(
            |error| ProbeError::incompatible(format!("设置 host 写入超时失败: {error}")),
        )?))
        .map_err(|error| ProbeError::incompatible(format!("设置 host 写入超时失败: {error}")))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {endpoint}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| ProbeError::incompatible(format!("发送 host probe 请求失败: {error}")))?;
    read_http_response(&mut stream, deadline, timeouts.io)
        .map_err(|error| ProbeError::incompatible(error.to_string()))
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    content_type: String,
    body: Vec<u8>,
}

fn remaining_timeout(deadline: Instant, maximum: Duration) -> io::Result<Duration> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "host probe total deadline exceeded",
        ))
    } else {
        Ok(remaining.min(maximum))
    }
}

fn read_http_response(
    stream: &mut TcpStream,
    deadline: Instant,
    io_timeout: Duration,
) -> io::Result<HttpResponse> {
    let mut bytes = Vec::new();
    let header_end = loop {
        stream.set_read_timeout(Some(remaining_timeout(deadline, io_timeout)?))?;
        let mut chunk = [0_u8; 4096];
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "host probe 在响应头完成前关闭连接",
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > MAX_HTTP_RESPONSE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "host probe 响应过大",
            ));
        }
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break index + 4;
        }
    };

    let header_text = std::str::from_utf8(&bytes[..header_end]).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("host probe 响应头不是 UTF-8: {error}"),
        )
    })?;
    let mut lines = header_text.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "host probe 状态行缺失"))?;
    if !status_line.starts_with("HTTP/1.1 ")
        || status_line
            .bytes()
            .any(|byte| !byte.is_ascii() || byte < b' ' || byte == 0x7f)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host probe 状态行必须为 HTTP/1.1 SP 3DIGIT 结构",
        ));
    }
    let status_bytes = status_line.as_bytes();
    let status_code = status_bytes
        .get(9..12)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "host probe 状态码无效"))?;
    if !status_code.iter().all(|byte| byte.is_ascii_digit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host probe 状态码无效",
        ));
    }
    if status_bytes.get(12) != Some(&b' ') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host probe 状态码后必须为空格",
        ));
    }
    let status = status_code
        .iter()
        .fold(0_u16, |status, byte| status * 10 + u16::from(byte - b'0'));
    if !(100..=599).contains(&status) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host probe 状态码超出范围",
        ));
    }
    let mut content_length = None;
    let mut content_type = None;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "host probe 响应头字段无效")
        })?;
        if name.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "host probe 响应头字段名为空",
            ));
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "host probe 拒绝 transfer-encoding 响应",
            ));
        }
        if name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "host probe 拒绝重复 content-length",
                ));
            }
            content_length = Some(value.trim().parse::<usize>().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "host probe content-length 无效")
            })?);
        } else if name.eq_ignore_ascii_case("content-type") {
            if content_type.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "host probe 拒绝重复 content-type",
                ));
            }
            content_type = Some(value.trim().to_owned());
        }
    }
    let content_length = content_length.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "host probe 响应必须提供 content-length",
        )
    })?;
    let content_type = content_type.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "host probe 响应必须提供 content-type",
        )
    })?;

    if content_length > MAX_HTTP_RESPONSE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host probe body 过大",
        ));
    }
    let mut body = bytes.split_off(header_end);
    if body.len() > content_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "host probe body 超过 content-length",
        ));
    }
    while body.len() < content_length {
        stream.set_read_timeout(Some(remaining_timeout(deadline, io_timeout)?))?;
        let remaining = content_length - body.len();
        let mut chunk = vec![0_u8; remaining.min(4096)];
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "host probe body 不完整",
            ));
        }
        body.extend_from_slice(&chunk[..count]);
    }
    body.truncate(content_length);
    Ok(HttpResponse {
        status,
        content_type,
        body,
    })
}

#[derive(Debug, Clone)]
pub struct HostLaunchConfig {
    pub endpoint: SocketAddr,
    pub sidecar_path: PathBuf,
    pub web_dir: PathBuf,
    pub db_path: PathBuf,
    pub actor: String,
    pub board: String,
    pub startup_timeout: Duration,
    probe_timeouts: ProbeTimeouts,
}

impl HostLaunchConfig {
    pub fn new(
        sidecar_path: impl Into<PathBuf>,
        web_dir: impl Into<PathBuf>,
        db_path: impl Into<PathBuf>,
        actor: impl Into<String>,
        board: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: default_endpoint(),
            sidecar_path: sidecar_path.into(),
            web_dir: web_dir.into(),
            db_path: db_path.into(),
            actor: actor.into(),
            board: board.into(),
            startup_timeout: HOST_STARTUP_TIMEOUT,
            probe_timeouts: ProbeTimeouts::default(),
        }
    }

    #[cfg(test)]
    fn with_probe_timeout(mut self, timeout: Duration) -> Self {
        self.probe_timeouts = ProbeTimeouts::with_single_timeout(timeout);
        self
    }
}

#[derive(Debug)]
pub enum HostStartupError {
    SidecarSpawn {
        path: PathBuf,
        message: String,
    },
    PortConflict {
        endpoint: SocketAddr,
        message: String,
    },
    SidecarExited {
        message: String,
    },
    SidecarIncompatible {
        message: String,
    },
    StartupTimeout {
        endpoint: SocketAddr,
        message: String,
    },
}

impl std::fmt::Display for HostStartupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SidecarSpawn { path, message } => write!(
                formatter,
                "无法启动 bundled kanban serve sidecar {}：{message}",
                path.display()
            ),
            Self::PortConflict { endpoint, message } => write!(
                formatter,
                "固定 host 端口 {endpoint} 已被不兼容进程占用，未随机改端口：{message}；请停止冲突 host 后重试"
            ),
            Self::SidecarExited { message } => {
                write!(formatter, "kanban serve sidecar 提前退出：{message}")
            }
            Self::SidecarIncompatible { message } => {
                write!(
                    formatter,
                    "bundled kanban serve sidecar 版本不兼容：{message}"
                )
            }
            Self::StartupTimeout { endpoint, message } => write!(
                formatter,
                "等待固定 host {endpoint} 就绪超时：{message}；未尝试随机端口"
            ),
        }
    }
}

impl std::error::Error for HostStartupError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOwnership {
    External,
    Owned,
}

#[derive(Debug)]
pub struct HostHandle {
    endpoint: SocketAddr,
    ownership: HostOwnership,
    child: Option<OwnedChild>,
}

#[derive(Debug)]
struct OwnedChild {
    process: Child,
    stderr: Arc<Mutex<VecDeque<u8>>>,
    drain: Option<thread::JoinHandle<()>>,
    drop_cleanup: bool,
}

impl OwnedChild {
    fn diagnostic(&self, status: std::process::ExitStatus) -> String {
        let bytes = self
            .stderr
            .lock()
            .expect("sidecar stderr 诊断锁已失效")
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let output = String::from_utf8_lossy(&bytes).trim().to_owned();
        if output.is_empty() {
            format!("exit status {status}")
        } else {
            format!("exit status {status}: {output}")
        }
    }

    fn finish_drain(&mut self) {
        if let Some(drain) = self.drain.take() {
            let _ = drain.join();
        }
    }

    /// Drop 路径只做不会等待的 best-effort 重试；无法确认退出时放弃 join，避免悬死。
    fn best_effort_drop_retry(&mut self) {
        if matches!(self.process.try_wait(), Ok(Some(_))) {
            self.finish_drain();
            return;
        }
        let _ = self.process.kill();
        if matches!(self.process.try_wait(), Ok(Some(_))) {
            self.finish_drain();
        }
    }

    fn disarm_drop_cleanup(&mut self) {
        self.drop_cleanup = false;
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.drop_cleanup {
            self.best_effort_drop_retry();
        }
    }
}

impl HostHandle {
    pub fn app_url(&self) -> String {
        format!("http://{}/app/", self.endpoint)
    }

    pub fn shutdown(&mut self) -> Result<ShutdownResult, ShutdownError> {
        if self.ownership == HostOwnership::External {
            if let Some(child) = self.child.as_mut() {
                child.disarm_drop_cleanup();
            }
            return Ok(ShutdownResult::ExternalHostKept);
        }
        let Some(child) = self.child.as_mut() else {
            return Ok(ShutdownResult::AlreadyExited);
        };
        match child.process.try_wait() {
            Ok(Some(_)) => {
                child.finish_drain();
                self.child.take();
                return Ok(ShutdownResult::AlreadyExited);
            }
            Ok(None) => {}
            Err(error) => {
                let first_error = ShutdownError::Io(error.to_string());
                return self.cleanup_after_error(first_error);
            }
        }

        if let Err(error) = request_graceful_stop(&child.process) {
            let first_error = ShutdownError::GracefulRequest(error.to_string());
            return self.cleanup_after_error(first_error);
        }
        let deadline = Instant::now() + GRACEFUL_SHUTDOWN_TIMEOUT;
        loop {
            match child.process.try_wait() {
                Ok(Some(_)) => {
                    child.finish_drain();
                    self.child.take();
                    return Ok(ShutdownResult::Graceful);
                }
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(HOST_POLL_INTERVAL);
                }
                Ok(None) => {
                    let cleanup = force_stop(child);
                    let ownership = cleanup.ownership();
                    let cleanup_error = cleanup
                        .first_error
                        .map(|error| ShutdownError::Io(error.to_string()));
                    if ownership == CleanupOwnership::Release {
                        self.child.take();
                    }
                    return cleanup_error.map_or(Ok(ShutdownResult::Forced), Err);
                }
                Err(error) => {
                    let first_error = ShutdownError::Io(error.to_string());
                    return self.cleanup_after_error(first_error);
                }
            }
        }
    }

    fn cleanup_after_error(
        &mut self,
        first_error: ShutdownError,
    ) -> Result<ShutdownResult, ShutdownError> {
        let cleanup = self
            .child
            .as_mut()
            .expect("owned host child disappeared during shutdown");
        let cleanup = force_stop(cleanup);
        if cleanup.ownership() == CleanupOwnership::Release {
            self.child.take();
        }
        if let Some(cleanup_error) = cleanup.first_error {
            eprintln!("kanban owned host cleanup 失败：{cleanup_error}");
        }
        Err(first_error)
    }
}

impl Drop for HostHandle {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownResult {
    ExternalHostKept,
    AlreadyExited,
    Graceful,
    Forced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownError {
    Io(String),
    GracefulRequest(String),
}

impl std::fmt::Display for ShutdownError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) | Self::GracefulRequest(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ShutdownError {}

pub fn connect_or_spawn(config: &HostLaunchConfig) -> Result<HostHandle, HostStartupError> {
    match probe_host_with_timeouts(config.endpoint, config.probe_timeouts) {
        Ok(_) => Ok(HostHandle {
            endpoint: config.endpoint,
            ownership: HostOwnership::External,
            child: None,
        }),
        Err(ProbeError::Incompatible(message)) => Err(HostStartupError::PortConflict {
            endpoint: config.endpoint,
            message,
        }),
        Err(initial_probe @ ProbeError::Unavailable(_)) => {
            let mut child = spawn_sidecar(config)?;
            let result = wait_for_sidecar(config, &mut child, initial_probe);
            match result {
                Ok(_) => Ok(HostHandle {
                    endpoint: config.endpoint,
                    ownership: HostOwnership::Owned,
                    child: Some(child),
                }),
                Err(error) => {
                    let cleanup = force_stop(&mut child);
                    if let Some(cleanup_error) = cleanup.first_error {
                        eprintln!("kanban sidecar cleanup 失败：{cleanup_error}");
                    }
                    Err(error)
                }
            }
        }
    }
}

fn spawn_sidecar(config: &HostLaunchConfig) -> Result<OwnedChild, HostStartupError> {
    if !config.sidecar_path.is_file() {
        return Err(HostStartupError::SidecarSpawn {
            path: config.sidecar_path.clone(),
            message: "sidecar 文件不存在".to_owned(),
        });
    }
    let mut command = Command::new(&config.sidecar_path);
    command
        .arg("--db")
        .arg(&config.db_path)
        .arg("--board")
        .arg(&config.board)
        .arg("--actor")
        .arg(&config.actor)
        .arg("serve")
        .arg("--host")
        .arg(DEFAULT_HOST)
        .arg("--port")
        .arg(config.endpoint.port().to_string())
        .arg("--web-dir")
        .arg(&config.web_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut process = command
        .spawn()
        .map_err(|error| HostStartupError::SidecarSpawn {
            path: config.sidecar_path.clone(),
            message: error.to_string(),
        })?;
    let stderr = match process.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let _ = process.kill();
            let _ = process.wait();
            return Err(HostStartupError::SidecarSpawn {
                path: config.sidecar_path.clone(),
                message: "sidecar stderr 管道创建失败".to_owned(),
            });
        }
    };
    let diagnostics = Arc::new(Mutex::new(VecDeque::with_capacity(
        MAX_STDERR_DIAGNOSTIC_BYTES,
    )));
    let diagnostics_for_thread = Arc::clone(&diagnostics);
    let drain = match thread::Builder::new()
        .name("kanban-desktop-sidecar-stderr".to_owned())
        .spawn(move || drain_stderr(stderr, diagnostics_for_thread))
    {
        Ok(drain) => drain,
        Err(error) => {
            let _ = process.kill();
            let _ = process.wait();
            return Err(HostStartupError::SidecarSpawn {
                path: config.sidecar_path.clone(),
                message: format!("sidecar stderr drain 启动失败: {error}"),
            });
        }
    };
    Ok(OwnedChild {
        process,
        stderr: diagnostics,
        drain: Some(drain),
        drop_cleanup: true,
    })
}

fn wait_for_sidecar(
    config: &HostLaunchConfig,
    child: &mut OwnedChild,
    initial_probe: ProbeError,
) -> Result<HostCompatibility, HostStartupError> {
    let deadline = Instant::now() + config.startup_timeout;
    loop {
        if Instant::now() >= deadline {
            return Err(HostStartupError::StartupTimeout {
                endpoint: config.endpoint,
                message: "startup 总 deadline 已耗尽".to_owned(),
            });
        }
        let probe_deadline =
            capped_probe_deadline(Instant::now(), deadline, config.probe_timeouts.total);
        let latest_error =
            match probe_host_until(config.endpoint, config.probe_timeouts, probe_deadline) {
                Ok(compatibility) => return Ok(compatibility),
                Err(error) => error,
            };
        if let Some(status) =
            child
                .process
                .try_wait()
                .map_err(|error| HostStartupError::SidecarExited {
                    message: error.to_string(),
                })?
        {
            child.finish_drain();
            let diagnostic = child.diagnostic(status);
            if !initial_probe.is_unavailable()
                || diagnostic
                    .to_ascii_lowercase()
                    .contains("address already in use")
                || diagnostic.contains("地址已在使用")
            {
                return Err(HostStartupError::PortConflict {
                    endpoint: config.endpoint,
                    message: diagnostic,
                });
            }
            return Err(HostStartupError::SidecarExited {
                message: diagnostic,
            });
        }
        if Instant::now() >= deadline {
            return Err(match latest_error {
                ProbeError::Incompatible(message) => {
                    HostStartupError::SidecarIncompatible { message }
                }
                ProbeError::Unavailable(message) => HostStartupError::StartupTimeout {
                    endpoint: config.endpoint,
                    message,
                },
            });
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if !remaining.is_zero() {
            thread::sleep(remaining.min(HOST_POLL_INTERVAL));
        }
    }
}

fn capped_probe_deadline(
    started_at: Instant,
    startup_deadline: Instant,
    probe_total: Duration,
) -> Instant {
    (started_at + probe_total).min(startup_deadline)
}

fn drain_stderr(mut stderr: impl Read, diagnostics: Arc<Mutex<VecDeque<u8>>>) {
    let mut chunk = [0_u8; 4096];
    loop {
        let count = match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(count) => count,
        };
        let mut buffer = diagnostics.lock().expect("sidecar stderr 诊断锁已失效");
        for byte in &chunk[..count] {
            if buffer.len() == MAX_STDERR_DIAGNOSTIC_BYTES {
                buffer.pop_front();
            }
            buffer.push_back(*byte);
        }
    }
}

trait ProcessCleanup {
    fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>>;
    fn kill(&mut self) -> io::Result<()>;
    fn wait(&mut self) -> io::Result<std::process::ExitStatus>;
}

impl ProcessCleanup for Child {
    fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        Child::try_wait(self)
    }

    fn kill(&mut self) -> io::Result<()> {
        Child::kill(self)
    }

    fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        Child::wait(self)
    }
}

#[derive(Debug)]
struct CleanupOutcome {
    first_error: Option<io::Error>,
    process_reaped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupOwnership {
    RetainForRetry,
    Release,
}

impl CleanupOutcome {
    fn ownership(&self) -> CleanupOwnership {
        if self.process_reaped {
            CleanupOwnership::Release
        } else {
            CleanupOwnership::RetainForRetry
        }
    }

    fn should_join_drain(&self) -> bool {
        self.process_reaped
    }
}

/// 以 fake process 实现覆盖 cleanup 错误组合，避免测试依赖真实 OS wait error。
fn force_stop_with<P: ProcessCleanup>(process: &mut P) -> CleanupOutcome {
    let mut first_error = None;
    let process_reaped = match process.try_wait() {
        Ok(Some(_)) => true,
        Ok(None) => stop_and_wait(process, &mut first_error),
        Err(error) => {
            first_error = Some(error);
            stop_and_wait(process, &mut first_error)
        }
    };
    CleanupOutcome {
        first_error,
        process_reaped,
    }
}

fn stop_and_wait<P: ProcessCleanup>(process: &mut P, first_error: &mut Option<io::Error>) -> bool {
    if let Err(error) = process.kill() {
        if first_error.is_none() {
            *first_error = Some(error);
        }
        // kill 失败时不调用可能无限等待的 wait；只用一次非阻塞 try_wait 争取确认。
        return matches!(process.try_wait(), Ok(Some(_)));
    }
    match process.wait() {
        Ok(_) => true,
        Err(error) => {
            if first_error.is_none() {
                *first_error = Some(error);
            }
            false
        }
    }
}

fn force_stop(child: &mut OwnedChild) -> CleanupOutcome {
    let outcome = force_stop_with(&mut child.process);
    if outcome.should_join_drain() {
        child.finish_drain();
    }
    outcome
}

#[cfg(unix)]
fn request_graceful_stop(child: &Child) -> io::Result<()> {
    let result = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn request_graceful_stop(_child: &Child) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Desktop host graceful shutdown 仅支持 Linux/Unix",
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Read, Write},
        net::TcpListener,
        sync::atomic::{AtomicUsize, Ordering},
        thread,
    };

    #[cfg(unix)]
    use std::os::unix::process::CommandExt;
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    use kanban_protocol::{
        HealthReport, HealthResponse, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
        WebArtifactFile, web_artifact_build_id_for,
    };

    use super::*;

    static NEXT_FIXTURE_ID: AtomicUsize = AtomicUsize::new(0);

    fn compatible_values() -> (WebRuntimeConfig, WebArtifactManifest) {
        let payload = WebArtifactFile {
            path: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
            bytes: 1,
            sha256: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
        };
        let build_id = web_artifact_build_id_for(
            WEB_ARTIFACT_FORMAT_VERSION,
            WEB_ARTIFACT_BASE_PATH,
            WEB_ARTIFACT_ENTRYPOINT,
            "3.0.0",
            WEB_PROTOCOL_VERSION,
            std::slice::from_ref(&payload),
        )
        .expect("build id");
        let manifest = WebArtifactManifest {
            format_version: WEB_ARTIFACT_FORMAT_VERSION,
            base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
            entrypoint: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
            server_version: "3.0.0".to_owned(),
            protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
            build_id: build_id.clone(),
            files: vec![payload],
        };
        let runtime = WebRuntimeConfig {
            api_base_url: String::new(),
            web_base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
            actor: "local".to_owned(),
            default_board: "default".to_owned(),
            server_version: "3.0.0".to_owned(),
            protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
            web_build_id: build_id,
        };
        (runtime, manifest)
    }

    #[test]
    fn compatible_host_requires_matching_protocol_and_artifact_build() {
        let (runtime, manifest) = compatible_values();
        let compatibility = HostCompatibility::verify(runtime, manifest, "3.0.0")
            .expect("same-version host should attach");
        assert_eq!(compatibility.runtime.default_board, "default");
    }

    #[test]
    fn stale_protocol_is_not_an_attachable_host() {
        let (mut runtime, manifest) = compatible_values();
        runtime.protocol_version = "v0".to_owned();
        let error = HostCompatibility::verify(runtime, manifest, "3.0.0")
            .expect_err("stale protocol must not attach");
        assert!(error.to_string().contains("protocolVersion"));
    }

    #[test]
    fn mismatched_web_build_is_not_an_attachable_host() {
        let (mut runtime, manifest) = compatible_values();
        runtime.web_build_id =
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_owned();
        let error = HostCompatibility::verify(runtime, manifest, "3.0.0")
            .expect_err("different web artifact must not attach");
        assert!(error.to_string().contains("webBuildId"));
    }

    #[test]
    fn default_probe_endpoint_never_changes_port() {
        assert_eq!(default_endpoint().ip().to_string(), DEFAULT_HOST);
        assert_eq!(default_endpoint().port(), DEFAULT_PORT);
        assert_eq!(
            HostLaunchConfig::new("sidecar", "web", "db", "actor", "board").endpoint,
            default_endpoint()
        );
    }

    #[test]
    fn probe_attaches_only_after_health_runtime_and_manifest_match() {
        let (runtime, manifest) = compatible_values();
        let health = HealthResponse::new(HealthReport {
            ok: true,
            db: "turso".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            db_path: "private".to_owned(),
            db_fingerprint: "private".to_owned(),
        });
        let endpoint = spawn_json_host([
            serde_json::to_vec(&health).expect("health JSON"),
            serde_json::to_vec(&runtime).expect("runtime JSON"),
            serde_json::to_vec(&manifest).expect("manifest JSON"),
        ]);

        let attached = probe_host(endpoint).expect("compatible host");
        assert_eq!(attached.manifest.build_id, manifest.build_id);
    }

    #[test]
    fn probe_rejects_unbounded_http_response_headers() {
        let endpoint = spawn_raw_host([
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}".to_vec(),
        ]);
        let error = probe_host(endpoint).expect_err("missing content-length must fail closed");
        assert!(
            matches!(error, ProbeError::Incompatible(message) if message.contains("content-length"))
        );

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            MAX_HTTP_RESPONSE_BYTES + 1
        );
        let endpoint = spawn_raw_host([response.into_bytes()]);
        let error = probe_host(endpoint).expect_err("oversized body must fail closed");
        assert!(
            matches!(error, ProbeError::Incompatible(message) if message.contains("body 过大"))
        );
    }

    #[test]
    fn probe_rejects_malformed_http_status_lines() {
        let endpoint = spawn_raw_host([
            b"NOTHTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"
                .to_vec(),
        ]);
        let error = probe_host(endpoint).expect_err("non-HTTP status line must fail closed");
        assert!(matches!(error, ProbeError::Incompatible(message) if message.contains("HTTP/1.1")));

        let endpoint = spawn_raw_host([
            b" HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"
                .to_vec(),
        ]);
        let error = probe_host(endpoint).expect_err("leading whitespace status must fail closed");
        assert!(matches!(error, ProbeError::Incompatible(message) if message.contains("状态行")));

        let endpoint = spawn_raw_host([
            b"HTTP/1.1 20 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"
                .to_vec(),
        ]);
        let error = probe_host(endpoint).expect_err("two-digit status must fail closed");
        assert!(matches!(error, ProbeError::Incompatible(message) if message.contains("状态码")));

        let endpoint = spawn_raw_host([
            b"HTTP/1.1 200\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"
                .to_vec(),
        ]);
        let error = probe_host(endpoint).expect_err("missing reason separator must fail closed");
        assert!(matches!(error, ProbeError::Incompatible(message) if message.contains("状态码后")));
    }

    #[test]
    fn probe_rejects_body_bytes_beyond_content_length() {
        let endpoint = spawn_raw_host([
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}x"
                .to_vec(),
        ]);
        let error = probe_host(endpoint).expect_err("trailing body bytes must fail closed");
        assert!(
            matches!(error, ProbeError::Incompatible(message) if message.contains("超过 content-length"))
        );
    }

    #[test]
    fn incompatible_occupied_port_does_not_spawn_sidecar() {
        let health = HealthResponse::new(HealthReport {
            ok: true,
            db: "turso".to_owned(),
            version: "0.0.0".to_owned(),
            db_path: "private".to_owned(),
            db_fingerprint: "private".to_owned(),
        });
        let endpoint = spawn_json_host([serde_json::to_vec(&health).expect("health JSON")]);
        assert_no_sidecar_spawn(endpoint);
    }

    #[test]
    fn malformed_connected_host_does_not_spawn_sidecar() {
        let endpoint = spawn_raw_host([
            b"garbage\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}".to_vec(),
        ]);
        assert_no_sidecar_spawn(endpoint);
    }

    #[test]
    fn reset_connected_host_does_not_spawn_sidecar() {
        assert_no_sidecar_spawn(spawn_raw_host([Vec::new()]));
    }

    #[test]
    fn stalled_connected_host_does_not_spawn_sidecar() {
        assert_no_sidecar_spawn(spawn_stalled_host());
    }

    #[test]
    fn slow_drip_connected_host_hits_total_probe_deadline_without_spawn() {
        assert_no_sidecar_spawn(spawn_slow_drip_host());
    }

    #[test]
    fn probe_total_timeout_is_configured_once_for_the_full_probe() {
        let timeouts = ProbeTimeouts::with_single_timeout(Duration::from_millis(37));
        assert_eq!(timeouts.connect, Duration::from_millis(37));
        assert_eq!(timeouts.io, Duration::from_millis(37));
        assert_eq!(timeouts.total, Duration::from_millis(37));
    }

    #[test]
    fn probe_deadline_caps_to_startup_deadline() {
        let started_at = Instant::now();
        let startup_deadline = started_at + Duration::from_millis(20);
        let deadline = capped_probe_deadline(started_at, startup_deadline, Duration::from_secs(2));
        assert_eq!(deadline, startup_deadline);
    }

    #[test]
    fn probe_deadline_uses_probe_total_before_startup_cap() {
        let started_at = Instant::now();
        let startup_deadline = started_at + Duration::from_secs(2);
        let deadline =
            capped_probe_deadline(started_at, startup_deadline, Duration::from_millis(20));
        assert_eq!(deadline, started_at + Duration::from_millis(20));
    }

    #[test]
    fn probe_deadline_never_rebases_after_elapsed_round_time() {
        let started_at = Instant::now();
        let startup_deadline = started_at + Duration::from_millis(80);
        let first = capped_probe_deadline(started_at, startup_deadline, Duration::from_secs(2));
        thread::sleep(Duration::from_millis(20));
        let second = capped_probe_deadline(
            started_at + Duration::from_millis(20),
            startup_deadline,
            Duration::from_secs(2),
        );
        assert_eq!(first, startup_deadline);
        assert_eq!(second, startup_deadline);
    }

    #[test]
    fn remaining_timeout_rejects_expired_probe_deadline() {
        let deadline = Instant::now();
        let error = remaining_timeout(deadline, Duration::from_secs(1))
            .expect_err("expired probe deadline must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn remaining_timeout_never_exceeds_probe_io_budget() {
        let deadline = Instant::now() + Duration::from_secs(1);
        let timeout = remaining_timeout(deadline, Duration::from_millis(25))
            .expect("future probe deadline should be usable");
        assert!(timeout <= Duration::from_millis(25));
    }

    #[test]
    fn probe_health_runtime_manifest_share_one_total_deadline() {
        let (runtime, manifest) = compatible_values();
        let health = HealthResponse::new(HealthReport {
            ok: true,
            db: "turso".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            db_path: "private".to_owned(),
            db_fingerprint: "private".to_owned(),
        });
        let endpoint = spawn_delayed_json_host(
            [
                serde_json::to_vec(&health).expect("health JSON"),
                serde_json::to_vec(&runtime).expect("runtime JSON"),
                serde_json::to_vec(&manifest).expect("manifest JSON"),
            ],
            Duration::from_millis(80),
        );
        let started_at = Instant::now();
        let error = probe_host_with_timeouts(
            endpoint,
            ProbeTimeouts {
                connect: Duration::from_millis(200),
                io: Duration::from_millis(200),
                total: Duration::from_millis(120),
            },
        )
        .expect_err("three delayed responses must exceed one shared deadline");
        let elapsed = started_at.elapsed();
        assert!(
            elapsed < Duration::from_millis(200),
            "probe drifted: {elapsed:?}"
        );
        assert!(matches!(error, ProbeError::Incompatible(_)));
    }

    #[cfg(unix)]
    #[test]
    fn wait_for_sidecar_caps_a_slow_probe_to_startup_deadline() {
        let process = test_sleep_child(false);
        let mut child = test_owned_child(process);
        let mut config = HostLaunchConfig::new("sidecar", "web", "db", "actor", "board");
        config.endpoint = spawn_stalled_host();
        config.startup_timeout = Duration::from_millis(100);
        config.probe_timeouts = ProbeTimeouts {
            connect: Duration::from_millis(500),
            io: Duration::from_millis(500),
            total: Duration::from_secs(2),
        };
        let started_at = Instant::now();
        let error = wait_for_sidecar(
            &config,
            &mut child,
            ProbeError::unavailable("preflight unavailable"),
        )
        .expect_err("stalled sidecar must hit startup deadline");
        let elapsed = started_at.elapsed();
        assert!(
            elapsed < Duration::from_millis(250),
            "startup drifted: {elapsed:?}"
        );
        assert!(matches!(
            error,
            HostStartupError::SidecarIncompatible { .. }
        ));
        let cleanup = force_stop(&mut child);
        assert!(cleanup.process_reaped, "test sidecar must be reaped");
    }

    fn assert_no_sidecar_spawn(endpoint: SocketAddr) {
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let marker = std::env::temp_dir().join(format!(
            "kanban-desktop-no-spawn-marker-{}-{id}",
            std::process::id(),
        ));
        let sidecar = std::env::temp_dir().join(format!(
            "kanban-desktop-no-spawn-sidecar-{}-{id}",
            std::process::id(),
        ));
        let _ = std::fs::remove_file(&marker);
        let _ = std::fs::remove_file(&sidecar);
        let script = format!(
            "#!/bin/sh\nprintf spawned > {}\nsleep 5\n",
            marker.display()
        );
        std::fs::write(&sidecar, script).expect("sidecar fixture");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&sidecar)
                .expect("sidecar metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&sidecar, permissions).expect("sidecar executable");
        }

        let mut config = HostLaunchConfig::new(sidecar.clone(), "web", "db", "actor", "board");
        config.endpoint = endpoint;
        config.startup_timeout = Duration::from_millis(100);
        config = config.with_probe_timeout(Duration::from_millis(50));
        let error = connect_or_spawn(&config).expect_err("incompatible host must block spawn");
        assert!(matches!(error, HostStartupError::PortConflict { .. }));
        assert!(
            !marker.exists(),
            "sidecar marker proves an unexpected spawn"
        );
        let _ = std::fs::remove_file(marker);
        let _ = std::fs::remove_file(sidecar);
    }

    #[cfg(unix)]
    #[test]
    fn startup_failure_reaps_owned_sidecar() {
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let marker = std::env::temp_dir().join(format!(
            "kanban-desktop-startup-exit-marker-{}-{id}",
            std::process::id(),
        ));
        let sidecar = std::env::temp_dir().join(format!(
            "kanban-desktop-startup-exit-sidecar-{}-{id}",
            std::process::id(),
        ));
        let endpoint = TcpListener::bind((DEFAULT_HOST, 0))
            .expect("startup fixture endpoint")
            .local_addr()
            .expect("startup fixture address");
        let script = format!("#!/bin/sh\nprintf '%s' $$ > {}\nexit 7\n", marker.display());
        std::fs::write(&sidecar, script).expect("startup sidecar fixture");
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&sidecar)
                .expect("startup sidecar metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&sidecar, permissions).expect("startup sidecar executable");
        }

        let mut config = HostLaunchConfig::new(sidecar.clone(), "web", "db", "actor", "board");
        config.endpoint = endpoint;
        config.startup_timeout = Duration::from_millis(100);
        config = config.with_probe_timeout(Duration::from_millis(25));
        let error = connect_or_spawn(&config).expect_err("exited sidecar must fail startup");
        assert!(matches!(error, HostStartupError::SidecarExited { .. }));
        let pid = std::fs::read_to_string(&marker)
            .expect("sidecar pid marker")
            .parse::<i32>()
            .expect("sidecar pid");
        assert!(
            unsafe { libc::kill(pid, 0) } != 0,
            "sidecar pid remains alive"
        );
        let _ = std::fs::remove_file(marker);
        let _ = std::fs::remove_file(sidecar);
    }

    #[test]
    fn external_host_shutdown_is_a_noop() {
        let mut handle = HostHandle {
            endpoint: default_endpoint(),
            ownership: HostOwnership::External,
            child: None,
        };
        assert_eq!(
            handle.shutdown().expect("external shutdown"),
            ShutdownResult::ExternalHostKept
        );
    }

    #[cfg(unix)]
    #[test]
    fn owned_host_shutdown_gracefully_stops_child() {
        let process = test_sleep_child(false);
        let mut handle = HostHandle {
            endpoint: default_endpoint(),
            ownership: HostOwnership::Owned,
            child: Some(test_owned_child(process)),
        };
        assert_eq!(
            handle.shutdown().expect("owned graceful shutdown"),
            ShutdownResult::Graceful
        );
        assert!(handle.child.is_none(), "graceful child must be reaped");
    }

    #[cfg(unix)]
    #[test]
    fn owned_host_shutdown_forces_child_after_graceful_timeout() {
        let process = test_sleep_child(true);
        let mut handle = HostHandle {
            endpoint: default_endpoint(),
            ownership: HostOwnership::Owned,
            child: Some(test_owned_child(process)),
        };
        assert_eq!(
            handle.shutdown().expect("owned force shutdown"),
            ShutdownResult::Forced
        );
        assert!(handle.child.is_none(), "forced child must be reaped");
    }

    #[cfg(unix)]
    #[test]
    fn external_host_shutdown_never_kills_process() {
        let process = test_sleep_child(false);
        let mut handle = HostHandle {
            endpoint: default_endpoint(),
            ownership: HostOwnership::External,
            child: Some(test_owned_child(process)),
        };
        assert_eq!(
            handle.shutdown().expect("external shutdown"),
            ShutdownResult::ExternalHostKept
        );
        let child = handle.child.as_mut().expect("external process retained");
        assert!(child.process.try_wait().expect("external status").is_none());
        child.process.kill().expect("cleanup external fixture");
        child.process.wait().expect("wait external fixture");
        child.finish_drain();
    }

    #[cfg(unix)]
    #[derive(Clone, Copy)]
    enum FakeTryWait {
        Running,
        Reaped,
        Error(&'static str),
    }

    #[cfg(unix)]
    #[derive(Clone, Copy)]
    enum FakeResult {
        Ok,
        Error(&'static str),
    }

    #[cfg(unix)]
    struct FakeCleanupProcess {
        try_wait_results: VecDeque<FakeTryWait>,
        kill_results: VecDeque<FakeResult>,
        wait_results: VecDeque<FakeResult>,
        kill_calls: usize,
        wait_calls: usize,
    }

    #[cfg(unix)]
    impl FakeCleanupProcess {
        fn new(
            try_wait_results: impl IntoIterator<Item = FakeTryWait>,
            kill_results: impl IntoIterator<Item = FakeResult>,
            wait_results: impl IntoIterator<Item = FakeResult>,
        ) -> Self {
            Self {
                try_wait_results: try_wait_results.into_iter().collect(),
                kill_results: kill_results.into_iter().collect(),
                wait_results: wait_results.into_iter().collect(),
                kill_calls: 0,
                wait_calls: 0,
            }
        }
    }

    #[cfg(unix)]
    impl ProcessCleanup for FakeCleanupProcess {
        fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
            match self
                .try_wait_results
                .pop_front()
                .unwrap_or(FakeTryWait::Running)
            {
                FakeTryWait::Running => Ok(None),
                FakeTryWait::Reaped => Ok(Some(std::process::ExitStatus::from_raw(0))),
                FakeTryWait::Error(message) => Err(io::Error::other(message)),
            }
        }

        fn kill(&mut self) -> io::Result<()> {
            self.kill_calls += 1;
            match self.kill_results.pop_front().unwrap_or(FakeResult::Ok) {
                FakeResult::Ok => Ok(()),
                FakeResult::Error(message) => Err(io::Error::other(message)),
            }
        }

        fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
            self.wait_calls += 1;
            match self.wait_results.pop_front().unwrap_or(FakeResult::Ok) {
                FakeResult::Ok => Ok(std::process::ExitStatus::from_raw(0)),
                FakeResult::Error(message) => Err(io::Error::other(message)),
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_reaps_already_exited_without_kill_or_wait() {
        let mut process = FakeCleanupProcess::new([FakeTryWait::Reaped], [], []);
        let outcome = force_stop_with(&mut process);
        assert!(outcome.process_reaped);
        assert!(outcome.first_error.is_none());
        assert_eq!(process.kill_calls, 0);
        assert_eq!(process.wait_calls, 0);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_kills_and_waits_running_process() {
        let mut process =
            FakeCleanupProcess::new([FakeTryWait::Running], [FakeResult::Ok], [FakeResult::Ok]);
        let outcome = force_stop_with(&mut process);
        assert!(outcome.process_reaped);
        assert_eq!(process.kill_calls, 1);
        assert_eq!(process.wait_calls, 1);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_wait_error_does_not_confirm_exit_or_join_drain() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running],
            [FakeResult::Ok],
            [FakeResult::Error("wait failed")],
        );
        let outcome = force_stop_with(&mut process);
        assert!(!outcome.process_reaped);
        assert!(!outcome.should_join_drain());
        assert_eq!(
            outcome.first_error.expect("wait error").to_string(),
            "wait failed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_kill_error_does_not_call_blocking_wait() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Running],
            [FakeResult::Error("kill failed")],
            [FakeResult::Error("wait must not run")],
        );
        let outcome = force_stop_with(&mut process);
        assert!(!outcome.process_reaped);
        assert_eq!(
            outcome.first_error.expect("kill error").to_string(),
            "kill failed"
        );
        assert_eq!(process.wait_calls, 0);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_kill_error_can_confirm_exit_with_nonblocking_retry() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Reaped],
            [FakeResult::Error("kill failed")],
            [],
        );
        let outcome = force_stop_with(&mut process);
        assert!(outcome.process_reaped);
        assert_eq!(
            outcome.first_error.expect("kill error").to_string(),
            "kill failed"
        );
        assert_eq!(process.wait_calls, 0);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_try_wait_error_is_preserved_as_first_error() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Error("try_wait failed"), FakeTryWait::Running],
            [FakeResult::Error("kill failed")],
            [FakeResult::Error("wait failed")],
        );
        let outcome = force_stop_with(&mut process);
        assert!(!outcome.process_reaped);
        assert_eq!(
            outcome.first_error.expect("try_wait error").to_string(),
            "try_wait failed"
        );
        assert_eq!(process.wait_calls, 0);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_wait_error_is_preserved_after_successful_kill() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running],
            [FakeResult::Ok],
            [FakeResult::Error("wait failed")],
        );
        let outcome = force_stop_with(&mut process);
        assert_eq!(
            outcome.first_error.expect("wait error").to_string(),
            "wait failed"
        );
        assert!(!outcome.process_reaped);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_reaped_outcome_releases_child_ownership() {
        let mut process = FakeCleanupProcess::new([FakeTryWait::Reaped], [], []);
        let outcome = force_stop_with(&mut process);
        assert_eq!(outcome.ownership(), CleanupOwnership::Release);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_unconfirmed_outcome_retains_child_ownership_for_retry() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Running],
            [FakeResult::Error("kill failed")],
            [],
        );
        let outcome = force_stop_with(&mut process);
        assert_eq!(outcome.ownership(), CleanupOwnership::RetainForRetry);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_retry_can_reap_after_unconfirmed_first_attempt() {
        let mut process = FakeCleanupProcess::new(
            [
                FakeTryWait::Running,
                FakeTryWait::Running,
                FakeTryWait::Running,
            ],
            [FakeResult::Error("kill failed"), FakeResult::Ok],
            [FakeResult::Ok],
        );
        let first = force_stop_with(&mut process);
        assert_eq!(first.ownership(), CleanupOwnership::RetainForRetry);
        let second = force_stop_with(&mut process);
        assert_eq!(second.ownership(), CleanupOwnership::Release);
        assert!(second.first_error.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_drain_join_is_gated_by_confirmed_reap() {
        let mut running = FakeCleanupProcess::new(
            [FakeTryWait::Running],
            [FakeResult::Ok],
            [FakeResult::Error("wait failed")],
        );
        let running_outcome = force_stop_with(&mut running);
        assert!(!running_outcome.should_join_drain());

        let mut reaped = FakeCleanupProcess::new([FakeTryWait::Reaped], [], []);
        let reaped_outcome = force_stop_with(&mut reaped);
        assert!(reaped_outcome.should_join_drain());
    }

    #[cfg(unix)]
    fn test_owned_child(mut process: Child) -> OwnedChild {
        let stderr = process.stderr.take().expect("stderr fixture pipe");
        let diagnostics = Arc::new(Mutex::new(VecDeque::with_capacity(
            MAX_STDERR_DIAGNOSTIC_BYTES,
        )));
        let diagnostics_for_thread = Arc::clone(&diagnostics);
        let drain = thread::spawn(move || drain_stderr(stderr, diagnostics_for_thread));
        OwnedChild {
            process,
            stderr: diagnostics,
            drain: Some(drain),
            drop_cleanup: true,
        }
    }

    #[cfg(unix)]
    fn test_sleep_child(ignore_sigint: bool) -> Child {
        let mut command = Command::new("sleep");
        command
            .arg("10")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        unsafe {
            command.pre_exec(move || {
                let handler = if ignore_sigint {
                    libc::SIG_IGN
                } else {
                    libc::SIG_DFL
                };
                if libc::signal(libc::SIGINT, handler) == libc::SIG_ERR {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command.spawn().expect("sleep fixture")
    }

    fn spawn_json_host<const N: usize>(responses: [Vec<u8>; N]) -> std::net::SocketAddr {
        let responses = responses.map(|body| {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .into_bytes()
                    .into_iter()
                    .chain(body)
                    .collect::<Vec<_>>()
        });
        spawn_raw_host(responses)
    }

    fn spawn_delayed_json_host<const N: usize>(
        bodies: [Vec<u8>; N],
        delay: Duration,
    ) -> std::net::SocketAddr {
        let responses = bodies.map(|body| {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes()
            .into_iter()
            .chain(body)
            .collect::<Vec<_>>()
        });
        let listener = TcpListener::bind((DEFAULT_HOST, 0)).expect("delayed probe listener");
        let endpoint = listener.local_addr().expect("delayed fixture address");
        thread::spawn(move || {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut request = Vec::new();
                let mut chunk = [0_u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let Ok(count) = stream.read(&mut chunk) else {
                        return;
                    };
                    if count == 0 {
                        return;
                    }
                    request.extend_from_slice(&chunk[..count]);
                }
                thread::sleep(delay);
                let _ = stream.write_all(&response);
            }
        });
        endpoint
    }

    fn spawn_raw_host<const N: usize>(responses: [Vec<u8>; N]) -> std::net::SocketAddr {
        let listener = TcpListener::bind((DEFAULT_HOST, 0)).expect("probe fixture listener");
        let endpoint = listener.local_addr().expect("fixture address");
        thread::spawn(move || {
            for response in responses {
                let (mut stream, _) = listener.accept().expect("fixture connection");
                let mut request = Vec::new();
                let mut chunk = [0_u8; 1024];
                while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let count = stream.read(&mut chunk).expect("fixture request");
                    if count == 0 {
                        return;
                    }
                    request.extend_from_slice(&chunk[..count]);
                }
                stream.write_all(&response).expect("fixture response");
            }
        });
        endpoint
    }

    fn spawn_stalled_host() -> SocketAddr {
        let listener = TcpListener::bind((DEFAULT_HOST, 0)).expect("stall fixture listener");
        let endpoint = listener.local_addr().expect("stall fixture address");
        thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                thread::sleep(Duration::from_millis(150));
                drop(stream);
            }
        });
        endpoint
    }

    fn spawn_slow_drip_host() -> SocketAddr {
        let listener = TcpListener::bind((DEFAULT_HOST, 0)).expect("drip fixture listener");
        let endpoint = listener.local_addr().expect("drip fixture address");
        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let response =
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
            for byte in response {
                if stream.write_all(&[*byte]).is_err() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        endpoint
    }
}
