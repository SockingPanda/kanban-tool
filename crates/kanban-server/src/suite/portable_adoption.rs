use std::{collections::BTreeMap, fs, path::Path, sync::OnceLock};

use kanban_protocol::portable_contract_catalog;
use serde_json::{Map, Value};
use tokio::sync::OnceCell;

use crate::AppState;

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
    kanban_service::adoption_test_support::populate_portable_source(&source_path).await?;
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
    let records =
        kanban_service::adoption_test_support::validate_portable_export(&source_path, &export_path)
            .await?;
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
    kanban_service::adoption_test_support::assert_portable_facts_equal(
        &export_path,
        &imported_export_path,
    )?;

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
    kanban_service::adoption_test_support::assert_portable_facts_equal(
        &export_path,
        &replaced_export_path,
    )?;
    Ok(())
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
