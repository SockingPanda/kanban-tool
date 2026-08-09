use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

#[cfg(target_os = "linux")]
use std::process::Command;

use serde::Serialize;
use tauri::{Manager, State};

use super::{
    bootstrap_state::{
        AttemptKind, BootstrapDiagnostic, BootstrapPhase, BootstrapSnapshot, DiagnosticKind,
        GenerationFence,
    },
    host_lifecycle::{
        DEFAULT_HOST, DEFAULT_PORT, HostHandle, HostLaunchConfig, ProbeError, connect_or_spawn,
        probe_host,
    },
};

pub(crate) const FIXED_APP_URL: &str = "http://127.0.0.1:8721/app/";

/// 在进入 Tauri navigation 或浏览器启动前完成一次固定 URL 校验。
#[derive(Clone)]
pub(crate) struct ValidatedAppUrl(tauri::Url);

impl ValidatedAppUrl {
    pub(crate) fn fixed() -> Result<Self, BootstrapDiagnostic> {
        let url = tauri::Url::parse(FIXED_APP_URL).map_err(|error| {
            BootstrapDiagnostic::new(
                DiagnosticKind::Configuration,
                format!("固定 Web URL 无法解析: {error}"),
            )
        })?;
        if url.scheme() != "http"
            || url.host_str() != Some(DEFAULT_HOST)
            || url.port() != Some(DEFAULT_PORT)
            || url.path() != "/app/"
        {
            return Err(BootstrapDiagnostic::new(
                DiagnosticKind::Configuration,
                "固定 Web URL 必须是 http://127.0.0.1:8721/app/",
            ));
        }
        Ok(Self(url))
    }

    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

struct BootstrapWorker {
    cancel: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Drop for BootstrapWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        let Some(join) = self.join.take() else {
            return;
        };
        debug_assert_ne!(join.thread().id(), thread::current().id());
        let _ = join.join();
    }
}

struct RuntimeState {
    fence: GenerationFence,
    phase: BootstrapPhase,
    diagnostic: Option<BootstrapDiagnostic>,
    setup_diagnostic: Option<BootstrapDiagnostic>,
    external_ready: bool,
    navigation_claimed: Option<u64>,
}

/// Desktop 对 host 的生命周期、bootstrap 状态和后台 worker 的唯一 owner。
pub(crate) struct DesktopHost {
    /// Desktop 进程唯一拥有的 host；外部 host 只保留 non-owning handle。
    handle: Mutex<Option<HostHandle>>,
    runtime: Mutex<RuntimeState>,
    worker: Mutex<Option<BootstrapWorker>>,
    config: Option<HostLaunchConfig>,
    app_url: Option<ValidatedAppUrl>,
}

impl DesktopHost {
    #[cfg(test)]
    pub(crate) fn new(config: HostLaunchConfig) -> Self {
        Self::from_setup(Some(config), ValidatedAppUrl::fixed().ok(), None)
    }

    pub(crate) fn from_setup(
        config: Option<HostLaunchConfig>,
        app_url: Option<ValidatedAppUrl>,
        setup_diagnostic: Option<BootstrapDiagnostic>,
    ) -> Self {
        let phase = if setup_diagnostic.is_some() {
            BootstrapPhase::Recovery
        } else {
            BootstrapPhase::StartingLocal
        };
        Self {
            handle: Mutex::new(None),
            runtime: Mutex::new(RuntimeState {
                fence: GenerationFence::default(),
                phase,
                diagnostic: setup_diagnostic.clone(),
                setup_diagnostic,
                external_ready: false,
                navigation_claimed: None,
            }),
            worker: Mutex::new(None),
            config,
            app_url,
        }
    }

    fn config(&self) -> Option<HostLaunchConfig> {
        self.config.clone()
    }

    fn begin(&self, kind: AttemptKind) -> Option<u64> {
        self.reap_finished_worker();
        let mut runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        if self
            .handle
            .lock()
            .expect("桌面 host 生命周期锁已失效")
            .is_some()
        {
            return None;
        }
        let generation = runtime.fence.begin(kind)?;
        runtime.phase = match kind {
            AttemptKind::ProbeExisting => BootstrapPhase::ProbingExisting,
            AttemptKind::StartLocal => BootstrapPhase::StartingLocal,
        };
        runtime.diagnostic = runtime.setup_diagnostic.clone();
        runtime.navigation_claimed = None;
        Some(generation)
    }

    fn finish_probe(&self, generation: u64, result: Result<(), BootstrapDiagnostic>) -> bool {
        let mut runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        if !runtime.fence.finish(generation, AttemptKind::ProbeExisting) {
            return false;
        }
        if let Some(diagnostic) = runtime.setup_diagnostic.clone() {
            runtime.phase = BootstrapPhase::Recovery;
            runtime.diagnostic = Some(diagnostic);
            runtime.external_ready = false;
            return true;
        }
        match result {
            Ok(()) => {
                runtime.phase = BootstrapPhase::Ready;
                runtime.diagnostic = None;
                runtime.external_ready = true;
            }
            Err(diagnostic) => {
                runtime.phase = BootstrapPhase::Recovery;
                runtime.diagnostic = Some(diagnostic);
                runtime.external_ready = false;
            }
        }
        true
    }

    fn finish_start(
        &self,
        generation: u64,
        result: Result<HostHandle, BootstrapDiagnostic>,
    ) -> bool {
        let mut runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        if !runtime.fence.finish(generation, AttemptKind::StartLocal) {
            return false;
        }
        match result {
            Ok(handle) => {
                // HostHandle 自身验证过生命周期；导航只接受一次已校验的固定 URL。
                self.handle
                    .lock()
                    .expect("桌面 host 生命周期锁已失效")
                    .replace(handle);
                if let Some(diagnostic) = runtime.setup_diagnostic.clone() {
                    runtime.phase = BootstrapPhase::Recovery;
                    runtime.diagnostic = Some(diagnostic);
                    runtime.external_ready = false;
                } else {
                    runtime.phase = BootstrapPhase::Ready;
                    runtime.diagnostic = None;
                    runtime.external_ready = true;
                }
            }
            Err(diagnostic) => {
                runtime.phase = BootstrapPhase::Recovery;
                runtime.diagnostic = Some(runtime.setup_diagnostic.clone().map_or(
                    diagnostic.clone(),
                    |setup| {
                        BootstrapDiagnostic::new(
                            setup.kind,
                            format!("{}；host bootstrap: {}", setup.message, diagnostic.message),
                        )
                    },
                ));
                runtime.external_ready = false;
            }
        }
        true
    }

    fn claim_navigation(&self, generation: u64) -> Option<ValidatedAppUrl> {
        let mut runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        if runtime.phase != BootstrapPhase::Ready
            || runtime.navigation_claimed.is_some()
            || !runtime.fence.is_current(generation)
        {
            return None;
        }
        let app_url = self.app_url.clone()?;
        runtime.navigation_claimed = Some(generation);
        Some(app_url)
    }

    pub(crate) fn cancel_in_flight(&self) {
        let in_flight = self
            .runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .fence
            .in_flight()
            .is_some();
        if in_flight {
            let mut runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
            runtime.fence.fence();
            runtime.phase = BootstrapPhase::Recovery;
            runtime.diagnostic = Some(BootstrapDiagnostic::new(
                DiagnosticKind::Internal,
                "桌面 bootstrap 已取消",
            ));
            runtime.navigation_claimed = None;
        }
        self.cancel_and_join_worker();
    }

    fn close(&self) {
        let mut runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        runtime.fence.close();
        runtime.navigation_claimed = None;
    }

    pub(crate) fn record_setup_diagnostic(&self, diagnostic: BootstrapDiagnostic) {
        let mut runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        let diagnostic = match runtime.setup_diagnostic.take() {
            None => diagnostic,
            Some(previous) => BootstrapDiagnostic::new(
                diagnostic.kind,
                format!("{}；{}", previous.message, diagnostic.message),
            ),
        };
        runtime.setup_diagnostic = Some(diagnostic.clone());
        runtime.diagnostic = Some(diagnostic);
        runtime.phase = BootstrapPhase::Recovery;
        runtime.external_ready = false;
    }

    pub(crate) fn snapshot(&self) -> BootstrapSnapshot {
        let runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        let has_host = self
            .handle
            .lock()
            .expect("桌面 host 生命周期锁已失效")
            .is_some();
        BootstrapSnapshot {
            phase: runtime.phase,
            endpoint: self
                .config
                .as_ref()
                .map(|config| config.endpoint.to_string())
                .unwrap_or_else(|| format!("{DEFAULT_HOST}:{DEFAULT_PORT}")),
            app_url: FIXED_APP_URL.to_owned(),
            diagnostic: runtime.diagnostic.clone(),
            can_open_browser: cfg!(target_os = "linux")
                && runtime.phase == BootstrapPhase::Ready
                && self.app_url.is_some()
                && (has_host || runtime.external_ready),
            generation: runtime.fence.generation(),
            in_flight: runtime.fence.in_flight().is_some(),
            closed: runtime.fence.is_closed(),
        }
    }

    pub(crate) fn diagnostics(&self) -> Option<BootstrapDiagnostic> {
        self.runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .diagnostic
            .clone()
    }

    pub(crate) fn shutdown(
        &self,
    ) -> Result<Option<super::host_lifecycle::ShutdownResult>, super::host_lifecycle::ShutdownError>
    {
        self.close();
        self.cancel_and_join_worker();
        self.shutdown_host_only()
    }

    fn shutdown_host_only(
        &self,
    ) -> Result<Option<super::host_lifecycle::ShutdownResult>, super::host_lifecycle::ShutdownError>
    {
        let Some(mut handle) = self
            .handle
            .lock()
            .expect("桌面 host 生命周期锁已失效")
            .take()
        else {
            return Ok(None);
        };
        handle.shutdown().map(Some)
    }

    fn abort_after_navigation_failure(&self) {
        self.close();
        if let Err(error) = self.shutdown_host_only() {
            eprintln!("kanban 导航失败后的 host cleanup 失败：{error}");
        }
    }

    fn install_worker(&self, cancel: Arc<AtomicBool>, join: JoinHandle<()>, generation: u64) {
        let keep = self
            .runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .fence
            .is_current(generation);
        if keep {
            self.worker
                .lock()
                .expect("桌面 bootstrap worker 锁已失效")
                .replace(BootstrapWorker {
                    cancel,
                    join: Some(join),
                });
        } else {
            cancel.store(true, Ordering::Release);
            let _ = join.join();
        }
    }

    fn reap_finished_worker(&self) {
        let join = {
            let mut worker = self.worker.lock().expect("桌面 bootstrap worker 锁已失效");
            let finished = worker
                .as_ref()
                .and_then(|worker| worker.join.as_ref())
                .map(|join| join.is_finished())
                .unwrap_or(false);
            if finished {
                worker.take().and_then(|mut worker| worker.join.take())
            } else {
                None
            }
        };
        if let Some(join) = join {
            let _ = join.join();
        }
    }

    fn cancel_and_join_worker(&self) {
        let worker = self
            .worker
            .lock()
            .expect("桌面 bootstrap worker 锁已失效")
            .take();
        let Some(mut worker) = worker else {
            return;
        };
        worker.cancel.store(true, Ordering::Release);
        if let Some(join) = worker.join.take() {
            let _ = join.join();
        }
    }
}

pub(crate) fn start_background_attempt(
    app: tauri::AppHandle,
    kind: AttemptKind,
) -> BootstrapSnapshot {
    let Some(state) = app.try_state::<DesktopHost>() else {
        return fallback_snapshot("桌面 bootstrap state 尚未初始化");
    };
    let Some(generation) = state.begin(kind) else {
        return state.snapshot();
    };
    let Some(config) = state.config() else {
        let diagnostic =
            BootstrapDiagnostic::new(DiagnosticKind::Configuration, "桌面 host 配置不可用");
        match kind {
            AttemptKind::ProbeExisting => {
                state.finish_probe(generation, Err(diagnostic));
            }
            AttemptKind::StartLocal => {
                state.finish_start(generation, Err(diagnostic));
            }
        }
        return state.snapshot();
    };

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_thread = Arc::clone(&cancel);
    let app_for_thread = app.clone();
    let spawn_result = thread::Builder::new()
        .name("kanban-desktop-bootstrap".to_owned())
        .spawn(move || match kind {
            AttemptKind::ProbeExisting => {
                let result = if cancel_for_thread.load(Ordering::Acquire) {
                    Err(BootstrapDiagnostic::new(
                        DiagnosticKind::Internal,
                        "桌面 bootstrap 已取消",
                    ))
                } else {
                    match probe_host(config.endpoint) {
                        Ok(_) => Ok(()),
                        Err(error) => Err(diagnostic_from_probe(error)),
                    }
                };
                complete_probe_attempt(&app_for_thread, generation, result);
            }
            AttemptKind::StartLocal => {
                let result = if cancel_for_thread.load(Ordering::Acquire) {
                    Err(BootstrapDiagnostic::new(
                        DiagnosticKind::Internal,
                        "桌面 bootstrap 已取消",
                    ))
                } else {
                    connect_or_spawn(&config).map_err(diagnostic_from_start)
                };
                complete_start_attempt(&app_for_thread, generation, result);
            }
        });
    let join = match spawn_result {
        Ok(join) => join,
        Err(error) => {
            let diagnostic = BootstrapDiagnostic::new(
                DiagnosticKind::Internal,
                format!("启动 bootstrap 后台线程失败: {error}"),
            );
            match kind {
                AttemptKind::ProbeExisting => {
                    state.finish_probe(generation, Err(diagnostic));
                }
                AttemptKind::StartLocal => {
                    state.finish_start(generation, Err(diagnostic));
                }
            }
            return state.snapshot();
        }
    };
    state.install_worker(cancel, join, generation);
    state.snapshot()
}

fn complete_probe_attempt(
    app: &tauri::AppHandle,
    generation: u64,
    result: Result<(), BootstrapDiagnostic>,
) {
    let Some(state) = app.try_state::<DesktopHost>() else {
        return;
    };
    if state.finish_probe(generation, result)
        && let Some(url) = state.claim_navigation(generation)
    {
        navigate_to_fixed_app(app, &state, url);
    }
}

fn complete_start_attempt(
    app: &tauri::AppHandle,
    generation: u64,
    result: Result<HostHandle, BootstrapDiagnostic>,
) {
    let Some(state) = app.try_state::<DesktopHost>() else {
        return;
    };
    if state.finish_start(generation, result)
        && let Some(url) = state.claim_navigation(generation)
    {
        navigate_to_fixed_app(app, &state, url);
    }
}

fn navigate_to_fixed_app(app: &tauri::AppHandle, state: &DesktopHost, url: ValidatedAppUrl) {
    let Some(window) = app.get_webview_window("main") else {
        state.abort_after_navigation_failure();
        return;
    };
    if window.navigate(url.0).is_err() {
        state.abort_after_navigation_failure();
    }
}

fn diagnostic_from_probe(error: ProbeError) -> BootstrapDiagnostic {
    match error {
        ProbeError::Unavailable(message) => {
            BootstrapDiagnostic::new(DiagnosticKind::ProbeUnavailable, message)
        }
        ProbeError::Incompatible(message) => {
            BootstrapDiagnostic::new(DiagnosticKind::HostIncompatible, message)
        }
    }
}

fn diagnostic_from_start(error: super::host_lifecycle::HostStartupError) -> BootstrapDiagnostic {
    let kind = match error {
        super::host_lifecycle::HostStartupError::SidecarSpawn { .. } => {
            DiagnosticKind::SidecarSpawn
        }
        super::host_lifecycle::HostStartupError::PortConflict { .. } => {
            DiagnosticKind::HostIncompatible
        }
        super::host_lifecycle::HostStartupError::SidecarExited { .. } => {
            DiagnosticKind::SidecarExited
        }
        super::host_lifecycle::HostStartupError::SidecarIncompatible { .. } => {
            DiagnosticKind::SidecarIncompatible
        }
        super::host_lifecycle::HostStartupError::StartupTimeout { .. } => {
            DiagnosticKind::StartupTimeout
        }
    };
    BootstrapDiagnostic::new(kind, error.to_string())
}

fn fallback_snapshot(message: impl Into<String>) -> BootstrapSnapshot {
    BootstrapSnapshot {
        phase: BootstrapPhase::Recovery,
        endpoint: format!("{DEFAULT_HOST}:{DEFAULT_PORT}"),
        app_url: FIXED_APP_URL.to_owned(),
        diagnostic: Some(BootstrapDiagnostic::new(DiagnosticKind::Internal, message)),
        can_open_browser: false,
        generation: 0,
        in_flight: false,
        closed: true,
    }
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub(crate) enum BrowserOpenError {
    UnsupportedPlatform,
    HostIncompatible(String),
    HostUnavailable(String),
    BrowserLaunch(String),
    InvalidFixedUrl,
}

#[tauri::command]
pub(crate) fn bootstrap_snapshot(state: State<'_, DesktopHost>) -> BootstrapSnapshot {
    state.snapshot()
}

#[tauri::command]
pub(crate) fn retry_probe_existing(
    app: tauri::AppHandle,
    _state: State<'_, DesktopHost>,
) -> BootstrapSnapshot {
    start_background_attempt(app, AttemptKind::ProbeExisting)
}

#[tauri::command]
pub(crate) fn start_local_host(
    app: tauri::AppHandle,
    _state: State<'_, DesktopHost>,
) -> BootstrapSnapshot {
    start_background_attempt(app, AttemptKind::StartLocal)
}

#[tauri::command]
pub(crate) fn open_in_browser(state: State<'_, DesktopHost>) -> Result<(), BrowserOpenError> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = state;
        return Err(BrowserOpenError::UnsupportedPlatform);
    }

    #[cfg(target_os = "linux")]
    {
        let snapshot = state.snapshot();
        if !snapshot.can_open_browser {
            return Err(match state.diagnostics() {
                Some(diagnostic) if diagnostic.kind == DiagnosticKind::HostIncompatible => {
                    BrowserOpenError::HostIncompatible(diagnostic.message)
                }
                Some(diagnostic) => BrowserOpenError::HostUnavailable(diagnostic.message),
                None => BrowserOpenError::HostUnavailable("本地 host 尚未就绪".to_owned()),
            });
        }
        let url = state
            .app_url
            .as_ref()
            .ok_or(BrowserOpenError::InvalidFixedUrl)?;
        // xdg-open 自身立即返回；不在 Tauri command 中同步 probe 或等待浏览器进程。
        Command::new("xdg-open")
            .arg(url.as_str())
            .spawn()
            .map(|_| ())
            .map_err(|error| BrowserOpenError::BrowserLaunch(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use super::{
        super::bootstrap_state::{
            AttemptKind, BootstrapDiagnostic, BootstrapPhase, DiagnosticKind,
        },
        super::host_lifecycle::HostLaunchConfig,
        BootstrapWorker, DesktopHost, FIXED_APP_URL, ValidatedAppUrl,
    };

    fn test_host() -> DesktopHost {
        DesktopHost::new(HostLaunchConfig::new(
            "missing-sidecar",
            "web",
            "db",
            "actor",
            "board",
        ))
    }

    #[test]
    fn fixed_url_is_validated_once_at_the_boundary() {
        let url = ValidatedAppUrl::fixed().expect("fixed URL should be valid");
        assert_eq!(url.as_str(), FIXED_APP_URL);
    }

    #[test]
    fn shutdown_closes_state_and_rejects_stale_start_completion() {
        let host = test_host();
        let generation = host
            .begin(AttemptKind::StartLocal)
            .expect("bootstrap operation should start");
        assert!(host.snapshot().in_flight);
        host.shutdown().expect("shutdown should fence state");
        assert!(host.snapshot().closed);
        assert!(!host.finish_start(
            generation,
            Err(BootstrapDiagnostic::new(
                DiagnosticKind::Internal,
                "stale completion",
            )),
        ));
    }

    #[test]
    fn setup_errors_survive_a_successful_host_start() {
        let host = DesktopHost::from_setup(
            Some(HostLaunchConfig::new(
                "sidecar", "web", "db", "actor", "board",
            )),
            ValidatedAppUrl::fixed().ok(),
            Some(BootstrapDiagnostic::new(
                DiagnosticKind::Tray,
                "tray unavailable",
            )),
        );
        let generation = host
            .begin(AttemptKind::StartLocal)
            .expect("bootstrap operation should start");
        let result = host.finish_start(
            generation,
            Err(BootstrapDiagnostic::new(
                DiagnosticKind::SidecarExited,
                "sidecar exited",
            )),
        );
        assert!(result);
        assert_eq!(host.snapshot().phase, BootstrapPhase::Recovery);
        assert!(
            host.snapshot()
                .diagnostic
                .expect("diagnostic")
                .message
                .contains("tray unavailable")
        );
    }

    #[test]
    fn dropping_a_bootstrap_worker_cancels_and_joins_it() {
        let cancel = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let cancel_for_thread = Arc::clone(&cancel);
        let finished_for_thread = Arc::clone(&finished);
        let join = std::thread::spawn(move || {
            while !cancel_for_thread.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            finished_for_thread.store(true, Ordering::Release);
        });
        drop(BootstrapWorker {
            cancel,
            join: Some(join),
        });
        assert!(finished.load(Ordering::Acquire));
    }
}
