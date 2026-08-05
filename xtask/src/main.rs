use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask 失败: {error}");
        std::process::exit(1);
    }
}

fn run() -> xtask::ToolResult<()> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|argument| argument == "--help" || argument == "-h")
    {
        print_usage();
        if arguments.is_empty() {
            return Err(std::io::Error::other("缺少 command").into());
        }
        return Ok(());
    }

    let group = arguments[0].as_str();
    let subcommand = arguments.get(1).map(String::as_str);
    let (root, options) = parse_options(&arguments[2..])?;

    match (group, subcommand) {
        ("schema", Some(command)) => run_schema(command, &root, options.require_closed),
        ("deps", Some("check")) => run_deps_check(&root),
        ("agents", Some("check")) => run_agents_check(&root),
        ("schema", None) => invalid("schema 缺少子命令"),
        ("deps", Some(command)) => invalid(format!("deps 不支持子命令: {command}")),
        ("agents", Some(command)) => invalid(format!("agents 不支持子命令: {command}")),
        (group, Some(command)) => invalid(format!("未知 command: {group} {command}")),
        (group, None) => invalid(format!("未知 command: {group}")),
    }
}

#[derive(Default)]
struct Options {
    require_closed: bool,
}

fn parse_options(arguments: &[String]) -> xtask::ToolResult<(PathBuf, Options)> {
    let mut root = PathBuf::from(".");
    let mut options = Options::default();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--root" => {
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| std::io::Error::other("--root 缺少路径"))?;
                root = PathBuf::from(value);
            }
            "--require-closed" => options.require_closed = true,
            unknown => return Err(std::io::Error::other(format!("未知参数: {unknown}")).into()),
        }
        index += 1;
    }
    Ok((root, options))
}

fn run_schema(command: &str, root: &Path, require_closed: bool) -> xtask::ToolResult<()> {
    match command {
        "generate" => {
            xtask::write_generated(root)?;
            xtask::check_contract(root, require_closed)?;
            println!(
                "已生成并验证 {} 个 schema roots（未闭合项: {}）",
                kanban_protocol::schema_registry().len(),
                xtask::unfinished_contract_count()
            );
            Ok(())
        }
        "check" => {
            xtask::check_contract(root, require_closed)?;
            println!(
                "schema contract 已通过：{} roots，{} 未闭合项",
                kanban_protocol::schema_registry().len(),
                xtask::unfinished_contract_count()
            );
            Ok(())
        }
        "audit" => {
            xtask::audit_inventory(require_closed)?;
            println!(
                "contract/surface catalog 已通过：{} contract entries，{} surface entries，{} 未闭合项",
                kanban_protocol::operation_inventory().len(),
                kanban_protocol::surface_operation_catalog().len(),
                xtask::unfinished_contract_count()
            );
            Ok(())
        }
        "witnesses" => {
            xtask::audit_inventory(false)?;
            print_adopted_inventory();
            Ok(())
        }
        other => invalid(format!("未知 schema command: {other}")),
    }
}

fn print_adopted_inventory() {
    let adopted = kanban_protocol::operation_inventory()
        .iter()
        .filter(|operation| operation.migration == kanban_protocol::MigrationState::Adopted)
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string_pretty(&adopted).expect("operation inventory must serialize")
    );
}

fn run_deps_check(root: &Path) -> xtask::ToolResult<()> {
    run_checked(
        root,
        "python3",
        ["-B", "scripts/schema_dependency_policy.py"],
    )?;
    run_checked(
        root,
        "scripts/test-schema-cargo-tree.sh",
        std::iter::empty::<&str>(),
    )?;
    run_checked(
        root,
        "python3",
        ["-B", "scripts/check-single-host-dependencies.py"],
    )?;
    Ok(())
}

fn run_agents_check(root: &Path) -> xtask::ToolResult<()> {
    let agents = root.join("AGENTS.md");
    ensure_regular_file(&agents, "根 AGENTS.md")?;
    let line_count = fs::read_to_string(&agents)?.lines().count();
    if !(80..=120).contains(&line_count) {
        return Err(std::io::Error::other(format!(
            "根 AGENTS.md 行数必须在 80..=120 内，实际为 {line_count}"
        ))
        .into());
    }
    check_skill_packages(root)?;
    check_active_maps(root)?;
    println!("ok: AGENTS.md、技能包结构和 active recipe/package map 已通过");
    Ok(())
}

fn check_skill_packages(root: &Path) -> xtask::ToolResult<()> {
    let agents_dir = required_directory(root.join(".agents"), ".agents")?;
    let skills_dir = required_directory(agents_dir.join("skills"), ".agents/skills")?;
    let expected = ["prose", "docs", "check", "commit", "style"];
    let mut actual = fs::read_dir(&skills_dir)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    actual.sort_unstable();
    let mut expected_sorted = expected.map(str::to_owned).to_vec();
    expected_sorted.sort_unstable();
    if actual != expected_sorted {
        return Err(std::io::Error::other(format!(
            ".agents/skills 必须精确包含 {expected_sorted:?}，实际为 {actual:?}"
        ))
        .into());
    }
    for skill in expected {
        let path = skills_dir.join(skill);
        ensure_regular_directory(&path, "技能包目录")?;
        let skill_file = path.join("SKILL.md");
        ensure_regular_file(&skill_file, "技能包 SKILL.md")?;
        let text = fs::read_to_string(&skill_file)?;
        check_skill_contract(skill, &text)?;
        let agents = path.join("agents");
        ensure_regular_directory(&agents, "技能包 agents 目录")?;
        let openai = agents.join("openai.yaml");
        ensure_regular_file(&openai, "技能包 agents/openai.yaml")?;
        check_openai_contract(skill, &openai)?;
    }
    Ok(())
}

fn check_skill_contract(skill: &str, text: &str) -> xtask::ToolResult<()> {
    let Some(frontmatter) = text.strip_prefix("---\n").and_then(|text| {
        text.split_once("\n---\n")
            .map(|(frontmatter, _)| frontmatter)
    }) else {
        return Err(std::io::Error::other(format!("技能包 {skill} 缺少 YAML frontmatter")).into());
    };
    let name = frontmatter
        .lines()
        .find_map(|line| line.strip_prefix("name:").map(str::trim))
        .filter(|value| !value.is_empty());
    if name != Some(skill) {
        return Err(std::io::Error::other(format!(
            "技能包 {skill} frontmatter name 必须精确为 {skill:?}，实际为 {name:?}"
        ))
        .into());
    }
    if !frontmatter.lines().any(|line| {
        line.strip_prefix("description:")
            .is_some_and(|value| !value.trim().is_empty())
    }) {
        return Err(std::io::Error::other(format!(
            "技能包 {skill} frontmatter 缺少非空 description:"
        ))
        .into());
    }
    for heading in ["## 行为契约", "## 验证案例"] {
        if !text.contains(heading) {
            return Err(std::io::Error::other(format!("技能包 {skill} 缺少 {heading}")).into());
        }
    }
    Ok(())
}

fn check_openai_contract(skill: &str, path: &Path) -> xtask::ToolResult<()> {
    let text = fs::read_to_string(path)?;
    if !text.lines().any(|line| line.trim() == "interface:")
        && !text.lines().any(|line| {
            line.trim_start()
                .strip_prefix("interface:")
                .is_some_and(|value| !value.trim().is_empty())
        })
    {
        return Err(
            std::io::Error::other(format!("技能包 {skill} openai.yaml 缺少 interface:")).into(),
        );
    }
    for key in ["display_name:", "short_description:", "default_prompt:"] {
        if !text.lines().any(|line| {
            line.trim_start()
                .strip_prefix(key)
                .is_some_and(|value| !value.trim().is_empty())
        }) {
            return Err(std::io::Error::other(format!(
                "技能包 {skill} openai.yaml 缺少非空 {key}"
            ))
            .into());
        }
    }
    Ok(())
}

fn check_active_maps(root: &Path) -> xtask::ToolResult<()> {
    const ACTIVE_MAPS: &[&str] = &[
        "justfile",
        "scripts/affected-validation.py",
        "scripts/schema_dependency_policy.py",
        "scripts/test-schema-cargo-tree.sh",
        "scripts/test_schema_recipe_witness.py",
    ];
    const STALE_REFERENCES: &[&str] = &[
        "kanban-schema-tool",
        "kanban-sqlite",
        "kanban-local",
        "kanban-context",
        "kanban-entity",
        "kanban-indexer",
        "kanban-labels",
    ];
    for relative in ACTIVE_MAPS {
        let path = root.join(relative);
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        for stale in STALE_REFERENCES {
            if text.contains(stale) {
                return Err(std::io::Error::other(format!(
                    "active map {} 仍引用 stale 名称 {stale}",
                    path.display()
                ))
                .into());
            }
        }
    }
    Ok(())
}

fn required_directory(path: PathBuf, label: &str) -> xtask::ToolResult<PathBuf> {
    if path.is_symlink() || !path.is_dir() {
        return Err(std::io::Error::other(format!(
            "{label} 不存在或不是普通目录: {}",
            path.display()
        ))
        .into());
    }
    Ok(path)
}

fn ensure_regular_directory(path: &Path, label: &str) -> xtask::ToolResult<()> {
    if path.is_symlink() || !path.is_dir() {
        return Err(std::io::Error::other(format!(
            "{label} 不存在或不是普通目录: {}",
            path.display()
        ))
        .into());
    }
    Ok(())
}

fn ensure_regular_file(path: &Path, label: &str) -> xtask::ToolResult<()> {
    if path.is_symlink() || !path.is_file() {
        return Err(std::io::Error::other(format!(
            "{label} 不存在或不是普通文件: {}",
            path.display()
        ))
        .into());
    }
    Ok(())
}

fn run_checked<I, S>(root: &Path, program: &str, arguments: I) -> xtask::ToolResult<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let status = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .status()?;
    if status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "命令失败（{}）: {program}",
        status_description(status)
    ))
    .into())
}

fn status_description(status: ExitStatus) -> String {
    status
        .code()
        .map_or_else(|| "signal".to_owned(), |code| format!("exit={code}"))
}

fn invalid(message: impl Into<String>) -> xtask::ToolResult<()> {
    print_usage();
    Err(std::io::Error::other(message.into()).into())
}

fn print_usage() {
    println!(
        "Usage: xtask <schema generate|check|audit|witnesses|deps check|agents check> [--root PATH] [--require-closed]"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, time::SystemTime};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("xtask-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).expect("temporary root should be creatable");
        path
    }

    fn skill_text(name: &str, headings: (&str, &str)) -> String {
        format!(
            "---\nname: {name}\ndescription: test skill\n---\n\n{}\n\n{}\n",
            headings.0, headings.1
        )
    }

    fn write_skill(root: &Path, name: &str) {
        let skill = root.join(".agents/skills").join(name);
        fs::create_dir_all(skill.join("agents")).expect("skill directory should be creatable");
        fs::write(
            skill.join("SKILL.md"),
            skill_text(name, ("## 行为契约", "## 验证案例")),
        )
        .expect("skill contract should be writable");
        fs::write(
            skill.join("agents/openai.yaml"),
            "interface: conversation\ndisplay_name: Test\nshort_description: Test\ndefault_prompt: Test\n",
        )
        .expect("openai contract should be writable");
    }

    #[test]
    fn skill_contract_requires_chinese_headings_and_exact_name() {
        assert!(
            check_skill_contract(
                "check",
                &skill_text("wrong", ("## 行为契约", "## 验证案例"))
            )
            .is_err()
        );
        assert!(
            check_skill_contract(
                "check",
                &skill_text("check", ("## Behavior contract", "## Evidence cases"))
            )
            .is_err()
        );
        assert!(
            check_skill_contract(
                "check",
                &skill_text("check", ("## 行为契约", "## 验证案例"))
            )
            .is_ok()
        );
    }

    #[test]
    fn agents_check_is_fail_closed_for_skill_layout_and_openai_contract() {
        let root = temp_root("agents");
        fs::write(
            root.join("AGENTS.md"),
            (0..80).map(|_| "line\n").collect::<String>(),
        )
        .expect("root AGENTS should be writable");
        assert!(run_agents_check(&root).is_err());

        for skill in ["prose", "docs", "check", "commit", "style"] {
            write_skill(&root, skill);
        }
        assert!(run_agents_check(&root).is_ok());

        write_skill(&root, "extra");
        assert!(run_agents_check(&root).is_err());
        fs::remove_dir_all(root.join(".agents/skills/extra"))
            .expect("extra skill should be removable");
        fs::remove_file(root.join(".agents/skills/check/agents/openai.yaml"))
            .expect("openai contract should be removable");
        assert!(run_agents_check(&root).is_err());

        fs::remove_dir_all(root).expect("temporary root should be removable");
    }
}
