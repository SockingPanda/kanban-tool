use crate::common::*;

const TASK_READ_ENDPOINTS: &[&str] = &[
    "/api/v1/boards/default/tasks",
    "/api/v1/boards/default/tasks/by-status",
];

async fn assert_status(
    app: axum::Router,
    uri: &str,
    expected: StatusCode,
) -> anyhow::Result<Value> {
    let (status, response) = get_json(app, uri).await?;
    assert_eq!(status, expected, "{uri}: {response}");
    if expected == StatusCode::BAD_REQUEST {
        assert_eq!(response["error"]["code"], "invalid_input", "{uri}");
    }
    Ok(response)
}

async fn assert_bad_request_contains(
    app: axum::Router,
    uri: &str,
    expected_message: &str,
) -> anyhow::Result<()> {
    let response = assert_status(app, uri, StatusCode::BAD_REQUEST).await?;
    let message = response["error"]["message"]
        .as_str()
        .context("invalid_input message")?;
    assert!(
        message.contains(expected_message),
        "{uri}: expected {expected_message:?}, got {message:?}"
    );
    Ok(())
}

fn seed_filter_task(
    test: &TestApp,
    title: &str,
    assignee: &str,
    priority: i64,
    labels: &[&str],
    with_incomplete_step: bool,
) -> anyhow::Result<kanban_sqlite::api::TaskRecord> {
    let mut input = kanban_sqlite::api::CreateTask::ready(title);
    input.assignee = Some(assignee.to_owned());
    input.priority = priority;
    let task = kanban_sqlite::api::create_task(test.db_path(), "default", "filter-seed", input)?;
    for label in labels {
        kanban_sqlite::api::create_label(
            test.db_path(),
            "default",
            kanban_sqlite::api::CreateLabel {
                name: (*label).to_owned(),
                color: None,
            },
        )?;
        kanban_sqlite::api::add_task_label(
            test.db_path(),
            "default",
            "filter-seed",
            &task.id,
            label,
        )?;
    }
    if with_incomplete_step {
        kanban_sqlite::api::create_step(
            test.db_path(),
            "default",
            "filter-seed",
            &task.id,
            kanban_sqlite::api::CreateStepInput {
                title: "可观察的未完成必需步骤".to_owned(),
                body: None,
                linked_task_ref: None,
                position: None,
                required: true,
            },
        )?;
    } else {
        mark_plan_not_required_for_test(test.db_path(), "default", "filter-seed", &task.id)?;
    }
    kanban_sqlite::api::get_task(test.db_path(), "default", &task.id).map_err(Into::into)
}

fn encoded_task_read_uri(endpoint: &str, pairs: &[(&str, String)]) -> String {
    let query = serde_urlencoded::to_string(pairs).expect("form-encode task-read URI");
    format!("{endpoint}?{query}")
}

#[tokio::test]
async fn task_read_uri_matrix_locks_defaults_and_repeated_ordered_parameters() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let app = test.router();

    let list = assert_status(app.clone(), "/api/v1/boards/default/tasks", StatusCode::OK).await?;
    assert_eq!(list["meta"]["limit"], 100);
    assert_eq!(list["meta"]["offset"], 0);

    let windows = assert_status(
        app.clone(),
        "/api/v1/boards/default/tasks/by-status",
        StatusCode::OK,
    )
    .await?;
    assert_eq!(windows["data"]["statuses"], json!([]));
    assert_eq!(windows["meta"], json!({"limit": 100, "offset": 0}));

    for endpoint in TASK_READ_ENDPOINTS {
        let uri = format!(
            "{endpoint}?status=todo&status=ready&priority=0&priority=2&label=api&label=backend&plan_filter=plan_needed&plan_filter=incomplete_required_steps"
        );
        let response = assert_status(app.clone(), &uri, StatusCode::OK).await?;
        if endpoint.ends_with("by-status") {
            let statuses = response["data"]["statuses"]
                .as_array()
                .context("status windows")?;
            assert_eq!(statuses[0]["status"], "todo");
            assert_eq!(statuses[1]["status"], "ready");
        }
    }
    Ok(())
}

#[tokio::test]
async fn task_read_uri_matrix_rejects_unknown_alias_and_semantic_duplicates() -> anyhow::Result<()>
{
    let test = TestApp::new()?;
    let app = test.router();

    for endpoint in TASK_READ_ENDPOINTS {
        for suffix in ["unexpected=1", "search=removed-alias"] {
            let uri = format!("{endpoint}?{suffix}");
            assert_status(app.clone(), &uri, StatusCode::BAD_REQUEST).await?;
        }
        for duplicate in [
            "assignee=a&assignee=b",
            "q=a&q=b",
            "include_archived=false&include_archived=true",
            "limit=10&limit=20",
            "offset=0&offset=1",
            "sort=position&sort=-updated_at",
        ] {
            let uri = format!("{endpoint}?{duplicate}");
            assert_status(app.clone(), &uri, StatusCode::BAD_REQUEST).await?;
        }
        for duplicate in [
            "status=ready&status=ready",
            "priority=1&priority=01",
            "label=api&label=%20api%20",
            "plan_filter=has_steps&plan_filter=has_steps",
        ] {
            let uri = format!("{endpoint}?{duplicate}");
            assert_bad_request_contains(app.clone(), &uri, "duplicate repeated").await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn task_read_uri_matrix_locks_empty_malformed_and_bound_values() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();

    for endpoint in TASK_READ_ENDPOINTS {
        let allowed_empty = format!("{endpoint}?q=&assignee=");
        assert_status(app.clone(), &allowed_empty, StatusCode::OK).await?;
        let normalized_label = format!("{endpoint}?label=%E3%80%80api%E3%80%80");
        assert_status(app.clone(), &normalized_label, StatusCode::OK).await?;
        for whitespace_label in ["label=%C2%A0", "label=%E3%80%80"] {
            let uri = format!("{endpoint}?{whitespace_label}");
            assert_bad_request_contains(app.clone(), &uri, "non-whitespace").await?;
        }

        for rejected in [
            "status=",
            "priority=",
            "label=",
            "plan_filter=",
            "include_archived=",
            "limit=",
            "offset=",
            "sort=",
            "priority=4",
            "limit=1001",
            "offset=9223372036854775808",
            "q=%",
            "q=%GG",
            "q=%FF",
        ] {
            let uri = format!("{endpoint}?{rejected}");
            assert_status(app.clone(), &uri, StatusCode::BAD_REQUEST).await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn task_read_label_raw_budget_counts_trimmed_unicode_edges_on_both_endpoints()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let normalized_label = "界".repeat(126);
    let task = seed_filter_task(
        &test,
        "label raw budget sentinel",
        "boundary-agent",
        1,
        &[normalized_label.as_str()],
        false,
    )?;
    let app = test.router();

    let raw_at_limit = format!("\u{3000}{normalized_label}\u{2003}");
    assert_eq!(
        raw_at_limit.chars().count(),
        kanban_contract::MAX_TASK_READ_LABEL_CHARS
    );
    let raw_over_limit = format!("\u{3000}{}\u{2003}", "界".repeat(127));
    assert_eq!(
        raw_over_limit.chars().count(),
        kanban_contract::MAX_TASK_READ_LABEL_CHARS + 1
    );

    for endpoint in TASK_READ_ENDPOINTS {
        let accepted_uri = encoded_task_read_uri(
            endpoint,
            &[
                ("status", "ready".to_owned()),
                ("label", raw_at_limit.clone()),
            ],
        );
        let response = assert_status(app.clone(), &accepted_uri, StatusCode::OK).await?;
        if endpoint.ends_with("by-status") {
            assert_eq!(response["data"]["statuses"][0]["status"], "ready");
            assert_eq!(
                response["data"]["statuses"][0]["tasks"][0]["id"], task.id,
                "raw 128 字符必须先计入预算，再规范化为真实 label filter: {response}"
            );
        } else {
            assert_eq!(
                response["data"][0]["id"], task.id,
                "raw 128 字符必须先计入预算，再规范化为真实 label filter: {response}"
            );
        }

        let rejected_uri = encoded_task_read_uri(
            endpoint,
            &[
                ("status", "ready".to_owned()),
                ("label", raw_over_limit.clone()),
            ],
        );
        assert_bad_request_contains(app.clone(), &rejected_uri, "label exceeds").await?;
    }
    Ok(())
}

#[test]
fn task_read_second_review_limit_authority_is_single_source() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let service = std::fs::read_to_string(root.join("crates/kanban-sqlite/src/service.rs"))
        .expect("sqlite service source");
    assert!(
        service.contains(
            "pub const MAX_TASK_LIST_LIMIT: usize = \
             kanban_application::dto::MAX_TASK_LIST_LIMIT;"
        ),
        "SQLite service defensive limit 必须直接引用唯一 application authority"
    );
    let handlers = std::fs::read_to_string(manifest.join("src/handlers/tasks.rs"))
        .expect("task handlers source");
    assert!(
        handlers.contains("MAX_TASK_READ_LIMIT == kanban_sqlite::service::MAX_TASK_LIST_LIMIT"),
        "server equality gate 必须覆盖实际 SQLite service defensive path"
    );
    assert_eq!(
        kanban_contract::MAX_TASK_READ_LIMIT,
        kanban_sqlite::service::MAX_TASK_LIST_LIMIT
    );
}

#[tokio::test]
async fn task_read_uri_matrix_locks_every_budget_at_max_and_max_plus_one() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let app = test.router();
    let statuses = [
        "triage",
        "todo",
        "scheduled",
        "ready",
        "running",
        "blocked",
        "review",
        "done",
        "archived",
    ];
    let status_max = statuses
        .iter()
        .map(|status| format!("status={status}"))
        .collect::<Vec<_>>()
        .join("&");
    let status_over = format!("{status_max}&status=ready");
    let priority_max = (0..4)
        .map(|priority| format!("priority={priority}"))
        .collect::<Vec<_>>()
        .join("&");
    let priority_over = format!("{priority_max}&priority=0");
    let plan_filters = ["plan_needed", "has_steps", "incomplete_required_steps"];
    let plan_max = plan_filters
        .iter()
        .map(|filter| format!("plan_filter={filter}"))
        .collect::<Vec<_>>()
        .join("&");
    let plan_over = format!("{plan_max}&plan_filter=has_steps");
    let labels = (0..32)
        .map(|index| format!("label-{index:02}"))
        .collect::<Vec<_>>();
    let label_max = labels
        .iter()
        .map(|label| format!("label={label}"))
        .collect::<Vec<_>>()
        .join("&");
    let label_over = format!("{label_max}&label=label-32");

    let q_multibyte_max = serde_urlencoded::to_string([("q", "é".repeat(1_024))])?;
    let q_multibyte_over = serde_urlencoded::to_string([("q", "é".repeat(1_025))])?;
    let assignee_multibyte_max = serde_urlencoded::to_string([("assignee", "界".repeat(128))])?;
    let assignee_multibyte_over = serde_urlencoded::to_string([("assignee", "界".repeat(129))])?;
    let label_multibyte_max = serde_urlencoded::to_string([("label", "界".repeat(128))])?;
    let label_multibyte_over = serde_urlencoded::to_string([("label", "界".repeat(129))])?;

    let mut maximal_pairs = Vec::new();
    maximal_pairs.extend(statuses.iter().map(|value| ("status", (*value).to_owned())));
    maximal_pairs.extend((0..4).map(|value| ("priority", value.to_string())));
    maximal_pairs.extend(labels.iter().cloned().map(|value| ("label", value)));
    maximal_pairs.extend(
        plan_filters
            .iter()
            .map(|value| ("plan_filter", (*value).to_owned())),
    );
    maximal_pairs.extend([
        ("assignee", "worker".to_owned()),
        ("q", "needle".to_owned()),
        ("include_archived", "true".to_owned()),
        ("limit", "1".to_owned()),
        ("offset", "0".to_owned()),
        ("sort", "position".to_owned()),
    ]);
    let pairs_max = serde_urlencoded::to_string(&maximal_pairs)?;
    assert_eq!(pairs_max.split('&').count(), 54);
    let pairs_over = format!("{pairs_max}&q=again");
    assert_eq!(pairs_over.split('&').count(), 55);

    let bytes_max_value = format!("{}{}", "界".repeat(682), "é".repeat(342));
    assert_eq!(bytes_max_value.chars().count(), 1_024);
    let bytes_max = serde_urlencoded::to_string([("q", bytes_max_value)])?;
    assert_eq!(bytes_max.len(), 8_192);
    let bytes_over_value = format!("{}{}a", "界".repeat(684), "é".repeat(339));
    assert_eq!(bytes_over_value.chars().count(), 1_024);
    let bytes_over = serde_urlencoded::to_string([("q", bytes_over_value)])?;
    assert_eq!(bytes_over.len(), 8_193);

    for endpoint in TASK_READ_ENDPOINTS {
        for allowed in [
            "limit=1000",
            status_max.as_str(),
            priority_max.as_str(),
            plan_max.as_str(),
            label_max.as_str(),
            q_multibyte_max.as_str(),
            assignee_multibyte_max.as_str(),
            label_multibyte_max.as_str(),
            pairs_max.as_str(),
            bytes_max.as_str(),
        ] {
            let uri = format!("{endpoint}?{allowed}");
            assert_status(app.clone(), &uri, StatusCode::OK).await?;
        }

        for (rejected, message) in [
            ("limit=1001", "limit must be"),
            (status_over.as_str(), "too many status"),
            (priority_over.as_str(), "too many priority"),
            (plan_over.as_str(), "too many plan_filter"),
            (label_over.as_str(), "too many label"),
            (q_multibyte_over.as_str(), "q exceeds"),
            (assignee_multibyte_over.as_str(), "assignee exceeds"),
            (label_multibyte_over.as_str(), "label exceeds"),
            (pairs_over.as_str(), "exceeds 54 parameter pairs"),
            (bytes_over.as_str(), "exceeds 8192 bytes"),
        ] {
            let uri = format!("{endpoint}?{rejected}");
            assert_bad_request_contains(app.clone(), &uri, message).await?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn task_read_form_encoding_and_every_filter_are_observable() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let labels = ["后端 & API", "值班=A+B / 中文"];
    let assignee = "值班 & A=B+C";
    let needle = "编码 & /=+ 空格";
    let first = seed_filter_task(&test, &format!("A {needle}"), assignee, 1, &labels, true)?;
    let second = seed_filter_task(&test, &format!("B {needle}"), assignee, 1, &labels, true)?;
    seed_filter_task(&test, &format!("C {needle}"), assignee, 2, &labels, true)?;
    seed_filter_task(
        &test,
        &format!("D {needle}"),
        assignee,
        1,
        &labels[..1],
        true,
    )?;
    seed_filter_task(&test, &format!("E {needle}"), assignee, 1, &labels, false)?;
    seed_filter_task(
        &test,
        &format!("F {needle}"),
        "其他负责人",
        1,
        &labels,
        true,
    )?;
    seed_filter_task(&test, "G 不匹配搜索词", assignee, 1, &labels, true)?;
    let archived = seed_filter_task(&test, &format!("Z {needle}"), assignee, 1, &labels, true)?;
    kanban_sqlite::api::archive_task(test.db_path(), "default", "filter-seed", &archived.id, true)?;

    let app = test.router();
    for endpoint in TASK_READ_ENDPOINTS {
        let pairs = vec![
            ("status", "ready".to_owned()),
            ("priority", "1".to_owned()),
            ("label", labels[0].to_owned()),
            ("label", labels[1].to_owned()),
            ("plan_filter", "has_steps".to_owned()),
            ("plan_filter", "incomplete_required_steps".to_owned()),
            ("assignee", assignee.to_owned()),
            ("q", needle.to_owned()),
            ("include_archived", "false".to_owned()),
            ("limit", "1".to_owned()),
            ("offset", "1".to_owned()),
            ("sort", "title".to_owned()),
        ];
        let uri = encoded_task_read_uri(endpoint, &pairs);
        assert!(uri.contains("%26"), "真实 form encoder 必须转义 &: {uri}");
        assert!(uri.contains("%2F"), "真实 form encoder 必须转义 /: {uri}");
        assert!(uri.contains("%3D"), "真实 form encoder 必须转义 =: {uri}");
        assert!(uri.contains("%2B"), "真实 form encoder 必须转义 +: {uri}");
        let response = assert_status(app.clone(), &uri, StatusCode::OK).await?;
        if endpoint.ends_with("by-status") {
            let window = &response["data"]["statuses"][0];
            assert_eq!(window["status"], "ready");
            assert_eq!(window["page"]["total"], 2);
            assert_eq!(window["tasks"][0]["id"], second.id);
        } else {
            assert_eq!(response["meta"]["total"], 2);
            assert_eq!(response["data"][0]["id"], second.id);
        }

        let include_archived_pairs = vec![
            ("status", "ready".to_owned()),
            ("status", "archived".to_owned()),
            ("priority", "1".to_owned()),
            ("label", labels[0].to_owned()),
            ("label", labels[1].to_owned()),
            ("plan_filter", "has_steps".to_owned()),
            ("plan_filter", "incomplete_required_steps".to_owned()),
            ("assignee", assignee.to_owned()),
            ("q", needle.to_owned()),
            ("include_archived", "true".to_owned()),
            ("limit", "100".to_owned()),
            ("offset", "0".to_owned()),
            ("sort", "title".to_owned()),
        ];
        let uri = encoded_task_read_uri(endpoint, &include_archived_pairs);
        let response = assert_status(app.clone(), &uri, StatusCode::OK).await?;
        let archived_visible = if endpoint.ends_with("by-status") {
            response["data"]["statuses"]
                .as_array()
                .context("status windows")?
                .iter()
                .flat_map(|window| window["tasks"].as_array().expect("window tasks").iter())
                .any(|task| task["id"] == archived.id)
        } else {
            response["data"]
                .as_array()
                .context("task list")?
                .iter()
                .any(|task| task["id"] == archived.id)
        };
        assert!(archived_visible, "include_archived 未转发: {response}");
    }

    let mut todo_input = kanban_sqlite::api::CreateTask::ready("状态探针");
    todo_input.status = Some(kanban_core::TaskStatus::Todo);
    let todo =
        kanban_sqlite::api::create_task(test.db_path(), "default", "filter-seed", todo_input)?;
    let ready = create_ready_task_for_test(test.db_path(), "default", "filter-seed", "状态探针")?;
    for endpoint in TASK_READ_ENDPOINTS {
        let pairs = vec![
            ("status", "todo".to_owned()),
            ("q", "状态探针".to_owned()),
            ("limit", "100".to_owned()),
        ];
        let uri = encoded_task_read_uri(endpoint, &pairs);
        let response = assert_status(app.clone(), &uri, StatusCode::OK).await?;
        let ids = if endpoint.ends_with("by-status") {
            response["data"]["statuses"][0]["tasks"]
                .as_array()
                .context("todo window")?
        } else {
            response["data"].as_array().context("todo list")?
        };
        assert_eq!(ids.len(), 1, "status filter 未转发: {response}");
        assert_eq!(ids[0]["id"], todo.id);
        assert_ne!(ids[0]["id"], ready.id);
    }

    assert_ne!(first.id, second.id);
    Ok(())
}

#[tokio::test]
async fn task_read_uri_matrix_decodes_query_and_path_percent_encoding() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let task =
        create_ready_task_for_test(test.db_path(), "default", "seed", "strict query+decoder")?;
    kanban_sqlite::api::create_board(
        test.db_path(),
        "seed",
        kanban_sqlite::api::CreateBoard {
            slug: "percent-board".to_owned(),
            name: "Percent Board".to_owned(),
            description: None,
        },
    )?;
    let app = test.router();

    let list = assert_status(
        app.clone(),
        "/api/v1/boards/default/tasks?q=strict+query%2Bdecoder",
        StatusCode::OK,
    )
    .await?;
    assert_eq!(list["data"][0]["id"], task.id);

    let windows = assert_status(
        app.clone(),
        "/api/v1/boards/default/tasks/by-status?status=ready&q=strict+query%2Bdecoder",
        StatusCode::OK,
    )
    .await?;
    assert_eq!(windows["data"]["statuses"][0]["tasks"][0]["id"], task.id);

    assert_status(
        app.clone(),
        "/api/v1/boards/percent%2Dboard/tasks",
        StatusCode::OK,
    )
    .await?;
    assert_status(app, "/api/v1/boards/%FF/tasks", StatusCode::BAD_REQUEST).await?;
    Ok(())
}
