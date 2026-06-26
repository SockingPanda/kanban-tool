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
    let alpha = mark_plan_not_required_for_test(&temp.path, "default", "tester", &alpha.id)?;
    let beta = mark_plan_not_required_for_test(&temp.path, "default", "tester", &beta.id)?;
    let gamma = mark_plan_not_required_for_test(&temp.path, "default", "tester", &gamma.id)?;
    let archived = mark_plan_not_required_for_test(&temp.path, "default", "tester", &archived.id)?;

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
            labels: vec![],
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
            labels: vec![],
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
            labels: vec![],
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
            labels: vec![],
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
fn sqlite_task_list_search_matches_task_refs_exactly() -> anyhow::Result<()> {
    let temp = TempDb::new("sqlite_task_list_search_matches_task_refs_exactly")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;

    let first = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("first task"),
    )?;
    let second = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("title mentions 1 but should not match numeric search"),
    )?;
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archived task"),
    )?;
    let other = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other board same seq"),
    )?;
    let first = mark_plan_not_required_for_test(&temp.path, "default", "tester", &first.id)?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &second.id)?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &archived.id)?;
    archive_task(&temp.path, "default", "tester", &archived.id, false)?;
    mark_plan_not_required_for_test(&temp.path, "other", "tester", &other.id)?;

    for query in ["1", "#1", "default#1", "default/#1", first.id.as_str()] {
        let page = kanban_sqlite::list_tasks_page(
            &temp.path,
            "default",
            kanban_sqlite::TaskListOptions {
                statuses: vec![TaskStatus::Ready],
                priorities: vec![],
                labels: vec![],
                include_archived: false,
                assignee: None,
                search: Some(query.to_owned()),
                sort: kanban_sqlite::TaskListSort::Seq,
                limit: 10,
                offset: 0,
            },
        )?;
        assert_eq!(
            page.tasks
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            vec![first.id.as_str()],
            "{query}"
        );
    }

    let numeric_page = kanban_sqlite::list_tasks_page(
        &temp.path,
        "default",
        kanban_sqlite::TaskListOptions {
            statuses: vec![],
            priorities: vec![],
            labels: vec![],
            include_archived: false,
            assignee: None,
            search: Some("1".into()),
            sort: kanban_sqlite::TaskListSort::Seq,
            limit: 10,
            offset: 0,
        },
    )?;
    assert!(!numeric_page.tasks.iter().any(|task| task.id == second.id));

    for (query, include_archived) in [
        ("other#1", false),
        (other.id.as_str(), false),
        ("#3", false),
        ("#3", true),
    ] {
        let page = kanban_sqlite::list_tasks_page(
            &temp.path,
            "default",
            kanban_sqlite::TaskListOptions {
                statuses: vec![TaskStatus::Ready],
                priorities: vec![],
                labels: vec![],
                include_archived,
                assignee: None,
                search: Some(query.to_owned()),
                sort: kanban_sqlite::TaskListSort::Seq,
                limit: 10,
                offset: 0,
            },
        )?;
        assert_eq!(
            page.tasks
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            Vec::<&str>::new(),
            "{query} include_archived={include_archived}"
        );
    }
    let archived_page = kanban_sqlite::list_tasks_page(
        &temp.path,
        "default",
        kanban_sqlite::TaskListOptions {
            statuses: vec![],
            priorities: vec![],
            labels: vec![],
            include_archived: true,
            assignee: None,
            search: Some("#3".into()),
            sort: kanban_sqlite::TaskListSort::Seq,
            limit: 10,
            offset: 0,
        },
    )?;
    assert_eq!(
        archived_page
            .tasks
            .iter()
            .map(|task| task.id.as_str())
            .collect::<Vec<_>>(),
        vec![archived.id.as_str()]
    );

    Ok(())
}

#[test]
fn sqlite_search_matches_task_refs_exactly() -> anyhow::Result<()> {
    let temp = TempDb::new("sqlite_search_matches_task_refs_exactly")?;
    init_database(&temp.path, "tester")?;
    create_board(
        &temp.path,
        "tester",
        CreateBoard {
            slug: "other".into(),
            name: "Other".into(),
            description: None,
        },
    )?;

    let first = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("first searchable task"),
    )?;
    let second = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("title mentions 1 but numeric search is exact"),
    )?;
    let archived = create_task(
        &temp.path,
        "default",
        "tester",
        CreateTask::ready("archived searchable task"),
    )?;
    let other = create_task(
        &temp.path,
        "other",
        "tester",
        CreateTask::ready("other board same seq"),
    )?;
    let first = mark_plan_not_required_for_test(&temp.path, "default", "tester", &first.id)?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &second.id)?;
    mark_plan_not_required_for_test(&temp.path, "default", "tester", &archived.id)?;
    archive_task(&temp.path, "default", "tester", &archived.id, false)?;
    mark_plan_not_required_for_test(&temp.path, "other", "tester", &other.id)?;

    for query in ["1", "#1", "default#1", first.id.as_str()] {
        let results = search_tasks(
            &temp.path,
            kanban_search::SearchQuery {
                board: "default".into(),
                q: Some(query.to_owned()),
                statuses: vec![TaskStatus::Ready],
                labels: vec![],
                assignee: None,
                include_archived: false,
                limit: 10,
                offset: 0,
            },
        )?;
        assert_eq!(
            results
                .hits
                .iter()
                .map(|hit| hit.task_id.as_str())
                .collect::<Vec<_>>(),
            vec![first.id.as_str()],
            "{query}"
        );
        assert!(results.hits[0].score > 0.0);
    }

    let numeric_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("1".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: false,
            limit: 10,
            offset: 0,
        },
    )?;
    assert!(
        !numeric_results
            .hits
            .iter()
            .any(|hit| hit.task_id == second.id)
    );

    for (query, include_archived) in [
        ("other#1", false),
        (other.id.as_str(), false),
        ("#3", false),
        ("#3", true),
    ] {
        let results = search_tasks(
            &temp.path,
            kanban_search::SearchQuery {
                board: "default".into(),
                q: Some(query.to_owned()),
                statuses: vec![TaskStatus::Ready],
                labels: vec![],
                assignee: None,
                include_archived,
                limit: 10,
                offset: 0,
            },
        )?;
        assert_eq!(
            results
                .hits
                .iter()
                .map(|hit| hit.task_id.as_str())
                .collect::<Vec<_>>(),
            Vec::<&str>::new(),
            "{query} include_archived={include_archived}"
        );
    }
    let archived_results = search_tasks(
        &temp.path,
        kanban_search::SearchQuery {
            board: "default".into(),
            q: Some("#3".into()),
            statuses: vec![],
            labels: vec![],
            assignee: None,
            include_archived: true,
            limit: 10,
            offset: 0,
        },
    )?;
    assert_eq!(
        archived_results
            .hits
            .iter()
            .map(|hit| hit.task_id.as_str())
            .collect::<Vec<_>>(),
        vec![archived.id.as_str()]
    );

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
            labels: vec![],
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
            labels: vec![],
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
            labels: vec![],
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
