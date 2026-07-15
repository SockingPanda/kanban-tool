use std::{fs, io::Write, path::Path};

use kanban_contract::jsonl_ledger::*;
use kanban_sqlite::{
    api::{export_jsonl_to_writer, import_jsonl},
    db::connect_file,
    init::init_database,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn temp_db(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix(name)
        .tempdir()
        .expect("tempdir");
    let path = dir.path().join("kb.db");
    init_database(&path, "contract-test").expect("init database");
    (dir, path)
}

fn seed_ledger(path: &Path) {
    let conn = connect_file(path).expect("connect");
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = OFF;
        UPDATE boards SET id='b_fixture' WHERE slug='default';
        UPDATE board_columns SET board_id='b_fixture';
        PRAGMA foreign_keys = ON;

        INSERT INTO tasks(id,board_id,seq,title,status,priority,position,created_by,created_at,updated_at,retry_count,max_retries,metadata_json,lock_version)
        VALUES ('t_fixture','b_fixture',1,'Fixture task','todo',3,0,'tester',1,2,0,NULL,'{}',0);
        INSERT INTO labels(id,board_id,name,color,created_at,updated_at)
        VALUES ('l_fixture','b_fixture','rust',NULL,1,2);
        INSERT INTO label_semantics(label_id,board_id,description,applies_when,excludes_when,positive_examples,negative_examples,created_at,updated_at)
        VALUES ('l_fixture','b_fixture',NULL,'["systems language"]','[]','["cargo"]','[]',1,2);
        INSERT INTO label_atoms(id,label_id,board_id,polarity,kind,text,ordinal,content_hash,created_at,updated_at)
        VALUES ('la_fixture','l_fixture','b_fixture','positive','positive_example','cargo',0,'atom-hash',1,2);
        INSERT INTO label_semantic_proposals(id,board_id,task_id,status,name,description,applies_when,excludes_when,positive_examples,negative_examples,heuristic_coverage,heuristic_residual_norm,top1_existing_label_id,top1_existing_label_name,diagnostics_json,created_by,decision_reason,resolved_label_id,created_at,updated_at,decided_at,heuristic_coverage_cosine)
        VALUES ('lp_fixture','b_fixture','t_fixture','proposed','rust-new',NULL,'["rust"]','[]','["cargo"]','[]',0.4,0.6,'l_fixture','rust','[{"code":"candidate"}]','tester',NULL,NULL,1,2,NULL,0.3);
        INSERT INTO label_ontology_observations(id,board_id,task_id,task_ref_snapshot,task_snapshot_json,agent_candidates_json,suggestion_snapshot_json,final_decision_json,suggest_coverage,suggest_coverage_cosine,suggest_residual_norm,suggest_needs_new_label,suggest_degraded,diagnostics_json,capture_fingerprint,created_by,created_by_type,agent_type,created_at,suggest_input_hash)
        VALUES ('lor_fixture','b_fixture','t_fixture','default#1','{"title":"fixture"}','[{"label":"rust"}]','{"selected":[]}','{"labels":["rust"]}',0.8,0.7,0.2,0,0,'[]','capture-hash','tester','agent','codex',1,'input-hash');
        INSERT INTO label_ontology_signals(id,observation_id,board_id,kind,status,target_label_id,target_label_name_snapshot,related_labels_json,proposed_action,candidate_atom_polarity,candidate_atom_kind,candidate_text,candidate_content_hash,proposed_label_name,proposed_label_name_normalized,proposal_json,agent_selected,suggest_state,suggest_score,suggest_rank,final_selected,rationale,confidence,signal_key,superseded_by_signal_id,status_reason,created_at,updated_at,reviewed_at,closed_at)
        VALUES ('los_fixture','lor_fixture','b_fixture','boundary_issue','open','l_fixture','rust','["l_fixture"]','observe',NULL,NULL,NULL,NULL,NULL,NULL,'{"source":"fixture"}',1,'selected',0.9,1,1,'fixture rationale',0.8,'fixture-key',NULL,NULL,1,2,NULL,NULL);
        INSERT INTO label_ontology_actions(id,board_id,parent_action_id,action_type,reason,target_label_id,result_label_id,result_atom_id,result_atom_content_hash,result_proposal_id,canonical_before_hash,canonical_after_hash,change_json,validation_status,validation_json,created_by,created_by_type,agent_type,created_at,validation_requirement)
        VALUES ('loa_fixture','b_fixture',NULL,'confirm','fixture reason','l_fixture',NULL,NULL,NULL,NULL,NULL,NULL,'{"status":"confirmed"}','not_required','{}','tester','agent','codex',1,'none');
        INSERT INTO label_ontology_action_atom_effects(board_id,action_id,label_id_snapshot,atom_id_snapshot,atom_content_hash,polarity,kind,text,effect,created_at)
        VALUES ('b_fixture','loa_fixture','l_fixture','la_fixture','atom-hash','positive','positive_example','cargo','added',1);
        INSERT INTO label_ontology_action_signals(board_id,action_id,signal_id,created_at)
        VALUES ('b_fixture','loa_fixture','los_fixture',1);
        INSERT INTO signal_observations(id,board_id,task_id,task_ref_snapshot,run_id,comment_id,actor,agent_type,source,evidence_json,created_at)
        VALUES ('obs_fixture','b_fixture','t_fixture','default#1',NULL,NULL,'tester','codex','contract-test','{"confidence":0.9}',1);
        INSERT INTO signals(id,board_id,observation_id,kind,title,summary,severity,status,dedupe_key,superseded_by_signal_id,reviewed_by,reviewed_at,review_reason,created_at,updated_at)
        VALUES ('sig_fixture','b_fixture','obs_fixture','quality','Fixture signal','Fixture summary','info','open','fixture',NULL,NULL,NULL,NULL,1,2);
        INSERT INTO app_settings(key,value_json,updated_at)
        VALUES ('contract.fixture','{"enabled":true}',2);
        "#,
    )
    .expect("seed canonical ledger");
}

fn export_records(path: &Path) -> Vec<Value> {
    let mut bytes = Vec::new();
    export_jsonl_to_writer(path, "default", &mut bytes).expect("real export");
    String::from_utf8(bytes)
        .expect("utf8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("jsonl"))
        .collect()
}

fn parent_exporter_records(path: &Path) -> Vec<Value> {
    let mut records = export_records(path);
    for record in &mut records {
        let discriminator = record["type"].as_str().expect("record type").to_owned();
        let data = record["data"].as_object_mut().expect("record data");
        let json_fields: &[(&str, &str)] = match discriminator.as_str() {
            "task" => &[("result", "result_json"), ("metadata", "metadata_json")],
            "run" | "comment" => &[("metadata", "metadata_json")],
            "event" => &[("payload", "payload_json")],
            "label_semantics" => &[
                ("applies_when", "applies_when"),
                ("excludes_when", "excludes_when"),
                ("positive_examples", "positive_examples"),
                ("negative_examples", "negative_examples"),
            ],
            "label_semantic_proposal" => &[
                ("applies_when", "applies_when"),
                ("excludes_when", "excludes_when"),
                ("positive_examples", "positive_examples"),
                ("negative_examples", "negative_examples"),
                ("diagnostics", "diagnostics_json"),
            ],
            "label_ontology_observation" => &[
                ("task_snapshot", "task_snapshot_json"),
                ("agent_candidates", "agent_candidates_json"),
                ("suggestion_snapshot", "suggestion_snapshot_json"),
                ("final_decision", "final_decision_json"),
                ("diagnostics", "diagnostics_json"),
            ],
            "label_ontology_signal" => &[
                ("related_labels", "related_labels_json"),
                ("proposal", "proposal_json"),
            ],
            "label_ontology_action" => {
                &[("change", "change_json"), ("validation", "validation_json")]
            }
            "signal_observation" => &[("evidence", "evidence_json")],
            "setting" => &[("value", "value_json")],
            _ => &[],
        };
        for &(wire, storage) in json_fields {
            let value = data.remove(wire).expect("parent exporter JSON column");
            data.insert(
                storage.into(),
                if discriminator == "task" && wire == "result" && value.is_null() {
                    Value::Null
                } else {
                    Value::String(value.to_string())
                },
            );
        }
        let boolean_fields: &[&str] = match discriminator.as_str() {
            "column" => &["hidden"],
            "label_ontology_observation" => &["suggest_needs_new_label", "suggest_degraded"],
            "label_ontology_signal" => &["agent_selected", "final_selected"],
            _ => &[],
        };
        for &field in boolean_fields {
            let value = data[field].as_bool().expect("parent exporter boolean");
            data.insert(field.into(), json!(i64::from(value)));
        }
    }
    records
}

fn fixture(contents: &str) -> Value {
    serde_json::from_str(contents).expect("committed fixture JSON")
}

fn contract_roundtrip<T>(contents: &str)
where
    T: DeserializeOwned + Serialize,
{
    let expected = fixture(contents);
    let typed: T = serde_json::from_value(expected.clone()).expect("fixture satisfies contract");
    assert_eq!(
        serde_json::to_value(typed).expect("serialize contract"),
        expected
    );
}

fn real_export_matches(path: &Path, discriminator: &str, contents: &str) {
    let actual = export_records(path)
        .into_iter()
        .find(|record| record["type"] == discriminator)
        .unwrap_or_else(|| panic!("missing exported {discriminator}"));
    assert_eq!(actual, fixture(contents));
}

fn real_import_consumes(discriminator: &str, contents: &str) {
    let (source_dir, source) = temp_db(&format!("portable-{discriminator}-source"));
    seed_ledger(&source);
    let expected = fixture(contents);
    let records = export_records(&source)
        .into_iter()
        .map(|record| {
            if record["type"] == discriminator {
                expected.clone()
            } else {
                record
            }
        })
        .collect::<Vec<_>>();
    let input = source_dir.path().join("import.jsonl");
    let mut file = fs::File::create(&input).expect("import fixture");
    for record in records {
        writeln!(file, "{record}").expect("write JSONL");
    }
    drop(file);

    let (_target_dir, target) = temp_db(&format!("portable-{discriminator}-target"));
    import_jsonl(&target, &input, true).expect("real import consumes fixture");
    real_export_matches(&target, discriminator, contents);
}

#[test]
fn parent_exporter_storage_native_snapshot_migrates_one_way() {
    let (source_dir, source) = temp_db("portable-parent-source");
    seed_ledger(&source);
    let expected = export_records(&source);
    let input = source_dir.path().join("parent.jsonl");
    let mut file = fs::File::create(&input).expect("parent snapshot");
    for record in parent_exporter_records(&source) {
        writeln!(file, "{record}").expect("write parent JSONL");
    }
    drop(file);

    let (_target_dir, target) = temp_db("portable-parent-target");
    import_jsonl(&target, &input, true).expect("migrate parent snapshot");
    assert_eq!(export_records(&target), expected);
}

#[test]
fn importer_rejects_hybrid_ledger_records_before_normalization_and_rolls_back_replace() {
    for (record_type, natural_field, storage_field) in [
        ("label_semantic_proposal", "diagnostics", "diagnostics_json"),
        (
            "label_ontology_observation",
            "task_snapshot",
            "task_snapshot_json",
        ),
        (
            "label_ontology_observation",
            "agent_candidates",
            "agent_candidates_json",
        ),
        (
            "label_ontology_observation",
            "suggestion_snapshot",
            "suggestion_snapshot_json",
        ),
        (
            "label_ontology_observation",
            "final_decision",
            "final_decision_json",
        ),
        (
            "label_ontology_observation",
            "diagnostics",
            "diagnostics_json",
        ),
        (
            "label_ontology_signal",
            "related_labels",
            "related_labels_json",
        ),
        ("label_ontology_signal", "proposal", "proposal_json"),
        ("label_ontology_action", "change", "change_json"),
        ("label_ontology_action", "validation", "validation_json"),
        ("signal_observation", "evidence", "evidence_json"),
        ("setting", "value", "value_json"),
    ] {
        let (source_dir, source) = temp_db(&format!(
            "portable-ledger-hybrid-source-{record_type}-{natural_field}"
        ));
        seed_ledger(&source);
        let natural_value = export_records(&source)
            .into_iter()
            .find(|record| record["type"] == record_type)
            .unwrap_or_else(|| panic!("missing natural {record_type} record"))["data"]
            .get(natural_field)
            .unwrap_or_else(|| panic!("missing natural {record_type}.{natural_field}"))
            .clone();
        let mut records = parent_exporter_records(&source);
        let record = records
            .iter_mut()
            .find(|record| record["type"] == record_type)
            .unwrap_or_else(|| panic!("missing parent {record_type} record"));
        let data = record["data"].as_object_mut().expect("record data");
        assert!(data.contains_key(storage_field));
        data.insert(natural_field.into(), natural_value);

        let input = source_dir.path().join("hybrid.jsonl");
        let mut file = fs::File::create(&input).expect("hybrid snapshot");
        for record in records {
            writeln!(file, "{record}").expect("write hybrid JSONL");
        }
        drop(file);

        let (_target_dir, target) = temp_db(&format!(
            "portable-ledger-hybrid-target-{record_type}-{natural_field}"
        ));
        let error = import_jsonl(&target, &input, true)
            .expect_err("same-record natural/storage-native keys must be rejected");
        let message = error.to_string();
        assert!(
            message.contains("cannot contain both natural and parent storage-native keys"),
            "{message}"
        );
        assert!(message.contains(natural_field), "{message}");
        assert!(message.contains(storage_field), "{message}");

        let conn = connect_file(&target).expect("connect rolled-back target");
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
            conn.query_row("SELECT COUNT(*) FROM label_ontology_actions", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count imported ledger actions"),
            0,
            "failed replace must roll back imported ledger rows"
        );
    }
}

fn rn<T>(value: Option<T>) -> RequiredNullable<T> {
    RequiredNullable(value)
}

fn object(value: Value) -> JsonObject {
    serde_json::from_value(value).expect("typed opaque object")
}

fn typed_label_input() -> LabelInput {
    PortableRecord {
        record_type: LabelRecordType::Label,
        data: LabelData {
            id: "l_fixture".into(),
            board_id: "b_fixture".into(),
            name: "rust".into(),
            color: rn(None),
            created_at: 1,
            updated_at: 2,
        },
    }
}

fn typed_label_semantics_input() -> LabelSemanticsInput {
    PortableRecord {
        record_type: LabelSemanticsRecordType::LabelSemantics,
        data: LabelSemanticsData {
            label_id: "l_fixture".into(),
            board_id: "b_fixture".into(),
            description: rn(None),
            applies_when: vec!["systems language".into()],
            excludes_when: vec![],
            positive_examples: vec!["cargo".into()],
            negative_examples: vec![],
            created_at: 1,
            updated_at: 2,
        },
    }
}

fn typed_label_atom_input() -> LabelAtomInput {
    PortableRecord {
        record_type: LabelAtomRecordType::LabelAtom,
        data: LabelAtomData {
            id: "la_fixture".into(),
            label_id: "l_fixture".into(),
            board_id: "b_fixture".into(),
            polarity: AtomPolarity::Positive,
            kind: AtomKind::PositiveExample,
            text: "cargo".into(),
            ordinal: 0,
            content_hash: "atom-hash".into(),
            created_at: 1,
            updated_at: 2,
        },
    }
}

fn typed_label_semantic_proposal_input() -> LabelSemanticProposalInput {
    PortableRecord {
        record_type: LabelSemanticProposalRecordType::LabelSemanticProposal,
        data: LabelSemanticProposalData {
            id: "lp_fixture".into(),
            board_id: "b_fixture".into(),
            task_id: "t_fixture".into(),
            status: ProposalStatus::Proposed,
            name: "rust-new".into(),
            description: rn(None),
            applies_when: vec!["rust".into()],
            excludes_when: vec![],
            positive_examples: vec!["cargo".into()],
            negative_examples: vec![],
            heuristic_coverage: 0.4,
            heuristic_residual_norm: 0.6,
            top1_existing_label_id: rn(Some("l_fixture".into())),
            top1_existing_label_name: rn(Some("rust".into())),
            diagnostics: vec![json!({"code": "candidate"})],
            created_by: "tester".into(),
            decision_reason: rn(None),
            resolved_label_id: rn(None),
            created_at: 1,
            updated_at: 2,
            decided_at: rn(None),
            heuristic_coverage_cosine: 0.3,
        },
    }
}

fn typed_label_ontology_observation_input() -> LabelOntologyObservationInput {
    PortableRecord {
        record_type: LabelOntologyObservationRecordType::LabelOntologyObservation,
        data: LabelOntologyObservationData {
            id: "lor_fixture".into(),
            board_id: "b_fixture".into(),
            task_id: "t_fixture".into(),
            task_ref_snapshot: "default#1".into(),
            task_snapshot: object(json!({"title": "fixture"})),
            agent_candidates: vec![json!({"label": "rust"})],
            suggestion_snapshot: object(json!({"selected": []})),
            final_decision: object(json!({"labels": ["rust"]})),
            suggest_coverage: rn(Some(0.8)),
            suggest_coverage_cosine: rn(Some(0.7)),
            suggest_residual_norm: rn(Some(0.2)),
            suggest_needs_new_label: false,
            suggest_degraded: false,
            diagnostics: vec![],
            capture_fingerprint: "capture-hash".into(),
            created_by: "tester".into(),
            created_by_type: ActorType::Agent,
            agent_type: rn(Some("codex".into())),
            created_at: 1,
            suggest_input_hash: rn(Some("input-hash".into())),
        },
    }
}

fn typed_label_ontology_signal_input() -> LabelOntologySignalInput {
    PortableRecord {
        record_type: LabelOntologySignalRecordType::LabelOntologySignal,
        data: LabelOntologySignalData {
            id: "los_fixture".into(),
            observation_id: "lor_fixture".into(),
            board_id: "b_fixture".into(),
            kind: OntologySignalKind::BoundaryIssue,
            status: SignalStatus::Open,
            target_label_id: rn(Some("l_fixture".into())),
            target_label_name_snapshot: rn(Some("rust".into())),
            related_labels: vec![json!("l_fixture")],
            proposed_action: OntologyProposedAction::Observe,
            candidate_atom_polarity: rn(None),
            candidate_atom_kind: rn(None),
            candidate_text: rn(None),
            candidate_content_hash: rn(None),
            proposed_label_name: rn(None),
            proposed_label_name_normalized: rn(None),
            proposal: object(json!({"source": "fixture"})),
            agent_selected: true,
            suggest_state: rn(Some(SuggestState::Selected)),
            suggest_score: rn(Some(0.9)),
            suggest_rank: rn(Some(1)),
            final_selected: true,
            rationale: "fixture rationale".into(),
            confidence: rn(Some(0.8)),
            signal_key: "fixture-key".into(),
            superseded_by_signal_id: rn(None),
            status_reason: rn(None),
            created_at: 1,
            updated_at: 2,
            reviewed_at: rn(None),
            closed_at: rn(None),
        },
    }
}

fn typed_label_ontology_action_input() -> LabelOntologyActionInput {
    PortableRecord {
        record_type: LabelOntologyActionRecordType::LabelOntologyAction,
        data: LabelOntologyActionData {
            id: "loa_fixture".into(),
            board_id: "b_fixture".into(),
            parent_action_id: rn(None),
            action_type: OntologyActionType::Confirm,
            reason: "fixture reason".into(),
            target_label_id: rn(Some("l_fixture".into())),
            result_label_id: rn(None),
            result_atom_id: rn(None),
            result_atom_content_hash: rn(None),
            result_proposal_id: rn(None),
            canonical_before_hash: rn(None),
            canonical_after_hash: rn(None),
            change: object(json!({"status": "confirmed"})),
            validation_status: ValidationStatus::NotRequired,
            validation: object(json!({})),
            created_by: "tester".into(),
            created_by_type: ActorType::Agent,
            agent_type: rn(Some("codex".into())),
            created_at: 1,
            validation_requirement: ValidationRequirement::None,
        },
    }
}

fn typed_label_ontology_action_atom_effect_input() -> LabelOntologyActionAtomEffectInput {
    PortableRecord {
        record_type: LabelOntologyActionAtomEffectRecordType::LabelOntologyActionAtomEffect,
        data: LabelOntologyActionAtomEffectData {
            board_id: "b_fixture".into(),
            action_id: "loa_fixture".into(),
            label_id_snapshot: "l_fixture".into(),
            atom_id_snapshot: "la_fixture".into(),
            atom_content_hash: "atom-hash".into(),
            polarity: AtomPolarity::Positive,
            kind: AtomKind::PositiveExample,
            text: "cargo".into(),
            effect: AtomEffect::Added,
            created_at: 1,
        },
    }
}

fn typed_label_ontology_action_signal_input() -> LabelOntologyActionSignalInput {
    PortableRecord {
        record_type: LabelOntologyActionSignalRecordType::LabelOntologyActionSignal,
        data: LabelOntologyActionSignalData {
            board_id: "b_fixture".into(),
            action_id: "loa_fixture".into(),
            signal_id: "los_fixture".into(),
            created_at: 1,
        },
    }
}

fn typed_signal_observation_input() -> SignalObservationInput {
    PortableRecord {
        record_type: SignalObservationRecordType::SignalObservation,
        data: SignalObservationData {
            id: "obs_fixture".into(),
            board_id: "b_fixture".into(),
            task_id: rn(Some("t_fixture".into())),
            task_ref_snapshot: rn(Some("default#1".into())),
            run_id: rn(None),
            comment_id: rn(None),
            actor: "tester".into(),
            agent_type: rn(Some("codex".into())),
            source: rn(Some("contract-test".into())),
            evidence: object(json!({"confidence": 0.9})),
            created_at: 1,
        },
    }
}

fn typed_signal_input() -> SignalInput {
    PortableRecord {
        record_type: SignalRecordType::Signal,
        data: SignalData {
            id: "sig_fixture".into(),
            board_id: "b_fixture".into(),
            observation_id: "obs_fixture".into(),
            kind: "quality".into(),
            title: "Fixture signal".into(),
            summary: "Fixture summary".into(),
            severity: "info".into(),
            status: SignalStatus::Open,
            dedupe_key: rn(Some("fixture".into())),
            superseded_by_signal_id: rn(None),
            reviewed_by: rn(None),
            reviewed_at: rn(None),
            review_reason: rn(None),
            created_at: 1,
            updated_at: 2,
        },
    }
}

fn typed_setting_input() -> SettingInput {
    PortableRecord {
        record_type: SettingRecordType::Setting,
        data: SettingData {
            key: "contract.fixture".into(),
            value: json!({"enabled": true}),
            updated_at: 2,
        },
    }
}

macro_rules! adoption_tests {
    ($disc:literal, $input:ty, $output:ty, $typed_input:path, $input_producer:ident, $input_consumer:ident, $output_producer:ident, $output_consumer:ident) => {
        #[test]
        fn $input_producer() {
            let typed: $input = $typed_input();
            assert_eq!(
                serde_json::to_value(typed).expect("serialize typed input contract"),
                fixture(include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../schemas/fixtures/jsonl/",
                    $disc,
                    "-input.v1.valid.json"
                )))
            );
        }

        #[test]
        fn $input_consumer() {
            real_import_consumes(
                $disc,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../schemas/fixtures/jsonl/",
                    $disc,
                    "-input.v1.valid.json"
                )),
            );
        }

        #[test]
        fn $output_producer() {
            let (_dir, path) = temp_db(concat!("portable-", $disc, "-output"));
            seed_ledger(&path);
            real_export_matches(
                &path,
                $disc,
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../schemas/fixtures/jsonl/",
                    $disc,
                    "-output.v1.valid.json"
                )),
            );
        }

        #[test]
        fn $output_consumer() {
            contract_roundtrip::<$output>(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/",
                $disc,
                "-output.v1.valid.json"
            )));
        }
    };
}

adoption_tests!(
    "label",
    LabelInput,
    LabelOutput,
    typed_label_input,
    label_input_fixture_is_produced_by_contract,
    label_input_fixture_is_consumed_by_real_import,
    label_output_fixture_is_produced_by_real_export,
    label_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "label_semantics",
    LabelSemanticsInput,
    LabelSemanticsOutput,
    typed_label_semantics_input,
    label_semantics_input_fixture_is_produced_by_contract,
    label_semantics_input_fixture_is_consumed_by_real_import,
    label_semantics_output_fixture_is_produced_by_real_export,
    label_semantics_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "label_atom",
    LabelAtomInput,
    LabelAtomOutput,
    typed_label_atom_input,
    label_atom_input_fixture_is_produced_by_contract,
    label_atom_input_fixture_is_consumed_by_real_import,
    label_atom_output_fixture_is_produced_by_real_export,
    label_atom_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "label_semantic_proposal",
    LabelSemanticProposalInput,
    LabelSemanticProposalOutput,
    typed_label_semantic_proposal_input,
    label_semantic_proposal_input_fixture_is_produced_by_contract,
    label_semantic_proposal_input_fixture_is_consumed_by_real_import,
    label_semantic_proposal_output_fixture_is_produced_by_real_export,
    label_semantic_proposal_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "label_ontology_observation",
    LabelOntologyObservationInput,
    LabelOntologyObservationOutput,
    typed_label_ontology_observation_input,
    label_ontology_observation_input_fixture_is_produced_by_contract,
    label_ontology_observation_input_fixture_is_consumed_by_real_import,
    label_ontology_observation_output_fixture_is_produced_by_real_export,
    label_ontology_observation_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "label_ontology_signal",
    LabelOntologySignalInput,
    LabelOntologySignalOutput,
    typed_label_ontology_signal_input,
    label_ontology_signal_input_fixture_is_produced_by_contract,
    label_ontology_signal_input_fixture_is_consumed_by_real_import,
    label_ontology_signal_output_fixture_is_produced_by_real_export,
    label_ontology_signal_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "label_ontology_action",
    LabelOntologyActionInput,
    LabelOntologyActionOutput,
    typed_label_ontology_action_input,
    label_ontology_action_input_fixture_is_produced_by_contract,
    label_ontology_action_input_fixture_is_consumed_by_real_import,
    label_ontology_action_output_fixture_is_produced_by_real_export,
    label_ontology_action_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "label_ontology_action_atom_effect",
    LabelOntologyActionAtomEffectInput,
    LabelOntologyActionAtomEffectOutput,
    typed_label_ontology_action_atom_effect_input,
    label_ontology_action_atom_effect_input_fixture_is_produced_by_contract,
    label_ontology_action_atom_effect_input_fixture_is_consumed_by_real_import,
    label_ontology_action_atom_effect_output_fixture_is_produced_by_real_export,
    label_ontology_action_atom_effect_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "label_ontology_action_signal",
    LabelOntologyActionSignalInput,
    LabelOntologyActionSignalOutput,
    typed_label_ontology_action_signal_input,
    label_ontology_action_signal_input_fixture_is_produced_by_contract,
    label_ontology_action_signal_input_fixture_is_consumed_by_real_import,
    label_ontology_action_signal_output_fixture_is_produced_by_real_export,
    label_ontology_action_signal_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "signal_observation",
    SignalObservationInput,
    SignalObservationOutput,
    typed_signal_observation_input,
    signal_observation_input_fixture_is_produced_by_contract,
    signal_observation_input_fixture_is_consumed_by_real_import,
    signal_observation_output_fixture_is_produced_by_real_export,
    signal_observation_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "signal",
    SignalInput,
    SignalOutput,
    typed_signal_input,
    signal_input_fixture_is_produced_by_contract,
    signal_input_fixture_is_consumed_by_real_import,
    signal_output_fixture_is_produced_by_real_export,
    signal_output_fixture_is_consumed_by_contract
);
adoption_tests!(
    "setting",
    SettingInput,
    SettingOutput,
    typed_setting_input,
    setting_input_fixture_is_produced_by_contract,
    setting_input_fixture_is_consumed_by_real_import,
    setting_output_fixture_is_produced_by_real_export,
    setting_output_fixture_is_consumed_by_contract
);

#[test]
fn ledger_contract_rejects_internal_json_keys_unknown_fields_and_missing_nullable_fields() {
    let valid = fixture(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../schemas/fixtures/jsonl/label_semantics-input.v1.valid.json"
    )));
    let mut old_key = valid.clone();
    let data = old_key["data"].as_object_mut().expect("data");
    let applies_when = data.remove("applies_when").expect("field");
    data.insert("applies_when_json".into(), applies_when);
    assert!(serde_json::from_value::<LabelSemanticsInput>(old_key).is_err());

    let mut unknown = valid.clone();
    unknown["data"]["unexpected"] = json!(true);
    assert!(serde_json::from_value::<LabelSemanticsInput>(unknown).is_err());

    for (discriminator, contents, nullable_fields) in [
        (
            "label",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/label-input.v1.valid.json"
            )),
            &["color"][..],
        ),
        (
            "label_semantics",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/label_semantics-input.v1.valid.json"
            )),
            &["description"][..],
        ),
        (
            "label_semantic_proposal",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/label_semantic_proposal-input.v1.valid.json"
            )),
            &[
                "description",
                "top1_existing_label_id",
                "top1_existing_label_name",
                "decision_reason",
                "resolved_label_id",
                "decided_at",
            ][..],
        ),
        (
            "label_ontology_observation",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/label_ontology_observation-input.v1.valid.json"
            )),
            &[
                "suggest_coverage",
                "suggest_coverage_cosine",
                "suggest_residual_norm",
                "agent_type",
                "suggest_input_hash",
            ][..],
        ),
        (
            "label_ontology_signal",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/label_ontology_signal-input.v1.valid.json"
            )),
            &[
                "target_label_id",
                "target_label_name_snapshot",
                "candidate_atom_polarity",
                "candidate_atom_kind",
                "candidate_text",
                "candidate_content_hash",
                "proposed_label_name",
                "proposed_label_name_normalized",
                "suggest_state",
                "suggest_score",
                "suggest_rank",
                "confidence",
                "superseded_by_signal_id",
                "status_reason",
                "reviewed_at",
                "closed_at",
            ][..],
        ),
        (
            "label_ontology_action",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/label_ontology_action-input.v1.valid.json"
            )),
            &[
                "parent_action_id",
                "target_label_id",
                "result_label_id",
                "result_atom_id",
                "result_atom_content_hash",
                "result_proposal_id",
                "canonical_before_hash",
                "canonical_after_hash",
                "agent_type",
            ][..],
        ),
        (
            "signal_observation",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/signal_observation-input.v1.valid.json"
            )),
            &[
                "task_id",
                "task_ref_snapshot",
                "run_id",
                "comment_id",
                "agent_type",
                "source",
            ][..],
        ),
        (
            "signal",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/signal-input.v1.valid.json"
            )),
            &[
                "dedupe_key",
                "superseded_by_signal_id",
                "reviewed_by",
                "reviewed_at",
                "review_reason",
            ][..],
        ),
    ] {
        let valid = fixture(contents);
        for field in nullable_fields {
            let mut missing = valid.clone();
            let data = missing["data"].as_object_mut().expect("data");
            assert!(
                data.remove(*field).is_some(),
                "fixture field {discriminator}.{field}"
            );
            assert!(
                kanban_contract::jsonl_ledger::validate_input_data(discriminator, data.clone(),)
                    .is_err(),
                "missing required-nullable field accepted: {discriminator}.{field}"
            );
        }
    }
}

#[test]
fn ledger_boolean_fields_reject_integer_and_non_boolean_wire_values() {
    for (discriminator, contents, fields) in [
        (
            "label_ontology_observation",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/label_ontology_observation-input.v1.valid.json"
            )),
            &["suggest_needs_new_label", "suggest_degraded"][..],
        ),
        (
            "label_ontology_signal",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../schemas/fixtures/jsonl/label_ontology_signal-input.v1.valid.json"
            )),
            &["agent_selected", "final_selected"][..],
        ),
    ] {
        for field in fields {
            let mut invalid = fixture(contents);
            invalid["data"][*field] = json!(1);
            assert!(
                kanban_contract::jsonl_ledger::validate_input_data(
                    discriminator,
                    invalid["data"].as_object().expect("data").clone(),
                )
                .is_err(),
                "integer boolean accepted: {discriminator}.{field}"
            );
        }
    }
}

#[test]
fn ontology_signal_import_rejects_non_candidate_atom_kind_and_rolls_back_replace() {
    let (source_dir, source) = temp_db("portable-invalid-candidate-kind-source");
    seed_ledger(&source);
    let mut records = export_records(&source);
    let signal = records
        .iter_mut()
        .find(|record| record["type"] == "label_ontology_signal")
        .expect("ontology signal");
    signal["data"]["candidate_atom_polarity"] = json!("positive");
    signal["data"]["candidate_atom_kind"] = json!("name");
    signal["data"]["candidate_text"] = json!("rust");
    signal["data"]["candidate_content_hash"] = json!("invalid-kind-hash");
    assert!(
        kanban_contract::jsonl_ledger::validate_input_data(
            "label_ontology_signal",
            signal["data"].as_object().expect("signal data").clone(),
        )
        .is_err(),
        "contract must reject non-candidate atom kinds before SQLite"
    );
    let input = source_dir.path().join("invalid-candidate-kind.jsonl");
    let mut file = fs::File::create(&input).expect("fixture");
    for record in records {
        writeln!(file, "{record}").expect("JSONL");
    }
    drop(file);

    let (_target_dir, target) = temp_db("portable-invalid-candidate-kind-target");
    let before: i64 = connect_file(&target)
        .expect("connect before")
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .expect("board count before");
    let error = import_jsonl(&target, &input, true).expect_err("name kind must fail closed");
    assert!(
        error
            .to_string()
            .contains("label_ontology_signal import row violates portable contract"),
        "unexpected error: {error}"
    );
    let after: i64 = connect_file(&target)
        .expect("connect after")
        .query_row("SELECT COUNT(*) FROM boards", [], |row| row.get(0))
        .expect("board count after");
    assert_eq!(
        after, before,
        "replace transaction must roll back on contract error"
    );
}

#[test]
fn ledger_runtime_ownership_is_fail_closed_for_every_discriminator() {
    for discriminator in [
        "label",
        "label_semantics",
        "label_atom",
        "label_semantic_proposal",
        "label_ontology_observation",
        "label_ontology_signal",
        "label_ontology_action",
        "label_ontology_action_atom_effect",
        "label_ontology_action_signal",
        "signal_observation",
        "signal",
        "setting",
    ] {
        let (_dir, path) = temp_db(&format!("portable-{discriminator}-ownership"));
        seed_ledger(&path);
        let record = export_records(&path)
            .into_iter()
            .find(|record| record["type"] == discriminator)
            .expect("owned record");
        assert!(
            record["data"]
                .as_object()
                .is_some_and(|data| !data.is_empty())
        );
    }
}

#[test]
fn empty_ledger_data_probe_cannot_bypass_real_import_validation() {
    let (dir, path) = temp_db("portable-empty-ledger-import");
    let input = dir.path().join("empty.jsonl");
    fs::write(&input, "{\"type\":\"label\",\"data\":{}}\n").expect("empty fixture");
    let error = import_jsonl(&path, &input, true).expect_err("empty public record must fail");
    assert!(
        error
            .to_string()
            .contains("export record data cannot be empty")
    );
}
