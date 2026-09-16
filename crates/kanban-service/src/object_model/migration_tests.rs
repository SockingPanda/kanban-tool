use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use crate::{KanbanService, StoreError, TursoStore, UpgradeBackupHook, UpgradeBackupRequest};

#[derive(Default)]
struct Backups(Mutex<Vec<std::path::PathBuf>>);
impl UpgradeBackupHook for Backups {
    fn before_upgrade(&self, request: &UpgradeBackupRequest) -> Result<(), StoreError> {
        assert!(request.backup_path.is_file());
        self.0.lock().unwrap().push(request.backup_path.clone());
        Ok(())
    }
}

async fn baseline(path: &Path) -> TursoStore {
    let store = TursoStore::open(path).await.unwrap();
    let mut c = store.connection().await.unwrap();
    crate::migration::apply(&mut c, path, None).await.unwrap();
    c.execute("INSERT INTO boards(id,slug,name,created_at,updated_at) VALUES ('b_default','default','Default',1,1)",()).await.unwrap();
    c.execute("INSERT INTO tasks(id,board_id,seq,title,status,created_by,created_at,updated_at) VALUES ('t_old','b_default',1,'hex:原始文本','todo','legacy-agent',1,2)",()).await.unwrap();
    store
}

#[tokio::test]
async fn v4_upgrade_preserves_attachment_identity_bytes_and_verified_backups() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v4.db");
    let store = baseline(&path).await;
    let root = dir.path().join("attachments");
    let relative = "b_default/t_old/a_old-proof.bin";
    std::fs::create_dir_all(root.join("b_default/t_old")).unwrap();
    std::fs::write(root.join(relative), b"legacy bytes").unwrap();
    let c = store.connection().await.unwrap();
    c.execute("INSERT INTO task_attachments(id,board_id,task_id,filename,rel_path,size_bytes,created_by,created_at) VALUES ('a_old','b_default','t_old','proof.bin',?1,12,'legacy-agent',3)",[relative]).await.unwrap();
    drop(c);
    let backups = Backups::default();
    store.initialize_requiring_backup(&backups).await.unwrap();
    assert_eq!(backups.0.lock().unwrap().len(), 2);
    store.initialize_requiring_backup(&backups).await.unwrap();
    assert_eq!(backups.0.lock().unwrap().len(), 2, "重复打开不应再次迁移");
    let mut service = KanbanService::new(store);
    service.attachment_root = Some(Arc::new(root));
    let object = service.object_get("b_default", "t_old").await.unwrap();
    assert_eq!(object.title, "hex:原始文本");
    assert_eq!(object.version.source, Some(0));
    let attachment = service.read_attachment("t_old", "a_old").await.unwrap();
    assert_eq!(attachment.content, b"legacy bytes");
    assert_eq!(attachment.attachment.created_by, "legacy-agent");
    let c = service.store.connection().await.unwrap();
    assert!(c.execute("INSERT INTO task_attachments(id,board_id,task_id,filename,rel_path,size_bytes,created_by,created_at) VALUES ('a_bypass','b_default','t_old','bad','bad',0,'bad',4)",()).await.is_err());
    assert!(
        service
            .object_diagnostics("b_default")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn fresh_database_needs_no_upgrade_backup_and_rejects_ddl_drift() {
    let dir = tempfile::tempdir().unwrap();
    let store = TursoStore::open(dir.path().join("fresh.db")).await.unwrap();
    let backups = Backups::default();
    store.initialize_requiring_backup(&backups).await.unwrap();
    assert!(backups.0.lock().unwrap().is_empty());
    let c = store.connection().await.unwrap();
    c.execute("DROP TRIGGER object_task_identity_insert", ())
        .await
        .unwrap();
    assert!(
        store
            .initialize()
            .await
            .unwrap_err()
            .to_string()
            .contains("object_task_identity_insert")
    );
}

#[test]
fn ddl_normalization_preserves_string_literal_identity() {
    use super::migration::compact_sql;
    assert_eq!(
        compact_sql("CREATE TABLE x(key TEXT)"),
        compact_sql("CREATE TABLE x (\"key\" TEXT)")
    );
    assert_ne!(
        compact_sql("CHECK (x='a b')"),
        compact_sql("CHECK (x='ab')")
    );
    assert_ne!(
        compact_sql("CHECK (x='ABC')"),
        compact_sql("CHECK (x='abc')")
    );
}
