use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use tempfile::TempDir;

const PASS_FIXTURES: &[&str] = &[
    "tests/ui/api_facade_contract.rs",
    "tests/ui/api_provider_plane_contract.rs",
    "tests/ui/api_lifecycle_plane_contract.rs",
    "tests/ui/api_projection_v2_contract.rs",
    "tests/ui/db_init_module_contract.rs",
];

const COMPILE_FAIL_FIXTURES: &[&str] = &[
    "tests/ui/trusted_evidence_private.rs",
    "tests/ui/structure_plan_writer_private.rs",
    "tests/ui/api_facade_excludes_non_slice_symbol.rs",
    "tests/ui/api_facade_excludes_provider_vector_helper.rs",
    "tests/ui/root_legacy_reexport_removed.rs",
    "tests/ui/api_root_excludes_provider/build_context_pack_with_vector_store.rs",
    "tests/ui/api_root_excludes_provider/disabled_label_proposal_provider.rs",
    "tests/ui/api_root_excludes_provider/label_atom_index_status_with.rs",
    "tests/ui/api_root_excludes_provider/label_ontology_trusted_validation_input.rs",
    "tests/ui/api_root_excludes_provider/label_proposal_provider.rs",
    "tests/ui/api_root_excludes_provider/manual_label_proposal_provider.rs",
    "tests/ui/api_root_excludes_provider/projection_corpus_metadata.rs",
    "tests/ui/api_root_excludes_provider/propose_task_label_with.rs",
    "tests/ui/api_root_excludes_provider/propose_task_label_with_create_options.rs",
    "tests/ui/api_root_excludes_provider/propose_task_label_with_store.rs",
    "tests/ui/api_root_excludes_provider/propose_task_label_with_store_and_create_options.rs",
    "tests/ui/api_root_excludes_provider/query_label_atom_index_by_vector_with.rs",
    "tests/ui/api_root_excludes_provider/query_label_atom_index_with.rs",
    "tests/ui/api_root_excludes_provider/rebuild_label_atom_index_with.rs",
    "tests/ui/api_root_excludes_provider/rebuild_vector_store_with.rs",
    "tests/ui/api_root_excludes_provider/suggest_task_labels_with.rs",
    "tests/ui/api_root_excludes_provider/sync_vector_store_with.rs",
    "tests/ui/api_root_excludes_provider/validate_label_ontology_action_with_trusted_suggestions.rs",
    "tests/ui/api_root_excludes_provider/vector_store_status_with.rs",
    "tests/ui/api_root_excludes_lifecycle/begin_database_replace.rs",
    "tests/ui/api_root_excludes_lifecycle/begin_database_runtime.rs",
    "tests/ui/api_root_excludes_lifecycle/database_replace_guard.rs",
    "tests/ui/api_root_excludes_lifecycle/database_runtime_guard.rs",
    "tests/ui/api_projection_v2_provider_root_private.rs",
    "tests/ui/api_root_excludes_db_init/connect.rs",
    "tests/ui/api_root_excludes_db_init/connect_existing_database_read_only.rs",
    "tests/ui/api_root_excludes_db_init/connect_existing_read_only.rs",
    "tests/ui/api_root_excludes_db_init/connect_file.rs",
    "tests/ui/api_root_excludes_db_init/default_pragmas.rs",
    "tests/ui/api_root_excludes_db_init/init_database.rs",
    "tests/ui/database_connection_no_deref_mut.rs",
    "tests/ui/database_connection_no_into_inner.rs",
];

const KNOWN_SUGGESTION_DRIFT_FIXTURE: &str = "tests/ui/database_connection_no_into_inner.rs";
const KNOWN_SUGGESTION_DRIFT_TAIL: &str = "\n  |\nhelp: there is a method `into_either` with a similar name, but with different arguments\n --> $CARGO/either-$VERSION/src/into_either.rs\n  |\n  |     fn into_either(self, into_left: bool) -> Either<Self, Self> {\n  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n";

fn assert_fixture_inventory() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let (disk_pass, disk_compile_fail) = fixture_inventory(&crate_dir);
    let declared_pass = declared_fixture_set("pass", PASS_FIXTURES);
    let declared_compile_fail = declared_fixture_set("compile-fail", COMPILE_FAIL_FIXTURES);

    assert!(
        declared_pass.is_disjoint(&declared_compile_fail),
        "a fixture cannot be both pass and compile-fail"
    );
    assert_eq!(
        disk_pass, declared_pass,
        "pass fixture inventory differs from tests/ui"
    );
    assert_eq!(
        disk_compile_fail, declared_compile_fail,
        "compile-fail fixture inventory differs from tests/ui"
    );
}

fn declared_fixture_set(kind: &str, fixtures: &[&str]) -> BTreeSet<String> {
    let fixture_set = fixtures
        .iter()
        .map(|fixture| (*fixture).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fixture_set.len(),
        fixtures.len(),
        "duplicate {kind} fixture declaration"
    );
    fixture_set
}

fn fixture_inventory(crate_dir: &Path) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut source_files = BTreeSet::new();
    let mut stderr_files = BTreeSet::new();
    collect_ui_files(
        &crate_dir.join("tests").join("ui"),
        &mut source_files,
        &mut stderr_files,
    );

    for stderr_file in &stderr_files {
        assert!(
            source_files.contains(&stderr_file.with_extension("rs")),
            "compile-fail diagnostic without a source fixture: {}",
            stderr_file.display()
        );
    }

    let mut pass = BTreeSet::new();
    let mut compile_fail = BTreeSet::new();
    for source_file in source_files {
        let fixture = source_file
            .strip_prefix(crate_dir)
            .expect("UI fixture belongs to crate directory");
        let fixture = normalize_fixture_path(fixture);
        if stderr_files.contains(&source_file.with_extension("stderr")) {
            compile_fail.insert(fixture);
        } else {
            pass.insert(fixture);
        }
    }
    (pass, compile_fail)
}

fn collect_ui_files(
    directory: &Path,
    source_files: &mut BTreeSet<PathBuf>,
    stderr_files: &mut BTreeSet<PathBuf>,
) {
    for entry in fs::read_dir(directory).expect("read UI fixture directory") {
        let path = entry.expect("read UI fixture entry").path();
        if path.is_dir() {
            collect_ui_files(&path, source_files, stderr_files);
            continue;
        }
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("rs") => {
                source_files.insert(path);
            }
            Some("stderr") => {
                stderr_files.insert(path);
            }
            _ => {}
        }
    }
}

fn normalize_fixture_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

struct CompileHarness {
    _directory: TempDir,
    manifest_path: PathBuf,
}

impl CompileHarness {
    fn new(fixtures: &[&str]) -> Self {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_dir = crate_dir
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let directory = tempfile::tempdir().expect("compile harness directory");
        let manifest_path = directory.path().join("Cargo.toml");

        let mut manifest = format!(
            "[package]\nname = \"kanban-sqlite-public-api-contract\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n\n[dependencies]\nkanban-sqlite = {{ path = {} }}\nserde_json = \"1.0\"\nrusqlite = {{ version = \"0.32\", features = [\"bundled\"] }}\n",
            toml_string(&crate_dir),
        );
        for (index, fixture) in fixtures.iter().enumerate() {
            manifest.push_str(&format!(
                "\n[[bin]]\nname = \"fixture-{index}\"\npath = {}\n",
                toml_string(&crate_dir.join(fixture)),
            ));
        }
        fs::write(&manifest_path, manifest).expect("write compile harness manifest");
        fs::copy(
            workspace_dir.join("Cargo.lock"),
            directory.path().join("Cargo.lock"),
        )
        .expect("copy workspace lockfile into compile harness");

        Self {
            _directory: directory,
            manifest_path,
        }
    }

    fn check(&self, fixture_index: usize) -> Output {
        Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args([
                "check",
                "--offline",
                "--quiet",
                "--color=never",
                "--manifest-path",
            ])
            .arg(&self.manifest_path)
            .args(["--bin", &format!("fixture-{fixture_index}")])
            .output()
            .expect("run Cargo public API fixture")
    }
}

fn toml_string(path: &Path) -> String {
    serde_json::to_string(&path.to_string_lossy()).expect("serialize TOML path")
}

fn rendered_diagnostics(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .split("error: could not compile")
        .next()
        .unwrap_or_default()
        .split("\nFor more information about this error")
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn normalize_diagnostic_paths(diagnostic: &str, crate_dir: &Path) -> String {
    let crate_dir = normalize_fixture_path(crate_dir);
    diagnostic
        .replace('\\', "/")
        .replace(&format!("{crate_dir}/"), "")
}

fn normalize_known_suggestion_drift(
    fixture: &str,
    diagnostic: &str,
    checked_in_snapshot: bool,
) -> String {
    if fixture != KNOWN_SUGGESTION_DRIFT_FIXTURE {
        return diagnostic.to_owned();
    }

    let diagnostic = if checked_in_snapshot {
        diagnostic
            .strip_suffix(KNOWN_SUGGESTION_DRIFT_TAIL)
            .expect("known suggestion drift snapshot changed")
    } else {
        assert!(
            !diagnostic.contains("\nhelp: "),
            "known suggestion drift gained a compiler help section"
        );
        diagnostic
    };
    diagnostic
        .lines()
        .map(trim_caret_message)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn trim_caret_message(line: &str) -> String {
    let Some((prefix, suffix)) = line.split_once('|') else {
        return line.to_owned();
    };
    let marker = suffix.trim_start();
    if !marker.starts_with('^') {
        return line.to_owned();
    }
    let caret_len = marker.bytes().take_while(|byte| *byte == b'^').count();
    let indentation_len = suffix.len() - marker.len();
    format!(
        "{prefix}|{}{}",
        &suffix[..indentation_len],
        &marker[..caret_len]
    )
}

fn assert_compile_fail(harness: &CompileHarness, fixture_index: usize, fixture: &str) {
    let output = harness.check(fixture_index);
    assert!(
        !output.status.success(),
        "{fixture} unexpectedly compiled\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let expected = fs::read_to_string(crate_dir.join(fixture).with_extension("stderr"))
        .expect("read checked-in compile-fail diagnostic")
        .replace("\r\n", "\n");
    let actual = normalize_known_suggestion_drift(
        fixture,
        &normalize_diagnostic_paths(&rendered_diagnostics(&output), &crate_dir),
        false,
    );
    assert!(
        !actual.trim().is_empty(),
        "{fixture} failed without a compiler diagnostic; refusing to accept an unattributed Cargo failure\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let expected = normalize_known_suggestion_drift(
        fixture,
        &normalize_diagnostic_paths(&expected, &crate_dir),
        true,
    );
    assert_eq!(
        actual,
        expected,
        "{fixture} diagnostic changed\nraw compiler stderr:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

fn assert_compile_pass(harness: &CompileHarness, fixture_index: usize, fixture: &str) {
    let output = harness.check(fixture_index);
    assert!(
        output.status.success(),
        "{fixture} failed to compile\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn public_api_compile_contract() {
    let fixtures = PASS_FIXTURES
        .iter()
        .chain(COMPILE_FAIL_FIXTURES)
        .copied()
        .collect::<Vec<_>>();
    let harness = CompileHarness::new(&fixtures);

    for (fixture_index, fixture) in PASS_FIXTURES.iter().enumerate() {
        assert_compile_pass(&harness, fixture_index, fixture);
    }
    for (fixture_index, fixture) in COMPILE_FAIL_FIXTURES.iter().enumerate() {
        assert_compile_fail(&harness, PASS_FIXTURES.len() + fixture_index, fixture);
    }
}

#[test]
fn public_api_fixture_inventory_is_complete() {
    assert_fixture_inventory();
}

#[test]
fn diagnostic_paths_are_portable() {
    assert_eq!(
        normalize_diagnostic_paths(
            " --> /repo/crates/kanban-sqlite/tests/ui/private.rs:1:1\n",
            Path::new("/repo/crates/kanban-sqlite"),
        ),
        " --> tests/ui/private.rs:1:1\n",
    );
    assert_eq!(
        normalize_diagnostic_paths(
            " --> C:\\repo\\crates\\kanban-sqlite\\tests\\ui\\private.rs:1:1\n",
            Path::new(r"C:\repo\crates\kanban-sqlite"),
        ),
        " --> tests/ui/private.rs:1:1\n",
    );
}

#[test]
fn only_the_known_suggestion_drift_is_normalized() {
    assert_eq!(
        normalize_known_suggestion_drift(
            "tests/ui/database_connection_no_into_inner.rs",
            "error[E0599]\n  |\n4 | value.into_inner()\n  |       ^^^^^^^^^^ detail\n",
            false,
        ),
        "error[E0599]\n  |\n4 | value.into_inner()\n  |       ^^^^^^^^^^\n",
    );
    assert_eq!(
        normalize_known_suggestion_drift(
            "tests/ui/other.rs",
            "error[E0432]\nhelp: an unexpected suggestion\n",
            false,
        ),
        "error[E0432]\nhelp: an unexpected suggestion\n",
    );
}
