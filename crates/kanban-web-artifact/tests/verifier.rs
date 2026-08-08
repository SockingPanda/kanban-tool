use std::{
    fs,
    path::{Path, PathBuf},
};

use kanban_protocol::{
    WEB_ARTIFACT_BASE_PATH, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
    WEB_PROTOCOL_VERSION, WebArtifactFile, WebArtifactManifest, web_artifact_file_from_bytes,
    web_artifact_sha256_for_bytes,
};
use kanban_web_artifact::verify_directory;
use tempfile::TempDir;

const SERVER_VERSION: &str = "3.0.0";

fn payloads() -> Vec<(&'static str, &'static [u8])> {
    vec![("assets/app.js", b"abc"), ("index.html", b"<main />")]
}

fn descriptors(payloads: &[(&str, &[u8])]) -> Vec<WebArtifactFile> {
    let mut files = payloads
        .iter()
        .map(|(path, bytes)| web_artifact_file_from_bytes(path, bytes).unwrap())
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files
}

fn manifest_for(payloads: &[(&str, &[u8])]) -> WebArtifactManifest {
    let files = descriptors(payloads);
    let build_id = kanban_protocol::web_artifact_build_id_for(
        WEB_ARTIFACT_FORMAT_VERSION,
        WEB_ARTIFACT_BASE_PATH,
        WEB_ARTIFACT_ENTRYPOINT,
        SERVER_VERSION,
        WEB_PROTOCOL_VERSION,
        &files,
    )
    .unwrap();
    WebArtifactManifest {
        format_version: WEB_ARTIFACT_FORMAT_VERSION,
        base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
        entrypoint: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
        server_version: SERVER_VERSION.to_owned(),
        protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
        build_id,
        files,
    }
}

fn write_artifact(root: &Path, payloads: &[(&str, &[u8])]) -> Vec<u8> {
    for (path, bytes) in payloads {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    let manifest = manifest_for(payloads);
    let bytes = serde_json::to_vec(&manifest).unwrap();
    fs::write(root.join("manifest.json"), &bytes).unwrap();
    bytes
}

fn valid_artifact() -> (TempDir, Vec<u8>) {
    let directory = tempfile::tempdir().unwrap();
    let raw_manifest = write_artifact(directory.path(), &payloads());
    (directory, raw_manifest)
}

fn assert_error(root: &Path, expected_server_version: &str, expected_message: &str) {
    let error = verify_directory(root, expected_server_version).unwrap_err();
    assert!(
        error.to_string().contains(expected_message),
        "expected {expected_message:?} in error, got {error}"
    );
}

#[test]
fn valid_snapshot_has_known_digest_and_sorted_lookup() {
    let (directory, _) = valid_artifact();
    let artifact = verify_directory(directory.path(), SERVER_VERSION).unwrap();

    assert_eq!(artifact.manifest().server_version, SERVER_VERSION);
    assert_eq!(
        artifact.manifest().build_id,
        "sha256:e08ed69384e5d70472d70590d8d449359636002cba68847dee50a9d5eb496b0b"
    );
    assert_eq!(
        artifact.manifest_sha256(),
        web_artifact_sha256_for_bytes(artifact.manifest_bytes())
    );
    assert_eq!(
        artifact
            .payloads()
            .map(|payload| payload.path())
            .collect::<Vec<_>>(),
        vec!["assets/app.js", "index.html"]
    );
    assert_eq!(artifact.payload("assets/app.js").unwrap().bytes(), b"abc");
    assert!(artifact.payload("missing.js").is_none());
}

#[test]
fn verifier_requires_exact_server_version() {
    let (directory, _) = valid_artifact();
    assert_error(directory.path(), "3.0.1", "serverVersion");
}

#[test]
fn verifier_rejects_bad_json_build_id_and_protocol() {
    let (directory, _) = valid_artifact();
    fs::write(directory.path().join("manifest.json"), b"not json").unwrap();
    assert_error(directory.path(), SERVER_VERSION, "manifest");

    let (directory, _) = valid_artifact();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.path().join("manifest.json")).unwrap()).unwrap();
    manifest["buildId"] = serde_json::Value::String(
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
    );
    fs::write(
        directory.path().join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert_error(directory.path(), SERVER_VERSION, "buildId");

    let (directory, _) = valid_artifact();
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.path().join("manifest.json")).unwrap()).unwrap();
    manifest["protocolVersion"] = serde_json::Value::String("legacy".into());
    fs::write(
        directory.path().join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    assert_error(directory.path(), SERVER_VERSION, "protocolVersion");
}

#[test]
fn verifier_rejects_missing_extra_and_empty_directories() {
    let (directory, _) = valid_artifact();
    fs::remove_file(directory.path().join("index.html")).unwrap();
    assert_error(directory.path(), SERVER_VERSION, "缺少");

    let (directory, _) = valid_artifact();
    fs::write(directory.path().join("assets/extra.js"), b"extra").unwrap();
    assert_error(directory.path(), SERVER_VERSION, "extra");

    let (directory, _) = valid_artifact();
    fs::create_dir(directory.path().join("empty")).unwrap();
    assert_error(directory.path(), SERVER_VERSION, "empty");
}

#[test]
fn verifier_rejects_payload_hash_or_bytes_drift() {
    let (directory, _) = valid_artifact();
    fs::write(directory.path().join("assets/app.js"), b"changed").unwrap();
    assert_error(directory.path(), SERVER_VERSION, "mismatch");
}

#[test]
fn snapshot_retains_immutable_bytes_after_disk_mutation_and_raw_manifest() {
    let directory = tempfile::tempdir().unwrap();
    let payload = vec![("index.html", b"<main />".as_slice())];
    for (path, bytes) in &payload {
        fs::write(directory.path().join(path), bytes).unwrap();
    }
    let manifest = serde_json::to_vec(&manifest_for(&payload)).unwrap();
    let mut raw_manifest = b"\n  ".to_vec();
    raw_manifest.extend_from_slice(&manifest);
    raw_manifest.extend_from_slice(b"\n");
    fs::write(directory.path().join("manifest.json"), &raw_manifest).unwrap();

    let artifact = verify_directory(directory.path(), SERVER_VERSION).unwrap();
    assert_eq!(artifact.manifest_bytes(), raw_manifest.as_slice());
    assert_eq!(artifact.payload("index.html").unwrap().bytes(), b"<main />");

    fs::write(directory.path().join("index.html"), b"mutated").unwrap();
    fs::write(directory.path().join("manifest.json"), b"mutated manifest").unwrap();
    assert_eq!(artifact.payload("index.html").unwrap().bytes(), b"<main />");
    assert_eq!(artifact.manifest_bytes(), raw_manifest.as_slice());
}

#[test]
fn verifier_requires_absolute_directory_root() {
    let relative = PathBuf::from("target/kanban-web-artifact-test");
    assert_error(&relative, SERVER_VERSION, "absolute");

    let parent = tempfile::tempdir().unwrap();
    let artifact = parent.path().join("artifact");
    fs::create_dir(&artifact).unwrap();
    write_artifact(&artifact, &payloads());
    let dotted = artifact.join("..").join("artifact");
    assert_error(&dotted, SERVER_VERSION, "`.`");
}

#[cfg(unix)]
mod unix_security {
    use super::*;
    use std::os::unix::{ffi::OsStringExt, fs::symlink, net::UnixListener};

    #[test]
    fn rejects_root_directory_and_manifest_symlinks() {
        let parent = tempfile::tempdir().unwrap();
        let artifact_root = parent.path().join("artifact");
        fs::create_dir(&artifact_root).unwrap();
        write_artifact(&artifact_root, &payloads());
        let link = parent.path().join("root-link");
        symlink(&artifact_root, &link).unwrap();
        assert_error(&link, SERVER_VERSION, "symlink");

        let (directory, _) = valid_artifact();
        let target = directory.path().join("manifest-target.json");
        fs::rename(directory.path().join("manifest.json"), &target).unwrap();
        symlink(&target, directory.path().join("manifest.json")).unwrap();
        assert_error(directory.path(), SERVER_VERSION, "symlink");
    }

    #[test]
    fn rejects_payload_and_directory_symlinks() {
        let (directory, _) = valid_artifact();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("target.js");
        fs::write(&target, b"abc").unwrap();
        fs::remove_file(directory.path().join("assets/app.js")).unwrap();
        symlink(&target, directory.path().join("assets/app.js")).unwrap();
        assert_error(directory.path(), SERVER_VERSION, "symlink");

        let (directory, _) = valid_artifact();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("extra.js"), b"extra").unwrap();
        fs::remove_dir_all(directory.path().join("assets")).unwrap();
        symlink(outside.path(), directory.path().join("assets")).unwrap();
        assert_error(directory.path(), SERVER_VERSION, "symlink");
    }

    #[test]
    fn rejects_hardlinks_and_nonregular_files() {
        let (directory, _) = valid_artifact();
        let outside = tempfile::tempdir().unwrap();
        let hardlink_target = outside.path().join("hardlink-target");
        fs::write(&hardlink_target, b"<main />").unwrap();
        fs::remove_file(directory.path().join("index.html")).unwrap();
        fs::hard_link(&hardlink_target, directory.path().join("index.html")).unwrap();
        assert_error(directory.path(), SERVER_VERSION, "hardlink");

        let (directory, raw_manifest) = valid_artifact();
        let outside = tempfile::tempdir().unwrap();
        let hardlink_target = outside.path().join("manifest-target.json");
        fs::write(&hardlink_target, raw_manifest).unwrap();
        fs::remove_file(directory.path().join("manifest.json")).unwrap();
        fs::hard_link(&hardlink_target, directory.path().join("manifest.json")).unwrap();
        assert_error(directory.path(), SERVER_VERSION, "hardlink");

        let (directory, _) = valid_artifact();
        let socket_path = directory.path().join("index.html");
        fs::remove_file(&socket_path).unwrap();
        let _listener = UnixListener::bind(&socket_path).unwrap();
        assert_error(directory.path(), SERVER_VERSION, "regular");
    }

    #[test]
    fn rejects_non_utf8_filename() {
        let (directory, _) = valid_artifact();
        let invalid_name = std::ffi::OsString::from_vec(vec![b'b', 0xff, b'\n']);
        fs::write(directory.path().join(invalid_name), b"invalid").unwrap();
        assert_error(directory.path(), SERVER_VERSION, "UTF-8");
    }
}
