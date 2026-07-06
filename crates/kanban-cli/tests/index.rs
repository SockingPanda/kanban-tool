mod common;

use common::{TempDb, kanban};

#[cfg(feature = "tantivy-backend")]
mod tantivy_backend {
    use super::*;
    use anyhow::Context;

    #[test]
    fn index_rebuild_enables_tantivy_search_backend() -> anyhow::Result<()> {
        let temp = TempDb::new("index_rebuild_enables_tantivy_search_backend")?;
        kanban(&temp.path, &["init"])?.success()?;
        kanban(
            &temp.path,
            &[
                "task",
                "create",
                "cli tantivy comet",
                "--description",
                "ready spec",
            ],
        )?
        .success()?;

        let rebuilt = kanban(&temp.path, &["--json", "index", "rebuild"])?.success_json()?;
        assert_eq!(rebuilt["data"]["backend"], "tantivy");
        assert_eq!(rebuilt["data"]["derived_index"], true);
        assert!(temp.dir.join("index/v1/tasks").exists());

        let search = kanban(&temp.path, &["--json", "search", "comet"])?.success_json()?;
        assert_eq!(search["meta"]["backend"], "tantivy");
        assert!(search["data"].get("meta").is_none(), "{search}");
        assert_eq!(
            search["data"]["hits"][0]["task"]["title"],
            "cli tantivy comet"
        );
        Ok(())
    }

    #[test]
    fn index_sync_refreshes_stale_tantivy_search_backend() -> anyhow::Result<()> {
        let temp = TempDb::new("index_sync_refreshes_stale_tantivy_search_backend")?;
        kanban(&temp.path, &["init"])?.success()?;
        let created = kanban(
            &temp.path,
            &[
                "--json",
                "task",
                "create",
                "cli sync source",
                "--description",
                "ready spec",
            ],
        )?
        .success_json()?;
        let task_id = created["data"]["id"]
            .as_str()
            .context("expected JSON string")?;
        kanban(&temp.path, &["index", "rebuild"])?.success()?;
        kanban(
            &temp.path,
            &[
                "task",
                "update",
                task_id,
                "--title",
                "cli sync comet",
                "--expected-lock-version",
                created["data"]["lock_version"]
                    .as_i64()
                    .context("expected JSON i64")?
                    .to_string()
                    .as_str(),
            ],
        )?
        .success()?;

        let stale = kanban(&temp.path, &["--json", "index", "status"])?.success_json()?;
        assert_eq!(stale["data"]["backend"], "tantivy");
        assert_eq!(stale["data"]["stale"], true);
        assert!(
            stale["data"]["index_lag_events"]
                .as_i64()
                .context("expected JSON i64")?
                > 0
        );

        let synced = kanban(&temp.path, &["--json", "index", "sync"])?.success_json()?;
        assert_eq!(synced["data"]["backend"], "tantivy");
        assert_eq!(synced["data"]["stale"], false);
        assert_eq!(synced["data"]["index_lag_events"], 0);

        let search = kanban(&temp.path, &["--json", "search", "comet"])?.success_json()?;
        assert_eq!(search["meta"]["backend"], "tantivy");
        assert!(search["data"].get("meta").is_none(), "{search}");
        assert_eq!(search["data"]["hits"][0]["task"]["title"], "cli sync comet");
        Ok(())
    }
}

#[test]
fn index_commands_report_sqlite_fallback_backend() -> anyhow::Result<()> {
    let temp = TempDb::new("index_commands_report_sqlite_fallback_backend")?;
    kanban(&temp.path, &["init"])?.success()?;

    for command in ["status", "doctor"] {
        let json = kanban(&temp.path, &["--json", "index", command])?.success_json()?;
        assert_eq!(json["data"]["backend"], "sqlite");
        assert_eq!(json["data"]["derived_index"], false);
        assert_eq!(json["data"]["stale"], false);
    }

    #[cfg(not(feature = "tantivy-backend"))]
    {
        let json = kanban(&temp.path, &["--json", "index", "rebuild"])?.success_json()?;
        assert_eq!(json["data"]["backend"], "sqlite");
        assert_eq!(json["data"]["derived_index"], false);
        assert_eq!(json["data"]["stale"], false);

        let json = kanban(&temp.path, &["--json", "index", "sync"])?.success_json()?;
        assert_eq!(json["data"]["backend"], "sqlite");
        assert_eq!(json["data"]["derived_index"], false);
        assert_eq!(json["data"]["stale"], false);
    }

    #[cfg(not(feature = "tantivy-backend"))]
    let human = kanban(&temp.path, &["index", "rebuild"])?;
    #[cfg(not(feature = "tantivy-backend"))]
    assert!(human.output.status.success());
    #[cfg(not(feature = "tantivy-backend"))]
    let stdout = String::from_utf8_lossy(&human.output.stdout);
    #[cfg(not(feature = "tantivy-backend"))]
    assert!(stdout.contains("SQLite fallback"), "{stdout}");
    #[cfg(not(feature = "tantivy-backend"))]
    assert!(stdout.contains("no derived index"), "{stdout}");
    Ok(())
}

#[cfg(feature = "tantivy-backend")]
mod tantivy_degraded {
    use super::*;
    use anyhow::Context;

    #[test]
    fn index_status_and_doctor_report_degraded_partial_tantivy_index() -> anyhow::Result<()> {
        let temp = TempDb::new("index_status_and_doctor_report_degraded_partial_tantivy_index")?;
        kanban(&temp.path, &["init"])?.success()?;
        std::fs::create_dir_all(temp.dir.join("index/v1/tasks"))?;
        std::fs::write(
            temp.dir.join("index/v1/tasks/kb-index-meta.json"),
            b"partial tantivy meta",
        )?;

        for command in ["status", "doctor"] {
            let json = kanban(&temp.path, &["--json", "index", command])?.success_json()?;
            assert_eq!(json["data"]["backend"], "sqlite");
            assert_eq!(json["data"]["derived_index"], true);
            assert_eq!(json["data"]["stale"], true);
            let message = json["data"]["message"]
                .as_str()
                .context("expected JSON string")?;
            assert!(message.contains("degraded"), "{message}");
            assert!(message.contains("SQLite fallback"), "{message}");
        }
        Ok(())
    }
}
