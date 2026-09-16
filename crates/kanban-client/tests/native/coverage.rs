use kanban_protocol::rpc;
use std::{collections::BTreeSet, path::Path};

#[test]
fn every_formal_unary_request_has_one_named_native_client_call() {
    fn collect(directory: &Path, calls: &mut Vec<String>) {
        for file in std::fs::read_dir(directory).unwrap() {
            let path = file.unwrap().path();
            if path.is_dir() {
                collect(&path, calls);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(path).unwrap();
            for call in source.split("rpc!(").skip(1) {
                let request = call.split(',').nth(2).unwrap().trim().to_owned();
                calls.push(request);
            }
        }
    }
    let manifest: Vec<serde_json::Value> = serde_json::from_str(rpc::METHOD_MANIFEST).unwrap();
    let expected: BTreeSet<_> = manifest
        .iter()
        .map(|entry| entry["request"].as_str().unwrap().to_owned())
        .collect();
    let mut calls = Vec::new();
    collect(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/operations"),
        &mut calls,
    );
    let actual: BTreeSet<_> = calls.iter().cloned().collect();
    assert_eq!(calls.len(), actual.len(), "重复的具名 RPC 路径");
    assert_eq!(actual, expected, "新增正式操作必须贯通 client");
}
