use std::process::Command;

use kanban_protocol::VectorProjectionHelperResponse;
use kanban_vector_lancedb::{vector_helper_build_identity, vector_projection_descriptor_response};

fn expected_build_identity() -> &'static str {
    match option_env!("KANBAN_BUILD_ID") {
        Some(build_id) => build_id,
        None => concat!(
            "dev:",
            env!("CARGO_PKG_NAME"),
            "@",
            env!("CARGO_PKG_VERSION")
        ),
    }
}

#[test]
fn helper_reports_the_exact_compile_time_identity_used_by_projection_descriptors() {
    let expected = expected_build_identity();
    assert_eq!(vector_helper_build_identity(), expected);

    let output = Command::new(env!("CARGO_BIN_EXE_kanban-vector-lancedb"))
        .arg("__build-identity")
        .output()
        .expect("run the real LanceDB helper");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected.as_bytes());

    let response = vector_projection_descriptor_response("req_build_identity");
    let VectorProjectionHelperResponse::Descriptor(descriptor) = response else {
        panic!("expected descriptor response");
    };
    assert_eq!(descriptor.build_identity, expected);
}

#[test]
fn build_identity_command_is_hidden_from_operator_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_kanban-vector-lancedb"))
        .arg("--help")
        .output()
        .expect("run the real LanceDB helper");
    assert!(output.status.success());
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("__build-identity"),
        "release identity probe must remain an internal compatibility command"
    );
}
