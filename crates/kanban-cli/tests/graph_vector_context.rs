mod common;

use anyhow::Context;
#[cfg(feature = "vector-lancedb")]
use common::kanban_in_dir_envs;
use common::{TempDb, kanban};
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

    let graph = kanban(&temp.path, &["--json", "graph", "status"])?.success_json()?;
    #[cfg(not(feature = "graph-oxigraph"))]
    {
        assert_eq!(graph["data"]["backend"], "disabled");
        assert_eq!(graph["data"]["enabled"], false);

        let neighbors = kanban(
            &temp.path,
            &[
                "--json",
                "graph",
                "neighbors",
                &format!("kb://task/{task_id}"),
            ],
        )?
        .success_json()?;
        assert_eq!(
            neighbors["data"]
                .as_array()
                .context("expected JSON array")?
                .len(),
            0
        );
    }
    #[cfg(feature = "graph-oxigraph")]
    {
        let board_id = created["data"]["board_id"]
            .as_str()
            .context("expected JSON string")?;
        assert_eq!(graph["data"]["backend"], "oxigraph");
        assert_eq!(graph["data"]["enabled"], true);

        let rebuilt = kanban(&temp.path, &["--json", "graph", "rebuild"])?.success_json()?;
        assert_eq!(rebuilt["data"]["backend"], "oxigraph");
        assert_eq!(rebuilt["data"]["enabled"], true);

        let neighbors = kanban(
            &temp.path,
            &[
                "--json",
                "graph",
                "neighbors",
                &format!("kb://task/{task_id}"),
            ],
        )?
        .success_json()?;
        assert!(
            neighbors["data"]
                .as_array()
                .context("expected JSON array")?
                .iter()
                .any(|relation| relation["predicate"] == "belongs_to_board"
                    && relation["object_uri"] == format!("kb://board/{board_id}"))
        );

        let query = kanban(
            &temp.path,
            &[
                "--json",
                "graph",
                "query",
                &format!(
                    "SELECT ?board WHERE {{ GRAPH ?g {{ <kb://task/{task_id}> <kb://predicate/belongs_to_board> ?board }} }}"
                ),
            ],
        )?
        .success_json()?;
        assert_eq!(
            query["data"]
                .as_array()
                .context("expected JSON array")?
                .len(),
            1
        );
    }

    let vector = kanban(&temp.path, &["--json", "vector", "status"])?.success_json()?;
    #[cfg(not(feature = "vector-lancedb"))]
    {
        assert_eq!(vector["data"]["backend"], "disabled");
        assert_eq!(vector["data"]["enabled"], false);
    }
    #[cfg(feature = "vector-lancedb")]
    {
        assert_eq!(vector["data"]["backend"], "lancedb");
        assert_eq!(vector["data"]["enabled"], false);
        assert!(
            vector["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("without an embedding provider")
        );
    }

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
    #[cfg(not(feature = "graph-oxigraph"))]
    assert!(
        context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "graph_disabled")
    );
    #[cfg(feature = "graph-oxigraph")]
    assert!(
        !context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "graph_disabled")
    );
    #[cfg(not(feature = "vector-lancedb"))]
    assert!(
        context["data"]["degraded"]
            .as_array()
            .context("expected JSON array")?
            .iter()
            .any(|value| value == "vector_disabled")
    );
    #[cfg(feature = "vector-lancedb")]
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

#[cfg(feature = "vector-lancedb")]
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
    fn vector_status_reports_lancedb_degraded_without_embedding_provider() -> anyhow::Result<()> {
        let temp =
            TempDb::new("vector_status_reports_lancedb_degraded_without_embedding_provider")?;
        kanban(&temp.path, &["init"])?.success()?;
        kanban(
            &temp.path,
            &[
                "task",
                "create",
                "degraded vector source",
                "--description",
                "ready spec",
            ],
        )?
        .success()?;

        let status = kanban(&temp.path, &["--json", "vector", "status"])?.success_json()?;
        assert_eq!(status["data"]["backend"], "lancedb");
        assert_eq!(status["data"]["enabled"], false);
        assert!(
            status["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("without an embedding provider")
        );
        assert_eq!(status["data"]["dirty"], true);
        assert_eq!(status["data"]["board_dirty"], true);
        assert!(
            status["data"]["diagnostics"]
                .as_array()
                .context("expected diagnostics array")?
                .iter()
                .any(|code| code == "vector_dirty")
        );
        Ok(())
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
