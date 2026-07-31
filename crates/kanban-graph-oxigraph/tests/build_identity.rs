use std::process::Command;

use kanban_graph_oxigraph::graph_helper_build_identity;

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
fn helper_reports_its_exact_compile_time_identity() {
    let expected = expected_build_identity();
    assert_eq!(graph_helper_build_identity(), expected);

    let output = Command::new(env!("CARGO_BIN_EXE_kanban-graph-oxigraph"))
        .arg("__build-identity")
        .output()
        .expect("run the real Oxigraph helper");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn build_identity_command_is_hidden_from_operator_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_kanban-graph-oxigraph"))
        .arg("--help")
        .output()
        .expect("run the real Oxigraph helper");
    assert!(output.status.success());
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("__build-identity"),
        "release identity probe must remain an internal compatibility command"
    );
}
