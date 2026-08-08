use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

use kanban_protocol::{
    WEB_ARTIFACT_BASE_PATH, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
    WEB_PROTOCOL_VERSION, WebArtifactManifest, web_artifact_build_id_for,
    web_artifact_file_from_bytes,
};
use serde_json::Value;

struct TemporaryTree {
    path: PathBuf,
}

impl TemporaryTree {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "xtask-web-assets-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary tree should be creatable");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn create_artifact(root: &Path, relative_dir: &str, server_version: &str) -> PathBuf {
    let dist = root.join(relative_dir);
    fs::create_dir_all(dist.join("assets")).expect("artifact directory should be creatable");
    let payloads = [
        ("assets/app.js", b"console.log('ok');".as_slice()),
        ("index.html", b"<!doctype html>".as_slice()),
    ];
    let mut files = Vec::new();
    for (path, bytes) in payloads {
        let target = dist.join(path);
        fs::write(&target, bytes).expect("artifact payload should be writable");
        files.push(web_artifact_file_from_bytes(path, bytes).expect("payload should be valid"));
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let build_id = web_artifact_build_id_for(
        WEB_ARTIFACT_FORMAT_VERSION,
        WEB_ARTIFACT_BASE_PATH,
        WEB_ARTIFACT_ENTRYPOINT,
        server_version,
        WEB_PROTOCOL_VERSION,
        &files,
    )
    .expect("artifact build id should be computable");
    let manifest = WebArtifactManifest {
        format_version: WEB_ARTIFACT_FORMAT_VERSION,
        base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
        entrypoint: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
        server_version: server_version.to_owned(),
        protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
        build_id,
        files,
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest).expect("manifest should serialize");
    bytes.push(b'\n');
    fs::write(dist.join("manifest.json"), bytes).expect("manifest should be writable");
    dist
}

fn run_web_assets(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(arguments)
        .output()
        .expect("xtask binary should execute")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr should be UTF-8")
}

fn mismatched_server_version() -> String {
    let mut parts = env!("CARGO_PKG_VERSION").split('.').map(|part| {
        part.parse::<u64>()
            .expect("workspace version should be numeric")
    });
    let major = parts.next().expect("workspace version major");
    let minor = parts.next().expect("workspace version minor");
    let patch = parts.next().expect("workspace version patch");
    assert!(parts.next().is_none(), "workspace version should be SemVer");
    format!("{major}.{minor}.{}", patch + 1)
}

#[test]
fn committed_schema_tree_and_fixtures_match_registry() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate must live below workspace root");

    xtask::check_contract(repo_root)
        .expect("committed schema contract should match fresh generation");
}

#[test]
fn generated_artifact_set_is_byte_deterministic() {
    let first = xtask::expected_artifacts().expect("first generation should succeed");
    let second = xtask::expected_artifacts().expect("second generation should succeed");

    assert_eq!(first, second);
    assert!(first.contains_key("manifest.json"));
    assert!(first.contains_key("operations.json"));
    assert!(first.contains_key("surface-operations.json"));
}

#[test]
fn every_root_is_self_contained_and_has_a_complete_fixture_pair() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate must live below workspace root");

    for root in kanban_protocol::schema::schema_registry() {
        assert!(root.valid_fixture.starts_with("schemas/fixtures/"));
        assert!(root.invalid_fixture.starts_with("schemas/fixtures/"));
        assert_ne!(root.valid_fixture, root.invalid_fixture);
        assert!(
            repo_root.join(root.valid_fixture).is_file(),
            "missing positive fixture for {}",
            root.id
        );
        assert!(
            repo_root.join(root.invalid_fixture).is_file(),
            "missing negative fixture for {}",
            root.id
        );

        let artifact_path = repo_root
            .join(xtask::ARTIFACT_DIRECTORY)
            .join(root.artifact_path);
        let schema: Value =
            serde_json::from_slice(&std::fs::read(&artifact_path).unwrap_or_else(|error| {
                panic!("cannot read {}: {error}", artifact_path.display())
            }))
            .unwrap_or_else(|error| panic!("{} is not JSON: {error}", artifact_path.display()));
        assert_eq!(schema.get("$id"), Some(&Value::String(root.id.to_owned())));
        assert_eq!(
            schema.get("$schema"),
            Some(&Value::String(
                kanban_protocol::schema::DRAFT_2020_12.to_owned()
            ))
        );
        assert_local_references(&schema, root.id);
    }
}

fn assert_local_references(value: &Value, root_id: &str) {
    match value {
        Value::Object(object) => {
            if let Some(reference) = object.get("$ref") {
                let reference = reference
                    .as_str()
                    .unwrap_or_else(|| panic!("{root_id} contains a non-string $ref"));
                assert!(
                    reference.starts_with("#/$defs/"),
                    "{root_id} contains non-local $ref {reference}"
                );
            }
            for child in object.values() {
                assert_local_references(child, root_id);
            }
        }
        Value::Array(array) => {
            for child in array {
                assert_local_references(child, root_id);
            }
        }
        _ => {}
    }
}

#[test]
fn binary_help_preserves_public_cli_contract() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("--help")
        .output()
        .expect("schema tool binary should execute");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("help output must be UTF-8"),
        "用法：xtask <affected plan|json|run|self-test|docs check|schema generate|check|audit|web-contracts generate|check|web-assets check|deps check|agents check|tooling check|package cli> [--base REF] [--root PATH]\n用法：xtask web-assets check [--root PATH] [--dir PATH]\n"
    );
}

#[test]
fn web_assets_check_accepts_default_dir_and_prints_deterministic_summary() {
    let tree = TemporaryTree::new("default");
    let dist = create_artifact(tree.path(), "apps/web/dist", env!("CARGO_PKG_VERSION"));
    let output = run_web_assets(&[
        "web-assets",
        "check",
        "--root",
        tree.path().to_str().unwrap(),
    ]);

    assert!(
        output.status.success(),
        "checker should accept a fresh artifact at {}: {}",
        dist.display(),
        stderr(&output)
    );
    assert!(stdout(&output).contains("Web artifact 校验通过"));
    assert!(stdout(&output).contains(&format!("serverVersion={}", env!("CARGO_PKG_VERSION"))));
    assert!(stdout(&output).contains("buildId=sha256:"));
    assert!(stdout(&output).contains("files=2"));
    assert!(stdout(&output).contains("bytes=33"));
}

#[test]
fn web_assets_check_accepts_an_explicit_root_relative_dir() {
    let tree = TemporaryTree::new("explicit");
    create_artifact(tree.path(), "custom/dist", env!("CARGO_PKG_VERSION"));
    let output = run_web_assets(&[
        "web-assets",
        "check",
        "--root",
        tree.path().to_str().unwrap(),
        "--dir",
        "custom/dist",
    ]);

    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn web_assets_check_rejects_missing_tampered_and_server_version_mismatch() {
    let missing = TemporaryTree::new("missing");
    let missing_dist = create_artifact(missing.path(), "apps/web/dist", env!("CARGO_PKG_VERSION"));
    fs::remove_file(missing_dist.join("index.html")).expect("payload should be removable");
    let output = run_web_assets(&[
        "web-assets",
        "check",
        "--root",
        missing.path().to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("缺少 manifest 声明的 payload"));

    let tampered = TemporaryTree::new("tampered");
    let tampered_dist =
        create_artifact(tampered.path(), "apps/web/dist", env!("CARGO_PKG_VERSION"));
    fs::write(tampered_dist.join("index.html"), b"tampered").expect("payload should be writable");
    let output = run_web_assets(&[
        "web-assets",
        "check",
        "--root",
        tampered.path().to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("bytes/hash mismatch"));

    let mismatched = TemporaryTree::new("version-mismatch");
    let mismatched_version = mismatched_server_version();
    create_artifact(mismatched.path(), "apps/web/dist", &mismatched_version);
    let output = run_web_assets(&[
        "web-assets",
        "check",
        "--root",
        mismatched.path().to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("serverVersion 不匹配"));
}

#[test]
fn web_assets_check_rejects_unsafe_dir_and_root_paths() {
    let tree = TemporaryTree::new("unsafe");
    create_artifact(tree.path(), "apps/web/dist", env!("CARGO_PKG_VERSION"));

    for dir in ["/tmp/apps/web/dist", "./apps/web/dist", "apps/../web/dist"] {
        let output = run_web_assets(&[
            "web-assets",
            "check",
            "--root",
            tree.path().to_str().unwrap(),
            "--dir",
            dir,
        ]);
        assert!(!output.status.success(), "unsafe dir should fail: {dir}");
        assert!(
            stderr(&output).contains("root-relative")
                || stderr(&output).contains("regular path")
                || stderr(&output).contains("`.`"),
            "unexpected error for {dir}: {}",
            stderr(&output)
        );
    }

    let parent_root = format!(
        "{}/../{}",
        tree.path().display(),
        tree.path().file_name().unwrap().to_string_lossy()
    );
    let output = run_web_assets(&["web-assets", "check", "--root", &parent_root]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("parent traversal"));

    #[cfg(unix)]
    {
        let symlink_root = TemporaryTree::new("symlink-root");
        create_artifact(
            symlink_root.path(),
            "real/apps/web/dist",
            env!("CARGO_PKG_VERSION"),
        );
        std::os::unix::fs::symlink(
            symlink_root.path().join("real"),
            symlink_root.path().join("linked"),
        )
        .expect("root symlink should be creatable");
        let linked_root = symlink_root.path().join("linked");
        let output = run_web_assets(&[
            "web-assets",
            "check",
            "--root",
            linked_root.to_str().unwrap(),
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("symlink"));

        let symlink_dir = TemporaryTree::new("symlink-dir");
        create_artifact(
            symlink_dir.path(),
            "real/apps/web/dist",
            env!("CARGO_PKG_VERSION"),
        );
        fs::create_dir_all(symlink_dir.path().join("apps/web"))
            .expect("symlink parent should be creatable");
        std::os::unix::fs::symlink(
            symlink_dir.path().join("real/apps/web/dist"),
            symlink_dir.path().join("apps/web/dist"),
        )
        .expect("dist symlink should be creatable");
        let output = run_web_assets(&[
            "web-assets",
            "check",
            "--root",
            symlink_dir.path().to_str().unwrap(),
            "--dir",
            "apps/web/dist",
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("symlink"));
    }
}

#[test]
fn unknown_schema_commands_are_rejected() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["schema", "legacy"])
        .output()
        .expect("schema tool binary should execute");
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("错误输出必须是 UTF-8")
            .contains("未知 schema command")
    );
}

#[test]
fn unknown_web_contract_commands_are_rejected() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["web-contracts", "legacy"])
        .output()
        .expect("web contract tool binary should execute");
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("错误输出必须是 UTF-8")
            .contains("未知 web-contracts command")
    );
}

#[test]
fn web_contract_generation_is_selection_scoped() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate must live below workspace root");
    let files = xtask::web_contracts::expected_files(root).expect("selection should generate");
    let operations: Value = serde_json::from_slice(
        files
            .get("operations.json")
            .expect("generated operation manifest"),
    )
    .expect("operations manifest should be JSON");
    let operation_ids = operations
        .as_array()
        .expect("operations should be an array")
        .iter()
        .filter_map(|operation| operation.get("id").and_then(Value::as_str))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(operation_ids.contains("api.list-tasks"));
    assert!(operation_ids.contains("sse.stream-events"));
    assert!(!operation_ids.contains("api.create-board"));
    assert!(!operation_ids.contains("api.update-step"));
    assert!(!operation_ids.contains("api.list-task-labels"));
    let contracts: Value =
        serde_json::from_slice(files.get("contracts.json").expect("contracts manifest"))
            .expect("contracts manifest should be JSON");
    assert!(
        contracts
            .as_array()
            .expect("contracts should be an array")
            .iter()
            .any(|contract| contract.get("id")
                == Some(&Value::String("api.error.response".to_owned())))
    );
    assert!(files.contains_key("sse.ts"));
    assert!(files.contains_key("manifest.json"));
}

#[test]
fn decision_schema_accepts_missing_but_rejects_explicit_null() {
    let root = kanban_protocol::schema_registry()
        .iter()
        .find(|root| root.contract_id == "metadata.decision.input")
        .expect("decision root must exist");
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&kanban_protocol::schema::schema_document(root))
        .expect("decision schema must compile");
    let missing = serde_json::json!({
        "options": [{
            "slug": "typed-open",
            "title": "Typed open contract",
            "detail": "Known fields stay typed."
        }],
        "selected": "typed-open",
        "reason": "missing is valid"
    });
    assert!(validator.is_valid(&missing));

    for field in ["risk", "verification"] {
        let mut explicit_null = missing.clone();
        explicit_null
            .as_object_mut()
            .expect("fixture is an object")
            .insert(field.to_owned(), serde_json::Value::Null);
        assert!(
            !validator.is_valid(&explicit_null),
            "{field}=null 必须被 JSON Schema 拒绝"
        );
    }
}

#[test]
fn data_envelope_meta_rejects_untyped_value() {
    let payload = serde_json::json!({"data": {"ok": true}, "meta": {"arbitrary": true}});
    assert!(
        serde_json::from_value::<kanban_protocol::DataEnvelope<serde_json::Value>>(payload)
            .is_err()
    );
}
