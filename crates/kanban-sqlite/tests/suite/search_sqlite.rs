use crate::common::*;

#[test]
fn sqlite_search_fallback_matches_task_related_text_with_filters_and_paging() -> anyhow::Result<()>
{
    let temp =
        TempDb::new("sqlite_search_fallback_matches_task_related_text_with_filters_and_paging")?;
    init_database(&temp.path, "tester")?;

    let alpha = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Alpha primary".into(),
            description: Some("plain spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 1,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let beta = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Beta secondary".into(),
            description: Some("mentions fallback needle in the spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let gamma = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Gamma unrelated".into(),
            description: Some("plain spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-b".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "Archived fallback needle".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: Some("worker-a".into()),
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;

    create_comment(
        &temp.path,
        &alpha.id,
        "tester",
        "comment carries fallback needle",
        None,
    )?;
    archive_task(&temp.path, "default", "tester", &archived.id, false)?;

    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, claim_token, claim_owner, claim_expires_at, started_at, summary, error, metadata_json) VALUES (?1, ?2, ?3, 'failed', 'token', 'tester', 1, 1, ?4, ?5, '{}')",
        params![new_run_id(), board_id, gamma.id, "run fallback needle summary", "run fallback needle error"],
    )
    ?;

    let results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("fallback needle".into()),
            statuses: vec![TaskStatus::Ready],
            assignee: Some("worker-a".into()),
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;

    assert_eq!(results.meta.backend, "sqlite");
    assert!(!results.meta.stale);
    assert_eq!(
        results
            .hits
            .iter()
            .map(|hit| hit.task_id.as_str())
            .collect::<Vec<_>>(),
        vec![beta.id.as_str(), alpha.id.as_str()]
    );
    assert!(results.hits.iter().all(|hit| hit.snippet.is_some()));
    assert!(results.hits[0].score >= results.hits[1].score);

    let second_page = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("fallback needle".into()),
            statuses: vec![],
            assignee: None,
            include_archived: true,
            limit: 2,
            offset: 2,
        },
    )?;
    assert_eq!(second_page.hits.len(), 2);
    assert!(
        second_page
            .hits
            .iter()
            .any(|hit| hit.task_id == gamma.id || hit.task_id == archived.id)
    );
    Ok(())
}

#[test]
fn sqlite_search_rejects_limit_that_cannot_be_bounded_safely() -> anyhow::Result<()> {
    let temp = TempDb::new("sqlite_search_rejects_limit_that_cannot_be_bounded_safely")?;
    init_database(&temp.path, "tester")?;

    let error = result_err(search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("anything".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: usize::MAX,
            offset: 0,
        },
    ))?;

    assert!(error.to_string().contains("limit must be <= 1000"));
    Ok(())
}

#[test]
fn sqlite_task_list_rejects_limit_that_cannot_be_bounded_safely() -> anyhow::Result<()> {
    let temp = TempDb::new("sqlite_task_list_rejects_limit_that_cannot_be_bounded_safely")?;
    init_database(&temp.path, "tester")?;

    let error = result_err(kanban_sqlite::list_tasks_page(
        &temp.path,
        "default",
        kanban_sqlite::TaskListOptions {
            statuses: vec![],
            priorities: vec![],
            include_archived: false,
            assignee: None,
            search: None,
            sort: kanban_sqlite::TaskListSort::Position,
            limit: usize::MAX,
            offset: 0,
        },
    ))?;

    assert!(error.to_string().contains("limit must be <= 1000"));
    Ok(())
}

#[test]
fn sqlite_search_treats_like_wildcards_and_escape_characters_as_literal_query_text()
-> anyhow::Result<()> {
    let temp = TempDb::new(
        "sqlite_search_treats_like_wildcards_and_escape_characters_as_literal_query_text",
    )?;
    init_database(&temp.path, "tester")?;

    let title_percent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "literal percent % title".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let description_underscore = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "description literal".into(),
            description: Some("ready spec with literal _ marker".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let comment_percent = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("comment literal source"),
    )?;
    let run_underscore = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("run literal source"),
    )?;
    let title_backslash = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask {
            title: "literal backslash \\ title".into(),
            description: Some("ready spec".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 0,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "{}".into(),
        },
    )?;
    let control = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("control plain source"),
    )?;

    create_comment(
        &temp.path,
        &comment_percent.id,
        "tester",
        "comment contains literal % marker",
        None,
    )?;

    let conn = connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT id FROM boards WHERE slug='default'", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO task_runs(id, board_id, task_id, status, claim_token, claim_owner, claim_expires_at, started_at, summary, error, metadata_json) VALUES (?1, ?2, ?3, 'failed', 'token', 'tester', 1, 1, ?4, NULL, '{}')",
        params![new_run_id(), board_id, run_underscore.id, "run contains literal _ marker"],
    )
    ?;

    let percent_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("%".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    let percent_ids = percent_results
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    assert!(percent_ids.contains(&title_percent.id.as_str()));
    assert!(percent_ids.contains(&comment_percent.id.as_str()));
    assert!(!percent_ids.contains(&control.id.as_str()));

    let underscore_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("_".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    let underscore_ids = underscore_results
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    assert!(underscore_ids.contains(&description_underscore.id.as_str()));
    assert!(underscore_ids.contains(&run_underscore.id.as_str()));
    assert!(!underscore_ids.contains(&control.id.as_str()));

    let backslash_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("\\".into()),
            statuses: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    let backslash_ids = backslash_results
        .hits
        .iter()
        .map(|hit| hit.task_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(backslash_ids, vec![title_backslash.id.as_str()]);
    Ok(())
}
