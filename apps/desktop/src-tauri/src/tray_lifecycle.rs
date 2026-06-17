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

pub const TRAY_SHOW_ID: &str = "show";
pub const TRAY_QUIT_ID: &str = "quit";

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
}
