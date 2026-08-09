#[cfg(target_os = "linux")]
use ksni::blocking::TrayMethods;
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use super::{
    bootstrap::DesktopHost,
    tray_lifecycle::{
        TRAY_QUIT_ID, TRAY_SHOW_ID, TrayBackendKind, TrayIconAction, TrayMenuAction,
        restore_window_action, status_notifier_activate_action,
        status_notifier_secondary_activate_action, tray_backend_kind, tray_icon_left_click_action,
        tray_icon_left_double_click_action, tray_menu_action,
    },
};

pub(crate) fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
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
            super::tray_lifecycle::StatusNotifierActivationAction::ShowWindow => {
                show_main_window(&self.app)
            }
        }
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        match status_notifier_secondary_activate_action() {
            super::tray_lifecycle::StatusNotifierActivationAction::ShowWindow => {
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

pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        match restore_window_action() {
            super::tray_lifecycle::RestoreWindowAction::ShowAndRaiseWithoutFocus => {
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
