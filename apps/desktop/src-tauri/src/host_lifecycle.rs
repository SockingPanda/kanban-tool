use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
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
        if runtime.api_base_url != "" {
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
pub fn probe_host(endpoint: SocketAddr) -> Result<HostCompatibility, ProbeError> {
    let health = get_json::<HealthResponse>(endpoint, "/health")?;
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

    let runtime = get_json::<WebRuntimeConfig>(endpoint, "/app/runtime.json")?;
    let manifest = get_json::<WebArtifactManifest>(endpoint, "/app/manifest.json")?;
    HostCompatibility::verify(runtime, manifest, env!("CARGO_PKG_VERSION"))
        .map_err(|error| ProbeError::incompatible(error.to_string()))
}

fn get_json<T>(endpoint: SocketAddr, path: &str) -> Result<T, ProbeError>
where
    T: serde::de::DeserializeOwned,
{
    let response = get(endpoint, path)?;
    if response.status != 200 {
        return Err(ProbeError::incompatible(format!(
            "host {path} 返回 HTTP {}，需要 200",
            response.status
        )));
    }
    serde_json::from_slice(&response.body)
        .map_err(|error| ProbeError::incompatible(format!("host {path} JSON 无法解析: {error}")))
}

fn get(endpoint: SocketAddr, path: &str) -> Result<HttpResponse, ProbeError> {
    let mut stream =
        TcpStream::connect_timeout(&endpoint, HTTP_CONNECT_TIMEOUT).map_err(|error| {
            ProbeError::unavailable(format!(
                "无法连接固定 host http://{endpoint}{path}: {error}"
            ))
        })?;
    stream
        .set_read_timeout(Some(HTTP_READ_TIMEOUT))
        .map_err(|error| ProbeError::unavailable(format!("设置 host 读取超时失败: {error}")))?;
    stream
        .set_write_timeout(Some(HTTP_READ_TIMEOUT))
        .map_err(|error| ProbeError::unavailable(format!("设置 host 写入超时失败: {error}")))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {endpoint}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| ProbeError::unavailable(format!("发送 host probe 请求失败: {error}")))?;
    read_http_response(&mut stream).map_err(|error| ProbeError::unavailable(error.to_string()))
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

fn read_http_response(stream: &mut TcpStream) -> io::Result<HttpResponse> {
    let mut bytes = Vec::new();
    let header_end = loop {
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
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "host probe 状态行无效"))?;
    let content_length = lines.find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    });

    let mut body = bytes.split_off(header_end);
    if let Some(content_length) = content_length {
        if content_length > MAX_HTTP_RESPONSE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "host probe body 过大",
            ));
        }
        while body.len() < content_length {
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
    } else {
        stream.read_to_end(&mut body)?;
    }
    Ok(HttpResponse { status, body })
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
        }
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
    child: Option<Child>,
}

impl HostHandle {
    pub fn app_url(&self) -> String {
        format!("http://{}/app/", self.endpoint)
    }

    pub fn shutdown(&mut self) -> Result<ShutdownResult, ShutdownError> {
        if self.ownership == HostOwnership::External {
            return Ok(ShutdownResult::ExternalHostKept);
        }
        let Some(child) = self.child.as_mut() else {
            return Ok(ShutdownResult::AlreadyExited);
        };
        if child
            .try_wait()
            .map_err(|error| ShutdownError::Io(error.to_string()))?
            .is_some()
        {
            self.child = None;
            return Ok(ShutdownResult::AlreadyExited);
        }

        if let Err(error) = request_graceful_stop(child) {
            let _ = child.kill();
            let _ = child.wait();
            self.child = None;
            return Err(ShutdownError::GracefulRequest(error.to_string()));
        }
        let deadline = Instant::now() + GRACEFUL_SHUTDOWN_TIMEOUT;
        loop {
            if child
                .try_wait()
                .map_err(|error| ShutdownError::Io(error.to_string()))?
                .is_some()
            {
                self.child = None;
                return Ok(ShutdownResult::Graceful);
            }
            if Instant::now() >= deadline {
                child
                    .kill()
                    .map_err(|error| ShutdownError::Io(error.to_string()))?;
                child
                    .wait()
                    .map_err(|error| ShutdownError::Io(error.to_string()))?;
                self.child = None;
                return Ok(ShutdownResult::Forced);
            }
            thread::sleep(HOST_POLL_INTERVAL);
        }
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
    match probe_host(config.endpoint) {
        Ok(_) => {
            return Ok(HostHandle {
                endpoint: config.endpoint,
                ownership: HostOwnership::External,
                child: None,
            });
        }
        Err(initial_probe) => {
            let mut child = spawn_sidecar(config)?;
            let result = wait_for_sidecar(config, &mut child, initial_probe);
            match result {
                Ok(_) => Ok(HostHandle {
                    endpoint: config.endpoint,
                    ownership: HostOwnership::Owned,
                    child: Some(child),
                }),
                Err(error) => {
                    let _ = force_stop(&mut child);
                    Err(error)
                }
            }
        }
    }
}

fn spawn_sidecar(config: &HostLaunchConfig) -> Result<Child, HostStartupError> {
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
    command
        .spawn()
        .map_err(|error| HostStartupError::SidecarSpawn {
            path: config.sidecar_path.clone(),
            message: error.to_string(),
        })
}

fn wait_for_sidecar(
    config: &HostLaunchConfig,
    child: &mut Child,
    initial_probe: ProbeError,
) -> Result<HostCompatibility, HostStartupError> {
    let deadline = Instant::now() + config.startup_timeout;
    loop {
        let latest_error = match probe_host(config.endpoint) {
            Ok(compatibility) => return Ok(compatibility),
            Err(error) => error,
        };
        if let Some(status) = child
            .try_wait()
            .map_err(|error| HostStartupError::SidecarExited {
                message: error.to_string(),
            })?
        {
            let diagnostic = child_diagnostic(child, status);
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
        thread::sleep(HOST_POLL_INTERVAL);
    }
}

fn child_diagnostic(child: &mut Child, status: std::process::ExitStatus) -> String {
    let mut output = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let mut bytes = Vec::new();
        let _ = stderr.by_ref().take(16 * 1024).read_to_end(&mut bytes);
        output = String::from_utf8_lossy(&bytes).trim().to_owned();
    }
    if output.is_empty() {
        format!("exit status {status}")
    } else {
        format!("exit status {status}: {output}")
    }
}

fn force_stop(child: &mut Child) -> io::Result<()> {
    if child.try_wait()?.is_none() {
        child.kill()?;
    }
    let _ = child.wait();
    Ok(())
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
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use kanban_protocol::{
        HealthReport, HealthResponse, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
        WebArtifactFile, web_artifact_build_id_for,
    };

    use super::*;

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

    fn spawn_json_host<const N: usize>(responses: [Vec<u8>; N]) -> std::net::SocketAddr {
        let listener = TcpListener::bind((DEFAULT_HOST, 0)).expect("probe fixture listener");
        let endpoint = listener.local_addr().expect("fixture address");
        thread::spawn(move || {
            for body in responses {
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
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .expect("fixture response headers");
                stream.write_all(&body).expect("fixture response body");
            }
        });
        endpoint
    }
}
