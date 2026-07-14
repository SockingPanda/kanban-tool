use crate::common::*;
use quote::ToTokens;
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> Value {
    serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../schemas/fixtures/api")
                .join(name),
        )
        .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn doctor_response_maps_real_non_default_report_before_fixture_normalization()
-> anyhow::Result<()> {
    let test = TestApp::new()?;
    let conn = kanban_sqlite::db::connect_file(test.db_path())?;
    conn.execute(
        "UPDATE derived_store_state
         SET schema_version=71,last_event_id=73,dirty=1,last_error='sentinel-error'
         WHERE store_name='oxigraph_relations'",
        [],
    )?;
    conn.execute_batch(
        "PRAGMA foreign_keys=OFF;
         INSERT INTO task_dependencies(board_id,parent_task_id,child_task_id,created_at)
         VALUES('b_missing','t_missing_parent','t_missing_child',79);",
    )?;
    drop(conn);

    let expected = kanban_sqlite::api::doctor_database(test.db_path())?;
    let expected_json = serde_json::to_value(&expected)?;
    let expected_store = expected
        .derived_stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("oxigraph sentinel store");
    assert_eq!(expected_store.schema_version, 71);
    assert_eq!(expected_store.last_event_id, 73);
    assert!(expected_store.dirty);
    assert_eq!(expected_store.last_error.as_deref(), Some("sentinel-error"));
    assert!(
        !expected.consistency_issues.is_empty(),
        "foreign-key sentinel must produce a mapped doctor issue"
    );

    let (status, body) = request_json(
        test.router(),
        "POST",
        "/api/v1/maintenance/doctor",
        None,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let parsed: kanban_contract::DoctorResponse = serde_json::from_value(body.clone())?;
    assert_eq!(body, json!({ "data": expected_json }));

    let actual_store = parsed
        .data
        .derived_stores
        .iter()
        .find(|store| store.store_name == "oxigraph_relations")
        .expect("mapped oxigraph sentinel store");
    assert_eq!(actual_store.schema_version, 71);
    assert_eq!(actual_store.last_event_id, 73);
    assert!(actual_store.dirty);
    assert_eq!(actual_store.last_error.as_deref(), Some("sentinel-error"));
    assert_eq!(
        serde_json::to_value(&parsed.data.consistency_issues)?,
        serde_json::to_value(&expected.consistency_issues)?
    );
    Ok(())
}

#[test]
fn doctor_response_contract_consumes_producer_fixture() {
    let fixture = fixture("doctor-response.v1.valid.json");
    let parsed: kanban_contract::DoctorResponse = serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), fixture);
}

#[tokio::test]
async fn checkpoint_response_reports_real_wal_field_relationships() -> anyhow::Result<()> {
    let test = TestApp::new()?;
    let conn = kanban_sqlite::db::connect_file(test.db_path())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE maintenance_wal_sentinel(value INTEGER NOT NULL);
         INSERT INTO maintenance_wal_sentinel(value) VALUES(83),(89),(97);",
    )?;

    let (status, body) = request_json(
        test.router(),
        "POST",
        "/api/v1/maintenance/checkpoint",
        None,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let parsed: kanban_contract::CheckpointResponse = serde_json::from_value(body)?;
    assert_eq!(parsed.data.busy, 0);
    assert!(parsed.data.log_frames >= 0);
    assert!(parsed.data.checkpointed_frames >= 0);
    assert!(parsed.data.checkpointed_frames <= parsed.data.log_frames);
    drop(conn);
    Ok(())
}

#[test]
fn checkpoint_response_contract_consumes_producer_fixture() {
    let fixture = fixture("checkpoint-response.v1.valid.json");
    let parsed: kanban_contract::CheckpointResponse =
        serde_json::from_value(fixture.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), fixture);
}

fn function<'a>(file: &'a syn::File, name: &str) -> anyhow::Result<&'a syn::ItemFn> {
    file.items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .with_context(|| format!("missing {name} function"))
}

fn canonical_block(source: &str) -> String {
    syn::parse_str::<syn::Block>(source)
        .unwrap()
        .into_token_stream()
        .to_string()
}

fn require_direct_fields(body: &str, fields: &[&str]) -> anyhow::Result<()> {
    for field in fields {
        let expected = format!("{field} : value . {field}");
        anyhow::ensure!(
            body.matches(&expected).count() == 1,
            "mapper must directly map {expected} exactly once"
        );
    }
    Ok(())
}

fn validate_maintenance_adapter(source: &str) -> anyhow::Result<()> {
    let file = syn::parse_file(source)?;
    let normalized = file.to_token_stream().to_string();
    anyhow::ensure!(!normalized.contains("kanban_sqlite :: service"));
    anyhow::ensure!(!normalized.contains("rusqlite"));

    for (handler, response, expected_body) in [
        (
            "doctor",
            "DoctorResponse",
            "{ Ok(Json(DoctorResponse::new(crate::doctor_report_from_record(kanban_sqlite::api::doctor_database(state.db_path())?,)))) }",
        ),
        (
            "checkpoint",
            "CheckpointResponse",
            "{ Ok(Json(CheckpointResponse::new(checkpoint_report(kanban_sqlite::api::checkpoint_database(state.db_path())?,)))) }",
        ),
    ] {
        let function = function(&file, handler)?;
        anyhow::ensure!(
            function.sig.output.to_token_stream().to_string()
                == format!("-> Result < Json < {response} > , ApiError >")
        );
        anyhow::ensure!(
            function.block.to_token_stream().to_string() == canonical_block(expected_body),
            "{handler} must be the unique state.db_path facade -> mapper -> contract Json tail"
        );
    }

    for facade in ["doctor_database", "checkpoint_database"] {
        anyhow::ensure!(
            normalized
                .matches(&format!("kanban_sqlite :: api :: {facade}"))
                .count()
                == 1,
            "{facade} must have exactly one reachable call"
        );
    }

    let issue = function(&file, "doctor_issue_from_record")?
        .block
        .to_token_stream()
        .to_string();
    require_direct_fields(&issue, &["severity", "code", "message", "record_ids"])?;

    let store = function(&file, "doctor_store_from_record")?
        .block
        .to_token_stream()
        .to_string();
    require_direct_fields(
        &store,
        &[
            "store_name",
            "schema_version",
            "last_event_id",
            "dirty",
            "last_error",
            "pending_outbox",
            "running_outbox",
            "failed_outbox",
        ],
    )?;

    let checkpoint = function(&file, "checkpoint_report")?
        .block
        .to_token_stream()
        .to_string();
    require_direct_fields(&checkpoint, &["busy", "log_frames", "checkpointed_frames"])?;

    let report = function(&file, "doctor_report_from_record")?
        .block
        .to_token_stream()
        .to_string();
    require_direct_fields(
        &report,
        &[
            "ok",
            "integrity_check",
            "migration_version",
            "user_version",
            "expired_running_tasks",
            "running_tasks_without_active_run",
            "orphan_running_runs",
            "dependency_cycles",
            "archived_dependency_edges",
            "missing_run_logs",
            "suspicious_run_log_paths",
            "executable_dependency_violations",
            "executable_spec_violations",
            "executable_schedule_violations",
            "unplanned_active_tasks",
            "active_parents_with_incomplete_required_steps",
            "outbox_pending",
            "outbox_running",
            "outbox_failed",
            "derived_dirty_stores",
            "derived_error_stores",
            "consistency_errors",
            "consistency_warnings",
            "ontology_ledger_errors",
            "ontology_ledger_warnings",
        ],
    )?;
    for expected in [
        "derived_stores : value . derived_stores . into_iter () . map (doctor_store_from_record) . collect ()",
        "consistency_issues : value . consistency_issues . into_iter () . map (doctor_issue_from_record) . collect ()",
        "ontology_ledger_issues : value . ontology_ledger_issues . into_iter () . map (doctor_issue_from_record) . collect ()",
    ] {
        anyhow::ensure!(report.matches(expected).count() == 1);
    }
    Ok(())
}

#[test]
fn maintenance_adapter_operation_gate_rejects_alias_shadow_dead_constant_swap_and_omit() {
    let handler_source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/handlers/maintenance.rs"),
    )
    .unwrap();
    let dto_source =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/dto.rs")).unwrap();
    let normalized_handler = syn::parse_file(&handler_source)
        .unwrap()
        .into_token_stream()
        .to_string();
    assert!(!normalized_handler.contains("kanban_sqlite :: service"));
    assert!(!normalized_handler.contains("rusqlite"));
    let source = format!("{handler_source}\n{dto_source}");
    validate_maintenance_adapter(&source).unwrap();

    let hostiles = [
        source.replacen(
            "kanban_sqlite::api::doctor_database(state.db_path())?",
            "{ let alias = state.db_path(); kanban_sqlite::api::doctor_database(alias)? }",
            1,
        ),
        source.replacen(
            "kanban_sqlite::api::doctor_database(state.db_path())?",
            "{ let state = state; kanban_sqlite::api::doctor_database(state.db_path())? }",
            1,
        ),
        source.replacen(
            "pub(crate) async fn doctor(",
            r##"fn dead() { let _ = kanban_sqlite::api::doctor_database(r#"dead"#); }\n\npub(crate) async fn doctor("##,
            1,
        ),
        source.replacen(
            "Ok(Json(CheckpointResponse::new(checkpoint_report(",
            "return Ok(Json(CheckpointResponse::new(checkpoint_report(",
            1,
        ),
        source.replacen(
            "busy: value.busy,",
            "busy: 0,",
            1,
        ),
        source.replacen(
            "log_frames: value.log_frames,\n        checkpointed_frames: value.checkpointed_frames,",
            "log_frames: value.checkpointed_frames,\n        checkpointed_frames: value.log_frames,",
            1,
        ),
        source.replacen(
            "record_ids: value.record_ids,",
            "",
            1,
        ),
        source.replacen(
            "Result<Json<DoctorResponse>",
            "Result<Json<DataEnvelope<kanban_sqlite::api::DoctorReport>>",
            1,
        ),
    ];
    for hostile in hostiles {
        assert!(validate_maintenance_adapter(&hostile).is_err());
    }
}

#[tokio::test]
async fn maintenance_routes_return_exact_zh_missing_database_error_without_creating_file()
-> anyhow::Result<()> {
    for endpoint in [
        "/api/v1/maintenance/doctor",
        "/api/v1/maintenance/checkpoint",
    ] {
        let test = TestApp::new()?;
        let db_path = test.db_path().to_path_buf();
        let app = test.router();
        fs::remove_file(&db_path)?;
        let (status, body) =
            request_json_with_accept_language(app, "POST", endpoint, None, "zh-CN").await?;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{endpoint}");
        assert_eq!(
            body,
            json!({
                "error": {
                    "code": "invalid_input",
                    "message": format!("输入无效：数据库文件不存在：{}", db_path.display())
                }
            }),
            "{endpoint}"
        );
        assert!(
            !db_path.exists(),
            "{endpoint} must not recreate the database"
        );
    }
    Ok(())
}
