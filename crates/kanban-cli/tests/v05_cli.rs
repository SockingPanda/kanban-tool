use std::{path::Path, process::Command};

use kanban_sqlite::maintenance_lock_path;

#[test]
fn serve_help_includes_default_localhost_bind_options() {
    let output = Command::new(env!("CARGO_BIN_EXE_kb"))
        .args(["serve", "--help"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--host"), "{stdout}");
    assert!(stdout.contains("127.0.0.1"), "{stdout}");
    assert!(stdout.contains("--port"), "{stdout}");
    assert!(stdout.contains("8721"), "{stdout}");
    assert!(stdout.contains("--search-sync-interval-ms"), "{stdout}");
    assert!(stdout.contains("5000"), "{stdout}");
}

#[test]
fn serve_rejects_non_loopback_host_without_creating_database() {
    let temp = TempDb::new("serve_rejects_non_loopback_host_without_creating_database");

    kb(&temp.path, &["serve", "--host", "0.0.0.0", "--port", "0"])
        .failure_containing("kb serve only supports loopback hosts");
    assert!(!temp.path.exists());
}

#[test]
fn task_update_sets_and_clears_scheduled_at_and_due_at() {
    let temp = TempDb::new("task_update_sets_and_clears_scheduled_at_and_due_at");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli update dates",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();

    let updated = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--scheduled-at",
            "1767225600000",
            "--due-at",
            "1767312000000",
        ],
    )
    .success_json();
    assert_eq!(updated["data"]["scheduled_at"], 1_767_225_600_000_i64);
    assert_eq!(updated["data"]["due_at"], 1_767_312_000_000_i64);

    let cleared = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "update",
            task_id,
            "--clear-scheduled-at",
            "--clear-due-at",
        ],
    )
    .success_json();
    assert!(cleared["data"]["scheduled_at"].is_null());
    assert!(cleared["data"]["due_at"].is_null());
}

#[test]
fn task_complete_alias_finishes_like_done() {
    let temp = TempDb::new("task_complete_alias_finishes_like_done");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli complete alias",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    let claim = kb(&temp.path, &["--json", "task", "claim", task_id]).success_json();
    let token = claim["data"]["claim_token"].as_str().unwrap();

    let completed = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "complete",
            task_id,
            "--claim-token",
            token,
        ],
    )
    .success_json();
    assert_eq!(completed["data"]["status"], "done");
}

#[test]
fn task_reclaim_expired_alias_matches_bare_reclaim() {
    let bare = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_bare");
    let explicit = TempDb::new("task_reclaim_expired_alias_matches_bare_reclaim_explicit");

    for temp in [&bare, &explicit] {
        kb(&temp.path, &["init"]).success();
        let created = kb(
            &temp.path,
            &[
                "--json",
                "task",
                "create",
                "cli reclaim alias",
                "--description",
                "ready spec",
            ],
        )
        .success_json();
        let task_id = created["data"]["id"].as_str().unwrap();
        kb(
            &temp.path,
            &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
        )
        .success_json();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let bare_result = kb(&bare.path, &["--json", "task", "reclaim"]).success_json();
    let explicit_result =
        kb(&explicit.path, &["--json", "task", "reclaim", "--expired"]).success_json();

    assert_eq!(bare_result, explicit_result);
    assert_eq!(explicit_result["data"]["reclaimed"], 1);
}

#[test]
fn doctor_reports_integrity_migration_and_expired_running_tasks() {
    let temp = TempDb::new("doctor_reports_integrity_migration_and_expired_running_tasks");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "doctor expired",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    kb(
        &temp.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "1"],
    )
    .success_json();
    std::thread::sleep(std::time::Duration::from_millis(5));

    let doctor = kb(&temp.path, &["--json", "doctor"]).success_json();

    assert_eq!(doctor["data"]["integrity_check"], "ok");
    assert_eq!(doctor["data"]["migration_version"], 2);
    assert_eq!(doctor["data"]["user_version"], 2);
    assert_eq!(doctor["data"]["expired_running_tasks"], 1);
    assert_eq!(doctor["data"]["dependency_cycles"], 0);
    assert_eq!(doctor["data"]["archived_dependency_edges"], 0);
    assert_eq!(doctor["data"]["missing_run_logs"], 0);
    assert_eq!(doctor["data"]["executable_dependency_violations"], 0);
    assert_eq!(doctor["data"]["executable_spec_violations"], 0);
    assert_eq!(doctor["data"]["executable_schedule_violations"], 0);
    assert_eq!(doctor["data"]["ok"], false);
}

#[test]
fn dispatch_loop_uses_worker_profile_config_and_respects_assignee_routing() {
    let temp =
        TempDb::new("dispatch_loop_uses_worker_profile_config_and_respects_assignee_routing");
    kb(&temp.path, &["init"]).success();
    let backend = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "backend work",
            "--description",
            "ready spec",
            "--assignee",
            "backend",
        ],
    )
    .success_json();
    let frontend = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "frontend work",
            "--description",
            "ready spec",
            "--assignee",
            "frontend",
        ],
    )
    .success_json();
    let config = temp.dir.join("workers.toml");
    let logs = temp.dir.join("logs");
    std::fs::write(
        &config,
        format!(
            "[workers.backend]\ncommand = \"sh -c 'true'\"\nclaim_ttl_ms = 60000\nheartbeat_interval_ms = 10\non_success = \"done\"\non_failure = \"blocked\"\nlog_dir = \"{}\"\n",
            logs.display()
        ),
    )
    .unwrap();

    let result = kb(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--worker-profile",
            "backend",
            "--profile-config",
            config.to_str().unwrap(),
            "--max-iterations",
            "1",
        ],
    )
    .success_json();

    assert_eq!(result["data"]["iterations"], 1);
    assert_eq!(result["data"]["claimed"], 1);
    let backend_task = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            backend["data"]["id"].as_str().unwrap(),
        ],
    )
    .success_json();
    let frontend_task = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "show",
            frontend["data"]["id"].as_str().unwrap(),
        ],
    )
    .success_json();
    assert_eq!(backend_task["data"]["status"], "done");
    assert_eq!(frontend_task["data"]["status"], "ready");
}

#[test]
fn retry_policy_and_run_log_commands_support_operator_recovery() {
    let temp = TempDb::new("retry_policy_and_run_log_commands_support_operator_recovery");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "retry policy",
            "--description",
            "ready spec",
            "--max-retries",
            "2",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    assert_eq!(created["data"]["max_retries"], 2);

    let updated = kb(
        &temp.path,
        &["--json", "task", "update", task_id, "--clear-max-retries"],
    )
    .success_json();
    assert!(updated["data"]["max_retries"].is_null());

    let reset = kb(
        &temp.path,
        &["--json", "task", "update", task_id, "--max-retries", "1"],
    )
    .success_json();
    assert_eq!(reset["data"]["max_retries"], 1);

    let logs = temp.dir.join("logs");
    let dispatch = kb(
        &temp.path,
        &[
            "--json",
            "dispatch",
            "--once",
            "--command",
            "printf 'operator log\\n'",
            "--log-dir",
            logs.to_str().unwrap(),
        ],
    )
    .success_json();
    let run_id = dispatch["data"]["run_id"].as_str().unwrap();

    let run = kb(&temp.path, &["--json", "run", "show", run_id]).success_json();
    assert_eq!(run["data"]["id"], run_id);
    assert!(run["data"].get("claim_token").is_some());

    let log = kb(&temp.path, &["--json", "run", "logs", run_id]).success_json();
    assert_eq!(log["data"]["run_id"], run_id);
    assert_eq!(log["data"]["content"], "operator log\n");
    assert_eq!(log["data"]["truncated"], false);
}

#[test]
fn task_list_supports_search_assignee_sort_limit_and_offset() {
    let temp = TempDb::new("task_list_supports_search_assignee_sort_limit_and_offset");
    kb(&temp.path, &["init"]).success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "Alpha search match",
            "--description",
            "ready spec search-term",
            "--assignee",
            "worker-a",
            "--priority",
            "1",
        ],
    )
    .success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "Beta search match",
            "--description",
            "ready spec search-term",
            "--assignee",
            "worker-a",
            "--priority",
            "10",
        ],
    )
    .success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "Gamma search match",
            "--description",
            "ready spec search-term",
            "--assignee",
            "worker-b",
            "--priority",
            "100",
        ],
    )
    .success();

    let tasks = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "list",
            "--search",
            "search-term",
            "--assignee",
            "worker-a",
            "--sort",
            "priority_desc",
            "--limit",
            "1",
            "--offset",
            "1",
        ],
    )
    .success_json();

    let data = tasks["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["title"], "Alpha search match");
}

#[test]
fn search_command_outputs_json_and_human_hits() {
    let temp = TempDb::new("search_command_outputs_json_and_human_hits");
    kb(&temp.path, &["init"]).success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "Alpha search surface",
            "--description",
            "ready spec unique-needle",
            "--assignee",
            "worker-a",
        ],
    )
    .success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "Beta search surface",
            "--description",
            "ready spec unique-needle",
            "--assignee",
            "worker-b",
        ],
    )
    .success();

    let json = kb(
        &temp.path,
        &[
            "--json",
            "search",
            "unique-needle",
            "--assignee",
            "worker-a",
            "--limit",
            "5",
        ],
    )
    .success_json();
    assert_eq!(json["data"]["meta"]["backend"], "sqlite");
    let hits = json["data"]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "Alpha search surface");
    assert!(hits[0]["score"].as_f64().unwrap() > 0.0);
    assert!(
        hits[0]["snippet"]
            .as_str()
            .unwrap()
            .contains("unique-needle")
    );

    let human = kb(
        &temp.path,
        &["search", "unique-needle", "--assignee", "worker-a"],
    );
    assert!(human.output.status.success());
    let stdout = String::from_utf8_lossy(&human.output.stdout);
    assert!(stdout.contains("#1"), "{stdout}");
    assert!(stdout.contains("[ready]"), "{stdout}");
    assert!(stdout.contains("score="), "{stdout}");
    assert!(stdout.contains("Alpha search surface"), "{stdout}");
    assert!(stdout.contains("unique-needle"), "{stdout}");
}

#[test]
fn substrate_commands_report_entities_outbox_and_derived_status() {
    let temp = TempDb::new("substrate_commands_report_entities_outbox_and_derived_status");
    kb(&temp.path, &["init"]).success();

    let entities = kb(
        &temp.path,
        &[
            "--json", "entity", "list", "--kind", "board", "--limit", "5",
        ],
    )
    .success_json();
    let entity_rows = entities["data"].as_array().unwrap();
    assert_eq!(entity_rows.len(), 1);
    assert_eq!(entity_rows[0]["kind"], "board");
    let uri = entity_rows[0]["uri"].as_str().unwrap();
    assert!(uri.starts_with("kb://board/"));

    let shown = kb(&temp.path, &["--json", "entity", "show", uri]).success_json();
    assert_eq!(shown["data"]["uri"], uri);

    let outbox = kb(&temp.path, &["--json", "outbox", "list"]).success_json();
    assert_eq!(outbox["data"].as_array().unwrap().len(), 0);

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
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    let task_uri = format!("kb://task/{task_id}");
    let task_entity = kb(&temp.path, &["--json", "entity", "show", &task_uri]).success_json();
    assert_eq!(task_entity["data"]["title"], "substrate task");

    let outbox = kb(&temp.path, &["--json", "outbox", "list"]).success_json();
    let jobs = outbox["data"].as_array().unwrap();
    assert_eq!(jobs.len(), 3);
    let targets = jobs
        .iter()
        .map(|job| job["target"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(targets, vec!["tantivy", "oxigraph", "lancedb"]);
    assert!(jobs.iter().all(|job| job["entity_uri"] == task_uri));

    let derived = kb(&temp.path, &["--json", "derived", "status"]).success_json();
    let stores = derived["data"].as_array().unwrap();
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
}

#[test]
fn graph_vector_and_context_commands_report_disabled_fallbacks() {
    let temp = TempDb::new("graph_vector_and_context_commands_report_disabled_fallbacks");
    kb(&temp.path, &["init"]).success();
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
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();

    let graph = kb(&temp.path, &["--json", "graph", "status"]).success_json();
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
    )
    .success_json();
    assert_eq!(neighbors["data"].as_array().unwrap().len(), 0);

    let vector = kb(&temp.path, &["--json", "vector", "status"]).success_json();
    assert_eq!(vector["data"]["backend"], "disabled");
    assert_eq!(vector["data"]["enabled"], false);

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
    )
    .success_json();
    assert_eq!(context["data"]["subject"], format!("kb://task/{task_id}"));
    assert!(
        context["data"]["degraded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "graph_disabled")
    );
    assert!(
        context["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["entity_uri"] == format!("kb://task/{task_id}"))
    );
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn index_rebuild_enables_tantivy_search_backend() {
    let temp = TempDb::new("index_rebuild_enables_tantivy_search_backend");
    kb(&temp.path, &["init"]).success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "cli tantivy comet",
            "--description",
            "ready spec",
        ],
    )
    .success();

    let rebuilt = kb(&temp.path, &["--json", "index", "rebuild"]).success_json();
    assert_eq!(rebuilt["data"]["backend"], "tantivy");
    assert_eq!(rebuilt["data"]["derived_index"], true);
    assert!(temp.dir.join("index/v1/tasks").exists());

    let search = kb(&temp.path, &["--json", "search", "comet"]).success_json();
    assert_eq!(search["data"]["meta"]["backend"], "tantivy");
    assert_eq!(
        search["data"]["hits"][0]["task"]["title"],
        "cli tantivy comet"
    );
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn index_sync_refreshes_stale_tantivy_search_backend() {
    let temp = TempDb::new("index_sync_refreshes_stale_tantivy_search_backend");
    kb(&temp.path, &["init"]).success();
    let created = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "cli sync source",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    kb(&temp.path, &["index", "rebuild"]).success();
    kb(
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
                .unwrap()
                .to_string()
                .as_str(),
        ],
    )
    .success();

    let stale = kb(&temp.path, &["--json", "index", "status"]).success_json();
    assert_eq!(stale["data"]["backend"], "tantivy");
    assert_eq!(stale["data"]["stale"], true);
    assert!(stale["data"]["index_lag_events"].as_i64().unwrap() > 0);

    let synced = kb(&temp.path, &["--json", "index", "sync"]).success_json();
    assert_eq!(synced["data"]["backend"], "tantivy");
    assert_eq!(synced["data"]["stale"], false);
    assert_eq!(synced["data"]["index_lag_events"], 0);

    let search = kb(&temp.path, &["--json", "search", "comet"]).success_json();
    assert_eq!(search["data"]["meta"]["backend"], "tantivy");
    assert_eq!(search["data"]["hits"][0]["task"]["title"], "cli sync comet");
}

#[test]
fn search_command_rejects_unbounded_limit() {
    let temp = TempDb::new("search_command_rejects_unbounded_limit");
    kb(&temp.path, &["init"]).success();

    kb(
        &temp.path,
        &["search", "needle", "--limit", &usize::MAX.to_string()],
    )
    .failure_containing("limit must be <= 1000");
}

#[test]
fn task_list_command_rejects_unbounded_limit() {
    let temp = TempDb::new("task_list_command_rejects_unbounded_limit");
    kb(&temp.path, &["init"]).success();

    kb(
        &temp.path,
        &["task", "list", "--limit", &usize::MAX.to_string()],
    )
    .failure_containing("limit must be <= 1000");
}

#[test]
fn search_command_treats_like_wildcards_and_escape_characters_as_literal_text() {
    let temp =
        TempDb::new("search_command_treats_like_wildcards_and_escape_characters_as_literal_text");
    kb(&temp.path, &["init"]).success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "literal percent % cli",
            "--description",
            "ready spec",
        ],
    )
    .success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "literal backslash \\ cli",
            "--description",
            "ready spec",
        ],
    )
    .success();
    kb(
        &temp.path,
        &[
            "task",
            "create",
            "plain cli control",
            "--description",
            "ready spec",
        ],
    )
    .success();

    let json = kb(&temp.path, &["--json", "search", "%"]).success_json();
    let hits = json["data"]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "literal percent % cli");

    let json = kb(&temp.path, &["--json", "search", "\\"]).success_json();
    let hits = json["data"]["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["task"]["title"], "literal backslash \\ cli");
}

#[test]
fn index_commands_report_sqlite_fallback_backend() {
    let temp = TempDb::new("index_commands_report_sqlite_fallback_backend");
    kb(&temp.path, &["init"]).success();

    for command in ["status", "doctor"] {
        let json = kb(&temp.path, &["--json", "index", command]).success_json();
        assert_eq!(json["data"]["backend"], "sqlite");
        assert_eq!(json["data"]["derived_index"], false);
        assert_eq!(json["data"]["stale"], false);
    }

    #[cfg(not(feature = "tantivy-backend"))]
    {
        let json = kb(&temp.path, &["--json", "index", "rebuild"]).success_json();
        assert_eq!(json["data"]["backend"], "sqlite");
        assert_eq!(json["data"]["derived_index"], false);
        assert_eq!(json["data"]["stale"], false);

        let json = kb(&temp.path, &["--json", "index", "sync"]).success_json();
        assert_eq!(json["data"]["backend"], "sqlite");
        assert_eq!(json["data"]["derived_index"], false);
        assert_eq!(json["data"]["stale"], false);
    }

    #[cfg(not(feature = "tantivy-backend"))]
    let human = kb(&temp.path, &["index", "rebuild"]);
    #[cfg(not(feature = "tantivy-backend"))]
    assert!(human.output.status.success());
    #[cfg(not(feature = "tantivy-backend"))]
    let stdout = String::from_utf8_lossy(&human.output.stdout);
    #[cfg(not(feature = "tantivy-backend"))]
    assert!(stdout.contains("SQLite fallback"), "{stdout}");
    #[cfg(not(feature = "tantivy-backend"))]
    assert!(stdout.contains("no derived index"), "{stdout}");
}

#[cfg(feature = "tantivy-backend")]
#[test]
fn index_status_and_doctor_report_degraded_partial_tantivy_index() {
    let temp = TempDb::new("index_status_and_doctor_report_degraded_partial_tantivy_index");
    kb(&temp.path, &["init"]).success();
    std::fs::create_dir_all(temp.dir.join("index/v1/tasks")).unwrap();
    std::fs::write(
        temp.dir.join("index/v1/tasks/kb-index-meta.json"),
        b"partial tantivy meta",
    )
    .unwrap();

    for command in ["status", "doctor"] {
        let json = kb(&temp.path, &["--json", "index", command]).success_json();
        assert_eq!(json["data"]["backend"], "sqlite");
        assert_eq!(json["data"]["derived_index"], true);
        assert_eq!(json["data"]["stale"], true);
        let message = json["data"]["message"].as_str().unwrap();
        assert!(message.contains("degraded"), "{message}");
        assert!(message.contains("SQLite fallback"), "{message}");
    }
}

#[test]
fn stats_command_reports_stale_claims_and_blocked_reasons() {
    let temp = TempDb::new("stats_command_reports_stale_claims_and_blocked_reasons");
    kb(&temp.path, &["init"]).success();
    let stale = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "stale cli",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let stale_id = stale["data"]["id"].as_str().unwrap();
    kb(
        &temp.path,
        &["--json", "task", "claim", stale_id, "--ttl-ms", "1"],
    )
    .success_json();
    let blocked = kb(
        &temp.path,
        &[
            "--json",
            "task",
            "create",
            "blocked cli",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let blocked_id = blocked["data"]["id"].as_str().unwrap();
    kb(
        &temp.path,
        &[
            "--json",
            "task",
            "block",
            blocked_id,
            "operator needed",
            "--force",
        ],
    )
    .success_json();
    std::thread::sleep(std::time::Duration::from_millis(5));

    let stats = kb(&temp.path, &["--json", "stats"]).success_json();

    assert_eq!(stats["data"]["stale_claims"][0]["task_id"], stale_id);
    assert_eq!(
        stats["data"]["blocked_reasons"][0]["reason"],
        "operator needed"
    );
    assert_eq!(stats["data"]["blocked_reasons"][0]["count"], 1);
}

#[test]
fn maintenance_commands_backup_checkpoint_vacuum_export_and_import_jsonl() {
    let source = TempDb::new("maintenance_commands_source");
    kb(&source.path, &["init"]).success();
    let created = kb(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "release smoke",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    let dispatch = kb_in_dir(
        &source.path,
        &[
            "--json",
            "dispatch",
            "--once",
            "--command",
            "printf 'exported log\\n'",
            "--log-dir",
            "logs",
        ],
        &source.dir,
    )
    .success_json();
    let run_id = dispatch["data"]["run_id"].as_str().unwrap();
    let run = kb(&source.path, &["--json", "run", "show", run_id]).success_json();
    let log_path = run["data"]["log_path"].as_str().unwrap();
    assert!(Path::new(log_path).is_absolute());
    assert!(log_path.starts_with(source.dir.to_str().unwrap()));

    let checkpoint = kb(&source.path, &["--json", "checkpoint"]).success_json();
    assert_eq!(checkpoint["data"]["busy"], 0);
    kb(&source.path, &["--json", "vacuum"]).success_json();

    let backup_path = source.dir.join("backup.sqlite");
    let backup = kb(
        &source.path,
        &["--json", "backup", "--out", backup_path.to_str().unwrap()],
    )
    .success_json();
    assert_eq!(backup["data"]["out_path"], backup_path.to_str().unwrap());
    assert!(backup_path.exists());

    let export_path = source.dir.join("board.jsonl");
    let export = kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();
    assert!(export["data"]["records"].as_u64().unwrap() >= 2);
    let export_content = std::fs::read_to_string(&export_path).unwrap();
    assert!(export_content.contains(task_id));
    assert!(export_content.contains(r#""log_path":null"#));

    let target = TempDb::new("maintenance_commands_target");
    let imported = kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .success_json();
    assert_eq!(imported["data"]["records"], export["data"]["records"]);
    let tasks = kb(&target.path, &["--json", "task", "list"]).success_json();
    assert_eq!(tasks["data"][0]["id"], task_id);
    assert_eq!(tasks["data"][0]["title"], "release smoke");
    let doctor = kb(&target.path, &["--json", "doctor"]).success_json();
    assert_eq!(doctor["data"]["ok"], true);
    assert_eq!(doctor["data"]["missing_run_logs"], 0);
    assert_eq!(doctor["data"]["executable_dependency_violations"], 0);
    assert_eq!(doctor["data"]["executable_spec_violations"], 0);
    assert_eq!(doctor["data"]["executable_schedule_violations"], 0);

    let invalid_export_path = source.dir.join("invalid-board.jsonl");
    std::fs::write(
        &invalid_export_path,
        export_content.replace(r#""log_path":null"#, r#""log_path":"/missing/kb-run.log""#),
    )
    .unwrap();
    let rejected = TempDb::new("maintenance_commands_rejected_import");
    kb(
        &rejected.path,
        &[
            "--json",
            "import",
            "--input",
            invalid_export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("imported data failed doctor checks");
    assert!(!rejected.path.exists());
}

#[test]
fn maintenance_commands_reject_missing_database() {
    let temp = TempDb::new("maintenance_commands_reject_missing_database");
    let backup_path = temp.dir.join("backup.sqlite");
    let nested_backup_path = temp.dir.join("new/subdir/backup.sqlite");
    let export_path = temp.dir.join("board.jsonl");
    let cases = [
        vec!["--json", "doctor"],
        vec!["--json", "checkpoint"],
        vec!["--json", "vacuum"],
        vec!["--json", "backup", "--out", backup_path.to_str().unwrap()],
        vec![
            "--json",
            "backup",
            "--out",
            nested_backup_path.to_str().unwrap(),
        ],
        vec!["--json", "export", "--out", export_path.to_str().unwrap()],
    ];

    for args in cases {
        kb(&temp.path, &args).failure_containing("database does not exist");
        assert!(
            !temp.path.exists(),
            "maintenance command should not create missing DB for args {args:?}"
        );
    }
    assert!(!backup_path.exists());
    assert!(!nested_backup_path.parent().unwrap().exists());
    assert!(!export_path.exists());
}

#[test]
fn import_rejects_missing_input_without_creating_database() {
    let temp = TempDb::new("import_rejects_missing_input_without_creating_database");
    let input_path = temp.dir.join("missing.jsonl");

    kb(
        &temp.path,
        &[
            "--json",
            "import",
            "--input",
            input_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("import input does not exist");
    assert!(!temp.path.exists());
}

#[test]
fn import_requires_replace_without_creating_database() {
    let temp = TempDb::new("import_requires_replace_without_creating_database");
    let input_path = temp.dir.join("board.jsonl");
    std::fs::write(
        &input_path,
        invalid_import_jsonl(&[task_record(
            "t_ready",
            1,
            "ready",
            "ready",
            Some("spec"),
            None,
        )]),
    )
    .unwrap();

    kb(
        &temp.path,
        &["--json", "import", "--input", input_path.to_str().unwrap()],
    )
    .failure_containing("import requires --replace");
    assert!(!temp.path.exists());
}

#[test]
fn import_replace_restores_over_corrupt_existing_database() {
    let source = TempDb::new("import_replace_restores_over_corrupt_existing_database_source");
    kb(&source.path, &["init"]).success();
    let created = kb(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "restore me",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();

    let target = TempDb::new("import_replace_restores_over_corrupt_existing_database_target");
    std::fs::write(&target.path, b"not a sqlite database").unwrap();
    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .success_json();

    let imported = kb(&target.path, &["--json", "task", "show", task_id]).success_json();
    assert_eq!(imported["data"]["title"], "restore me");
    assert_eq!(imported["data"]["status"], "ready");
}

#[test]
fn import_replace_rejects_maintenance_locked_database() {
    let source = TempDb::new("import_replace_rejects_maintenance_locked_database_source");
    kb(&source.path, &["init"]).success();
    kb(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "incoming task",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();

    let target = TempDb::new("import_replace_rejects_maintenance_locked_database_target");
    kb(&target.path, &["init"]).success();
    let existing = kb(
        &target.path,
        &[
            "--json",
            "task",
            "create",
            "keep existing",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let existing_id = existing["data"]["id"].as_str().unwrap();
    let lock_path = maintenance_lock_path(&target.path);
    std::fs::write(&lock_path, "locked").unwrap();

    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("database is locked for maintenance");
    std::fs::remove_file(lock_path).unwrap();

    let existing_after = kb(&target.path, &["--json", "task", "show", existing_id]).success_json();
    assert_eq!(existing_after["data"]["title"], "keep existing");
}

#[test]
fn import_replace_rejects_directory_database_path() {
    let source = TempDb::new("import_replace_rejects_directory_database_path_source");
    kb(&source.path, &["init"]).success();
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();

    let target = TempDb::new("import_replace_rejects_directory_database_path_target");
    let db_dir = target.dir.join("db-dir");
    std::fs::create_dir(&db_dir).unwrap();

    kb(
        &db_dir,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("database path is not a file");
    assert!(db_dir.is_dir());
}

#[test]
fn import_replace_rejects_running_work_in_target_database() {
    let source = TempDb::new("import_replace_rejects_running_work_source");
    kb(&source.path, &["init"]).success();
    let export_path = source.dir.join("restore.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();

    let target = TempDb::new("import_replace_rejects_running_work_target");
    kb(&target.path, &["init"]).success();
    let created = kb(
        &target.path,
        &[
            "--json",
            "task",
            "create",
            "running work",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    kb(&target.path, &["--json", "task", "claim", task_id]).success_json();

    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .failure_containing("database has running work");
    let running = kb(&target.path, &["--json", "task", "show", task_id]).success_json();
    assert_eq!(running["data"]["status"], "running");
}

#[test]
fn maintenance_lock_uses_canonical_database_path() {
    let temp = TempDb::new("maintenance_lock_uses_canonical_database_path");
    kb(&temp.path, &["init"]).success();
    let lock_path = maintenance_lock_path(&temp.path);
    std::fs::write(&lock_path, format!("pid={}", std::process::id())).unwrap();

    kb_in_dir(Path::new("kb.db"), &["--json", "doctor"], &temp.dir)
        .failure_containing("database is locked for maintenance");
    std::fs::remove_file(lock_path).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn maintenance_lock_with_dead_pid_is_removed() {
    let temp = TempDb::new("maintenance_lock_with_dead_pid_is_removed");
    kb(&temp.path, &["init"]).success();
    let lock_path = maintenance_lock_path(&temp.path);
    std::fs::write(&lock_path, "pid=999999999").unwrap();

    kb(&temp.path, &["--json", "doctor"]).success_json();
    assert!(!lock_path.exists());
}

#[test]
fn export_scrubs_active_running_claims_for_roundtrip_import() {
    let source = TempDb::new("export_scrubs_active_running_claims_for_roundtrip_import");
    kb(&source.path, &["init"]).success();
    let created = kb(
        &source.path,
        &[
            "--json",
            "task",
            "create",
            "active claim restore",
            "--description",
            "ready spec",
        ],
    )
    .success_json();
    let task_id = created["data"]["id"].as_str().unwrap();
    kb(
        &source.path,
        &["--json", "task", "claim", task_id, "--ttl-ms", "600000"],
    )
    .success_json();
    let export_path = source.dir.join("active-claim.jsonl");
    kb(
        &source.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .success_json();
    let export_content = std::fs::read_to_string(&export_path).unwrap();
    assert!(export_content.contains(r#""status":"ready""#));
    assert!(export_content.contains(r#""current_run_id":null"#));
    assert!(export_content.contains(r#""claim_token":null"#));

    let target = TempDb::new("export_scrubs_active_running_claims_target");
    kb(
        &target.path,
        &[
            "--json",
            "import",
            "--input",
            export_path.to_str().unwrap(),
            "--replace",
        ],
    )
    .success_json();
    let imported = kb(&target.path, &["--json", "task", "show", task_id]).success_json();
    assert_eq!(imported["data"]["status"], "ready");
    assert!(imported["data"]["claim_token"].is_null());
    assert!(imported["data"]["current_run_id"].is_null());
    assert!(imported["data"]["started_at"].is_null());
    let events = kb(&target.path, &["--json", "events", task_id]).success_json();
    assert!(
        events["data"].as_array().unwrap().iter().any(|event| {
            event["kind"] == "task.export_sanitized" && event["run_id"].as_str().is_some()
        }),
        "events: {events}"
    );
}

#[test]
fn export_rejects_existing_output_file() {
    let temp = TempDb::new("export_rejects_existing_output_file");
    kb(&temp.path, &["init"]).success();
    let export_path = temp.dir.join("board.jsonl");
    std::fs::write(&export_path, "keepme").unwrap();

    kb(
        &temp.path,
        &["--json", "export", "--out", export_path.to_str().unwrap()],
    )
    .failure_containing("export target already exists");
    assert_eq!(std::fs::read_to_string(&export_path).unwrap(), "keepme");
}

#[test]
fn import_rejects_executable_status_invariant_violations() {
    let cases = [
        (
            "dependency",
            invalid_import_jsonl(&[
                task_record("t_parent", 1, "parent", "todo", Some("spec"), None),
                task_record("t_child", 2, "child", "ready", Some("spec"), None),
                dependency_record("t_parent", "t_child"),
            ]),
            "executable_dependency_violations",
        ),
        (
            "spec",
            invalid_import_jsonl(&[task_record(
                "t_missing_spec",
                1,
                "missing spec",
                "ready",
                None,
                None,
            )]),
            "executable_spec_violations",
        ),
        (
            "schedule",
            invalid_import_jsonl(&[task_record(
                "t_future",
                1,
                "future",
                "ready",
                Some("spec"),
                Some(4_102_444_800_000),
            )]),
            "executable_schedule_violations",
        ),
    ];

    for (name, jsonl, doctor_field) in cases {
        let temp = TempDb::new(&format!(
            "import_rejects_executable_status_invariant_violations_{name}"
        ));
        let input_path = temp.dir.join("invalid.jsonl");
        std::fs::write(&input_path, jsonl).unwrap();

        kb(
            &temp.path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().unwrap(),
                "--replace",
            ],
        )
        .failure_containing("imported data failed doctor checks");
        assert!(!temp.path.exists(), "{doctor_field}");
    }
}

#[test]
fn import_rejects_empty_or_unusable_snapshots_without_creating_database() {
    let cases = [
        (
            "empty",
            String::new(),
            "imported data must contain at least one board",
        ),
        (
            "board_without_columns",
            board_only_import_jsonl(),
            "imported data must contain columns for every board",
        ),
    ];

    for (name, jsonl, error) in cases {
        let temp = TempDb::new(&format!("import_rejects_unusable_snapshot_{name}"));
        let input_path = temp.dir.join("unusable.jsonl");
        std::fs::write(&input_path, jsonl).unwrap();

        kb(
            &temp.path,
            &[
                "--json",
                "import",
                "--input",
                input_path.to_str().unwrap(),
                "--replace",
            ],
        )
        .failure_containing(error);
        assert!(!temp.path.exists(), "{name}");
    }
}

fn invalid_import_jsonl(records: &[serde_json::Value]) -> String {
    let mut lines = board_and_column_records();
    lines.extend(records.iter().map(ToString::to_string));
    lines.join("\n")
}

fn board_only_import_jsonl() -> String {
    board_record().to_string()
}

fn board_and_column_records() -> Vec<String> {
    vec![board_record().to_string(), column_record().to_string()]
}

fn board_record() -> serde_json::Value {
    serde_json::json!({
        "type": "board",
        "data": {
            "id": "b_import",
            "slug": "default",
            "name": "Default",
            "description": null,
            "created_at": 1,
            "updated_at": 1,
            "archived_at": null
        }
    })
}

fn column_record() -> serde_json::Value {
    serde_json::json!({
        "type": "column",
        "data": {
            "id": "col_import_ready",
            "board_id": "b_import",
            "status": "ready",
            "title": "Ready",
            "position": 40,
            "hidden": 0,
            "wip_limit": null,
            "created_at": 1,
            "updated_at": 1
        }
    })
}

fn task_record(
    id: &str,
    seq: i64,
    title: &str,
    status: &str,
    description: Option<&str>,
    scheduled_at: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "type": "task",
        "data": {
            "id": id,
            "board_id": "b_import",
            "seq": seq,
            "title": title,
            "description": description,
            "status": status,
            "status_reason": null,
            "assignee": null,
            "priority": 0,
            "position": seq * 1024,
            "scheduled_at": scheduled_at,
            "due_at": null,
            "created_by": "test",
            "created_at": 1,
            "updated_at": 1,
            "started_at": null,
            "completed_at": null,
            "archived_at": null,
            "claim_token": null,
            "claim_owner": null,
            "claim_expires_at": null,
            "last_heartbeat_at": null,
            "current_run_id": null,
            "retry_count": 0,
            "max_retries": null,
            "result_summary": null,
            "result_json": null,
            "metadata_json": "{}",
            "lock_version": 0
        }
    })
}

fn dependency_record(parent: &str, child: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "dependency",
        "data": {
            "board_id": "b_import",
            "parent_task_id": parent,
            "child_task_id": child,
            "created_at": 1
        }
    })
}

fn kb(db_path: &Path, args: &[&str]) -> CmdResult {
    kb_in_dir(db_path, args, Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn kb_in_dir(db_path: &Path, args: &[&str], current_dir: &Path) -> CmdResult {
    let output = Command::new(env!("CARGO_BIN_EXE_kb"))
        .current_dir(current_dir)
        .arg("--db")
        .arg(db_path)
        .args(args)
        .output()
        .unwrap();
    CmdResult { output }
}

struct CmdResult {
    output: std::process::Output,
}

impl CmdResult {
    fn success(self) {
        assert!(
            self.output.status.success(),
            "status: {:?}\nstdout:\n{}\nstderr:\n{}",
            self.output.status.code(),
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
    }

    fn success_json(self) -> serde_json::Value {
        assert!(
            self.output.status.success(),
            "status: {:?}\nstdout:\n{}\nstderr:\n{}",
            self.output.status.code(),
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        serde_json::from_slice(&self.output.stdout).unwrap()
    }

    fn failure_containing(self, expected: &str) {
        assert!(
            !self.output.status.success(),
            "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr)
        );
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        assert!(
            stderr.contains(expected),
            "expected stderr to contain {expected:?}, got:\n{stderr}"
        );
    }
}

struct TempDb {
    dir: std::path::PathBuf,
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let mut dir = std::env::temp_dir();
        dir.push(format!("kb-cli-v05-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("kb.db");
        Self { dir, path }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
