use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Mutex, mpsc},
    time::Duration,
};

use serde::Serialize;
use tauri::{Manager, State};
use tokio::sync::oneshot;

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
        .setup(|app| {
            let runtime = start_embedded_api().map_err(|error| error.to_string())?;
            app.manage(runtime);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![runtime_config, set_runtime_board])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.app_handle().state::<EmbeddedApiRuntime>().shutdown();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running kanban desktop");
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
