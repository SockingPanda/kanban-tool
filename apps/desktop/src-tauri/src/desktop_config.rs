use std::{env, fs, path::PathBuf};

use tauri::Manager;

use super::host_lifecycle::HostLaunchConfig;

pub(crate) fn host_launch_config(app: &tauri::App) -> tauri::Result<HostLaunchConfig> {
    let resource_dir = app.path().resource_dir()?;
    let web_dir = resolve_web_dir(&resource_dir);
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

pub(crate) fn resolve_sidecar_path(resource_dir: &std::path::Path) -> PathBuf {
    resource_dir.join("kanban")
}

fn resolve_web_dir(resource_dir: &std::path::Path) -> PathBuf {
    let packaged = resource_dir.join("web");
    #[cfg(debug_assertions)]
    if !packaged.is_dir() {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist");
    }
    packaged
}

fn first_non_empty_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        env::var(name)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    })
}

pub(crate) fn desktop_window_title() -> String {
    concat!("kanban ", env!("CARGO_PKG_VERSION")).to_owned()
}

pub(crate) fn set_main_window_title(app: &tauri::App) -> tauri::Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or(tauri::Error::WindowNotFound)?;
    window.set_title(&desktop_window_title())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::desktop_window_title;

    #[test]
    fn desktop_window_title_includes_package_version() {
        assert_eq!(
            desktop_window_title(),
            format!("kanban {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn sidecar_path_matches_package_resource_destination() {
        let resource_dir = Path::new("/tmp/kanban-resource");
        assert_eq!(
            super::resolve_sidecar_path(resource_dir),
            resource_dir.join("kanban")
        );
    }

    #[test]
    fn web_path_prefers_resource_bundle() {
        let resource_dir = Path::new("/tmp/kanban-resource");
        assert_eq!(
            super::resolve_web_dir(resource_dir),
            if cfg!(debug_assertions) {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist")
            } else {
                resource_dir.join("web")
            }
        );
    }
}
