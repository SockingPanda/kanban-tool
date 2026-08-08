use std::{collections::BTreeMap, path::Path, process::Command};

use serde::Serialize;

use crate::git::{self, Sources};
use xtask::ToolResult;

/// affected 只允许调用这些仓库级 recipe，避免重新引入 package 命令分叉。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Recipe {
    DocsCheck,
    RustFast,
    RustFull,
    WebCheck,
    DesktopCheck,
    #[serde(rename = "schema-contract")]
    SchemaCheck,
    DepsCheck,
    ToolingCheck,
    DiffCheck,
}

impl Recipe {
    fn name(self) -> &'static str {
        match self {
            Self::DocsCheck => "docs-check",
            Self::RustFast => "rust-fast",
            Self::RustFull => "rust-full",
            Self::WebCheck => "web-check",
            Self::DesktopCheck => "desktop-check",
            Self::SchemaCheck => "schema-contract",
            Self::DepsCheck => "deps-check",
            Self::ToolingCheck => "tooling-check",
            Self::DiffCheck => "diff-check",
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct Plan {
    pub(crate) base: String,
    pub(crate) changed_files: Vec<String>,
    pub(crate) classifications: BTreeMap<String, Vec<String>>,
    pub(crate) recipes: Vec<Recipe>,
    pub(crate) sources: Sources,
}

pub(crate) fn run(root: &Path, command: &str, base: &str) -> ToolResult<()> {
    if command == "self-test" {
        self_test();
        println!("affected planner self-test 已通过");
        return Ok(());
    }
    if !matches!(command, "plan" | "json" | "run") {
        return Err(std::io::Error::other(format!("未知 affected command: {command}")).into());
    }

    let base = normalise_base(base)?;
    let sources = git::changed_sources(root, &base)?;
    let plan = build_plan(base, sources);
    match command {
        "plan" => print_plan(&plan),
        "json" => println!(
            "{}",
            serde_json::to_string_pretty(&plan)
                .map_err(|error| std::io::Error::other(format!("affected JSON 失败: {error}")))?
        ),
        "run" => execute(root, &plan)?,
        other => {
            return Err(std::io::Error::other(format!("未知 affected command: {other}")).into());
        }
    }
    Ok(())
}

pub(crate) fn normalise_base(base: &str) -> ToolResult<String> {
    let base = base.strip_prefix("base=").unwrap_or(base);
    if base.is_empty() {
        return Err(std::io::Error::other("--base 不能为空").into());
    }
    Ok(base.to_owned())
}

fn build_plan(base: String, sources: Sources) -> Plan {
    let changed_files = sources.merged();
    let classifications = classify(&changed_files);
    let recipes = plan_recipes(&changed_files);
    Plan {
        base,
        changed_files,
        classifications,
        recipes,
        sources,
    }
}

fn classify(paths: &[String]) -> BTreeMap<String, Vec<String>> {
    let mut classifications = BTreeMap::new();
    for (name, predicate) in [
        ("docs-only", is_document as fn(&str) -> bool),
        ("root-risk", is_root_risk),
        ("tooling", is_tooling),
        ("schema", is_schema),
        ("web", is_web),
        ("desktop", is_desktop),
        ("rust", is_rust_product),
    ] {
        let matches = paths
            .iter()
            .filter(|path| predicate(path))
            .cloned()
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            classifications.insert(name.to_owned(), matches);
        }
    }
    classifications
}

fn plan_recipes(paths: &[String]) -> Vec<Recipe> {
    if paths.is_empty() {
        return Vec::new();
    }

    let mut recipes = Vec::new();
    let docs_only =
        paths.iter().all(|path| is_document(path)) && paths.iter().all(|path| !is_tooling(path));
    if docs_only {
        recipes.push(Recipe::DocsCheck);
    } else {
        let root_risk = paths.iter().any(|path| is_root_risk(path));
        if root_risk {
            recipes.push(Recipe::RustFull);
            recipes.push(Recipe::DepsCheck);
            recipes.push(Recipe::ToolingCheck);
        } else if paths.iter().any(|path| is_tooling(path)) {
            recipes.push(Recipe::ToolingCheck);
        }
        if paths.iter().any(|path| is_schema(path)) {
            recipes.push(Recipe::SchemaCheck);
        }
        if paths.iter().any(|path| is_web(path)) {
            recipes.push(Recipe::WebCheck);
        }
        if paths.iter().any(|path| is_desktop(path)) {
            recipes.push(Recipe::DesktopCheck);
        }
        if !root_risk && paths.iter().any(|path| is_rust_product(path)) {
            recipes.push(Recipe::RustFast);
        }
    }
    recipes.push(Recipe::DiffCheck);
    dedupe_recipes(recipes)
}

fn dedupe_recipes(recipes: Vec<Recipe>) -> Vec<Recipe> {
    let mut deduped = Vec::new();
    for recipe in recipes {
        if !deduped.contains(&recipe) {
            deduped.push(recipe);
        }
    }
    deduped
}

fn is_document(path: &str) -> bool {
    path == "README.md" || path.starts_with("docs/") || path.ends_with(".md")
}

fn is_root_risk(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    let exact = matches!(
        path,
        "Cargo.toml"
            | "Cargo.lock"
            | "rust-toolchain.toml"
            | ".cargo/config"
            | ".cargo/config.toml"
            | ".config/nextest.toml"
            | "deny.toml"
    );
    let ci = path.starts_with(".github/workflows/");
    let node_manifest = file_name == "package.json";
    let named_risk = !node_manifest
        && !file_name.ends_with(".md")
        && (file_name.contains("release")
            || file_name.contains("version")
            || file_name.contains("package"));
    exact || ci || (path.ends_with("Cargo.toml")) || named_risk
}

fn is_tooling(path: &str) -> bool {
    path == "justfile"
        || path.starts_with("xtask/")
        || path.starts_with("scripts/")
        || path.starts_with(".github/workflows/")
        || path.starts_with(".agents/")
}

fn is_schema(path: &str) -> bool {
    path.starts_with("crates/kanban-protocol/")
        || path.starts_with("schemas/")
        || path.starts_with("migrations/")
}

fn is_desktop(path: &str) -> bool {
    path.starts_with("apps/desktop/") || is_root_node_workspace_file(path)
}

fn is_web(path: &str) -> bool {
    path.starts_with("apps/web/")
        || path == "xtask/src/web_assets.rs"
        || path.starts_with("crates/kanban-web-artifact/")
        || matches!(
            path,
            "crates/kanban-protocol/src/web_artifact.rs"
                | "crates/kanban-protocol/tests/web_artifact.rs"
        )
        || is_root_node_workspace_file(path)
}

fn is_root_node_workspace_file(path: &str) -> bool {
    matches!(
        path,
        "package.json" | "pnpm-lock.yaml" | "pnpm-workspace.yaml"
    )
}

fn is_rust_product(path: &str) -> bool {
    path.starts_with("crates/")
        && !is_document(path)
        && !path.ends_with("Cargo.toml")
        && !path.ends_with("Cargo.lock")
}

fn print_plan(plan: &Plan) {
    println!("base: {}", plan.base);
    println!("changed_files:");
    if plan.changed_files.is_empty() {
        println!("  - <none>");
    } else {
        for path in &plan.changed_files {
            println!("  - {path}");
        }
    }
    println!("classifications:");
    if plan.classifications.is_empty() {
        println!("  - <none>");
    } else {
        for (name, paths) in &plan.classifications {
            println!("  {name}:");
            for path in paths {
                println!("    - {path}");
            }
        }
    }
    println!("recipes:");
    if plan.recipes.is_empty() {
        println!("  - <none>");
    } else {
        for recipe in &plan.recipes {
            println!("  - {}", recipe.name());
        }
    }
}

fn execute(root: &Path, plan: &Plan) -> ToolResult<()> {
    for recipe in &plan.recipes {
        let name = recipe.name();
        println!("+ just {name}");
        let status = Command::new("just")
            .arg(name)
            .current_dir(root)
            .status()
            .map_err(|error| std::io::Error::other(format!("执行 just {name} 失败: {error}")))?;
        if !status.success() {
            let code = status
                .code()
                .map_or_else(|| "signal".to_owned(), |code| code.to_string());
            return Err(std::io::Error::other(format!("just {name} 失败（exit={code}）")).into());
        }
    }
    Ok(())
}

fn self_test() {
    let empty = Sources::default();
    let plan = build_plan("main".to_owned(), empty);
    assert!(plan.changed_files.is_empty());
    assert!(plan.recipes.is_empty());

    let docs = Sources {
        working_tree: vec!["docs/guide.md".to_owned(), "README.md".to_owned()],
        ..Sources::default()
    };
    assert_eq!(
        build_plan("main".to_owned(), docs).recipes,
        vec![Recipe::DocsCheck, Recipe::DiffCheck]
    );

    let overlap = Sources {
        base: vec!["crates/kanban-protocol/src/schema.rs".to_owned()],
        staged: vec!["crates/kanban-protocol/src/schema.rs".to_owned()],
        ..Sources::default()
    };
    let plan = build_plan("main".to_owned(), overlap);
    assert_eq!(plan.changed_files, ["crates/kanban-protocol/src/schema.rs"]);
    assert_eq!(
        plan.recipes,
        vec![Recipe::SchemaCheck, Recipe::RustFast, Recipe::DiffCheck]
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        os::unix::fs::PermissionsExt,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn sources(paths: &[&str]) -> Sources {
        Sources {
            working_tree: paths.iter().map(|path| (*path).to_owned()).collect(),
            ..Sources::default()
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!("xtask-affected-{label}-{nonce}"));
        fs::create_dir_all(&root).expect("temporary root should be creatable");
        root
    }

    #[test]
    fn source_merge_is_stable_and_deduplicated() {
        let sources = Sources {
            base: vec!["b".to_owned(), "shared".to_owned()],
            staged: vec!["a".to_owned(), "shared".to_owned()],
            working_tree: vec!["c".to_owned(), "a".to_owned()],
            untracked: vec!["d".to_owned(), "shared".to_owned()],
        };
        assert_eq!(sources.merged(), ["b", "shared", "a", "c", "d"]);
    }

    #[test]
    fn overlapping_paths_keep_all_classifications_but_dedupe_recipes() {
        let plan = build_plan(
            "main".to_owned(),
            sources(&["apps/desktop/src-tauri/Cargo.toml"]),
        );
        assert!(plan.classifications.contains_key("root-risk"));
        assert!(plan.classifications.contains_key("desktop"));
        assert_eq!(
            plan.recipes,
            vec![
                Recipe::RustFull,
                Recipe::DepsCheck,
                Recipe::ToolingCheck,
                Recipe::DesktopCheck,
                Recipe::DiffCheck,
            ]
        );
        let keys = plan
            .recipes
            .iter()
            .map(|recipe| recipe.name())
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), {
            let mut unique = keys.clone();
            unique.sort_unstable();
            unique.dedup();
            unique.len()
        });
    }

    #[test]
    fn empty_plan_has_no_recipe() {
        let plan = build_plan("main".to_owned(), Sources::default());
        assert!(plan.recipes.is_empty());
        assert!(plan.classifications.is_empty());
    }

    #[test]
    fn docs_only_uses_narrow_gate() {
        let plan = build_plan(
            "main".to_owned(),
            sources(&["README.md", "docs/architecture.md"]),
        );
        assert_eq!(plan.recipes, vec![Recipe::DocsCheck, Recipe::DiffCheck]);
    }

    #[test]
    fn core_schema_desktop_and_tooling_paths_route_as_expected() {
        assert_eq!(
            build_plan(
                "main".to_owned(),
                sources(&["crates/kanban-service/src/lib.rs"])
            )
            .recipes,
            vec![Recipe::RustFast, Recipe::DiffCheck]
        );
        assert_eq!(
            build_plan("main".to_owned(), sources(&["schemas/fixtures/api.json"])).recipes,
            vec![Recipe::SchemaCheck, Recipe::DiffCheck]
        );
        assert_eq!(
            build_plan("main".to_owned(), sources(&["apps/desktop/src/App.tsx"])).recipes,
            vec![Recipe::DesktopCheck, Recipe::DiffCheck]
        );
        assert_eq!(
            build_plan("main".to_owned(), sources(&["xtask/src/affected.rs"])).recipes,
            vec![Recipe::ToolingCheck, Recipe::DiffCheck]
        );
    }

    #[test]
    fn web_paths_use_the_web_gate() {
        let plan = build_plan("main".to_owned(), sources(&["apps/web/src/App.tsx"]));
        let recipes = plan
            .recipes
            .iter()
            .map(|recipe| recipe.name())
            .collect::<Vec<_>>();

        assert_eq!(plan.classifications["web"], ["apps/web/src/App.tsx"]);
        assert!(!plan.classifications.contains_key("desktop"));
        assert_eq!(recipes, ["web-check", "diff-check"]);
    }

    #[test]
    fn web_artifact_tooling_and_verifier_paths_use_web_and_owner_gates() {
        let tooling = build_plan("main".to_owned(), sources(&["xtask/src/web_assets.rs"]));
        assert_eq!(
            tooling.recipes,
            vec![Recipe::ToolingCheck, Recipe::WebCheck, Recipe::DiffCheck]
        );
        assert_eq!(tooling.classifications["web"], ["xtask/src/web_assets.rs"]);

        let verifier = build_plan(
            "main".to_owned(),
            sources(&["crates/kanban-web-artifact/src/lib.rs"]),
        );
        assert_eq!(
            verifier.recipes,
            vec![Recipe::WebCheck, Recipe::RustFast, Recipe::DiffCheck]
        );
        assert_eq!(
            verifier.classifications["web"],
            ["crates/kanban-web-artifact/src/lib.rs"]
        );

        let manifest = build_plan(
            "main".to_owned(),
            sources(&["crates/kanban-web-artifact/Cargo.toml"]),
        );
        assert_eq!(
            manifest.recipes,
            vec![
                Recipe::RustFull,
                Recipe::DepsCheck,
                Recipe::ToolingCheck,
                Recipe::WebCheck,
                Recipe::DiffCheck,
            ]
        );
        assert_eq!(
            manifest.classifications["web"],
            ["crates/kanban-web-artifact/Cargo.toml"]
        );
    }

    #[test]
    fn protocol_web_artifact_paths_use_web_schema_and_rust_owner_gates() {
        for path in [
            "crates/kanban-protocol/src/web_artifact.rs",
            "crates/kanban-protocol/tests/web_artifact.rs",
        ] {
            let plan = build_plan("main".to_owned(), sources(&[path]));
            assert_eq!(plan.classifications["web"], [path]);
            assert_eq!(plan.classifications["schema"], [path]);
            assert_eq!(plan.classifications["rust"], [path]);
            assert_eq!(
                plan.recipes,
                vec![
                    Recipe::SchemaCheck,
                    Recipe::WebCheck,
                    Recipe::RustFast,
                    Recipe::DiffCheck,
                ],
                "unexpected recipes for {path}"
            );
        }

        let protocol = build_plan(
            "main".to_owned(),
            sources(&["crates/kanban-protocol/src/schema.rs"]),
        );
        assert!(!protocol.classifications.contains_key("web"));
        assert_eq!(
            protocol.recipes,
            vec![Recipe::SchemaCheck, Recipe::RustFast, Recipe::DiffCheck]
        );
    }

    #[test]
    fn node_manifests_use_frontend_gates_without_rust_release_gates() {
        assert_eq!(
            build_plan("main".to_owned(), sources(&["package.json"])).recipes,
            vec![Recipe::WebCheck, Recipe::DesktopCheck, Recipe::DiffCheck]
        );
        assert_eq!(
            build_plan("main".to_owned(), sources(&["apps/desktop/package.json"])).recipes,
            vec![Recipe::DesktopCheck, Recipe::DiffCheck]
        );
    }

    #[test]
    fn root_node_workspace_files_cover_both_frontend_gates() {
        for path in ["package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml"] {
            let plan = build_plan("main".to_owned(), sources(&[path]));
            assert_eq!(
                plan.recipes,
                vec![Recipe::WebCheck, Recipe::DesktopCheck, Recipe::DiffCheck],
                "root workspace file must cover web and desktop for {path}"
            );
            assert_eq!(plan.classifications["web"], [path]);
            assert_eq!(plan.classifications["desktop"], [path]);
        }
    }

    #[test]
    fn root_manifest_and_package_paths_use_full_dependency_and_tooling_gates() {
        for path in [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".github/workflows/ci.yml",
            "xtask/src/package.rs",
            "scripts/test-cli-package-layout.sh",
        ] {
            assert_eq!(
                build_plan("main".to_owned(), sources(&[path])).recipes,
                vec![
                    Recipe::RustFull,
                    Recipe::DepsCheck,
                    Recipe::ToolingCheck,
                    Recipe::DiffCheck,
                ],
                "unexpected recipes for {path}"
            );
        }

        let mixed = build_plan(
            "main".to_owned(),
            sources(&["Cargo.toml", "crates/kanban-service/src/lib.rs"]),
        );
        assert!(!mixed.recipes.contains(&Recipe::RustFast));
    }

    #[test]
    fn recipe_order_is_stable() {
        let paths = [
            "crates/kanban-service/src/lib.rs",
            "xtask/src/affected.rs",
            "apps/desktop/src/App.tsx",
            "schemas/api.json",
        ];
        let first = build_plan("main".to_owned(), sources(&paths)).recipes;
        let second = build_plan("main".to_owned(), sources(&paths)).recipes;
        assert_eq!(first, second);
        assert_eq!(
            first,
            vec![
                Recipe::ToolingCheck,
                Recipe::SchemaCheck,
                Recipe::DesktopCheck,
                Recipe::RustFast,
                Recipe::DiffCheck,
            ]
        );
    }

    #[test]
    fn base_prefix_is_accepted_and_empty_base_is_rejected() {
        assert_eq!(normalise_base("base=HEAD").unwrap(), "HEAD");
        assert_eq!(normalise_base("HEAD").unwrap(), "HEAD");
        assert!(normalise_base("base=").is_err());
    }

    #[test]
    fn json_shape_keeps_sources_separate() {
        let plan = build_plan(
            "HEAD".to_owned(),
            Sources {
                base: vec!["a".to_owned()],
                staged: vec!["b".to_owned()],
                working_tree: vec!["a".to_owned()],
                untracked: vec!["c".to_owned()],
            },
        );
        let value = serde_json::to_value(plan).expect("plan should serialize");
        assert_eq!(value["base"], "HEAD");
        assert_eq!(value["changed_files"], serde_json::json!(["a", "b", "c"]));
        assert_eq!(value["sources"]["base"], serde_json::json!(["a"]));
        assert_eq!(value["sources"]["working_tree"], serde_json::json!(["a"]));
        assert_eq!(
            serde_json::to_value(Recipe::SchemaCheck).expect("recipe should serialize"),
            "schema-contract"
        );
        assert!(value.get("full_gate_recommended").is_none());
        assert!(value.get("full_gate_commands").is_none());
    }

    #[test]
    fn run_failure_is_propagated_without_running_following_recipes() {
        let root = temp_root("run-failure");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).expect("bin should be creatable");
        let log = root.join("recipes.log");
        let just = bin.join("just");
        fs::write(
            &just,
            format!(
                "#!/bin/sh\necho \"$1\" >> \"{}\"\n[ \"$1\" = \"docs-check\" ] && exit 7\nexit 0\n",
                log.display()
            ),
        )
        .expect("fake just should be writable");
        let mut permissions = fs::metadata(&just)
            .expect("fake just metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&just, permissions).expect("fake just should be executable");

        let plan = Plan {
            base: "HEAD".to_owned(),
            changed_files: vec!["README.md".to_owned()],
            classifications: BTreeMap::new(),
            recipes: vec![Recipe::DocsCheck, Recipe::DiffCheck],
            sources: Sources::default(),
        };
        let old_path = env::var_os("PATH");
        let path = match old_path.as_ref() {
            Some(old) => format!("{}:{}", bin.display(), old.to_string_lossy()),
            None => bin.display().to_string(),
        };
        // 测试进程串行运行时修改 PATH，确保 fake just 优先命中。
        unsafe { env::set_var("PATH", path) };
        let result = execute(&root, &plan);
        if let Some(old) = old_path {
            unsafe { env::set_var("PATH", old) };
        } else {
            unsafe { env::remove_var("PATH") };
        }
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(log).expect("recipe log should exist"),
            "docs-check\n"
        );
        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}
