//! portable 格式的完整往返契约；每次测试运行只构建一次富样本。
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use kanban_protocol::portable_contract_catalog;
use serde_json::{Map, Value};

use crate::AppState;

// 记录类型不是独立的集成场景：同一个跨表样本必须完整往返。
// 不使用进程内缓存，也不共享可写数据库；nextest 与 libtest 执行同一条路径。
#[tokio::test]
async fn portable_roundtrip_preserves_all_catalog_records() {
    run_portable_flow()
        .await
        .expect("portable 全目录导出、导入及替换往返");
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
    assert_fixture_records(&records)?;
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
    let catalog = portable_contract_catalog();
    let actual = catalog
        .iter()
        .map(|descriptor| descriptor.discriminator)
        .collect::<BTreeSet<_>>();
    let expected = FIXTURE_TABLES
        .iter()
        .map(|(discriminator, _)| *discriminator)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected, "portable 目录与往返样本库存必须一致");
    assert_eq!(catalog.len(), actual.len(), "portable 目录不能有重复类型");
    assert_eq!(
        FIXTURE_TABLES.len(),
        expected.len(),
        "portable fixture 映射不能有重复类型"
    );

    for descriptor in catalog {
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
        let input_identity =
            find_fixture_record(descriptor.discriminator, actual_records, &input_data);
        assert_fixture_identity(descriptor.discriminator, input_identity, &input_data);
    }
    Ok(())
}

// 单一映射表同时约束目录库存；不允许删掉 descriptor 后让循环静默少测。
const FIXTURE_TABLES: &[(&str, &str)] = &[
    ("board", "boards"),
    ("column", "board_columns"),
    ("task", "tasks"),
    ("dependency", "task_dependencies"),
    ("run", "task_runs"),
    ("comment", "task_comments"),
    ("signal_observation", "signal_observations"),
    ("signal", "signals"),
    ("event", "task_events"),
    ("attachment", "file_objects"),
    ("label", "labels"),
    ("label_semantics", "label_semantics"),
    ("label_atom", "label_atoms"),
    ("label_semantic_proposal", "label_semantic_proposals"),
    ("label_ontology_observation", "label_ontology_observations"),
    ("label_ontology_signal", "label_ontology_signals"),
    ("label_ontology_action", "label_ontology_actions"),
    (
        "label_ontology_action_atom_effect",
        "label_ontology_action_atom_effects",
    ),
    (
        "label_ontology_action_signal",
        "label_ontology_action_signals",
    ),
    ("task_label", "task_labels"),
    ("setting", "app_settings"),
];

fn fixture_table(discriminator: &str) -> &'static str {
    FIXTURE_TABLES
        .iter()
        .find_map(|(key, table)| (*key == discriminator).then_some(*table))
        .unwrap_or_else(|| panic!("unknown portable fixture discriminator {discriminator}"))
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
        .or_else(|| {
            (discriminator == "attachment")
                .then(|| data.get("object_id"))
                .flatten()
        })
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
                    "attachment" => "object_id",
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
