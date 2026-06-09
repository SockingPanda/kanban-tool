mod common;

use anyhow::Context;
use common::{TempDb, kb};
#[test]
fn substrate_commands_report_entities_outbox_and_derived_status() -> anyhow::Result<()> {
    let temp = TempDb::new("substrate_commands_report_entities_outbox_and_derived_status")?;
    kb(&temp.path, &["init"])?.success()?;

    let entities = kb(
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

    let shown = kb(&temp.path, &["--json", "entity", "show", uri])?.success_json()?;
    assert_eq!(shown["data"]["uri"], uri);

    let outbox = kb(&temp.path, &["--json", "outbox", "list"])?.success_json()?;
    assert_eq!(
        outbox["data"]
            .as_array()
            .context("expected JSON array")?
            .len(),
        0
    );

    let created = kb(
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
    let task_entity = kb(&temp.path, &["--json", "entity", "show", &task_uri])?.success_json()?;
    assert_eq!(task_entity["data"]["title"], "substrate task");

    let outbox = kb(&temp.path, &["--json", "outbox", "list"])?.success_json()?;
    let jobs = outbox["data"].as_array().context("expected JSON array")?;
    assert_eq!(jobs.len(), 3);
    let targets = jobs
        .iter()
        .map(|job| job["target"].as_str().context("expected JSON string"))
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(targets, vec!["tantivy", "oxigraph", "lancedb"]);
    assert!(jobs.iter().all(|job| job["entity_uri"] == task_uri));

    let derived = kb(&temp.path, &["--json", "derived", "status"])?.success_json()?;
    let stores = derived["data"].as_array().context("expected JSON array")?;
    assert_eq!(stores.len(), 3);
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
    Ok(())
}

#[test]
fn graph_vector_and_context_commands_report_disabled_fallbacks() -> anyhow::Result<()> {
    let temp = TempDb::new("graph_vector_and_context_commands_report_disabled_fallbacks")?;
    kb(&temp.path, &["init"])?.success()?;
    let created = kb(
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

    let graph = kb(&temp.path, &["--json", "graph", "status"])?.success_json()?;
    #[cfg(not(feature = "graph-oxigraph"))]
    {
        assert_eq!(graph["data"]["backend"], "disabled");
        assert_eq!(graph["data"]["enabled"], false);

        let neighbors = kb(
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

        let rebuilt = kb(&temp.path, &["--json", "graph", "rebuild"])?.success_json()?;
        assert_eq!(rebuilt["data"]["backend"], "oxigraph");
        assert_eq!(rebuilt["data"]["enabled"], true);

        let neighbors = kb(
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

        let query = kb(
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

    let vector = kb(&temp.path, &["--json", "vector", "status"])?.success_json()?;
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

    let context = kb(
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

    #[test]
    fn vector_status_reports_lancedb_degraded_without_embedding_provider() -> anyhow::Result<()> {
        let temp =
            TempDb::new("vector_status_reports_lancedb_degraded_without_embedding_provider")?;
        kb(&temp.path, &["init"])?.success()?;
        kb(
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

        let status = kb(&temp.path, &["--json", "vector", "status"])?.success_json()?;
        assert_eq!(status["data"]["backend"], "lancedb");
        assert_eq!(status["data"]["enabled"], false);
        assert!(
            status["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("without an embedding provider")
        );
        assert!(
            status["data"]["message"]
                .as_str()
                .context("expected JSON string")?
                .contains("dirty=true")
        );
        Ok(())
    }
}

#[test]
fn context_build_command_rejects_zero_max_items() -> anyhow::Result<()> {
    let temp = TempDb::new("context_build_command_rejects_zero_max_items")?;
    kb(&temp.path, &["init"])?.success()?;
    let created = kb(
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

    kb(
        &temp.path,
        &["context", "build", task_id, "--max-items", "0"],
    )?
    .failure_containing("max_items must be >= 1")?;
    Ok(())
}
