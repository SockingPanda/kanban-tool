#[cfg(feature = "legacy-sqlite-import")]
use std::path::Path;

#[cfg(feature = "legacy-sqlite-import")]
use std::{fs, path::PathBuf, process::Command};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
#[cfg(feature = "legacy-sqlite-import")]
use rusqlite::Connection as SqliteConnection;
use serde_json::Value;
use tokio::sync::OnceCell;
use tower::ServiceExt;

use crate::{AppState, build_router};

#[cfg(feature = "legacy-sqlite-import")]
const LEGACY_MIGRATION_CHECKSUMS: &[&str] = &[
    "fnv64:c2b3cfdaf5fd0ac9",
    "fnv64:a549c98f375abb33",
    "fnv64:d753b64649f2d5e8",
    "fnv64:8e71302408c0f5ec",
    "fnv64:5f57751fa5ae355b",
    "fnv64:5722037d819848d2",
    "fnv64:49ab6f02badb38b9",
    "fnv64:d95bb151e044abc1",
    "fnv64:90d55fe98f14936d",
    "fnv64:a751d75a3e5f8baf",
    "fnv64:290fbadac17c29f1",
    "fnv64:6d8c91d4e8e19867",
    "fnv64:62c9ab9e70f13de6",
    "fnv64:695a4deb53af8a8f",
    "fnv64:48083714dafd3134",
    "fnv64:8f0929c89221a551",
    "fnv64:35e5380b866144cf",
    "fnv64:67124280774a3ab3",
    "fnv64:4e9fa46c02814766",
    "fnv64:ec251890669cc15c",
    "fnv64:03f363173d517df3",
    "fnv64:6fce00e46a30ddcf",
    "fnv64:0c7dd431257a6946",
    "fnv64:2401135db5f7d807",
    "fnv64:d8bf6ea31135dc83",
    "fnv64:c5eddec1f4511bae",
    "fnv64:5df731a27efdae55",
    "fnv64:7ea454008b72e2fc",
    "fnv64:f41cb49971216fe0",
    "fnv64:ad2e4075068e7794",
];

static LEGACY_HTTP_FLOW: OnceCell<()> = OnceCell::const_new();

pub(crate) async fn ensure_legacy_http_flow() {
    LEGACY_HTTP_FLOW
        .get_or_init(|| async {
            run_legacy_http_flow()
                .await
                .expect("legacy SQLite v30 HTTP flow");
        })
        .await;
}

async fn run_legacy_http_flow() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let target_path = directory.path().join("legacy-target.db");
    let target = AppState::open(&target_path, "legacy-adoption")
        .await
        .map_err(|error| error.to_string())?;
    let router = build_router(target.clone());

    #[cfg(feature = "legacy-sqlite-import")]
    {
        let source_path = make_legacy_source(directory.path())?;
        let attachment_root = directory.path().join("canonical-attachments");
        let response = router
            .oneshot(post_json(
                "/api/v1/maintenance/import-v30",
                serde_json::json!({
                    "path": source_path,
                    "canonical_attachment_root": attachment_root,
                }),
            ))
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let body = decode_json(response).await?;
        assert_eq!(status, StatusCode::OK, "legacy import response: {body}");
        assert_eq!(body["data"]["phase"], "completed");
        assert_eq!(body["data"]["resumed"], false);
        assert_eq!(body["data"]["attachment_count"], 1);
        assert_eq!(table_count(&body, "boards"), 1);
        assert_eq!(table_count(&body, "tasks"), 2);
        assert_eq!(table_count(&body, "task_dependencies"), 1);
        assert_eq!(table_count(&body, "task_attachments"), 1);
        let published = attachment_root.join("attachments/legacy.txt");
        assert_eq!(
            fs::read(&published).map_err(|error| error.to_string())?,
            b"legacy\n"
        );
        assert_target_facts(&target).await?;
    }

    #[cfg(not(feature = "legacy-sqlite-import"))]
    {
        let response = router
            .oneshot(post_json(
                "/api/v1/maintenance/import-v30",
                serde_json::json!({"path":"/tmp/legacy-v30.sqlite"}),
            ))
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }
    Ok(())
}

#[cfg(feature = "legacy-sqlite-import")]
fn table_count(body: &Value, table: &str) -> u64 {
    body["data"]["table_counts"]
        .as_array()
        .expect("legacy table counts")
        .iter()
        .find(|count| count["table"] == table)
        .and_then(|count| count["source_rows"].as_u64())
        .expect("legacy table count entry")
}

#[cfg(feature = "legacy-sqlite-import")]
async fn assert_target_facts(target: &AppState) -> Result<(), String> {
    let export_path = target
        .db_path()
        .parent()
        .ok_or("target database has no parent")?
        .join("legacy-target.jsonl");
    target
        .application()
        .export(
            export_path
                .to_str()
                .ok_or("target export path is not UTF-8")?,
        )
        .await
        .map_err(|error| error.to_string())?;
    let bytes = fs::read(export_path).map_err(|error| error.to_string())?;
    let mut task = None;
    let mut dependency = None;
    let mut attachment = None;
    for line in bytes
        .split(|byte| *byte == b'\n')
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let record: Value = serde_json::from_slice(line).map_err(|error| error.to_string())?;
        match record["type"].as_str() {
            Some("tasks") if record["data"]["id"].as_str() == Some("t_legacy") => {
                task = record.get("data").cloned();
            }
            Some("task_dependencies")
                if record["data"]["parent_task_id"].as_str() == Some("t_legacy") =>
            {
                dependency = record.get("data").cloned();
            }
            Some("task_attachments") if record["data"]["id"].as_str() == Some("a_legacy") => {
                attachment = record.get("data").cloned();
            }
            _ => {}
        }
    }
    let task = task.ok_or("legacy task fact missing")?;
    assert_eq!(task["board_id"], "b_legacy");
    assert_eq!(task["created_at"], 100);
    assert_eq!(task["updated_at"], 101);
    let dependency = dependency.ok_or("legacy dependency fact missing")?;
    assert_eq!(dependency["board_id"], "b_legacy");
    assert_eq!(dependency["parent_task_id"], "t_legacy");
    assert_eq!(dependency["child_task_id"], "t_child");
    assert_eq!(dependency["created_at"], 110);
    let attachment = attachment.ok_or("legacy attachment fact missing")?;
    assert_eq!(attachment["rel_path"], "attachments/legacy.txt");
    assert_eq!(attachment["size_bytes"], 7);
    assert_eq!(attachment["created_at"], 120);
    Ok(())
}

#[cfg(feature = "legacy-sqlite-import")]
fn make_legacy_source(directory: &Path) -> Result<PathBuf, String> {
    let source_path = directory.join("legacy-v30.sqlite");
    let connection = SqliteConnection::open(&source_path).map_err(|error| error.to_string())?;
    let migration_files = migration_files()?;
    assert_eq!(migration_files.len(), LEGACY_MIGRATION_CHECKSUMS.len());
    for (name, path) in &migration_files {
        let sql = git_show(path)?;
        connection
            .execute_batch(&sql)
            .map_err(|error| format!("apply legacy migration {name}: {error}"))?;
    }
    connection
        .pragma_update(None, "user_version", 30_i64)
        .map_err(|error| error.to_string())?;
    for (version, ((name, _), checksum)) in migration_files
        .iter()
        .zip(LEGACY_MIGRATION_CHECKSUMS)
        .enumerate()
    {
        connection
            .execute(
                "INSERT OR REPLACE INTO schema_migrations(version,name,checksum,applied_at) VALUES (?1,?2,?3,?4)",
                (version as i64 + 1, name, *checksum, version as i64 + 1),
            )
            .map_err(|error| error.to_string())?;
    }
    let ledger = connection
        .prepare("SELECT version,name,checksum FROM schema_migrations ORDER BY version")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|error| error.to_string())?;
    if ledger.len() != LEGACY_MIGRATION_CHECKSUMS.len()
        || ledger
            .iter()
            .zip(LEGACY_MIGRATION_CHECKSUMS)
            .enumerate()
            .any(|(index, ((version, name, checksum), expected_checksum))| {
                *version != index as i64 + 1
                    || name != &migration_files[index].0
                    || checksum != expected_checksum
            })
    {
        return Err(format!("legacy migration ledger mismatch: {ledger:?}"));
    }
    connection
        .execute_batch(
            r#"
PRAGMA foreign_keys = ON;
INSERT INTO boards(id,slug,name,description,created_at,updated_at,archived_at)
VALUES ('b_legacy','legacy','Legacy board','v30 fixture',90,91,NULL);
INSERT INTO board_columns(id,board_id,status,title,position,hidden,wip_limit,created_at,updated_at)
VALUES ('col_legacy','b_legacy','todo','Todo',1,0,NULL,92,93);
INSERT INTO tasks(id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version)
VALUES
 ('t_legacy','b_legacy',1,'Legacy task',NULL,'todo',NULL,NULL,3,10,NULL,NULL,'legacy-test',100,101,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,0,NULL,NULL,NULL,'{}',0),
 ('t_child','b_legacy',2,'Legacy child',NULL,'todo',NULL,NULL,3,20,NULL,NULL,'legacy-test',102,103,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,0,NULL,NULL,NULL,'{}',0);
INSERT INTO task_dependencies(board_id,parent_task_id,child_task_id,created_at)
VALUES ('b_legacy','t_legacy','t_child',110);
INSERT INTO task_runs(id,board_id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,claim_expires_at,started_at,last_heartbeat_at,finished_at,exit_code,summary,error,log_path,metadata_json)
VALUES ('r_legacy','b_legacy','t_legacy','succeeded',NULL,NULL,'legacy-token','legacy-owner',115,116,NULL,117,0,'done',NULL,NULL,'{}');
INSERT INTO task_comments(id,board_id,task_id,author,author_type,agent_type,body,kind,metadata_json,created_at)
VALUES ('c_legacy','b_legacy','t_legacy','legacy-test','user',NULL,'legacy comment','note','{}',118);
INSERT INTO task_events(event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at)
VALUES ('e_legacy','b_legacy','t_legacy','r_legacy','custom.opaque','legacy-test','{"legacy":true}',119);
INSERT INTO task_attachments(id,board_id,task_id,filename,rel_path,content_type,size_bytes,sha256,created_by,created_at)
VALUES ('a_legacy','b_legacy','t_legacy','legacy.txt','attachments/legacy.txt','text/plain',7,NULL,'legacy-test',120);
"#,
        )
        .map_err(|error| error.to_string())?;
    let attachment = directory.join("attachments/attachments/legacy.txt");
    if let Some(parent) = attachment.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(attachment, b"legacy\n").map_err(|error| error.to_string())?;
    Ok(source_path)
}

#[cfg(feature = "legacy-sqlite-import")]
fn migration_files() -> Result<Vec<(String, String)>, String> {
    let listing = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", "b4d6a2b7^"])
        .current_dir(repository_root())
        .output()
        .map_err(|error| error.to_string())?;
    if !listing.status.success() {
        return Err(String::from_utf8_lossy(&listing.stderr).into_owned());
    }
    let mut files = String::from_utf8_lossy(&listing.stdout)
        .lines()
        .filter(|path| path.starts_with("migrations/") && path.ends_with(".sql"))
        .map(|path| {
            let name = path
                .strip_prefix("migrations/")
                .and_then(|path| path.strip_suffix(".sql"))
                .ok_or_else(|| format!("invalid legacy migration path: {path}"))?;
            Ok((name.to_owned(), path.to_owned()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    if files.is_empty() {
        return Err(format!(
            "legacy migration history unavailable: status={} stdout={} stderr={}",
            listing.status,
            String::from_utf8_lossy(&listing.stdout),
            String::from_utf8_lossy(&listing.stderr)
        ));
    }
    Ok(files)
}

#[cfg(feature = "legacy-sqlite-import")]
fn git_show(path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["show", &format!("b4d6a2b7^:{path}")])
        .current_dir(repository_root())
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

#[cfg(feature = "legacy-sqlite-import")]
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("kanban workspace root")
}

fn post_json(uri: &str, value: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&value).expect("legacy request JSON"),
        ))
        .expect("legacy POST request")
}

#[cfg(feature = "legacy-sqlite-import")]
async fn decode_json(response: axum::response::Response) -> Result<Value, String> {
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .map_err(|error| error.to_string())?
        .to_bytes();
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}
