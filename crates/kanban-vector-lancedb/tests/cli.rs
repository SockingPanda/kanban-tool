use std::process::Command;

#[test]
fn vector_json_error_includes_invalid_element_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_kanban-vector-lancedb"))
        .args([
            "query-label-atoms",
            "--db",
            "/tmp/kanban-vector-lancedb-path-diagnostics.db",
            "--board",
            "kanban-tool",
            "--vector-json",
            r#"[1.0,"bad"]"#,
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("invalid --vector-json payload"), "{stdout}");
    assert!(stdout.contains("[1]"), "{stdout}");
}
