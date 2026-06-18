#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseRequestAction {
    HideToTray,
}

pub fn close_request_action() -> CloseRequestAction {
    CloseRequestAction::HideToTray
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleInstanceAction {
    ShowWindow,
}

pub fn single_instance_launch_action() -> SingleInstanceAction {
    SingleInstanceAction::ShowWindow
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreWindowAction {
    ShowAndRaiseWithoutFocus,
}

pub fn restore_window_action() -> RestoreWindowAction {
    RestoreWindowAction::ShowAndRaiseWithoutFocus
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayBackendKind {
    StatusNotifierItem,
    TauriTrayIcon,
}

pub fn tray_backend_kind() -> TrayBackendKind {
    if cfg!(target_os = "linux") {
        TrayBackendKind::StatusNotifierItem
    } else {
        TrayBackendKind::TauriTrayIcon
    }
}

pub const TRAY_SHOW_ID: &str = "show";
pub const TRAY_QUIT_ID: &str = "quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayIconAction {
    ShowWindow,
    Ignore,
}

pub fn tray_icon_left_click_action(button_is_up: bool) -> TrayIconAction {
    if button_is_up {
        TrayIconAction::ShowWindow
    } else {
        TrayIconAction::Ignore
    }
}

pub fn tray_icon_left_double_click_action() -> TrayIconAction {
    TrayIconAction::ShowWindow
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusNotifierActivationAction {
    ShowWindow,
}

pub fn status_notifier_activate_action() -> StatusNotifierActivationAction {
    StatusNotifierActivationAction::ShowWindow
}

pub fn status_notifier_secondary_activate_action() -> StatusNotifierActivationAction {
    StatusNotifierActivationAction::ShowWindow
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMenuAction {
    ShowWindow,
    QuitApp,
    Ignore,
}

pub fn tray_menu_action(id: &str) -> TrayMenuAction {
    match id {
        TRAY_SHOW_ID => TrayMenuAction::ShowWindow,
        TRAY_QUIT_ID => TrayMenuAction::QuitApp,
        _ => TrayMenuAction::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_request_hides_window_without_shutdown() {
        assert_eq!(close_request_action(), CloseRequestAction::HideToTray);
    }

    #[test]
    fn tray_menu_ids_map_to_window_and_process_actions() {
        assert_eq!(tray_menu_action(TRAY_SHOW_ID), TrayMenuAction::ShowWindow);
        assert_eq!(tray_menu_action(TRAY_QUIT_ID), TrayMenuAction::QuitApp);
        assert_eq!(tray_menu_action("unknown"), TrayMenuAction::Ignore);
    }

    #[test]
    fn second_launch_requests_existing_window() {
        assert_eq!(
            single_instance_launch_action(),
            SingleInstanceAction::ShowWindow
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn tray_left_double_click_requests_existing_window() {
        assert_eq!(
            tray_icon_left_double_click_action(),
            TrayIconAction::ShowWindow
        );
    }

    #[test]
    fn restore_window_does_not_force_focus() {
        assert_eq!(
            restore_window_action(),
            RestoreWindowAction::ShowAndRaiseWithoutFocus
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_tray_uses_status_notifier_activation() {
        assert_eq!(tray_backend_kind(), TrayBackendKind::StatusNotifierItem);
        assert_eq!(
            status_notifier_activate_action(),
            StatusNotifierActivationAction::ShowWindow
        );
        assert_eq!(
            status_notifier_secondary_activate_action(),
            StatusNotifierActivationAction::ShowWindow
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn non_linux_tray_uses_tauri_tray_icon_events() {
        assert_eq!(tray_backend_kind(), TrayBackendKind::TauriTrayIcon);
    }
}
