use std::{collections::BTreeMap, fs, path::Path};

use kanban_contract::jsonl_core::{
    AttachmentJsonlData, AttachmentJsonlInput, AttachmentJsonlOutput, AttachmentJsonlType,
    BoardJsonlData, BoardJsonlInput, BoardJsonlOutput, BoardJsonlType, ColumnJsonlData,
    ColumnJsonlInput, ColumnJsonlOutput, ColumnJsonlType, CommentJsonlData, CommentJsonlInput,
    CommentJsonlOutput, CommentJsonlType, DependencyJsonlData, DependencyJsonlInput,
    DependencyJsonlOutput, DependencyJsonlType, EventJsonlData, EventJsonlInput, EventJsonlOutput,
    EventJsonlType, PortableRunStatus, PortableTaskStatus, RunJsonlData, RunJsonlInput,
    RunJsonlOutput, RunJsonlType, TaskJsonlData, TaskJsonlInput, TaskJsonlOutput, TaskJsonlType,
    TaskLabelJsonlData, TaskLabelJsonlInput, TaskLabelJsonlOutput, TaskLabelJsonlType,
};
use kanban_contract::{ApiTaskPriority, CommentAuthorType, CommentKind, portable_contract_catalog};
use kanban_core::TaskStatus;
use kanban_sqlite::{
    api::{
        CreateTask, claim_task, create_task, export_jsonl_to_writer, import_jsonl,
        mark_execution_plan_not_required,
    },
    db::connect_file,
    init::init_database,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

const FIXTURE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/fixtures/jsonl");

struct TempDb {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(prefix: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .expect("temporary database directory");
        let path = dir.path().join("kanban.db");
        init_database(&path, "tester").expect("initialize temporary database");
        Self { _dir: dir, path }
    }
}

fn fixture(name: &str) -> Value {
    let path = Path::new(FIXTURE_ROOT).join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("read committed fixture"))
        .expect("fixture is JSON")
}

fn assert_output_contract_fixture<T>(valid_name: &str, invalid_name: &str)
where
    T: DeserializeOwned + Serialize,
{
    let valid = fixture(valid_name);
    let typed = serde_json::from_value::<T>(valid.clone()).expect("valid fixture is consumed");
    assert_eq!(
        serde_json::to_value(typed).expect("serialize fixture DTO"),
        valid
    );
    assert!(
        serde_json::from_value::<T>(fixture(invalid_name)).is_err(),
        "invalid fixture must fail closed"
    );
}

fn programmatic_input(discriminator: &str) -> Value {
    let priority = ApiTaskPriority::new(3).expect("P3 priority");
    let value = match discriminator {
        "board" => serde_json::to_value(BoardJsonlInput {
            record_type: BoardJsonlType::Board,
            data: BoardJsonlData {
                id: "b_core".into(),
                slug: "core".into(),
                name: "Core".into(),
                description: None,
                created_at: 1,
                updated_at: 2,
                archived_at: None,
            },
        }),
        "column" => serde_json::to_value(ColumnJsonlInput {
            record_type: ColumnJsonlType::Column,
            data: ColumnJsonlData {
                id: "col_core".into(),
                board_id: "b_core".into(),
                status: PortableTaskStatus::Todo,
                title: "Todo".into(),
                position: 1,
                hidden: false,
                wip_limit: None,
                created_at: 3,
                updated_at: 4,
            },
        }),
        "task" => serde_json::to_value(TaskJsonlInput {
            record_type: TaskJsonlType::Task,
            data: TaskJsonlData {
                id: "t_core".into(),
                board_id: "b_core".into(),
                seq: 1,
                title: "Core task".into(),
                description: None,
                status: PortableTaskStatus::Todo,
                status_reason: None,
                assignee: None,
                priority,
                position: 1024,
                scheduled_at: None,
                due_at: None,
                created_by: "tester".into(),
                created_at: 10,
                updated_at: 11,
                started_at: None,
                completed_at: None,
                archived_at: None,
                claim_token: None,
                claim_owner: None,
                claim_expires_at: None,
                last_heartbeat_at: None,
                current_run_id: None,
                retry_count: 0,
                max_retries: None,
                result_summary: None,
                result: Some(json!({"ok": true})),
                metadata: json!([{"source": "fixture"}]),
                lock_version: 0,
            },
        }),
        "dependency" => serde_json::to_value(DependencyJsonlInput {
            record_type: DependencyJsonlType::Dependency,
            data: DependencyJsonlData {
                board_id: "b_core".into(),
                parent_task_id: "t_core".into(),
                child_task_id: "t_child".into(),
                created_at: 20,
            },
        }),
        "run" => serde_json::to_value(RunJsonlInput {
            record_type: RunJsonlType::Run,
            data: RunJsonlData {
                id: "r_core".into(),
                board_id: "b_core".into(),
                task_id: "t_core".into(),
                status: PortableRunStatus::Succeeded,
                worker_profile: None,
                worker_pid: None,
                claim_token: "historical-token".into(),
                claim_owner: "tester".into(),
                claim_expires_at: 25,
                started_at: 21,
                last_heartbeat_at: None,
                finished_at: Some(30),
                exit_code: Some(0),
                summary: Some("done".into()),
                error: None,
                log_path: None,
                metadata: json!(42),
            },
        }),
        "comment" => serde_json::to_value(CommentJsonlInput {
            record_type: CommentJsonlType::Comment,
            data: CommentJsonlData {
                id: "c_core".into(),
                board_id: "b_core".into(),
                task_id: "t_core".into(),
                author: "tester".into(),
                author_type: CommentAuthorType::User,
                agent_type: None,
                body: "portable".into(),
                kind: CommentKind::Note,
                metadata: BTreeMap::from([("source".into(), json!("fixture"))]),
                created_at: 31,
            },
        }),
        "event" => serde_json::to_value(EventJsonlInput {
            record_type: EventJsonlType::Event,
            data: EventJsonlData {
                id: 1,
                event_id: "e_core".into(),
                board_id: "b_core".into(),
                task_id: Some("t_core".into()),
                run_id: Some("r_core".into()),
                kind: "custom.opaque".into(),
                actor: Some("tester".into()),
                payload: json!(["opaque", 1]),
                created_at: 32,
            },
        }),
        "attachment" => serde_json::to_value(AttachmentJsonlInput {
            record_type: AttachmentJsonlType::Attachment,
            data: AttachmentJsonlData {
                id: "a_core".into(),
                board_id: "b_core".into(),
                task_id: "t_core".into(),
                filename: "artifact.txt".into(),
                rel_path: "attachments/artifact.txt".into(),
                content_type: Some("text/plain".into()),
                size_bytes: 7,
                sha256: None,
                created_by: "tester".into(),
                created_at: 33,
            },
        }),
        "task_label" => serde_json::to_value(TaskLabelJsonlInput {
            record_type: TaskLabelJsonlType::TaskLabel,
            data: TaskLabelJsonlData {
                board_id: "b_core".into(),
                task_id: "t_core".into(),
                label_id: "l_core".into(),
                created_at: 34,
            },
        }),
        _ => panic!("unexpected core discriminator: {discriminator}"),
    };
    value.expect("serialize programmatic input DTO")
}

fn canonical_input_records() -> Vec<Value> {
    let task = fixture("task-input.v1.valid.json");
    let mut child = task.clone();
    let child_data = child["data"].as_object_mut().expect("task data object");
    child_data.insert("id".into(), json!("t_child"));
    child_data.insert("seq".into(), json!(2));
    child_data.insert("title".into(), json!("Child task"));
    child_data.insert("position".into(), json!(2048));

    vec![
        fixture("board-input.v1.valid.json"),
        fixture("column-input.v1.valid.json"),
        task,
        child,
        fixture("dependency-input.v1.valid.json"),
        fixture("run-input.v1.valid.json"),
        fixture("comment-input.v1.valid.json"),
        fixture("event-input.v1.valid.json"),
        fixture("attachment-input.v1.valid.json"),
        json!({
            "type": "label",
            "data": {
                "id": "l_core",
                "board_id": "b_core",
                "name": "core",
                "color": null,
                "created_at": 5,
                "updated_at": 5
            }
        }),
        fixture("task_label-input.v1.valid.json"),
    ]
}

fn write_jsonl(path: &Path, records: &[Value]) {
    let body = records
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(path, body).expect("write JSONL fixture");
}

fn import_canonical_db() -> TempDb {
    let db = TempDb::new("portable-core-import-");
    let input = db._dir.path().join("input.jsonl");
    write_jsonl(&input, &canonical_input_records());
    import_jsonl(&db.path, &input, true).expect("real importer consumes core contracts");
    db
}

fn assert_real_import(discriminator: &str) {
    let db = import_canonical_db();
    let conn = connect_file(&db.path).expect("connect imported DB");
    match discriminator {
        "board" => assert_eq!(conn.query_row("SELECT slug FROM boards WHERE id='b_core'", [], |row| row.get::<_, String>(0)).expect("board"), "core"),
        "column" => assert_eq!(conn.query_row("SELECT hidden FROM board_columns WHERE id='col_core'", [], |row| row.get::<_, i64>(0)).expect("column"), 0),
        "task" => assert_eq!(conn.query_row("SELECT result_json,metadata_json,priority FROM tasks WHERE id='t_core'", [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))).expect("task"), (r#"{"ok":true}"#.into(), r#"[{"source":"fixture"}]"#.into(), 3)),
        "dependency" => assert_eq!(conn.query_row("SELECT COUNT(*) FROM task_dependencies WHERE parent_task_id='t_core' AND child_task_id='t_child'", [], |row| row.get::<_, i64>(0)).expect("dependency"), 1),
        "run" => assert_eq!(conn.query_row("SELECT metadata_json,log_path FROM task_runs WHERE id='r_core'", [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))).expect("run"), ("42".into(), None)),
        "comment" => assert_eq!(conn.query_row("SELECT author_type,kind,metadata_json FROM task_comments WHERE id='c_core'", [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))).expect("comment"), ("user".into(), "note".into(), r#"{"source":"fixture"}"#.into())),
        "event" => assert_eq!(conn.query_row("SELECT payload_json FROM task_events WHERE event_id='e_core'", [], |row| row.get::<_, String>(0)).expect("event"), r#"["opaque",1]"#),
        "attachment" => assert_eq!(conn.query_row("SELECT filename FROM task_attachments WHERE id='a_core'", [], |row| row.get::<_, String>(0)).expect("attachment"), "artifact.txt"),
        "task_label" => assert_eq!(conn.query_row("SELECT COUNT(*) FROM task_labels WHERE task_id='t_core' AND label_id='l_core'", [], |row| row.get::<_, i64>(0)).expect("task label"), 1),
        _ => panic!("unexpected core discriminator: {discriminator}"),
    }
}

fn seed_output_db() -> TempDb {
    let db = TempDb::new("portable-core-output-");
    connect_file(&db.path)
        .expect("connect output seed")
        .execute_batch(
            r#"
            DELETE FROM task_events;
            INSERT INTO boards(id,slug,name,description,created_at,updated_at,archived_at) VALUES ('b_core','core','Core',NULL,1,2,NULL);
            INSERT INTO board_columns(id,board_id,status,title,position,hidden,wip_limit,created_at,updated_at) VALUES ('col_core','b_core','todo','Todo',1,0,NULL,3,4);
            INSERT INTO tasks(id,board_id,seq,title,description,status,status_reason,assignee,priority,position,scheduled_at,due_at,created_by,created_at,updated_at,started_at,completed_at,archived_at,claim_token,claim_owner,claim_expires_at,last_heartbeat_at,current_run_id,retry_count,max_retries,result_summary,result_json,metadata_json,lock_version) VALUES
              ('t_core','b_core',1,'Core task',NULL,'todo',NULL,NULL,3,1024,NULL,NULL,'tester',10,11,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,0,NULL,NULL,'{"ok":true}','[{"source":"fixture"}]',0),
              ('t_child','b_core',2,'Child task',NULL,'todo',NULL,NULL,3,2048,NULL,NULL,'tester',10,11,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,0,NULL,NULL,'{"ok":true}','[{"source":"fixture"}]',0);
            INSERT INTO task_dependencies(board_id,parent_task_id,child_task_id,created_at) VALUES ('b_core','t_core','t_child',20);
            INSERT INTO task_runs(id,board_id,task_id,status,worker_profile,worker_pid,claim_token,claim_owner,claim_expires_at,started_at,last_heartbeat_at,finished_at,exit_code,summary,error,log_path,metadata_json) VALUES ('r_core','b_core','t_core','succeeded',NULL,NULL,'historical-token','tester',25,21,NULL,30,0,'done',NULL,NULL,'42');
            INSERT INTO task_comments(id,board_id,task_id,author,author_type,agent_type,body,kind,metadata_json,created_at) VALUES ('c_core','b_core','t_core','tester','user',NULL,'portable','note','{"source":"fixture"}',31);
            INSERT INTO task_events(id,event_id,board_id,task_id,run_id,kind,actor,payload_json,created_at) VALUES (1,'e_core','b_core','t_core','r_core','custom.opaque','tester','["opaque",1]',32);
            INSERT INTO task_attachments(id,board_id,task_id,filename,rel_path,content_type,size_bytes,sha256,created_by,created_at) VALUES ('a_core','b_core','t_core','artifact.txt','attachments/artifact.txt','text/plain',7,NULL,'tester',33);
            INSERT INTO labels(id,board_id,name,color,created_at,updated_at) VALUES ('l_core','b_core','core',NULL,5,5);
            INSERT INTO task_labels(board_id,task_id,label_id,created_at) VALUES ('b_core','t_core','l_core',34);
            "#,
        )
        .expect("seed output DB without importer");
    db
}

fn output_records_from_seed() -> Vec<Value> {
    let db = seed_output_db();
    let mut out = Vec::new();
    export_jsonl_to_writer(&db.path, "core", &mut out)
        .expect("real exporter produces core contracts");
    String::from_utf8(out)
        .expect("JSONL is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("export line is JSON"))
        .collect()
}

fn assert_real_export(expected_fixture: &str) {
    let expected = fixture(expected_fixture);
    assert!(
        output_records_from_seed()
            .iter()
            .any(|actual| actual == &expected),
        "real export did not produce {expected_fixture}"
    );
}

macro_rules! adoption_tests {
    (
        $input_producer:ident,
        $input_consumer:ident,
        $output_producer:ident,
        $output_consumer:ident,
        $input_ty:ty,
        $output_ty:ty,
        $discriminator:literal,
        $input_valid:literal,
        $input_invalid:literal,
        $output_valid:literal,
        $output_invalid:literal
    ) => {
        #[test]
        fn $input_producer() {
            assert_eq!(programmatic_input($discriminator), fixture($input_valid));
            assert!(serde_json::from_value::<$input_ty>(fixture($input_invalid)).is_err());
        }

        #[test]
        fn $input_consumer() {
            assert_real_import($discriminator);
        }

        #[test]
        fn $output_producer() {
            assert_real_export($output_valid);
        }

        #[test]
        fn $output_consumer() {
            assert_output_contract_fixture::<$output_ty>($output_valid, $output_invalid);
        }
    };
}

adoption_tests!(
    board_input_fixture_is_produced_by_contract,
    board_input_fixture_is_consumed_by_real_import,
    board_output_fixture_is_produced_by_real_export,
    board_output_fixture_is_consumed_by_contract,
    BoardJsonlInput,
    BoardJsonlOutput,
    "board",
    "board-input.v1.valid.json",
    "board-input.v1.invalid.json",
    "board-output.v1.valid.json",
    "board-output.v1.invalid.json"
);
adoption_tests!(
    column_input_fixture_is_produced_by_contract,
    column_input_fixture_is_consumed_by_real_import,
    column_output_fixture_is_produced_by_real_export,
    column_output_fixture_is_consumed_by_contract,
    ColumnJsonlInput,
    ColumnJsonlOutput,
    "column",
    "column-input.v1.valid.json",
    "column-input.v1.invalid.json",
    "column-output.v1.valid.json",
    "column-output.v1.invalid.json"
);
adoption_tests!(
    task_input_fixture_is_produced_by_contract,
    task_input_fixture_is_consumed_by_real_import,
    task_output_fixture_is_produced_by_real_export,
    task_output_fixture_is_consumed_by_contract,
    TaskJsonlInput,
    TaskJsonlOutput,
    "task",
    "task-input.v1.valid.json",
    "task-input.v1.invalid.json",
    "task-output.v1.valid.json",
    "task-output.v1.invalid.json"
);
adoption_tests!(
    dependency_input_fixture_is_produced_by_contract,
    dependency_input_fixture_is_consumed_by_real_import,
    dependency_output_fixture_is_produced_by_real_export,
    dependency_output_fixture_is_consumed_by_contract,
    DependencyJsonlInput,
    DependencyJsonlOutput,
    "dependency",
    "dependency-input.v1.valid.json",
    "dependency-input.v1.invalid.json",
    "dependency-output.v1.valid.json",
    "dependency-output.v1.invalid.json"
);
adoption_tests!(
    run_input_fixture_is_produced_by_contract,
    run_input_fixture_is_consumed_by_real_import,
    run_output_fixture_is_produced_by_real_export,
    run_output_fixture_is_consumed_by_contract,
    RunJsonlInput,
    RunJsonlOutput,
    "run",
    "run-input.v1.valid.json",
    "run-input.v1.invalid.json",
    "run-output.v1.valid.json",
    "run-output.v1.invalid.json"
);
adoption_tests!(
    comment_input_fixture_is_produced_by_contract,
    comment_input_fixture_is_consumed_by_real_import,
    comment_output_fixture_is_produced_by_real_export,
    comment_output_fixture_is_consumed_by_contract,
    CommentJsonlInput,
    CommentJsonlOutput,
    "comment",
    "comment-input.v1.valid.json",
    "comment-input.v1.invalid.json",
    "comment-output.v1.valid.json",
    "comment-output.v1.invalid.json"
);
adoption_tests!(
    event_input_fixture_is_produced_by_contract,
    event_input_fixture_is_consumed_by_real_import,
    event_output_fixture_is_produced_by_real_export,
    event_output_fixture_is_consumed_by_contract,
    EventJsonlInput,
    EventJsonlOutput,
    "event",
    "event-input.v1.valid.json",
    "event-input.v1.invalid.json",
    "event-output.v1.valid.json",
    "event-output.v1.invalid.json"
);
adoption_tests!(
    attachment_input_fixture_is_produced_by_contract,
    attachment_input_fixture_is_consumed_by_real_import,
    attachment_output_fixture_is_produced_by_real_export,
    attachment_output_fixture_is_consumed_by_contract,
    AttachmentJsonlInput,
    AttachmentJsonlOutput,
    "attachment",
    "attachment-input.v1.valid.json",
    "attachment-input.v1.invalid.json",
    "attachment-output.v1.valid.json",
    "attachment-output.v1.invalid.json"
);
adoption_tests!(
    task_label_input_fixture_is_produced_by_contract,
    task_label_input_fixture_is_consumed_by_real_import,
    task_label_output_fixture_is_produced_by_real_export,
    task_label_output_fixture_is_consumed_by_contract,
    TaskLabelJsonlInput,
    TaskLabelJsonlOutput,
    "task_label",
    "task_label-input.v1.valid.json",
    "task_label-input.v1.invalid.json",
    "task_label-output.v1.valid.json",
    "task_label-output.v1.invalid.json"
);

#[test]
fn natural_json_values_roundtrip_without_double_encoding() {
    let exported = output_records_from_seed();
    let task = exported
        .iter()
        .find(|row| row["type"] == "task" && row["data"]["id"] == "t_core")
        .expect("task row");
    let run = exported
        .iter()
        .find(|row| row["type"] == "run")
        .expect("run row");
    let event = exported
        .iter()
        .find(|row| row["type"] == "event" && row["data"]["event_id"] == "e_core")
        .expect("event row");
    assert_eq!(task["data"]["metadata"], json!([{"source": "fixture"}]));
    assert_eq!(task["data"]["result"], json!({"ok": true}));
    assert_eq!(run["data"]["metadata"], json!(42));
    assert_eq!(event["data"]["payload"], json!(["opaque", 1]));
    assert!(task["data"].get("metadata_json").is_none());
    assert!(run["data"].get("metadata_json").is_none());
    assert!(event["data"].get("payload_json").is_none());
}

fn parent_exporter_core_records() -> Vec<Value> {
    let mut records = canonical_input_records();
    for task in records.iter_mut().filter(|row| row["type"] == "task") {
        let data = task["data"].as_object_mut().expect("task data");
        let metadata = data.remove("metadata").expect("task metadata");
        data.insert("metadata_json".into(), json!(metadata.to_string()));
        let result = data.remove("result").expect("task result");
        data.insert("result_json".into(), json!(result.to_string()));
    }
    let column = records
        .iter_mut()
        .find(|row| row["type"] == "column")
        .expect("column row");
    column["data"]["hidden"] = json!(0);
    let event = records
        .iter_mut()
        .find(|row| row["type"] == "event")
        .expect("event row");
    let payload = event["data"]
        .as_object_mut()
        .expect("event data")
        .remove("payload")
        .expect("event payload");
    event["data"]["payload_json"] = json!(payload.to_string());
    for record_type in ["run", "comment"] {
        let record = records
            .iter_mut()
            .find(|row| row["type"] == record_type)
            .expect("metadata-bearing row");
        let metadata = record["data"]
            .as_object_mut()
            .expect("record data")
            .remove("metadata")
            .expect("record metadata");
        record["data"]["metadata_json"] = json!(metadata.to_string());
    }
    records
}

#[test]
fn importer_migrates_parent_exporter_json_text_keys_and_integer_booleans() {
    let db = TempDb::new("portable-core-legacy-key-");
    let records = parent_exporter_core_records();
    let input = db._dir.path().join("legacy.jsonl");
    write_jsonl(&input, &records);
    import_jsonl(&db.path, &input, true).expect("parent exporter snapshot must migrate");

    let mut output = Vec::new();
    export_jsonl_to_writer(&db.path, "b_core", &mut output).expect("re-export migrated snapshot");
    let exported = String::from_utf8(output).expect("utf-8 re-export");
    assert!(!exported.contains("metadata_json"), "{exported}");
    assert!(!exported.contains("result_json"), "{exported}");
    assert!(!exported.contains("payload_json"), "{exported}");
    assert!(!exported.contains(r#""hidden":0"#), "{exported}");
}

#[test]
fn importer_rejects_hybrid_core_records_before_normalization_and_rolls_back_replace() {
    for (record_type, natural_field, storage_field) in [
        ("task", "result", "result_json"),
        ("task", "metadata", "metadata_json"),
        ("run", "metadata", "metadata_json"),
        ("comment", "metadata", "metadata_json"),
        ("event", "payload", "payload_json"),
    ] {
        let db = TempDb::new(&format!(
            "portable-core-hybrid-{record_type}-{natural_field}-"
        ));
        let mut records = parent_exporter_core_records();
        let record = records
            .iter_mut()
            .find(|row| row["type"] == record_type)
            .expect("hybrid record");
        let data = record["data"].as_object_mut().expect("record data");
        let natural_value = match data.get(storage_field).expect("storage-native field") {
            Value::Null => Value::Null,
            Value::String(text) => serde_json::from_str(text).expect("storage JSON text"),
            other => panic!("unexpected storage-native value: {other}"),
        };
        data.insert(natural_field.into(), natural_value);

        let input = db._dir.path().join("hybrid.jsonl");
        write_jsonl(&input, &records);
        let error = import_jsonl(&db.path, &input, true)
            .expect_err("same-record natural/storage-native keys must be rejected");
        let message = error.to_string();
        assert!(
            message.contains("cannot contain both natural and parent storage-native keys"),
            "{message}"
        );
        assert!(message.contains(natural_field), "{message}");
        assert!(message.contains(storage_field), "{message}");

        let conn = connect_file(&db.path).expect("connect rolled-back target");
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM boards WHERE slug='default'",
                [],
                |row| { row.get::<_, i64>(0) }
            )
            .expect("count retained default board"),
            1,
            "failed replace must retain the original board"
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .expect("count retained tasks"),
            0,
            "failed replace must roll back imported tasks"
        );
    }
}

#[test]
fn importer_rejects_dependency_cycles_and_rolls_back_replace() {
    let db = TempDb::new("portable-core-cycle-");
    let before: i64 = connect_file(&db.path)
        .expect("connect target")
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .expect("count boards");
    let mut records = canonical_input_records();
    records.push(json!({
        "type": "dependency",
        "data": {
            "board_id": "b_core",
            "parent_task_id": "t_child",
            "child_task_id": "t_core",
            "created_at": 21
        }
    }));
    let input = db._dir.path().join("cycle.jsonl");
    write_jsonl(&input, &records);
    let error = import_jsonl(&db.path, &input, true).expect_err("cycle must be rejected");
    assert!(
        error
            .to_string()
            .contains("imported data failed doctor checks"),
        "{error}"
    );
    let after: i64 = connect_file(&db.path)
        .expect("connect target")
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .expect("count boards");
    assert_eq!(after, before, "failed replace import must roll back");
}

#[test]
fn importer_rejects_cross_board_history_and_rolls_back_replace() {
    let db = TempDb::new("portable-core-board-scope-");
    let before: i64 = connect_file(&db.path)
        .expect("connect target")
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .expect("count boards");
    let mut records = canonical_input_records();
    let attachment = records
        .iter_mut()
        .find(|row| row["type"] == "attachment")
        .expect("attachment row");
    attachment["data"]["board_id"] = json!("b_other");
    let input = db._dir.path().join("cross-board.jsonl");
    write_jsonl(&input, &records);
    import_jsonl(&db.path, &input, true).expect_err("cross-board history must be rejected");
    let after: i64 = connect_file(&db.path)
        .expect("connect target")
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .expect("count boards");
    assert_eq!(after, before, "failed replace import must roll back");
}

#[test]
fn nullable_fields_must_be_present_even_when_their_value_is_null() {
    let mut task = fixture("task-input.v1.valid.json");
    task["data"]
        .as_object_mut()
        .expect("task data")
        .remove("description");
    assert!(serde_json::from_value::<TaskJsonlInput>(task).is_err());
}

#[test]
fn real_importer_rejects_top_level_unknown_fields_for_every_portable_discriminator() {
    for descriptor in portable_contract_catalog() {
        let db = TempDb::new("portable-top-level-unknown-");
        let input = db._dir.path().join("unknown-top.jsonl");
        write_jsonl(
            &input,
            &[json!({
                "type": descriptor.discriminator,
                "data": {},
                "unexpected": true
            })],
        );
        let error = import_jsonl(&db.path, &input, true)
            .expect_err("top-level unknown field must fail before lane dispatch");
        assert!(
            error.to_string().contains("top-level JSONL contract"),
            "{}: {error}",
            descriptor.discriminator
        );
        let boards: i64 = connect_file(&db.path)
            .expect("connect target")
            .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
            .expect("count boards");
        assert_eq!(boards, 1, "failed replace must roll back");
    }
}

#[test]
fn real_importer_rejects_unknown_and_mismatched_singleton_types() {
    for record in [
        json!({"type": "not_a_record", "data": {}}),
        json!({"type": "column", "data": fixture("board-input.v1.valid.json")["data"]}),
    ] {
        let db = TempDb::new("portable-singleton-mismatch-");
        let input = db._dir.path().join("mismatch.jsonl");
        write_jsonl(&input, &[record]);
        import_jsonl(&db.path, &input, true).expect_err("singleton type mismatch must fail");
        let boards: i64 = connect_file(&db.path)
            .expect("connect target")
            .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
            .expect("count boards");
        assert_eq!(boards, 1, "failed replace must roll back");
    }
}

fn assert_task_priority_fails(priority: Option<i64>) {
    let db = TempDb::new("portable-priority-range-");
    let mut records = canonical_input_records();
    let task = records
        .iter_mut()
        .find(|row| row["type"] == "task")
        .expect("task row");
    match priority {
        Some(priority) => task["data"]["priority"] = json!(priority),
        None => {
            task["data"]
                .as_object_mut()
                .expect("task data")
                .remove("priority");
        }
    }
    let input = db._dir.path().join("priority.jsonl");
    write_jsonl(&input, &records);
    let error =
        import_jsonl(&db.path, &input, true).expect_err("non-canonical priority must fail closed");
    assert!(error.to_string().contains("priority"), "{error}");
    assert_eq!(
        connect_file(&db.path)
            .expect("connect target")
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get::<_, i64>(0))
            .expect("count tasks"),
        0
    );
}

#[test]
fn task_priority_below_p0_fails_closed_without_clamping() {
    assert_task_priority_fails(Some(-1));
}

#[test]
fn task_priority_above_p3_fails_closed_without_clamping() {
    assert_task_priority_fails(Some(4));
}

#[test]
fn missing_task_priority_fails_closed_without_defaulting() {
    assert_task_priority_fails(None);
}

fn assert_comment_shape_fails(mutate: impl FnOnce(&mut serde_json::Map<String, Value>)) {
    let db = TempDb::new("portable-comment-shape-");
    let mut records = canonical_input_records();
    let comment = records
        .iter_mut()
        .find(|row| row["type"] == "comment")
        .expect("comment row");
    mutate(comment["data"].as_object_mut().expect("comment data"));
    let input = db._dir.path().join("comment.jsonl");
    write_jsonl(&input, &records);
    let error =
        import_jsonl(&db.path, &input, true).expect_err("legacy comment shape must fail closed");
    assert!(
        error
            .to_string()
            .contains("comment import row violates its contract"),
        "{error}"
    );
    assert_eq!(
        connect_file(&db.path)
            .expect("connect target")
            .query_row("SELECT COUNT(*) FROM task_comments", [], |row| row
                .get::<_, i64>(0))
            .expect("count comments"),
        0
    );
}

#[test]
fn legacy_comment_author_type_fails_closed_without_normalization() {
    assert_comment_shape_fails(|data| {
        data.insert("author_type".into(), json!("system"));
    });
}

#[test]
fn missing_comment_author_type_fails_closed_without_inference() {
    assert_comment_shape_fails(|data| {
        data.remove("author_type");
    });
}

#[test]
fn legacy_comment_kind_fails_closed_without_normalization() {
    assert_comment_shape_fails(|data| {
        data.insert("kind".into(), json!("worker"));
    });
}

#[test]
fn missing_comment_kind_fails_closed_without_defaulting() {
    assert_comment_shape_fails(|data| {
        data.remove("kind");
    });
}

#[test]
fn mixed_legacy_comment_metadata_json_key_fails_closed() {
    let db = TempDb::new("portable-comment-mixed-format-");
    let mut records = canonical_input_records();
    let comment = records
        .iter_mut()
        .find(|row| row["type"] == "comment")
        .expect("comment row");
    let data = comment["data"].as_object_mut().expect("comment data");
    data.remove("metadata");
    data.insert("metadata_json".into(), json!(r#"{"source":"legacy"}"#));
    let input = db._dir.path().join("mixed.jsonl");
    write_jsonl(&input, &records);
    let error = import_jsonl(&db.path, &input, true).expect_err("mixed format must fail closed");
    assert!(
        error
            .to_string()
            .contains("cannot mix natural and parent storage-native records"),
        "{error}"
    );
    assert_eq!(
        connect_file(&db.path)
            .expect("connect target")
            .query_row("SELECT COUNT(*) FROM task_comments", [], |row| row
                .get::<_, i64>(0))
            .expect("count comments"),
        0
    );
}

#[test]
fn missing_comment_metadata_fails_closed_without_defaulting() {
    assert_comment_shape_fails(|data| {
        data.remove("metadata");
    });
}

#[test]
fn exporter_sanitizes_running_claim_and_internal_log_path() {
    let db = TempDb::new("portable-core-running-");
    let task = create_task(
        &db.path,
        "default",
        "tester",
        CreateTask {
            title: "running portable task".into(),
            description: Some("specified".into()),
            status: Some(TaskStatus::Ready),
            assignee: None,
            priority: 3,
            scheduled_at: None,
            due_at: None,
            max_retries: None,
            metadata_json: "[]".into(),
        },
    )
    .expect("create task");
    mark_execution_plan_not_required(&db.path, "default", "tester", &task.id, "test")
        .expect("mark plan");
    let claim = claim_task(&db.path, "default", "tester", &task.id, 60_000).expect("claim task");
    connect_file(&db.path)
        .expect("connect source")
        .execute(
            "UPDATE task_runs SET log_path='/private/worker.log', metadata_json='false' WHERE id=?1",
            [&claim.run_id],
        )
        .expect("seed private run fields");

    let mut out = Vec::new();
    export_jsonl_to_writer(&db.path, "default", &mut out).expect("export running task");
    let rows = String::from_utf8(out)
        .expect("UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("JSON line"))
        .collect::<Vec<_>>();
    let task_row = rows
        .iter()
        .find(|row| row["type"] == "task" && row["data"]["id"] == task.id)
        .expect("task export");
    let run_row = rows
        .iter()
        .find(|row| row["type"] == "run" && row["data"]["id"] == claim.run_id)
        .expect("run export");
    assert_eq!(task_row["data"]["status"], "ready");
    for field in [
        "claim_token",
        "claim_owner",
        "claim_expires_at",
        "last_heartbeat_at",
        "current_run_id",
        "started_at",
    ] {
        assert!(task_row["data"][field].is_null(), "{field}");
    }
    assert_eq!(run_row["data"]["status"], "canceled");
    assert!(run_row["data"]["log_path"].is_null());
    assert_eq!(run_row["data"]["metadata"], false);
}
