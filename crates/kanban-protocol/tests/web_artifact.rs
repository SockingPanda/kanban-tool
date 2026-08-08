use kanban_protocol::{
    WEB_ARTIFACT_BASE_PATH, WEB_ARTIFACT_ENTRYPOINT, WEB_ARTIFACT_FORMAT_VERSION,
    WEB_PROTOCOL_VERSION, WebArtifactFile, WebArtifactManifest, validate_web_artifact_manifest,
    web_artifact_build_id_for, web_artifact_file_from_bytes, web_artifact_sha256_for_bytes,
};

fn file(path: &str, bytes: u64, sha256: &str) -> WebArtifactFile {
    WebArtifactFile {
        path: path.to_owned(),
        bytes,
        sha256: sha256.to_owned(),
    }
}

fn sample_manifest() -> WebArtifactManifest {
    WebArtifactManifest {
        format_version: WEB_ARTIFACT_FORMAT_VERSION,
        base_path: WEB_ARTIFACT_BASE_PATH.to_owned(),
        entrypoint: WEB_ARTIFACT_ENTRYPOINT.to_owned(),
        server_version: "3.0.0".to_owned(),
        protocol_version: WEB_PROTOCOL_VERSION.to_owned(),
        build_id: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            .to_owned(),
        files: vec![
            file(
                "assets/app.js",
                3,
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ),
            file(
                "index.html",
                2,
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
        ],
    }
}

#[test]
fn known_vector_freezes_build_id_and_preimage_order() {
    let manifest = sample_manifest();
    let preimage = kanban_protocol::web_artifact_build_preimage(
        manifest.format_version,
        &manifest.base_path,
        &manifest.entrypoint,
        &manifest.server_version,
        &manifest.protocol_version,
        &manifest.files,
    )
    .unwrap();
    assert_eq!(
        preimage
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "00000000000000246b616e62616e2d746f6f6c3a7765622d61727469666163742d6275696c642d69643a7631000000000000000100000000000000052f6170702f000000000000000a696e6465782e68746d6c0000000000000005332e302e30000000000000000276310000000000000002000000000000000d6173736574732f6170702e6a730000000000000003aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa000000000000000a696e6465782e68746d6c0000000000000002bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    assert_eq!(
        web_artifact_build_id_for(
            manifest.format_version,
            &manifest.base_path,
            &manifest.entrypoint,
            &manifest.server_version,
            &manifest.protocol_version,
            &manifest.files,
        )
        .unwrap(),
        "sha256:ce7b387aff6a614f4e376260a8edbd1341148d932df90db96dd00bce038f44a7",
    );
}

#[test]
fn validation_requires_sorted_safe_files_and_index_entrypoint() {
    let mut manifest = sample_manifest();
    manifest.build_id = web_artifact_build_id_for(
        manifest.format_version,
        &manifest.base_path,
        &manifest.entrypoint,
        &manifest.server_version,
        &manifest.protocol_version,
        &manifest.files,
    )
    .unwrap();
    validate_web_artifact_manifest(&manifest).unwrap();

    manifest.files.swap(0, 1);
    assert!(validate_web_artifact_manifest(&manifest).is_err());

    let mut unsafe_path = sample_manifest();
    unsafe_path.files[0].path = "../secret.js".to_owned();
    assert!(validate_web_artifact_manifest(&unsafe_path).is_err());

    for path in [
        "",
        "assets/../secret.js",
        "assets/./bundle.js",
        "assets\\bundle.js",
        "assets/app bundle.js",
        "assets/%2e%2e/secret.js",
        "assets/雪.js",
        "/absolute.js",
    ] {
        let mut invalid = sample_manifest();
        invalid.files[0].path = path.to_owned();
        assert!(
            validate_web_artifact_manifest(&invalid).is_err(),
            "path should be rejected: {path:?}"
        );
    }
}

#[test]
fn validation_rejects_manifest_self_listing_duplicates_and_bad_digest() {
    let mut manifest = sample_manifest();
    manifest.files.push(file(
        "manifest.json",
        1,
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    ));
    manifest.files.sort_by(|a, b| a.path.cmp(&b.path));
    assert!(validate_web_artifact_manifest(&manifest).is_err());

    let mut duplicate = sample_manifest();
    duplicate.files.push(duplicate.files[0].clone());
    duplicate.files.sort_by(|a, b| a.path.cmp(&b.path));
    assert!(validate_web_artifact_manifest(&duplicate).is_err());

    let mut bad_digest = sample_manifest();
    bad_digest.files[0].sha256 = "not-a-digest".to_owned();
    bad_digest.build_id =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000".to_owned();
    assert!(validate_web_artifact_manifest(&bad_digest).is_err());
}

#[test]
fn serde_shape_is_camel_case_and_strict() {
    let manifest = sample_manifest();
    let value = serde_json::to_value(&manifest).unwrap();
    assert_eq!(value["formatVersion"], 1);
    assert_eq!(value["basePath"], "/app/");
    assert!(value.get("format_version").is_none());
    assert!(
        serde_json::from_value::<WebArtifactManifest>(serde_json::json!({
            "formatVersion": 1,
            "basePath": "/app/",
            "entrypoint": "index.html",
            "serverVersion": "3.0.0",
            "protocolVersion": "v1",
            "buildId": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "files": [],
            "extra": true
        }))
        .is_err()
    );
}

#[test]
fn protocol_version_is_wire_generation_and_matches_runtime_fixture() {
    assert_eq!(WEB_PROTOCOL_VERSION, "v1");
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/fixtures/runtime/web-config.v1.valid.json");
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&fixture_path).expect("runtime fixture should be readable"),
    )
    .expect("runtime fixture should be valid JSON");
    assert_eq!(fixture["protocolVersion"], WEB_PROTOCOL_VERSION);

    let mut manifest = sample_manifest();
    manifest.protocol_version = "3.0.0".to_owned();
    assert!(
        web_artifact_build_id_for(
            manifest.format_version,
            &manifest.base_path,
            &manifest.entrypoint,
            &manifest.server_version,
            &manifest.protocol_version,
            &manifest.files,
        )
        .is_err()
    );
}

#[test]
fn file_helper_freezes_bytes_length_and_sha256_format() {
    let file = web_artifact_file_from_bytes("assets/app.js", b"abc").unwrap();
    assert_eq!(file.path, "assets/app.js");
    assert_eq!(file.bytes, 3);
    assert_eq!(
        file.sha256,
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        web_artifact_sha256_for_bytes(b"abc"),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(web_artifact_file_from_bytes("manifest.json", b"root").is_err());
}
