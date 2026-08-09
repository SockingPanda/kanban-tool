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
        DEFAULT_HOST, DEFAULT_PORT, HostHandle, HostLaunchConfig, ProbeError,
        connect_or_spawn_with_cancel, probe_host_with_cancel,
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

struct PendingWorker {
    generation: u64,
    cancel: Arc<AtomicBool>,
}

#[cfg(test)]
static PAUSE_BEFORE_WORKER_INSTALL: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static WORKER_INSTALL_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static PAUSE_BEFORE_CANCEL_RECHECK: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static CANCEL_RECHECK_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static PAUSE_AFTER_CLOSE_GATE: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static CLOSE_GATE_PAUSED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
fn pause_before_worker_install() {
    if !PAUSE_BEFORE_WORKER_INSTALL.load(Ordering::Acquire) {
        return;
    }
    WORKER_INSTALL_PAUSED.store(true, Ordering::Release);
    while PAUSE_BEFORE_WORKER_INSTALL.load(Ordering::Acquire) {
        thread::yield_now();
    }
    WORKER_INSTALL_PAUSED.store(false, Ordering::Release);
}

#[cfg(test)]
fn pause_before_cancel_recheck() {
    if !PAUSE_BEFORE_CANCEL_RECHECK.load(Ordering::Acquire) {
        return;
    }
    CANCEL_RECHECK_PAUSED.store(true, Ordering::Release);
    while PAUSE_BEFORE_CANCEL_RECHECK.load(Ordering::Acquire) {
        thread::yield_now();
    }
    CANCEL_RECHECK_PAUSED.store(false, Ordering::Release);
}

#[cfg(test)]
fn pause_after_close_gate() {
    if !PAUSE_AFTER_CLOSE_GATE.load(Ordering::Acquire) {
        return;
    }
    CLOSE_GATE_PAUSED.store(true, Ordering::Release);
    while PAUSE_AFTER_CLOSE_GATE.load(Ordering::Acquire) {
        thread::yield_now();
    }
    CLOSE_GATE_PAUSED.store(false, Ordering::Release);
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
    /// 将 worker 注册/启动与 close/cancel 串行化；close 在生命周期线性化中胜出后，不能再
    /// 注册或启动新的 pending worker。
    registration_gate: Mutex<()>,
    worker: Mutex<Option<BootstrapWorker>>,
    pending_worker: Mutex<Option<PendingWorker>>,
    installing_worker: AtomicBool,
    exit_in_progress: AtomicBool,
    exit_ready: AtomicBool,
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
            registration_gate: Mutex::new(()),
            worker: Mutex::new(None),
            pending_worker: Mutex::new(None),
            installing_worker: AtomicBool::new(false),
            exit_in_progress: AtomicBool::new(false),
            exit_ready: AtomicBool::new(false),
            config,
            app_url,
        }
    }

    fn config(&self) -> Option<HostLaunchConfig> {
        self.config.clone()
    }

    pub(crate) fn setup_can_start(&self) -> bool {
        self.config.is_some() && self.app_url.is_some()
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
        match result {
            Ok(()) => {
                runtime.phase = BootstrapPhase::Ready;
                runtime.diagnostic = runtime.setup_diagnostic.clone();
                runtime.external_ready = true;
            }
            Err(diagnostic) => {
                runtime.phase = BootstrapPhase::Recovery;
                runtime.diagnostic = Some(runtime.setup_diagnostic.clone().map_or(
                    diagnostic.clone(),
                    |setup| {
                        BootstrapDiagnostic::new(
                            setup.kind,
                            format!("{}；host probe: {}", setup.message, diagnostic.message),
                        )
                    },
                ));
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
                runtime.phase = BootstrapPhase::Ready;
                runtime.diagnostic = runtime.setup_diagnostic.clone();
                runtime.external_ready = true;
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
        let _registration_gate = self
            .registration_gate
            .lock()
            .expect("桌面 bootstrap registration 锁已失效");
        let in_flight = self
            .runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .fence
            .in_flight()
            .is_some();
        if in_flight {
            #[cfg(test)]
            pause_before_cancel_recheck();
            let mut runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
            // Worker completion may win between the first and second runtime lock.  Re-check
            // under the lock that performs the fence so a successful start stays Ready.
            if runtime.fence.in_flight().is_some() {
                runtime.fence.fence();
                runtime.phase = BootstrapPhase::Recovery;
                runtime.diagnostic = Some(BootstrapDiagnostic::new(
                    DiagnosticKind::Internal,
                    "桌面 bootstrap 已取消",
                ));
                runtime.navigation_claimed = None;
            }
        }
        self.cancel_workers();
    }

    fn close(&self) {
        let _registration_gate = self
            .registration_gate
            .lock()
            .expect("桌面 bootstrap registration 锁已失效");
        #[cfg(test)]
        pause_after_close_gate();
        self.close_locked();
    }

    fn close_locked(&self) {
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

    #[allow(dead_code)]
    pub(crate) fn shutdown(
        &self,
    ) -> Result<Option<super::host_lifecycle::ShutdownResult>, super::host_lifecycle::ShutdownError>
    {
        self.close();
        self.cancel_workers();
        self.reap_finished_worker();
        if self.worker_in_flight() {
            return Err(super::host_lifecycle::ShutdownError::WorkerInFlight);
        }
        self.shutdown_host_only()
    }

    pub(crate) fn request_exit(&self, app: tauri::AppHandle, code: i32) -> bool {
        if self.exit_ready.swap(false, Ordering::AcqRel) {
            return true;
        }
        if self
            .exit_in_progress
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.close();
        self.cancel_workers();
        let app_for_thread = app.clone();
        let spawn_result = thread::Builder::new()
            .name("kanban-desktop-exit-cleanup".to_owned())
            .spawn(move || {
                for _ in 0..120 {
                    let Some(state) = app_for_thread.try_state::<DesktopHost>() else {
                        return;
                    };
                    match state.shutdown_for_exit_attempt() {
                        Ok(_) => {
                            state.exit_ready.store(true, Ordering::Release);
                            state.exit_in_progress.store(false, Ordering::Release);
                            app_for_thread.exit(code);
                            return;
                        }
                        Err(error) => {
                            eprintln!("kanban exit cleanup retry 失败：{error}");
                        }
                    }
                    thread::sleep(std::time::Duration::from_millis(25));
                }
                if let Some(state) = app_for_thread.try_state::<DesktopHost>() {
                    state.exit_in_progress.store(false, Ordering::Release);
                }
                eprintln!("kanban exit cleanup bounded retry exhausted；保留桌面进程避免 orphan");
            });
        if spawn_result.is_err() {
            self.exit_in_progress.store(false, Ordering::Release);
            eprintln!("kanban exit cleanup thread 启动失败；保留桌面进程避免 orphan");
        }
        false
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
        match handle.shutdown() {
            Ok(result) => Ok(Some(result)),
            Err(error) => {
                self.handle
                    .lock()
                    .expect("桌面 host 生命周期锁已失效")
                    .replace(handle);
                Err(error)
            }
        }
    }

    fn shutdown_for_exit_attempt(
        &self,
    ) -> Result<Option<super::host_lifecycle::ShutdownResult>, super::host_lifecycle::ShutdownError>
    {
        self.close();
        self.cancel_workers();
        self.reap_finished_worker();
        if self.worker_in_flight() {
            return Err(super::host_lifecycle::ShutdownError::WorkerInFlight);
        }
        if super::host_lifecycle::reaper_pending_count() != 0 {
            return Err(super::host_lifecycle::ShutdownError::ReaperInFlight);
        }
        let Some(mut handle) = self
            .handle
            .lock()
            .expect("桌面 host 生命周期锁已失效")
            .take()
        else {
            return Ok(None);
        };
        match handle.shutdown_nonblocking() {
            Ok(result) => Ok(Some(result)),
            Err(error) => {
                self.handle
                    .lock()
                    .expect("桌面 host 生命周期锁已失效")
                    .replace(handle);
                Err(error)
            }
        }
    }

    fn abort_after_navigation_failure(&self) {
        self.close();
        if let Err(error) = self.shutdown_host_only() {
            eprintln!("kanban 导航失败后的 host cleanup 失败：{error}");
        }
    }

    #[cfg(test)]
    fn register_pending_worker(&self, cancel: Arc<AtomicBool>, generation: u64) -> bool {
        let _registration_gate = self
            .registration_gate
            .lock()
            .expect("桌面 bootstrap registration 锁已失效");
        self.register_pending_worker_locked(cancel, generation)
    }

    fn register_pending_worker_locked(&self, cancel: Arc<AtomicBool>, generation: u64) -> bool {
        let runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        if !runtime.fence.is_current(generation) || runtime.fence.is_closed() {
            return false;
        }
        let mut pending = self
            .pending_worker
            .lock()
            .expect("桌面 bootstrap pending worker 锁已失效");
        if pending.is_some() {
            return false;
        }
        *pending = Some(PendingWorker { generation, cancel });
        true
    }

    fn unregister_pending_worker_locked(&self, cancel: &Arc<AtomicBool>) {
        let mut pending = self
            .pending_worker
            .lock()
            .expect("桌面 bootstrap pending worker 锁已失效");
        if pending
            .as_ref()
            .is_some_and(|pending| Arc::ptr_eq(&pending.cancel, cancel))
        {
            pending.take();
        }
    }

    fn install_worker(&self, cancel: Arc<AtomicBool>, join: JoinHandle<()>, generation: u64) {
        self.installing_worker.store(true, Ordering::SeqCst);
        let registered = {
            let pending = self
                .pending_worker
                .lock()
                .expect("桌面 bootstrap pending worker 锁已失效");
            pending.as_ref().is_some_and(|pending| {
                pending.generation == generation && Arc::ptr_eq(&pending.cancel, &cancel)
            })
        };
        if !registered {
            cancel.store(true, Ordering::Release);
        }
        #[cfg(test)]
        pause_before_worker_install();
        let pending_cancel = Arc::clone(&cancel);
        let keep = self
            .runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .fence
            .is_current(generation);
        self.worker
            .lock()
            .expect("桌面 bootstrap worker 锁已失效")
            .replace(BootstrapWorker {
                cancel,
                join: Some(join),
            });
        {
            let mut pending = self
                .pending_worker
                .lock()
                .expect("桌面 bootstrap pending worker 锁已失效");
            if pending.as_ref().is_some_and(|pending| {
                pending.generation == generation && Arc::ptr_eq(&pending.cancel, &pending_cancel)
            }) {
                pending.take();
            }
        }
        self.installing_worker.store(false, Ordering::SeqCst);
        if !keep {
            self.reap_finished_worker();
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

    fn cancel_workers(&self) {
        if let Some(worker) = self
            .worker
            .lock()
            .expect("桌面 bootstrap worker 锁已失效")
            .as_ref()
        {
            worker.cancel.store(true, Ordering::Release);
        }
        if let Some(worker) = self
            .pending_worker
            .lock()
            .expect("桌面 bootstrap pending worker 锁已失效")
            .as_ref()
        {
            worker.cancel.store(true, Ordering::Release);
        }
    }

    fn worker_in_flight(&self) -> bool {
        if self.installing_worker.load(Ordering::SeqCst) {
            return true;
        }
        if self
            .pending_worker
            .lock()
            .expect("桌面 bootstrap pending worker 锁已失效")
            .is_some()
        {
            return true;
        }
        self.worker
            .lock()
            .expect("桌面 bootstrap worker 锁已失效")
            .as_ref()
            .and_then(|worker| worker.join.as_ref())
            .is_some_and(|join| !join.is_finished())
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
    // Keep the registration gate through the actual thread spawn and worker-slot install.  A
    // concurrent close/cancel therefore linearizes either before this whole section or after it;
    // it cannot observe an open state and then leave a worker registered/spawned after close.
    let registration_gate = state
        .registration_gate
        .lock()
        .expect("桌面 bootstrap registration 锁已失效");
    if !state.register_pending_worker_locked(Arc::clone(&cancel), generation) {
        drop(registration_gate);
        state.cancel_in_flight();
        return state.snapshot();
    }
    let cancel_for_thread = Arc::clone(&cancel);
    let app_for_thread = app.clone();
    let spawn_result = thread::Builder::new()
        .name("kanban-desktop-bootstrap".to_owned())
        .spawn(move || match kind {
            AttemptKind::ProbeExisting => {
                if cancel_for_thread.load(Ordering::Acquire) {
                    return;
                }
                let result = match probe_host_with_cancel(config.endpoint, &cancel_for_thread) {
                    Ok(_) => Ok(()),
                    Err(error) => Err(diagnostic_from_probe(error)),
                };
                if cancel_for_thread.load(Ordering::Acquire) {
                    return;
                }
                complete_probe_attempt(&app_for_thread, generation, result);
            }
            AttemptKind::StartLocal => {
                if cancel_for_thread.load(Ordering::Acquire) {
                    return;
                }
                let result = connect_or_spawn_with_cancel(&config, &cancel_for_thread)
                    .map_err(diagnostic_from_start);
                if cancel_for_thread.load(Ordering::Acquire) {
                    return;
                }
                complete_start_attempt(&app_for_thread, generation, result);
            }
        });
    let join = match spawn_result {
        Ok(join) => join,
        Err(error) => {
            state.unregister_pending_worker_locked(&cancel);
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
            drop(registration_gate);
            return state.snapshot();
        }
    };
    state.install_worker(cancel, join, generation);
    drop(registration_gate);
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
        ProbeError::Cancelled => {
            BootstrapDiagnostic::new(DiagnosticKind::Internal, "桌面 bootstrap 已取消")
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
        super::host_lifecycle::HostStartupError::Cancelled => DiagnosticKind::Internal,
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
        super::host_lifecycle::{HostHandle, HostLaunchConfig},
        BootstrapWorker, CANCEL_RECHECK_PAUSED, CLOSE_GATE_PAUSED, DesktopHost, FIXED_APP_URL,
        PAUSE_AFTER_CLOSE_GATE, PAUSE_BEFORE_CANCEL_RECHECK, PAUSE_BEFORE_WORKER_INSTALL,
        ValidatedAppUrl, WORKER_INSTALL_PAUSED,
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
        let result = host.finish_start(generation, Ok(HostHandle::external_for_test()));
        assert!(result);
        assert_eq!(host.snapshot().phase, BootstrapPhase::Ready);
        assert!(host.snapshot().can_open_browser || !cfg!(target_os = "linux"));
        assert!(
            host.snapshot()
                .diagnostic
                .expect("diagnostic")
                .message
                .contains("tray unavailable")
        );
        assert!(host.claim_navigation(generation).is_some());
    }

    #[test]
    fn successful_external_probe_recovers_from_setup_diagnostic() {
        let host = DesktopHost::from_setup(
            Some(HostLaunchConfig::new(
                "sidecar", "web", "db", "actor", "board",
            )),
            ValidatedAppUrl::fixed().ok(),
            Some(BootstrapDiagnostic::new(
                DiagnosticKind::Window,
                "window setup warning",
            )),
        );
        let generation = host
            .begin(AttemptKind::ProbeExisting)
            .expect("probe should start");
        assert!(host.finish_probe(generation, Ok(())));
        assert_eq!(host.snapshot().phase, BootstrapPhase::Ready);
        assert!(host.snapshot().can_open_browser || !cfg!(target_os = "linux"));
        assert!(host.claim_navigation(generation).is_some());
    }

    #[test]
    fn cancel_rechecks_before_overwriting_a_successful_start() {
        let host = std::sync::Arc::new(test_host());
        let generation = host
            .begin(AttemptKind::StartLocal)
            .expect("bootstrap operation should start");
        PAUSE_BEFORE_CANCEL_RECHECK.store(true, Ordering::Release);
        CANCEL_RECHECK_PAUSED.store(false, Ordering::Release);
        let host_for_finish = std::sync::Arc::clone(&host);
        let finisher = std::thread::spawn(move || {
            while !CANCEL_RECHECK_PAUSED.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            assert!(host_for_finish.finish_start(generation, Ok(HostHandle::external_for_test()),));
        });
        let host_for_cancel = std::sync::Arc::clone(&host);
        let cancel = std::thread::spawn(move || host_for_cancel.cancel_in_flight());
        let pause_deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !CANCEL_RECHECK_PAUSED.load(Ordering::Acquire)
            && std::time::Instant::now() < pause_deadline
        {
            std::thread::yield_now();
        }
        assert!(
            CANCEL_RECHECK_PAUSED.load(Ordering::Acquire),
            "cancel recheck failpoint was not reached"
        );
        finisher.join().expect("start finisher");
        PAUSE_BEFORE_CANCEL_RECHECK.store(false, Ordering::Release);
        cancel.join().expect("cancel worker");
        assert_eq!(host.snapshot().phase, BootstrapPhase::Ready);
        assert!(host.snapshot().diagnostic.is_none());
    }

    #[test]
    fn shutdown_fences_pending_worker_before_thread_installation() {
        let host = test_host();
        let generation = host
            .begin(AttemptKind::StartLocal)
            .expect("bootstrap operation should start");
        let cancel = Arc::new(AtomicBool::new(false));
        assert!(host.register_pending_worker(Arc::clone(&cancel), generation));
        let error = host
            .shutdown()
            .expect_err("shutdown must wait for pending worker registration");
        assert!(matches!(
            error,
            super::super::host_lifecycle::ShutdownError::WorkerInFlight
        ));
        assert!(cancel.load(Ordering::Acquire));

        let join = std::thread::spawn({
            let cancel = Arc::clone(&cancel);
            move || {
                while !cancel.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
            }
        });
        host.install_worker(cancel, join, generation);
        for _ in 0..100 {
            host.reap_finished_worker();
            if host.worker.lock().expect("worker lock").is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(host.worker.lock().expect("worker lock").is_none());
    }

    #[test]
    fn close_wins_before_pending_registration_can_store_or_spawn() {
        let host = std::sync::Arc::new(test_host());
        let generation = host
            .begin(AttemptKind::StartLocal)
            .expect("bootstrap operation should start");
        let cancel = Arc::new(AtomicBool::new(false));
        PAUSE_AFTER_CLOSE_GATE.store(true, Ordering::Release);
        CLOSE_GATE_PAUSED.store(false, Ordering::Release);
        let host_for_close = std::sync::Arc::clone(&host);
        let closer = std::thread::spawn(move || host_for_close.close());
        let pause_deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !CLOSE_GATE_PAUSED.load(Ordering::Acquire)
            && std::time::Instant::now() < pause_deadline
        {
            std::thread::yield_now();
        }
        assert!(
            CLOSE_GATE_PAUSED.load(Ordering::Acquire),
            "close registration gate failpoint was not reached"
        );
        let host_for_register = std::sync::Arc::clone(&host);
        let cancel_for_register = Arc::clone(&cancel);
        let registered = std::thread::spawn(move || {
            host_for_register.register_pending_worker(cancel_for_register, generation)
        });
        PAUSE_AFTER_CLOSE_GATE.store(false, Ordering::Release);
        closer.join().expect("close worker");
        assert!(!registered.join().expect("registration worker"));
        assert!(host.snapshot().closed);
        assert!(!cancel.load(Ordering::Acquire));
        assert!(!host.worker_in_flight());
    }

    #[test]
    fn shutdown_waits_during_pending_to_installed_worker_transfer() {
        let host = std::sync::Arc::new(test_host());
        let generation = host
            .begin(AttemptKind::StartLocal)
            .expect("bootstrap operation should start");
        let cancel = Arc::new(AtomicBool::new(false));
        assert!(host.register_pending_worker(Arc::clone(&cancel), generation));
        PAUSE_BEFORE_WORKER_INSTALL.store(true, Ordering::Release);
        WORKER_INSTALL_PAUSED.store(false, Ordering::Release);
        let host_for_install = std::sync::Arc::clone(&host);
        let cancel_for_install = Arc::clone(&cancel);
        let installer = std::thread::spawn(move || {
            let join = std::thread::spawn({
                let cancel = Arc::clone(&cancel_for_install);
                move || {
                    while !cancel.load(Ordering::Acquire) {
                        std::thread::yield_now();
                    }
                }
            });
            host_for_install.install_worker(cancel_for_install, join, generation);
        });
        let pause_deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !WORKER_INSTALL_PAUSED.load(Ordering::Acquire)
            && std::time::Instant::now() < pause_deadline
        {
            std::thread::yield_now();
        }
        assert!(
            WORKER_INSTALL_PAUSED.load(Ordering::Acquire),
            "worker install failpoint was not reached"
        );
        let error = host
            .shutdown()
            .expect_err("exit cleanup must wait for installing worker");
        assert!(matches!(
            error,
            super::super::host_lifecycle::ShutdownError::WorkerInFlight
        ));
        assert!(cancel.load(Ordering::Acquire));
        PAUSE_BEFORE_WORKER_INSTALL.store(false, Ordering::Release);
        installer.join().expect("worker installer");
        for _ in 0..100 {
            host.reap_finished_worker();
            if !host.worker_in_flight() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(!host.worker_in_flight());
        assert!(host.shutdown().is_ok());
    }

    #[test]
    fn exit_cleanup_waits_for_reaper_ownership_confirmation() {
        let _reaper_guard = super::super::host_lifecycle::reaper_test_guard();
        let host = test_host();
        super::super::host_lifecycle::hold_reaper_pending_for_test();
        let error = host
            .shutdown_for_exit_attempt()
            .expect_err("exit cleanup must retain prevent-exit barrier");
        assert!(matches!(
            error,
            super::super::host_lifecycle::ShutdownError::ReaperInFlight
        ));
        super::super::host_lifecycle::release_reaper_pending_for_test();
        assert!(host.shutdown_for_exit_attempt().is_ok());
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
