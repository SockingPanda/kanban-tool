use std::{collections::BTreeMap, fs, path::Path, sync::OnceLock};

use kanban_protocol::portable_contract_catalog;
use rusqlite::Connection as SqliteConnection;
use serde_json::{Map, Value};
use tokio::sync::OnceCell;

use crate::AppState;

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

#[derive(Clone)]
struct PortableEvidence {
    records: BTreeMap<String, Vec<Value>>,
}

static PORTABLE_EVIDENCE: OnceCell<PortableEvidence> = OnceCell::const_new();
static EXPORTED_RECORDS: OnceLock<BTreeMap<String, Vec<Value>>> = OnceLock::new();

async fn evidence() -> &'static PortableEvidence {
    PORTABLE_EVIDENCE
        .get_or_init(|| async {
            run_portable_flow()
                .await
                .expect("portable JSONL adoption flow");
            PortableEvidence {
                records: EXPORTED_RECORDS
                    .get()
                    .cloned()
                    .expect("portable export evidence records"),
            }
        })
        .await
}

async fn run_portable_flow() -> Result<(), String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    let source_path = directory.path().join("portable-source.db");
    let source = AppState::open(&source_path, "portable-adoption")
        .await
        .map_err(|error| error.to_string())?;
    drop(source);
    populate_source(&source_path).await?;
    let source = AppState::open(&source_path, "portable-adoption")
        .await
        .map_err(|error| error.to_string())?;
    let export_path = directory.path().join("portable.jsonl");
    let export = source
        .application()
        .export(export_path.to_str().ok_or("export path is not UTF-8")?)
        .await
        .map_err(|error| error.to_string())?;
    assert!(
        export.record_count > 20,
        "rich portable fixture must have records"
    );
    let records = validate_export(&source_path, &export_path)?;
    EXPORTED_RECORDS
        .set(records)
        .map_err(|_| "portable export evidence was initialized twice".to_owned())?;
    assert_fixture_records(EXPORTED_RECORDS.get().expect("exported records"))?;
    drop(source);

    let import_path = directory.path().join("portable-import.db");
    let target = AppState::open(&import_path, "portable-adoption")
        .await
        .map_err(|error| error.to_string())?;
    let imported = target
        .application()
        .import(
            export_path.to_str().ok_or("portable path is not UTF-8")?,
            false,
        )
        .await
        .map_err(|error| error.to_string())?;
    assert_eq!(imported.phase, "completed");
    assert!(!imported.restart_required);
    let imported_export_path = directory.path().join("portable-import.jsonl");
    target
        .application()
        .export(
            imported_export_path
                .to_str()
                .ok_or("import export path is not UTF-8")?,
        )
        .await
        .map_err(|error| error.to_string())?;
    drop(target);
    assert_database_facts_equal(&export_path, &imported_export_path)?;

    let replace_path = directory.path().join("portable-replace.db");
    let replace_target = AppState::open(&replace_path, "portable-adoption")
        .await
        .map_err(|error| error.to_string())?;
    let replaced = replace_target
        .application()
        .import(
            export_path.to_str().ok_or("portable path is not UTF-8")?,
            true,
        )
        .await
        .map_err(|error| error.to_string())?;
    assert_eq!(replaced.phase, "completed");
    assert!(!replaced.restart_required);
    let replaced_export_path = directory.path().join("portable-replace.jsonl");
    replace_target
        .application()
        .export(
            replaced_export_path
                .to_str()
                .ok_or("replace export path is not UTF-8")?,
        )
        .await
        .map_err(|error| error.to_string())?;
    drop(replace_target);
    assert_database_facts_equal(&export_path, &replaced_export_path)?;
    Ok(())
}

fn validate_export(
    source_path: &Path,
    export_path: &Path,
) -> Result<BTreeMap<String, Vec<Value>>, String> {
    let connection = open_sqlite(source_path)?;
    let bytes = fs::read(export_path).map_err(|error| error.to_string())?;
    let mut lines = bytes.split(|byte| *byte == b'\n');
    let header: Value = serde_json::from_slice(
        lines
            .next()
            .ok_or_else(|| "portable export has no header".to_owned())?,
    )
    .map_err(|error| error.to_string())?;
    let canonical = header
        .get("canonical_tables")
        .and_then(Value::as_array)
        .ok_or_else(|| "portable header lacks canonical_tables".to_owned())?;
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
            .ok_or_else(|| format!("portable record lacks table: {record}"))?;
        let data = record
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("portable {table} record lacks data"))?;
        assert!(PORTABLE_TABLES.contains(&table));
        let columns = table_columns(&connection, table)?;
        let mut actual = data.keys().cloned().collect::<Vec<_>>();
        actual.sort();
        let mut expected = columns;
        expected.sort();
        assert_eq!(
            actual, expected,
            "portable {table} columns must be complete"
        );
        *counts.entry(table.to_owned()).or_default() += 1;
        records_by_table
            .entry(table.to_owned())
            .or_default()
            .push(Value::Object(data.clone().into_iter().collect()));
    }

    let header_counts = header
        .get("table_counts")
        .and_then(Value::as_object)
        .ok_or_else(|| "portable header lacks table_counts".to_owned())?;
    for table in PORTABLE_TABLES {
        let expected = table_count(&connection, table)? as usize;
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

fn assert_fixture_records(records: &BTreeMap<String, Vec<Value>>) -> Result<(), String> {
    for descriptor in portable_contract_catalog() {
        let input = fixture(descriptor.discriminator, true)?;
        let output = fixture(descriptor.discriminator, false)?;
        let input_data = descriptor
            .decode_input_envelope(input)
            .map_err(|error| error.to_string())?;
        let output_data = output
            .get("data")
            .and_then(Value::as_object)
            .ok_or_else(|| format!("{} output fixture lacks data", descriptor.discriminator))?;
        let table = fixture_table(descriptor.discriminator);
        let actual_records = records
            .get(table)
            .ok_or_else(|| format!("portable export lacks {table} record"))?;
        let actual = find_fixture_record(descriptor.discriminator, actual_records, output_data);
        assert_fixture_identity(descriptor.discriminator, actual, output_data);
        assert_fixture_identity(descriptor.discriminator, actual, &input_data);
    }
    Ok(())
}

fn fixture_table(discriminator: &str) -> &'static str {
    match discriminator {
        "board" => "boards",
        "column" => "board_columns",
        "task" => "tasks",
        "dependency" => "task_dependencies",
        "run" => "task_runs",
        "comment" => "task_comments",
        "signal_observation" => "signal_observations",
        "signal" => "signals",
        "event" => "task_events",
        "attachment" => "task_attachments",
        "label" => "labels",
        "label_semantics" => "label_semantics",
        "label_atom" => "label_atoms",
        "label_semantic_proposal" => "label_semantic_proposals",
        "label_ontology_observation" => "label_ontology_observations",
        "label_ontology_signal" => "label_ontology_signals",
        "label_ontology_action" => "label_ontology_actions",
        "label_ontology_action_atom_effect" => "label_ontology_action_atom_effects",
        "label_ontology_action_signal" => "label_ontology_action_signals",
        "task_label" => "task_labels",
        "setting" => "app_settings",
        _ => panic!("unknown portable fixture discriminator {discriminator}"),
    }
}

fn fixture_identity<'a>(discriminator: &str, data: &'a Map<String, Value>) -> &'a Value {
    let key = match discriminator {
        "dependency" => "parent_task_id",
        "label_semantics" => "label_id",
        "label_ontology_action_atom_effect" | "label_ontology_action_signal" => "action_id",
        "task_label" => "task_id",
        "setting" => "key",
        _ => "id",
    };
    data.get(key)
        .unwrap_or_else(|| panic!("{discriminator} fixture lacks identity field {key}"))
}

fn assert_fixture_identity(
    discriminator: &str,
    actual: &Value,
    expected_data: &Map<String, Value>,
) {
    let actual_data = actual
        .as_object()
        .unwrap_or_else(|| panic!("{discriminator} exported record is not an object"));
    let expected = fixture_identity(discriminator, expected_data);
    let actual = fixture_identity(discriminator, actual_data);
    assert_eq!(
        actual, expected,
        "{discriminator} fixture identity must survive portable export/import"
    );
}

fn find_fixture_record<'a>(
    discriminator: &str,
    records: &'a [Value],
    expected_data: &Map<String, Value>,
) -> &'a Value {
    let expected = fixture_identity(discriminator, expected_data);
    records
        .iter()
        .find(|record| {
            record.as_object().and_then(|data| {
                data.get(match discriminator {
                    "dependency" => "parent_task_id",
                    "label_semantics" => "label_id",
                    "label_ontology_action_atom_effect" | "label_ontology_action_signal" => {
                        "action_id"
                    }
                    "task_label" => "task_id",
                    "setting" => "key",
                    _ => "id",
                })
            }) == Some(expected)
        })
        .unwrap_or_else(|| panic!("{discriminator} fixture identity was not exported"))
}

fn fixture(discriminator: &str, input: bool) -> Result<Value, String> {
    let kind = if input { "input" } else { "output" };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/fixtures/jsonl")
        .join(format!("{discriminator}-{kind}.v1.valid.json"));
    let raw = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&raw).map_err(|error| error.to_string())
}

async fn populate_source(path: &Path) -> Result<(), String> {
    let connection = open_sqlite(path)?;
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
        .map_err(|error| error.to_string())?;
    connection
        .execute("DELETE FROM projection_jobs", [])
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn open_sqlite(path: &Path) -> Result<SqliteConnection, String> {
    let path = path.to_str().ok_or("portable path is not UTF-8")?;
    let connection = SqliteConnection::open(path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "PRAGMA writable_schema = ON; DELETE FROM sqlite_master WHERE name LIKE '__turso_internal_fts%'; PRAGMA writable_schema = OFF;",
        )
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

fn table_columns(connection: &SqliteConnection, table: &str) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare(&format!(
            "PRAGMA table_info(\"{}\")",
            table.replace('"', "\"\"")
        ))
        .map_err(|error| error.to_string())?;
    let mut columns = Vec::new();
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;
    for row in rows {
        columns.push(row.map_err(|error| error.to_string())?);
    }
    Ok(columns)
}

fn table_count(connection: &SqliteConnection, table: &str) -> Result<i64, String> {
    connection
        .query_row(
            &format!("SELECT COUNT(*) FROM \"{}\"", table.replace('"', "\"\"")),
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

fn assert_database_facts_equal(source: &Path, target: &Path) -> Result<(), String> {
    let source_records = read_export(source)?;
    let target_records = read_export(target)?;
    assert_eq!(
        source_records, target_records,
        "portable canonical records changed"
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
            .ok_or_else(|| format!("portable record lacks table: {record}"))?;
        let data = record
            .get("data")
            .cloned()
            .ok_or_else(|| format!("portable {table} record lacks data"))?;
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
        .ok_or_else(|| format!("portable {table} record {key}={expected} missing"))
}

macro_rules! portable_witness {
    ($producer:ident, $consumer:ident, $discriminator:literal) => {
        #[tokio::test]
        async fn $producer() {
            let evidence = evidence().await;
            assert_fixture_fact(evidence, $discriminator, false);
        }

        #[tokio::test]
        async fn $consumer() {
            let evidence = evidence().await;
            assert_fixture_fact(evidence, $discriminator, true);
        }
    };
}

fn assert_fixture_fact(evidence: &PortableEvidence, discriminator: &str, input: bool) {
    let descriptor = portable_contract_catalog()
        .iter()
        .find(|descriptor| descriptor.discriminator == discriminator)
        .expect("portable descriptor");
    let fixture = fixture(discriminator, input).expect("portable fixture");
    if input {
        let input_data = descriptor
            .decode_input_envelope(fixture.clone())
            .expect("input fixture is consumed by portable contract");
        let records = &evidence.records[fixture_table(discriminator)];
        let actual = find_fixture_record(discriminator, records, &input_data);
        assert_fixture_identity(discriminator, actual, &input_data);
        return;
    }
    let output_data = fixture
        .get("data")
        .and_then(Value::as_object)
        .expect("output fixture data");
    let records = &evidence.records[fixture_table(discriminator)];
    let actual = find_fixture_record(discriminator, records, output_data);
    assert_fixture_identity(discriminator, actual, output_data);
}

portable_witness!(
    board_output_fixture_is_produced_by_real_export,
    board_input_fixture_is_consumed_by_real_import,
    "board"
);
portable_witness!(
    column_output_fixture_is_produced_by_real_export,
    column_input_fixture_is_consumed_by_real_import,
    "column"
);
portable_witness!(
    task_output_fixture_is_produced_by_real_export,
    task_input_fixture_is_consumed_by_real_import,
    "task"
);
portable_witness!(
    dependency_output_fixture_is_produced_by_real_export,
    dependency_input_fixture_is_consumed_by_real_import,
    "dependency"
);
portable_witness!(
    run_output_fixture_is_produced_by_real_export,
    run_input_fixture_is_consumed_by_real_import,
    "run"
);
portable_witness!(
    comment_output_fixture_is_produced_by_real_export,
    comment_input_fixture_is_consumed_by_real_import,
    "comment"
);
portable_witness!(
    signal_observation_output_fixture_is_produced_by_real_export,
    signal_observation_input_fixture_is_consumed_by_real_import,
    "signal_observation"
);
portable_witness!(
    signal_output_fixture_is_produced_by_real_export,
    signal_input_fixture_is_consumed_by_real_import,
    "signal"
);
portable_witness!(
    event_output_fixture_is_produced_by_real_export,
    event_input_fixture_is_consumed_by_real_import,
    "event"
);
portable_witness!(
    attachment_output_fixture_is_produced_by_real_export,
    attachment_input_fixture_is_consumed_by_real_import,
    "attachment"
);
portable_witness!(
    label_output_fixture_is_produced_by_real_export,
    label_input_fixture_is_consumed_by_real_import,
    "label"
);
portable_witness!(
    label_semantics_output_fixture_is_produced_by_real_export,
    label_semantics_input_fixture_is_consumed_by_real_import,
    "label_semantics"
);
portable_witness!(
    label_atom_output_fixture_is_produced_by_real_export,
    label_atom_input_fixture_is_consumed_by_real_import,
    "label_atom"
);
portable_witness!(
    label_semantic_proposal_output_fixture_is_produced_by_real_export,
    label_semantic_proposal_input_fixture_is_consumed_by_real_import,
    "label_semantic_proposal"
);
portable_witness!(
    label_ontology_observation_output_fixture_is_produced_by_real_export,
    label_ontology_observation_input_fixture_is_consumed_by_real_import,
    "label_ontology_observation"
);
portable_witness!(
    label_ontology_signal_output_fixture_is_produced_by_real_export,
    label_ontology_signal_input_fixture_is_consumed_by_real_import,
    "label_ontology_signal"
);
portable_witness!(
    label_ontology_action_output_fixture_is_produced_by_real_export,
    label_ontology_action_input_fixture_is_consumed_by_real_import,
    "label_ontology_action"
);
portable_witness!(
    label_ontology_action_atom_effect_output_fixture_is_produced_by_real_export,
    label_ontology_action_atom_effect_input_fixture_is_consumed_by_real_import,
    "label_ontology_action_atom_effect"
);
portable_witness!(
    label_ontology_action_signal_output_fixture_is_produced_by_real_export,
    label_ontology_action_signal_input_fixture_is_consumed_by_real_import,
    "label_ontology_action_signal"
);
portable_witness!(
    task_label_output_fixture_is_produced_by_real_export,
    task_label_input_fixture_is_consumed_by_real_import,
    "task_label"
);
portable_witness!(
    setting_output_fixture_is_produced_by_real_export,
    setting_input_fixture_is_consumed_by_real_import,
    "setting"
);
