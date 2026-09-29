use crate::TursoStore;
use crate::maintenance::{integer_value, text_value};
use crate::test_support::{create_input, store};

#[tokio::test]
async fn replace_backup_journal_failure_is_retryable_without_fact_loss() {
    // A/B 与 A′/C 必须有真实差异，才能同时拒绝 no-op、只做 upsert 和漏删旧记录。
    // 独立目标多一次初始化，但不通过破坏 canonical 表来伪造来源独有记录的缺席。
    let (directory, source, _) = store("maintenance-backup-journal-source").await;
    source.initialize().await.expect("initialize source");
    for (id, title) in [
        ("t_backup_common", "snapshot common"),
        ("t_backup_incoming", "snapshot only"),
    ] {
        source
            .create_task("default", create_input(id, None, title))
            .await
            .expect("snapshot task");
    }
    let snapshot = read_replace_task_facts(&source).await;
    assert_eq!(
        snapshot
            .iter()
            .map(|t| (t.id.as_str(), t.title.as_str()))
            .collect::<Vec<_>>(),
        [
            ("t_backup_common", "snapshot common"),
            ("t_backup_incoming", "snapshot only"),
        ]
    );
    let export_path = directory.path().join("portable.jsonl");
    source.export(&export_path).await.expect("portable export");
    drop(source);

    let (_target_directory, target, _) = store("maintenance-backup-journal-target").await;
    target.initialize().await.expect("initialize target");
    for (id, title) in [
        ("t_backup_common", "target common changed"),
        ("t_backup_existing", "target only"),
    ] {
        target
            .create_task("default", create_input(id, None, title))
            .await
            .expect("target task");
    }
    let before = read_replace_task_facts(&target).await;
    assert_eq!(
        before
            .iter()
            .map(|t| (t.id.as_str(), t.title.as_str()))
            .collect::<Vec<_>>(),
        [
            ("t_backup_common", "target common changed"),
            ("t_backup_existing", "target only"),
        ]
    );

    target.set_import_failpoint(crate::maintenance::FAILPOINT_BACKUP_JOURNAL);
    let first = target
        .import(&export_path, true)
        .await
        .expect_err("backup journal fault must surface before replace transaction");
    assert!(first.to_string().contains("故障注入"));
    assert_eq!(
        read_replace_task_facts(&target).await,
        before,
        "故障后必须保持 A′/C 及其版本，不能提前插入 B、覆盖 A′ 或清除 C"
    );

    let resumed = target
        .import(&export_path, true)
        .await
        .expect("prepared replace journal retries backup and transaction");
    assert_eq!(resumed.phase, "completed");
    assert_eq!(
        read_replace_task_facts(&target).await,
        snapshot,
        "成功后必须精确恢复 A/B，而不是 upsert 得到 A/B/C"
    );
}

#[derive(Debug, PartialEq, Eq)]
struct ReplaceTaskFact {
    id: String,
    title: String,
    lock_version: i64,
}

async fn read_replace_task_facts(target: &TursoStore) -> Vec<ReplaceTaskFact> {
    // 不用有分页／归档过滤的列表 API，避免残留记录只是被隐藏而让测试假通过。
    let connection = target
        .connection()
        .await
        .expect("replacement fact connection");
    let mut rows = connection
        .query("SELECT id, title, lock_version FROM tasks ORDER BY id", ())
        .await
        .expect("replacement fact query");
    let mut tasks = Vec::new();
    while let Some(row) = rows.next().await.expect("replacement fact row") {
        tasks.push(ReplaceTaskFact {
            id: text_value(row.get_value(0).unwrap(), "id").unwrap(),
            title: text_value(row.get_value(1).unwrap(), "title").unwrap(),
            lock_version: integer_value(row.get_value(2).unwrap(), "lock_version").unwrap(),
        });
    }
    tasks
}
