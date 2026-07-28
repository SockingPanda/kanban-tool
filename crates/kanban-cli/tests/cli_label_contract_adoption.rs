mod common;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use common::{TempDb, kanban, kanban_in_dir_str_envs};
use kanban_contract::cli_labels::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

// Exact output fixtures use this actor; pass it explicitly instead of inheriting
// the host username.
const FIXTURE_ACTOR: &str = "root";

fn fixture(slug: &str, valid: bool) -> Result<Value> {
    let suffix = if valid { "valid" } else { "invalid" };
    let path = if slug == "label-add-task" {
        format!(
            "{}/tests/fixtures/contracts/{slug}-output.v1.{suffix}.json",
            env!("CARGO_MANIFEST_DIR")
        )
    } else {
        format!(
            "{}/../../schemas/fixtures/cli/{slug}-output.v1.{suffix}.json",
            env!("CARGO_MANIFEST_DIR")
        )
    };
    serde_json::from_str(&fs::read_to_string(&path).with_context(|| format!("read {path}"))?)
        .with_context(|| format!("parse {path}"))
}

fn consume<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).context("consume exact CLI label output contract")
}

fn assert_producer<T: DeserializeOwned>(slug: &str) -> Result<()> {
    let actual = produce(slug)?;
    let _: T = consume(actual.clone())?;
    assert_eq!(normalize(actual), normalize(fixture(slug, true)?));
    Ok(())
}

fn assert_consumer<T: DeserializeOwned>(slug: &str) -> Result<()> {
    let valid = fixture(slug, true)?;
    let _: T = consume(valid.clone())?;
    assert!(consume::<T>(fixture(slug, false)?).is_err());
    for missing in variants_with_each_null_field_removed(&valid) {
        assert!(
            consume::<T>(missing.clone()).is_err(),
            "{slug} accepted a producer-required nullable field after removal: {missing}"
        );
    }
    Ok(())
}

fn variants_with_each_null_field_removed(value: &Value) -> Vec<Value> {
    let mut paths = Vec::new();
    collect_null_paths(value, &mut Vec::new(), &mut paths);
    paths
        .into_iter()
        .map(|path| {
            let mut candidate = value.clone();
            remove_object_field(&mut candidate, &path);
            candidate
        })
        .collect()
}

#[derive(Clone, Debug)]
enum PathPart {
    Field(String),
    Index(usize),
}

fn collect_null_paths(value: &Value, current: &mut Vec<PathPart>, paths: &mut Vec<Vec<PathPart>>) {
    match value {
        Value::Object(object) => {
            for (field, value) in object {
                current.push(PathPart::Field(field.clone()));
                if value.is_null() {
                    paths.push(current.clone());
                } else if is_opaque_natural_json_field(field) {
                    // Structured metadata is natural JSON but remains intentionally opaque;
                    // nested nullability belongs to its producer, not this envelope DTO.
                } else {
                    collect_null_paths(value, current, paths);
                }
                current.pop();
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                current.push(PathPart::Index(index));
                collect_null_paths(value, current, paths);
                current.pop();
            }
        }
        _ => {}
    }
}

fn is_opaque_natural_json_field(field: &str) -> bool {
    matches!(
        field,
        "evidence"
            | "related_labels"
            | "proposal"
            | "change"
            | "validation"
            | "task_snapshot"
            | "agent_candidates"
            | "suggestion_snapshot"
            | "final_decision"
            | "diagnostics"
    )
}

fn remove_object_field(value: &mut Value, path: &[PathPart]) {
    let (last, parents) = path.split_last().expect("null field path");
    let mut parent = value;
    for part in parents {
        parent = match part {
            PathPart::Field(field) => parent
                .as_object_mut()
                .and_then(|object| object.get_mut(field))
                .expect("object field path"),
            PathPart::Index(index) => parent
                .as_array_mut()
                .and_then(|array| array.get_mut(*index))
                .expect("array index path"),
        };
    }
    let PathPart::Field(field) = last else {
        panic!("null path must end in an object field");
    };
    parent
        .as_object_mut()
        .expect("null field parent")
        .remove(field);
}

fn setup(name: &str) -> Result<TempDb> {
    let temp = TempDb::new(name)?;
    kanban(&temp.path, &fixture_args(&["init"]))?.success()?;
    Ok(temp)
}

fn fixture_args<'a>(args: &[&'a str]) -> Vec<&'a str> {
    let mut fixture_args = Vec::with_capacity(args.len() + 2);
    fixture_args.extend(["--actor", FIXTURE_ACTOR]);
    fixture_args.extend_from_slice(args);
    fixture_args
}

fn run(temp: &TempDb, args: &[&str]) -> Result<Value> {
    kanban(&temp.path, &fixture_args(args))?.success_json()
}

fn create_task(temp: &TempDb, title: &str) -> Result<String> {
    let output = run(
        temp,
        &[
            "--json",
            "task",
            "create",
            title,
            "--description",
            "fixture task description",
            "--status",
            "ready",
        ],
    )?;
    output["data"]["id"]
        .as_str()
        .map(str::to_owned)
        .context("task id")
}

fn create_label(temp: &TempDb, name: &str) -> Result<Value> {
    run(
        temp,
        &["--json", "label", "create", name, "--color", "blue"],
    )
}

fn seed_semantics(temp: &TempDb) -> Result<Value> {
    create_label(temp, "backend")?;
    run(
        temp,
        &[
            "--json",
            "label",
            "semantics",
            "upsert",
            "backend",
            "--description",
            "Backend implementation work",
            "--applies-when",
            "touches Rust service code",
            "--positive-example",
            "add a service command",
            "--reason",
            "fixture semantics",
        ],
    )
}

#[cfg(unix)]
fn write_executable(path: &Path, body: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, body)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(unix)]
fn vector_helper(temp: &TempDb) -> Result<PathBuf> {
    let helper = temp.dir.join("label-contract-vector-helper.py");
    write_executable(
        &helper,
        r#"#!/usr/bin/env python3
import json, sys
cmd = sys.argv[1]
if cmd == "label-atoms-status":
    payload = {"backend":"fixture-helper","enabled":True,"message":"fixture label status","diagnostics":["fixture"],"dirty":False,"board_dirty":False}
elif cmd == "rebuild-label-atoms":
    payload = {"backend":"fixture-helper","enabled":True,"message":"fixture labels rebuilt","diagnostics":["fixture"],"dirty":False,"board_dirty":False,"generation":7}
elif cmd == "query-label-atoms":
    hit = {"atom_id":"lat_fixture","label_id":"l_fixture","label_name":"backend","board_id":"b_fixture","polarity":"positive","kind":"applies_when","text":"touches Rust service code","ordinal":0,"content_hash":"hash_fixture","embedding_model":"fixture-model","distance":0.0}
    payload = [{"hit":hit,"vector":[1.0,0.0]}] if "--vector-json" in sys.argv else [hit]
elif cmd == "status":
    payload = {"backend":"fixture-helper","enabled":True,"message":"fixture vector status","diagnostics":[],"dirty":False,"board_dirty":False}
elif cmd == "embed-query":
    payload = [1.0, 0.0]
else:
    payload = []
print(json.dumps({"protocol":"kanban-derived-helper.v1","payload_json":json.dumps(payload)}))
"#,
    )?;
    Ok(helper)
}

#[cfg(unix)]
fn run_with_helper(temp: &TempDb, args: &[&str]) -> Result<Value> {
    let helper = vector_helper(temp)?;
    let args = fixture_args(args);
    kanban_in_dir_str_envs(
        &temp.path,
        &args,
        &temp.dir,
        &[(
            "KANBAN_VECTOR_HELPER",
            helper.to_str().context("helper path")?,
        )],
    )?
    .success_json()
}

fn run_without_helper(temp: &TempDb, args: &[&str]) -> Result<Value> {
    let missing = temp.dir.join("missing-vector-helper");
    let args = fixture_args(args);
    kanban_in_dir_str_envs(
        &temp.path,
        &args,
        &temp.dir,
        &[(
            "KANBAN_VECTOR_HELPER",
            missing.to_str().context("missing helper path")?,
        )],
    )?
    .success_json()
}

#[test]
fn fixture_commands_pin_actor_instead_of_using_host_username() {
    assert_eq!(
        fixture_args(&["--json", "label", "list"]),
        ["--actor", FIXTURE_ACTOR, "--json", "label", "list"]
    );
}

fn seed_proposal(temp: &TempDb, task_id: &str, name: &str) -> Result<String> {
    let conn = kanban_test_support::connect_file(&temp.path)?;
    let board_id: String =
        conn.query_row("SELECT board_id FROM tasks WHERE id=?1", [task_id], |row| {
            row.get(0)
        })?;
    let proposal_id = format!("lp_fixture_{name}");
    conn.execute(
        "INSERT INTO label_semantic_proposals(
            id, board_id, task_id, status, name, description, applies_when, excludes_when,
            positive_examples, negative_examples, heuristic_coverage,
            heuristic_coverage_cosine, heuristic_residual_norm, diagnostics_json,
            created_by, created_at, updated_at
         ) VALUES (?1, ?2, ?3, 'proposed', ?4, 'Fixture proposal',
            '[\"touches Rust service code\"]', '[]', '[\"service change\"]', '[]',
            0.0, 0.0, 1.0, '[]', 'fixture', 100, 100)",
        (&proposal_id, &board_id, task_id, name),
    )?;
    Ok(proposal_id)
}

fn record_signal(temp: &TempDb, suffix: &str) -> Result<Value> {
    let task_id = create_task(temp, &format!("ontology fixture {suffix}"))?;
    if run(temp, &["--json", "label", "list"])?["data"]
        .as_array()
        .is_some_and(Vec::is_empty)
    {
        create_label(temp, "backend")?;
    }
    let input = temp.dir.join(format!("ontology-record-{suffix}.json"));
    let payload = json!({
        "actor": {"name": "fixture-agent", "type": "agent", "agent_type": "executor"},
        "agent_candidates": [{"label": "backend", "reason": "fixture"}],
        "suggestion_snapshot": {
            "task_id": task_id,
            "board_id": "fixture-board-snapshot",
            "selected_labels": [],
            "candidates": [],
            "coverage": 0.0,
            "coverage_cosine": 0.0,
            "residual_norm": 1.0,
            "needs_new_label": false,
            "reason_codes": ["no_selected_labels"],
            "degraded": false,
            "diagnostics": []
        },
        "final_decision": {"selected": ["backend"], "rejected": []},
        "signals": [{
            "kind": "false_negative",
            "target_label_ref": "backend",
            "related_labels": [],
            "proposed_action": "add_positive_atom",
            "candidate_atom": {
                "polarity": "positive",
                "kind": "applies_when",
                "text": format!("touches Rust service code {suffix}")
            },
            "proposal": {},
            "agent_selected": true,
            "suggest_state": "absent",
            "suggest_score": 0.0,
            "suggest_rank": null,
            "final_selected": true,
            "rationale": format!("fixture rationale {suffix}"),
            "confidence": 0.9
        }]
    });
    fs::write(&input, serde_json::to_vec_pretty(&payload)?)?;
    run(
        temp,
        &[
            "--json",
            "label",
            "ontology",
            "record",
            &task_id,
            "--input",
            input.to_str().context("ontology input path")?,
        ],
    )
}

fn signal_id(observation: &Value) -> Result<String> {
    observation["data"]["signals"][0]["id"]
        .as_str()
        .map(str::to_owned)
        .context("ontology signal id")
}

fn confirm_signal(temp: &TempDb, suffix: &str) -> Result<(String, Value)> {
    let observation = record_signal(temp, suffix)?;
    let id = signal_id(&observation)?;
    let action = run(
        temp,
        &[
            "--json",
            "label",
            "ontology",
            "confirm",
            &id,
            "--reason",
            "fixture confirmation",
            "--actor-type",
            "agent",
            "--agent-type",
            "reviewer",
        ],
    )?;
    Ok((id, action))
}

fn apply_atom(temp: &TempDb, suffix: &str) -> Result<Value> {
    let (signal_id, _) = confirm_signal(temp, suffix)?;
    run(
        temp,
        &[
            "--json",
            "label",
            "ontology",
            "apply",
            "atom",
            &signal_id,
            "--label",
            "backend",
            "--kind",
            "applies-when",
            "--text",
            &format!("canonical fixture atom {suffix}"),
            "--reason",
            "fixture atom adoption",
            "--actor-type",
            "agent",
            "--agent-type",
            "reviewer",
        ],
    )
}

fn produce(slug: &str) -> Result<Value> {
    let temp = setup(slug)?;
    match slug {
        "label-list" => run(&temp, &["--json", "label", "list"]),
        "label-create" => create_label(&temp, "backend"),
        "label-delete" => {
            create_label(&temp, "backend")?;
            run(&temp, &["--json", "label", "delete", "backend"])
        }
        "label-bootstrap" => {
            let task = create_task(&temp, "bootstrap fixture")?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "bootstrap",
                    &task,
                    "backend",
                    "--description",
                    "Backend implementation work",
                    "--applies-when",
                    "touches Rust service code",
                    "--positive-example",
                    "add a service command",
                ],
            )
        }
        "label-add" => {
            let task = create_task(&temp, "add fixture")?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "add",
                    "--create-missing",
                    &task,
                    "backend",
                ],
            )
        }
        "label-add-task" => {
            let task = create_task(&temp, "add task variant fixture")?;
            create_label(&temp, "backend")?;
            run(&temp, &["--json", "label", "add", &task, "backend"])
        }
        "label-remove" => {
            let task = create_task(&temp, "remove fixture")?;
            create_label(&temp, "backend")?;
            run(&temp, &["--json", "label", "add", &task, "backend"])?;
            run(&temp, &["--json", "label", "remove", &task, "backend"])
        }
        "label-semantics-list" => {
            seed_semantics(&temp)?;
            run(&temp, &["--json", "label", "semantics", "list"])
        }
        "label-semantics-show" => {
            seed_semantics(&temp)?;
            run(&temp, &["--json", "label", "semantics", "show", "backend"])
        }
        "label-semantics-upsert" => seed_semantics(&temp),
        "label-semantics-delete" => {
            let semantics = seed_semantics(&temp)?;
            let hash = semantics["data"]["semantics_hash"]
                .as_str()
                .context("semantics hash")?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "semantics",
                    "delete",
                    "backend",
                    "--expected-semantics-hash",
                    hash,
                    "--reason",
                    "fixture clear",
                ],
            )
        }
        "label-atoms-list" => {
            seed_semantics(&temp)?;
            run(&temp, &["--json", "label", "atoms", "list"])
        }
        "label-atoms-explain" => {
            let semantics = seed_semantics(&temp)?;
            let atom = semantics["data"]["atoms"][0]["id"]
                .as_str()
                .context("atom id")?;
            run(&temp, &["--json", "label", "atoms", "explain", atom])
        }
        "label-atom-index-status" => {
            run_without_helper(&temp, &["--json", "label", "atom-index", "status"])
        }
        "label-atom-index-rebuild" => {
            run_with_helper(&temp, &["--json", "label", "atom-index", "rebuild"])
        }
        "label-atom-index-query" => run_with_helper(
            &temp,
            &["--json", "label", "atom-index", "query", "backend"],
        ),
        "label-suggest" => {
            let task = create_task(&temp, "suggest fixture")?;
            run_with_helper(&temp, &["--json", "label", "suggest", &task])
        }
        "label-propose" => {
            let task = create_task(&temp, "propose fixture")?;
            run_with_helper(&temp, &["--json", "label", "propose", &task])
        }
        "label-proposals-list" => {
            let task = create_task(&temp, "proposal list fixture")?;
            seed_proposal(&temp, &task, "database")?;
            run(&temp, &["--json", "label", "proposals", "list"])
        }
        "label-proposals-show" => {
            let task = create_task(&temp, "proposal show fixture")?;
            let id = seed_proposal(&temp, &task, "database")?;
            run(&temp, &["--json", "label", "proposals", "show", &id])
        }
        "label-proposals-accept" => {
            let task = create_task(&temp, "proposal accept fixture")?;
            let id = seed_proposal(&temp, &task, "database")?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "proposals",
                    "accept",
                    &id,
                    "--reason",
                    "fixture accept",
                ],
            )
        }
        "label-proposals-reject" => {
            let task = create_task(&temp, "proposal reject fixture")?;
            let id = seed_proposal(&temp, &task, "database")?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "proposals",
                    "reject",
                    &id,
                    "--reason",
                    "fixture reject",
                ],
            )
        }
        "label-ontology-record" => record_signal(&temp, "record"),
        "label-ontology-list" => {
            record_signal(&temp, "list")?;
            run(
                &temp,
                &["--json", "label", "ontology", "list", "--include-all"],
            )
        }
        "label-ontology-show" => {
            let observation = record_signal(&temp, "show")?;
            let id = signal_id(&observation)?;
            run(&temp, &["--json", "label", "ontology", "show", &id])
        }
        "label-ontology-review" => {
            record_signal(&temp, "review")?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "ontology",
                    "review",
                    "--group-by",
                    "label",
                    "--include-all",
                ],
            )
        }
        "label-ontology-quality" => {
            record_signal(&temp, "quality")?;
            run(&temp, &["--json", "label", "ontology", "quality"])
        }
        "label-ontology-confirm" => confirm_signal(&temp, "confirm").map(|(_, value)| value),
        "label-ontology-reject" => {
            let observation = record_signal(&temp, "reject")?;
            let id = signal_id(&observation)?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "ontology",
                    "reject",
                    &id,
                    "--reason",
                    "fixture reject",
                ],
            )
        }
        "label-ontology-resolve" => {
            let observation = record_signal(&temp, "resolve")?;
            let id = signal_id(&observation)?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "ontology",
                    "resolve",
                    &id,
                    "--no-change",
                    "--reason",
                    "fixture no change",
                ],
            )
        }
        "label-ontology-supersede" => {
            let first = signal_id(&record_signal(&temp, "supersede-first")?)?;
            let second = signal_id(&record_signal(&temp, "supersede-second")?)?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "ontology",
                    "supersede",
                    &first,
                    "--by",
                    &second,
                    "--reason",
                    "fixture duplicate",
                ],
            )
        }
        "label-ontology-apply-atom" => apply_atom(&temp, "apply"),
        "label-ontology-revert" => {
            let action = apply_atom(&temp, "revert")?;
            let id = action["data"]["id"].as_str().context("action id")?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "ontology",
                    "revert",
                    id,
                    "--reason",
                    "fixture revert",
                ],
            )
        }
        "label-ontology-validate" => {
            let action = apply_atom(&temp, "validate")?;
            let id = action["data"]["id"].as_str().context("action id")?;
            let input = temp.dir.join("ontology-validation.json");
            fs::write(
                &input,
                include_bytes!(
                    "../../../schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json"
                ),
            )?;
            run(
                &temp,
                &[
                    "--json",
                    "label",
                    "ontology",
                    "validate",
                    id,
                    "--status",
                    "failed",
                    "--reason",
                    "fixture validation",
                    "--input",
                    input.to_str().context("validation input")?,
                ],
            )
        }
        _ => anyhow::bail!("unknown CLI label contract slug {slug}"),
    }
}

#[derive(Default)]
struct Normalizer {
    ids: BTreeMap<String, String>,
    prefix_counts: BTreeMap<String, usize>,
}

fn normalize(value: Value) -> Value {
    let mut normalizer = Normalizer::default();
    normalizer.value(value, None)
}

impl Normalizer {
    fn value(&mut self, value: Value, key: Option<&str>) -> Value {
        match value {
            Value::Object(object) => Value::Object(
                object
                    .into_iter()
                    .map(|(key, value)| {
                        let value = self.value(value, Some(&key));
                        (key, value)
                    })
                    .collect(),
            ),
            Value::Array(values) => Value::Array(
                values
                    .into_iter()
                    .map(|value| self.value(value, key))
                    .collect(),
            ),
            Value::String(value) => self.string(value, key),
            Value::Number(_) if key.is_some_and(is_dynamic_number_key) => json!(100),
            Value::Number(number)
                if number.is_f64()
                    && number.as_f64().is_some_and(|number| number.fract() == 0.0) =>
            {
                json!(number.as_f64().expect("f64 number") as i64)
            }
            other => other,
        }
    }

    fn string(&mut self, value: String, key: Option<&str>) -> Value {
        if key == Some("message") && value.contains("missing-vector-helper") {
            return json!("fixture missing vector helper");
        }
        if key.is_some_and(|key| key.ends_with("_json"))
            && let Ok(nested) = serde_json::from_str::<Value>(&value)
        {
            return Value::String(
                serde_json::to_string(&self.value(nested, None)).expect("normalize nested JSON"),
            );
        }
        if key.is_some_and(|key| {
            key.contains("hash") || key.contains("fingerprint") || key == "signal_key"
        }) {
            return json!("hash_fixture");
        }
        if let Some(prefix) = dynamic_id_prefix(&value) {
            let normalized = if let Some(existing) = self.ids.get(&value) {
                existing.clone()
            } else {
                let count = self.prefix_counts.entry(prefix.to_owned()).or_default();
                *count += 1;
                let normalized = format!("{prefix}FIXTURE_{count}");
                self.ids.insert(value, normalized.clone());
                normalized
            };
            return Value::String(normalized);
        }
        Value::String(value)
    }
}

fn dynamic_id_prefix(value: &str) -> Option<&'static str> {
    [
        "lat_", "lor_", "los_", "loa_", "lp_", "la_", "lo_", "b_", "t_", "l_", "r_", "e_",
    ]
    .into_iter()
    .find(|prefix| value.starts_with(prefix))
}

fn is_dynamic_number_key(key: &str) -> bool {
    key.ends_with("_at") || key.ends_with("_age_ms")
}

macro_rules! contract_case {
    ($producer:ident, $consumer:ident, $root:ty, $slug:literal) => {
        #[test]
        fn $producer() -> Result<()> {
            assert_producer::<$root>($slug)
        }

        #[test]
        fn $consumer() -> Result<()> {
            assert_consumer::<$root>($slug)
        }
    };
}

contract_case!(
    producer_label_add_matches_exact_fixture,
    label_add_output_fixture_is_consumed_by_public_contract,
    CliLabelAddOutput,
    "label-add"
);

#[test]
fn metadata_ontology_validation_evidence_input_fixture_is_consumed_by_real_cli() -> Result<()> {
    let temp = setup("metadata_ontology_validation_manual_extension")?;
    let action = apply_atom(&temp, "metadata-validation-manual")?;
    let id = action["data"]["id"].as_str().context("action id")?;
    let mut evidence: Value = serde_json::from_slice(include_bytes!(
        "../../../schemas/fixtures/metadata/ontology-validation-evidence-input.v1.valid.json"
    ))?;
    let manual = json!({
        "reviewer": "fixture-human",
        "ticket": "manual-extension-437"
    });
    evidence["manual"] = manual.clone();
    let input = temp.dir.join("ontology-validation-manual-extension.json");
    fs::write(&input, serde_json::to_vec_pretty(&evidence)?)?;
    let output = run(
        &temp,
        &[
            "--json",
            "label",
            "ontology",
            "validate",
            id,
            "--status",
            "failed",
            "--reason",
            "fixture validation with manual extension",
            "--input",
            input.to_str().context("validation input")?,
        ],
    )?;
    let _: CliLabelOntologyValidateOutput = consume(output.clone())?;
    assert_eq!(output["data"]["validation"]["manual"], evidence);
    assert_eq!(output["data"]["validation"]["manual"]["manual"], manual);
    Ok(())
}
contract_case!(
    producer_label_add_task_variant_matches_exact_fixture,
    label_add_task_variant_output_fixture_is_consumed_by_public_contract,
    CliLabelAddOutput,
    "label-add-task"
);
contract_case!(
    producer_label_atom_index_query_matches_exact_fixture,
    label_atom_index_query_output_fixture_is_consumed_by_public_contract,
    CliLabelAtomIndexQueryOutput,
    "label-atom-index-query"
);
contract_case!(
    producer_label_atom_index_rebuild_matches_exact_fixture,
    label_atom_index_rebuild_output_fixture_is_consumed_by_public_contract,
    CliLabelAtomIndexRebuildOutput,
    "label-atom-index-rebuild"
);
contract_case!(
    producer_label_atom_index_status_matches_exact_fixture,
    label_atom_index_status_output_fixture_is_consumed_by_public_contract,
    CliLabelAtomIndexStatusOutput,
    "label-atom-index-status"
);
contract_case!(
    producer_label_atoms_explain_matches_exact_fixture,
    label_atoms_explain_output_fixture_is_consumed_by_public_contract,
    CliLabelAtomsExplainOutput,
    "label-atoms-explain"
);
contract_case!(
    producer_label_atoms_list_matches_exact_fixture,
    label_atoms_list_output_fixture_is_consumed_by_public_contract,
    CliLabelAtomsListOutput,
    "label-atoms-list"
);
contract_case!(
    producer_label_bootstrap_matches_exact_fixture,
    label_bootstrap_output_fixture_is_consumed_by_public_contract,
    CliLabelBootstrapOutput,
    "label-bootstrap"
);
contract_case!(
    producer_label_create_matches_exact_fixture,
    label_create_output_fixture_is_consumed_by_public_contract,
    CliLabelCreateOutput,
    "label-create"
);
contract_case!(
    producer_label_delete_matches_exact_fixture,
    label_delete_output_fixture_is_consumed_by_public_contract,
    CliLabelDeleteOutput,
    "label-delete"
);
contract_case!(
    producer_label_list_matches_exact_fixture,
    label_list_output_fixture_is_consumed_by_public_contract,
    CliLabelListOutput,
    "label-list"
);
contract_case!(
    producer_label_ontology_apply_atom_matches_exact_fixture,
    label_ontology_apply_atom_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyApplyAtomOutput,
    "label-ontology-apply-atom"
);
contract_case!(
    producer_label_ontology_confirm_matches_exact_fixture,
    label_ontology_confirm_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyConfirmOutput,
    "label-ontology-confirm"
);
contract_case!(
    producer_label_ontology_list_matches_exact_fixture,
    label_ontology_list_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyListOutput,
    "label-ontology-list"
);
contract_case!(
    producer_label_ontology_quality_matches_exact_fixture,
    label_ontology_quality_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyQualityOutput,
    "label-ontology-quality"
);
contract_case!(
    producer_label_ontology_record_matches_exact_fixture,
    label_ontology_record_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyRecordOutput,
    "label-ontology-record"
);
contract_case!(
    producer_label_ontology_reject_matches_exact_fixture,
    label_ontology_reject_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyRejectOutput,
    "label-ontology-reject"
);
contract_case!(
    producer_label_ontology_resolve_matches_exact_fixture,
    label_ontology_resolve_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyResolveOutput,
    "label-ontology-resolve"
);
contract_case!(
    producer_label_ontology_revert_matches_exact_fixture,
    label_ontology_revert_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyRevertOutput,
    "label-ontology-revert"
);
contract_case!(
    producer_label_ontology_review_matches_exact_fixture,
    label_ontology_review_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyReviewOutput,
    "label-ontology-review"
);
contract_case!(
    producer_label_ontology_show_matches_exact_fixture,
    label_ontology_show_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyShowOutput,
    "label-ontology-show"
);
contract_case!(
    producer_label_ontology_supersede_matches_exact_fixture,
    label_ontology_supersede_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologySupersedeOutput,
    "label-ontology-supersede"
);
contract_case!(
    producer_label_ontology_validate_matches_exact_fixture,
    label_ontology_validate_output_fixture_is_consumed_by_public_contract,
    CliLabelOntologyValidateOutput,
    "label-ontology-validate"
);
contract_case!(
    producer_label_proposals_accept_matches_exact_fixture,
    label_proposals_accept_output_fixture_is_consumed_by_public_contract,
    CliLabelProposalsAcceptOutput,
    "label-proposals-accept"
);
contract_case!(
    producer_label_proposals_list_matches_exact_fixture,
    label_proposals_list_output_fixture_is_consumed_by_public_contract,
    CliLabelProposalsListOutput,
    "label-proposals-list"
);
contract_case!(
    producer_label_proposals_reject_matches_exact_fixture,
    label_proposals_reject_output_fixture_is_consumed_by_public_contract,
    CliLabelProposalsRejectOutput,
    "label-proposals-reject"
);
contract_case!(
    producer_label_proposals_show_matches_exact_fixture,
    label_proposals_show_output_fixture_is_consumed_by_public_contract,
    CliLabelProposalsShowOutput,
    "label-proposals-show"
);
contract_case!(
    producer_label_propose_matches_exact_fixture,
    label_propose_output_fixture_is_consumed_by_public_contract,
    CliLabelProposeOutput,
    "label-propose"
);
contract_case!(
    producer_label_remove_matches_exact_fixture,
    label_remove_output_fixture_is_consumed_by_public_contract,
    CliLabelRemoveOutput,
    "label-remove"
);
contract_case!(
    producer_label_semantics_delete_matches_exact_fixture,
    label_semantics_delete_output_fixture_is_consumed_by_public_contract,
    CliLabelSemanticsDeleteOutput,
    "label-semantics-delete"
);
contract_case!(
    producer_label_semantics_list_matches_exact_fixture,
    label_semantics_list_output_fixture_is_consumed_by_public_contract,
    CliLabelSemanticsListOutput,
    "label-semantics-list"
);
contract_case!(
    producer_label_semantics_show_matches_exact_fixture,
    label_semantics_show_output_fixture_is_consumed_by_public_contract,
    CliLabelSemanticsShowOutput,
    "label-semantics-show"
);
contract_case!(
    producer_label_semantics_upsert_matches_exact_fixture,
    label_semantics_upsert_output_fixture_is_consumed_by_public_contract,
    CliLabelSemanticsUpsertOutput,
    "label-semantics-upsert"
);
contract_case!(
    producer_label_suggest_matches_exact_fixture,
    label_suggest_output_fixture_is_consumed_by_public_contract,
    CliLabelSuggestOutput,
    "label-suggest"
);

#[test]
fn label_add_contract_accepts_both_closed_variants_and_rejects_ambiguous_shape() -> Result<()> {
    let task_variant = fixture("label-add-task", true)?;
    let with_created_variant = fixture("label-add", true)?;
    let _: CliLabelAddOutput = consume(task_variant.clone())?;
    let _: CliLabelAddOutput = consume(with_created_variant)?;

    let task = task_variant["data"].clone();
    let mut ambiguous = task_variant;
    let data = ambiguous["data"]
        .as_object_mut()
        .context("task variant data")?;
    data.insert("task".to_owned(), task);
    data.insert("created_labels".to_owned(), json!([]));
    assert!(consume::<CliLabelAddOutput>(ambiguous).is_err());

    let mut unknown = fixture("label-add", true)?;
    unknown["data"]
        .as_object_mut()
        .context("with-created variant data")?
        .insert("unexpected".to_owned(), json!(true));
    assert!(consume::<CliLabelAddOutput>(unknown).is_err());
    Ok(())
}

#[derive(Clone, Copy)]
struct HandlerBinding {
    operation: &'static str,
    root: &'static str,
    constructor: &'static str,
    renderer: &'static str,
}

const HANDLER_BINDINGS: &[HandlerBinding] = &[
    binding(
        "label list",
        "CliLabelListOutput",
        "let output: CliLabelListOutput = contract_output(&labels)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label create",
        "CliLabelCreateOutput",
        "let output: CliLabelCreateOutput = contract_output(&label)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label delete",
        "CliLabelDeleteOutput",
        "let output: CliLabelDeleteOutput = contract_output(&result)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label bootstrap",
        "CliLabelBootstrapOutput",
        "let output = CliLabelBootstrapOutput::new(output);",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label add",
        "CliLabelAddOutput",
        "let output = CliLabelAddOutput::new(CliLabelAddResult::WithCreated(data));",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label add",
        "CliLabelAddOutput",
        "let output = CliLabelAddOutput::new(CliLabelAddResult::Task(",
        "print_contract_or_human(true, &output",
    ),
    binding(
        "label remove",
        "CliLabelRemoveOutput",
        "let output = CliLabelRemoveOutput::new(api_task_from_record(&task)?);",
        "print_contract_or_human(true, &output",
    ),
    binding(
        "label atoms list",
        "CliLabelAtomsListOutput",
        "let output: CliLabelAtomsListOutput = contract_output(&atoms)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label atoms explain",
        "CliLabelAtomsExplainOutput",
        "let output: CliLabelAtomsExplainOutput = label_atom_explain_contract_output(&explain)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label suggest",
        "CliLabelSuggestOutput",
        "let output: CliLabelSuggestOutput = contract_output(&suggestions)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label propose",
        "CliLabelProposeOutput",
        "let output: CliLabelProposeOutput = contract_output(&attempt)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label proposals list",
        "CliLabelProposalsListOutput",
        "let output: CliLabelProposalsListOutput = contract_output(&proposals)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label proposals show",
        "CliLabelProposalsShowOutput",
        "let output: CliLabelProposalsShowOutput = contract_output(&proposal)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label proposals accept",
        "CliLabelProposalsAcceptOutput",
        "let output: CliLabelProposalsAcceptOutput = contract_output(&proposal)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label proposals reject",
        "CliLabelProposalsRejectOutput",
        "let output: CliLabelProposalsRejectOutput = contract_output(&proposal)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label semantics list",
        "CliLabelSemanticsListOutput",
        "let output: CliLabelSemanticsListOutput = contract_output(&semantics)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label semantics show",
        "CliLabelSemanticsShowOutput",
        "let output: CliLabelSemanticsShowOutput = contract_output(&semantics)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label semantics upsert",
        "CliLabelSemanticsUpsertOutput",
        "let output: CliLabelSemanticsUpsertOutput = contract_output(&semantics)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label semantics delete",
        "CliLabelSemanticsDeleteOutput",
        "CliLabelSemanticsDeleteOutput::new(CliLabelSemanticsDeleteResult { deleted: true })",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label atom-index status",
        "CliLabelAtomIndexStatusOutput",
        "CliLabelAtomIndexStatusOutput::new(label_atom_index_status_contract(",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label atom-index rebuild",
        "CliLabelAtomIndexRebuildOutput",
        "CliLabelAtomIndexRebuildOutput::new(label_atom_index_status_contract(",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label atom-index query",
        "CliLabelAtomIndexQueryOutput",
        "let output: CliLabelAtomIndexQueryOutput = contract_output(&hits)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology record",
        "CliLabelOntologyRecordOutput",
        "let output: CliLabelOntologyRecordOutput = contract_output(&observation)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology list",
        "CliLabelOntologyListOutput",
        "let output: CliLabelOntologyListOutput = contract_output(&signals)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology show",
        "CliLabelOntologyShowOutput",
        "let output: CliLabelOntologyShowOutput = contract_output(&detail)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology review",
        "CliLabelOntologyReviewOutput",
        "let output: CliLabelOntologyReviewOutput = contract_output(&groups)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology quality",
        "CliLabelOntologyQualityOutput",
        "let output: CliLabelOntologyQualityOutput = contract_output(&report)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology confirm",
        "CliLabelOntologyConfirmOutput",
        "let output: CliLabelOntologyConfirmOutput = contract_output(&action)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology reject",
        "CliLabelOntologyRejectOutput",
        "let output: CliLabelOntologyRejectOutput = contract_output(&action)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology supersede",
        "CliLabelOntologySupersedeOutput",
        "let output: CliLabelOntologySupersedeOutput = contract_output(&action)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology resolve",
        "CliLabelOntologyResolveOutput",
        "let output: CliLabelOntologyResolveOutput = contract_output(&action)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology apply atom",
        "CliLabelOntologyApplyAtomOutput",
        "let output: CliLabelOntologyApplyAtomOutput = contract_output(&action)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology revert",
        "CliLabelOntologyRevertOutput",
        "let output: CliLabelOntologyRevertOutput = contract_output(&action)?;",
        "print_contract_or_human(json, &output",
    ),
    binding(
        "label ontology validate",
        "CliLabelOntologyValidateOutput",
        "let output: CliLabelOntologyValidateOutput = contract_output(&action)?;",
        "print_contract_or_human(json, &output",
    ),
];

const fn binding(
    operation: &'static str,
    root: &'static str,
    constructor: &'static str,
    renderer: &'static str,
) -> HandlerBinding {
    HandlerBinding {
        operation,
        root,
        constructor,
        renderer,
    }
}

fn normalized_source(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn check_handler_binding(body: &str, binding: HandlerBinding) -> Result<(), String> {
    let body = normalized_source(body);
    let constructor = normalized_source(binding.constructor);
    let renderer = normalized_source(binding.renderer);
    if !constructor.contains(binding.root) {
        return Err(format!(
            "{} constructor does not name {}",
            binding.operation, binding.root
        ));
    }
    let constructor_at = body
        .find(&constructor)
        .ok_or_else(|| format!("{} missing constructor `{constructor}`", binding.operation))?;
    let after_constructor = &body[constructor_at + constructor.len()..];
    let renderer_at = after_constructor.find(&renderer).ok_or_else(|| {
        format!(
            "{} missing contract renderer `{renderer}`",
            binding.operation
        )
    })?;
    if after_constructor[..renderer_at].contains(" let output") {
        return Err(format!(
            "{} constructor is not bound to the next contract renderer",
            binding.operation
        ));
    }
    Ok(())
}

#[test]
fn all_33_label_handlers_bind_root_constructor_and_contract_renderer() {
    let source = include_str!("../src/commands/label.rs");
    let body = source
        .split_once("pub(crate) fn handle_label(")
        .expect("label handler body")
        .1;
    assert!(!body.contains("print_or_json(json"));
    assert!(!body.contains("LabelAddCommandOutput"));
    assert!(!body.contains("LabelBootstrapCommandOutput"));
    let operations = HANDLER_BINDINGS
        .iter()
        .map(|binding| binding.operation)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(operations.len(), 33);
    for binding in HANDLER_BINDINGS {
        check_handler_binding(body, *binding).unwrap_or_else(|error| panic!("{error}"));
    }
}

#[test]
fn handler_binding_gate_rejects_root_constructor_renderer_and_manual_envelope_mutations() {
    let binding = binding(
        "label list",
        "CliLabelListOutput",
        "let output: CliLabelListOutput = contract_output(&labels)?;",
        "print_contract_or_human(json, &output",
    );
    let valid = "let output: CliLabelListOutput = contract_output(&labels)?; print_contract_or_human(json, &output, || String::new())?;";
    assert!(check_handler_binding(valid, binding).is_ok());
    for mutation in [
        valid.replace("CliLabelListOutput", "PrivateLabelListOutput"),
        valid.replace("contract_output(&labels)?", "labels"),
        valid.replace("contract_output(&labels)?", "DataEnvelope::new(labels)"),
        valid.replace("print_contract_or_human", "print_or_json"),
    ] {
        assert!(
            check_handler_binding(&mutation, binding).is_err(),
            "ownership mutation unexpectedly passed: {mutation}"
        );
    }
}
