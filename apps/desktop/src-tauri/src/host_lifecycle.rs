use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

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
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(25);
const HOST_POLL_INTERVAL: Duration = Duration::from_millis(50);
const REAPER_RETRY_INTERVAL: Duration = Duration::from_millis(25);
const REAPER_MAX_BACKOFF: Duration = Duration::from_millis(500);
const REAPER_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(1);
const STDERR_DRAIN_TIMEOUT: Duration = Duration::from_millis(250);
const MAX_HTTP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const MAX_STDERR_DIAGNOSTIC_BYTES: usize = 16 * 1024;

#[cfg(unix)]
type OwnedProcessGroup = libc::pid_t;
#[cfg(not(unix))]
type OwnedProcessGroup = ();

#[cfg(unix)]
fn is_process_group_gone(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn is_process_group_gone(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::NotFound
}

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
    Cancelled,
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
            Self::Cancelled => formatter.write_str("host probe 已取消"),
        }
    }
}

impl std::error::Error for ProbeError {}

/// 探测固定 loopback host 的 health、runtime 与 Web artifact manifest。
#[allow(dead_code)]
pub fn probe_host(endpoint: SocketAddr) -> Result<HostCompatibility, ProbeError> {
    probe_host_with_timeouts(endpoint, ProbeTimeouts::default())
}

pub(crate) fn probe_host_with_cancel(
    endpoint: SocketAddr,
    cancel: &AtomicBool,
) -> Result<HostCompatibility, ProbeError> {
    probe_host_with_timeouts_cancel(endpoint, ProbeTimeouts::default(), Some(cancel))
}

fn probe_host_with_timeouts(
    endpoint: SocketAddr,
    timeouts: ProbeTimeouts,
) -> Result<HostCompatibility, ProbeError> {
    probe_host_with_timeouts_cancel(endpoint, timeouts, None)
}

fn probe_host_with_timeouts_cancel(
    endpoint: SocketAddr,
    timeouts: ProbeTimeouts,
    cancel: Option<&AtomicBool>,
) -> Result<HostCompatibility, ProbeError> {
    let deadline = deadline_after(Instant::now(), timeouts.total);
    probe_host_until_cancel(endpoint, timeouts, deadline, cancel)
}

fn probe_host_until_cancel(
    endpoint: SocketAddr,
    timeouts: ProbeTimeouts,
    deadline: Instant,
    cancel: Option<&AtomicBool>,
) -> Result<HostCompatibility, ProbeError> {
    if is_cancelled(cancel) {
        return Err(ProbeError::Cancelled);
    }
    if !endpoint.ip().is_loopback() {
        return Err(ProbeError::incompatible(
            "Desktop host probe 只允许 loopback 地址",
        ));
    }
    let health =
        get_json_cancel::<HealthResponse>(endpoint, "/health", timeouts, deadline, cancel)?;
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

    let runtime = get_json_cancel::<WebRuntimeConfig>(
        endpoint,
        "/app/runtime.json",
        timeouts,
        deadline,
        cancel,
    )?;
    let manifest = get_json_cancel::<WebArtifactManifest>(
        endpoint,
        "/app/manifest.json",
        timeouts,
        deadline,
        cancel,
    )?;
    let compatibility = HostCompatibility::verify(runtime, manifest, env!("CARGO_PKG_VERSION"))
        .map_err(|error| ProbeError::incompatible(error.to_string()))?;
    if is_cancelled(cancel) {
        return Err(ProbeError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(ProbeError::incompatible(
            "host probe total deadline exceeded",
        ));
    }
    Ok(compatibility)
}

fn get_json_cancel<T>(
    endpoint: SocketAddr,
    path: &str,
    timeouts: ProbeTimeouts,
    deadline: Instant,
    cancel: Option<&AtomicBool>,
) -> Result<T, ProbeError>
where
    T: serde::de::DeserializeOwned,
{
    let response = get_cancel(endpoint, path, timeouts, deadline, cancel)?;
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

fn get_cancel(
    endpoint: SocketAddr,
    path: &str,
    timeouts: ProbeTimeouts,
    deadline: Instant,
    cancel: Option<&AtomicBool>,
) -> Result<HttpResponse, ProbeError> {
    if is_cancelled(cancel) {
        return Err(ProbeError::Cancelled);
    }
    let connect_timeout = remaining_timeout(deadline, timeouts.connect)
        .map(|timeout| cancellation_timeout(timeout, cancel))
        .map_err(|error| {
            ProbeError::unavailable(format!(
                "无法连接固定 host http://{endpoint}{path}: {error}"
            ))
        })?;
    let mut stream = TcpStream::connect_timeout(&endpoint, connect_timeout).map_err(|error| {
        ProbeError::unavailable(format!(
            "无法连接固定 host http://{endpoint}{path}: {error}"
        ))
    })?;
    stream
        .set_read_timeout(Some(
            remaining_timeout(deadline, timeouts.io)
                .map(|timeout| cancellation_timeout(timeout, cancel))
                .map_err(|error| {
                    ProbeError::incompatible(format!("设置 host 读取超时失败: {error}"))
                })?,
        ))
        .map_err(|error| ProbeError::incompatible(format!("设置 host 读取超时失败: {error}")))?;
    stream
        .set_write_timeout(Some(
            remaining_timeout(deadline, timeouts.io)
                .map(|timeout| cancellation_timeout(timeout, cancel))
                .map_err(|error| {
                    ProbeError::incompatible(format!("设置 host 写入超时失败: {error}"))
                })?,
        ))
        .map_err(|error| ProbeError::incompatible(format!("设置 host 写入超时失败: {error}")))?;
    if is_cancelled(cancel) {
        return Err(ProbeError::Cancelled);
    }
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {endpoint}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| ProbeError::incompatible(format!("发送 host probe 请求失败: {error}")))?;
    read_http_response_cancel(&mut stream, deadline, timeouts.io, cancel).map_err(|error| {
        if error.kind() == io::ErrorKind::Interrupted {
            ProbeError::Cancelled
        } else {
            ProbeError::incompatible(error.to_string())
        }
    })
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

fn read_http_response_cancel(
    stream: &mut TcpStream,
    deadline: Instant,
    io_timeout: Duration,
    cancel: Option<&AtomicBool>,
) -> io::Result<HttpResponse> {
    let mut bytes = Vec::new();
    let header_end = loop {
        if is_cancelled(cancel) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "host probe 已取消",
            ));
        }
        stream.set_read_timeout(Some(
            remaining_timeout(deadline, io_timeout)
                .map(|timeout| cancellation_timeout(timeout, cancel))?,
        ))?;
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
        if is_cancelled(cancel) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "host probe 已取消",
            ));
        }
        stream.set_read_timeout(Some(
            remaining_timeout(deadline, io_timeout)
                .map(|timeout| cancellation_timeout(timeout, cancel))?,
        ))?;
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

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|cancel| cancel.load(Ordering::Acquire))
}

fn cancellation_timeout(timeout: Duration, cancel: Option<&AtomicBool>) -> Duration {
    if cancel.is_some() {
        timeout.min(CANCEL_POLL_INTERVAL)
    } else {
        timeout
    }
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
    Cancelled,
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
            Self::Cancelled => formatter.write_str("Desktop host bootstrap 已取消"),
        }
    }
}

impl std::error::Error for HostStartupError {}

#[cfg(unix)]
fn configure_owned_process_group(command: &mut Command) {
    // sidecar 成为独立 process group leader；owned cleanup 只向保存的 PGID 发信号。
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_owned_process_group(_command: &mut Command) {}

fn process_group_for(process: &Child) -> Option<OwnedProcessGroup> {
    #[cfg(unix)]
    {
        Some(process.id() as libc::pid_t)
    }
    #[cfg(not(unix))]
    {
        let _ = process;
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOwnership {
    External,
    Owned,
}

#[derive(Debug)]
pub struct HostHandle {
    ownership: HostOwnership,
    child: Option<OwnedChild>,
}

#[derive(Debug)]
struct DrainHandle {
    join: thread::JoinHandle<()>,
    done: Receiver<()>,
}

#[derive(Debug)]
struct OwnedChild {
    process: Option<Child>,
    process_group: Option<OwnedProcessGroup>,
    group_signal_sent: bool,
    stderr: Arc<Mutex<VecDeque<u8>>>,
    drain: Option<DrainHandle>,
    drop_cleanup: bool,
}

static REAPER_FALLBACK_QUEUE: OnceLock<Mutex<Vec<OwnedChild>>> = OnceLock::new();
static REAPER_SERVICE: OnceLock<Mutex<Option<mpsc::Sender<OwnedChild>>>> = OnceLock::new();
static REAPER_RETRY_PUMP_RUNNING: AtomicBool = AtomicBool::new(false);
static REAPER_PENDING_CHILDREN: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
static REAPER_TEST_GUARD: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
static FORCE_REAPER_SPAWN_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(test)]
static FORCE_REAPER_SERVICE_SPAWN_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

impl OwnedChild {
    fn process(&self) -> &Child {
        self.process
            .as_ref()
            .expect("owned sidecar process ownership已转移")
    }

    fn process_mut(&mut self) -> &mut Child {
        self.process
            .as_mut()
            .expect("owned sidecar process ownership已转移")
    }

    fn release_process(&mut self) {
        self.process.take();
    }

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
        let Some(drain) = self.drain.take() else {
            return;
        };
        if drain.done.recv_timeout(STDERR_DRAIN_TIMEOUT).is_ok() {
            let _ = drain.join.join();
        } else {
            eprintln!(
                "kanban sidecar stderr drain 超过 {:?}，detach reader",
                STDERR_DRAIN_TIMEOUT
            );
        }
    }

    fn detach_for_reaper(&mut self) -> Option<Self> {
        self.process.take().map(|process| Self {
            process: Some(process),
            process_group: self.process_group,
            group_signal_sent: self.group_signal_sent,
            stderr: Arc::clone(&self.stderr),
            drain: self.drain.take(),
            drop_cleanup: false,
        })
    }

    fn disarm_drop_cleanup(&mut self) {
        self.drop_cleanup = false;
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.drop_cleanup {
            return;
        }
        let Some(mut child) = self.detach_for_reaper() else {
            return;
        };
        let cleanup = force_stop(&mut child);
        if cleanup.ownership() == CleanupOwnership::Release {
            handoff_to_reaper(child, "OwnedChild Drop after confirmed exit");
        } else {
            handoff_to_reaper(child, "OwnedChild Drop");
        }
    }
}

impl HostHandle {
    #[cfg(test)]
    pub(crate) fn external_for_test() -> Self {
        Self {
            ownership: HostOwnership::External,
            child: None,
        }
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
        match child.process_mut().try_wait() {
            Ok(Some(_)) => {
                if finish_reaped_child(child) {
                    self.child.take();
                    return Ok(ShutdownResult::AlreadyExited);
                }
                return Err(ShutdownError::Io(
                    "sidecar leader 已退出但 process group 尚未确认消失；保留 ownership 重试"
                        .to_owned(),
                ));
            }
            Ok(None) => {}
            Err(error) => {
                return self.cleanup_after_error(ShutdownError::Io(error.to_string()));
            }
        }

        if let Err(error) = request_graceful_stop(child) {
            let first_error = ShutdownError::GracefulRequest(error.to_string());
            return self.cleanup_after_error(first_error);
        }
        let deadline = deadline_after(Instant::now(), GRACEFUL_SHUTDOWN_TIMEOUT);
        loop {
            match child.process_mut().try_wait() {
                Ok(Some(_)) => {
                    if finish_reaped_child(child) {
                        self.child.take();
                        return Ok(ShutdownResult::Graceful);
                    }
                    return Err(ShutdownError::Io(
                        "sidecar leader 已退出但 process group 尚未确认消失；保留 ownership 重试"
                            .to_owned(),
                    ));
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
                        child.finish_drain();
                        child.release_process();
                        self.child.take();
                    }
                    if ownership == CleanupOwnership::RetainForRetry {
                        return Err(cleanup_error.unwrap_or_else(|| {
                            ShutdownError::Io(
                                "强制停止已发出，但尚未确认 sidecar reap；保留 ownership 供重试"
                                    .to_owned(),
                            )
                        }));
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

    pub(crate) fn shutdown_nonblocking(&mut self) -> Result<ShutdownResult, ShutdownError> {
        if self.ownership == HostOwnership::External {
            if let Some(child) = self.child.as_mut() {
                child.disarm_drop_cleanup();
            }
            return Ok(ShutdownResult::ExternalHostKept);
        }
        let Some(child) = self.child.as_mut() else {
            return Ok(ShutdownResult::AlreadyExited);
        };
        if child
            .process_mut()
            .try_wait()
            .map_err(|error| ShutdownError::Io(error.to_string()))?
            .is_some()
        {
            if finish_reaped_child(child) {
                self.child.take();
                return Ok(ShutdownResult::AlreadyExited);
            }
            return Err(ShutdownError::Io(
                "sidecar leader 已退出但 process group 尚未确认消失；保留 ownership 重试"
                    .to_owned(),
            ));
        }

        let graceful_error = request_graceful_stop(child).err();
        let cleanup = force_stop(child);
        if cleanup.ownership() == CleanupOwnership::Release {
            child.finish_drain();
            child.release_process();
            self.child.take();
            if let Some(error) = cleanup.first_error.or(graceful_error) {
                return Err(ShutdownError::Io(error.to_string()));
            }
            return Ok(ShutdownResult::Forced);
        }
        Err(ShutdownError::Io(
            cleanup
                .first_error
                .or(graceful_error)
                .map(|error| error.to_string())
                .unwrap_or_else(|| {
                    "强制停止已发出，但尚未确认 sidecar reap；保留 ownership 供重试".to_owned()
                }),
        ))
    }

    fn cleanup_after_error(
        &mut self,
        first_error: ShutdownError,
    ) -> Result<ShutdownResult, ShutdownError> {
        let cleanup = self
            .child
            .as_mut()
            .expect("owned host child disappeared during shutdown");
        let cleanup_outcome = force_stop(cleanup);
        if cleanup_outcome.ownership() == CleanupOwnership::Release {
            cleanup.finish_drain();
            cleanup.release_process();
            self.child.take();
        }
        if let Some(cleanup_error) = cleanup_outcome.first_error {
            eprintln!("kanban owned host cleanup 失败：{cleanup_error}");
        }
        Err(first_error)
    }
}

impl Drop for HostHandle {
    fn drop(&mut self) {
        if self.ownership == HostOwnership::External {
            if let Some(child) = self.child.as_mut() {
                child.disarm_drop_cleanup();
            }
            return;
        }
        let Some(mut child) = self.child.take() else {
            return;
        };
        let cleanup = force_stop(&mut child);
        if let Some(ref error) = cleanup.first_error {
            eprintln!("kanban owned host Drop cleanup 失败：{error}");
        }
        if let Some(child) = child.detach_for_reaper() {
            let context = if cleanup.ownership() == CleanupOwnership::Release {
                "owned host Drop after confirmed exit"
            } else {
                "owned host Drop"
            };
            handoff_to_reaper(child, context);
        }
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
    WorkerInFlight,
    ReaperInFlight,
}

impl std::fmt::Display for ShutdownError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) | Self::GracefulRequest(message) => formatter.write_str(message),
            Self::WorkerInFlight => {
                formatter.write_str("桌面 bootstrap worker 尚未退出；保留 host ownership 重试")
            }
            Self::ReaperInFlight => {
                formatter.write_str("sidecar reaper 尚未确认 process group 清理；保留桌面进程重试")
            }
        }
    }
}

impl std::error::Error for ShutdownError {}

#[allow(dead_code)]
pub fn connect_or_spawn(config: &HostLaunchConfig) -> Result<HostHandle, HostStartupError> {
    connect_or_spawn_inner(config, None)
}

pub(crate) fn connect_or_spawn_with_cancel(
    config: &HostLaunchConfig,
    cancel: &AtomicBool,
) -> Result<HostHandle, HostStartupError> {
    connect_or_spawn_inner(config, Some(cancel))
}

fn connect_or_spawn_inner(
    config: &HostLaunchConfig,
    cancel: Option<&AtomicBool>,
) -> Result<HostHandle, HostStartupError> {
    if is_cancelled(cancel) {
        return Err(HostStartupError::Cancelled);
    }
    match probe_host_with_timeouts_cancel(config.endpoint, config.probe_timeouts, cancel) {
        Ok(_) => Ok(HostHandle {
            ownership: HostOwnership::External,
            child: None,
        }),
        Err(ProbeError::Cancelled) => Err(HostStartupError::Cancelled),
        Err(ProbeError::Incompatible(message)) => Err(HostStartupError::PortConflict {
            endpoint: config.endpoint,
            message,
        }),
        Err(initial_probe @ ProbeError::Unavailable(_)) => {
            if is_cancelled(cancel) {
                return Err(HostStartupError::Cancelled);
            }
            let mut child = spawn_sidecar(config)?;
            let result = wait_for_sidecar_with_cancel(config, &mut child, initial_probe, cancel);
            match result {
                Ok(_) => Ok(HostHandle {
                    ownership: HostOwnership::Owned,
                    child: Some(child),
                }),
                Err(error) => {
                    let cleanup = force_stop(&mut child);
                    if let Some(ref cleanup_error) = cleanup.first_error {
                        eprintln!("kanban sidecar cleanup 失败：{cleanup_error}");
                    }
                    if cleanup.ownership() == CleanupOwnership::Release {
                        child.finish_drain();
                        child.release_process();
                    } else if let Some(child) = child.detach_for_reaper() {
                        handoff_to_reaper(child, "startup error");
                    }
                    Err(error)
                }
            }
        }
    }
}

fn spawn_sidecar(config: &HostLaunchConfig) -> Result<OwnedChild, HostStartupError> {
    // 先建立可恢复的后台 cleanup owner；这样真正拥有 Child 之后，Drop 不会遇到无法交接的
    // spawn failure。测试仍可通过 FORCE_REAPER_SPAWN_FAILURE 覆盖 handoff fallback。
    #[cfg(not(test))]
    if reaper_service("sidecar bootstrap").is_none() {
        return Err(HostStartupError::SidecarSpawn {
            path: config.sidecar_path.clone(),
            message: "后台 sidecar reaper 无法启动".to_owned(),
        });
    }
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
    configure_owned_process_group(&mut command);
    let mut process = command
        .spawn()
        .map_err(|error| HostStartupError::SidecarSpawn {
            path: config.sidecar_path.clone(),
            message: error.to_string(),
        })?;
    let stderr = match process.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let diagnostics = Arc::new(Mutex::new(VecDeque::with_capacity(
                MAX_STDERR_DIAGNOSTIC_BYTES,
            )));
            cleanup_spawn_failure(
                process,
                diagnostics,
                "sidecar stderr 管道创建失败".to_owned(),
            );
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
    let (drain_done_tx, drain_done_rx) = mpsc::channel();
    let drain = match thread::Builder::new()
        .name("kanban-desktop-sidecar-stderr".to_owned())
        .spawn(move || {
            drain_stderr(stderr, diagnostics_for_thread);
            let _ = drain_done_tx.send(());
        }) {
        Ok(drain) => drain,
        Err(error) => {
            cleanup_spawn_failure(
                process,
                diagnostics,
                format!("sidecar stderr drain 启动失败: {error}"),
            );
            return Err(HostStartupError::SidecarSpawn {
                path: config.sidecar_path.clone(),
                message: format!("sidecar stderr drain 启动失败: {error}"),
            });
        }
    };
    let process_group = process_group_for(&process);
    Ok(OwnedChild {
        process: Some(process),
        process_group,
        group_signal_sent: false,
        stderr: diagnostics,
        drain: Some(DrainHandle {
            join: drain,
            done: drain_done_rx,
        }),
        drop_cleanup: true,
    })
}

fn cleanup_spawn_failure(process: Child, diagnostics: Arc<Mutex<VecDeque<u8>>>, message: String) {
    let process_group = process_group_for(&process);
    let mut child = OwnedChild {
        process: Some(process),
        process_group,
        group_signal_sent: false,
        stderr: diagnostics,
        drain: None,
        drop_cleanup: true,
    };
    let cleanup = force_stop(&mut child);
    if let Some(ref error) = cleanup.first_error {
        eprintln!("kanban sidecar spawn cleanup 失败：{error}");
    }
    if cleanup.ownership() == CleanupOwnership::Release {
        child.release_process();
    } else if let Some(child) = child.detach_for_reaper() {
        handoff_to_reaper(child, &message);
    }
}

#[allow(dead_code)]
fn wait_for_sidecar(
    config: &HostLaunchConfig,
    child: &mut OwnedChild,
    initial_probe: ProbeError,
) -> Result<HostCompatibility, HostStartupError> {
    wait_for_sidecar_with_cancel(config, child, initial_probe, None)
}

fn wait_for_sidecar_with_cancel(
    config: &HostLaunchConfig,
    child: &mut OwnedChild,
    initial_probe: ProbeError,
    cancel: Option<&AtomicBool>,
) -> Result<HostCompatibility, HostStartupError> {
    let deadline = deadline_after(Instant::now(), config.startup_timeout);
    loop {
        if is_cancelled(cancel) {
            return Err(HostStartupError::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(HostStartupError::StartupTimeout {
                endpoint: config.endpoint,
                message: "startup 总 deadline 已耗尽".to_owned(),
            });
        }
        let probe_deadline =
            capped_probe_deadline(Instant::now(), deadline, config.probe_timeouts.total);
        let latest_error = match probe_host_until_cancel(
            config.endpoint,
            config.probe_timeouts,
            probe_deadline,
            cancel,
        ) {
            Ok(compatibility) => return Ok(compatibility),
            Err(error) => error,
        };
        if matches!(latest_error, ProbeError::Cancelled) {
            return Err(HostStartupError::Cancelled);
        }
        if let Some(status) =
            child
                .process_mut()
                .try_wait()
                .map_err(|error| HostStartupError::SidecarExited {
                    message: error.to_string(),
                })?
        {
            let _ = finish_reaped_child(child);
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
                ProbeError::Cancelled => HostStartupError::Cancelled,
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
    deadline_after(started_at, probe_total).min(startup_deadline)
}

fn deadline_after(started_at: Instant, duration: Duration) -> Instant {
    started_at.checked_add(duration).unwrap_or(started_at)
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

    /// 清理过程除了 leader 之外的 process group；fake seam 没有 group，默认视为已确认。
    fn group_cleanup_confirmed(&mut self, _first_error: &mut Option<io::Error>) -> bool {
        true
    }
}

#[derive(Debug)]
struct CleanupOutcome {
    first_error: Option<io::Error>,
    process_reaped: bool,
    group_cleanup_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupOwnership {
    RetainForRetry,
    Release,
}

impl CleanupOutcome {
    fn ownership(&self) -> CleanupOwnership {
        if self.process_reaped && self.group_cleanup_confirmed {
            CleanupOwnership::Release
        } else {
            CleanupOwnership::RetainForRetry
        }
    }

    #[cfg(test)]
    fn should_join_drain(&self) -> bool {
        self.process_reaped
    }
}

/// 只做非阻塞 cleanup；未确认 reap 时必须继续持有 Child ownership。
fn force_stop_with<P: ProcessCleanup>(process: &mut P) -> CleanupOutcome {
    let mut first_error = None;
    let process_reaped = match process.try_wait() {
        Ok(Some(_)) => true,
        Ok(None) => stop_without_wait(process, &mut first_error),
        Err(error) => {
            first_error = Some(error);
            stop_without_wait(process, &mut first_error)
        }
    };
    let group_cleanup_confirmed = process.group_cleanup_confirmed(&mut first_error);
    CleanupOutcome {
        first_error,
        process_reaped,
        group_cleanup_confirmed,
    }
}

fn stop_without_wait<P: ProcessCleanup>(
    process: &mut P,
    first_error: &mut Option<io::Error>,
) -> bool {
    if let Err(error) = process.kill() {
        if first_error.is_none() {
            *first_error = Some(error);
        }
        return matches!(process.try_wait(), Ok(Some(_)));
    }
    match process.try_wait() {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(error) => {
            if first_error.is_none() {
                *first_error = Some(error);
            }
            false
        }
    }
}

#[cfg(unix)]
fn kill_owned_process_group(child: &OwnedChild, signal: libc::c_int) -> io::Result<()> {
    let process_group = child.process_group.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "owned sidecar process group is unavailable",
        )
    })?;
    let leader = child.process().id() as libc::pid_t;
    if process_group <= 1 || process_group != leader {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "owned sidecar process group no longer matches its leader",
        ));
    }
    let result = unsafe { libc::kill(-process_group, signal) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn owned_process_group_gone(child: &OwnedChild) -> io::Result<bool> {
    let process_group = child.process_group.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "owned sidecar process group is unavailable",
        )
    })?;
    if process_group <= 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "owned sidecar process group is invalid",
        ));
    }
    let result = unsafe { libc::kill(-process_group, 0) };
    if result == 0 {
        Ok(false)
    } else {
        let error = io::Error::last_os_error();
        if is_process_group_gone(&error) {
            Ok(true)
        } else {
            Err(error)
        }
    }
}

fn confirm_owned_group_cleanup(
    child: &mut OwnedChild,
    first_error: &mut Option<io::Error>,
) -> bool {
    #[cfg(unix)]
    {
        let probe = match owned_process_group_gone(child) {
            Ok(gone) => gone,
            Err(error) if is_process_group_gone(&error) => true,
            Err(error) => {
                if first_error.is_none() {
                    *first_error = Some(error);
                }
                return false;
            }
        };
        if probe {
            return true;
        }
        if child.group_signal_sent {
            return false;
        }
        match kill_owned_process_group(child, libc::SIGKILL) {
            Ok(()) => child.group_signal_sent = true,
            Err(error) if is_process_group_gone(&error) => return true,
            Err(error) => {
                if first_error.is_none() {
                    *first_error = Some(error);
                }
                return false;
            }
        }
        match owned_process_group_gone(child) {
            Ok(gone) => gone,
            Err(error) if is_process_group_gone(&error) => true,
            Err(error) => {
                if first_error.is_none() {
                    *first_error = Some(error);
                }
                false
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (child, first_error);
        true
    }
}

fn finish_reaped_child(child: &mut OwnedChild) -> bool {
    // leader 已经被 wait/reap 后仍持有原 Child/PGID ownership；若 group 仍在，只补发一次
    // SIGKILL，之后必须等待 ESRCH 才释放 ownership。
    let mut cleanup_error = None;
    let group_gone = confirm_owned_group_cleanup(child, &mut cleanup_error);
    if let Some(error) = cleanup_error {
        eprintln!("kanban sidecar descendant group cleanup 失败：{error}");
    }
    if !group_gone {
        return false;
    }
    child.finish_drain();
    child.release_process();
    true
}

struct OwnedProcessCleanup<'a> {
    child: &'a mut OwnedChild,
    group_signal_sent: bool,
}

impl<'a> OwnedProcessCleanup<'a> {
    fn new(child: &'a mut OwnedChild) -> Self {
        Self {
            child,
            group_signal_sent: false,
        }
    }
}

impl ProcessCleanup for OwnedProcessCleanup<'_> {
    fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        let status = self.child.process_mut().try_wait()?;
        Ok(status)
    }

    fn kill(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            match kill_owned_process_group(self.child, libc::SIGKILL) {
                Ok(()) => {
                    self.group_signal_sent = true;
                    self.child.group_signal_sent = true;
                    Ok(())
                }
                Err(error) if is_process_group_gone(&error) => {
                    self.group_signal_sent = true;
                    self.child.group_signal_sent = true;
                    // leader 可能仍存活但已自行脱离原 PGID；此时旧 group 的 ESRCH 不能
                    // 代替 leader cleanup，Child 仍由本调用方持有，直接 kill 是安全兜底。
                    match self.child.process_mut().kill() {
                        Ok(()) => Ok(()),
                        Err(kill_error) if is_process_group_gone(&kill_error) => Ok(()),
                        Err(kill_error) => Err(kill_error),
                    }
                }
                Err(group_error) => {
                    // leader kill 仅作最后兜底，group error 仍需保留，使 caller 不会误以为
                    // descendant 已清理完毕。
                    let _ = self.child.process_mut().kill();
                    Err(group_error)
                }
            }
        }
        #[cfg(not(unix))]
        {
            self.child.process_mut().kill()
        }
    }

    fn group_cleanup_confirmed(&mut self, first_error: &mut Option<io::Error>) -> bool {
        #[cfg(unix)]
        {
            self.child.group_signal_sent |= self.group_signal_sent;
            confirm_owned_group_cleanup(self.child, first_error)
        }
        #[cfg(not(unix))]
        {
            let _ = first_error;
            true
        }
    }
}

fn force_stop(child: &mut OwnedChild) -> CleanupOutcome {
    if child.process.is_none() {
        return CleanupOutcome {
            first_error: None,
            process_reaped: true,
            group_cleanup_confirmed: true,
        };
    }
    let mut cleanup = OwnedProcessCleanup::new(child);
    force_stop_with(&mut cleanup)
}

/// 将仍由本进程拥有的 sidecar 交给后台 reaper，避免 UI/Drop 等待子进程。
fn handoff_to_reaper(child: OwnedChild, context: &str) {
    REAPER_PENDING_CHILDREN.fetch_add(1, Ordering::AcqRel);
    #[cfg(test)]
    if FORCE_REAPER_SPAWN_FAILURE.load(Ordering::Acquire) {
        eprintln!("kanban {context} 注入 reaper spawn failure；保留 Child ownership");
        REAPER_FALLBACK_QUEUE
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(child);
        schedule_reaper_retry(context);
        return;
    }
    REAPER_FALLBACK_QUEUE
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(child);
    let Some(service) = reaper_service(context) else {
        schedule_reaper_retry(context);
        return;
    };
    flush_pending_reapers(service, context);
}

pub(crate) fn reaper_pending_count() -> usize {
    REAPER_PENDING_CHILDREN.load(Ordering::Acquire)
}

#[cfg(test)]
pub(crate) fn reaper_test_guard() -> std::sync::MutexGuard<'static, ()> {
    REAPER_TEST_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
pub(crate) fn hold_reaper_pending_for_test() {
    REAPER_PENDING_CHILDREN.fetch_add(1, Ordering::AcqRel);
}

#[cfg(test)]
pub(crate) fn release_reaper_pending_for_test() {
    mark_reaper_child_released();
}

fn mark_reaper_child_released() {
    let _ = REAPER_PENDING_CHILDREN.fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
        Some(count.saturating_sub(1))
    });
}

fn reaper_queue_is_empty() -> bool {
    REAPER_FALLBACK_QUEUE
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .is_empty()
}

/// service 建立失败时由独立、有限次数的 pump 自驱重试；调用方只入队，不等待子进程。
fn schedule_reaper_retry(context: &str) {
    if REAPER_RETRY_PUMP_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let context = context.to_owned();
    let context_for_thread = context.clone();
    let result = thread::Builder::new()
        .name("kanban-desktop-sidecar-reaper-retry".to_owned())
        .spawn(move || {
            let mut delay = REAPER_RETRY_INTERVAL;
            for _ in 0..8 {
                if let Some(service) = reaper_service(&context_for_thread) {
                    flush_pending_reapers(service, &context_for_thread);
                    if reaper_queue_is_empty() {
                        REAPER_RETRY_PUMP_RUNNING.store(false, Ordering::Release);
                        if !reaper_queue_is_empty() {
                            schedule_reaper_retry(&context_for_thread);
                        }
                        return;
                    }
                }
                thread::sleep(delay);
                delay = delay
                    .checked_mul(2)
                    .unwrap_or(REAPER_MAX_BACKOFF)
                    .min(REAPER_MAX_BACKOFF);
            }
            REAPER_RETRY_PUMP_RUNNING.store(false, Ordering::Release);
            eprintln!(
                "kanban {context_for_thread} reaper retry pump 已耗尽；保留 Child ownership 等待后续 handoff"
            );
        });
    if result.is_err() {
        REAPER_RETRY_PUMP_RUNNING.store(false, Ordering::Release);
        eprintln!("kanban {context} reaper retry pump 启动失败；保留 Child ownership");
    }
}

fn reaper_service(context: &str) -> Option<mpsc::Sender<OwnedChild>> {
    #[cfg(test)]
    if FORCE_REAPER_SERVICE_SPAWN_FAILURE.load(Ordering::Acquire) {
        eprintln!("kanban {context} 注入 reaper service spawn failure");
        return None;
    }
    let service = REAPER_SERVICE.get_or_init(|| Mutex::new(None));
    let mut service = service
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(sender) = service.as_ref() {
        return Some(sender.clone());
    }

    let (sender, receiver) = mpsc::channel();
    let result = thread::Builder::new()
        .name("kanban-desktop-sidecar-reaper".to_owned())
        .spawn(move || run_reaper_service(receiver));
    match result {
        Ok(_) => {
            *service = Some(sender.clone());
            Some(sender)
        }
        Err(error) => {
            eprintln!(
                "kanban {context} 后台 reaper 启动失败：{error}；保留 Child ownership 等待后续重试"
            );
            None
        }
    }
}

#[derive(Debug)]
struct ReapTask {
    child: OwnedChild,
    next_due: Instant,
    backoff: Duration,
    next_error_log: Instant,
}

impl ReapTask {
    fn new(child: OwnedChild) -> Self {
        Self {
            child,
            next_due: Instant::now(),
            backoff: REAPER_RETRY_INTERVAL,
            next_error_log: Instant::now(),
        }
    }
}

fn run_reaper_service(receiver: mpsc::Receiver<OwnedChild>) {
    let mut pending = VecDeque::<ReapTask>::new();
    loop {
        while let Ok(child) = receiver.try_recv() {
            pending.push_back(ReapTask::new(child));
        }

        let now = Instant::now();
        if let Some(index) = pending.iter().position(|task| task.next_due <= now) {
            let mut task = pending
                .remove(index)
                .expect("reaper task index disappeared");
            let cleanup = force_stop(&mut task.child);
            if cleanup.ownership() == CleanupOwnership::Release {
                task.child.finish_drain();
                task.child.release_process();
                mark_reaper_child_released();
                continue;
            }
            if let Some(error) = cleanup.first_error
                && Instant::now() >= task.next_error_log
            {
                eprintln!("kanban sidecar reaper {error}，继续持有 ownership");
                task.next_error_log = deadline_after(Instant::now(), REAPER_ERROR_LOG_INTERVAL);
            }
            task.next_due = deadline_after(Instant::now(), task.backoff);
            task.backoff = task
                .backoff
                .checked_mul(2)
                .unwrap_or(REAPER_MAX_BACKOFF)
                .min(REAPER_MAX_BACKOFF);
            pending.push_back(task);
            continue;
        }

        let wait = pending
            .iter()
            .map(|task| task.next_due.saturating_duration_since(Instant::now()))
            .min()
            .unwrap_or(REAPER_MAX_BACKOFF);
        match receiver.recv_timeout(wait) {
            Ok(child) => pending.push_back(ReapTask::new(child)),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn clear_reaper_service() {
    if let Some(service) = REAPER_SERVICE.get() {
        *service
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

fn flush_pending_reapers(mut service: mpsc::Sender<OwnedChild>, context: &str) {
    let mut restart_attempts = 0;
    loop {
        let child = REAPER_FALLBACK_QUEUE
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop();
        let Some(child) = child else {
            return;
        };
        match service.send(child) {
            Ok(()) => {}
            Err(mpsc::SendError(child)) => {
                REAPER_FALLBACK_QUEUE
                    .get_or_init(|| Mutex::new(Vec::new()))
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(child);
                clear_reaper_service();
                restart_attempts += 1;
                if restart_attempts > 1 {
                    eprintln!(
                        "kanban {context} reaper service 已关闭；保留 Child ownership 等待后续重试"
                    );
                    return;
                }
                let Some(restarted) = reaper_service(context) else {
                    schedule_reaper_retry(context);
                    return;
                };
                service = restarted;
            }
        }
    }
}

#[cfg(test)]
fn reap_blocking(child: &mut OwnedChild) {
    let cleanup = force_stop(child);
    if let Some(error) = cleanup.first_error {
        eprintln!("kanban sidecar test cleanup kill 失败：{error}");
    }
    match child.process_mut().wait() {
        Ok(_) => {
            child.finish_drain();
            child.release_process();
        }
        Err(error) => panic!("test cleanup wait 失败：{error}"),
    }
}

#[cfg(unix)]
fn request_graceful_stop(child: &OwnedChild) -> io::Result<()> {
    let process_group = child.process_group.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "owned sidecar process group is unavailable",
        )
    })?;
    let leader = child.process().id() as libc::pid_t;
    if process_group <= 1 || process_group != leader {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "owned sidecar process group no longer matches its leader",
        ));
    }
    let result = unsafe { libc::kill(-process_group, libc::SIGINT) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn request_graceful_stop(_child: &OwnedChild) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Desktop host graceful shutdown 仅支持 Linux/Unix",
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Read, Write},
        net::{TcpListener, TcpStream},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
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
        let fixture = spawn_delayed_json_host(
            [
                serde_json::to_vec(&health).expect("health JSON"),
                serde_json::to_vec(&runtime).expect("runtime JSON"),
                serde_json::to_vec(&manifest).expect("manifest JSON"),
            ],
            Duration::from_millis(80),
        );
        let started_at = Instant::now();
        let error = probe_host_with_timeouts(
            fixture.endpoint,
            ProbeTimeouts {
                connect: Duration::from_millis(200),
                io: Duration::from_millis(200),
                total: Duration::from_millis(120),
            },
        )
        .expect_err("three delayed responses must exceed one shared deadline");
        let elapsed = started_at.elapsed();
        assert!(
            elapsed < Duration::from_millis(300),
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
        if cleanup.process_reaped {
            child.finish_drain();
            child.release_process();
        } else {
            reap_blocking(&mut child);
        }
        assert!(child.process.is_none(), "test sidecar must be reaped");
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
    fn startup_timeout_handoffs_owned_sidecar_to_reaper() {
        let _reaper_guard = reaper_test_guard();
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let leader_marker = std::env::temp_dir().join(format!(
            "kanban-desktop-startup-exit-marker-{}-{id}",
            std::process::id(),
        ));
        let descendant_marker = std::env::temp_dir().join(format!(
            "kanban-desktop-startup-descendant-marker-{}-{id}",
            std::process::id(),
        ));
        let go_marker = std::env::temp_dir().join(format!(
            "kanban-desktop-startup-go-marker-{}-{id}",
            std::process::id(),
        ));
        let sidecar = std::env::temp_dir().join(format!(
            "kanban-desktop-startup-exit-sidecar-{}-{id}",
            std::process::id(),
        ));
        for marker in [&leader_marker, &descendant_marker, &go_marker] {
            match std::fs::remove_file(marker) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => panic!("remove stale startup marker {}: {error}", marker.display()),
            }
        }
        let endpoint = TcpListener::bind((DEFAULT_HOST, 0))
            .expect("startup fixture endpoint")
            .local_addr()
            .expect("startup fixture address");
        let script = format!(
            "#!/bin/sh\nprintf '%s' $$ > {}\nwhile [ ! -f {} ]; do sleep 0.01; done\nsleep 10 &\nprintf '%s' $! > {}\nwait\n",
            leader_marker.display(),
            go_marker.display(),
            descendant_marker.display()
        );
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
        config.startup_timeout = Duration::from_millis(250);
        config = config.with_probe_timeout(Duration::from_millis(25));
        let launch = thread::spawn(move || connect_or_spawn(&config));
        let ready_deadline = Instant::now() + Duration::from_secs(1);
        while !leader_marker.exists() && Instant::now() < ready_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            leader_marker.exists(),
            "sidecar readiness marker was not written"
        );
        std::fs::write(&go_marker, b"go").expect("sidecar readiness release");
        let error = launch
            .join()
            .expect("startup launch thread")
            .expect_err("timed out sidecar must fail startup");
        assert!(matches!(error, HostStartupError::StartupTimeout { .. }));
        let read_pid = |path: &std::path::Path| {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                if let Ok(value) = std::fs::read_to_string(path)
                    && let Ok(pid) = value.parse::<i32>()
                {
                    return pid;
                }
                assert!(Instant::now() < deadline, "pid marker was not written");
                thread::sleep(Duration::from_millis(10));
            }
        };
        let leader_pid = read_pid(&leader_marker);
        let descendant_pid = read_pid(&descendant_marker);
        let reap_deadline = Instant::now() + Duration::from_secs(2);
        while (unsafe { libc::kill(leader_pid, 0) } == 0
            || unsafe { libc::kill(descendant_pid, 0) } == 0)
            && Instant::now() < reap_deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            unsafe { libc::kill(leader_pid, 0) } != 0,
            "sidecar leader pid remains alive"
        );
        assert!(
            unsafe { libc::kill(descendant_pid, 0) } != 0,
            "sidecar descendant pid remains alive"
        );
        let _ = std::fs::remove_file(leader_marker);
        let _ = std::fs::remove_file(descendant_marker);
        let _ = std::fs::remove_file(go_marker);
        let _ = std::fs::remove_file(sidecar);
    }

    #[cfg(unix)]
    #[test]
    fn cancelled_startup_reaps_sidecar_descendants_before_returning_control() {
        let _reaper_guard = reaper_test_guard();
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let leader_marker = std::env::temp_dir().join(format!(
            "kanban-desktop-cancel-leader-{}-{id}",
            std::process::id(),
        ));
        let descendant_marker = std::env::temp_dir().join(format!(
            "kanban-desktop-cancel-descendant-{}-{id}",
            std::process::id(),
        ));
        let sidecar = std::env::temp_dir().join(format!(
            "kanban-desktop-cancel-sidecar-{}-{id}",
            std::process::id(),
        ));
        for marker in [&leader_marker, &descendant_marker] {
            let _ = std::fs::remove_file(marker);
        }
        let endpoint = TcpListener::bind((DEFAULT_HOST, 0))
            .expect("cancel fixture endpoint")
            .local_addr()
            .expect("cancel fixture address");
        let script = format!(
            "#!/bin/sh\nprintf '%s' $$ > {}\nsleep 30 &\nprintf '%s' $! > {}\nwait\n",
            leader_marker.display(),
            descendant_marker.display()
        );
        std::fs::write(&sidecar, script).expect("cancel sidecar fixture");
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&sidecar)
                .expect("cancel sidecar metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&sidecar, permissions).expect("cancel sidecar executable");
        }

        let mut config = HostLaunchConfig::new(sidecar.clone(), "web", "db", "actor", "board");
        config.endpoint = endpoint;
        config.startup_timeout = Duration::from_secs(8);
        config = config.with_probe_timeout(Duration::from_millis(100));
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_for_thread = Arc::clone(&cancel);
        let launch =
            thread::spawn(move || connect_or_spawn_with_cancel(&config, &cancel_for_thread));
        let marker_deadline = Instant::now() + Duration::from_secs(1);
        while !leader_marker.exists() && Instant::now() < marker_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(leader_marker.exists(), "cancel sidecar did not start");
        let started_at = Instant::now();
        cancel.store(true, Ordering::Release);
        let error = launch
            .join()
            .expect("cancel launch thread")
            .expect_err("cancelled startup must fail closed");
        assert!(matches!(error, HostStartupError::Cancelled));
        assert!(
            started_at.elapsed() < Duration::from_millis(500),
            "cancelled startup blocked for {:?}",
            started_at.elapsed()
        );
        let read_pid = |path: &std::path::Path| {
            let deadline = Instant::now() + Duration::from_secs(1);
            loop {
                if let Ok(value) = std::fs::read_to_string(path)
                    && let Ok(pid) = value.parse::<i32>()
                {
                    return pid;
                }
                assert!(
                    Instant::now() < deadline,
                    "cancel pid marker was not written"
                );
                thread::sleep(Duration::from_millis(10));
            }
        };
        let leader_pid = read_pid(&leader_marker);
        let descendant_pid = read_pid(&descendant_marker);
        let reap_deadline = Instant::now() + Duration::from_secs(2);
        while (unsafe { libc::kill(leader_pid, 0) } == 0
            || unsafe { libc::kill(descendant_pid, 0) } == 0)
            && Instant::now() < reap_deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            unsafe { libc::kill(leader_pid, 0) } != 0,
            "cancelled leader remains alive"
        );
        assert!(
            unsafe { libc::kill(descendant_pid, 0) } != 0,
            "cancelled descendant remains alive"
        );
        let _ = std::fs::remove_file(leader_marker);
        let _ = std::fs::remove_file(descendant_marker);
        let _ = std::fs::remove_file(sidecar);
    }

    #[cfg(unix)]
    #[test]
    fn nonblocking_exit_cleanup_retries_until_descendant_group_is_gone() {
        let id = NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
        let descendant_marker = std::env::temp_dir().join(format!(
            "kanban-desktop-exit-descendant-{}-{id}",
            std::process::id(),
        ));
        let sidecar = std::env::temp_dir().join(format!(
            "kanban-desktop-exit-sidecar-{}-{id}",
            std::process::id(),
        ));
        let _ = std::fs::remove_file(&descendant_marker);
        let script = format!(
            "#!/bin/sh\nsleep 30 &\nprintf '%s' $! > {}\nwait\n",
            descendant_marker.display()
        );
        std::fs::write(&sidecar, script).expect("exit sidecar fixture");
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&sidecar)
                .expect("exit sidecar metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&sidecar, permissions).expect("exit sidecar executable");
        }
        let mut command = Command::new(&sidecar);
        command.stderr(Stdio::piped());
        configure_owned_process_group(&mut command);
        let process = command.spawn().expect("exit sidecar process");
        let marker_deadline = Instant::now() + Duration::from_secs(1);
        while !descendant_marker.exists() && Instant::now() < marker_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(descendant_marker.exists(), "exit descendant did not start");
        let mut handle = HostHandle {
            ownership: HostOwnership::Owned,
            child: Some(test_owned_child(process)),
        };
        let started_at = Instant::now();
        let result = loop {
            match handle.shutdown_nonblocking() {
                Ok(result) => break result,
                Err(error) => {
                    assert!(
                        started_at.elapsed() < Duration::from_secs(2),
                        "nonblocking exit cleanup did not converge: {error}"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        assert!(matches!(
            result,
            ShutdownResult::Forced | ShutdownResult::AlreadyExited
        ));
        let descendant_pid = std::fs::read_to_string(&descendant_marker)
            .expect("exit descendant pid")
            .parse::<i32>()
            .expect("exit descendant pid integer");
        let reap_deadline = Instant::now() + Duration::from_secs(1);
        while unsafe { libc::kill(descendant_pid, 0) } == 0 && Instant::now() < reap_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            unsafe { libc::kill(descendant_pid, 0) } != 0,
            "exit descendant remains alive"
        );
        let _ = std::fs::remove_file(descendant_marker);
        let _ = std::fs::remove_file(sidecar);
    }

    #[test]
    fn external_host_shutdown_is_a_noop() {
        let mut handle = HostHandle {
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
        let _reaper_guard = reaper_test_guard();
        let process = test_sleep_child(true);
        let mut handle = HostHandle {
            ownership: HostOwnership::Owned,
            child: Some(test_owned_child(process)),
        };
        let pid = handle.child.as_ref().expect("owned child").process().id();
        match handle.shutdown() {
            Ok(result) => {
                assert!(matches!(
                    result,
                    ShutdownResult::Forced | ShutdownResult::AlreadyExited
                ));
                assert!(handle.child.is_none(), "forced child must be reaped");
            }
            Err(_) => {
                assert!(
                    handle.child.is_some(),
                    "unconfirmed cleanup retains ownership"
                );
                drop(handle);
                let reap_deadline = Instant::now() + Duration::from_secs(2);
                while unsafe { libc::kill(pid as libc::pid_t, 0) } == 0
                    && Instant::now() < reap_deadline
                {
                    thread::sleep(Duration::from_millis(10));
                }
                assert!(
                    unsafe { libc::kill(pid as libc::pid_t, 0) } != 0,
                    "background reaper did not reap sidecar"
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn owned_host_drop_handoffs_unconfirmed_child_to_reaper() {
        let _reaper_guard = reaper_test_guard();
        let process = test_sleep_child(true);
        let handle = HostHandle {
            ownership: HostOwnership::Owned,
            child: Some(test_owned_child(process)),
        };
        let pid = handle.child.as_ref().expect("owned child").process().id();
        drop(handle);

        let reap_deadline = Instant::now() + Duration::from_secs(2);
        while unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 && Instant::now() < reap_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            unsafe { libc::kill(pid as libc::pid_t, 0) } != 0,
            "background reaper did not reap dropped sidecar"
        );
    }

    #[cfg(unix)]
    #[test]
    fn reaper_spawn_failure_retains_child_until_service_recovers() {
        let _reaper_guard = reaper_test_guard();
        let process = test_sleep_child(true);
        let mut child = test_owned_child(process);
        let pid = child.process().id() as libc::pid_t;
        let detached = child.detach_for_reaper().expect("reaper ownership");
        FORCE_REAPER_SPAWN_FAILURE.store(true, Ordering::Release);
        let started_at = Instant::now();
        handoff_to_reaper(detached, "injected reaper spawn failure");
        FORCE_REAPER_SPAWN_FAILURE.store(false, Ordering::Release);
        assert!(
            started_at.elapsed() < Duration::from_millis(100),
            "spawn-failure handoff must not block the caller"
        );

        let service = reaper_service("reaper recovery test").expect("reaper restart");
        flush_pending_reapers(service, "reaper recovery test");
        let reap_deadline = Instant::now() + Duration::from_secs(2);
        while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < reap_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            unsafe { libc::kill(pid, 0) } != 0,
            "reaper recovery did not consume retained child"
        );
    }

    #[cfg(unix)]
    #[test]
    fn reaper_pending_count_stays_until_process_group_is_confirmed_gone() {
        let _reaper_guard = reaper_test_guard();
        let process = test_sleep_child(true);
        let mut child = test_owned_child(process);
        let pid = child.process().id() as libc::pid_t;
        let before = reaper_pending_count();
        let detached = child.detach_for_reaper().expect("reaper ownership");
        handoff_to_reaper(detached, "pending count regression");
        assert!(
            reaper_pending_count() > before,
            "handoff must remain visible to exit cleanup"
        );
        let service = reaper_service("pending count regression").expect("reaper service");
        flush_pending_reapers(service, "pending count regression");
        let deadline = Instant::now() + Duration::from_secs(2);
        while (unsafe { libc::kill(pid, 0) } == 0 || reaper_pending_count() > before)
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            unsafe { libc::kill(pid, 0) } != 0,
            "reaper did not reap child"
        );
        assert!(
            reaper_pending_count() <= before,
            "reaper pending count did not clear after confirmed cleanup"
        );
    }

    #[cfg(unix)]
    #[test]
    fn closed_reaper_restart_failure_still_schedules_retry_pump() {
        let _reaper_guard = reaper_test_guard();
        let process = test_sleep_child(true);
        let mut child = test_owned_child(process);
        let pid = child.process().id() as libc::pid_t;
        let detached = child.detach_for_reaper().expect("reaper ownership");

        let (closed_sender, receiver) = mpsc::channel();
        drop(receiver);
        *REAPER_SERVICE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(closed_sender.clone());
        FORCE_REAPER_SERVICE_SPAWN_FAILURE.store(true, Ordering::Release);
        handoff_to_reaper(detached, "closed reaper restart failure");
        FORCE_REAPER_SERVICE_SPAWN_FAILURE.store(false, Ordering::Release);

        let reap_deadline = Instant::now() + Duration::from_secs(2);
        while unsafe { libc::kill(pid, 0) } == 0 && Instant::now() < reap_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            unsafe { libc::kill(pid, 0) } != 0,
            "retry pump did not drain child after closed sender restart failure"
        );
    }

    #[cfg(unix)]
    #[test]
    fn external_host_shutdown_never_kills_process() {
        let process = test_sleep_child(false);
        let mut handle = HostHandle {
            ownership: HostOwnership::External,
            child: Some(test_owned_child(process)),
        };
        assert_eq!(
            handle.shutdown().expect("external shutdown"),
            ShutdownResult::ExternalHostKept
        );
        let child = handle.child.as_mut().expect("external process retained");
        assert!(
            child
                .process_mut()
                .try_wait()
                .expect("external status")
                .is_none()
        );
        child
            .process_mut()
            .kill()
            .expect("cleanup external fixture");
        child.process_mut().wait().expect("wait external fixture");
        child.finish_drain();
    }

    #[cfg(unix)]
    #[test]
    fn blocking_reaper_reaps_child_before_releasing_drain() {
        let process = test_sleep_child(true);
        let mut child = test_owned_child(process);
        reap_blocking(&mut child);
        assert!(
            child.process.is_none(),
            "blocking reaper must release child"
        );
        assert!(child.drain.is_none(), "reaper must join stderr drain");
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_falls_back_to_leader_when_owned_group_disappeared() {
        let process = test_sleep_child_detached_from_owned_group();
        let mut child = test_owned_child(process);
        let process_group = child.process_group.expect("owned process group");
        assert_eq!(
            unsafe { libc::kill(-process_group, 0) },
            -1,
            "detached fixture must leave its original process group"
        );
        let deadline = Instant::now() + Duration::from_secs(1);
        let cleanup = loop {
            let cleanup = force_stop(&mut child);
            if cleanup.ownership() == CleanupOwnership::Release || Instant::now() >= deadline {
                break cleanup;
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(cleanup.ownership(), CleanupOwnership::Release);
        child.finish_drain();
        child.release_process();
        assert!(child.process.is_none(), "leader fallback must reap child");
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
        kill_calls: usize,
    }

    #[cfg(unix)]
    impl FakeCleanupProcess {
        fn new(
            try_wait_results: impl IntoIterator<Item = FakeTryWait>,
            kill_results: impl IntoIterator<Item = FakeResult>,
        ) -> Self {
            Self {
                try_wait_results: try_wait_results.into_iter().collect(),
                kill_results: kill_results.into_iter().collect(),
                kill_calls: 0,
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
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_reaps_already_exited_without_kill_or_wait() {
        let mut process = FakeCleanupProcess::new([FakeTryWait::Reaped], []);
        let outcome = force_stop_with(&mut process);
        assert!(outcome.process_reaped);
        assert!(outcome.first_error.is_none());
        assert_eq!(process.kill_calls, 0);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_kills_and_rechecks_running_process() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Reaped],
            [FakeResult::Ok],
        );
        let outcome = force_stop_with(&mut process);
        assert!(outcome.process_reaped);
        assert_eq!(process.kill_calls, 1);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_try_wait_error_does_not_confirm_exit_or_join_drain() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Error("try_wait failed")],
            [FakeResult::Ok],
        );
        let outcome = force_stop_with(&mut process);
        assert!(!outcome.process_reaped);
        assert!(!outcome.should_join_drain());
        assert_eq!(
            outcome.first_error.expect("try_wait error").to_string(),
            "try_wait failed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_kill_error_does_not_call_blocking_wait() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Running],
            [FakeResult::Error("kill failed")],
        );
        let outcome = force_stop_with(&mut process);
        assert!(!outcome.process_reaped);
        assert_eq!(
            outcome.first_error.expect("kill error").to_string(),
            "kill failed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_kill_error_can_confirm_exit_with_nonblocking_retry() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Reaped],
            [FakeResult::Error("kill failed")],
        );
        let outcome = force_stop_with(&mut process);
        assert!(outcome.process_reaped);
        assert_eq!(
            outcome.first_error.expect("kill error").to_string(),
            "kill failed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_try_wait_error_is_preserved_as_first_error() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Error("try_wait failed"), FakeTryWait::Running],
            [FakeResult::Error("kill failed")],
        );
        let outcome = force_stop_with(&mut process);
        assert!(!outcome.process_reaped);
        assert_eq!(
            outcome.first_error.expect("try_wait error").to_string(),
            "try_wait failed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_recheck_error_is_preserved_after_successful_kill() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Error("try_wait failed")],
            [FakeResult::Ok],
        );
        let outcome = force_stop_with(&mut process);
        assert_eq!(
            outcome.first_error.expect("try_wait error").to_string(),
            "try_wait failed"
        );
        assert!(!outcome.process_reaped);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_reaped_outcome_releases_child_ownership() {
        let mut process = FakeCleanupProcess::new([FakeTryWait::Reaped], []);
        let outcome = force_stop_with(&mut process);
        assert_eq!(outcome.ownership(), CleanupOwnership::Release);
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_unconfirmed_outcome_retains_child_ownership_for_retry() {
        let mut process = FakeCleanupProcess::new(
            [FakeTryWait::Running, FakeTryWait::Running],
            [FakeResult::Error("kill failed")],
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
                FakeTryWait::Reaped,
            ],
            [FakeResult::Error("kill failed")],
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
            [FakeTryWait::Running, FakeTryWait::Running],
            [FakeResult::Ok],
        );
        let running_outcome = force_stop_with(&mut running);
        assert!(!running_outcome.should_join_drain());

        let mut reaped = FakeCleanupProcess::new([FakeTryWait::Reaped], []);
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
        let (drain_done_tx, drain_done_rx) = mpsc::channel();
        let drain = thread::spawn(move || {
            drain_stderr(stderr, diagnostics_for_thread);
            let _ = drain_done_tx.send(());
        });
        let process_group = process_group_for(&process);
        OwnedChild {
            process: Some(process),
            process_group,
            group_signal_sent: false,
            stderr: diagnostics,
            drain: Some(DrainHandle {
                join: drain,
                done: drain_done_rx,
            }),
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
        configure_owned_process_group(&mut command);
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

    #[cfg(unix)]
    fn test_sleep_child_detached_from_owned_group() -> Child {
        let mut command = Command::new("sleep");
        command
            .arg("10")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        configure_owned_process_group(&mut command);
        unsafe {
            command.pre_exec(|| {
                let parent_group = libc::getpgid(libc::getppid());
                if parent_group < 0 {
                    return Err(io::Error::last_os_error());
                }
                if libc::setpgid(0, parent_group) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command.spawn().expect("detached sleep fixture")
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

    struct DelayedJsonHost {
        endpoint: SocketAddr,
        stop: Arc<AtomicBool>,
        join: Option<thread::JoinHandle<()>>,
    }

    impl Drop for DelayedJsonHost {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            // 唤醒 fixture 可能正在等待的下一次 accept，使 join 有界结束。
            let _ = TcpStream::connect(self.endpoint);
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    fn spawn_delayed_json_host<const N: usize>(
        bodies: [Vec<u8>; N],
        delay: Duration,
    ) -> DelayedJsonHost {
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
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let join = thread::spawn(move || {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                if stop_for_thread.load(Ordering::Acquire) {
                    return;
                }
                let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
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
                if stop_for_thread.load(Ordering::Acquire) {
                    return;
                }
                let _ = stream.write_all(&response);
            }
        });
        DelayedJsonHost {
            endpoint,
            stop,
            join: Some(join),
        }
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
