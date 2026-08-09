#![doc = include_str!("../../README.md")]

use tauri::Manager;

mod bootstrap;
mod bootstrap_state;
mod desktop_config;
mod desktop_tray;
mod host_lifecycle;
mod tray_lifecycle;

use bootstrap::{DesktopHost, ValidatedAppUrl, start_background_attempt};
use bootstrap_state::{AttemptKind, BootstrapDiagnostic, DiagnosticKind};
use desktop_config::{host_launch_config, set_main_window_title};
use tray_lifecycle::close_request_action;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            match tray_lifecycle::single_instance_launch_action() {
                tray_lifecycle::SingleInstanceAction::ShowWindow => {
                    desktop_tray::show_main_window(app)
                }
            }
        }))
        .invoke_handler(tauri::generate_handler![
            bootstrap::bootstrap_snapshot,
            bootstrap::retry_probe_existing,
            bootstrap::start_local_host,
            bootstrap::open_in_browser,
        ])
        .setup(|app| {
            let config_result = host_launch_config(app);
            let config = config_result.as_ref().ok().cloned();
            let app_url_result = ValidatedAppUrl::fixed();
            let app_url = app_url_result.as_ref().ok().cloned();
            let setup_diagnostic = config_result
                .err()
                .map(|error| {
                    BootstrapDiagnostic::new(DiagnosticKind::Configuration, error.to_string())
                })
                .or_else(|| app_url_result.err());

            app.manage(DesktopHost::from_setup(config, app_url, setup_diagnostic));
            let state = app.state::<DesktopHost>();

            if let Err(error) = set_main_window_title(app) {
                state.record_setup_diagnostic(BootstrapDiagnostic::new(
                    DiagnosticKind::Window,
                    format!("设置桌面窗口标题失败: {error}"),
                ));
            }
            if let Err(error) = desktop_tray::setup_tray(app) {
                state.record_setup_diagnostic(BootstrapDiagnostic::new(
                    DiagnosticKind::Tray,
                    format!("初始化系统托盘失败: {error}"),
                ));
            }
            if state.setup_can_start() {
                let _ = start_background_attempt(app.handle().clone(), AttemptKind::StartLocal);
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                if let Some(host) = window.app_handle().try_state::<DesktopHost>() {
                    // 隐藏到托盘也必须取消正在启动的 sidecar，避免关闭路径留下 detached worker。
                    host.cancel_in_flight();
                }
                match close_request_action() {
                    tray_lifecycle::CloseRequestAction::HideToTray => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
            tauri::WindowEvent::Destroyed => {
                if let Some(host) = window.app_handle().try_state::<DesktopHost>()
                    && host.request_exit(window.app_handle().clone(), 0)
                {
                    window.app_handle().exit(0);
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("运行 kanban 桌面端时出错")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event
                && let Some(host) = app.try_state::<DesktopHost>()
                && !host.request_exit(app.clone(), code.unwrap_or(0))
            {
                api.prevent_exit();
            }
        });
}

#[cfg(test)]
mod tests {
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
        assert!(javascript.contains("POLL_DELAYS_MS"));
        assert!(javascript.contains("latest.closed"));

        assert_eq!(
            config["build"]["frontendDist"],
            serde_json::Value::String("../bootstrap".to_owned())
        );
        assert!(config["build"]["devUrl"].is_null());
        assert!(
            config["build"]["beforeDevCommand"]
                .as_str()
                .is_some_and(|command| command.contains("desktop-dev-prep"))
        );
        assert!(
            config["build"]["beforeBuildCommand"]
                .as_str()
                .is_some_and(|command| command.contains("@kanban-tool/web"))
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
        assert!(csp.contains("connect-src 'self' ipc: http://ipc.localhost http://127.0.0.1:8721"));
        assert!(csp.contains("font-src 'self'"));
        assert_eq!(
            config["bundle"]["resources"]["../../web/dist/"],
            serde_json::Value::String("web/".to_owned())
        );
        assert_eq!(
            config["bundle"]["resources"]["bin/kanban"],
            serde_json::Value::String("kanban".to_owned())
        );
    }
}
