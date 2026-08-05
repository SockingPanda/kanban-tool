use crate::{
    ArchiveBoardInput, CreateBoardInput, StoreError,
    shared::{first_row, integer_value, text_value},
    test_support::{create_input, store},
};

fn board_input(id: &str, slug: &str, name: &str, event_id: &str) -> CreateBoardInput {
    CreateBoardInput {
        id: id.to_owned(),
        slug: slug.to_owned(),
        name: name.to_owned(),
        description: Some("描述".to_owned()),
        actor: "tester".to_owned(),
        event_id: event_id.to_owned(),
        created_at: 100,
    }
}

fn archive_input(event_id: &str, archived_at: i64) -> ArchiveBoardInput {
    ArchiveBoardInput {
        actor: "archiver".to_owned(),
        event_id: event_id.to_owned(),
        archived_at,
    }
}

#[tokio::test]
async fn create_board_initializes_columns_and_event() {
    let (_directory, store, _path) = store("board-create").await;
    store.initialize().await.expect("初始化数据库");

    let board = store
        .create_board(board_input(
            "b_created",
            "created",
            "新看板",
            "e_board_created",
        ))
        .await
        .expect("创建看板");
    assert_eq!(board.id, "b_created");
    assert_eq!(board.slug, "created");
    assert_eq!(board.name, "新看板");
    assert_eq!(board.description.as_deref(), Some("描述"));
    assert_eq!(board.archived_at, None);

    let connection = store.connection().await.expect("连接数据库");
    let columns = first_row(
        connection
            .query(
                "SELECT COUNT(*) FROM board_columns WHERE board_id = ?1",
                [board.id.as_str()],
            )
            .await
            .expect("查询默认列"),
    )
    .await
    .expect("默认列计数");
    assert_eq!(
        integer_value(columns.get_value(0).expect("列计数"), "board_columns.count")
            .expect("列计数整数"),
        9
    );

    let event = first_row(
        connection
            .query(
                "SELECT kind, actor, payload_json FROM task_events WHERE event_id = ?1",
                ["e_board_created"],
            )
            .await
            .expect("查询创建事件"),
    )
    .await
    .expect("创建事件");
    assert_eq!(
        text_value(event.get_value(0).expect("事件类型"), "event.kind").expect("事件类型文本"),
        "board.created"
    );
    assert_eq!(
        text_value(event.get_value(1).expect("事件操作人"), "event.actor").expect("事件操作人文本"),
        "tester"
    );
    assert_eq!(
        text_value(event.get_value(2).expect("事件载荷"), "event.payload").expect("事件载荷文本"),
        r#"{"slug":"created"}"#
    );
}

#[tokio::test]
async fn create_board_rejects_duplicate_slug_atomically() {
    let (_directory, store, _path) = store("board-duplicate-slug").await;
    store.initialize().await.expect("初始化数据库");
    store
        .create_board(board_input(
            "b_first",
            "same-slug",
            "第一个",
            "e_board_first",
        ))
        .await
        .expect("创建第一个看板");

    let error = store
        .create_board(board_input(
            "b_second",
            "same-slug",
            "第二个",
            "e_board_second",
        ))
        .await
        .expect_err("重复 slug 必须失败");
    assert!(matches!(
        error,
        StoreError::InvalidInput(message) if message.contains("看板 slug 已存在")
    ));

    let connection = store.connection().await.expect("连接数据库");
    let boards = first_row(
        connection
            .query("SELECT COUNT(*) FROM boards WHERE slug = ?1", ["same-slug"])
            .await
            .expect("查询重复 slug"),
    )
    .await
    .expect("重复 slug 计数");
    assert_eq!(
        integer_value(boards.get_value(0).expect("看板计数"), "boards.count")
            .expect("看板计数整数"),
        1
    );
}

#[tokio::test]
async fn archive_board_rejects_running_work_and_preserves_active_board() {
    let (_directory, store, _path) = store("board-running-guard").await;
    store.initialize().await.expect("初始化数据库");
    let board = store
        .create_board(board_input(
            "b_running",
            "running-board",
            "运行看板",
            "e_board_running",
        ))
        .await
        .expect("创建看板");
    let task = store
        .create_task("running-board", create_input("t_running", None, "运行任务"))
        .await
        .expect("创建任务");

    let connection = store.connection().await.expect("连接数据库");
    connection
        .execute(
            "UPDATE tasks SET status = 'running', claim_token = 'claim-token', claim_owner = 'worker', claim_expires_at = 500 WHERE id = ?1",
            [task.id.as_str()],
        )
        .await
        .expect("设置运行中任务");

    let error = store
        .archive_board(
            "running-board",
            archive_input("e_board_running_archive", 600),
        )
        .await
        .expect_err("运行中看板不能归档");
    assert!(matches!(
        error,
        StoreError::InvalidTransition(message)
            if message.contains("运行中的任务或执行记录")
    ));
    assert_eq!(
        store.get_board("running-board").await.expect("读取看板").id,
        board.id
    );
}

#[tokio::test]
async fn archived_board_is_hidden_from_get_and_task_create() {
    let (_directory, store, _path) = store("board-archive").await;
    store.initialize().await.expect("初始化数据库");
    let board = store
        .create_board(board_input(
            "b_archived_now",
            "archived-now",
            "待归档",
            "e_board_archive",
        ))
        .await
        .expect("创建看板");

    let archived = store
        .archive_board("archived-now", archive_input("e_board_archived", 700))
        .await
        .expect("归档看板");
    assert_eq!(archived.id, board.id);
    assert_eq!(archived.archived_at, Some(700));
    assert!(matches!(
        store.get_board("archived-now").await,
        Err(StoreError::BoardNotFound(selector)) if selector == "archived-now"
    ));

    let error = store
        .create_task(
            "archived-now",
            create_input("t_archived_board", None, "归档后任务"),
        )
        .await
        .expect_err("归档看板不能创建任务");
    assert!(matches!(
        error,
        StoreError::InvalidTransition(message) if message.contains("已归档看板不能创建任务")
    ));

    let connection = store.connection().await.expect("连接数据库");
    let events = first_row(
        connection
            .query(
                "SELECT COUNT(*) FROM task_events WHERE board_id = ?1 AND kind = 'board.archived'",
                [board.id.as_str()],
            )
            .await
            .expect("查询归档事件"),
    )
    .await
    .expect("归档事件计数");
    assert_eq!(
        integer_value(events.get_value(0).expect("事件计数"), "events.count")
            .expect("事件计数整数"),
        1
    );
}
