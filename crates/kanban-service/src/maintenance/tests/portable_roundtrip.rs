use crate::TursoStore;
use crate::maintenance::{integer_value, text_value};
use crate::test_support::{create_input, store};

#[tokio::test]
async fn verified_backup_and_portable_export_import_roundtrip() {
    let (source_directory, source, _source_path) = store("maintenance-source").await;
    source.initialize().await.expect("initialize source");
    source
        .create_task(
            "default",
            create_input("t_maintenance", None, "Maintenance fixture"),
        )
        .await
        .expect("fixture task");
    source
        .create_task("default", create_input("t_parent", None, "Parent fixture"))
        .await
        .expect("parent task");
    source
        .create_task("default", create_input("t_child", None, "Child fixture"))
        .await
        .expect("child task");
    let source_connection = source.connection().await.expect("source connection");
    source_connection
        .execute(
            "INSERT INTO task_dependencies(board_id, parent_task_id, child_task_id, created_at) VALUES ('b_default', 't_parent', 't_child', 424242)",
            (),
        )
        .await
        .expect("dependency");
    drop(source_connection);

    let backup_path = source_directory.path().join("verified.db");
    let backup = source.backup(&backup_path).await.expect("verified backup");
    assert!(backup_path.is_file());
    assert!(backup.bytes > 0);
    assert!(backup.checksum_sha256.starts_with("sha256:"));

    let export_path = source_directory.path().join("portable.jsonl");
    let export = source.export(&export_path).await.expect("portable export");
    assert!(export.record_count > 0);
    assert!(export_path.is_file());

    let (_target_directory, target, _target_path) = store("maintenance-target").await;
    target.initialize().await.expect("initialize target");
    let import = target
        .import(&export_path, false)
        .await
        .expect("portable import");
    assert!(import.imported_records > 0);
    assert_eq!(import.phase, "completed");
    assert!(!import.restart_required);
    assert!(import.rebuild_jobs_enqueued > 0);
    assert!(
        !target
            .maintenance_status()
            .await
            .expect("target status")
            .owner
            .active
    );
    // 首次导入必须独立成立；不能让第二次 import 修复第一次的关系或字段。
    assert_portable_roundtrip_facts(&target, "首次导入").await;

    let repeated = target
        .import(&export_path, false)
        .await
        .expect("repeated portable import is idempotent");
    assert_eq!(repeated.journal_id, import.journal_id);
    assert_eq!(repeated.phase, "completed");
    assert_portable_roundtrip_facts(&target, "完成态重放").await;
}

async fn assert_portable_roundtrip_facts(target: &TursoStore, stage: &str) {
    let page = target
        .list_tasks(
            "default",
            crate::store_operations::StoreTaskListOptions::default(),
        )
        .await
        .expect(stage);
    let mut tasks = page
        .tasks
        .iter()
        .map(|task| (task.id.as_str(), task.title.as_str()))
        .collect::<Vec<_>>();
    tasks.sort_unstable();
    assert_eq!(
        tasks,
        [
            ("t_child", "Child fixture"),
            ("t_maintenance", "Maintenance fixture"),
            ("t_parent", "Parent fixture"),
        ],
        "{stage}: 任务集合和字段必须完整"
    );
    let connection = target.connection().await.expect(stage);
    let mut rows = connection
        .query(
            "SELECT parent_task_id, child_task_id, created_at FROM task_dependencies",
            (),
        )
        .await
        .expect(stage);
    let row = rows.next().await.expect(stage).expect(stage);
    let relation = (
        text_value(row.get_value(0).expect(stage), "parent").expect(stage),
        text_value(row.get_value(1).expect(stage), "child").expect(stage),
        integer_value(row.get_value(2).expect(stage), "created_at").expect(stage),
    );
    assert_eq!(
        relation,
        ("t_parent".to_owned(), "t_child".to_owned(), 424242),
        "{stage}: 关系端点和时间戳必须保真"
    );
    assert!(
        rows.next().await.expect(stage).is_none(),
        "{stage}: 不得重复导入关系"
    );
}
