//! 供 host adoption 测试使用的 service-owned 数据库 fixture 与事实核验。
//!
//! 该模块只在 `test-support` feature 下编译。它可以准备和读取 canonical 数据库，
//! 但不暴露数据库连接或第二个 runtime backend 给 server；server 测试只消费这些
//! 不透明的 fixture/核验 helper，并通过正常 application/router 路径驱动行为。

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use rusqlite::Connection as SqliteConnection;
use serde_json::{Map, Value};
use turso::Connection;

use crate::{
    TursoStore,
    shared::{integer_value, text_value},
};

const PORTABLE_TABLES: &[&str] = &[
    "boards",
    "board_columns",
    "tasks",
    "task_execution_plans",
    "task_steps",
    "task_dependencies",
    "task_runs",
    "task_comments",
    "task_events",
    "task_attachments",
    "labels",
    "task_labels",
    "app_settings",
    "task_subtasks",
    "entities",
    "relation_predicates",
    "entity_relations",
    "label_semantics",
    "label_atoms",
    "label_atom_index_boards",
    "label_semantic_proposals",
    "label_ontology_observations",
    "label_ontology_signals",
    "label_ontology_actions",
    "label_ontology_action_signals",
    "label_ontology_action_atom_effects",
    "signal_observations",
    "signals",
];

/// 在已经由 service 初始化的 canonical 数据库中预置 portable adoption 事实。
pub async fn populate_portable_source(path: &Path) -> Result<(), String> {
    let store = TursoStore::open(path)
        .await
        .map_err(|error| error.to_string())?;
    let connection = store
        .connection()
        .await
        .map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            r#"
PRAGMA foreign_keys = ON;
BEGIN;
INSERT INTO boards(id,slug,name,description,created_at,updated_at,archived_at) VALUES ('b_core','core','Core','portable board',1,2,NULL);
INSERT INTO boards(id,slug,name,description,created_at,updated_at,archived_at) VALUES ('b_fixture','fixture','Fixture','portable ledger board',5,6,NULL);
INSERT INTO board_columns(id,board_id,status,title,position,hidden,wip_limit,created_at,updated_at) VALUES ('col_core','b_core','todo','Todo',1,0,NULL,3,4), ('col_fixture','b_fixture','todo','Todo',1,0,NULL,7,8);
INSERT INTO tasks(id,board_id,seq,idempotency_key,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version) VALUES
 ('t_core','b_core',1,NULL,'Core task',NULL,'todo',NULL,NULL,3,1,NULL,NULL,'tester',10,11,0,NULL,NULL,'{"ok":true}','{"source":"fixture"}',0),
 ('t_child','b_core',2,NULL,'Child task',NULL,'todo',NULL,NULL,3,2,NULL,NULL,'tester',12,13,0,NULL,NULL,NULL,'{}',0),
 ('t_fixture','b_fixture',1,NULL,'Fixture task',NULL,'todo',NULL,NULL,3,1,NULL,NULL,'tester',17,18,0,NULL,NULL,NULL,'{}',0);
INSERT INTO task_execution_plans(board_id,task_id,state,reason,updated_by,updated_at) VALUES ('b_core','t_core','planned','fixture plan','tester',14);
INSERT INTO task_steps(id,board_id,parent_task_id,idempotency_key,position,title,body,linked_task_id,required,status,resolution_note,resolved_by,resolved_at,created_by,created_at,updated_by,updated_at) VALUES ('step_core','b_core','t_core',NULL,1,'Step','step body',NULL,1,'todo',NULL,NULL,NULL,'tester',15,'tester',16);
INSERT INTO task_dependencies(board_id,parent_task_id,child_task_id,created_at) VALUES ('b_core','t_core','t_child',20);
INSERT INTO task_runs(id,board_id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,claim_expires_at,started_at,last_heartbeat_at,finished_at,exit_code,summary,error,log_path,metadata_json) VALUES ('r_core','b_core','t_core','succeeded','manual',7,'claim-core','tester',100,21,22,23,0,'done',NULL,NULL,'{}');
INSERT INTO task_comments(id,board_id,task_id,idempotency_key,author,author_type,agent_type,body,kind,metadata_json,created_at) VALUES ('c_core','b_core','t_core',NULL,'tester','user',NULL,'portable comment','note','{"source":"fixture"}',31);
INSERT INTO task_events(event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at) VALUES ('e_core','b_core','t_core','r_core','custom.opaque','tester','["opaque",1]',32);
INSERT INTO task_attachments(id,board_id,task_id,filename,rel_path,content_type,size_bytes,sha256,created_by,created_at) VALUES ('a_core','b_core','t_core','artifact.txt','attachments/artifact.txt','text/plain',7,NULL,'tester',33);
INSERT INTO labels(id,board_id,name,color,created_at,updated_at) VALUES ('l_core','b_core','core',NULL,34,35), ('l_fixture','b_fixture','rust',NULL,34,35);
INSERT INTO task_labels(board_id,task_id,label_id,created_at) VALUES ('b_core','t_core','l_core',36);
INSERT INTO app_settings(key,value_json,updated_at) VALUES ('contract.fixture','{"enabled":true}',37);
INSERT INTO task_subtasks(board_id,parent_task_id,child_task_id,position,required,created_by,created_at) VALUES ('b_core','t_core','t_child',1,1,'tester',38);
INSERT INTO entities(uri,kind,source_table,source_id,board_id,task_id,title,summary,content_hash,created_at,updated_at,archived_at) VALUES
 ('kb://task/t_core','task','tasks','t_core','b_core','t_core','Core task',NULL,NULL,39,40,NULL),
 ('kb://task/t_child','task','tasks','t_child','b_core','t_child','Child task',NULL,NULL,41,42,NULL),
 ('kb://task/t_fixture','task','tasks','t_fixture','b_fixture','t_fixture','Fixture task',NULL,NULL,41,42,NULL);
INSERT INTO relation_predicates(name,domain_kind,range_kind,cardinality,authoritative_store,description,created_at) VALUES ('portable_depends','task','task','many','turso','portable relation',43);
INSERT INTO entity_relations(subject_uri,predicate,object_uri,graph_uri,board_id,authoritative_store,source_table,source_id,source_event_id,metadata_json,created_at,updated_at) VALUES ('kb://task/t_core','portable_depends','kb://task/t_child','kb://graph/b_core','b_core','turso','tasks','t_core',NULL,'{}',44,45);
INSERT INTO label_semantics(label_id,board_id,description,applies_when,excludes_when,positive_examples,negative_examples,created_at,updated_at) VALUES ('l_fixture','b_fixture','fixture semantics','["cargo"]','[]','["rust"]','[]',46,47);
INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at) VALUES ('la_fixture','l_fixture','b_fixture','positive','positive_example','cargo',0,'atom-hash',48,49);
INSERT INTO label_atom_index_boards(store_name,board_id,dirty,last_rebuild_at,last_error,updated_at) VALUES ('fts','b_fixture',1,NULL,NULL,50);
INSERT INTO label_semantic_proposals(id,board_id,task_id,status,name,description,applies_when,excludes_when,positive_examples,negative_examples,heuristic_coverage,heuristic_residual_norm,heuristic_coverage_cosine,top1_existing_label_id,top1_existing_label_name,diagnostics_json,created_by,decision_reason,resolved_label_id,created_at,updated_at,decided_at) VALUES ('lp_fixture','b_fixture','t_fixture','proposed','fixture proposal','proposal','[]','[]','["cargo"]','[]',0.5,0.5,NULL,'l_fixture','rust','[]','tester',NULL,NULL,51,52,NULL);
INSERT INTO label_ontology_observations(id,board_id,task_id,task_ref_snapshot,task_snapshot_json,agent_candidates_json,suggestion_snapshot_json,final_decision_json,suggest_coverage,suggest_coverage_cosine,suggest_residual_norm,suggest_needs_new_label,suggest_degraded,diagnostics_json,capture_fingerprint,suggest_input_hash,created_by,created_by_type,agent_type,created_at) VALUES ('lor_fixture','b_fixture','t_fixture','default#1','{}','[]','{}','{}',NULL,NULL,NULL,0,0,'[]','capture-fixture',NULL,'tester','user',NULL,53);
INSERT INTO label_ontology_signals(id,board_id,observation_id,kind,status,target_label_id,target_label_name_snapshot,related_labels_json,proposed_action,candidate_atom_polarity,candidate_atom_kind,candidate_text,candidate_content_hash,proposed_label_name,proposed_label_name_normalized,proposal_json,agent_selected,suggest_state,suggest_score,suggest_rank,final_selected,rationale,confidence,signal_key,superseded_by_signal_id,status_reason,created_at,updated_at,reviewed_at,closed_at) VALUES ('los_fixture','b_fixture','lor_fixture','vocabulary_gap','open','l_fixture','rust','[]','observe',NULL,NULL,NULL,NULL,NULL,NULL,'{}',0,NULL,NULL,NULL,0,'fixture rationale',NULL,'fixture-key',NULL,NULL,54,55,NULL,NULL);
INSERT INTO label_ontology_actions(id,board_id,parent_action_id,action_type,reason,target_label_id,result_label_id,result_atom_id,result_atom_content_hash,result_proposal_id,canonical_before_hash,canonical_after_hash,change_json,validation_status,validation_json,validation_requirement,created_by,created_by_type,agent_type,created_at) VALUES ('loa_fixture','b_fixture',NULL,'confirm','fixture action','l_fixture',NULL,NULL,NULL,NULL,NULL,NULL,'{}','not_required','{}','none','tester','user',NULL,56);
INSERT INTO label_ontology_action_signals(board_id,action_id,signal_id,created_at) VALUES ('b_fixture','loa_fixture','los_fixture',57);
INSERT INTO label_ontology_action_atom_effects(board_id,action_id,label_id_snapshot,atom_id_snapshot,atom_content_hash,polarity,kind,text,effect,created_at) VALUES ('b_fixture','loa_fixture','l_fixture','la_fixture','atom-hash','positive','positive_example','cargo','added',58);
INSERT INTO signal_observations(id,board_id,task_id,run_id,comment_id,task_ref_snapshot,actor,agent_type,source,evidence_json,created_at) VALUES ('obs_fixture','b_fixture','t_fixture',NULL,NULL,'default#1','tester','codex','contract-test','{}',59);
INSERT INTO signals(id,board_id,observation_id,kind,title,summary,severity,status,dedupe_key,superseded_by_signal_id,reviewed_by,reviewed_at,review_reason,created_at,updated_at) VALUES ('sig_fixture','b_fixture','obs_fixture','quality','Fixture signal','Fixture summary','info','open','fixture',NULL,NULL,NULL,NULL,60,61);
COMMIT;
"#,
        )
        .await
        .map_err(|error| error.to_string())?;
    connection
        .execute("DELETE FROM projection_jobs", ())
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// 校验 portable JSONL 的 canonical 表、完整列集合和逐表行数，并返回逐表记录。
pub async fn validate_portable_export(
    source_path: &Path,
    export_path: &Path,
) -> Result<BTreeMap<String, Vec<Value>>, String> {
    let store = TursoStore::open(source_path)
        .await
        .map_err(|error| error.to_string())?;
    let connection = store
        .connection()
        .await
        .map_err(|error| error.to_string())?;
    let bytes = fs::read(export_path).map_err(|error| error.to_string())?;
    let mut lines = bytes.split(|byte| *byte == b'\n');
    let header: Value = serde_json::from_slice(
        lines
            .next()
            .ok_or_else(|| "portable 导出缺少 header".to_owned())?,
    )
    .map_err(|error| error.to_string())?;
    let canonical = header
        .get("canonical_tables")
        .and_then(Value::as_array)
        .ok_or_else(|| "portable header 缺少 canonical_tables".to_owned())?;
    assert_eq!(
        canonical,
        &PORTABLE_TABLES
            .iter()
            .map(|table| Value::String((*table).to_owned()))
            .collect::<Vec<_>>()
    );

    let mut records_by_table = BTreeMap::<String, Vec<Value>>::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let record: Value = serde_json::from_slice(line).map_err(|error| error.to_string())?;
        let table = record
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("portable record 缺少 table：{record}"))?;
        let data = record
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("portable {table} record 缺少 data"))?;
        assert!(PORTABLE_TABLES.contains(&table));
        let columns = table_columns(&connection, table).await?;
        let mut actual = data.keys().cloned().collect::<Vec<_>>();
        actual.sort();
        let mut expected = columns;
        expected.sort();
        assert_eq!(actual, expected, "portable {table} 列集合必须完整");
        *counts.entry(table.to_owned()).or_default() += 1;
        records_by_table
            .entry(table.to_owned())
            .or_default()
            .push(Value::Object(data.clone().into_iter().collect()));
    }

    let header_counts = header
        .get("table_counts")
        .and_then(Value::as_object)
        .ok_or_else(|| "portable header 缺少 table_counts".to_owned())?;
    for table in PORTABLE_TABLES {
        let expected = table_count(&connection, table).await? as usize;
        assert_eq!(counts.get(*table).copied().unwrap_or_default(), expected);
        assert_eq!(
            header_counts
                .get(*table)
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize,
            expected
        );
    }
    Ok(records_by_table)
}

/// 比较 portable round-trip/replace 的 canonical JSONL 事实，并核验关键关系字段。
pub fn assert_portable_facts_equal(source: &Path, target: &Path) -> Result<(), String> {
    let source_records = read_export(source)?;
    let target_records = read_export(target)?;
    assert_eq!(
        source_records, target_records,
        "portable canonical records 发生变化"
    );
    let attachment = find_record(&target_records, "task_attachments", "id", "a_core")?;
    assert_eq!(attachment["rel_path"], "attachments/artifact.txt");
    assert_eq!(attachment["size_bytes"], 7);
    assert_eq!(attachment["sha256"], Value::Null);
    assert_eq!(attachment["created_at"], 33);

    let dependency = find_record(
        &target_records,
        "task_dependencies",
        "parent_task_id",
        "t_core",
    )?;
    assert_eq!(dependency["parent_task_id"], "t_core");
    assert_eq!(dependency["child_task_id"], "t_child");
    assert_eq!(dependency["created_at"], 20);
    Ok(())
}

/// 校验 legacy v30 HTTP 导入后导出的 canonical facts。
pub fn assert_legacy_target_facts(export_path: &Path) -> Result<(), String> {
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
    let task = task.ok_or("缺少 legacy task fact")?;
    assert_eq!(task["board_id"], "b_legacy");
    assert_eq!(task["created_at"], 100);
    assert_eq!(task["updated_at"], 101);
    let dependency = dependency.ok_or("缺少 legacy dependency fact")?;
    assert_eq!(dependency["board_id"], "b_legacy");
    assert_eq!(dependency["parent_task_id"], "t_legacy");
    assert_eq!(dependency["child_task_id"], "t_child");
    assert_eq!(dependency["created_at"], 110);
    let attachment = attachment.ok_or("缺少 legacy attachment fact")?;
    assert_eq!(attachment["rel_path"], "attachments/legacy.txt");
    assert_eq!(attachment["size_bytes"], 7);
    assert_eq!(attachment["created_at"], 120);
    Ok(())
}

async fn table_columns(connection: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut statement = connection
        .query(
            &format!("PRAGMA table_info(\"{}\")", table.replace('"', "\"\"")),
            (),
        )
        .await
        .map_err(|error| error.to_string())?;
    let mut columns = Vec::new();
    while let Some(row) = statement.next().await.map_err(|error| error.to_string())? {
        columns.push(
            text_value(
                row.get_value(1).map_err(|error| error.to_string())?,
                "portable.table_info.name",
            )
            .map_err(|error| error.to_string())?,
        );
    }
    Ok(columns)
}

async fn table_count(connection: &Connection, table: &str) -> Result<i64, String> {
    let mut rows = connection
        .query(
            &format!("SELECT COUNT(*) FROM \"{}\"", table.replace('"', "\"\"")),
            (),
        )
        .await
        .map_err(|error| error.to_string())?;
    let row = rows
        .next()
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("portable table count 缺少 {table}"))?;
    integer_value(
        row.get_value(0).map_err(|error| error.to_string())?,
        "portable.table_count",
    )
    .map_err(|error| error.to_string())
}

fn read_export(path: &Path) -> Result<BTreeMap<String, Vec<Value>>, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let mut lines = bytes.split(|byte| *byte == b'\n');
    lines.next();
    let mut records = BTreeMap::<String, Vec<Value>>::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let record: Value = serde_json::from_slice(line).map_err(|error| error.to_string())?;
        let table = record
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("portable record 缺少 table：{record}"))?;
        let data = record
            .get("data")
            .cloned()
            .ok_or_else(|| format!("portable {table} record 缺少 data"))?;
        records.entry(table.to_owned()).or_default().push(data);
    }
    Ok(records)
}

fn find_record<'a>(
    records: &'a BTreeMap<String, Vec<Value>>,
    table: &str,
    key: &str,
    expected: &str,
) -> Result<&'a Map<String, Value>, String> {
    records
        .get(table)
        .and_then(|rows| {
            rows.iter().find_map(|record| {
                let object = record.as_object()?;
                (object.get(key).and_then(Value::as_str) == Some(expected)).then_some(object)
            })
        })
        .ok_or_else(|| format!("portable {table} record 缺少 {key}={expected}"))
}

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

/// 构造真实 SQLite v30 legacy source，供 HTTP importer adoption 使用。
pub fn make_legacy_source(directory: &Path) -> Result<PathBuf, String> {
    let source_path = directory.join("legacy-v30.sqlite");
    let connection = SqliteConnection::open(&source_path).map_err(|error| error.to_string())?;
    let migration_files = migration_files()?;
    assert_eq!(migration_files.len(), LEGACY_MIGRATION_CHECKSUMS.len());
    for (name, path) in &migration_files {
        let sql = git_show(path)?;
        connection
            .execute_batch(&sql)
            .map_err(|error| format!("应用 legacy migration {name} 失败：{error}"))?;
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
        return Err(format!("legacy migration ledger 不匹配：{ledger:?}"));
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
                .ok_or_else(|| format!("legacy migration path 无效：{path}"))?;
            Ok((name.to_owned(), path.to_owned()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    if files.is_empty() {
        return Err(format!(
            "legacy migration history 不可用：status={} stdout={} stderr={}",
            listing.status,
            String::from_utf8_lossy(&listing.stdout),
            String::from_utf8_lossy(&listing.stderr)
        ));
    }
    Ok(files)
}

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

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("kanban workspace root 存在")
}
