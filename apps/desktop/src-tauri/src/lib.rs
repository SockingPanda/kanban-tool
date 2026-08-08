#![doc = include_str!("../../README.md")]

use std::{env, fs, path::PathBuf, sync::Mutex, thread};

#[cfg(target_os = "linux")]
use ksni::blocking::TrayMethods;
use serde::Serialize;
#[cfg(target_os = "linux")]
use std::process::Command;
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod bootstrap_state;
mod host_lifecycle;
mod tray_lifecycle;
use bootstrap_state::{
    AttemptKind, BootstrapDiagnostic, BootstrapPhase, BootstrapSnapshot, DiagnosticKind,
    GenerationFence,
};
use host_lifecycle::{HostHandle, HostLaunchConfig, ProbeError, connect_or_spawn, probe_host};
use tray_lifecycle::{
    CloseRequestAction, RestoreWindowAction, SingleInstanceAction, TRAY_QUIT_ID, TRAY_SHOW_ID,
    TrayBackendKind, TrayIconAction, TrayMenuAction, close_request_action, restore_window_action,
    single_instance_launch_action, status_notifier_activate_action,
    status_notifier_secondary_activate_action, tray_backend_kind, tray_icon_left_click_action,
    tray_icon_left_double_click_action, tray_menu_action,
};

const FIXED_APP_URL: &str = "http://127.0.0.1:8721/app/";

struct DesktopHost {
    /// Desktop 进程唯一拥有的 host；外部 host 也只以 non-owning handle 记录。
    handle: Mutex<Option<HostHandle>>,
    runtime: Mutex<RuntimeState>,
    config: HostLaunchConfig,
}

struct RuntimeState {
    fence: GenerationFence,
    phase: BootstrapPhase,
    diagnostic: Option<BootstrapDiagnostic>,
    external_ready: bool,
}

impl DesktopHost {
    fn new(config: HostLaunchConfig) -> Self {
        Self {
            handle: Mutex::new(None),
            runtime: Mutex::new(RuntimeState {
                fence: GenerationFence::default(),
                phase: BootstrapPhase::StartingLocal,
                diagnostic: None,
                external_ready: false,
            }),
            config,
        }
    }

    fn begin(&self, kind: AttemptKind) -> Option<u64> {
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
        runtime.diagnostic = None;
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
                // HostHandle 自身验证过的 URL 只用于保留生命周期信息；导航仍固定到常量。
                let _ = handle.app_url();
                self.handle
                    .lock()
                    .expect("桌面 host 生命周期锁已失效")
                    .replace(handle);
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

    fn is_current(&self, generation: u64) -> bool {
        self.runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .fence
            .is_current(generation)
    }

    fn fence(&self) {
        self.runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .fence
            .fence();
    }

    fn close(&self) {
        self.runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .fence
            .close();
    }

    fn snapshot(&self) -> BootstrapSnapshot {
        let runtime = self.runtime.lock().expect("桌面 bootstrap 状态锁已失效");
        let has_host = self
            .handle
            .lock()
            .expect("桌面 host 生命周期锁已失效")
            .is_some();
        BootstrapSnapshot {
            phase: runtime.phase,
            endpoint: self.config.endpoint.to_string(),
            app_url: FIXED_APP_URL.to_owned(),
            diagnostic: runtime.diagnostic.clone(),
            can_open_browser: cfg!(target_os = "linux")
                && runtime.phase == BootstrapPhase::Ready
                && (has_host || runtime.external_ready),
            generation: runtime.fence.generation(),
            in_flight: runtime.fence.in_flight().is_some(),
            closed: runtime.fence.is_closed(),
        }
    }

    fn diagnostics(&self) -> Option<BootstrapDiagnostic> {
        self.runtime
            .lock()
            .expect("桌面 bootstrap 状态锁已失效")
            .diagnostic
            .clone()
    }

    fn shutdown(
        &self,
    ) -> Result<Option<host_lifecycle::ShutdownResult>, host_lifecycle::ShutdownError> {
        self.close();
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
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            match single_instance_launch_action() {
                SingleInstanceAction::ShowWindow => show_main_window(app),
            }
        }))
        .invoke_handler(tauri::generate_handler![
            bootstrap_snapshot,
            retry_probe_existing,
            start_local_host,
            open_in_browser,
        ])
        .setup(|app| {
            let config = host_launch_config(app)?;
            app.manage(DesktopHost::new(config));
            set_main_window_title(app)?;
            setup_tray(app)?;
            let _ = start_background_attempt(app.handle().clone(), AttemptKind::StartLocal);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => match close_request_action() {
                CloseRequestAction::HideToTray => {
                    api.prevent_close();
                    let _ = window.hide();
                }
            },
            tauri::WindowEvent::Destroyed => {
                if let Some(host) = window.app_handle().try_state::<DesktopHost>() {
                    host.fence();
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("运行 kanban 桌面端时出错")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::ExitRequested { .. })
                && let Some(host) = app.try_state::<DesktopHost>()
                && let Err(error) = host.shutdown()
            {
                eprintln!("kanban owned host shutdown 失败：{error}");
            }
        });
}

fn host_launch_config(app: &tauri::App) -> tauri::Result<HostLaunchConfig> {
    let resource_dir = app.path().resource_dir()?;
    let web_dir = resource_dir.join("web");
    let sidecar_path = resolve_sidecar_path(&resource_dir);
    let app_data_dir = app.path().app_data_dir()?;
    fs::create_dir_all(&app_data_dir)?;
    let db_path = app_data_dir.join("kanban.db");
    let actor = first_non_empty_env(&["KANBAN_ACTOR", "USER", "USERNAME"])
        .unwrap_or_else(|| "local".to_owned());
    let board = first_non_empty_env(&["KB_BOARD"]).unwrap_or_else(|| "default".to_owned());
    Ok(HostLaunchConfig::new(
        sidecar_path,
        web_dir,
        db_path,
        actor,
        board,
    ))
}

fn resolve_sidecar_path(resource_dir: &std::path::Path) -> PathBuf {
    resource_dir.join("kanban")
}

fn start_background_attempt(app: tauri::AppHandle, kind: AttemptKind) -> BootstrapSnapshot {
    let Some(state) = app.try_state::<DesktopHost>() else {
        return BootstrapSnapshot {
            phase: BootstrapPhase::Recovery,
            endpoint: "127.0.0.1:8721".to_owned(),
            app_url: FIXED_APP_URL.to_owned(),
            diagnostic: Some(BootstrapDiagnostic::new(
                DiagnosticKind::Internal,
                "桌面 bootstrap state 尚未初始化",
            )),
            can_open_browser: false,
            generation: 0,
            in_flight: false,
            closed: true,
        };
    };
    let Some(generation) = state.begin(kind) else {
        return state.snapshot();
    };
    let config = state.config.clone();
    let app_for_thread = app.clone();
    let spawn_result = thread::Builder::new()
        .name("kanban-desktop-bootstrap".to_owned())
        .spawn(move || match kind {
            AttemptKind::ProbeExisting => {
                let result = match probe_host(config.endpoint) {
                    Ok(_) => Ok(()),
                    Err(error) => Err(diagnostic_from_probe(error)),
                };
                complete_probe_attempt(&app_for_thread, generation, result);
            }
            AttemptKind::StartLocal => {
                let result = connect_or_spawn(&config).map_err(diagnostic_from_start);
                complete_start_attempt(&app_for_thread, generation, result);
            }
        });
    if let Err(error) = spawn_result {
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
    }
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
    let accepted = state.finish_probe(generation, result);
    if accepted && state.is_current(generation) && state.snapshot().phase == BootstrapPhase::Ready {
        navigate_to_fixed_app(app, &state, generation);
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
    let accepted = state.finish_start(generation, result);
    if accepted && state.is_current(generation) && state.snapshot().phase == BootstrapPhase::Ready {
        navigate_to_fixed_app(app, &state, generation);
    }
}

fn navigate_to_fixed_app(app: &tauri::AppHandle, state: &DesktopHost, generation: u64) {
    if !state.is_current(generation) {
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        state.fence();
        let _ = state.shutdown();
        return;
    };
    let Ok(url) = tauri::Url::parse(FIXED_APP_URL) else {
        state.fence();
        let _ = state.shutdown();
        return;
    };
    if window.navigate(url).is_err() {
        state.fence();
        let _ = state.shutdown();
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

fn diagnostic_from_start(error: host_lifecycle::HostStartupError) -> BootstrapDiagnostic {
    let kind = match error {
        host_lifecycle::HostStartupError::SidecarSpawn { .. } => DiagnosticKind::SidecarSpawn,
        host_lifecycle::HostStartupError::PortConflict { .. } => DiagnosticKind::HostIncompatible,
        host_lifecycle::HostStartupError::SidecarExited { .. } => DiagnosticKind::SidecarExited,
        host_lifecycle::HostStartupError::SidecarIncompatible { .. } => {
            DiagnosticKind::SidecarIncompatible
        }
        host_lifecycle::HostStartupError::StartupTimeout { .. } => DiagnosticKind::StartupTimeout,
    };
    BootstrapDiagnostic::new(kind, error.to_string())
}

#[tauri::command]
fn bootstrap_snapshot(state: tauri::State<'_, DesktopHost>) -> BootstrapSnapshot {
    state.snapshot()
}

#[tauri::command]
fn retry_probe_existing(
    app: tauri::AppHandle,
    _state: tauri::State<'_, DesktopHost>,
) -> BootstrapSnapshot {
    start_background_attempt(app, AttemptKind::ProbeExisting)
}

#[tauri::command]
fn start_local_host(
    app: tauri::AppHandle,
    _state: tauri::State<'_, DesktopHost>,
) -> BootstrapSnapshot {
    start_background_attempt(app, AttemptKind::StartLocal)
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
enum BrowserOpenError {
    UnsupportedPlatform,
    HostIncompatible(String),
    HostUnavailable(String),
    BrowserLaunch(String),
    InvalidFixedUrl,
}

#[tauri::command]
fn open_in_browser(state: tauri::State<'_, DesktopHost>) -> Result<(), BrowserOpenError> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = state;
        return Err(BrowserOpenError::UnsupportedPlatform);
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(diagnostic) = state.diagnostics()
            && diagnostic.kind == DiagnosticKind::HostIncompatible
        {
            return Err(BrowserOpenError::HostIncompatible(diagnostic.message));
        }
        let url =
            tauri::Url::parse(FIXED_APP_URL).map_err(|_| BrowserOpenError::InvalidFixedUrl)?;
        if url.scheme() != "http"
            || url.host_str() != Some("127.0.0.1")
            || url.port() != Some(8721)
            || url.path() != "/app/"
        {
            return Err(BrowserOpenError::InvalidFixedUrl);
        }
        match probe_host(state.config.endpoint) {
            Ok(_) => {}
            Err(ProbeError::Incompatible(message)) => {
                return Err(BrowserOpenError::HostIncompatible(message));
            }
            Err(ProbeError::Unavailable(message)) => {
                return Err(BrowserOpenError::HostUnavailable(message));
            }
        }
        Command::new("xdg-open")
            .arg(url.as_str())
            .spawn()
            .map(|_| ())
            .map_err(|error| BrowserOpenError::BrowserLaunch(error.to_string()))
    }
}

fn first_non_empty_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        env::var(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

fn desktop_window_title() -> String {
    concat!("kanban ", env!("CARGO_PKG_VERSION")).to_owned()
}

fn set_main_window_title(app: &tauri::App) -> tauri::Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or(tauri::Error::WindowNotFound)?;
    window.set_title(&desktop_window_title())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    #[cfg(target_os = "linux")]
    {
        debug_assert_eq!(tray_backend_kind(), TrayBackendKind::StatusNotifierItem);
        match setup_status_notifier_tray(app) {
            Ok(()) => return Ok(()),
            Err(error) => {
                eprintln!("kanban 状态通知托盘不可用，将回退到 tauri 托盘：{error:?}");
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        debug_assert_eq!(tray_backend_kind(), TrayBackendKind::TauriTrayIcon);
    }

    setup_tauri_tray(app)
}

fn setup_tauri_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, TRAY_SHOW_ID, "显示", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Kanban Tool")
        .on_menu_event(|app, event| match tray_menu_action(event.id.as_ref()) {
            TrayMenuAction::ShowWindow => show_main_window(app),
            TrayMenuAction::QuitApp => quit_app(app),
            TrayMenuAction::Ignore => {}
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state,
                ..
            } if tray_icon_left_click_action(button_state == MouseButtonState::Up)
                == TrayIconAction::ShowWindow =>
            {
                show_main_window(tray.app_handle())
            }
            TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } if tray_icon_left_double_click_action() == TrayIconAction::ShowWindow => {
                show_main_window(tray.app_handle())
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

#[cfg(target_os = "linux")]
struct LinuxStatusNotifierTray {
    _handle: ksni::blocking::Handle<KanbanStatusNotifierTray>,
}

#[cfg(target_os = "linux")]
struct KanbanStatusNotifierTray {
    app: tauri::AppHandle,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for KanbanStatusNotifierTray {
    fn id(&self) -> String {
        "kanban-desktop".to_owned()
    }

    fn title(&self) -> String {
        "Kanban Tool".to_owned()
    }

    fn icon_name(&self) -> String {
        "kanban-desktop".to_owned()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Kanban Tool".to_owned(),
            description: "Kanban Tool".to_owned(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        match status_notifier_activate_action() {
            tray_lifecycle::StatusNotifierActivationAction::ShowWindow => {
                show_main_window(&self.app)
            }
        }
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        match status_notifier_secondary_activate_action() {
            tray_lifecycle::StatusNotifierActivationAction::ShowWindow => {
                show_main_window(&self.app)
            }
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};

        vec![
            StandardItem {
                label: "显示".to_owned(),
                activate: Box::new(|tray: &mut Self| match tray_menu_action(TRAY_SHOW_ID) {
                    TrayMenuAction::ShowWindow => show_main_window(&tray.app),
                    TrayMenuAction::QuitApp | TrayMenuAction::Ignore => {}
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "退出".to_owned(),
                icon_name: "application-exit".to_owned(),
                activate: Box::new(|tray: &mut Self| match tray_menu_action(TRAY_QUIT_ID) {
                    TrayMenuAction::QuitApp => quit_app(&tray.app),
                    TrayMenuAction::ShowWindow | TrayMenuAction::Ignore => {}
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(target_os = "linux")]
fn setup_status_notifier_tray(app: &tauri::App) -> Result<(), ksni::Error> {
    let handle = KanbanStatusNotifierTray {
        app: app.handle().clone(),
    }
    .assume_sni_available(true)
    .spawn()?;

    app.manage(LinuxStatusNotifierTray { _handle: handle });
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        match restore_window_action() {
            RestoreWindowAction::ShowAndRaiseWithoutFocus => {
                let _ = window.set_always_on_top(true);
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_always_on_top(false);
            }
        }
    }
}

fn quit_app(app: &tauri::AppHandle) {
    if let Some(host) = app.try_state::<DesktopHost>()
        && let Err(error) = host.shutdown()
    {
        eprintln!("kanban owned host graceful shutdown 失败：{error}");
    }
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::{
        DesktopHost, FIXED_APP_URL,
        bootstrap_state::{AttemptKind, BootstrapDiagnostic, DiagnosticKind},
        desktop_window_title,
        host_lifecycle::HostLaunchConfig,
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
    fn desktop_window_title_includes_package_version() {
        assert_eq!(
            desktop_window_title(),
            format!("kanban {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn shutdown_fences_in_flight_bootstrap_without_a_host() {
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
    fn static_bootstrap_contract_uses_external_assets_and_supported_ipc() {
        let html = include_str!("../../bootstrap/index.html");
        let css = include_str!("../../bootstrap/bootstrap.css");
        let javascript = include_str!("../../bootstrap/bootstrap.js");
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("valid Tauri config");

        assert!(html.contains("href=\"./bootstrap.css\""));
        assert!(html.contains("src=\"./bootstrap.js\""));
        assert!(html.contains("aria-live=\"polite\""));
        assert!(!html.contains("<style"));
        assert!(!html.contains("<script>"));
        assert!(!html.contains(" on"));
        assert!(!html.contains("https://"));
        assert!(!css.contains("https://"));
        assert!(javascript.contains("window.__TAURI__"));
        assert!(javascript.contains("api.core.invoke"));
        assert!(!javascript.contains("__TAURI_INTERNALS__"));
        assert!(!javascript.contains("eval("));
        assert!(!javascript.contains("https://"));
        assert!(javascript.contains("navigator.language"));

        assert_eq!(
            config["build"]["frontendDist"],
            serde_json::Value::String("../bootstrap".to_owned())
        );
        assert_eq!(
            config["app"]["withGlobalTauri"],
            serde_json::Value::Bool(true)
        );
        let csp = config["app"]["security"]["csp"]
            .as_str()
            .expect("CSP must be a string");
        assert!(!csp.contains("unsafe-inline"));
        assert!(!csp.contains("unsafe-eval"));
        assert!(csp.contains("connect-src 'self' http://127.0.0.1:8721"));
        assert!(csp.contains("font-src 'self'"));
        assert_eq!(
            config["bundle"]["resources"]["../../web/dist/"],
            serde_json::Value::String("web/".to_owned())
        );
        assert_eq!(FIXED_APP_URL, "http://127.0.0.1:8721/app/");
    }
}
