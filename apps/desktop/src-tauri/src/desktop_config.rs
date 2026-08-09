use std::{
    env, fs,
    path::{Path, PathBuf},
};

use kanban_protocol::{WebArtifactManifest, validate_web_artifact_manifest};
use tauri::Manager;

use super::host_lifecycle::HostLaunchConfig;

pub(crate) fn host_launch_config(app: &tauri::App) -> tauri::Result<HostLaunchConfig> {
    let resource_dir = app.path().resource_dir()?;
    let web_dir = resolve_web_dir(&resource_dir)?;
    let expected_web_build_id = load_web_artifact_build_id(&web_dir)?;
    let sidecar_path = resolve_sidecar_path_for_runtime(&resource_dir);
    let sidecar_resource_dir = resolve_sidecar_resource_dir(&resource_dir);
    let app_data_dir = app.path().app_data_dir()?;
    fs::create_dir_all(&app_data_dir)?;
    let db_path = app_data_dir.join("kanban.db");
    let actor = first_non_empty_env(&["KANBAN_ACTOR", "USER", "USERNAME"])
        .unwrap_or_else(|| "local".to_owned());
    let board = first_non_empty_env(&["KB_BOARD"]).unwrap_or_else(|| "default".to_owned());
    Ok(
        HostLaunchConfig::new(sidecar_path, web_dir, db_path, actor, board)
            .with_resource_dir(sidecar_resource_dir)
            .with_expected_web_build_id(expected_web_build_id),
    )
}

/// 读取并验证 Desktop 实际将托管的 Web manifest；不能只相信目录名或任意字符串 build id。
pub(crate) fn load_web_artifact_build_id(web_dir: &Path) -> std::io::Result<String> {
    let manifest_path = web_dir.join("manifest.json");
    let bytes = fs::read(&manifest_path)?;
    let manifest: WebArtifactManifest = serde_json::from_slice(&bytes).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Web artifact manifest JSON 无法解析: {error}"),
        )
    })?;
    validate_web_artifact_manifest(&manifest).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Web artifact manifest 校验失败: {error}"),
        )
    })?;
    Ok(manifest.build_id)
}

fn web_dir_candidate(resource_dir: &std::path::Path) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let _ = resource_dir;
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist")
    }
    #[cfg(not(debug_assertions))]
    {
        resource_dir.join("web")
    }
}

fn resolve_web_dir(resource_dir: &std::path::Path) -> tauri::Result<PathBuf> {
    let candidate = web_dir_candidate(resource_dir);
    fs::canonicalize(&candidate).map_err(Into::into)
}

fn resolve_sidecar_path_for_runtime(resource_dir: &std::path::Path) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        let _ = resource_dir;
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/kanban")
    }
    #[cfg(not(debug_assertions))]
    {
        resource_dir.join("kanban")
    }
}

fn resolve_sidecar_resource_dir(resource_dir: &std::path::Path) -> PathBuf {
    resolve_sidecar_path_for_runtime(resource_dir)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| resource_dir.to_owned())
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

    use kanban_protocol::{
        WEB_ARTIFACT_BASE_PATH, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
        WEB_PROTOCOL_VERSION, WebArtifactFile, WebArtifactManifest, web_artifact_build_id_for,
    };

    use super::desktop_window_title;

    #[test]
    fn desktop_window_title_includes_package_version() {
        assert_eq!(
            desktop_window_title(),
            format!("kanban {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn runtime_sidecar_path_matches_dev_or_package_layout() {
        let resource_dir = Path::new("/tmp/kanban-resource");
        let expected = if cfg!(debug_assertions) {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/kanban")
        } else {
            resource_dir.join("kanban")
        };
        assert_eq!(
            super::resolve_sidecar_path_for_runtime(resource_dir),
            expected
        );
        assert_eq!(
            super::resolve_sidecar_resource_dir(resource_dir),
            expected.parent().expect("sidecar parent").to_path_buf()
        );
    }

    #[test]
    fn local_web_manifest_build_id_is_loaded_only_after_validation() {
        let temp = tempfile::tempdir().expect("manifest fixture root");
        let payload = WebArtifactFile {
            path: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
            bytes: 1,
            sha256: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
        };
        let build_id = web_artifact_build_id_for(
            WEB_ARTIFACT_FORMAT_VERSION,
            WEB_ARTIFACT_BASE_PATH,
            WEB_ARTIFACT_ENTRYPOINT,
            "3.0.0",
            WEB_PROTOCOL_VERSION,
            std::slice::from_ref(&payload),
        )
        .expect("build id");
        let manifest = WebArtifactManifest {
            format_version: WEB_ARTIFACT_FORMAT_VERSION,
            base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
            entrypoint: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
            server_version: "3.0.0".to_owned(),
            protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
            build_id,
            files: vec![payload],
        };
        std::fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&manifest).expect("manifest JSON"),
        )
        .expect("valid manifest");
        let build_id = super::load_web_artifact_build_id(temp.path())
            .expect("valid Web artifact manifest should validate");
        assert!(build_id.starts_with("sha256:"));

        let invalid = tempfile::tempdir().expect("invalid manifest fixture root");
        std::fs::write(invalid.path().join("manifest.json"), b"{}").expect("invalid manifest");
        let error = super::load_web_artifact_build_id(invalid.path())
            .expect_err("malformed manifest must fail closed");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn web_path_prefers_resource_bundle() {
        let resource_dir = Path::new("/tmp/kanban-resource");
        assert_eq!(
            super::web_dir_candidate(resource_dir),
            if cfg!(debug_assertions) {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/dist")
            } else {
                resource_dir.join("web")
            }
        );
    }
}
