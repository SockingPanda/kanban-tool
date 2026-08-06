use std::{
    env,
    net::{IpAddr, SocketAddr},
    sync::{Mutex, MutexGuard},
};

#[cfg(target_os = "linux")]
use ksni::blocking::TrayMethods;
use serde::Serialize;
use tauri::{
    Manager, State,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod tray_lifecycle;
use tray_lifecycle::{
    CloseRequestAction, RestoreWindowAction, SingleInstanceAction, TRAY_QUIT_ID, TRAY_SHOW_ID,
    TrayBackendKind, TrayIconAction, TrayMenuAction, close_request_action, restore_window_action,
    single_instance_launch_action, status_notifier_activate_action,
    status_notifier_secondary_activate_action, tray_backend_kind, tray_icon_left_click_action,
    tray_icon_left_double_click_action, tray_menu_action,
};

const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8721";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfig {
    api_base_url: String,
    actor: String,
    board: String,
}

struct DesktopRuntime {
    config: Mutex<RuntimeConfig>,
}

impl DesktopRuntime {
    fn config(&self) -> MutexGuard<'_, RuntimeConfig> {
        self.config.lock().expect("桌面运行时配置锁已失效")
    }
}

#[tauri::command]
fn runtime_config(runtime: State<'_, DesktopRuntime>) -> RuntimeConfig {
    runtime.config().clone()
}

#[tauri::command]
fn set_runtime_board(
    board: String,
    runtime: State<'_, DesktopRuntime>,
) -> Result<RuntimeConfig, String> {
    let board = board.trim();
    if board.is_empty() {
        return Err("board 不能为空".to_owned());
    }

    let mut config = runtime.config();
    config.board = board.to_owned();
    Ok(config.clone())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            match single_instance_launch_action() {
                SingleInstanceAction::ShowWindow => show_main_window(app),
            }
        }))
        .setup(|app| {
            app.manage(DesktopRuntime {
                config: Mutex::new(default_runtime_config()?),
            });
            set_main_window_title(app)?;
            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![runtime_config, set_runtime_board])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match close_request_action() {
                    CloseRequestAction::HideToTray => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("运行 kanban 桌面端时出错");
}

fn default_runtime_config() -> Result<RuntimeConfig, String> {
    let api_base_url = env::var("KANBAN_SERVER_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SERVER_URL.to_owned());
    let api_base_url = normalize_loopback_url(&api_base_url)?;

    let actor = first_non_empty_env(&["KANBAN_ACTOR", "USER", "USERNAME"])
        .unwrap_or_else(|| "local".to_owned());
    let board = first_non_empty_env(&["KB_BOARD"]).unwrap_or_else(|| "default".to_owned());

    Ok(RuntimeConfig {
        api_base_url,
        actor,
        board,
    })
}

fn normalize_loopback_url(value: &str) -> Result<String, String> {
    let value = value.trim().trim_end_matches('/');
    let authority = value
        .strip_prefix("http://")
        .ok_or_else(|| "KANBAN_SERVER_URL 必须使用回环地址上的 http".to_owned())?;
    if authority.is_empty()
        || authority.contains(['/', '?', '#', '@'])
        || !is_loopback_authority(authority)
    {
        return Err("KANBAN_SERVER_URL 必须指向回环地址".to_owned());
    }
    Ok(value.to_owned())
}

fn is_loopback_authority(authority: &str) -> bool {
    if matches!(authority, "localhost" | "[::1]") {
        return true;
    }
    if let Some(port) = authority.strip_prefix("localhost:") {
        return port.parse::<u16>().is_ok();
    }
    authority.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
        || authority
            .parse::<SocketAddr>()
            .is_ok_and(|addr| addr.ip().is_loopback())
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
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::{desktop_window_title, normalize_loopback_url};

    #[test]
    fn server_url_accepts_only_loopback_http() {
        assert_eq!(
            normalize_loopback_url("http://127.0.0.1:8721/").expect("loopback URL"),
            "http://127.0.0.1:8721"
        );
        assert_eq!(
            normalize_loopback_url("http://localhost:8721").expect("localhost URL"),
            "http://localhost:8721"
        );
        assert!(normalize_loopback_url("https://127.0.0.1:8721").is_err());
        assert!(normalize_loopback_url("http://example.com:8721").is_err());
        assert!(normalize_loopback_url("http://127.0.0.1:8721@evil.example").is_err());
        assert!(normalize_loopback_url("http://localhost:8721/api").is_err());
    }

    #[test]
    fn desktop_window_title_includes_package_version() {
        assert_eq!(
            desktop_window_title(),
            format!("kanban {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
