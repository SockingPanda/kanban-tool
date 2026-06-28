use std::{
    env,
    net::SocketAddr,
    path::PathBuf,
    sync::{Mutex, mpsc},
    time::Duration,
};

#[cfg(target_os = "linux")]
use ksni::blocking::TrayMethods;
use serde::Serialize;
use tauri::{
    Manager, State,
    menu::{Menu, MenuItem},
    path::BaseDirectory,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tokio::sync::oneshot;

mod tray_lifecycle;
use tray_lifecycle::{
    CloseRequestAction, RestoreWindowAction, SingleInstanceAction, TRAY_QUIT_ID, TRAY_SHOW_ID,
    TrayBackendKind, TrayIconAction, TrayMenuAction, close_request_action, restore_window_action,
    single_instance_launch_action, status_notifier_activate_action,
    status_notifier_secondary_activate_action, tray_backend_kind, tray_icon_left_click_action,
    tray_icon_left_double_click_action, tray_menu_action,
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
            let runtime = start_embedded_api(app).map_err(|error| error.to_string())?;
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
    #[cfg(target_os = "linux")]
    {
        debug_assert_eq!(tray_backend_kind(), TrayBackendKind::StatusNotifierItem);
        match setup_status_notifier_tray(app) {
            Ok(()) => return Ok(()),
            Err(error) => {
                eprintln!(
                    "kanban status notifier tray unavailable; falling back to tauri tray: {error:?}"
                );
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
    app.state::<EmbeddedApiRuntime>().shutdown();
    app.exit(0);
}

fn embedded_app_state(
    app: &tauri::App,
    db_path: PathBuf,
    actor: String,
) -> kanban_server::AppState {
    let mut state = kanban_server::AppState::new(db_path, actor);
    if let Some(path) = bundled_helper_path(app, "kanban-vector-lancedb") {
        state = state.with_vector_helper_path(path);
    }
    if let Some(path) = bundled_helper_path(app, "kanban-graph-oxigraph") {
        state = state.with_graph_helper_path(path);
    }
    state
}

fn bundled_helper_path(app: &tauri::App, binary_name: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(path) = app.path().resolve(binary_name, BaseDirectory::Resource) {
        candidates.push(path);
    }
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(binary_name));
    }
    if let Ok(current_exe) = env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join(binary_name));
        }
    }
    first_existing_path(candidates)
}

fn first_existing_path(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|path| path.is_file())
}

fn start_embedded_api(app: &tauri::App) -> Result<EmbeddedApiRuntime, String> {
    let db_path = kanban_local::default_db_path();
    let actor = kanban_local::default_actor();
    let runtime_guard =
        kanban_sqlite::begin_database_runtime(&db_path).map_err(|error| error.to_string())?;
    kanban_sqlite::init_database(&db_path, &actor).map_err(|error| error.to_string())?;

    let state = embedded_app_state(app, db_path.clone(), actor.clone());
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::first_existing_path;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "kanban-desktop-{name}-{}-{}",
                std::process::id(),
                std::thread::current().name().unwrap_or("test")
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn first_existing_path_prefers_first_existing_regular_file() {
        let dir = TempDir::new("first-existing");
        let missing = dir.path.join("missing-helper");
        let first = dir.path.join("kanban-vector-lancedb");
        let second = dir.path.join("kanban-graph-oxigraph");
        std::fs::write(&first, b"vector").expect("write first");
        std::fs::write(&second, b"graph").expect("write second");

        let path = first_existing_path([missing, first.clone(), second]).expect("helper path");

        assert_eq!(path, first);
    }

    #[test]
    fn first_existing_path_ignores_directories_and_missing_candidates() {
        let dir = TempDir::new("ignore-directories");
        let directory = dir.path.join("kanban-vector-lancedb");
        std::fs::create_dir(&directory).expect("helper directory");

        assert!(first_existing_path([directory, dir.path.join("missing")]).is_none());
    }
}
