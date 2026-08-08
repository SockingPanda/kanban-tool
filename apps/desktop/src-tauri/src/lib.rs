#![doc = include_str!("../../README.md")]

use std::{env, fs, path::PathBuf, sync::Mutex};

#[cfg(target_os = "linux")]
use ksni::blocking::TrayMethods;
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

mod host_lifecycle;
mod tray_lifecycle;
use host_lifecycle::{HostHandle, HostLaunchConfig, connect_or_spawn};
use tray_lifecycle::{
    CloseRequestAction, RestoreWindowAction, SingleInstanceAction, TRAY_QUIT_ID, TRAY_SHOW_ID,
    TrayBackendKind, TrayIconAction, TrayMenuAction, close_request_action, restore_window_action,
    single_instance_launch_action, status_notifier_activate_action,
    status_notifier_secondary_activate_action, tray_backend_kind, tray_icon_left_click_action,
    tray_icon_left_double_click_action, tray_menu_action,
};

struct DesktopHost {
    handle: Mutex<HostHandle>,
}

impl DesktopHost {
    fn shutdown(&self) -> Result<host_lifecycle::ShutdownResult, host_lifecycle::ShutdownError> {
        self.handle
            .lock()
            .expect("桌面 host 生命周期锁已失效")
            .shutdown()
    }

    #[allow(dead_code)]
    fn app_url(&self) -> String {
        self.handle
            .lock()
            .expect("桌面 host 生命周期锁已失效")
            .app_url()
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            match single_instance_launch_action() {
                SingleInstanceAction::ShowWindow => show_main_window(app),
            }
        }))
        .setup(|app| {
            let host = start_desktop_host(app)?;
            let app_url = host.app_url();
            app.manage(DesktopHost {
                handle: Mutex::new(host),
            });
            set_main_window_title(app)?;
            setup_tray(app)?;
            navigate_main_window(app, &app_url)?;
            Ok(())
        })
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

fn start_desktop_host(app: &tauri::App) -> tauri::Result<HostHandle> {
    let resource_dir = app.path().resource_dir()?;
    let web_dir = resource_dir.join("web");
    let sidecar_path = resolve_sidecar_path(&resource_dir)?;
    let app_data_dir = app.path().app_data_dir()?;
    fs::create_dir_all(&app_data_dir)?;
    let db_path = app_data_dir.join("kanban.db");
    let actor = first_non_empty_env(&["KANBAN_ACTOR", "USER", "USERNAME"])
        .unwrap_or_else(|| "local".to_owned());
    let board = first_non_empty_env(&["KB_BOARD"]).unwrap_or_else(|| "default".to_owned());
    let config = HostLaunchConfig::new(sidecar_path, web_dir, db_path, actor, board);
    connect_or_spawn(&config).map_err(setup_error)
}

fn resolve_sidecar_path(resource_dir: &std::path::Path) -> tauri::Result<PathBuf> {
    let path = resource_dir.join("kanban");
    if path.is_file() {
        Ok(path)
    } else {
        Err(setup_error_message(format!(
            "bundled kanban serve sidecar 不存在于精确路径 {}",
            path.display()
        )))
    }
}

fn navigate_main_window(app: &tauri::App, app_url: &str) -> tauri::Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or(tauri::Error::WindowNotFound)?;
    let url = tauri::Url::parse(app_url).map_err(tauri::Error::InvalidUrl)?;
    window.navigate(url)
}

fn setup_error(error: impl std::error::Error + Send + Sync + 'static) -> tauri::Error {
    let error: Box<dyn std::error::Error> = Box::new(error);
    tauri::Error::Setup(error.into())
}

fn setup_error_message(message: impl Into<String>) -> tauri::Error {
    setup_error(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        message.into(),
    ))
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
    use super::desktop_window_title;

    #[test]
    fn desktop_window_title_includes_package_version() {
        assert_eq!(
            desktop_window_title(),
            format!("kanban {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
