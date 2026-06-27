use std::os::unix::fs::PermissionsExt;

use crate::common::*;

fn write_helper(
    dir: &std::path::Path,
    name: &str,
    body: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let path = dir.join(name);
    std::fs::write(&path, body)?;
    let mut permissions = std::fs::metadata(&path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions)?;
    Ok(path)
}

fn json_helper_body(kind: &str, log: &std::path::Path) -> String {
    let log_path = log.display().to_string();
    format!(
        r#"#!/usr/bin/env python3
import json, pathlib, sys
log = pathlib.Path({log_path:?})
args = sys.argv[1:]
log.write_text(json.dumps(args))
cmd = args[0] if args else ""
if cmd == "status":
    if {kind:?} == "vector":
        payload = {{"backend":"test-vector-helper","enabled":True,"message":"vector helper ok","diagnostics":["test_helper"],"dirty":False,"board_dirty":False}}
    else:
        payload = {{"backend":"test-graph-helper","enabled":True,"message":"graph helper ok"}}
elif cmd == "neighbors":
    payload = []
elif cmd == "query-label-atoms":
    payload = []
else:
    payload = {{"ok": True}}
print(json.dumps({{"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}}))
"#
    )
}

#[tokio::test]
async fn vector_status_degrades_when_helper_is_missing() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let missing = test.dir_path().join("missing-vector-helper");
    let app =
        build_router(AppState::new(test.db_path(), "api-test").with_vector_helper_path(missing));

    let (status, json) = get_json(app, "/api/v1/vector/status?board=default").await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], false);
    assert_eq!(json["data"]["backend"], "helper-missing");
    assert!(
        json["data"]["diagnostics"]
            .as_array()
            .context("diagnostics")?
            .contains(&json!("helper_missing"))
    );
    Ok(())
}

#[tokio::test]
async fn vector_status_rejects_invalid_helper_json_without_500() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let helper = write_helper(
        test.dir_path(),
        "bad-vector-helper",
        "#!/bin/sh\necho not-json\n",
    )?;
    let app =
        build_router(AppState::new(test.db_path(), "api-test").with_vector_helper_path(helper));

    let (status, json) = get_json(app, "/api/v1/vector/status?board=default").await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["enabled"], false);
    assert_eq!(json["data"]["backend"], "helper-invalid");
    assert!(
        json["data"]["diagnostics"]
            .as_array()
            .context("diagnostics")?
            .contains(&json!("helper_invalid_envelope"))
    );
    Ok(())
}

#[tokio::test]
async fn vector_and_label_atom_endpoints_use_vector_helper() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let log = test.dir_path().join("vector-args.json");
    let helper = write_helper(
        test.dir_path(),
        "vector-helper",
        &json_helper_body("vector", &log),
    )?;
    let app =
        build_router(AppState::new(test.db_path(), "api-test").with_vector_helper_path(helper));

    let (status, json) = get_json(app.clone(), "/api/v1/vector/status?board=default").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["backend"], "test-vector-helper");
    let args: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&log)?)?;
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--db" && pair[1] == test.db_path().to_str().unwrap())
    );
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--board" && pair[1] == "default")
    );

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/labels/atom-index/query?q=hello&polarity=positive&limit=3",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"].as_array().context("hits")?.len(), 0);
    let args: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&log)?)?;
    assert_eq!(args[0], "query-label-atoms");
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--text" && pair[1] == "hello")
    );
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--polarity" && pair[1] == "positive")
    );

    let (status, json) = get_json(
        app.clone(),
        "/api/v1/boards/default/labels/atom-index/query?vector_json=%5B1.0%2C0.0%5D&embedding_model=review-model&include_vector=true&polarity=positive&limit=2",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"].as_array().context("hits")?.len(), 0);
    let args: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&log)?)?;
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--vector-json" && pair[1] == "[1.0,0.0]")
    );
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--embedding-model" && pair[1] == "review-model")
    );
    assert!(args.iter().any(|arg| arg == "--include-vector"));

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/labels/atom-index/rebuild",
        json!({}),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("server helper adapter")
    );
    Ok(())
}

#[tokio::test]
async fn graph_status_and_neighbors_use_graph_helper() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let log = test.dir_path().join("graph-args.json");
    let helper = write_helper(
        test.dir_path(),
        "graph-helper",
        &json_helper_body("graph", &log),
    )?;
    let app =
        build_router(AppState::new(test.db_path(), "api-test").with_graph_helper_path(helper));

    let (status, json) = get_json(app.clone(), "/api/v1/graph/status?board=default").await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["backend"], "test-graph-helper");

    let (status, json) = get_json(
        app,
        "/api/v1/graph/neighbors?board=default&entity_uri=kb%3A%2F%2Ftask%2Ft_test&predicate=depends_on&limit=2",
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"].as_array().context("neighbors")?.len(), 0);
    let args: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&log)?)?;
    assert_eq!(args[0], "neighbors");
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--entity-uri" && pair[1] == "kb://task/t_test")
    );
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--predicate" && pair[1] == "depends_on")
    );
    Ok(())
}

#[tokio::test]
async fn label_atom_rebuild_does_not_treat_malformed_helper_output_as_degraded_success()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let helper = write_helper(
        test.dir_path(),
        "bad-vector-helper-rebuild",
        "#!/bin/sh\necho not-json\n",
    )?;
    let app =
        build_router(AppState::new(test.db_path(), "api-test").with_vector_helper_path(helper));

    let (status, json) = post_json(
        app,
        "/api/v1/boards/default/labels/atom-index/rebuild",
        json!({}),
    )
    .await?;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("error message")?
            .contains("server helper adapter")
    );
    Ok(())
}

#[tokio::test]
async fn graph_neighbors_malformed_helper_output_is_server_error() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let helper = write_helper(
        test.dir_path(),
        "bad-graph-helper",
        "#!/bin/sh\necho not-json\n",
    )?;
    let app =
        build_router(AppState::new(test.db_path(), "api-test").with_graph_helper_path(helper));

    let (status, json) = get_json(
        app,
        "/api/v1/graph/neighbors?board=default&entity_uri=kb%3A%2F%2Ftask%2Ft_test&limit=2",
    )
    .await?;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"]["code"], "internal");
    assert_ne!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("invalid JSON envelope")
    );
    Ok(())
}

#[tokio::test]
async fn label_atom_query_malformed_helper_output_is_server_error() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let helper = write_helper(
        test.dir_path(),
        "bad-vector-helper-query",
        "#!/bin/sh\necho not-json\n",
    )?;
    let app =
        build_router(AppState::new(test.db_path(), "api-test").with_vector_helper_path(helper));

    let (status, json) = get_json(
        app,
        "/api/v1/boards/default/labels/atom-index/query?q=hello&limit=1",
    )
    .await?;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"]["code"], "internal");
    assert_ne!(json["error"]["code"], "invalid_input");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("invalid JSON envelope")
    );
    Ok(())
}

#[tokio::test]
async fn helper_error_payload_maps_to_api_error() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let helper = write_helper(
        test.dir_path(),
        "failing-vector-helper",
        r#"#!/usr/bin/env python3
import json, sys
payload = {"code":"helper_error","message":"boom"}
print(json.dumps({"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}))
sys.exit(1)
"#,
    )?;
    let app =
        build_router(AppState::new(test.db_path(), "api-test").with_vector_helper_path(helper));

    let (status, json) = get_json(
        app,
        "/api/v1/boards/default/labels/atom-index/query?q=hello&limit=1",
    )
    .await?;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"]["code"], "internal");
    assert!(
        json["error"]["message"]
            .as_str()
            .context("message")?
            .contains("boom")
    );
    Ok(())
}
