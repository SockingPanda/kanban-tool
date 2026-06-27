mod common;

use anyhow::Context;
use common::{TempDb, kanban, kanban_in_dir_envs};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
fn write_executable(path: &std::path::Path, body: &str) -> anyhow::Result<()> {
    std::fs::write(path, body)?;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[test]
fn substrate_commands_report_entities_outbox_and_derived_status() -> anyhow::Result<()> {
    let temp = TempDb::new("substrate_commands_report_entities_outbox_and_derived_status")?;
    kanban(&temp.path, &["init"])?.success()?;

    let entities = kanban(
        &temp.path,
        &[
            "--json", "entity", "list", "--kind", "board", "--limit", "5",
        ],
    )?
    .success_json()?;
    let entity_rows = entities["data"].as_array().context("expected JSON array")?;
    assert_eq!(entity_rows.len(), 1);
    assert_eq!(entity_rows[0]["kind"], "board");
    let uri = entity_rows[0]["uri"]
        .as_str()
        .context("expected JSON string")?;
    assert!(uri.starts_with("kb://board/"));

    let shown = kanban(&temp.path, &["--json", "entity", "show", uri])?.success_json()?;
    assert_eq!(shown["data"]["uri"], uri);

    let outbox = kanban(&temp.path, &["--json", "outbox", "list"])?.success_json()?;
    assert_eq!(
        outbox["data"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        0
    );

    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "substrate task",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;
    let task_uri = format!("kb://task/{task_id}");
    let task_entity =
        kanban(&temp.path, &["--json", "entity", "show", &task_uri])?.success_json()?;
    assert_eq!(task_entity["data"]["title"], "substrate task");

    let outbox = kanban(&temp.path, &["--json", "outbox", "list"])?.success_json()?;
    let jobs = outbox["data"].as_array().context("expected JSON array")?;
    assert_eq!(jobs.len(), 3);
    let targets = jobs
        .iter()
        .map(|job| job["target"].as_str().context("expected JSON string"))
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(targets, vec!["tantivy", "oxigraph", "lancedb"]);
    assert!(jobs.iter().all(|job| job["entity_uri"] == task_uri));

    let derived = kanban(&temp.path, &["--json", "derived", "status"])?.success_json()?;
    let stores = derived["data"].as_array().context("expected JSON array")?;
    assert_eq!(stores.len(), 4);
    assert!(
        stores
            .iter()
            .any(|store| store["store_name"] == "tantivy_tasks")
    );
    assert!(
        stores
            .iter()
            .any(|store| store["store_name"] == "oxigraph_relations")
    );
    assert!(
        stores
            .iter()
            .any(|store| store["store_name"] == "lancedb_chunks")
    );
    assert!(
        stores
            .iter()
            .any(|store| store["store_name"] == "lancedb_label_atoms" && store["dirty"] == false)
    );
    Ok(())
}

#[test]
fn graph_vector_and_context_commands_report_disabled_fallbacks() -> anyhow::Result<()> {
    let temp = TempDb::new("graph_vector_and_context_commands_report_disabled_fallbacks")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "fallback context source",
            "--description",
            "ready spec context-needle",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    let missing_graph_helper = temp.dir.join("missing-graph-helper");
    let missing_vector_helper = temp.dir.join("missing-vector-helper");
    let graph = kanban_in_dir_envs(
        &temp.path,
        &["--json", "graph", "status"],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", missing_graph_helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(graph["data"]["backend"], "helper-missing");
    assert_eq!(graph["data"]["enabled"], false);
    assert!(
        graph["data"]["message"]
            .as_str()
            .context("expected JSON string")?
            .contains("graph helper unavailable")
    );

    kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "graph",
            "neighbors",
            &format!("kb://task/{task_id}"),
        ],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", missing_graph_helper.as_path())],
    )?
    .failure_containing("failed to run graph helper")?;

    let vector = kanban_in_dir_envs(
        &temp.path,
        &["--json", "vector", "status"],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", missing_vector_helper.as_path())],
    )?
    .success_json()?;
    assert_eq!(vector["data"]["backend"], "helper-missing");
    assert_eq!(vector["data"]["enabled"], false);
    assert!(
        vector["data"]["diagnostics"]
            .as_array()
            .context("expected diagnostics array")?
            .iter()
            .any(|value| value == "helper_missing")
    );

    let context = kanban(
        &temp.path,
        &[
            "--json",
            "context",
            "build",
            task_id,
            "--lexical-limit",
            "3",
        ],
    )?
    .success_json()?;
    assert_eq!(context["data"]["subject"], format!("kb://task/{task_id}"));
    assert!(
        context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "graph_disabled")
    );
    #[cfg(any())]
    assert!(
        !context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "graph_disabled")
    );
    assert!(
        context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "vector_disabled")
    );
    #[cfg(any())]
    assert!(
        context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "vector_disabled")
    );
    assert_eq!(
        context["data"]["items"][0]["entity_uri"],
        format!("kb://task/{task_id}")
    );
    assert_eq!(context["data"]["items"][0]["source"], "subject");
    assert!(
        context["data"]["items"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|item| item["entity_uri"] == format!("kb://task/{task_id}"))
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn context_build_uses_vector_helper_chunks_when_available() -> anyhow::Result<()> {
    let temp = TempDb::new("context_build_uses_vector_helper_chunks_when_available")?;
    kanban(&temp.path, &["init"])?.success()?;
    let subject = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "subject context helper",
            "--description",
            "needs helper context",
        ],
    )?
    .success_json()?;
    let subject_id = subject["data"]["id"].as_str().context("subject id")?;
    let related = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "vector helper related",
            "--description",
            "vector-only context item",
        ],
    )?
    .success_json()?;
    let related_id = related["data"]["id"].as_str().context("related id")?;
    let helper = temp.dir.join("vector-helper.py");
    write_executable(
        &helper,
        &format!(
            r#"#!/usr/bin/env python3
import json, sys
args = sys.argv[1:]
cmd = args[0]
if cmd == "status":
    payload = {{"backend":"test-vector-helper","enabled":True,"message":"ok","diagnostics":[]}}
elif cmd == "query-chunks":
    payload = [{{
        "chunk": {{"uri":"kb://chunk/task/{related_id}/0","entity_uri":"kb://task/{related_id}","ordinal":0,"content_hash":"hash"}},
        "score": 0.91,
        "text": "vector-only context item",
        "summary": "vector helper related"
    }}]
else:
    payload = []
print(json.dumps({{"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}}))
"#
        ),
    )?;

    let context = kanban_in_dir_envs(
        &temp.path,
        &[
            "--json",
            "context",
            "build",
            subject_id,
            "--vector-limit",
            "2",
        ],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", helper.as_path())],
    )?
    .success_json()?;

    assert!(
        context["data"]["items"]
            .as_array()
            .context("items")?
            .iter()
            .any(
                |item| item["entity_uri"] == format!("kb://task/{related_id}")
                    && item["source"] == "vector"
            )
    );
    assert!(
        !context["data"]["degraded"]
            .as_array()
            .context("degraded")?
            .iter()
            .any(|value| value == "vector_disabled")
    );
    Ok(())
}

#[test]
fn vector_status_reports_invalid_helper_json_as_degraded() -> anyhow::Result<()> {
    let temp = TempDb::new("vector_status_reports_invalid_helper_json_as_degraded")?;
    kanban(&temp.path, &["init"])?.success()?;
    let status = kanban_in_dir_envs(
        &temp.path,
        &["--json", "vector", "status"],
        &temp.dir,
        &[("KANBAN_VECTOR_HELPER", std::path::Path::new("/bin/echo"))],
    )?
    .success_json()?;
    assert_eq!(status["data"]["backend"], "helper-invalid");
    assert_eq!(status["data"]["enabled"], false);
    assert!(
        status["data"]["message"]
            .as_str()
            .context("expected JSON string")?
            .contains("invalid JSON envelope")
    );
    assert!(
        status["data"]["diagnostics"]
            .as_array()
            .context("expected diagnostics array")?
            .iter()
            .any(|value| value == "helper_invalid_envelope")
    );
    Ok(())
}

#[test]
fn graph_status_reports_invalid_helper_json_as_degraded() -> anyhow::Result<()> {
    let temp = TempDb::new("graph_status_reports_invalid_helper_json_as_degraded")?;
    kanban(&temp.path, &["init"])?.success()?;
    let status = kanban_in_dir_envs(
        &temp.path,
        &["--json", "graph", "status"],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", std::path::Path::new("/bin/echo"))],
    )?
    .success_json()?;
    assert_eq!(status["data"]["backend"], "helper-invalid");
    assert_eq!(status["data"]["enabled"], false);
    assert!(
        status["data"]["message"]
            .as_str()
            .context("expected JSON string")?
            .contains("invalid JSON envelope")
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn graph_status_preserves_helper_error_envelope() -> anyhow::Result<()> {
    let temp = TempDb::new("graph_status_preserves_helper_error_envelope")?;
    kanban(&temp.path, &["init"])?.success()?;
    let helper = temp.dir.join("helper-error.sh");
    std::fs::write(
        &helper,
        r#"#!/usr/bin/env bash
printf '%s\n' '{"protocol":"kanban-derived-helper.v1","payload_json":"{\"code\":\"bad_board\",\"message\":\"bad board\"}"}'
exit 1
"#,
    )?;
    let mut perms = std::fs::metadata(&helper)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&helper, perms)?;

    kanban_in_dir_envs(
        &temp.path,
        &["--json", "graph", "status"],
        &temp.dir,
        &[("KANBAN_GRAPH_HELPER", helper.as_path())],
    )?
    .failure_containing("graph helper failed: bad board (bad_board)")?;
    Ok(())
}

#[cfg(any())]
mod vector_lancedb {
    use super::*;

    struct StaticProvider {
        model: &'static str,
        dimensions: usize,
    }

    impl kanban_vector::EmbeddingProvider for StaticProvider {
        fn embedding_model(&self) -> &str {
            self.model
        }

        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn embed(&self, _text: &str) -> Result<Vec<f32>, kanban_vector::VectorError> {
            Ok(vec![0.0; self.dimensions])
        }
    }

    #[test]
    fn vector_configure_uses_global_project_and_explicit_config_precedence() -> anyhow::Result<()> {
        let temp =
            TempDb::new("vector_configure_uses_global_project_and_explicit_config_precedence")?;
        kanban(&temp.path, &["init"])?.success()?;
        let workspace = temp.dir.join("workspace");
        std::fs::create_dir_all(&workspace)?;
        let xdg_config_home = temp.dir.join("xdg-config");

        let configured = kanban_in_dir_envs(
            &temp.path,
            &[
                "--json",
                "vector",
                "configure",
                "--provider",
                "ollama",
                "--endpoint",
                "http://127.0.0.1:11434",
                "--model",
                "qwen3-embedding:0.6b",
                "--dimensions",
                "1024",
                "--skip-check",
            ],
            &workspace,
            &[("XDG_CONFIG_HOME", &xdg_config_home)],
        )?
        .success_json()?;
        assert_eq!(configured["data"]["provider"], "ollama");
        let global_config = xdg_config_home.join("kb/config.toml");
        let config = std::fs::read_to_string(&global_config)?;
        assert!(config.contains("[vector]"));
        assert!(config.contains("model = \"qwen3-embedding:0.6b\""));

        let status = kanban_in_dir_envs(
            &temp.path,
            &["--json", "vector", "status"],
            &workspace,
            &[("XDG_CONFIG_HOME", &xdg_config_home)],
        )?
        .success_json()?;
        assert_eq!(status["data"]["backend"], "lancedb");
        assert_eq!(status["data"]["enabled"], true);
        assert!(
            status["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("qwen3-embedding:0.6b")
        );

        std::fs::create_dir_all(workspace.join(".kb"))?;
        std::fs::write(
            workspace.join(".kb/config.toml"),
            r#"[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "project-model"
dimensions = 1024
"#,
        )?;
        let project_status = kanban_in_dir_envs(
            &temp.path,
            &["--json", "vector", "status"],
            &workspace,
            &[("XDG_CONFIG_HOME", &xdg_config_home)],
        )?
        .success_json()?;
        assert!(
            project_status["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("project-model")
        );

        let explicit_config = temp.dir.join("explicit-vector.toml");
        std::fs::write(
            &explicit_config,
            r#"[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:11434"
model = "explicit-model"
dimensions = 1024
"#,
        )?;
        let explicit_arg = explicit_config.to_string_lossy().to_string();
        let explicit_status = kanban_in_dir_envs(
            &temp.path,
            &[
                "--json",
                "vector",
                "status",
                "--vector-config",
                &explicit_arg,
            ],
            &workspace,
            &[("XDG_CONFIG_HOME", &xdg_config_home)],
        )?
        .success_json()?;
        assert!(
            explicit_status["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("explicit-model")
        );
        Ok(())
    }

    #[test]
    fn context_build_degrades_when_configured_vector_store_construction_fails() -> anyhow::Result<()>
    {
        let temp =
            TempDb::new("context_build_degrades_when_configured_vector_store_construction_fails")?;
        kanban(&temp.path, &["init"])?.success()?;
        let created = kanban(
            &temp.path,
            &[
                "--json",
                "task",
                "create",
                "schema mismatch context",
                "--description",
                "ready spec schema mismatch needle",
            ],
        )?
        .success_json()?;
        let task_id = created["data"]["id"]
            .as_str()
            .context("expected JSON string")?;

        let provider = std::sync::Arc::new(StaticProvider {
            model: "static-test",
            dimensions: 2,
        });
        let _store = kanban_vector_lancedb::LanceDbStore::connect(
            kanban_vector_lancedb::LanceDbConfig::new(
                kanban_local::vector_store_path(temp.path.clone()),
                provider,
            ),
        )
        .context("seed 2-dimensional LanceDB table")?;

        let vector_config = temp.dir.join("mismatched-vector.toml");
        std::fs::write(
            &vector_config,
            r#"[vector]
provider = "ollama"
endpoint = "http://127.0.0.1:1"
model = "offline-test-model"
dimensions = 3
"#,
        )?;
        let vector_config_arg = vector_config.to_string_lossy().to_string();

        let status = kanban(
            &temp.path,
            &[
                "--json",
                "vector",
                "status",
                "--vector-config",
                &vector_config_arg,
            ],
        )?
        .success_json()?;
        assert_eq!(status["data"]["enabled"], true);
        assert!(
            status["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("offline-test-model")
        );
        assert!(
            status["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("http://127.0.0.1:1")
        );

        let context = kanban(
            &temp.path,
            &[
                "--json",
                "context",
                "build",
                task_id,
                "--vector-config",
                &vector_config_arg,
            ],
        )?
        .success_json()?;
        assert_eq!(context["data"]["subject"], format!("kb://task/{task_id}"));
        assert!(
            context["data"]["degraded"]
                .as_array()
                .context("expected JSON array")?
                .contains(&serde_json::json!("vector_error"))
        );
        assert!(
            context["data"]["diagnostics"]
                .as_array()
                .context("expected JSON array")?
                .iter()
                .any(|diagnostic| diagnostic["source"] == "vector"
                    && diagnostic["code"] == "vector_error")
        );
        assert_eq!(context["data"]["items"][0]["source"], "subject");
        Ok(())
    }
}

#[test]
fn context_build_command_rejects_zero_max_items() -> anyhow::Result<()> {
    let temp = TempDb::new("context_build_command_rejects_zero_max_items")?;
    kanban(&temp.path, &["init"])?.success()?;
    let created = kanban(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "zero budget context",
            "--description",
            "ready spec",
        ],
    )?
    .success_json()?;
    let task_id = created["data"]["id"]
        .as_str()
        .context("expected JSON string")?;

    kanban(
        &temp.path,
        &["context", "build", task_id, "--max-items", "0"],
    )?
    .failure_containing("max_items must be >= 1")?;
    Ok(())
}
