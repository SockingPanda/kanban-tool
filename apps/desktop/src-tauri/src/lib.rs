use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Mutex, mpsc},
    time::Duration,
};

use serde::Serialize;
use tauri::{
    Manager, State,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tokio::sync::oneshot;

mod tray_lifecycle;
use tray_lifecycle::{
    CloseRequestAction, SingleInstanceAction, TRAY_QUIT_ID, TRAY_SHOW_ID, TrayMenuAction,
    close_request_action, single_instance_launch_action, tray_menu_action,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfig {
    api_base_url: String,
    db_path: PathBuf,
    actor: String,
    board: String,
}

struct EmbeddedApiRuntime {
    config: Mutex<RuntimeConfig>,
    _runtime_guard: kanban_sqlite::DatabaseRuntimeGuard,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
}

impl EmbeddedApiRuntime {
    fn shutdown(&self) {
        if let Some(tx) = self
            .shutdown_tx
            .lock()
            .expect("shutdown lock poisoned")
            .take()
        {
            let _ = tx.send(());
        }
    }
}

impl Drop for EmbeddedApiRuntime {
    fn drop(&mut self) {
        if let Some(tx) = self
            .shutdown_tx
            .get_mut()
            .expect("shutdown lock poisoned")
            .take()
        {
            let _ = tx.send(());
        }
    }
}

#[tauri::command]
fn runtime_config(runtime: State<'_, EmbeddedApiRuntime>) -> RuntimeConfig {
    runtime
        .config
        .lock()
        .expect("runtime config lock poisoned")
        .clone()
}

#[tauri::command]
fn set_runtime_board(
    board: String,
    runtime: State<'_, EmbeddedApiRuntime>,
) -> Result<RuntimeConfig, String> {
    let db_path = runtime
        .config
        .lock()
        .expect("runtime config lock poisoned")
        .db_path
        .clone();
    let board = kanban_sqlite::get_board(&db_path, &board)
        .map_err(|error| error.to_string())?
        .slug;
    let mut config = runtime.config.lock().expect("runtime config lock poisoned");
    config.board = board;
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
            let runtime = start_embedded_api().map_err(|error| error.to_string())?;
            app.manage(runtime);
            setup_tray(app).map_err(|error| error.to_string())?;
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
        .expect("error while running kanban desktop");
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
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
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => show_main_window(tray.app_handle()),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn quit_app(app: &tauri::AppHandle) {
    app.state::<EmbeddedApiRuntime>().shutdown();
    app.exit(0);
}

fn start_embedded_api() -> Result<EmbeddedApiRuntime, String> {
    let db_path = kanban_local::default_db_path();
    let actor = kanban_local::default_actor();
    let runtime_guard =
        kanban_sqlite::begin_database_runtime(&db_path).map_err(|error| error.to_string())?;
    kanban_sqlite::init_database(&db_path, &actor).map_err(|error| error.to_string())?;

    let state = kanban_server::AppState::new(db_path.clone(), actor.clone());
    let router = kanban_server::build_desktop_router(state);
    let (tx, rx) = mpsc::channel::<Result<SocketAddr, String>>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = tx.send(Err(error.to_string()));
                return;
            }
        };
        runtime.block_on(async move {
            let listener = match tokio::net::TcpListener::bind(("127.0.0.1", 0)).await {
                Ok(listener) => listener,
                Err(error) => {
                    let _ = tx.send(Err(error.to_string()));
                    return;
                }
            };
            let addr = match listener.local_addr() {
                Ok(addr) => addr,
                Err(error) => {
                    let _ = tx.send(Err(error.to_string()));
                    return;
                }
            };
            let _ = tx.send(Ok(addr));
            let server = axum::serve(listener, router).with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });
            if let Err(error) = server.await {
                eprintln!("kanban embedded API stopped: {error}");
            }
        });
    });

    let addr = rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "embedded API did not report a listening address".to_owned())??;

    Ok(EmbeddedApiRuntime {
        config: Mutex::new(RuntimeConfig {
            api_base_url: format!("http://{addr}"),
            db_path,
            actor,
            board: "default".to_owned(),
        }),
        _runtime_guard: runtime_guard,
        shutdown_tx: Mutex::new(Some(shutdown_tx)),
    })
}
